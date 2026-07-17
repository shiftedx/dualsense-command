use std::path::{Path as FsPath, PathBuf};

use serde::{Deserialize, Serialize};

mod discovery;
mod display_labels;
mod paddle_preset;
mod parser;
mod path_safety;
mod writer;

pub(crate) use discovery::{
    discover_steam_input_status_async, pending_steam_input_status, steam_input_discovery_pending,
    steam_root_candidates,
};
pub(crate) use paddle_preset::write_steam_input_paddle_preset;
#[cfg(test)]
pub(crate) use paddle_preset::{
    ensure_dualsense_edge_steam_layout, steam_edge_paddle_binding, STEAM_EDGE_BACK_RIGHT_INPUT_ID,
};
#[cfg(test)]
pub(crate) use parser::parse_steam_input_layout;
#[cfg(test)]
pub(crate) use path_safety::sanitized_steam_path;
#[cfg(test)]
pub(crate) use path_safety::validated_steam_input_layout_path;
pub(crate) use writer::write_steam_input_binding;
#[cfg(test)]
pub(crate) use writer::{mark_dscc_steam_profile_metadata, replace_steam_binding_value};

pub(crate) fn numeric_child_dirs(root: &FsPath, max_dirs: usize) -> Vec<PathBuf> {
    discovery::numeric_child_dirs(root, max_dirs)
}

pub(crate) fn quoted_tokens(line: &str) -> Vec<String> {
    parser::quoted_tokens(line)
}

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
    #[serde(default, alias = "dry_run")]
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
    #[serde(default, alias = "dry_run")]
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
