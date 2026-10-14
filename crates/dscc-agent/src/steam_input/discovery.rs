use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};

#[cfg(target_os = "windows")]
use crate::windows_process_running;

use super::{
    parser::parse_steam_input_layout, path_safety::sanitized_steam_path, SteamInputStatus,
};

const STEAM_INPUT_LAYOUT_SCAN_LIMIT: usize = 96;

pub(crate) fn discover_steam_input_status() -> SteamInputStatus {
    let steam_root = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("userdata").is_dir() || path.join("steam.exe").is_file());
    let running = steam_root.is_some() && steam_process_running();
    let mut warnings = Vec::new();
    let mut layouts = Vec::new();

    if let Some(root) = steam_root.as_ref() {
        let mut files = Vec::new();
        collect_steam_controller_config_files(root, &mut files);
        for file in files.into_iter().take(16) {
            match fs::read_to_string(&file) {
                Ok(contents) => {
                    if let Some(layout) = parse_steam_input_layout(root, &file, &contents) {
                        layouts.push(layout);
                    }
                }
                Err(error) => warnings.push(
                    format!(
                        "Steam Input layout could not be read: {}",
                        sanitized_steam_path(root, &file)
                            .unwrap_or_else(|| "userdata/<redacted>".to_string())
                    ) + &format!(" ({error})"),
                ),
            }
        }
    } else {
        warnings.push("Steam install was not found in standard user locations.".to_string());
    }

    if running && layouts.is_empty() {
        warnings.push(
            "Steam is running, but no local controller layout VDF files were discovered."
                .to_string(),
        );
    }

    SteamInputStatus {
        running,
        available: steam_root.is_some(),
        steam_path: steam_root.as_ref().map(|path| path.display().to_string()),
        layouts,
        warnings,
    }
}

pub(crate) async fn discover_steam_input_status_async() -> SteamInputStatus {
    tokio::task::spawn_blocking(discover_steam_input_status)
        .await
        .unwrap_or_else(|error| SteamInputStatus {
            running: false,
            available: false,
            steam_path: None,
            layouts: Vec::new(),
            warnings: vec![format!("Steam Input discovery task failed: {error}")],
        })
}

pub(crate) fn pending_steam_input_status() -> SteamInputStatus {
    SteamInputStatus {
        running: false,
        available: false,
        steam_path: None,
        layouts: Vec::new(),
        warnings: vec!["Steam Input discovery is warming in the background.".to_string()],
    }
}

pub(crate) fn steam_input_discovery_pending(status: &SteamInputStatus) -> bool {
    status
        .warnings
        .iter()
        .any(|warning| warning.contains("warming in the background"))
}

fn steam_process_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows_process_running("steam.exe")
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("pgrep")
            .args(["-x", "steam"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

pub(crate) fn steam_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(override_root) = std::env::var_os("DSCC_STEAM_ROOT") {
        candidates.push(PathBuf::from(override_root));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(program_files_x86).join("Steam"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Steam"));
        }
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local_app_data).join("Steam"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            candidates.push(home.join(".steam/steam"));
            candidates.push(home.join(".local/share/Steam"));
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates
}

pub(super) fn collect_steam_controller_config_files(steam_root: &FsPath, files: &mut Vec<PathBuf>) {
    let userdata_root = steam_root.join("userdata");
    for user_dir in numeric_child_dirs(&userdata_root, 8) {
        collect_steam_controller_config_files_bounded(&user_dir.join("config"), 0, 3, files);
        for app_dir in numeric_child_dirs(&user_dir, 96) {
            collect_steam_controller_config_files_bounded(&app_dir.join("remote"), 0, 3, files);
            if files.len() >= STEAM_INPUT_LAYOUT_SCAN_LIMIT {
                break;
            }
        }
        if files.len() >= STEAM_INPUT_LAYOUT_SCAN_LIMIT {
            break;
        }
    }

    let controller_configs = steam_root
        .join("steamapps")
        .join("common")
        .join("Steam Controller Configs");
    for user_dir in numeric_child_dirs(&controller_configs, 8) {
        collect_steam_controller_config_files_bounded(&user_dir.join("config"), 0, 5, files);
        if files.len() >= STEAM_INPUT_LAYOUT_SCAN_LIMIT {
            break;
        }
    }

    files.sort();
    files.dedup();
}

pub(crate) fn numeric_child_dirs(root: &FsPath, max_dirs: usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        if dirs.len() >= max_dirs {
            break;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.chars().all(|ch| ch.is_ascii_digit()))
        {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs
}

fn collect_steam_controller_config_files_bounded(
    root: &FsPath,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<PathBuf>,
) {
    if depth > max_depth || files.len() >= STEAM_INPUT_LAYOUT_SCAN_LIMIT || !root.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        if files.len() >= STEAM_INPUT_LAYOUT_SCAN_LIMIT {
            return;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_steam_controller_config_files_bounded(&path, depth + 1, max_depth, files);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let file_name = file_name.to_ascii_lowercase();
        if file_name.ends_with(".vdf")
            && (file_name.contains("controller_config")
                || (file_name.starts_with("controller_")
                    && !file_name.starts_with("controller_base")))
            && fs::metadata(&path)
                .map(|metadata| metadata.len() <= 256 * 1024)
                .unwrap_or(false)
        {
            files.push(path);
        }
    }
}
