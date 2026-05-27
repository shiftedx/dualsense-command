use std::fs;

use super::{
    display_labels::friendly_steam_input,
    parser::parse_steam_input_layout,
    path_safety::{resolve_steam_input_layout_path, sanitized_steam_path},
    writer::{
        backup_and_write_steam_input_layout, mark_dscc_steam_profile_metadata,
        normalize_steam_raw_binding, replace_steam_binding_value,
        steam_binding_matches_write_request, SteamInputWriteFailure,
    },
    SteamInputBinding, SteamInputBindingWriteRequest, SteamInputLayout,
    SteamInputPaddlePresetPaddleResult, SteamInputPaddlePresetRequest,
    SteamInputPaddlePresetResponse,
};

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
