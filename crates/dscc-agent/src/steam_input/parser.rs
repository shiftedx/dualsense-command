use std::{collections::BTreeMap, path::Path as FsPath};

use super::{
    display_labels::{
        clean_steam_layout_title, friendly_steam_activator, friendly_steam_binding,
        friendly_steam_controller_type, friendly_steam_input, friendly_steam_source,
        friendly_steam_source_mode, steam_binding_kind,
    },
    path_safety::{sanitized_steam_path, steam_app_id_from_path},
    SteamInputBinding, SteamInputLayout,
};

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

pub(super) fn steam_input_from_stack(stack: &[String]) -> Option<String> {
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

pub(super) fn steam_activator_from_stack(stack: &[String]) -> Option<String> {
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
