use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};

use super::{
    discovery::{collect_steam_controller_config_files, steam_root_candidates},
    writer::SteamInputWriteFailure,
};

pub(super) fn resolve_steam_input_layout_path(
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

pub(super) fn steam_app_id_from_path(path: &FsPath) -> Option<String> {
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
