use std::{
    cmp::Reverse,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    app_paths, apply_controller_names, configured_web_dist_dir, current_timestamp,
    env_flag_enabled, lan_api_enabled, materialized_adapters, materialized_telemetry_response,
    profile_resolution, web_dist_dir, AgentState, AgentStateInner, AppSettingsResponse,
    ControllerSummary, DiagnosticsResponse, GameDetectionResponse, InputBridgeStatusResponse,
    ProfileResolutionResponse, StatusResponse, SteamInputStatus, SupportedGameSummary,
    FORZA_LAN_ENABLE_ENV,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportBundleResponse {
    pub schema: String,
    pub generated_at: String,
    pub privacy: SupportPrivacy,
    pub environment: SupportEnvironment,
    pub status: StatusResponse,
    pub paths: SupportPaths,
    pub controllers: Vec<ControllerSummary>,
    pub diagnostics: DiagnosticsResponse,
    pub profile_resolution: ProfileResolutionResponse,
    pub game_detection: SupportGameDetectionSummary,
    pub adapters: Vec<SupportAdapterSummary>,
    pub telemetry: SupportTelemetrySummary,
    pub steam_input: SupportSteamInputSummary,
    pub input_bridge: SupportInputBridgeSummary,
    pub app_settings: SupportAppSettingsSummary,
    pub safety: SupportSafetySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportPrivacy {
    pub sanitized: bool,
    pub omitted: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportEnvironment {
    pub product: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub family: String,
    pub debug_build: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportPaths {
    pub app_paths_available: bool,
    pub config_dir: Option<String>,
    pub data_dir: Option<String>,
    pub log_dir: Option<String>,
    pub custom_config_dir: bool,
    pub web_dist_dir: String,
    pub web_dist_index_found: bool,
    pub web_dist_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportGameDetectionSummary {
    pub active_game_id: Option<String>,
    pub active_game_name: Option<String>,
    pub source: String,
    pub confidence: u8,
    pub process_name: Option<String>,
    pub module_id: Option<String>,
    pub adapter_id: Option<String>,
    pub profile_id: Option<String>,
    pub candidate_count: usize,
    pub selected_game: Option<SupportGameSummary>,
    pub supported_game_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportGameSummary {
    pub game_id: String,
    pub name: String,
    pub app_id: Option<String>,
    pub installed: bool,
    pub running: bool,
    pub support_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportAdapterSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub state: String,
    pub packet_rate_hz: Option<u16>,
    pub protocol: String,
    pub default_port: Option<u16>,
    pub listener_bound: bool,
    pub packet_count: u64,
    pub last_packet_age_ms: Option<u64>,
    pub last_packet_len: Option<usize>,
    pub parse_error_count: u64,
    pub last_parse_error_age_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportTelemetrySummary {
    pub signal_count: usize,
    pub source_id: Option<String>,
    pub live: bool,
    pub sample_signals: Vec<SupportTelemetrySignalSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportTelemetrySignalSummary {
    pub name: String,
    pub unit: Option<String>,
    pub updated_ms_ago: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportSteamInputSummary {
    pub running: bool,
    pub available: bool,
    pub install_detected: bool,
    pub layout_count: usize,
    pub binding_count: usize,
    pub warnings: Vec<String>,
    pub layouts: Vec<SupportSteamInputLayoutSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportSteamInputLayoutSummary {
    pub app_id: Option<String>,
    pub title: String,
    pub controller_type: Option<String>,
    pub controller_label: Option<String>,
    pub source: String,
    pub binding_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportInputBridgeSummary {
    pub available: bool,
    pub backend_id: String,
    pub provider: String,
    pub state: String,
    pub session_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportAppSettingsSummary {
    pub listen_on_all_interfaces: bool,
    pub effective_bind_address: String,
    pub desired_bind_address: String,
    pub restart_required: bool,
    pub forza_playstation_glyphs_enabled: bool,
    pub forza_playstation_glyphs_status: String,
    pub forza_playstation_glyphs_message: String,
    pub forza_playstation_glyphs_path_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportSafetySummary {
    pub hardware_output_enabled: bool,
    pub lan_api_enabled: bool,
    pub lan_forza_enabled: bool,
    pub api_bind_address: String,
    pub mutating_routes_origin_guarded: bool,
}

impl AgentState {
    pub async fn support_bundle(&self) -> SupportBundleResponse {
        let game_detection = self.cached_game_detection().await;
        let steam_input = self.cached_steam_input_status_or_refresh().await;
        let hardware_output_enabled = self.hardware_output_enabled();
        let output_diagnostics = self.output_diagnostics_snapshot();
        let inner = self.inner.read().await;
        let diagnostics = self.diagnostics_from_inner(
            &inner,
            &steam_input,
            &game_detection,
            hardware_output_enabled,
        );
        let status = self.status_from_inner(&inner, Some(&game_detection));
        let app_settings = self.app_settings_response(&inner.app_settings);

        SupportBundleResponse {
            schema: "dev.dscc.support-bundle.v1".to_string(),
            generated_at: current_timestamp(),
            privacy: SupportPrivacy {
                sanitized: true,
                omitted: vec![
                    "raw HID paths".to_string(),
                    "raw controller hardware IDs".to_string(),
                    "serial numbers".to_string(),
                    "Bluetooth addresses".to_string(),
                    "raw HID report bytes".to_string(),
                    "Steam user IDs".to_string(),
                    "Steam install paths".to_string(),
                    "Forza install paths".to_string(),
                    "raw Steam bindings".to_string(),
                    "local app executable paths".to_string(),
                    "virtual-output provider private paths".to_string(),
                ],
            },
            environment: support_environment(),
            status: sanitize_status_response(status),
            paths: support_paths(),
            controllers: self.apply_power_diagnostics_to_controllers(
                apply_controller_names(inner.controllers.summaries(), &inner.controller_names),
                &output_diagnostics,
                &inner.controller_configs,
            ),
            diagnostics: sanitize_diagnostics_response(diagnostics),
            profile_resolution: profile_resolution(&inner, Some(&game_detection)),
            game_detection: support_game_detection_summary(&game_detection),
            adapters: support_adapter_summaries(&inner, Some(&game_detection)),
            telemetry: support_telemetry_summary(&inner, Some(&game_detection)),
            steam_input: support_steam_input_summary(&steam_input),
            input_bridge: support_input_bridge_summary(self.input_bridge.status_response()),
            app_settings: support_app_settings_summary(app_settings),
            safety: SupportSafetySummary {
                hardware_output_enabled,
                lan_api_enabled: lan_api_enabled(),
                lan_forza_enabled: env_flag_enabled(FORZA_LAN_ENABLE_ENV),
                api_bind_address: self.bind_addr.to_string(),
                mutating_routes_origin_guarded: true,
            },
        }
    }
}

fn support_environment() -> SupportEnvironment {
    SupportEnvironment {
        product: "DualSense Command Center Agent".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        debug_build: cfg!(debug_assertions),
    }
}

fn support_paths() -> SupportPaths {
    let paths = app_paths();
    let web_dist = web_dist_dir();
    SupportPaths {
        app_paths_available: paths.is_some(),
        config_dir: paths
            .as_ref()
            .map(|paths| sanitize_support_path(&paths.config_dir)),
        data_dir: paths
            .as_ref()
            .map(|paths| sanitize_support_path(&paths.data_dir)),
        log_dir: paths
            .as_ref()
            .map(|paths| sanitize_support_path(&paths.log_dir)),
        custom_config_dir: std::env::var_os("DSCC_CONFIG_DIR").is_some(),
        web_dist_dir: sanitize_support_path(&web_dist.display().to_string()),
        web_dist_index_found: web_dist.join("index.html").is_file(),
        web_dist_configured: configured_web_dist_dir().is_some(),
    }
}

fn sanitize_status_response(mut status: StatusResponse) -> StatusResponse {
    status.bind_address = sanitize_support_text(&status.bind_address);
    status
}

pub(crate) fn sanitize_diagnostics_response(
    mut diagnostics: DiagnosticsResponse,
) -> DiagnosticsResponse {
    for check in &mut diagnostics.checks {
        check.detail = sanitize_support_text(&check.detail);
    }
    diagnostics
}

fn support_game_detection_summary(
    detection: &GameDetectionResponse,
) -> SupportGameDetectionSummary {
    SupportGameDetectionSummary {
        active_game_id: detection.active_game_id.clone(),
        active_game_name: detection.active_game_name.clone(),
        source: detection.source.clone(),
        confidence: detection.confidence,
        process_name: detection.process_name.clone(),
        module_id: detection.module_id.clone(),
        adapter_id: detection.adapter_id.clone(),
        profile_id: detection.profile_id.clone(),
        candidate_count: detection.candidates.len(),
        selected_game: detection.selected_game.as_ref().map(support_game_summary),
        supported_game_count: detection.supported_games.len(),
    }
}

fn support_game_summary(game: &SupportedGameSummary) -> SupportGameSummary {
    SupportGameSummary {
        game_id: game.game_id.clone(),
        name: game.name.clone(),
        app_id: game.app_id.clone(),
        installed: game.installed,
        running: game.running,
        support_level: game.support_level.clone(),
    }
}

fn support_adapter_summaries(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> Vec<SupportAdapterSummary> {
    let now = Instant::now();
    materialized_adapters(&inner.adapters, &inner.adapter_runtimes, game_detection)
        .into_iter()
        .map(|adapter| {
            let runtime = inner.adapter_runtime(&adapter.id);
            SupportAdapterSummary {
                id: adapter.id,
                name: adapter.name,
                enabled: adapter.enabled,
                state: adapter.state,
                packet_rate_hz: adapter.packet_rate_hz,
                protocol: adapter.protocol,
                default_port: runtime.and_then(|runtime| runtime.default_port),
                listener_bound: runtime.is_some_and(|runtime| runtime.listener_bound),
                packet_count: runtime
                    .map(|runtime| runtime.packet_count)
                    .unwrap_or_default(),
                last_packet_age_ms: runtime
                    .and_then(|runtime| runtime.last_packet_at)
                    .map(|last| duration_millis_u64(now.duration_since(last))),
                last_packet_len: runtime.and_then(|runtime| runtime.last_packet_len),
                parse_error_count: runtime
                    .map(|runtime| runtime.parse_error_count)
                    .unwrap_or_default(),
                last_parse_error_age_ms: runtime
                    .and_then(|runtime| runtime.last_parse_error_at)
                    .map(|last| duration_millis_u64(now.duration_since(last))),
            }
        })
        .collect()
}

pub(crate) fn support_telemetry_summary(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> SupportTelemetrySummary {
    let telemetry = materialized_telemetry_response(inner, game_detection);
    let source_id = inner.telemetry.text("source.id").map(str::to_string);
    let adapter_id = game_detection
        .and_then(|detection| detection.adapter_id.as_deref())
        .or(source_id.as_deref())
        .or(inner.active_adapter_id.as_deref());
    let live = adapter_id
        .and_then(|adapter_id| inner.adapter_runtime(adapter_id))
        .is_some_and(|runtime| runtime.has_recent_packet(Instant::now()));
    SupportTelemetrySummary {
        signal_count: telemetry.len(),
        source_id,
        live,
        sample_signals: telemetry
            .into_iter()
            .take(64)
            .map(|signal| SupportTelemetrySignalSummary {
                name: signal.name,
                unit: signal.unit,
                updated_ms_ago: signal.updated_ms_ago,
            })
            .collect(),
    }
}

pub(crate) fn support_steam_input_summary(status: &SteamInputStatus) -> SupportSteamInputSummary {
    SupportSteamInputSummary {
        running: status.running,
        available: status.available,
        install_detected: status.steam_path.is_some(),
        layout_count: status.layouts.len(),
        binding_count: status
            .layouts
            .iter()
            .map(|layout| layout.binding_count)
            .sum(),
        warnings: status
            .warnings
            .iter()
            .map(|warning| sanitize_support_text(warning))
            .collect(),
        layouts: status
            .layouts
            .iter()
            .map(|layout| SupportSteamInputLayoutSummary {
                app_id: layout.app_id.clone(),
                title: layout.title.clone(),
                controller_type: layout.controller_type.clone(),
                controller_label: layout.controller_label.clone(),
                source: sanitize_support_text(&layout.source),
                binding_count: layout.binding_count,
            })
            .collect(),
    }
}

fn support_input_bridge_summary(status: InputBridgeStatusResponse) -> SupportInputBridgeSummary {
    SupportInputBridgeSummary {
        available: status.available,
        backend_id: sanitize_support_text(&status.backend_id),
        provider: sanitize_support_text(&status.provider),
        state: status.state,
        session_count: status.sessions.len(),
        warnings: status
            .warnings
            .iter()
            .map(|warning| sanitize_support_text(warning))
            .collect(),
    }
}

fn support_app_settings_summary(settings: AppSettingsResponse) -> SupportAppSettingsSummary {
    let glyphs = settings.settings.forza_playstation_glyphs;
    SupportAppSettingsSummary {
        listen_on_all_interfaces: settings.settings.listen_on_all_interfaces,
        effective_bind_address: settings.effective_bind_address,
        desired_bind_address: settings.desired_bind_address,
        restart_required: settings.restart_required,
        forza_playstation_glyphs_enabled: glyphs.enabled,
        forza_playstation_glyphs_status: glyphs.last_status,
        forza_playstation_glyphs_message: sanitize_support_text(&glyphs.last_message),
        forza_playstation_glyphs_path_configured: glyphs.install_path.is_some(),
    }
}

fn duration_millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn sanitize_support_path(path: &str) -> String {
    sanitize_support_text(path)
}

pub(crate) fn sanitize_support_text(value: &str) -> String {
    let mut redacted = redact_windows_absolute_paths(value.to_string());
    for (raw, replacement) in support_redaction_roots() {
        if !raw.is_empty() {
            redacted = redacted.replace(&raw, &replacement);
        }
    }
    redact_steam_user_ids(redacted)
}

fn support_redaction_roots() -> Vec<(String, String)> {
    let mut roots = [
        ("USERPROFILE", "$HOME"),
        ("HOME", "$HOME"),
        ("LOCALAPPDATA", "%LOCALAPPDATA%"),
        ("APPDATA", "%APPDATA%"),
    ]
    .into_iter()
    .filter_map(|(name, replacement)| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| (value, replacement.to_string()))
    })
    .collect::<Vec<_>>();
    roots.sort_by_key(|root| Reverse(root.0.len()));
    roots.dedup_by(|a, b| a.0 == b.0);
    roots
}

fn redact_steam_user_ids(mut value: String) -> String {
    for marker in [
        "userdata\\",
        "userdata/",
        "Steam Controller Configs\\",
        "Steam Controller Configs/",
    ] {
        let mut search_from = 0;
        while let Some(relative_start) = value[search_from..].find(marker) {
            let start = search_from + relative_start + marker.len();
            let end = value[start..]
                .char_indices()
                .take_while(|(_, ch)| ch.is_ascii_digit())
                .last()
                .map(|(index, ch)| start + index + ch.len_utf8())
                .unwrap_or(start);
            if end > start {
                value.replace_range(start..end, "<steam-user>");
                search_from = start + "<steam-user>".len();
            } else {
                search_from = start;
            }
        }
    }
    value
}

fn redact_windows_absolute_paths(value: String) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut redacted = String::with_capacity(value.len());
    let mut index = 0;
    while index < chars.len() {
        if let Some(end) = windows_absolute_path_end(&chars, index) {
            redacted.push_str("[local-path]");
            index = end;
        } else {
            redacted.push(chars[index]);
            index += 1;
        }
    }
    redacted
}

fn windows_absolute_path_end(chars: &[char], start: usize) -> Option<usize> {
    if !starts_extended_windows_path(chars, start) && !starts_windows_drive_path(chars, start) {
        return None;
    }

    let mut end = start;
    while end < chars.len() {
        if is_support_path_boundary(chars[end])
            || (chars[end] == '.'
                && chars
                    .get(end + 1)
                    .is_none_or(|next| next.is_ascii_whitespace()))
        {
            break;
        }
        end += 1;
    }
    Some(end)
}

fn starts_extended_windows_path(chars: &[char], start: usize) -> bool {
    start + 6 < chars.len()
        && chars[start] == '\\'
        && chars[start + 1] == '\\'
        && (chars[start + 2] == '?' || chars[start + 2] == '.')
        && chars[start + 3] == '\\'
        && chars[start + 4].is_ascii_alphabetic()
        && chars[start + 5] == ':'
        && is_windows_separator(chars[start + 6])
}

fn starts_windows_drive_path(chars: &[char], start: usize) -> bool {
    start + 2 < chars.len()
        && chars[start].is_ascii_alphabetic()
        && chars[start + 1] == ':'
        && is_windows_separator(chars[start + 2])
}

fn is_windows_separator(ch: char) -> bool {
    ch == '\\' || ch == '/'
}

fn is_support_path_boundary(ch: char) -> bool {
    matches!(ch, '"' | '\'' | '\r' | '\n' | '\t')
}
