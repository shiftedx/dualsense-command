use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path as FsPath, PathBuf},
};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use crate::windows_process_running;

const STEAM_INPUT_LAYOUT_SCAN_LIMIT: usize = 96;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputStatus {
    pub running: bool,
    pub available: bool,
    pub steam_path: Option<String>,
    pub layouts: Vec<SteamInputLayout>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputLayout {
    pub app_id: Option<String>,
    pub title: String,
    pub controller_type: Option<String>,
    pub controller_label: Option<String>,
    pub source: String,
    pub binding_count: usize,
    pub bindings: Vec<SteamInputBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputBinding {
    pub input: String,
    pub input_id: String,
    pub binding: String,
    pub raw_binding: String,
    pub kind: String,
    pub source: Option<String>,
    pub source_mode: Option<String>,
    pub activator: Option<String>,
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputBindingWriteRequest {
    pub layout_source: String,
    pub app_id: Option<String>,
    pub input_id: String,
    pub group_id: Option<String>,
    pub activator: Option<String>,
    pub raw_binding: String,
    pub profile_name: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputBindingWriteResponse {
    pub accepted: bool,
    pub message: String,
    pub dry_run: bool,
    pub source: String,
    pub target_path: String,
    pub backup_path: Option<String>,
    pub binding: SteamInputBinding,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputPaddlePresetRequest {
    pub layout_source: String,
    pub app_id: Option<String>,
    pub left_key: Option<String>,
    pub right_key: Option<String>,
    pub profile_name: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputPaddlePresetResponse {
    pub accepted: bool,
    pub message: String,
    pub dry_run: bool,
    pub source: String,
    pub target_path: String,
    pub backup_path: Option<String>,
    pub paddles: Vec<SteamInputPaddlePresetPaddleResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamInputPaddlePresetPaddleResult {
    pub paddle: String,
    pub input_id: String,
    pub key: String,
    pub raw_binding: String,
    pub changed: bool,
    pub binding: SteamInputBinding,
    pub message: String,
}

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

fn collect_steam_controller_config_files(steam_root: &FsPath, files: &mut Vec<PathBuf>) {
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

pub(crate) fn parse_steam_input_layout(
    steam_root: &FsPath,
    file: &FsPath,
    contents: &str,
) -> Option<SteamInputLayout> {
    if !contents.contains("controller_mappings") {
        return None;
    }

    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;
    let mut title = None;
    let mut controller_type = None;
    let mut group_id: Option<String> = None;
    let mut group_mode: Option<String> = None;
    let mut group_sources: BTreeMap<String, String> = BTreeMap::new();
    let mut parsed_bindings = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
            continue;
        }
        if line == "}" {
            if let Some(block) = stack.pop() {
                if block == "group" {
                    group_id = None;
                    group_mode = None;
                }
            }
            continue;
        }

        let tokens = quoted_tokens(line);
        match tokens.as_slice() {
            [key] => pending_block = Some(key.to_string()),
            [key, value] => {
                pending_block = None;
                match key.as_str() {
                    "title" if stack.iter().any(|item| item == "english") => {
                        title = Some(clean_steam_layout_title(value))
                    }
                    "title" if !stack.iter().any(|item| item == "localization") => {
                        title = Some(clean_steam_layout_title(value))
                    }
                    "controller_type" => controller_type = Some(value.to_string()),
                    "id" | "ID" if stack.last().is_some_and(|item| item == "group") => {
                        group_id = Some(value.to_string())
                    }
                    "mode" if stack.last().is_some_and(|item| item == "group") => {
                        group_mode = Some(value.to_string())
                    }
                    _ if stack
                        .last()
                        .is_some_and(|item| item == "group_source_bindings") =>
                    {
                        let mut parts = value.split_whitespace();
                        let source = parts.next();
                        let state = parts.next();
                        if state == Some("active") {
                            if let Some(source) = source {
                                group_sources.insert(key.to_string(), source.to_string());
                            }
                        }
                    }
                    "binding" => {
                        if let Some(input_id) = steam_input_from_stack(&stack) {
                            parsed_bindings.push(ParsedSteamInputBinding {
                                input_id,
                                raw_binding: value.to_string(),
                                activator: steam_activator_from_stack(&stack),
                                group_id: group_id.clone(),
                                source_mode: group_mode.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    if parsed_bindings.is_empty() && title.is_none() {
        return None;
    }

    let has_group_source_bindings = !group_sources.is_empty();
    let mut bindings = parsed_bindings
        .into_iter()
        .filter_map(|binding| {
            if has_group_source_bindings
                && binding
                    .group_id
                    .as_deref()
                    .is_some_and(|id| !group_sources.contains_key(id))
            {
                return None;
            }
            let source = binding
                .group_id
                .as_deref()
                .and_then(|id| group_sources.get(id))
                .cloned();
            let input = friendly_steam_input(&binding.input_id, source.as_deref());
            let raw_binding = binding.raw_binding;
            let display_binding = friendly_steam_binding(&raw_binding);
            let binding_kind = steam_binding_kind(&raw_binding);
            Some(SteamInputBinding {
                input,
                input_id: binding.input_id,
                binding: display_binding,
                raw_binding,
                kind: binding_kind,
                source: source.as_deref().map(friendly_steam_source),
                source_mode: binding
                    .source_mode
                    .as_deref()
                    .map(friendly_steam_source_mode),
                activator: binding.activator.as_deref().map(friendly_steam_activator),
                group_id: binding.group_id,
            })
        })
        .collect::<Vec<_>>();
    bindings.truncate(64);
    let source = sanitized_steam_path(steam_root, file).unwrap_or_else(|| {
        file.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("controller_config.vdf")
            .to_string()
    });

    Some(SteamInputLayout {
        app_id: steam_app_id_from_path(file),
        title: title.unwrap_or_else(|| "Steam Input Layout".to_string()),
        controller_label: controller_type
            .as_deref()
            .map(friendly_steam_controller_type),
        controller_type,
        source,
        binding_count: bindings.len(),
        bindings,
    })
}

struct ParsedSteamInputBinding {
    input_id: String,
    raw_binding: String,
    activator: Option<String>,
    group_id: Option<String>,
    source_mode: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SteamInputWriteFailure {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl SteamInputWriteFailure {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    fn io(message: impl Into<String>, error: io::Error) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{}: {error}", message.into()),
        )
    }
}

pub(crate) fn write_steam_input_binding(
    request: SteamInputBindingWriteRequest,
) -> Result<SteamInputBindingWriteResponse, SteamInputWriteFailure> {
    if request.layout_source.trim().is_empty() {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam layout source is required.",
        ));
    }
    if request.input_id.trim().is_empty() {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam input id is required.",
        ));
    }

    let raw_binding = normalize_steam_raw_binding(&request.raw_binding)
        .map_err(SteamInputWriteFailure::bad_request)?;
    let (steam_root, target_path) =
        resolve_steam_input_layout_path(&request.layout_source, request.app_id.as_deref())?;
    let metadata = fs::metadata(&target_path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout metadata could not be read", error)
    })?;
    if metadata.len() > 256 * 1024 {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam Input layout is larger than DSCC's guarded write limit.",
        ));
    }

    let contents = fs::read_to_string(&target_path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout could not be read", error)
    })?;
    let next_contents = replace_steam_binding_value(&contents, &request, &raw_binding)?
        .map(|updated| mark_dscc_steam_profile_metadata(&updated, request.profile_name.as_deref()))
        .unwrap_or_else(|| {
            mark_dscc_steam_profile_metadata(&contents, request.profile_name.as_deref())
        });

    let layout =
        parse_steam_input_layout(&steam_root, &target_path, &next_contents).ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout could not be parsed after the binding update.",
            )
        })?;
    let binding = layout
        .bindings
        .iter()
        .find(|binding| steam_binding_matches_write_request(binding, &request))
        .cloned()
        .ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout was updated, but the target binding could not be re-read.",
            )
        })?;

    let changed = contents != next_contents;
    let backup_path = if !request.dry_run && changed {
        Some(backup_and_write_steam_input_layout(
            &target_path,
            &next_contents,
        )?)
    } else {
        None
    };

    let source = sanitized_steam_path(&steam_root, &target_path)
        .unwrap_or_else(|| target_path.display().to_string());
    let action = if request.dry_run {
        "Validated"
    } else if changed {
        "Saved"
    } else {
        "Already current"
    };

    Ok(SteamInputBindingWriteResponse {
        accepted: true,
        message: format!("{action} Steam Input binding for {}.", binding.input),
        dry_run: request.dry_run,
        source,
        target_path: target_path.display().to_string(),
        backup_path: backup_path.map(|path| path.display().to_string()),
        binding,
        warnings: Vec::new(),
    })
}

pub(crate) const STEAM_EDGE_BACK_LEFT_INPUT_ID: &str = "button_back_left";
pub(crate) const STEAM_EDGE_BACK_RIGHT_INPUT_ID: &str = "button_back_right";
const DEFAULT_FORZA_DOWN_SHIFT_KEY: &str = "Q";
const DEFAULT_FORZA_UP_SHIFT_KEY: &str = "E";

pub(crate) fn write_steam_input_paddle_preset(
    request: SteamInputPaddlePresetRequest,
) -> Result<SteamInputPaddlePresetResponse, SteamInputWriteFailure> {
    if request.layout_source.trim().is_empty() {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam layout source is required.",
        ));
    }

    let left_key = normalize_steam_keyboard_key(
        request.left_key.as_deref(),
        DEFAULT_FORZA_DOWN_SHIFT_KEY,
        "left paddle",
    )
    .map_err(SteamInputWriteFailure::bad_request)?;
    let right_key = normalize_steam_keyboard_key(
        request.right_key.as_deref(),
        DEFAULT_FORZA_UP_SHIFT_KEY,
        "right paddle",
    )
    .map_err(SteamInputWriteFailure::bad_request)?;
    let left_raw_binding = steam_key_press_binding(&left_key);
    let right_raw_binding = steam_key_press_binding(&right_key);
    normalize_steam_raw_binding(&left_raw_binding).map_err(SteamInputWriteFailure::bad_request)?;
    normalize_steam_raw_binding(&right_raw_binding).map_err(SteamInputWriteFailure::bad_request)?;

    let (steam_root, target_path) =
        resolve_steam_input_layout_path(&request.layout_source, request.app_id.as_deref())?;
    let metadata = fs::metadata(&target_path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout metadata could not be read", error)
    })?;
    if metadata.len() > 256 * 1024 {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam Input layout is larger than DSCC's guarded write limit.",
        ));
    }

    let contents = fs::read_to_string(&target_path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout could not be read", error)
    })?;
    let layout =
        parse_steam_input_layout(&steam_root, &target_path, &contents).ok_or_else(|| {
            SteamInputWriteFailure::conflict("Steam Input layout could not be parsed.")
        })?;
    ensure_dualsense_edge_steam_layout(&layout)?;

    let left_target = steam_edge_paddle_binding(&layout, STEAM_EDGE_BACK_LEFT_INPUT_ID)?;
    let right_target = steam_edge_paddle_binding(&layout, STEAM_EDGE_BACK_RIGHT_INPUT_ID)?;
    let left_request = steam_paddle_binding_write_request(
        &request,
        STEAM_EDGE_BACK_LEFT_INPUT_ID,
        left_target,
        &left_raw_binding,
    );
    let right_request = steam_paddle_binding_write_request(
        &request,
        STEAM_EDGE_BACK_RIGHT_INPUT_ID,
        right_target,
        &right_raw_binding,
    );

    let left_updated = replace_steam_binding_value(&contents, &left_request, &left_raw_binding)?
        .ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout changed before the left paddle preset could be written.",
            )
        })?;
    let left_changed = left_updated != contents;
    let right_updated =
        replace_steam_binding_value(&left_updated, &right_request, &right_raw_binding)?
            .ok_or_else(|| {
                SteamInputWriteFailure::conflict(
                    "Steam Input layout changed before the right paddle preset could be written.",
                )
            })?;
    let right_changed = right_updated != left_updated;
    let next_contents =
        mark_dscc_steam_profile_metadata(&right_updated, request.profile_name.as_deref());

    let updated_layout = parse_steam_input_layout(&steam_root, &target_path, &next_contents)
        .ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout could not be parsed after the paddle preset update.",
            )
        })?;
    let left_binding = updated_layout
        .bindings
        .iter()
        .find(|binding| steam_binding_matches_write_request(binding, &left_request))
        .cloned()
        .ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout was updated, but the left paddle binding could not be re-read.",
            )
        })?;
    let right_binding = updated_layout
        .bindings
        .iter()
        .find(|binding| steam_binding_matches_write_request(binding, &right_request))
        .cloned()
        .ok_or_else(|| {
            SteamInputWriteFailure::conflict(
                "Steam Input layout was updated, but the right paddle binding could not be re-read.",
            )
        })?;

    let changed = contents != next_contents;
    let backup_path = if !request.dry_run && changed {
        Some(backup_and_write_steam_input_layout(
            &target_path,
            &next_contents,
        )?)
    } else {
        None
    };
    let source = sanitized_steam_path(&steam_root, &target_path)
        .unwrap_or_else(|| target_path.display().to_string());
    let action = if request.dry_run {
        "Validated"
    } else if changed {
        "Saved"
    } else {
        "Already current"
    };

    Ok(SteamInputPaddlePresetResponse {
        accepted: true,
        message: format!("{action} DualSense Edge paddle shift preset."),
        dry_run: request.dry_run,
        source,
        target_path: target_path.display().to_string(),
        backup_path: backup_path.map(|path| path.display().to_string()),
        paddles: vec![
            SteamInputPaddlePresetPaddleResult {
                paddle: "Back Left".to_string(),
                input_id: STEAM_EDGE_BACK_LEFT_INPUT_ID.to_string(),
                key: left_key,
                raw_binding: left_raw_binding,
                changed: left_changed,
                binding: left_binding,
                message: "Back Left paddle mapped to downshift key.".to_string(),
            },
            SteamInputPaddlePresetPaddleResult {
                paddle: "Back Right".to_string(),
                input_id: STEAM_EDGE_BACK_RIGHT_INPUT_ID.to_string(),
                key: right_key,
                raw_binding: right_raw_binding,
                changed: right_changed,
                binding: right_binding,
                message: "Back Right paddle mapped to upshift key.".to_string(),
            },
        ],
        warnings: vec![
            "Steam Input paddle presets are PC-local and do not change portable DualSense Edge onboard profiles."
                .to_string(),
        ],
    })
}

fn steam_paddle_binding_write_request(
    preset: &SteamInputPaddlePresetRequest,
    input_id: &str,
    target: &SteamInputBinding,
    raw_binding: &str,
) -> SteamInputBindingWriteRequest {
    SteamInputBindingWriteRequest {
        layout_source: preset.layout_source.clone(),
        app_id: preset.app_id.clone(),
        input_id: input_id.to_string(),
        group_id: target.group_id.clone(),
        activator: target.activator.clone(),
        raw_binding: raw_binding.to_string(),
        profile_name: preset.profile_name.clone(),
        dry_run: preset.dry_run,
    }
}

pub(crate) fn ensure_dualsense_edge_steam_layout(
    layout: &SteamInputLayout,
) -> Result<(), SteamInputWriteFailure> {
    let controller_type = layout.controller_type.as_deref().unwrap_or_default();
    let has_edge_paddles = layout
        .bindings
        .iter()
        .any(|binding| binding.input_id == STEAM_EDGE_BACK_LEFT_INPUT_ID)
        && layout
            .bindings
            .iter()
            .any(|binding| binding.input_id == STEAM_EDGE_BACK_RIGHT_INPUT_ID);
    if !controller_type.to_ascii_lowercase().contains("edge") && !has_edge_paddles {
        return Err(SteamInputWriteFailure::bad_request(
            "The paddle preset requires a DualSense Edge Steam Input layout.",
        ));
    }
    Ok(())
}

pub(crate) fn steam_edge_paddle_binding<'a>(
    layout: &'a SteamInputLayout,
    input_id: &str,
) -> Result<&'a SteamInputBinding, SteamInputWriteFailure> {
    let mut matches = layout
        .bindings
        .iter()
        .filter(|binding| binding.input_id == input_id);
    let Some(first) = matches.next() else {
        let paddle = friendly_steam_input(input_id, Some("switch"));
        return Err(SteamInputWriteFailure::not_found(format!(
            "DualSense Edge {paddle} binding was not found in this Steam Input layout. Open Steam's configurator once and map the Edge paddles before applying the preset."
        )));
    };
    let preferred = std::iter::once(first)
        .chain(matches)
        .find(|binding| {
            binding.group_id.is_some()
                && binding.activator.as_deref().unwrap_or("Full Press") == "Full Press"
                && binding.source.as_deref().unwrap_or("Switches") == "Switches"
        })
        .unwrap_or(first);
    if preferred.group_id.is_none() {
        return Err(SteamInputWriteFailure::conflict(
            "DualSense Edge paddle binding is missing Steam group identity, so DSCC will not edit it.",
        ));
    }
    Ok(preferred)
}

fn normalize_steam_keyboard_key(
    value: Option<&str>,
    fallback: &str,
    label: &str,
) -> Result<String, String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    let key = value
        .unwrap_or(fallback)
        .replace(' ', "_")
        .to_ascii_uppercase();
    if key.len() > 32
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return Err(format!(
            "{label} key is not a supported Steam keyboard key."
        ));
    }
    Ok(key)
}

fn steam_key_press_binding(key: &str) -> String {
    format!("key_press {key}, , ")
}

fn resolve_steam_input_layout_path(
    layout_source: &str,
    app_id: Option<&str>,
) -> Result<(PathBuf, PathBuf), SteamInputWriteFailure> {
    let roots = steam_root_candidates();
    if roots.is_empty() {
        return Err(SteamInputWriteFailure::not_found(
            "Steam install path was not found.",
        ));
    }

    for root in roots {
        if !root.is_dir() {
            continue;
        }

        let mut files = Vec::new();
        collect_steam_controller_config_files(&root, &mut files);
        for file in files {
            if app_id
                .is_some_and(|expected| steam_app_id_from_path(&file).as_deref() != Some(expected))
            {
                continue;
            }
            if sanitized_steam_path(&root, &file).as_deref() == Some(layout_source) {
                return validated_steam_input_layout_path(root, file);
            }
        }

        if !layout_source.contains('<') {
            let candidate = if FsPath::new(layout_source).is_absolute() {
                PathBuf::from(layout_source)
            } else {
                root.join(layout_source)
            };
            if candidate.is_file()
                && app_id.is_none_or(|expected| {
                    steam_app_id_from_path(&candidate).as_deref() == Some(expected)
                })
            {
                return validated_steam_input_layout_path(root, candidate);
            }
        }
    }

    Err(SteamInputWriteFailure::not_found(
        "Steam Input layout file was not found. Open the Steam configurator once for this game and controller.",
    ))
}

pub(crate) fn validated_steam_input_layout_path(
    steam_root: PathBuf,
    path: PathBuf,
) -> Result<(PathBuf, PathBuf), SteamInputWriteFailure> {
    let canonical_root = fs::canonicalize(&steam_root).map_err(|error| {
        SteamInputWriteFailure::io("Steam install path could not be canonicalized", error)
    })?;
    let canonical_path = fs::canonicalize(&path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout path could not be canonicalized", error)
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(SteamInputWriteFailure::bad_request(
            "Steam Input layout must live inside the Steam install path.",
        ));
    }
    let file_name = canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if canonical_path.extension().and_then(|ext| ext.to_str()) != Some("vdf")
        || !file_name.starts_with("controller_")
        || file_name.starts_with("controller_base")
    {
        return Err(SteamInputWriteFailure::bad_request(
            "DSCC only writes controller_*.vdf Steam Input layout files.",
        ));
    }
    Ok((canonical_root, canonical_path))
}

fn normalize_steam_raw_binding(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Steam binding cannot be empty.".to_string());
    }
    if trimmed.len() > 128
        || trimmed
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '"' | '{' | '}'))
    {
        return Err("Steam binding contains unsupported characters.".to_string());
    }

    let Some((kind, rest)) = trimmed.split_once(char::is_whitespace) else {
        return Err("Steam binding must include a binding kind and target.".to_string());
    };
    let kind = kind.trim();
    if !matches!(
        kind,
        "xinput_button" | "key_press" | "mouse_button" | "mouse_wheel"
    ) {
        return Err(format!("Steam binding kind '{kind}' is not writable yet."));
    }
    let target = rest.split(',').next().unwrap_or_default().trim();
    if target.is_empty()
        || !target
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' '))
    {
        return Err("Steam binding target is not valid.".to_string());
    }

    if trimmed.contains(',') {
        let mut normalized = trimmed.to_string();
        if normalized.ends_with(", ,") {
            normalized.push(' ');
        }
        Ok(normalized)
    } else {
        Ok(format!("{trimmed}, , "))
    }
}

pub(crate) fn replace_steam_binding_value(
    contents: &str,
    request: &SteamInputBindingWriteRequest,
    raw_binding: &str,
) -> Result<Option<String>, SteamInputWriteFailure> {
    let requested_activator = raw_steam_activator(request.activator.as_deref());
    let escaped_binding = escape_vdf_value(raw_binding);
    let newline = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;
    let mut group_id: Option<String> = None;
    let mut updated = false;
    let mut output = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        let mut replacement: Option<String> = None;

        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
        } else if line == "}" {
            if let Some(block) = stack.pop() {
                if block == "group" {
                    group_id = None;
                }
            }
        } else {
            let tokens = quoted_tokens(line);
            match tokens.as_slice() {
                [key] => pending_block = Some(key.to_string()),
                [key, value] => {
                    pending_block = None;
                    if matches!(key.as_str(), "id" | "ID")
                        && stack.last().is_some_and(|item| item == "group")
                    {
                        group_id = Some(value.to_string());
                    } else if key == "binding"
                        && !updated
                        && stack.last().is_some_and(|item| item == "bindings")
                        && steam_binding_stack_matches_request(
                            &stack,
                            group_id.as_deref(),
                            request,
                            requested_activator.as_deref(),
                        )
                    {
                        let indent: String = raw_line
                            .chars()
                            .take_while(|ch| ch.is_whitespace())
                            .collect();
                        replacement = Some(format!("{indent}\"binding\" \"{escaped_binding}\""));
                        updated = true;
                    }
                }
                _ => pending_block = None,
            }
        }

        output.push(replacement.unwrap_or_else(|| raw_line.to_string()));
    }

    if !updated {
        return Err(SteamInputWriteFailure::not_found(
            "The selected Steam Input binding was not found in the layout file.",
        ));
    }

    let mut result = output.join(newline);
    if contents.ends_with('\n') {
        result.push_str(newline);
    }

    Ok((result != contents).then_some(result))
}

fn steam_binding_stack_matches_request(
    stack: &[String],
    current_group_id: Option<&str>,
    request: &SteamInputBindingWriteRequest,
    requested_activator: Option<&str>,
) -> bool {
    if request
        .group_id
        .as_deref()
        .is_some_and(|expected| current_group_id != Some(expected))
    {
        return false;
    }
    if steam_input_from_stack(stack).as_deref() != Some(request.input_id.as_str()) {
        return false;
    }
    requested_activator
        .is_none_or(|expected| steam_activator_from_stack(stack).as_deref() == Some(expected))
}

fn steam_binding_matches_write_request(
    binding: &SteamInputBinding,
    request: &SteamInputBindingWriteRequest,
) -> bool {
    if binding.input_id != request.input_id {
        return false;
    }
    if request
        .group_id
        .as_deref()
        .is_some_and(|expected| binding.group_id.as_deref() != Some(expected))
    {
        return false;
    }
    let expected_activator = raw_steam_activator(request.activator.as_deref());
    expected_activator.is_none_or(|expected| {
        raw_steam_activator(binding.activator.as_deref()).as_deref() == Some(expected.as_str())
    })
}

fn raw_steam_activator(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "Full Press" | "Full_Press" => "Full_Press",
            "Soft Pull" | "Soft Press" | "Soft_Press" => "Soft_Press",
            "Long Press" | "Long_Press" => "Long_Press",
            "Double Press" | "Double_Press" => "Double_Press",
            "Start Press" | "Start_Press" => "Start_Press",
            "Release" | "Release Press" | "Release_Press" => "Release_Press",
            "Chord" | "Chord Press" | "Chord_Press" => "Chord_Press",
            other => other,
        }
        .to_string(),
    )
}

fn escape_vdf_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn mark_dscc_steam_profile_metadata(
    contents: &str,
    profile_name: Option<&str>,
) -> String {
    let Some(profile_name) = profile_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return contents.to_string();
    };
    let dscc_title = format!(
        "DSCC / {}",
        profile_name.chars().take(64).collect::<String>()
    );
    let description = "Edited by DualSense Command Center";
    let newline = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;
    let mut title_updated = false;
    let mut description_updated = false;
    let mut revision_updated = false;
    let mut output = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        let mut replacement = None;
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
        } else if line == "}" {
            stack.pop();
        } else {
            let tokens = quoted_tokens(line);
            match tokens.as_slice() {
                [key] => pending_block = Some(key.to_string()),
                [key, value] => {
                    pending_block = None;
                    if stack.len() == 1
                        && stack
                            .last()
                            .is_some_and(|item| item == "controller_mappings")
                    {
                        let indent: String = raw_line
                            .chars()
                            .take_while(|ch| ch.is_whitespace())
                            .collect();
                        match key.as_str() {
                            "title" if !title_updated => {
                                replacement = Some(format!(
                                    "{indent}\"title\" \"{}\"",
                                    escape_vdf_value(&dscc_title)
                                ));
                                title_updated = true;
                            }
                            "description" if !description_updated => {
                                replacement = Some(format!(
                                    "{indent}\"description\" \"{}\"",
                                    escape_vdf_value(description)
                                ));
                                description_updated = true;
                            }
                            "revision" if !revision_updated => {
                                if let Ok(value) = value.parse::<u32>() {
                                    replacement =
                                        Some(format!("{indent}\"revision\" \"{}\"", value + 1));
                                    revision_updated = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => pending_block = None,
            }
        }
        output.push(replacement.unwrap_or_else(|| raw_line.to_string()));
    }

    let mut result = output.join(newline);
    if contents.ends_with('\n') {
        result.push_str(newline);
    }
    result
}

fn backup_and_write_steam_input_layout(
    target_path: &FsPath,
    contents: &str,
) -> Result<PathBuf, SteamInputWriteFailure> {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("controller_input.vdf");
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup_path = target_path.with_file_name(format!("{file_name}.dscc-backup-{stamp}"));
    fs::copy(target_path, &backup_path).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout backup could not be created", error)
    })?;
    fs::write(target_path, contents).map_err(|error| {
        SteamInputWriteFailure::io("Steam Input layout could not be written", error)
    })?;
    Ok(backup_path)
}

pub(crate) fn quoted_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut token = String::new();
        while let Some(next) = chars.next() {
            if next == '"' {
                break;
            }
            if next == '\\' {
                if let Some(escaped) = chars.next() {
                    token.push(escaped);
                }
            } else {
                token.push(next);
            }
        }
        tokens.push(token);
    }
    tokens
}

fn steam_input_from_stack(stack: &[String]) -> Option<String> {
    stack
        .iter()
        .rev()
        .find(|item| {
            !matches!(
                item.as_str(),
                "bindings"
                    | "activators"
                    | "disabled_activators"
                    | "inputs"
                    | "group"
                    | "settings"
                    | "group_source_bindings"
                    | "preset"
                    | "localization"
                    | "english"
                    | "Full_Press"
                    | "Soft_Press"
                    | "Long_Press"
                    | "Double_Press"
                    | "Start_Press"
                    | "Release_Press"
                    | "Chord_Press"
            )
        })
        .cloned()
}

fn steam_activator_from_stack(stack: &[String]) -> Option<String> {
    stack.iter().rev().find_map(|item| {
        matches!(
            item.as_str(),
            "Full_Press"
                | "Soft_Press"
                | "Long_Press"
                | "Double_Press"
                | "Start_Press"
                | "Release_Press"
                | "Chord_Press"
        )
        .then(|| item.clone())
    })
}

fn friendly_steam_input(input: &str, source: Option<&str>) -> String {
    match input {
        "button_a" => "Cross".to_string(),
        "button_b" => "Circle".to_string(),
        "button_x" => "Square".to_string(),
        "button_y" => "Triangle".to_string(),
        "dpad_north" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Up".to_string()
        }
        "dpad_south" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Down".to_string()
        }
        "dpad_east" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Right".to_string()
        }
        "dpad_west" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Left".to_string()
        }
        "dpad_north" => "D-Pad Up".to_string(),
        "dpad_south" => "D-Pad Down".to_string(),
        "dpad_east" => "D-Pad Right".to_string(),
        "dpad_west" => "D-Pad Left".to_string(),
        "button_escape" => "Options".to_string(),
        "button_menu" => "Create".to_string(),
        "button_back_left" => "Back Left".to_string(),
        "button_back_right" => "Back Right".to_string(),
        "button_back_left_upper" => "Fn Left".to_string(),
        "button_back_right_upper" => "Fn Right".to_string(),
        "click" => match source {
            Some("left_trigger") => "L2 Full Pull".to_string(),
            Some("right_trigger") => "R2 Full Pull".to_string(),
            Some("joystick") => "Left Stick Click".to_string(),
            Some("right_joystick") => "Right Stick Click".to_string(),
            Some("left_trackpad") => "Left Touchpad Press".to_string(),
            Some("right_trackpad") => "Right Touchpad Press".to_string(),
            Some("gyro") => "Gyro".to_string(),
            _ => "Click".to_string(),
        },
        "edge" => match source {
            Some("left_trigger") => "L2 Soft Pull".to_string(),
            Some("right_trigger") => "R2 Soft Pull".to_string(),
            _ => "Soft Pull".to_string(),
        },
        "dpad_up" => "Swipe Up".to_string(),
        "dpad_down" => "Swipe Down".to_string(),
        "dpad_left" => "Swipe Left".to_string(),
        "dpad_right" => "Swipe Right".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn friendly_steam_binding(binding: &str) -> String {
    let binding = binding.trim();
    let Some((kind, rest)) = binding.split_once(' ') else {
        return title_case_words(&binding.replace('_', " "));
    };
    let target = rest.split(',').next().unwrap_or(rest).trim();
    match kind {
        "xinput_button" => match target.to_ascii_lowercase().as_str() {
            "a" => "A Button".to_string(),
            "b" => "B Button".to_string(),
            "x" => "X Button".to_string(),
            "y" => "Y Button".to_string(),
            "dpad_up" | "dpad_north" => "DPad Up".to_string(),
            "dpad_down" | "dpad_south" => "DPad Down".to_string(),
            "dpad_left" | "dpad_west" => "DPad Left".to_string(),
            "dpad_right" | "dpad_east" => "DPad Right".to_string(),
            "start" => "Start".to_string(),
            "select" | "back" => "Select".to_string(),
            "shoulder_left" => "Left Bumper".to_string(),
            "shoulder_right" => "Right Bumper".to_string(),
            "trigger_left" => "Left Trigger".to_string(),
            "trigger_right" => "Right Trigger".to_string(),
            "joystick_left" => "Left Stick Click".to_string(),
            "joystick_right" => "Right Stick Click".to_string(),
            other => title_case_words(&other.replace('_', " ")),
        },
        "key_press" => format!("{} Key", friendly_key_name(target)),
        "mouse_button" => format!("{} Mouse", title_case_words(&target.replace('_', " "))),
        "mouse_wheel" => format!("Wheel {}", title_case_words(&target.replace('_', " "))),
        "mode_shift" => "Mode Shift".to_string(),
        other => title_case_words(&format!("{} {}", other.replace('_', " "), target)),
    }
}

fn steam_binding_kind(binding: &str) -> String {
    match binding.split_whitespace().next().unwrap_or("binding") {
        "xinput_button" => "XInput".to_string(),
        "key_press" => "Key".to_string(),
        "mouse_button" | "mouse_wheel" => "Mouse".to_string(),
        "mode_shift" => "Mode Shift".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn friendly_steam_source(source: &str) -> String {
    match source {
        "left_trackpad" => "Left Trackpad".to_string(),
        "right_trackpad" => "Right Trackpad".to_string(),
        "center_trackpad" => "Center Trackpad".to_string(),
        "joystick" => "Left Joystick".to_string(),
        "right_joystick" => "Right Joystick".to_string(),
        "dpad" => "Directional Pad".to_string(),
        "button_diamond" | "abxy" => "Face Buttons".to_string(),
        "left_trigger" => "Left Trigger".to_string(),
        "right_trigger" => "Right Trigger".to_string(),
        "gyro" => "Gyro".to_string(),
        "switch" => "Switches".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn friendly_steam_source_mode(mode: &str) -> String {
    match mode {
        "four_buttons" => "Four Buttons".to_string(),
        "dpad" => "Directional Pad".to_string(),
        "joystick_move" => "Joystick".to_string(),
        "joystick_camera" => "Joystick Camera".to_string(),
        "absolute_mouse" => "Mouse Region".to_string(),
        "relative_mouse" => "Mouse".to_string(),
        "mouse_joystick" => "Mouse Joystick".to_string(),
        "scrollwheel" => "Scroll Wheel".to_string(),
        "2dscroll" => "Directional Swipe".to_string(),
        "single_button" => "Single Button".to_string(),
        "trigger" => "Analog Trigger".to_string(),
        "switches" => "Switches".to_string(),
        "gyro" => "Gyro".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn friendly_steam_activator(activator: &str) -> String {
    match activator {
        "Full_Press" => "Full Press".to_string(),
        "Soft_Press" => "Soft Pull".to_string(),
        "Long_Press" => "Long Press".to_string(),
        "Double_Press" => "Double Press".to_string(),
        "Start_Press" => "Start Press".to_string(),
        "Release_Press" => "Release".to_string(),
        "Chord_Press" => "Chord".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn friendly_steam_controller_type(controller_type: &str) -> String {
    match controller_type {
        "controller_ps5_edge" => "DualSense Edge".to_string(),
        "controller_ps5" => "DualSense".to_string(),
        "controller_ps4" => "DualShock 4".to_string(),
        "controller_neptune" => "Steam Deck".to_string(),
        "controller_steamcontroller_gordon" => "Steam Controller".to_string(),
        "controller_xboxone" => "Xbox One".to_string(),
        "controller_xbox360" => "Xbox 360".to_string(),
        "controller_xboxelite" => "Xbox Elite".to_string(),
        "controller_generic" => "Generic Gamepad".to_string(),
        other => title_case_words(&other.replace("controller_", "").replace('_', " ")),
    }
}

fn friendly_key_name(key: &str) -> String {
    match key {
        "DASH" => "-".to_string(),
        "EQUALS" => "=".to_string(),
        "SPACE" => "Space".to_string(),
        "ENTER" => "Enter".to_string(),
        "ESCAPE" => "Esc".to_string(),
        other if other.len() == 1 => other.to_ascii_uppercase(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

fn clean_steam_layout_title(title: &str) -> String {
    if title.trim().is_empty() || title.starts_with('#') {
        "Steam Input Layout".to_string()
    } else {
        title.trim().chars().take(80).collect()
    }
}

pub(crate) fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn steam_app_id_from_path(path: &FsPath) -> Option<String> {
    let mut prior_was_user_id = false;
    let mut saw_userdata = false;
    let mut after_controller_config_root = false;
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if value == "userdata" {
            saw_userdata = true;
            prior_was_user_id = false;
            continue;
        }
        if value == "Steam Controller Configs" {
            after_controller_config_root = true;
            continue;
        }
        if after_controller_config_root && value == "config" {
            prior_was_user_id = true;
            saw_userdata = false;
            continue;
        }
        if saw_userdata && value.chars().all(|ch| ch.is_ascii_digit()) {
            if prior_was_user_id {
                return Some(value.to_string());
            }
            prior_was_user_id = true;
        }
        if after_controller_config_root && prior_was_user_id {
            let candidate = value.to_string();
            if !candidate.starts_with("controller_")
                && !candidate.starts_with("configset")
                && !candidate.starts_with("preferences")
                && !candidate.starts_with("personalization")
                && candidate != "steam_autocloud.vdf"
            {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn sanitized_steam_path(steam_root: &FsPath, path: &FsPath) -> Option<String> {
    let relative = path.strip_prefix(steam_root).ok()?;
    let mut result = Vec::new();
    let mut redact_next_numeric = false;
    for component in relative.components() {
        let value = component.as_os_str().to_string_lossy();
        if redact_next_numeric && value.chars().all(|ch| ch.is_ascii_digit()) {
            result.push("<steam-user>".to_string());
            redact_next_numeric = false;
            continue;
        }
        redact_next_numeric = value == "userdata";
        result.push(value.to_string());
    }
    Some(result.join("/"))
}
