use std::{
    fs, io,
    path::{Path as FsPath, PathBuf},
};

use axum::http::StatusCode;

use super::{
    parser::{
        parse_steam_input_layout, quoted_tokens, steam_activator_from_stack, steam_input_from_stack,
    },
    path_safety::{resolve_steam_input_layout_path, sanitized_steam_path},
    SteamInputBinding, SteamInputBindingWriteRequest, SteamInputBindingWriteResponse,
};

#[derive(Debug)]
pub(crate) struct SteamInputWriteFailure {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl SteamInputWriteFailure {
    pub(super) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub(super) fn io(message: impl Into<String>, error: io::Error) -> Self {
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

pub(super) fn normalize_steam_raw_binding(value: &str) -> Result<String, String> {
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

pub(super) fn steam_binding_matches_write_request(
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

pub(super) fn backup_and_write_steam_input_layout(
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
