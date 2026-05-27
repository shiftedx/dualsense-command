use super::*;

pub(crate) fn signal_gear_to_u8(value: f64) -> Option<u8> {
    value
        .is_finite()
        .then(|| value.round().clamp(0.0, 255.0) as u8)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResponse {
    pub product: String,
    pub version: String,
    pub healthy: bool,
    pub bind_address: String,
    pub uptime_seconds: u64,
    pub active_profile_id: Option<String>,
    pub active_adapter_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
    pub release_name: Option<String>,
    pub published_at: Option<String>,
    pub state: String,
    pub checked_at: Option<String>,
    pub error: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub listen_on_all_interfaces: bool,
    #[serde(default)]
    pub forza_playstation_glyphs: ForzaGlyphOverrideSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForzaGlyphOverrideSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub install_path: Option<String>,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_message: String,
}

impl Default for ForzaGlyphOverrideSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            install_path: None,
            last_status: "not_installed".to_string(),
            last_message: "PlayStation glyph override has not been applied.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsResponse {
    pub settings: AppSettings,
    pub effective_bind_address: String,
    pub desired_bind_address: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControllerSummary {
    pub id: String,
    pub name: String,
    pub model: String,
    pub transport: String,
    pub connected: bool,
    pub connection_state: ConnectionState,
    pub battery_percent: Option<u8>,
    pub battery_state: BatteryState,
    pub permission: ControllerPermissionState,
    pub diagnostic_state: ControllerDiagnosticState,
    #[serde(default)]
    pub power_diagnostics: ControllerPowerDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControllerDetail {
    pub id: String,
    pub name: String,
    pub model: String,
    pub transport: String,
    pub connected: bool,
    pub connection_state: ConnectionState,
    pub battery_percent: Option<u8>,
    pub battery_state: BatteryState,
    pub permission: ControllerPermissionState,
    pub diagnostic_state: ControllerDiagnosticState,
    pub vendor_id: u16,
    pub product_id: u16,
    pub capabilities: ControllerCapabilities,
    pub diagnostics: Vec<ControllerDiagnostic>,
    #[serde(default)]
    pub power_diagnostics: ControllerPowerDiagnostics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerPowerDiagnostics {
    pub written_reports: u64,
    pub suppressed_redundant_reports: u64,
    pub output_write_rate_hz: Option<u16>,
    pub output_cadence_ms: Option<u64>,
    pub keepalive_interval_ms: u64,
    pub last_write_age_ms: Option<u64>,
    pub last_suppressed_age_ms: Option<u64>,
    pub native_rumble_passthrough: bool,
    pub adaptive_triggers_retained: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateControllerRequest {
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerPermissionState {
    Unknown,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerDiagnosticState {
    Ok,
    Disconnected,
    PermissionDenied,
    CannotOpen,
    Unsupported,
    Faulted,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControllerDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredController {
    pub info: ControllerInfo,
    pub state: ControllerState,
    pub raw_device_id: Option<RawDeviceId>,
    pub name: Option<String>,
    pub transport_label: Option<String>,
    pub permission: ControllerPermissionState,
    pub diagnostics: Vec<ControllerDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePermissionProblem {
    pub id: Option<ControllerId>,
    pub transport: Option<ControllerTransportKind>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerDiscoveryEvent {
    Attached(DiscoveredController),
    Detached(ControllerId),
    StatusChanged(ControllerState),
    PermissionDenied(DevicePermissionProblem),
    Faulted {
        id: Option<ControllerId>,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub built_in: bool,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerConfig {
    pub controller_id: String,
    pub model: String,
    #[serde(default)]
    pub input_mode: ControllerInputMode,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    #[serde(default)]
    pub forza: ForzaTelemetryConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    #[serde(default)]
    pub input_bridge: InputBridgeConfig,
    #[serde(default)]
    pub profile_assignments: Vec<ProfileAssignmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfig {
    #[serde(default)]
    pub input_mode: ControllerInputMode,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    #[serde(default)]
    pub forza: ForzaTelemetryConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    #[serde(default)]
    pub input_bridge: InputBridgeConfig,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerInputMode {
    #[serde(rename = "native_dualsense")]
    #[default]
    NativeDualSense,
    SteamInputCompanion,
    DsccInputBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerCurve(u16);

impl TriggerCurve {
    pub(crate) const fn default_l2() -> Self {
        Self(135)
    }

    pub(crate) const fn default_r2() -> Self {
        Self(185)
    }

    pub(crate) fn from_ratio(value: f64) -> Self {
        if !value.is_finite() {
            return Self::default_l2();
        }
        Self((value * TRIGGER_CURVE_SCALE).round() as u16).normalized()
    }

    pub(crate) fn as_f64(self) -> f64 {
        f64::from(self.normalized().0) / TRIGGER_CURVE_SCALE
    }

    pub(crate) fn normalized(self) -> Self {
        Self(self.0.clamp(TRIGGER_CURVE_MIN, TRIGGER_CURVE_MAX))
    }
}

impl Serialize for TriggerCurve {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.as_f64())
    }
}

impl<'de> Deserialize<'de> for TriggerCurve {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_ratio(f64::deserialize(deserializer)?))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TriggerCurvePoint {
    pub input: u8,
    pub output: u8,
}

pub(crate) fn default_l2_trigger_curve() -> TriggerCurve {
    TriggerCurve::default_l2()
}

pub(crate) fn default_r2_trigger_curve() -> TriggerCurve {
    TriggerCurve::default_r2()
}

pub(crate) fn default_l2_trigger_curve_points() -> Vec<TriggerCurvePoint> {
    trigger_curve_points_from_curve(TriggerCurve::default_l2())
}

pub(crate) fn default_r2_trigger_curve_points() -> Vec<TriggerCurvePoint> {
    trigger_curve_points_from_curve(TriggerCurve::default_r2())
}

pub(crate) fn trigger_curve_points_from_curve(curve: TriggerCurve) -> Vec<TriggerCurvePoint> {
    [0_u8, 25, 50, 75, 100]
        .into_iter()
        .map(|input| TriggerCurvePoint {
            input,
            output: ((f64::from(input) / 100.0).powf(curve.as_f64()) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8,
        })
        .collect()
}

pub(crate) fn normalize_trigger_curve_points(
    points: Vec<TriggerCurvePoint>,
    fallback_curve: TriggerCurve,
) -> Vec<TriggerCurvePoint> {
    if points.len() < TRIGGER_CURVE_POINT_MIN {
        return trigger_curve_points_from_curve(fallback_curve);
    }

    let mut normalized: Vec<TriggerCurvePoint> = points
        .into_iter()
        .map(|point| TriggerCurvePoint {
            input: point.input.min(100),
            output: point.output.min(100),
        })
        .collect();
    normalized.sort_by_key(|point| point.input);

    let mut deduped: Vec<TriggerCurvePoint> = Vec::with_capacity(normalized.len());
    for point in normalized {
        if let Some(last) = deduped.last_mut() {
            if last.input == point.input {
                *last = point;
                continue;
            }
        }
        deduped.push(point);
    }

    if deduped.first().is_none_or(|point| point.input != 0) {
        deduped.insert(
            0,
            TriggerCurvePoint {
                input: 0,
                output: 0,
            },
        );
    } else if let Some(first) = deduped.first_mut() {
        first.output = 0;
    }
    if deduped.last().is_none_or(|point| point.input != 100) {
        deduped.push(TriggerCurvePoint {
            input: 100,
            output: 100,
        });
    } else if let Some(last) = deduped.last_mut() {
        last.output = 100;
    }

    if deduped.len() < TRIGGER_CURVE_POINT_MIN {
        return trigger_curve_points_from_curve(fallback_curve);
    }
    if deduped.len() > TRIGGER_CURVE_POINT_MAX {
        let mut trimmed = Vec::with_capacity(TRIGGER_CURVE_POINT_MAX);
        trimmed.push(deduped[0]);
        trimmed.extend(
            deduped[1..deduped.len() - 1]
                .iter()
                .copied()
                .take(TRIGGER_CURVE_POINT_MAX - 2),
        );
        trimmed.push(*deduped.last().expect("curve has endpoint"));
        return trimmed;
    }

    deduped
}

pub(crate) fn trigger_curve_value_points(points: &[TriggerCurvePoint]) -> Vec<ValuePoint> {
    points
        .iter()
        .map(|point| ValuePoint {
            input: f64::from(point.input) / 100.0,
            output: f64::from(point.output) / 100.0,
        })
        .collect()
}

pub(crate) fn default_vibration_mode() -> String {
    "Balanced".to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TriggerConfig {
    pub same_range: bool,
    pub l2_from: u8,
    pub l2_to: u8,
    pub r2_from: u8,
    pub r2_to: u8,
    #[serde(default = "default_l2_trigger_curve")]
    pub l2_curve: TriggerCurve,
    #[serde(default = "default_r2_trigger_curve")]
    pub r2_curve: TriggerCurve,
    pub l2_curve_points: Vec<TriggerCurvePoint>,
    pub r2_curve_points: Vec<TriggerCurvePoint>,
    pub effect: String,
    pub intensity: String,
    pub vibration: String,
    #[serde(default = "default_vibration_mode")]
    pub vibration_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TriggerConfigDeserialize {
    same_range: bool,
    l2_from: u8,
    l2_to: u8,
    r2_from: u8,
    r2_to: u8,
    #[serde(default = "default_l2_trigger_curve")]
    l2_curve: TriggerCurve,
    #[serde(default = "default_r2_trigger_curve")]
    r2_curve: TriggerCurve,
    #[serde(default)]
    l2_curve_points: Option<Vec<TriggerCurvePoint>>,
    #[serde(default)]
    r2_curve_points: Option<Vec<TriggerCurvePoint>>,
    effect: String,
    intensity: String,
    vibration: String,
    #[serde(default = "default_vibration_mode")]
    vibration_mode: String,
}

impl<'de> Deserialize<'de> for TriggerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TriggerConfigDeserialize::deserialize(deserializer)?;
        Ok(Self {
            same_range: wire.same_range,
            l2_from: wire.l2_from,
            l2_to: wire.l2_to,
            r2_from: wire.r2_from,
            r2_to: wire.r2_to,
            l2_curve: wire.l2_curve,
            r2_curve: wire.r2_curve,
            l2_curve_points: wire
                .l2_curve_points
                .unwrap_or_else(|| trigger_curve_points_from_curve(wire.l2_curve)),
            r2_curve_points: wire
                .r2_curve_points
                .unwrap_or_else(|| trigger_curve_points_from_curve(wire.r2_curve)),
            effect: wire.effect,
            intensity: wire.intensity,
            vibration: wire.vibration,
            vibration_mode: wire.vibration_mode,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForzaTelemetryConfig {
    #[serde(default = "default_forza_body_rumble_mode")]
    pub body_rumble_mode: String,
    #[serde(default)]
    pub effects: Vec<ForzaEffectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForzaEffectConfig {
    pub id: String,
    #[serde(default = "default_forza_effect_enabled")]
    pub enabled: bool,
    #[serde(default = "default_forza_effect_intensity")]
    pub intensity: u8,
    #[serde(default = "default_forza_effect_route")]
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LightbarConfig {
    pub enabled: bool,
    pub color: String,
    #[serde(default = "default_rpm_color")]
    pub rpm_color: String,
    pub brightness: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StickConfig {
    pub left_curve: String,
    pub left_curve_amount: u8,
    pub left_deadzone: u8,
    pub right_curve: String,
    pub right_curve_amount: u8,
    pub right_deadzone: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ButtonAssignmentConfig {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAssignmentConfig {
    pub game_id: String,
    pub game_name: String,
    pub profile_id: String,
    pub profile_name: String,
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub version: String,
    pub source: String,
    pub trusted: bool,
    pub protocol: String,
    pub setup_hint: String,
    pub setup_url: Option<String>,
    pub profile_templates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub state: String,
    pub packet_rate_hz: Option<u16>,
    pub protocol: String,
    pub setup_hint: String,
    pub setup_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetrySignalResponse {
    pub name: String,
    pub value: serde_json::Value,
    pub unit: Option<String>,
    pub updated_ms_ago: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticsResponse {
    pub loopback_only: bool,
    pub hardware_required: bool,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSnapshotResponse {
    pub status: StatusResponse,
    pub app_settings: AppSettingsResponse,
    pub controllers: Vec<ControllerSummary>,
    pub profiles: Vec<ProfileSummary>,
    pub adapters: Vec<AdapterSummary>,
    pub modules: Vec<ModuleSummary>,
    pub steam_input: SteamInputStatus,
    pub input_bridge: InputBridgeStatusResponse,
    pub game_detection: GameDetectionResponse,
    pub profile_resolution: ProfileResolutionResponse,
    pub effect_state: CurrentEffectResponse,
    pub telemetry: Vec<TelemetrySignalResponse>,
    pub logs: Vec<LogEntry>,
    pub diagnostics: DiagnosticsResponse,
    pub partial_errors: Vec<SnapshotPartialError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotPartialError {
    pub endpoint: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: String,
    pub data_dir: String,
    pub log_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileRequest {
    pub name: String,
    #[serde(default, alias = "game_id")]
    pub game_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportedProfile {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub built_in: bool,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<ProfileConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ImportProfileRequest {
    pub schema: String,
    pub id: Option<String>,
    pub name: String,
    #[serde(default, alias = "gameId")]
    pub game_id: Option<String>,
    #[serde(default)]
    pub config: Option<ProfileConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateControllerConfigRequest {
    #[serde(default)]
    pub input_mode: ControllerInputMode,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    #[serde(default)]
    pub forza: ForzaTelemetryConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    #[serde(default)]
    pub input_bridge: Option<InputBridgeConfig>,
    #[serde(default)]
    pub profile_assignments: Vec<ProfileAssignmentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileConfigRequest {
    #[serde(default)]
    pub input_mode: ControllerInputMode,
    #[serde(default)]
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    #[serde(default)]
    pub forza: ForzaTelemetryConfig,
    #[serde(default)]
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    #[serde(default)]
    pub input_bridge: Option<InputBridgeConfig>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResolutionResponse {
    pub controller_id: Option<String>,
    pub detected_game_id: Option<String>,
    pub active_adapter_id: Option<String>,
    pub selected_profile_id: Option<String>,
    pub reason: String,
    pub override_profile_id: Option<String>,
    pub validation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOverride {
    pub controller_id: Option<String>,
    pub game_id: Option<String>,
    pub profile_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOverrideScope {
    pub controller_id: Option<String>,
    pub game_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeBindingWriteRequest {
    pub controller_id: Option<String>,
    pub profile_id: Option<String>,
    pub input_id: String,
    pub target: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeBindingWriteResponse {
    pub accepted: bool,
    pub message: String,
    pub dry_run: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAdapterRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppSettingsRequest {
    pub listen_on_all_interfaces: Option<bool>,
    pub forza_playstation_glyphs: Option<UpdateForzaGlyphOverrideRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateForzaGlyphOverrideRequest {
    pub enabled: bool,
    pub install_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTestRequest {
    pub target: Option<String>,
    pub mode: Option<String>,
    pub intensity: Option<u8>,
    pub start_position: Option<f64>,
    pub l2_position: Option<f64>,
    pub r2_position: Option<f64>,
    pub duration_ms: Option<u64>,
    pub trigger: Option<TriggerConfig>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionAccepted {
    pub accepted: bool,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectTestResponse {
    pub accepted: bool,
    pub message: String,
    pub dry_run: bool,
    pub duration_ms: u64,
    pub output: ControllerOutputFrame,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInputResponse {
    pub controller_id: String,
    pub available: bool,
    pub source: String,
    pub message: String,
    pub sampled_at_ms: Option<u64>,
    pub age_ms: Option<u64>,
    pub axes: ControllerInputAxesResponse,
    pub triggers: ControllerInputTriggersResponse,
    pub buttons: Vec<ControllerInputButtonResponse>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInputAxesResponse {
    pub left_stick: ControllerInputStickResponse,
    pub right_stick: ControllerInputStickResponse,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInputStickResponse {
    pub x: f64,
    pub y: f64,
    pub magnitude: f64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInputTriggersResponse {
    pub l2: f64,
    pub r2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInputButtonResponse {
    pub id: String,
    pub label: String,
    pub pressed: bool,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentEffectResponse {
    pub controller_id: Option<String>,
    pub selected_profile_id: Option<String>,
    pub selected_profile_name: Option<String>,
    pub reason: String,
    pub dry_run: bool,
    pub hardware_output_enabled: bool,
    pub output: ControllerOutputFrame,
    pub parity_effects: Vec<EffectMappingStatus>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EffectMappingStatus {
    pub id: String,
    pub target: String,
    pub label: String,
    pub signal: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealtimeMessage {
    #[serde(rename = "type")]
    pub kind: String,
    pub controller: Option<ControllerSummary>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeviceBackendSummary {
    pub(crate) status: String,
    pub(crate) detail: String,
}

impl DeviceBackendSummary {
    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
    pub(crate) fn mock() -> Self {
        Self {
            status: "mock".to_string(),
            detail: "Controller discovery is running through dscc-device mock transport"
                .to_string(),
        }
    }

    pub(crate) fn hid(output_mode: OutputMode) -> Self {
        if output_mode.hardware_writes_enabled() {
            return Self {
                status: "hidapi".to_string(),
                detail: "Real HID discovery and guarded controller output are active".to_string(),
            };
        }

        Self {
            status: "hidapi".to_string(),
            detail: "Real HID discovery is active; hardware output is disabled".to_string(),
        }
    }

    pub(crate) fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: "unavailable".to_string(),
            detail: reason.into(),
        }
    }
}
