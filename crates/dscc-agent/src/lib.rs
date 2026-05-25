use std::{
    collections::BTreeMap,
    fs, io,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use directories::ProjectDirs;
use dscc_adapters::{
    built_in_adapters, built_in_udp_adapters, initial_detection, parse_udp_telemetry_packet,
    AdapterProtocol, BuiltInAdapter, UdpTelemetryAdapter,
};
use dscc_core::{
    BatteryState, ComparableValue, ComparisonOp, ConnectionState, ControllerCapabilities,
    ControllerFamily, ControllerId, ControllerInfo, ControllerOutputFrame, ControllerState,
    ControllerTransportKind, EffectEngine, EffectRule, EffectTarget, EffectTemplate,
    LightbarOutput, PlayerLedsOutput, Profile, RgbColor, RuleCondition, RumbleOutput, RumblePolicy,
    TriggerOutput, ValuePoint, ValueSource,
};
use dscc_device::{
    edge_onboard_transport_supported, BatteryInfo as DeviceBatteryInfo,
    BatteryState as DeviceBatteryState, ConnectionState as DeviceConnectionState,
    ControllerCapabilities as DeviceControllerCapabilities, ControllerId as DeviceControllerId,
    ControllerInfo as DeviceControllerInfo, ControllerInputState, ControllerOutputManager,
    ControllerOutputTarget, ControllerOutputWrite, ControllerState as DeviceControllerState,
    DeviceConfig, DeviceEvent, DeviceFamily, DeviceManager, DevicePathHint, DeviceTransport,
    DeviceTransportKind, EdgeButton, EdgeButtonMapping, EdgeOnboardProfile, EdgeOnboardSlotId,
    EdgeProfileIntensity, EdgeStickPreset, EdgeStickProfile, EdgeTriggerDeadzone, HidApiTransport,
    MockTransport, OutputMode, RawDeviceId, RawHidDevice,
};
use dscc_telemetry::{AdapterDetection, SignalName, SignalSnapshot, SignalUpdate, SignalValue};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::{broadcast, Mutex as AsyncMutex, RwLock},
};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

mod bind_addr;
mod env_policy;
mod forza_glyphs;
mod game_modules;
mod http_security;

pub use bind_addr::{
    resolve_agent_bind_addr, DEFAULT_BIND_ADDR, DEFAULT_FORZA_BIND_ADDR, FORZA_BIND_ADDR_ENV,
    FORZA_LAN_ENABLE_ENV, LAN_API_ENABLE_ENV,
};

pub(crate) use bind_addr::{
    default_agent_bind_addr, desired_agent_bind_addr, lan_api_enabled, resolve_forza_bind_addr,
};
pub(crate) use env_policy::configured_output_mode;
#[cfg(test)]
pub(crate) use forza_glyphs::{
    ensure_forza_icon_target_is_safe, forza_controller_icon_backup_path,
    forza_controller_icon_targets, FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP,
};
pub(crate) use forza_glyphs::{
    install_forza_playstation_glyphs, resolve_forza_horizon6_install_path,
    restore_forza_original_glyphs, trusted_forza_horizon6_install_path,
};
#[cfg(test)]
use game_modules::detect_running_game_from_processes;
use game_modules::{
    built_in_game_modules, detect_running_game_from_processes_with_user_games,
    game_executable_exists, game_module_summaries, no_game_detection, supported_game_summary,
    GameModule, ASSETTO_CORSA_RALLY_PROFILE_ID, ASSETTO_SHARED_MEMORY_ADAPTER_ID,
    FORZA_DATA_OUT_ADAPTER_ID, FORZA_HORIZON_IMMERSIVE_PROFILE_ID, FORZA_HORIZON_PROFILE_ID,
};
pub(crate) use http_security::{reject_cross_origin_mutations, request_origin_matches_host};

const GLOBAL_PROFILE_ID: &str = "global";
const DEFAULT_PROFILE_ID: &str = GLOBAL_PROFILE_ID;
const IMMERSIVE_PROFILE_ID: &str = FORZA_HORIZON_IMMERSIVE_PROFILE_ID;

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn current_timestamp_millis() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

const TELEMETRY_PACKET_STALE_AFTER: Duration = Duration::from_secs(2);
const HARDWARE_OUTPUT_INTERVAL: Duration = Duration::from_millis(33);
const HARDWARE_OUTPUT_KEEPALIVE_INTERVAL: Duration = Duration::from_millis(750);
const MANUAL_OUTPUT_REFRESH_INTERVAL: Duration = Duration::from_millis(250);
const BASE_FEEL_OUTPUT_REFRESH_INTERVAL: Duration = Duration::from_millis(33);
const HARDWARE_GAME_DETECTION_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_EFFECT_TEST_DURATION_MS: u64 = 650;
const MAX_EFFECT_TEST_DURATION_MS: u64 = 1_500;
const DEFAULT_BASE_FEEL_TEST_DURATION_MS: u64 = 30_000;
const MAX_BASE_FEEL_TEST_DURATION_MS: u64 = 60_000;
const UDP_TELEMETRY_PROCESS_INTERVAL: Duration = Duration::from_millis(33);
#[cfg(target_os = "windows")]
const SHARED_MEMORY_TELEMETRY_PROCESS_INTERVAL: Duration = Duration::from_millis(33);
const FORZA_SHIFT_EVENT_HOLD: Duration = Duration::from_millis(190);
const FORZA_SUSPENSION_IMPACT_HOLD: Duration = Duration::from_millis(170);
const GAME_DETECTION_CACHE_TTL: Duration = Duration::from_secs(5);
const STEAM_INPUT_CACHE_TTL: Duration = Duration::from_secs(30);
const STEAM_GAME_CATALOG_CACHE_TTL: Duration = Duration::from_secs(300);
const UPDATE_CHECK_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const UPDATE_CHECK_URL: &str =
    "https://api.github.com/repos/shiftedx/dualsense-command/releases/latest";
const STEAM_INPUT_LAYOUT_SCAN_LIMIT: usize = 96;
const TELEMETRY_WS_INVALIDATION_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(target_os = "windows")]
const WINDOWS_PNP_CONTROLLER_CACHE_TTL: Duration = Duration::from_secs(60);
const FORZA_BRAKE_FULL_FORCE_AT: f64 = 246.0 / 255.0;
const FORZA_THROTTLE_FULL_FORCE_AT: f64 = 252.0 / 255.0;
const FORZA_BRAKE_BASELINE_FORCE: f64 = 42.0 / 255.0;
const FORZA_BRAKE_NORMAL_FORCE: f64 = 164.0 / 255.0;
const FORZA_BRAKE_ENDSTOP_FORCE: f64 = 238.0 / 255.0;
const FORZA_THROTTLE_BASELINE_FORCE: f64 = 3.0 / 255.0;
const FORZA_THROTTLE_NORMAL_FORCE: f64 = 28.0 / 255.0;
const FORZA_THROTTLE_ENDSTOP_FORCE: f64 = 106.0 / 255.0;
const FORZA_HANDBRAKE_FORCE: f64 = 25.0 / 255.0;
const FORZA_ABS_RANGE_START_RATIO: f64 = 0.30;
const FORZA_ABS_MIN_SPEED_KMH: f64 = 15.0;
const FORZA_ABS_SLIP_THRESHOLD: f64 = 1.0;
const FORZA_ABS_PULSE_AMPLITUDE: f64 = 20.0 / 63.0;
const FORZA_ABS_PULSE_FREQUENCY_HZ: f64 = 10.0;
const FORZA_BRAKE_CURVE: f64 = 1.35;
const FORZA_THROTTLE_CURVE: f64 = 2.25;
const FORZA_ENDSTOP_WALL_OFFSET: f64 = 0.03;
const FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET: f64 = 0.28;
const FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION: f64 = 0.70;
const FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH: f64 = 0.16;
const FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE: f64 = 2.0;
const FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION: f64 = 0.80;
const FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION: f64 = 0.80;
const FORZA_BRAKE_ENDSTOP_FORCE_BOOST: f64 = 1.25;
const FORZA_THROTTLE_ENDSTOP_FORCE_BOOST: f64 = 3.0;
const FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH: f64 = 0.20;
const FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE: f64 = 2.4;
const FORZA_SHIFT_THUMP_DEFAULT_INTENSITY: u8 = 255;
const TRIGGER_CURVE_SCALE: f64 = 100.0;
const TRIGGER_CURVE_MIN: u16 = 50;
const TRIGGER_CURVE_MAX: u16 = 350;
const TRIGGER_CURVE_POINT_MIN: usize = 4;
const TRIGGER_CURVE_POINT_MAX: usize = 8;
const FORZA_REV_LIMIT_RATIO: f64 = 0.93;
const FORZA_SHIFT_WALL_FORM_AT: f64 = 0.15;
const FORZA_SHIFT_FREQUENCY_HZ: f64 = 34.0;
const FORZA_SHIFT_WALL_ZONES: f64 = 4.0;
const FORZA_SUSPENSION_IMPACT_TRIGGER_AT: f64 = 0.42;
const FORZA_SUSPENSION_IMPACT_RESET_AT: f64 = 0.22;

/// Built-in Forza preset designed from first principles to be immersive
/// without draining battery. The product owner directive is:
///
/// - Adaptive triggers do the heavy lifting (the DualSense's adaptive
///   triggers are passive solenoid loads — they only draw current while a
///   trigger is being squeezed, so they are essentially free at idle).
/// - Continuous low-amplitude body rumble (the rotating-mass actuators) is
///   the dominant battery drain on a DualSense. Road texture is enabled as
///   the default surface cue, while heavier continuous effects such as
///   rumble strip, suspension impact, tire slip, and puddle drag stay off.
///   Event-driven thumps (gear-shift, handbrake) stay enabled because they
///   only fire for a fraction of a second at a time.
/// - Intensities for the enabled effects are tuned conservatively against
///   the existing first-principles baseline forces in this file
///   (`FORZA_BRAKE_*`, `FORZA_THROTTLE_*`, etc.). All values come from the
///   public DualSense HID spec (trigger force 0..=255, body rumble 0..=255)
///   and physics intuition (real-car ABS modulates ~10-15 Hz, comfortable
///   pulse haptics are 20-50 Hz). No values were taken from any external
///   implementation.
///
/// The preset is written into a controller's saved `ForzaTelemetryConfig`
/// at profile-activation time, so changing profiles immediately rewrites
/// the controller config and the UI re-reads the new values.
fn forza_preset_for_profile(profile_id: &str) -> Option<ForzaTelemetryConfig> {
    match profile_id {
        FORZA_HORIZON_PROFILE_ID => Some(forza_horizon_preset()),
        IMMERSIVE_PROFILE_ID => Some(forza_horizon_immersive_preset()),
        ASSETTO_CORSA_RALLY_PROFILE_ID => Some(assetto_corsa_rally_preset()),
        _ => None,
    }
}

/// Battery-conscious "Base" preset. Adaptive triggers do most of the work,
/// with road texture enabled as the default surface cue.
fn forza_horizon_preset() -> ForzaTelemetryConfig {
    // (id, enabled, intensity 0..=255, route)
    //
    // Routes follow the natural side of each effect:
    //   - Brake / ABS  -> L2 adaptive trigger (left).
    //   - Throttle / rev limiter -> R2 adaptive trigger (right).
    //   - Handbrake -> L2 (driver actuates it from the left side).
    //   - Shift thump -> R2 + reduced body thump (short event, no sustained rumble).
    //
    // Road texture is the stock surface cue. Heavier continuous-rumble
    // effects (rumble strip, suspension impact, tire slip, puddle drag)
    // stay disabled by default; users can opt in via the tuning UI.
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 100, "l2"),
        ("throttle_resistance", true, 100, "r2"),
        ("abs_slip_pulse", true, 100, "l2"),
        ("handbrake_wall", true, 100, "l2"),
        ("rev_limiter_buzz", true, 55, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("road_texture", true, 40, "body_both"),
        ("rumble_strip", false, 55, "body_both"),
        ("tire_slip", false, 65, "body_right"),
        ("puddle_drag", false, 50, "body_left"),
        ("suspension_impact", false, 70, "body_both"),
        ("rpm_leds", false, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
    }
    .normalized()
}

/// Richer "Immersive" preset. This keeps the same trigger language as the stock
/// preset, then adds low-to-mid body layers for slip, curbs, puddles, and
/// suspension. Sustained tire slip stays restrained so it does not blur the
/// controller, while suspension impact is treated as a stronger event cue for
/// landing thumps. Gear LEDs and the RPM bar stay off unless the user opts in.
fn forza_horizon_immersive_preset() -> ForzaTelemetryConfig {
    // (id, enabled, intensity 0..=255, route)
    //
    // Body routing is intentionally spatial:
    //   - Tire slip -> right grip, so traction loss lives on the throttle side.
    //   - Puddle drag -> left grip, so water feels different from throttle load.
    //   - Suspension -> both grips with enough headroom to stand out on landings.
    //   - Rumble strips -> both grips, but below shift and impact events.
    //   - RPM LEDs -> disabled; visual gear/RPM overlays should be opt-in.
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 100, "l2"),
        ("throttle_resistance", true, 100, "r2"),
        ("abs_slip_pulse", true, 100, "l2"),
        ("handbrake_wall", true, 100, "l2"),
        ("rev_limiter_buzz", true, 62, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("road_texture", true, 35, "body_both"),
        ("rumble_strip", true, 38, "body_both"),
        ("tire_slip", true, 30, "body_right"),
        ("puddle_drag", true, 32, "body_left"),
        ("suspension_impact", true, 82, "body_both"),
        ("rpm_leds", false, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
    }
    .normalized()
}

/// Rally preset for Assetto Corsa Rally. It reuses DSCC's normalized racing
/// signal names, but tunes the surface and shift layers for a looser road feel.
fn assetto_corsa_rally_preset() -> ForzaTelemetryConfig {
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 100, "l2"),
        ("throttle_resistance", true, 92, "r2"),
        ("abs_slip_pulse", true, 95, "l2"),
        ("handbrake_wall", true, 115, "l2"),
        ("rev_limiter_buzz", true, 58, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY.saturating_add(22),
            "r2_and_body",
        ),
        ("road_texture", true, 46, "body_both"),
        ("rumble_strip", true, 35, "body_both"),
        ("tire_slip", true, 62, "body_right"),
        ("puddle_drag", false, 28, "body_left"),
        ("suspension_impact", true, 64, "body_both"),
        ("rpm_leds", false, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
    }
    .normalized()
}

fn forza_horizon_trigger_preset() -> TriggerConfig {
    TriggerConfig {
        same_range: false,
        l2_from: 0,
        l2_to: 100,
        r2_from: 4,
        r2_to: 100,
        l2_curve: TriggerCurve::from_ratio(FORZA_BRAKE_CURVE),
        r2_curve: TriggerCurve::from_ratio(FORZA_THROTTLE_CURVE),
        l2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(
            FORZA_BRAKE_CURVE,
        )),
        r2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(
            FORZA_THROTTLE_CURVE,
        )),
        effect: "Adaptive resistance".to_string(),
        intensity: "Strong (Standard)".to_string(),
        vibration: "Medium".to_string(),
        vibration_mode: "Balanced".to_string(),
    }
    .normalized()
}

#[derive(Clone)]
pub struct AgentState {
    inner: Arc<RwLock<AgentStateInner>>,
    event_tx: broadcast::Sender<RealtimeMessage>,
    started_at: Instant,
    bind_addr: SocketAddr,
    output_manager: Option<Arc<ControllerOutputManager<HidApiTransport>>>,
    output_runtime: Arc<Mutex<HardwareOutputRuntime>>,
    discovery_cache: Arc<DiscoveryCache>,
    realtime_runtime: Arc<Mutex<RealtimeRuntime>>,
    effect_runtime: Arc<Mutex<EffectRuntimeCache>>,
}

#[derive(Debug, Default)]
struct HardwareOutputRuntime {
    manual_override_until: Option<Instant>,
    manual_override_generation: u64,
    last_error: Option<String>,
    last_error_at: Option<Instant>,
    last_output_frames: BTreeMap<String, LastHardwareOutputFrame>,
}

#[derive(Debug, Clone)]
struct LastHardwareOutputFrame {
    frame: ControllerOutputFrame,
    written_at: Instant,
}

#[derive(Debug)]
struct DiscoveryCache {
    game_detection: AsyncMutex<CachedValue<GameDetectionResponse>>,
    steam_input: AsyncMutex<CachedValue<SteamInputStatus>>,
    steam_game_catalog: AsyncMutex<CachedValue<SteamGameCatalog>>,
    update_check: AsyncMutex<CachedValue<UpdateCheckResponse>>,
    steam_input_refreshing: AtomicBool,
}

impl Default for DiscoveryCache {
    fn default() -> Self {
        Self {
            game_detection: AsyncMutex::new(CachedValue::default()),
            steam_input: AsyncMutex::new(CachedValue::default()),
            steam_game_catalog: AsyncMutex::new(CachedValue::default()),
            update_check: AsyncMutex::new(CachedValue::default()),
            steam_input_refreshing: AtomicBool::new(false),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SteamGameCatalog {
    supported_games: Vec<SupportedGameSummary>,
    artwork_paths: BTreeMap<(String, String), PathBuf>,
}

#[derive(Debug)]
struct CachedValue<T> {
    value: Option<T>,
    refreshed_at: Option<Instant>,
}

impl<T> Default for CachedValue<T> {
    fn default() -> Self {
        Self {
            value: None,
            refreshed_at: None,
        }
    }
}

impl<T: Clone> CachedValue<T> {
    fn fresh(&self, ttl: Duration, now: Instant) -> Option<T> {
        match (self.value.as_ref(), self.refreshed_at) {
            (Some(value), Some(refreshed_at)) if now.duration_since(refreshed_at) < ttl => {
                Some(value.clone())
            }
            _ => None,
        }
    }

    fn store(&mut self, value: T, now: Instant) -> T {
        self.value = Some(value.clone());
        self.refreshed_at = Some(now);
        value
    }
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum VersionOrdering {
    Older,
    SameOrNewer,
    Unknown,
}

#[derive(Debug, Default)]
struct RealtimeRuntime {
    last_telemetry_event_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EffectEnginePurpose {
    Preview,
    Hardware,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EffectEngineKey {
    purpose: EffectEnginePurpose,
    controller_id: String,
    profile_id: String,
    revision: u64,
}

#[derive(Debug, Default)]
struct EffectRuntimeCache {
    engines: BTreeMap<EffectEngineKey, EffectEngine>,
}

impl EffectRuntimeCache {
    fn evaluate(
        &mut self,
        key: EffectEngineKey,
        profile: &Profile,
        snapshot: &SignalSnapshot,
    ) -> ControllerOutputFrame {
        if self.engines.len() > 16 {
            self.engines
                .retain(|existing, _| existing.revision == key.revision);
        }
        self.engines
            .entry(key)
            .or_default()
            .evaluate(profile, snapshot)
    }
}

#[derive(Debug)]
struct AgentStateInner {
    controllers: ControllerRegistry,
    controller_names: BTreeMap<String, String>,
    profiles: Vec<ProfileSummary>,
    adapters: Vec<AdapterSummary>,
    telemetry: SignalSnapshot,
    logs: Vec<LogEntry>,
    device_backend: DeviceBackendSummary,
    storage: Option<PersistenceStore>,
    controller_configs: BTreeMap<String, ControllerConfig>,
    profile_configs: BTreeMap<String, ProfileConfig>,
    profile_overrides: BTreeMap<String, ProfileOverride>,
    edge_profiles: BTreeMap<String, EdgeProfileStore>,
    app_settings: AppSettings,
    active_profile_id: Option<String>,
    active_adapter_id: Option<String>,
    auto_loaded_profile_id: Option<String>,
    adapter_runtimes: BTreeMap<String, AdapterRuntime>,
    forza_effect_runtime: ForzaEffectRuntime,
    effect_revision: u64,
    user_games: BTreeMap<String, UserGameConfig>,
}

#[derive(Debug, Clone)]
struct AdapterRuntime {
    adapter_id: String,
    display_name: String,
    protocol: AdapterProtocol,
    default_port: Option<u16>,
    bind_addr: Option<SocketAddr>,
    listener_bound: bool,
    listener_started_at: Option<Instant>,
    last_error: Option<String>,
    packet_count: u64,
    packet_rate_hz: Option<u16>,
    rate_window_started_at: Option<Instant>,
    rate_window_packet_count: u64,
    first_packet_at: Option<Instant>,
    last_packet_at: Option<Instant>,
    last_packet_len: Option<usize>,
    last_packet_sequence: Option<u64>,
    parse_error_count: u64,
    last_parse_error_len: Option<usize>,
    last_parse_error: Option<String>,
    last_parse_error_at: Option<Instant>,
}

impl Default for AdapterRuntime {
    fn default() -> Self {
        Self {
            adapter_id: String::new(),
            display_name: String::new(),
            protocol: AdapterProtocol::Custom,
            default_port: None,
            bind_addr: None,
            listener_bound: false,
            listener_started_at: None,
            last_error: None,
            packet_count: 0,
            packet_rate_hz: None,
            rate_window_started_at: None,
            rate_window_packet_count: 0,
            first_packet_at: None,
            last_packet_at: None,
            last_packet_len: None,
            last_packet_sequence: None,
            parse_error_count: 0,
            last_parse_error_len: None,
            last_parse_error: None,
            last_parse_error_at: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ForzaEffectRuntime {
    prev_shift_gear: Option<u8>,
    latched_shift_event: Option<&'static str>,
    latched_shift_until: Option<Instant>,
    prev_suspension_impact: f64,
    latched_suspension_impact: f64,
    latched_suspension_impact_until: Option<Instant>,
}

impl AdapterRuntime {
    fn for_udp_adapter(adapter: UdpTelemetryAdapter) -> Self {
        Self {
            adapter_id: adapter.id.to_string(),
            display_name: adapter.display_name.to_string(),
            protocol: AdapterProtocol::Udp,
            default_port: Some(adapter.default_port),
            ..Self::default()
        }
    }

    fn for_built_in_adapter(adapter: &BuiltInAdapter) -> Self {
        Self {
            adapter_id: adapter.id.to_string(),
            display_name: adapter.display_name.to_string(),
            protocol: adapter.protocol,
            default_port: adapter.default_port,
            ..Self::default()
        }
    }

    #[cfg(any(target_os = "windows", test))]
    fn mark_ready(&mut self) {
        self.listener_bound = true;
        self.listener_started_at.get_or_insert_with(Instant::now);
        self.last_error = None;
    }

    fn mark_bound(&mut self, bind_addr: SocketAddr) {
        self.bind_addr = Some(bind_addr);
        self.listener_bound = true;
        self.listener_started_at = Some(Instant::now());
        self.last_error = None;
    }

    fn mark_bind_error(&mut self, bind_addr: SocketAddr, error: impl Into<String>) {
        self.bind_addr = Some(bind_addr);
        self.listener_bound = false;
        self.last_error = Some(error.into());
    }

    fn mark_packet(&mut self, packet_len: usize, sequence: u64) -> u16 {
        let now = Instant::now();
        let window_start = *self.rate_window_started_at.get_or_insert(now);
        self.rate_window_packet_count = self.rate_window_packet_count.saturating_add(1);
        let window_seconds = now.duration_since(window_start).as_secs_f64();
        let rate = if window_seconds >= 1.0 {
            let rate = ((self.rate_window_packet_count.saturating_sub(1)) as f64 / window_seconds)
                .round()
                .clamp(1.0, 1000.0) as u16;
            self.rate_window_started_at = Some(now);
            self.rate_window_packet_count = 1;
            rate
        } else {
            self.packet_rate_hz.unwrap_or_default()
        };

        self.packet_count = self.packet_count.saturating_add(1);
        self.packet_rate_hz = Some(rate);
        self.first_packet_at.get_or_insert(now);
        self.last_packet_at = Some(now);
        self.last_packet_len = Some(packet_len);
        self.last_packet_sequence = Some(sequence);
        self.last_error = None;
        rate
    }

    fn mark_parse_error(&mut self, packet_len: usize, sequence: u64) {
        self.parse_error_count = self.parse_error_count.saturating_add(1);
        if self.last_parse_error_len != Some(packet_len) {
            self.last_parse_error = Some(format!(
                "unsupported {} packet length {packet_len}",
                self.display_name
            ));
            self.last_parse_error_len = Some(packet_len);
        }
        self.last_parse_error_at = Some(Instant::now());
        self.last_packet_sequence = Some(sequence);
    }

    fn has_recent_packet(&self, now: Instant) -> bool {
        self.last_packet_at.is_some_and(|last_packet_at| {
            now.duration_since(last_packet_at) <= TELEMETRY_PACKET_STALE_AFTER
        })
    }
}

impl ForzaEffectRuntime {
    fn latch_shift_event(&mut self, event: &'static str, now: Instant) {
        if event == "none" {
            return;
        }

        self.latched_shift_event = Some(event);
        self.latched_shift_until = Some(now + FORZA_SHIFT_EVENT_HOLD);
    }

    fn detect_shift_event(
        &mut self,
        current_gear: Option<f64>,
        telemetry_on: bool,
        shift_enabled: bool,
        now: Instant,
    ) -> Option<&'static str> {
        if !telemetry_on || !shift_enabled {
            return Some("none");
        }

        let current_gear = signal_gear_to_u8(current_gear?)?;
        let event = match self.prev_shift_gear {
            Some(previous_gear) if previous_gear != current_gear => "shift",
            _ => "none",
        };

        self.prev_shift_gear = Some(current_gear);
        self.latch_shift_event(event, now);
        Some(event)
    }

    fn latched_shift_event(&self, now: Instant) -> Option<&'static str> {
        self.latched_shift_event
            .filter(|_| self.latched_shift_until.is_some_and(|until| now < until))
    }

    fn latch_suspension_impact(&mut self, strength: f64, now: Instant) {
        self.latched_suspension_impact = clamp_unit(strength);
        self.latched_suspension_impact_until = Some(now + FORZA_SUSPENSION_IMPACT_HOLD);
    }

    fn detect_suspension_impact(
        &mut self,
        suspension_travel: Option<f64>,
        acceleration_magnitude: Option<f64>,
        speed_kmh: Option<f64>,
        telemetry_on: bool,
        impact_enabled: bool,
        now: Instant,
    ) -> f64 {
        if !telemetry_on || !impact_enabled {
            self.prev_suspension_impact = 0.0;
            self.latched_suspension_impact = 0.0;
            self.latched_suspension_impact_until = None;
            return 0.0;
        }

        let impact =
            suspension_impact_strength(suspension_travel, acceleration_magnitude, speed_kmh);
        let rising_impact = impact >= FORZA_SUSPENSION_IMPACT_TRIGGER_AT
            && self.prev_suspension_impact <= FORZA_SUSPENSION_IMPACT_RESET_AT;
        self.prev_suspension_impact = impact;

        if rising_impact
            || (self
                .latched_suspension_impact_until
                .is_some_and(|until| now < until)
                && impact > self.latched_suspension_impact)
        {
            self.latch_suspension_impact(impact, now);
        }

        self.latched_suspension_impact(now)
    }

    fn latched_suspension_impact(&self, now: Instant) -> f64 {
        if self
            .latched_suspension_impact_until
            .is_some_and(|until| now < until)
        {
            self.latched_suspension_impact
        } else {
            0.0
        }
    }
}

impl AgentStateInner {
    fn adapter_runtime(&self, adapter_id: &str) -> Option<&AdapterRuntime> {
        self.adapter_runtimes.get(adapter_id)
    }

    fn adapter_runtime_mut(&mut self, adapter_id: &str) -> &mut AdapterRuntime {
        self.adapter_runtimes
            .entry(adapter_id.to_string())
            .or_insert_with(|| {
                built_in_udp_adapters()
                    .iter()
                    .find(|adapter| adapter.id == adapter_id)
                    .copied()
                    .map(AdapterRuntime::for_udp_adapter)
                    .unwrap_or_else(|| AdapterRuntime {
                        adapter_id: adapter_id.to_string(),
                        display_name: adapter_id.to_string(),
                        protocol: AdapterProtocol::Custom,
                        default_port: None,
                        ..AdapterRuntime::default()
                    })
            })
    }

    #[cfg(test)]
    fn require_adapter_runtime(&self, adapter_id: &str) -> &AdapterRuntime {
        self.adapter_runtime(adapter_id)
            .expect("built-in adapter runtime is initialized")
    }
}

fn default_adapter_runtimes() -> BTreeMap<String, AdapterRuntime> {
    let mut runtimes: BTreeMap<String, AdapterRuntime> = built_in_adapters()
        .iter()
        .map(|adapter| {
            (
                adapter.id.to_string(),
                AdapterRuntime::for_built_in_adapter(adapter),
            )
        })
        .collect();
    for adapter in built_in_udp_adapters() {
        runtimes.insert(
            adapter.id.to_string(),
            AdapterRuntime::for_udp_adapter(*adapter),
        );
    }
    runtimes
}

fn signal_gear_to_u8(value: f64) -> Option<u8> {
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
}

#[derive(Debug, Deserialize)]
pub struct UpdateControllerRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeProfileSupportState {
    Unsupported,
    Unknown,
    ReadOnly,
    ReadWrite,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeProfileSlotState {
    Default,
    Assigned,
    Empty,
    Active,
    Unknown,
    Faulted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileSlot {
    pub slot_id: String,
    pub shortcut: String,
    pub name: Option<String>,
    pub state: EdgeProfileSlotState,
    pub editable: bool,
    pub hardware_synced: bool,
    pub staged: Option<EdgeProfileSlotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfilesResponse {
    pub controller_id: String,
    pub support_state: EdgeProfileSupportState,
    pub warning: String,
    pub slots: Vec<EdgeProfileSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileSlotConfig {
    pub name: String,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    pub updated_at: String,
    pub hardware_synced: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileStore {
    pub slots: BTreeMap<String, EdgeProfileSlotConfig>,
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
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControllerInputMode {
    #[serde(rename = "native_dualsense")]
    #[default]
    NativeDualSense,
    SteamInputCompanion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerCurve(u16);

impl TriggerCurve {
    const fn default_l2() -> Self {
        Self(135)
    }

    const fn default_r2() -> Self {
        Self(185)
    }

    fn from_ratio(value: f64) -> Self {
        if !value.is_finite() {
            return Self::default_l2();
        }
        Self((value * TRIGGER_CURVE_SCALE).round() as u16).normalized()
    }

    fn as_f64(self) -> f64 {
        f64::from(self.normalized().0) / TRIGGER_CURVE_SCALE
    }

    fn normalized(self) -> Self {
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

fn default_l2_trigger_curve() -> TriggerCurve {
    TriggerCurve::default_l2()
}

fn default_r2_trigger_curve() -> TriggerCurve {
    TriggerCurve::default_r2()
}

fn default_l2_trigger_curve_points() -> Vec<TriggerCurvePoint> {
    trigger_curve_points_from_curve(TriggerCurve::default_l2())
}

fn default_r2_trigger_curve_points() -> Vec<TriggerCurvePoint> {
    trigger_curve_points_from_curve(TriggerCurve::default_r2())
}

fn trigger_curve_points_from_curve(curve: TriggerCurve) -> Vec<TriggerCurvePoint> {
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

fn normalize_trigger_curve_points(
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

fn trigger_curve_value_points(points: &[TriggerCurvePoint]) -> Vec<ValuePoint> {
    points
        .iter()
        .map(|point| ValuePoint {
            input: f64::from(point.input) / 100.0,
            output: f64::from(point.output) / 100.0,
        })
        .collect()
}

fn default_vibration_mode() -> String {
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
    pub game_detection: GameDetectionResponse,
    pub profile_resolution: ProfileResolutionResponse,
    pub effect_state: CurrentEffectResponse,
    pub telemetry: Vec<TelemetrySignalResponse>,
    pub logs: Vec<LogEntry>,
    pub diagnostics: DiagnosticsResponse,
    pub partial_errors: Vec<SnapshotPartialError>,
}

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
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEdgeProfileRequest {
    pub name: String,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionResponse {
    pub active_game_id: Option<String>,
    pub active_game_name: Option<String>,
    pub source: String,
    pub confidence: u8,
    pub process_name: Option<String>,
    pub module_id: Option<String>,
    pub adapter_id: Option<String>,
    pub profile_id: Option<String>,
    pub candidates: Vec<GameDetectionCandidate>,
    #[serde(default)]
    pub supported_games: Vec<SupportedGameSummary>,
    #[serde(default)]
    pub selected_game: Option<SupportedGameSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionCandidate {
    pub game_id: String,
    pub name: String,
    pub process_name: String,
    pub module_id: String,
    pub adapter_id: String,
    pub profile_id: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameArtwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamGameStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub achievements: Option<SteamAchievementStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamAchievementStats {
    pub unlocked: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportedGameSummary {
    pub game_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    pub installed: bool,
    pub running: bool,
    pub support_level: String,
    #[serde(default)]
    pub artwork: GameArtwork,
    #[serde(default)]
    pub stats: SteamGameStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserGameConfig {
    pub game_id: String,
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryEntry {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    pub artwork: GameArtwork,
    pub stats: SteamGameStats,
    pub already_in_catalog: bool,
    pub suggested_game_id: String,
    pub process_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryListResponse {
    pub games: Vec<SteamLibraryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameRequest {
    pub app_id: String,
    /// Optional override for the .exe process names DSCC will watch for to
    /// auto-load this game's profile. When omitted/empty, the agent uses the
    /// candidates it discovered by scanning the install directory. Useful for
    /// games whose .exe lives in a subfolder or isn't named obviously.
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameResponse {
    pub game: SupportedGameSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseSteamLibraryParams {
    pub app_id: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseEntry {
    pub name: String,
    /// `"dir"` or `"exe"`. Kept as a string so the wire shape is forward
    /// compatible if we ever surface other kinds (data files, configs, ...).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseResponse {
    pub app_id: String,
    pub install_path: String,
    /// Path relative to `install_path`, using forward slashes and never
    /// containing `..` segments. Empty string means the install root.
    pub relative_path: String,
    pub entries: Vec<SteamLibraryBrowseEntry>,
    #[serde(default)]
    pub truncated: bool,
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
    pub l2: f64,
    pub r2: f64,
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
struct DeviceBackendSummary {
    status: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct PersistenceStore {
    state_file: PathBuf,
}

const PERSISTED_STATE_VERSION: u32 = 7;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAgentState {
    version: u32,
    profiles: Vec<ProfileSummary>,
    #[serde(default)]
    controller_names: BTreeMap<String, String>,
    controller_configs: BTreeMap<String, ControllerConfig>,
    #[serde(default)]
    profile_configs: BTreeMap<String, ProfileConfig>,
    profile_overrides: BTreeMap<String, ProfileOverride>,
    edge_profiles: BTreeMap<String, EdgeProfileStore>,
    app_settings: AppSettings,
    active_profile_id: Option<String>,
    #[serde(default)]
    user_games: BTreeMap<String, UserGameConfig>,
}

impl DeviceBackendSummary {
    fn mock() -> Self {
        Self {
            status: "mock".to_string(),
            detail: "Controller discovery is running through dscc-device mock transport"
                .to_string(),
        }
    }

    fn hid(output_mode: OutputMode) -> Self {
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

    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: "unavailable".to_string(),
            detail: reason.into(),
        }
    }
}

impl PersistenceStore {
    fn default() -> Option<Self> {
        if let Some(config_dir) = std::env::var_os("DSCC_CONFIG_DIR") {
            return Some(Self {
                state_file: PathBuf::from(config_dir).join("state.json"),
            });
        }

        ProjectDirs::from("dev", "DualSenseCommand", "DualSenseCommandCenter").map(|dirs| Self {
            state_file: dirs.config_dir().join("state.json"),
        })
    }

    fn load(&self) -> io::Result<PersistedAgentState> {
        if !self.state_file.exists() {
            return Ok(PersistedAgentState::default());
        }

        let contents = fs::read_to_string(&self.state_file)?;
        serde_json::from_str(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn save_snapshot(&self, snapshot: &PersistedAgentState) -> io::Result<()> {
        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let temp_file = temp_path_for(&self.state_file);
        fs::write(&temp_file, contents)?;
        if self.state_file.exists() {
            fs::remove_file(&self.state_file)?;
        }
        fs::rename(temp_file, &self.state_file)
    }
}

impl PersistedAgentState {
    fn normalized(mut self) -> Self {
        self.profiles = self
            .profiles
            .into_iter()
            .filter_map(|mut profile| {
                let id = profile.id.trim().to_string();
                if id.is_empty() || is_default_profile_id(&id) {
                    return None;
                }
                profile.id = id;
                profile.built_in = false;
                profile.active = false;
                Some(profile)
            })
            .collect();
        let persisted_profiles = self.profiles.clone();
        self.controller_names = self
            .controller_names
            .into_iter()
            .filter_map(|(id, name)| {
                let id = id.trim().chars().take(160).collect::<String>();
                let name = normalize_controller_display_name(&name)?;
                (!id.is_empty()).then_some((id, name))
            })
            .collect();
        self.controller_configs = self
            .controller_configs
            .into_iter()
            .map(|(id, config)| {
                let mut config = config.normalized();
                config.profile_assignments = normalize_existing_profile_assignments(
                    config.profile_assignments,
                    &persisted_profiles,
                );
                (id, config)
            })
            .collect();
        self.profile_configs = self
            .profile_configs
            .into_iter()
            .filter(|(id, _)| {
                let id = id.trim();
                !id.is_empty()
                    && !is_default_profile_id(id)
                    && profile_exists_in_defaults_or_persisted(id, &persisted_profiles)
            })
            .map(|(id, config)| {
                let config = config.normalized_for_model("DualSense");
                (id, config)
            })
            .collect();
        self.edge_profiles = self
            .edge_profiles
            .into_iter()
            .map(|(id, store)| (id, store.normalized()))
            .collect();
        self.profile_overrides = self
            .profile_overrides
            .into_iter()
            .filter_map(|(key, mut profile)| {
                let profile_id = profile.profile_id.trim().to_string();
                if profile_id.is_empty()
                    || !profile_exists_in_defaults_or_persisted(&profile_id, &persisted_profiles)
                {
                    return None;
                }
                profile.profile_id = profile_id;
                Some((key, profile))
            })
            .collect();
        self.active_profile_id = self.active_profile_id.and_then(|id| {
            let id = id.trim().to_string();
            (!id.is_empty() && profile_exists_in_defaults_or_persisted(&id, &persisted_profiles))
                .then_some(id)
        });
        self.app_settings.forza_playstation_glyphs.install_path = self
            .app_settings
            .forza_playstation_glyphs
            .install_path
            .and_then(|path| (!path.trim().is_empty()).then_some(path));
        self.user_games = self
            .user_games
            .into_iter()
            .filter_map(|(id, config)| {
                let game_id = config.game_id.trim().to_string();
                if game_id.is_empty()
                    || game_id != id.trim()
                    || built_in_game_modules()
                        .iter()
                        .any(|module| module.id == game_id)
                {
                    return None;
                }
                let mut config = config;
                config.game_id = game_id.clone();
                config.app_id = config.app_id.trim().to_string();
                config.name = config.name.trim().to_string();
                config.install_dir = config.install_dir.trim().to_string();
                config.install_path = config.install_path.trim().to_string();
                config.process_names = config
                    .process_names
                    .into_iter()
                    .filter_map(|name| {
                        let trimmed = name.trim();
                        (!trimmed.is_empty()).then(|| trimmed.to_string())
                    })
                    .collect();
                if config.app_id.is_empty() || config.name.is_empty() {
                    return None;
                }
                Some((game_id, config))
            })
            .collect();
        self.version = PERSISTED_STATE_VERSION;
        self
    }

    fn from_inner(inner: &AgentStateInner) -> Self {
        Self {
            version: PERSISTED_STATE_VERSION,
            profiles: inner
                .profiles
                .iter()
                .filter(|profile| !profile.built_in)
                .cloned()
                .collect(),
            controller_names: inner.controller_names.clone(),
            controller_configs: inner.controller_configs.clone(),
            profile_configs: inner
                .profile_configs
                .iter()
                .filter(|(id, _)| !is_default_profile_id(id))
                .map(|(id, config)| (id.clone(), config.clone()))
                .collect(),
            profile_overrides: inner.profile_overrides.clone(),
            edge_profiles: inner.edge_profiles.clone(),
            app_settings: inner.app_settings.clone(),
            active_profile_id: inner.active_profile_id.clone(),
            user_games: inner.user_games.clone(),
        }
    }
}

fn temp_path_for(path: &FsPath) -> PathBuf {
    let mut temp = path.to_path_buf();
    temp.set_extension("json.tmp");
    temp
}

fn default_profiles() -> Vec<ProfileSummary> {
    vec![
        ProfileSummary {
            id: DEFAULT_PROFILE_ID.to_string(),
            name: "Global".to_string(),
            built_in: true,
            active: true,
            game_id: None,
        },
        ProfileSummary {
            id: FORZA_HORIZON_PROFILE_ID.to_string(),
            name: "Base".to_string(),
            built_in: true,
            active: false,
            game_id: None,
        },
        ProfileSummary {
            id: IMMERSIVE_PROFILE_ID.to_string(),
            name: "Immersive".to_string(),
            built_in: true,
            active: false,
            game_id: None,
        },
        ProfileSummary {
            id: ASSETTO_CORSA_RALLY_PROFILE_ID.to_string(),
            name: "Rally".to_string(),
            built_in: true,
            active: false,
            game_id: Some("assetto-corsa-rally".to_string()),
        },
    ]
}

fn merge_profiles(persisted_profiles: Vec<ProfileSummary>) -> Vec<ProfileSummary> {
    let mut profiles = default_profiles();
    for mut profile in persisted_profiles {
        profile.built_in = false;
        profile.active = false;
        profile.game_id = normalize_optional_profile_game_id(profile.game_id);
        if !profile.id.trim().is_empty() && !profiles.iter().any(|item| item.id == profile.id) {
            profiles.push(profile);
        }
    }
    profiles
}

fn normalize_optional_profile_game_id(game_id: Option<String>) -> Option<String> {
    game_id
        .map(|id| id.trim().chars().take(96).collect::<String>())
        .filter(|id| !id.is_empty() && id != "all" && id != "global")
}

fn profiles_with_active(
    mut profiles: Vec<ProfileSummary>,
    active_profile_id: &Option<String>,
) -> Vec<ProfileSummary> {
    let active = active_profile_id.as_deref().unwrap_or(DEFAULT_PROFILE_ID);
    for profile in &mut profiles {
        profile.active = profile.id == active;
    }
    profiles
}

fn profile_exists_in_defaults_or_persisted(
    id: &str,
    persisted_profiles: &[ProfileSummary],
) -> bool {
    is_default_profile_id(id) || persisted_profiles.iter().any(|profile| profile.id == id)
}

fn is_default_profile_id(id: &str) -> bool {
    default_profiles().iter().any(|profile| profile.id == id)
}

#[derive(Clone)]
enum SelectedProfileConfig {
    Full(ProfileConfig),
    BuiltInPreset {
        trigger: Option<TriggerConfig>,
        forza: ForzaTelemetryConfig,
    },
}

fn selected_profile_config(
    inner: &AgentStateInner,
    profile_id: &str,
) -> Option<SelectedProfileConfig> {
    if let Some(forza) = forza_preset_for_profile(profile_id) {
        return Some(SelectedProfileConfig::BuiltInPreset {
            trigger: forza_trigger_preset_for_profile(profile_id),
            forza,
        });
    }

    if is_default_profile_id(profile_id) {
        return None;
    }

    inner
        .profile_configs
        .get(profile_id)
        .cloned()
        .map(SelectedProfileConfig::Full)
        .or_else(|| {
            forza_preset_for_profile(profile_id).map(|forza| SelectedProfileConfig::BuiltInPreset {
                trigger: forza_trigger_preset_for_profile(profile_id),
                forza,
            })
        })
}

fn forza_trigger_preset_for_profile(profile_id: &str) -> Option<TriggerConfig> {
    matches!(
        profile_id,
        FORZA_HORIZON_PROFILE_ID | IMMERSIVE_PROFILE_ID | ASSETTO_CORSA_RALLY_PROFILE_ID
    )
    .then(forza_horizon_trigger_preset)
}

fn apply_selected_profile_config(config: &mut ControllerConfig, selected: &SelectedProfileConfig) {
    match selected {
        SelectedProfileConfig::Full(profile_config) => {
            profile_config.apply_to_controller_config(config);
        }
        SelectedProfileConfig::BuiltInPreset { trigger, forza } => {
            if let Some(trigger) = trigger {
                config.trigger = trigger.clone();
            }
            config.forza = forza.clone();
        }
    }
}

fn apply_profile_config_to_controllers(
    inner: &mut AgentStateInner,
    selected_config: &SelectedProfileConfig,
) {
    let connected_models: BTreeMap<String, String> = inner
        .controllers
        .summaries()
        .into_iter()
        .filter(|controller| controller.connected)
        .map(|controller| (controller.id, controller.model))
        .collect();
    let mut controller_ids: Vec<String> = inner.controller_configs.keys().cloned().collect();
    for controller_id in connected_models.keys() {
        if !controller_ids.iter().any(|id| id == controller_id) {
            controller_ids.push(controller_id.clone());
        }
    }

    for controller_id in controller_ids {
        let model = connected_models
            .get(&controller_id)
            .cloned()
            .or_else(|| {
                inner
                    .controller_configs
                    .get(&controller_id)
                    .map(|config| config.model.clone())
            })
            .unwrap_or_else(|| "DualSense".to_string());
        let config = inner
            .controller_configs
            .entry(controller_id.clone())
            .or_insert_with(|| ControllerConfig::default_for(controller_id.clone(), model));
        apply_selected_profile_config(config, selected_config);
    }
}

fn apply_profile_selection_config(inner: &mut AgentStateInner, profile_id: &str) {
    if let Some(selected_config) = selected_profile_config(inner, profile_id) {
        apply_profile_config_to_controllers(inner, &selected_config);
    }
}

fn sync_auto_loaded_profile_for_detection(
    inner: &mut AgentStateInner,
    game_detection: &GameDetectionResponse,
) -> bool {
    let target_profile_id = if game_detection.profile_id.is_some() {
        profile_resolution(inner, Some(game_detection)).selected_profile_id
    } else {
        None
    };

    if inner.auto_loaded_profile_id == target_profile_id {
        return false;
    }

    match target_profile_id.as_deref() {
        Some(profile_id) => {
            apply_profile_selection_config(inner, profile_id);
        }
        None => {
            let fallback_profile_id = inner
                .active_profile_id
                .clone()
                .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
            apply_profile_selection_config(inner, &fallback_profile_id);
        }
    }

    inner.auto_loaded_profile_id = target_profile_id;
    inner.effect_revision = inner.effect_revision.saturating_add(1);
    true
}

fn build_persist_snapshot(
    inner: &AgentStateInner,
) -> Option<(PersistenceStore, PersistedAgentState)> {
    inner
        .storage
        .clone()
        .map(|store| (store, PersistedAgentState::from_inner(inner)))
}

async fn persist_snapshot(
    state: &AgentState,
    to_save: Option<(PersistenceStore, PersistedAgentState)>,
) {
    let Some((store, snapshot)) = to_save else {
        return;
    };
    let result = tokio::task::spawn_blocking(move || store.save_snapshot(&snapshot)).await;
    let save_error = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error.to_string(),
        Err(join_error) => format!("persistence task panicked: {join_error}"),
    };
    state
        .log_warn(format!("Could not persist DSCC state: {save_error}"))
        .await;
}

fn discover_steam_input_status() -> SteamInputStatus {
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

async fn discover_steam_input_status_async() -> SteamInputStatus {
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

fn pending_steam_input_status() -> SteamInputStatus {
    SteamInputStatus {
        running: false,
        available: false,
        steam_path: None,
        layouts: Vec::new(),
        warnings: vec!["Steam Input discovery is warming in the background.".to_string()],
    }
}

fn steam_input_discovery_pending(status: &SteamInputStatus) -> bool {
    status
        .warnings
        .iter()
        .any(|warning| warning.contains("warming in the background"))
}

#[derive(Debug, Clone)]
struct SteamAppManifest {
    app_id: String,
    name: String,
    install_dir: String,
    install_path: PathBuf,
}

fn discover_steam_game_catalog() -> SteamGameCatalog {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return unsupported_steam_game_catalog();
    };

    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    build_supported_steam_game_catalog(&steam_root, &libraries, &manifests)
}

fn unsupported_steam_game_catalog() -> SteamGameCatalog {
    SteamGameCatalog {
        supported_games: built_in_game_modules()
            .iter()
            .filter(|game| game.steam_catalog)
            .map(|game| {
                supported_game_summary(
                    game,
                    None,
                    None,
                    GameArtwork::default(),
                    SteamGameStats::default(),
                )
            })
            .collect(),
        artwork_paths: BTreeMap::new(),
    }
}

fn build_supported_steam_game_catalog(
    steam_root: &FsPath,
    libraries: &[PathBuf],
    manifests: &[SteamAppManifest],
) -> SteamGameCatalog {
    let mut supported_games = Vec::new();
    let mut artwork_paths = BTreeMap::new();
    let steam_stats = discover_steam_game_stats(steam_root);

    for game in built_in_game_modules()
        .iter()
        .filter(|game| game.steam_catalog)
    {
        let manifest = manifests
            .iter()
            .find(|manifest| steam_manifest_matches_game(manifest, game));
        let install_path = manifest
            .map(|manifest| manifest.install_path.clone())
            .or_else(|| find_steam_common_install_dir(libraries, game));
        let app_id = manifest
            .map(|manifest| manifest.app_id.clone())
            .or_else(|| {
                game.steam_app_ids
                    .first()
                    .map(|app_id| (*app_id).to_string())
            });
        let mut artwork = GameArtwork::default();

        if let Some(app_id) = app_id.as_deref() {
            for (kind, path) in discover_steam_artwork_paths(steam_root, app_id) {
                let key = (game.id.to_string(), kind);
                artwork_paths.insert(key.clone(), path);
                match key.1.as_str() {
                    "icon" => artwork.icon_url = Some(game_art_url(game.id, "icon")),
                    "banner" => artwork.banner_url = Some(game_art_url(game.id, "banner")),
                    "hero" => artwork.hero_url = Some(game_art_url(game.id, "hero")),
                    "capsule" => artwork.capsule_url = Some(game_art_url(game.id, "capsule")),
                    _ => {}
                }
            }
        }

        if let Some(app_id) = app_id.as_deref() {
            apply_steam_cdn_artwork_fallback(&mut artwork, app_id);
        }
        let stats = app_id
            .as_deref()
            .and_then(|app_id| steam_stats.get(app_id))
            .cloned()
            .unwrap_or_default();

        supported_games.push(supported_game_summary(
            game,
            app_id,
            install_path,
            artwork,
            stats,
        ));
    }

    SteamGameCatalog {
        supported_games,
        artwork_paths,
    }
}

fn discover_steam_game_stats(steam_root: &FsPath) -> BTreeMap<String, SteamGameStats> {
    let mut stats = BTreeMap::new();
    for user_dir in numeric_child_dirs(&steam_root.join("userdata"), 8) {
        let local_config = user_dir.join("config").join("localconfig.vdf");
        if let Ok(contents) = fs::read_to_string(&local_config) {
            merge_steam_game_stats_map(&mut stats, parse_steam_localconfig_stats(&contents));
        }
        merge_steam_game_achievement_cache(
            &mut stats,
            &user_dir.join("config").join("librarycache"),
        );
    }
    stats
}

fn merge_steam_game_stats_map(
    target: &mut BTreeMap<String, SteamGameStats>,
    updates: BTreeMap<String, SteamGameStats>,
) {
    for (app_id, update) in updates {
        merge_steam_game_stats(target.entry(app_id).or_default(), update);
    }
}

fn merge_steam_game_stats(target: &mut SteamGameStats, update: SteamGameStats) {
    if let Some(minutes) = update.playtime_minutes {
        target.playtime_minutes = Some(target.playtime_minutes.unwrap_or(0).max(minutes));
    }
    if let Some(last_played) = update.last_played_unix {
        target.last_played_unix = Some(target.last_played_unix.unwrap_or(0).max(last_played));
    }
    if let Some(achievements) = update.achievements {
        let replace = match target.achievements.as_ref() {
            Some(current) => {
                achievements.total > current.total
                    || (achievements.total == current.total
                        && achievements.unlocked > current.unlocked)
            }
            None => true,
        };
        if replace {
            target.achievements = Some(achievements);
        }
    }
}

fn parse_steam_localconfig_stats(contents: &str) -> BTreeMap<String, SteamGameStats> {
    let mut stats: BTreeMap<String, SteamGameStats> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
            continue;
        }
        if line == "}" {
            pending_block = None;
            stack.pop();
            continue;
        }

        let tokens = quoted_tokens(line);
        match tokens.as_slice() {
            [block] => pending_block = Some(block.to_string()),
            [key, value] => {
                pending_block = None;
                let Some(app_id) = steam_app_id_from_vdf_stack(&stack) else {
                    continue;
                };
                let entry = stats.entry(app_id.to_string()).or_default();
                match key.as_str() {
                    "Playtime" => entry.playtime_minutes = value.parse::<u64>().ok(),
                    "LastPlayed" => entry.last_played_unix = value.parse::<u64>().ok(),
                    _ => {}
                }
            }
            _ => pending_block = None,
        }
    }

    stats
}

fn steam_app_id_from_vdf_stack(stack: &[String]) -> Option<&str> {
    stack
        .windows(2)
        .rev()
        .find(|window| window[0] == "apps" && window[1].chars().all(|ch| ch.is_ascii_digit()))
        .map(|window| window[1].as_str())
}

fn merge_steam_game_achievement_cache(
    stats: &mut BTreeMap<String, SteamGameStats>,
    library_cache: &FsPath,
) {
    let progress = library_cache.join("achievement_progress.json");
    if let Ok(contents) = fs::read_to_string(progress) {
        for (app_id, achievements) in parse_steam_achievement_progress_cache(&contents) {
            merge_steam_game_stats(
                stats.entry(app_id).or_default(),
                SteamGameStats {
                    achievements: Some(achievements),
                    ..SteamGameStats::default()
                },
            );
        }
    }

    for app_id in built_in_game_modules()
        .iter()
        .filter(|game| game.steam_catalog)
        .flat_map(|game| game.steam_app_ids)
    {
        let app_cache = library_cache.join(format!("{app_id}.json"));
        if !fs::metadata(&app_cache)
            .map(|metadata| metadata.is_file() && metadata.len() <= 8 * 1024 * 1024)
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(contents) = fs::read_to_string(app_cache) else {
            continue;
        };
        if let Some(achievements) = parse_steam_librarycache_achievements(&contents) {
            merge_steam_game_stats(
                stats.entry((*app_id).to_string()).or_default(),
                SteamGameStats {
                    achievements: Some(achievements),
                    ..SteamGameStats::default()
                },
            );
        }
    }
}

fn parse_steam_achievement_progress_cache(
    contents: &str,
) -> BTreeMap<String, SteamAchievementStats> {
    let mut stats = BTreeMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(contents) else {
        return stats;
    };
    let Some(entries) = value.get("mapCache").and_then(|value| value.as_array()) else {
        return stats;
    };

    for entry in entries {
        let Some(pair) = entry.as_array() else {
            continue;
        };
        let [app_id_value, stats_value] = pair.as_slice() else {
            continue;
        };
        let Some(app_id) = app_id_value.as_u64().map(|id| id.to_string()) else {
            continue;
        };
        if let Some(achievements) = achievement_stats_from_json(stats_value) {
            stats.insert(app_id, achievements);
        }
    }

    stats
}

fn parse_steam_librarycache_achievements(contents: &str) -> Option<SteamAchievementStats> {
    let value = serde_json::from_str::<serde_json::Value>(contents).ok()?;
    let entries = value.as_array()?;
    for entry in entries {
        let pair = entry.as_array()?;
        let [key, payload] = pair.as_slice() else {
            continue;
        };
        if key.as_str() != Some("achievements") {
            continue;
        }
        if let Some(stats) = payload.get("data").and_then(achievement_stats_from_json) {
            return Some(stats);
        }
    }
    None
}

fn achievement_stats_from_json(value: &serde_json::Value) -> Option<SteamAchievementStats> {
    let unlocked = value
        .get("unlocked")
        .or_else(|| value.get("nAchieved"))?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())?;
    let total = value
        .get("total")
        .or_else(|| value.get("nTotal"))?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())?;
    if total == 0 || unlocked > total {
        return None;
    }
    Some(SteamAchievementStats { unlocked, total })
}

fn steam_library_dirs(steam_root: &FsPath) -> Vec<PathBuf> {
    let mut libraries = vec![steam_root.to_path_buf()];
    let libraryfolders = steam_root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(contents) = fs::read_to_string(libraryfolders) {
        libraries.extend(parse_steam_library_folders(&contents));
    }
    libraries.retain(|path| path.join("steamapps").is_dir());
    libraries.sort();
    libraries.dedup();
    libraries
}

fn parse_steam_library_folders(contents: &str) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
            continue;
        }
        if line == "}" {
            stack.pop();
            continue;
        }

        let tokens = quoted_tokens(line);
        match tokens.as_slice() {
            [key] => pending_block = Some(key.to_string()),
            [key, value] => {
                pending_block = None;
                if key == "path"
                    || key.chars().all(|ch| ch.is_ascii_digit()) && looks_like_path(value)
                {
                    folders.push(PathBuf::from(value));
                }
            }
            _ => {}
        }
    }

    folders
}

fn looks_like_path(value: &str) -> bool {
    value.contains(":\\") || value.starts_with('/') || value.starts_with("\\\\")
}

fn collect_steam_app_manifests(libraries: &[PathBuf]) -> Vec<SteamAppManifest> {
    let mut manifests = Vec::new();
    for library in libraries.iter().take(16) {
        let steamapps = library.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten().take(2048) {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.starts_with("appmanifest_") || !file_name.ends_with(".acf") {
                continue;
            }
            if !fs::metadata(&path)
                .map(|metadata| metadata.is_file() && metadata.len() <= 256 * 1024)
                .unwrap_or(false)
            {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(manifest) = parse_steam_app_manifest(library, &contents) {
                manifests.push(manifest);
            }
        }
    }
    manifests
}

fn parse_steam_app_manifest(library: &FsPath, contents: &str) -> Option<SteamAppManifest> {
    let mut app_id = None;
    let mut name = None;
    let mut install_dir = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let tokens = quoted_tokens(line);
        if let [key, value] = tokens.as_slice() {
            match key.as_str() {
                "appid" => app_id = Some(value.to_string()),
                "name" => name = Some(value.to_string()),
                "installdir" => install_dir = Some(value.to_string()),
                _ => {}
            }
        }
    }

    let app_id = app_id?;
    let name = name.unwrap_or_else(|| format!("Steam app {app_id}"));
    let install_dir = install_dir?;
    let install_path = library.join("steamapps").join("common").join(&install_dir);
    Some(SteamAppManifest {
        app_id,
        name,
        install_dir,
        install_path,
    })
}

fn steam_manifest_matches_game(manifest: &SteamAppManifest, game: &GameModule) -> bool {
    game.steam_app_ids
        .iter()
        .any(|app_id| manifest.app_id == *app_id)
        || manifest.name.eq_ignore_ascii_case(game.display_name)
        || game
            .steam_install_dirs
            .iter()
            .any(|dir| manifest.install_dir.eq_ignore_ascii_case(dir))
}

fn find_steam_common_install_dir(libraries: &[PathBuf], game: &GameModule) -> Option<PathBuf> {
    for library in libraries {
        for install_dir in game.steam_install_dirs {
            let candidate = library.join("steamapps").join("common").join(install_dir);
            if candidate.is_dir() || game_executable_exists(&candidate, game) {
                return Some(candidate);
            }
        }
    }
    None
}

fn discover_steam_artwork_paths(steam_root: &FsPath, app_id: &str) -> BTreeMap<String, PathBuf> {
    let mut paths = BTreeMap::new();
    for kind in ["icon", "banner", "hero", "capsule"] {
        if let Some(path) = steam_artwork_candidates(steam_root, app_id, kind)
            .into_iter()
            .find(|path| steam_artwork_file_usable(path))
        {
            paths.insert(kind.to_string(), path);
        }
    }
    paths
}

fn steam_artwork_candidates(steam_root: &FsPath, app_id: &str, kind: &str) -> Vec<PathBuf> {
    let cache = steam_root.join("appcache").join("librarycache");
    let app_cache = cache.join(app_id);
    let mut candidates = Vec::new();

    match kind {
        "icon" => {
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &["logo.png", "icon.jpg", "icon.png", "icon.ico"],
                true,
            ));
            candidates.extend([
                app_cache.join("icon.jpg"),
                app_cache.join("icon.png"),
                app_cache.join("icon.ico"),
                app_cache.join("logo.png"),
                cache.join(format!("{app_id}_icon.jpg")),
                cache.join(format!("{app_id}_icon.png")),
                steam_root
                    .join("steam")
                    .join("games")
                    .join(format!("{app_id}_icon.ico")),
            ]);
        }
        "banner" => {
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_header.jpg",
                    "library_header.png",
                    "header.jpg",
                    "header.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("header.jpg"),
                app_cache.join("header.png"),
                app_cache.join("library_header.jpg"),
                cache.join(format!("{app_id}_header.jpg")),
                cache.join(format!("{app_id}_header.png")),
            ]);
        }
        "hero" => {
            candidates.extend(custom_grid_candidates(steam_root, app_id, "hero"));
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_hero.jpg",
                    "library_hero.png",
                    "hero.jpg",
                    "hero.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("library_hero.jpg"),
                app_cache.join("library_hero.png"),
                app_cache.join("hero.jpg"),
                app_cache.join("hero.png"),
                cache.join(format!("{app_id}_library_hero.jpg")),
                cache.join(format!("{app_id}_library_hero.png")),
            ]);
        }
        "capsule" => {
            candidates.extend(custom_grid_candidates(steam_root, app_id, "capsule"));
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_capsule.jpg",
                    "library_capsule.png",
                    "library_600x900.jpg",
                    "library_600x900.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("library_600x900.jpg"),
                app_cache.join("library_600x900.png"),
                cache.join(format!("{app_id}_library_600x900.jpg")),
                cache.join(format!("{app_id}_library_600x900.png")),
            ]);
        }
        _ => {}
    }

    candidates
}

fn steam_nested_artwork_candidates(
    app_cache: &FsPath,
    file_names: &[&str],
    include_root_images: bool,
) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(app_cache) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    let mut root_images = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.push(path);
        } else if include_root_images && file_type.is_file() && steam_artwork_extension(&path) {
            root_images.push(path);
        }
    }

    dirs.sort();
    root_images.sort();

    let mut candidates = Vec::new();
    for dir in dirs {
        for file_name in file_names {
            candidates.push(dir.join(file_name));
        }
    }
    candidates.extend(root_images);
    candidates
}

fn steam_artwork_extension(path: &FsPath) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "ico"
    )
}

fn custom_grid_candidates(steam_root: &FsPath, app_id: &str, kind: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for user_dir in numeric_child_dirs(&steam_root.join("userdata"), 8) {
        let grid = user_dir.join("config").join("grid");
        match kind {
            "hero" => candidates.extend([
                grid.join(format!("{app_id}_hero.jpg")),
                grid.join(format!("{app_id}_hero.png")),
            ]),
            "capsule" => candidates.extend([
                grid.join(format!("{app_id}p.jpg")),
                grid.join(format!("{app_id}p.png")),
                grid.join(format!("{app_id}.jpg")),
                grid.join(format!("{app_id}.png")),
            ]),
            _ => {}
        }
    }
    candidates
}

fn steam_artwork_file_usable(path: &FsPath) -> bool {
    fs::metadata(path)
        .map(|metadata| {
            metadata.is_file() && metadata.len() > 0 && metadata.len() <= 10 * 1024 * 1024
        })
        .unwrap_or(false)
}

fn game_art_url(game_id: &str, kind: &str) -> String {
    format!("/api/games/art/{game_id}/{kind}")
}

fn steam_art_url_by_app(app_id: &str, kind: &str) -> String {
    format!("/api/games/steam-art/{app_id}/{kind}")
}

fn apply_steam_cdn_artwork_fallback(artwork: &mut GameArtwork, app_id: &str) {
    let base = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}");
    if artwork.banner_url.is_none() {
        artwork.banner_url = Some(format!("{base}/header.jpg"));
    }
    if artwork.hero_url.is_none() {
        artwork.hero_url = Some(format!("{base}/library_hero.jpg"));
    }
    if artwork.capsule_url.is_none() {
        artwork.capsule_url = Some(format!("{base}/library_600x900.jpg"));
    }
    if artwork.icon_url.is_none() {
        artwork.icon_url = artwork
            .capsule_url
            .clone()
            .or_else(|| artwork.banner_url.clone())
            .or_else(|| Some(format!("{base}/capsule_231x87.jpg")));
    }
}

fn enrich_game_detection(
    mut detection: GameDetectionResponse,
    catalog: &SteamGameCatalog,
) -> GameDetectionResponse {
    let active_game_id = detection.active_game_id.as_deref();
    let mut supported_games = catalog.supported_games.clone();
    for game in &mut supported_games {
        game.running = active_game_id == Some(game.game_id.as_str());
    }

    detection.selected_game = active_game_id.and_then(|id| {
        supported_games
            .iter()
            .find(|game| game.game_id == id)
            .cloned()
    });
    detection.supported_games = supported_games;
    detection
}

/// Append every registered user game to the detection's `supported_games`
/// list. Built-in modules sort first; user games sort alphabetically after.
fn append_user_games_to_detection(
    detection: &mut GameDetectionResponse,
    user_games: &BTreeMap<String, UserGameConfig>,
    steam_root: Option<&FsPath>,
    steam_stats: &BTreeMap<String, SteamGameStats>,
) {
    if user_games.is_empty() {
        return;
    }

    let active_game_id = detection.active_game_id.clone();
    let mut user_entries: Vec<SupportedGameSummary> = user_games
        .values()
        .map(|game| {
            let stats = steam_stats.get(&game.app_id).cloned().unwrap_or_default();
            let mut summary = user_game_to_supported_summary(game, steam_root, stats);
            summary.running = active_game_id.as_deref() == Some(summary.game_id.as_str());
            summary
        })
        .collect();
    user_entries.sort_by_key(|game| game.name.to_ascii_lowercase());

    detection.supported_games.extend(user_entries);

    if let Some(active_id) = active_game_id.as_deref() {
        if detection
            .selected_game
            .as_ref()
            .is_none_or(|game| game.game_id != active_id)
        {
            detection.selected_game = detection
                .supported_games
                .iter()
                .find(|game| game.game_id == active_id)
                .cloned();
        }
    }
}

const USER_GAME_PROCESS_CANDIDATE_LIMIT: usize = 8;
const USER_GAME_PROCESS_SCAN_LIMIT: usize = 256;

/// Build the synthesized user-game id for a Steam app.
fn user_game_id_for_app_id(app_id: &str) -> String {
    format!("custom-{}", app_id.trim())
}

/// Scan the top level of a Steam game's install path for plausible launcher
/// executables. Recursive scans are intentionally avoided so we don't walk
/// large game directories during a snapshot/library call.
/// Normalise an incoming list of process-name overrides. Trims whitespace,
/// strips any path separators (user might paste a full path), drops empty
/// entries, enforces a .exe suffix, deduplicates case-insensitively, and caps
/// the list at `USER_GAME_PROCESS_CANDIDATE_LIMIT` entries.
fn sanitize_user_game_process_names(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in raw {
        // Strip any directory components — only the file name is meaningful for
        // process matching.
        let name = std::path::Path::new(value.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        if !name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        if out
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        out.push(name);
        if out.len() >= USER_GAME_PROCESS_CANDIDATE_LIMIT {
            break;
        }
    }
    out
}

fn discover_user_game_process_candidates(install_path: &FsPath) -> Vec<String> {
    let Ok(entries) = fs::read_dir(install_path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten().take(USER_GAME_PROCESS_SCAN_LIMIT) {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        if is_excluded_user_game_process(file_name) {
            continue;
        }
        // Confirm it's a real file rather than a directory entry that happens
        // to end in .exe.
        let is_file = entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false);
        if !is_file {
            continue;
        }
        names.push(file_name.to_string());
        if names.len() >= USER_GAME_PROCESS_CANDIDATE_LIMIT {
            break;
        }
    }
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

fn is_excluded_user_game_process(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.starts_with("uninst")
        || lower.starts_with("setup")
        || lower.starts_with("unitycrashhandler")
        || lower.starts_with("ueprereqsetup")
        || lower.contains("crash")
        || lower.starts_with("vc_redist")
        || lower.starts_with("vcredist")
        || lower.starts_with("dotnetfx")
        || lower.starts_with("eossdk")
        || lower.starts_with("eacrash")
        || lower.starts_with("easetup")
        || lower.starts_with("easyanticheat")
        || lower.starts_with("redist")
        || lower.contains("installer")
        || lower.contains("launcher_setup")
}

fn user_game_artwork_for_app(steam_root: &FsPath, app_id: &str) -> GameArtwork {
    let mut artwork = GameArtwork::default();
    // Prefer the local Steam librarycache (what Steam actually shows in-client).
    // Routes back through /api/games/steam-art/:app_id/:kind which streams the
    // file from disk on demand. Many Steam apps lack public CDN capsules, so the
    // local cache is the most reliable source for the user's actual library.
    for kind in discover_steam_artwork_paths(steam_root, app_id).keys() {
        let url = steam_art_url_by_app(app_id, kind);
        match kind.as_str() {
            "icon" => artwork.icon_url = Some(url),
            "banner" => artwork.banner_url = Some(url),
            "hero" => artwork.hero_url = Some(url),
            "capsule" => artwork.capsule_url = Some(url),
            _ => {}
        }
    }
    // Fill remaining slots from Steam's CDN. apply_steam_cdn_artwork_fallback
    // only writes is_none() fields, so this preserves the local-cache choices.
    apply_steam_cdn_artwork_fallback(&mut artwork, app_id);
    artwork
}

/// Locate the configured Steam root (if any) and read per-app stats. Designed
/// to be run inside `spawn_blocking` so it does not block the async runtime.
fn steam_root_and_stats_for_user_games() -> (Option<PathBuf>, BTreeMap<String, SteamGameStats>) {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return (None, BTreeMap::new());
    };
    let stats = discover_steam_game_stats(&steam_root);
    (Some(steam_root), stats)
}

/// Look up a Steam app manifest by app_id across the entire library set.
/// Returns `None` if Steam isn't installed or the manifest can't be found.
fn locate_steam_manifest(app_id: &str) -> Option<SteamAppManifest> {
    let steam_root = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())?;
    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    manifests
        .into_iter()
        .find(|manifest| manifest.app_id == app_id)
}

/// Returns every Steam library entry that the agent can see on disk, with a
/// flag marking entries that are already represented by a built-in module or
/// a previously-added user game.
fn discover_steam_library_entries(
    user_games: &BTreeMap<String, UserGameConfig>,
) -> Vec<SteamLibraryEntry> {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return Vec::new();
    };

    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    let steam_stats = discover_steam_game_stats(&steam_root);
    let built_in_app_ids: std::collections::BTreeSet<&str> = built_in_game_modules()
        .iter()
        .flat_map(|game| game.steam_app_ids.iter().copied())
        .collect();

    let mut entries = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let suggested_game_id = user_game_id_for_app_id(&manifest.app_id);
        let already_in_catalog = built_in_app_ids.contains(manifest.app_id.as_str())
            || user_games.contains_key(&suggested_game_id);
        let artwork = user_game_artwork_for_app(&steam_root, &manifest.app_id);
        let stats = steam_stats
            .get(manifest.app_id.as_str())
            .cloned()
            .unwrap_or_default();
        let process_candidates = discover_user_game_process_candidates(&manifest.install_path);
        entries.push(SteamLibraryEntry {
            app_id: manifest.app_id.clone(),
            name: manifest.name.clone(),
            install_dir: manifest.install_dir.clone(),
            install_path: manifest.install_path.display().to_string(),
            artwork,
            stats,
            already_in_catalog,
            suggested_game_id,
            process_candidates,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    entries
}

/// Build a `SupportedGameSummary` entry for a user-registered game, suitable
/// for appending to the snapshot's `supported_games` list.
fn user_game_to_supported_summary(
    game: &UserGameConfig,
    steam_root: Option<&FsPath>,
    stats: SteamGameStats,
) -> SupportedGameSummary {
    let install_path = PathBuf::from(&game.install_path);
    let installed = !game.install_path.is_empty() && install_path.is_dir();
    let artwork = match steam_root {
        Some(root) => user_game_artwork_for_app(root, &game.app_id),
        None => {
            let mut artwork = GameArtwork::default();
            apply_steam_cdn_artwork_fallback(&mut artwork, &game.app_id);
            artwork
        }
    };
    SupportedGameSummary {
        game_id: game.game_id.clone(),
        name: game.name.clone(),
        app_id: (!game.app_id.is_empty()).then(|| game.app_id.clone()),
        install_path: (!game.install_path.is_empty()).then(|| game.install_path.clone()),
        installed,
        running: false,
        support_level: "custom".to_string(),
        artwork,
        stats,
    }
}

fn supported_game_install_path(catalog: &SteamGameCatalog, game_id: &str) -> Option<PathBuf> {
    catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == game_id && game.installed)
        .and_then(|game| game.install_path.as_deref())
        .map(PathBuf::from)
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

#[cfg(target_os = "windows")]
fn windows_process_names() -> io::Result<Vec<String>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut names = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let process_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if !process_name.is_empty() {
                    names.push(process_name);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        Ok(names)
    }
}

#[cfg(target_os = "windows")]
fn windows_process_running(target: &str) -> bool {
    windows_process_names()
        .map(|names| {
            names
                .iter()
                .any(|process_name| process_name.eq_ignore_ascii_case(target))
        })
        .unwrap_or(false)
}

fn steam_root_candidates() -> Vec<PathBuf> {
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

fn numeric_child_dirs(root: &FsPath, max_dirs: usize) -> Vec<PathBuf> {
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

fn parse_steam_input_layout(
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
struct SteamInputWriteFailure {
    status: StatusCode,
    message: String,
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

fn write_steam_input_binding(
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

fn validated_steam_input_layout_path(
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

fn replace_steam_binding_value(
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

fn mark_dscc_steam_profile_metadata(contents: &str, profile_name: Option<&str>) -> String {
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

fn quoted_tokens(line: &str) -> Vec<String> {
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

fn title_case_words(value: &str) -> String {
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

fn steam_app_id_from_path(path: &FsPath) -> Option<String> {
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

fn sanitized_steam_path(steam_root: &FsPath, path: &FsPath) -> Option<String> {
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

impl Default for AgentState {
    fn default() -> Self {
        Self::mock()
    }
}

impl AgentState {
    pub fn mock() -> Self {
        let mut manager = mock_device_manager();
        Self::from_device_manager_with_backend(&mut manager, DeviceBackendSummary::mock())
            .unwrap_or_else(|_| {
                let mut controllers = ControllerRegistry::default();
                controllers.apply(ControllerDiscoveryEvent::Faulted {
                    id: None,
                    message: "mock device manager failed during startup".to_string(),
                });
                Self::from_controller_registry_with_backend(
                    controllers,
                    DeviceBackendSummary::mock(),
                )
            })
    }

    pub fn from_controller_events<I>(events: I) -> Self
    where
        I: IntoIterator<Item = ControllerDiscoveryEvent>,
    {
        Self::from_controller_events_with_backend(events, DeviceBackendSummary::mock())
    }

    fn from_controller_events_with_backend<I>(
        events: I,
        device_backend: DeviceBackendSummary,
    ) -> Self
    where
        I: IntoIterator<Item = ControllerDiscoveryEvent>,
    {
        Self::from_controller_events_with_backend_and_storage(events, device_backend, None)
    }

    fn from_controller_events_with_backend_and_storage<I>(
        events: I,
        device_backend: DeviceBackendSummary,
        storage: Option<PersistenceStore>,
    ) -> Self
    where
        I: IntoIterator<Item = ControllerDiscoveryEvent>,
    {
        let mut controllers = ControllerRegistry::default();
        for event in events {
            controllers.apply(event);
        }

        Self::from_controller_registry_with_backend_and_storage(
            controllers,
            device_backend,
            storage,
        )
    }

    pub fn from_device_manager<T>(
        manager: &mut DeviceManager<T>,
    ) -> Result<Self, dscc_device::DeviceError>
    where
        T: DeviceTransport,
    {
        Self::from_device_manager_with_backend(
            manager,
            DeviceBackendSummary::hid(OutputMode::HardwareOutput),
        )
    }

    fn from_device_manager_with_backend<T>(
        manager: &mut DeviceManager<T>,
        device_backend: DeviceBackendSummary,
    ) -> Result<Self, dscc_device::DeviceError>
    where
        T: DeviceTransport,
    {
        Self::from_device_manager_with_backend_and_storage(manager, device_backend, None)
    }

    fn from_device_manager_with_backend_and_storage<T>(
        manager: &mut DeviceManager<T>,
        device_backend: DeviceBackendSummary,
        storage: Option<PersistenceStore>,
    ) -> Result<Self, dscc_device::DeviceError>
    where
        T: DeviceTransport,
    {
        let events = controller_events_from_device_manager(manager)?;
        Ok(Self::from_controller_events_with_backend_and_storage(
            events,
            device_backend,
            storage,
        ))
    }

    fn from_controller_registry_with_backend(
        controllers: ControllerRegistry,
        device_backend: DeviceBackendSummary,
    ) -> Self {
        Self::from_controller_registry_with_backend_and_storage(controllers, device_backend, None)
    }

    fn from_controller_registry_with_backend_and_storage(
        controllers: ControllerRegistry,
        device_backend: DeviceBackendSummary,
        storage: Option<PersistenceStore>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        let persisted = storage
            .as_ref()
            .and_then(|store| store.load().ok())
            .unwrap_or_default()
            .normalized();
        let active_profile_id = persisted
            .active_profile_id
            .clone()
            .filter(|id| profile_exists_in_defaults_or_persisted(id, &persisted.profiles))
            .map(|id| {
                if id == FORZA_HORIZON_PROFILE_ID {
                    DEFAULT_PROFILE_ID.to_string()
                } else {
                    id
                }
            })
            .or_else(|| Some(DEFAULT_PROFILE_ID.to_string()));

        Self {
            started_at: Instant::now(),
            bind_addr: default_agent_bind_addr(),
            event_tx,
            output_manager: None,
            output_runtime: Arc::new(Mutex::new(HardwareOutputRuntime::default())),
            discovery_cache: Arc::new(DiscoveryCache::default()),
            realtime_runtime: Arc::new(Mutex::new(RealtimeRuntime::default())),
            effect_runtime: Arc::new(Mutex::new(EffectRuntimeCache::default())),
            inner: Arc::new(RwLock::new(AgentStateInner {
                controllers,
                controller_names: persisted.controller_names,
                profiles: profiles_with_active(
                    merge_profiles(persisted.profiles),
                    &active_profile_id,
                ),
                adapters: default_adapters(),
                telemetry: SignalSnapshot::default(),
                logs: vec![LogEntry {
                    level: "info".to_string(),
                    message: "Agent initialized with dscc-device controller registry".to_string(),
                    timestamp: current_timestamp(),
                }],
                device_backend,
                storage,
                controller_configs: persisted.controller_configs,
                profile_configs: persisted.profile_configs,
                profile_overrides: persisted.profile_overrides,
                edge_profiles: persisted.edge_profiles,
                app_settings: persisted.app_settings,
                active_profile_id,
                active_adapter_id: None,
                auto_loaded_profile_id: None,
                adapter_runtimes: default_adapter_runtimes(),
                forza_effect_runtime: ForzaEffectRuntime::default(),
                effect_revision: 0,
                user_games: persisted.user_games,
            })),
        }
    }

    pub async fn apply_controller_event(&self, event: ControllerDiscoveryEvent) {
        let realtime = {
            let mut inner = self.inner.write().await;
            if inner.controllers.is_redundant_attach(&event) {
                return;
            }
            inner.controllers.apply(event.clone());
            if let Some(profile_id) = inner.auto_loaded_profile_id.clone() {
                if matches!(event, ControllerDiscoveryEvent::Attached(_)) {
                    apply_profile_selection_config(&mut inner, &profile_id);
                    inner.effect_revision = inner.effect_revision.saturating_add(1);
                }
            }
            inner.controllers.realtime_message_for(&event)
        };
        let _ = self.event_tx.send(realtime);
    }

    fn with_output_manager(
        mut self,
        manager: Arc<ControllerOutputManager<HidApiTransport>>,
    ) -> Self {
        self.output_manager = Some(manager);
        self
    }

    fn with_bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.bind_addr = bind_addr;
        self
    }

    fn hardware_output_enabled(&self) -> bool {
        self.output_manager
            .as_ref()
            .is_some_and(|manager| manager.hardware_writes_enabled())
    }

    fn app_settings_response(&self, settings: &AppSettings) -> AppSettingsResponse {
        let mut settings = settings.clone();
        if settings.listen_on_all_interfaces && !lan_api_enabled() {
            settings.listen_on_all_interfaces = false;
        }
        let desired = desired_agent_bind_addr(&settings, self.bind_addr.port());
        AppSettingsResponse {
            settings,
            effective_bind_address: self.bind_addr.to_string(),
            desired_bind_address: desired.to_string(),
            restart_required: desired != self.bind_addr,
        }
    }

    fn lock_output_runtime(&self) -> MutexGuard<'_, HardwareOutputRuntime> {
        match self.output_runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn begin_manual_output_override(&self, duration: Duration) -> u64 {
        let mut runtime = self.lock_output_runtime();
        runtime.manual_override_generation = runtime.manual_override_generation.wrapping_add(1);
        runtime.manual_override_until = Some(Instant::now() + duration);
        runtime.manual_override_generation
    }

    fn clear_manual_output_override(&self) {
        let mut runtime = self.lock_output_runtime();
        runtime.manual_override_generation = runtime.manual_override_generation.wrapping_add(1);
        runtime.manual_override_until = None;
    }

    fn clear_manual_output_override_if_generation(&self, generation: u64) {
        let mut runtime = self.lock_output_runtime();
        if runtime.manual_override_generation == generation {
            runtime.manual_override_generation = runtime.manual_override_generation.wrapping_add(1);
            runtime.manual_override_until = None;
        }
    }

    fn manual_output_override_active(&self) -> bool {
        let mut runtime = self.lock_output_runtime();
        if let Some(until) = runtime.manual_override_until {
            if Instant::now() < until {
                return true;
            }
            runtime.manual_override_until = None;
        }
        false
    }

    fn manual_output_override_active_for(&self, generation: u64) -> bool {
        let mut runtime = self.lock_output_runtime();
        if runtime.manual_override_generation != generation {
            return false;
        }
        if let Some(until) = runtime.manual_override_until {
            if Instant::now() < until {
                return true;
            }
            runtime.manual_override_until = None;
        }
        false
    }

    fn manual_output_override_generation_matches(&self, generation: u64) -> bool {
        self.lock_output_runtime().manual_override_generation == generation
    }

    fn output_frame_write_due(
        &self,
        controller_id: &str,
        frame: &ControllerOutputFrame,
        now: Instant,
    ) -> bool {
        let runtime = self.lock_output_runtime();
        match runtime.last_output_frames.get(controller_id) {
            Some(last) => {
                last.frame != *frame
                    || now.duration_since(last.written_at) >= HARDWARE_OUTPUT_KEEPALIVE_INTERVAL
            }
            None => true,
        }
    }

    fn record_output_frame_write(
        &self,
        controller_id: &str,
        frame: &ControllerOutputFrame,
        written_at: Instant,
    ) {
        let mut runtime = self.lock_output_runtime();
        runtime.last_output_frames.insert(
            controller_id.to_string(),
            LastHardwareOutputFrame {
                frame: frame.clone(),
                written_at,
            },
        );
    }

    fn has_non_neutral_output_frames(&self) -> bool {
        let runtime = self.lock_output_runtime();
        let neutral = ControllerOutputFrame::default();
        runtime
            .last_output_frames
            .values()
            .any(|last| last.frame != neutral)
    }

    fn non_neutral_output_controller_ids(&self) -> Vec<String> {
        let runtime = self.lock_output_runtime();
        let neutral = ControllerOutputFrame::default();
        runtime
            .last_output_frames
            .iter()
            .filter(|(_, last)| last.frame != neutral)
            .map(|(controller_id, _)| controller_id.clone())
            .collect()
    }

    fn clear_recorded_output_frames(&self) {
        self.lock_output_runtime().last_output_frames.clear();
    }

    fn release_all_output_sessions(&self) {
        if let Some(manager) = &self.output_manager {
            manager.release_all();
        }
    }

    async fn release_output_session_for_controller(&self, controller_id: &str) {
        if let Some(manager) = &self.output_manager {
            let target = {
                let inner = self.inner.read().await;
                controller_output_target_or_reason(&inner, controller_id).ok()
            };
            if let Some(target) = target {
                manager.release(&target);
            }
        }
        let mut runtime = self.lock_output_runtime();
        runtime.last_output_frames.remove(controller_id);
    }

    async fn neutralize_active_output_and_release(&self, reason: &str) {
        let controller_ids = self.non_neutral_output_controller_ids();
        for controller_id in controller_ids {
            if let Err(error) = self
                .write_output_frame_to_controller(&controller_id, &ControllerOutputFrame::default())
                .await
            {
                self.note_hardware_output_error(format!(
                    "Hardware trigger output could not neutralize controller {controller_id} after {reason}: {error}"
                ))
                .await;
            }
        }
        self.release_all_output_sessions();
        self.clear_recorded_output_frames();
    }

    async fn log_warn(&self, message: String) {
        let mut inner = self.inner.write().await;
        inner.logs.push(LogEntry {
            level: "warn".to_string(),
            message,
            timestamp: current_timestamp(),
        });
    }

    async fn note_hardware_output_error(&self, message: String) {
        let should_log = {
            let mut runtime = self.lock_output_runtime();
            let now = Instant::now();
            let stale_error_window = match runtime.last_error_at {
                Some(last) => now.duration_since(last) >= Duration::from_secs(2),
                None => true,
            };
            let should_log =
                stale_error_window || runtime.last_error.as_deref() != Some(message.as_str());
            if should_log {
                runtime.last_error = Some(message.clone());
                runtime.last_error_at = Some(now);
            }
            should_log
        };

        if should_log {
            let mut inner = self.inner.write().await;
            inner.logs.push(LogEntry {
                level: "warn".to_string(),
                message,
                timestamp: current_timestamp(),
            });
        }
    }

    async fn write_output_frame_to_controller(
        &self,
        controller_id: &str,
        frame: &ControllerOutputFrame,
    ) -> Result<ControllerOutputWrite, String> {
        let manager = self
            .output_manager
            .clone()
            .ok_or_else(|| "HID output manager is unavailable".to_string())?;
        let target = {
            let inner = self.inner.read().await;
            controller_output_target_or_reason(&inner, controller_id)?
        };
        let frame_for_write = frame.clone();
        let write =
            tokio::task::spawn_blocking(move || manager.write_frame(&target, &frame_for_write))
                .await
                .map_err(|error| format!("HID output task failed: {error}"))?
                .map_err(|error| error.to_string())?;
        self.record_output_frame_write(controller_id, frame, Instant::now());
        Ok(write)
    }

    async fn read_input_state_for_controller(
        &self,
        controller_id: &str,
    ) -> Result<Option<ControllerInputState>, String> {
        let manager = self
            .output_manager
            .clone()
            .ok_or_else(|| "HID output manager is unavailable".to_string())?;
        let target = {
            let inner = self.inner.read().await;
            inner
                .controllers
                .detail(controller_id)
                .ok_or_else(|| format!("Controller {controller_id} was not found"))?;
            controller_output_target_or_reason(&inner, controller_id)?
        };

        tokio::task::spawn_blocking(move || manager.read_input_state(&target))
            .await
            .map_err(|error| format!("HID input task failed: {error}"))?
            .map_err(|error| error.to_string())
    }

    async fn write_current_output_frame_if_due(
        &self,
        game_detection: Option<&GameDetectionResponse>,
    ) -> Result<Option<ControllerOutputWrite>, String> {
        let candidate = {
            let inner = self.inner.read().await;
            self.output_frame_for_current_resolution_cached(
                &inner,
                game_detection,
                EffectEnginePurpose::Hardware,
            )
        };
        let Some((controller_id, frame)) = candidate else {
            return Ok(None);
        };
        if !self.output_frame_write_due(&controller_id, &frame, Instant::now()) {
            return Ok(None);
        }
        self.write_output_frame_to_controller(&controller_id, &frame)
            .await
            .map(Some)
    }

    fn evaluate_runtime_profile(
        &self,
        inner: &AgentStateInner,
        controller_id: Option<&str>,
        profile_id: &str,
        profile: &Profile,
        snapshot: &SignalSnapshot,
        purpose: EffectEnginePurpose,
    ) -> ControllerOutputFrame {
        let key = EffectEngineKey {
            purpose,
            controller_id: controller_id.unwrap_or("none").to_string(),
            profile_id: profile_id.to_string(),
            revision: inner.effect_revision,
        };
        let mut runtime = match self.effect_runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        runtime.evaluate(key, profile, snapshot)
    }

    fn current_effect_response_cached(
        &self,
        inner: &AgentStateInner,
        game_detection: Option<&GameDetectionResponse>,
        hardware_output_enabled: bool,
        purpose: EffectEnginePurpose,
    ) -> CurrentEffectResponse {
        let resolution = profile_resolution(inner, game_detection);
        let config = controller_config_for_resolution(inner, &resolution);
        let (snapshot, telemetry_live) = current_effect_snapshot(inner, game_detection);
        let profile_id = resolution
            .selected_profile_id
            .clone()
            .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
        let profile_name =
            profile_name_by_id(inner, &profile_id).unwrap_or_else(|| profile_id.clone());
        let profile = runtime_profile_for(&profile_id, &profile_name, config.as_ref(), &snapshot);
        let mut output = self.evaluate_runtime_profile(
            inner,
            resolution.controller_id.as_deref(),
            &profile_id,
            &profile,
            &snapshot,
            purpose,
        );
        apply_runtime_output_enhancements(
            &profile_id,
            config.as_ref(),
            &snapshot,
            telemetry_live,
            &mut output,
        );
        apply_detection_lightbar_preview(game_detection, telemetry_live, &mut output);
        current_effect_response_from_parts(
            resolution,
            profile,
            config.as_ref(),
            snapshot,
            telemetry_live,
            output,
            game_detection,
            hardware_output_enabled,
        )
    }

    fn output_frame_for_current_resolution_cached(
        &self,
        inner: &AgentStateInner,
        game_detection: Option<&GameDetectionResponse>,
        purpose: EffectEnginePurpose,
    ) -> Option<(String, ControllerOutputFrame)> {
        let resolution = profile_resolution(inner, game_detection);
        let controller_id = resolution.controller_id.clone()?;
        if purpose == EffectEnginePurpose::Hardware
            && !hardware_output_runtime_allowed_for_resolution(inner, game_detection, &resolution)
        {
            if hardware_output_detection_lightbar_allowed_for_resolution(
                inner,
                game_detection,
                &resolution,
            ) {
                let detection = game_detection?;
                let output = ControllerOutputFrame {
                    lightbar: detection_lightbar_output(detection),
                    ..ControllerOutputFrame::default()
                };
                return Some((controller_id, output));
            }
            if hardware_output_global_lightbar_allowed_for_resolution(game_detection, &resolution) {
                if let Some(output) = global_lightbar_output(inner, &resolution) {
                    return Some((controller_id, output));
                }
            }
            return None;
        }
        let config = controller_config_for_resolution(inner, &resolution);
        let (snapshot, telemetry_live) = current_effect_snapshot(inner, game_detection);
        let profile_id = resolution
            .selected_profile_id
            .clone()
            .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
        let profile_name =
            profile_name_by_id(inner, &profile_id).unwrap_or_else(|| profile_id.clone());
        let profile = runtime_profile_for(&profile_id, &profile_name, config.as_ref(), &snapshot);
        let mut output = self.evaluate_runtime_profile(
            inner,
            Some(&controller_id),
            &profile_id,
            &profile,
            &snapshot,
            purpose,
        );
        apply_runtime_output_enhancements(
            &profile_id,
            config.as_ref(),
            &snapshot,
            telemetry_live,
            &mut output,
        );
        apply_detection_lightbar_preview(game_detection, telemetry_live, &mut output);
        Some((controller_id, output))
    }

    async fn apply_adapter_packet(
        &self,
        adapter_id: &'static str,
        packet_len: usize,
        sequence: u64,
        updates: Vec<SignalUpdate>,
    ) {
        let realtime = {
            let mut inner = self.inner.write().await;
            let mut updates = updates;
            let packet_rate_hz = inner
                .adapter_runtime_mut(adapter_id)
                .mark_packet(packet_len, sequence);
            if racing_shift_adapter(adapter_id) {
                let current_gear = update_number(&updates, "drivetrain.gear");
                let telemetry_on = update_text(&updates, "game.state") == Some("driving");
                let effect_toggles = racing_effect_toggles(&inner);
                let suspension_travel = update_number(&updates, "suspension.travel.max");
                let acceleration_magnitude =
                    update_number(&updates, "vehicle.acceleration.magnitude");
                let speed_kmh = update_number(&updates, "vehicle.speed_kmh");
                let now = Instant::now();
                if let Some(shift_event) = inner.forza_effect_runtime.detect_shift_event(
                    current_gear,
                    telemetry_on,
                    effect_toggles.shift_thump,
                    now,
                ) {
                    updates.push(
                        SignalUpdate::new(
                            SignalName::new("drivetrain.shift_event")
                                .expect("signal name is valid"),
                            shift_event,
                        )
                        .with_sequence(sequence),
                    )
                }
                let suspension_impact = inner.forza_effect_runtime.detect_suspension_impact(
                    suspension_travel,
                    acceleration_magnitude,
                    speed_kmh,
                    telemetry_on,
                    effect_toggles.suspension_impact,
                    now,
                );
                updates.push(sequenced_signal_update(
                    "suspension.impact_pulse",
                    suspension_impact,
                    sequence,
                ));
            }
            updates.push(
                SignalUpdate::new(
                    SignalName::new("source.packet_rate_hz").expect("signal name is valid"),
                    f64::from(packet_rate_hz),
                )
                .with_sequence(sequence),
            );
            if inner.telemetry.text("source.id") == Some(adapter_id) {
                inner.telemetry.apply_updates(updates);
            } else {
                inner.telemetry = SignalSnapshot::from_updates(updates);
            }
            if inner.active_adapter_id.as_deref() != Some(adapter_id) {
                inner.active_adapter_id = Some(adapter_id.to_string());
            }
            let was_running = inner
                .adapters
                .iter()
                .any(|adapter| adapter.id == adapter_id && adapter.state == "connected");
            set_adapter_running(&mut inner.adapters, adapter_id, true);
            if !was_running {
                let display_name = inner
                    .adapters
                    .iter()
                    .find(|adapter| adapter.id == adapter_id)
                    .map(|adapter| adapter.name.clone())
                    .unwrap_or_else(|| {
                        inner
                            .adapter_runtime(adapter_id)
                            .map(|runtime| runtime.display_name.clone())
                            .unwrap_or_else(|| adapter_id.to_string())
                    });
                inner.logs.push(LogEntry {
                    level: "info".to_string(),
                    message: format!("{display_name} stream connected ({packet_len} byte packets)"),
                    timestamp: current_timestamp(),
                });
            }
            self.should_emit_telemetry_invalidation()
                .then(|| RealtimeMessage {
                    kind: "snapshot_invalidated".to_string(),
                    controller: inner.controllers.summaries().into_iter().next(),
                    message: Some(adapter_id.to_string()),
                })
        };
        if let Some(realtime) = realtime {
            let _ = self.event_tx.send(realtime);
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<RealtimeMessage> {
        self.event_tx.subscribe()
    }

    async fn cached_game_detection_with_ttl(&self, ttl: Duration) -> GameDetectionResponse {
        let mut cache = self.discovery_cache.game_detection.lock().await;
        let now = Instant::now();
        if let Some(value) = cache.fresh(ttl, now) {
            return value;
        }

        let user_games = {
            let inner = self.inner.read().await;
            inner.user_games.clone()
        };
        let detection = detect_running_game(&user_games).await;
        let catalog = self.cached_steam_game_catalog().await;
        let mut detection = enrich_game_detection(detection, &catalog);
        let (steam_root, steam_stats) =
            tokio::task::spawn_blocking(steam_root_and_stats_for_user_games)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "Steam root/stats lookup task failed");
                    (None, BTreeMap::new())
                });
        append_user_games_to_detection(
            &mut detection,
            &user_games,
            steam_root.as_deref(),
            &steam_stats,
        );
        {
            let mut inner = self.inner.write().await;
            sync_auto_loaded_profile_for_detection(&mut inner, &detection);
        }
        cache.store(detection, Instant::now())
    }

    async fn cached_game_detection(&self) -> GameDetectionResponse {
        self.cached_game_detection_with_ttl(GAME_DETECTION_CACHE_TTL)
            .await
    }

    async fn cached_hardware_game_detection(&self) -> GameDetectionResponse {
        self.cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
            .await
    }

    async fn cached_steam_game_catalog(&self) -> SteamGameCatalog {
        let now = Instant::now();
        {
            let cache = self.discovery_cache.steam_game_catalog.lock().await;
            if let Some(value) = cache.fresh(STEAM_GAME_CATALOG_CACHE_TTL, now) {
                return value;
            }
        }

        let catalog = tokio::task::spawn_blocking(discover_steam_game_catalog)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Steam game catalog discovery task failed");
                unsupported_steam_game_catalog()
            });
        let mut cache = self.discovery_cache.steam_game_catalog.lock().await;
        cache.store(catalog, Instant::now())
    }

    async fn cached_steam_input_status(&self) -> SteamInputStatus {
        let now = Instant::now();
        {
            let cache = self.discovery_cache.steam_input.lock().await;
            if let Some(value) = cache.fresh(STEAM_INPUT_CACHE_TTL, now) {
                return value;
            }
        }

        let status = discover_steam_input_status_async().await;
        let mut cache = self.discovery_cache.steam_input.lock().await;
        cache.store(status, Instant::now())
    }

    async fn cached_steam_input_status_or_refresh(&self) -> SteamInputStatus {
        let now = Instant::now();
        let cached = {
            let cache = self.discovery_cache.steam_input.lock().await;
            if let Some(value) = cache.fresh(STEAM_INPUT_CACHE_TTL, now) {
                return value;
            }
            cache.value.clone()
        };

        self.spawn_steam_input_refresh();
        cached.unwrap_or_else(pending_steam_input_status)
    }

    fn spawn_steam_input_refresh(&self) {
        if self
            .discovery_cache
            .steam_input_refreshing
            .swap(true, Ordering::AcqRel)
        {
            return;
        }

        let state = self.clone();
        tokio::spawn(async move {
            let status = discover_steam_input_status_async().await;
            {
                let mut cache = state.discovery_cache.steam_input.lock().await;
                cache.store(status, Instant::now());
            }
            state
                .discovery_cache
                .steam_input_refreshing
                .store(false, Ordering::Release);
            let _ = state.event_tx.send(RealtimeMessage {
                kind: "snapshot_invalidated".to_string(),
                controller: None,
                message: Some("steam-input-updated".to_string()),
            });
        });
    }

    async fn update_check(&self) -> UpdateCheckResponse {
        let now = Instant::now();
        {
            let cache = self.discovery_cache.update_check.lock().await;
            if let Some(mut value) = cache.fresh(UPDATE_CHECK_CACHE_TTL, now) {
                value.cached = true;
                return value;
            }
        }

        match fetch_latest_release_update_check().await {
            Ok(response) => {
                let mut cache = self.discovery_cache.update_check.lock().await;
                cache.store(response, Instant::now())
            }
            Err(error) => {
                let mut response = unavailable_update_check(error.to_string());
                let cache = self.discovery_cache.update_check.lock().await;
                if let Some(cached) = cache.value.as_ref() {
                    response = cached.clone();
                    response.state = "stale".to_string();
                    response.error = Some(error.to_string());
                    response.cached = true;
                }
                response
            }
        }
    }

    fn should_emit_telemetry_invalidation(&self) -> bool {
        if self.event_tx.receiver_count() == 0 {
            return false;
        }

        let mut runtime = match self.realtime_runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        if runtime
            .last_telemetry_event_at
            .is_some_and(|last| now.duration_since(last) < TELEMETRY_WS_INVALIDATION_INTERVAL)
        {
            return false;
        }
        runtime.last_telemetry_event_at = Some(now);
        true
    }

    pub async fn status(&self) -> StatusResponse {
        self.status_with_detection(None).await
    }

    async fn status_with_detection(
        &self,
        game_detection: Option<&GameDetectionResponse>,
    ) -> StatusResponse {
        let inner = self.inner.read().await;
        self.status_from_inner(&inner, game_detection)
    }

    fn status_from_inner(
        &self,
        inner: &AgentStateInner,
        game_detection: Option<&GameDetectionResponse>,
    ) -> StatusResponse {
        let resolution = profile_resolution(inner, game_detection);
        let supported_foreground_game_detected =
            game_detection.is_some_and(|detection| detection.profile_id.is_some());
        StatusResponse {
            product: "DualSense Command Center Agent".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            healthy: true,
            bind_address: self.bind_addr.to_string(),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            active_profile_id: if supported_foreground_game_detected {
                resolution
                    .selected_profile_id
                    .or_else(|| inner.active_profile_id.clone())
            } else {
                inner.active_profile_id.clone()
            },
            active_adapter_id: if supported_foreground_game_detected {
                resolution
                    .active_adapter_id
                    .or_else(|| inner.active_adapter_id.clone())
            } else {
                inner.active_adapter_id.clone()
            },
        }
    }

    pub async fn diagnostics(&self) -> DiagnosticsResponse {
        let steam_input = self.cached_steam_input_status_or_refresh().await;
        let game_detection = self.cached_game_detection().await;
        self.diagnostics_with_discovery(&steam_input, &game_detection)
            .await
    }

    async fn diagnostics_with_discovery(
        &self,
        steam_input: &SteamInputStatus,
        game_detection: &GameDetectionResponse,
    ) -> DiagnosticsResponse {
        let inner = self.inner.read().await;
        let hardware_output_enabled = self.hardware_output_enabled();
        self.diagnostics_from_inner(&inner, steam_input, game_detection, hardware_output_enabled)
    }

    fn diagnostics_from_inner(
        &self,
        inner: &AgentStateInner,
        steam_input: &SteamInputStatus,
        game_detection: &GameDetectionResponse,
        hardware_output_enabled: bool,
    ) -> DiagnosticsResponse {
        let mut checks = vec![
            HealthCheck {
                name: "api".to_string(),
                status: "ok".to_string(),
                detail: "Local API is responding".to_string(),
            },
            HealthCheck {
                name: "device-backend".to_string(),
                status: inner.device_backend.status.clone(),
                detail: inner.device_backend.detail.clone(),
            },
            HealthCheck {
                name: "controller-output".to_string(),
                status: if hardware_output_enabled {
                    "ok"
                } else {
                    "disabled"
                }
                .to_string(),
                detail: if hardware_output_enabled {
                    "Guarded DualSense adaptive-trigger output is enabled".to_string()
                } else {
                    "Controller output is encoded and validated but not written to hardware because hardware output is disabled".to_string()
                },
            },
        ];
        if let Some(paths) = app_paths() {
            checks.push(HealthCheck {
                name: "app-paths".to_string(),
                status: "ok".to_string(),
                detail: format!(
                    "config={}, data={}, logs={}",
                    paths.config_dir, paths.data_dir, paths.log_dir
                ),
            });
        } else {
            checks.push(HealthCheck {
                name: "app-paths".to_string(),
                status: "warning".to_string(),
                detail: "Could not resolve OS application directories".to_string(),
            });
        }
        let steam_pending = steam_input_discovery_pending(steam_input);
        checks.push(HealthCheck {
            name: "steam-input".to_string(),
            status: if steam_pending {
                "pending".to_string()
            } else if steam_input.running {
                "ok".to_string()
            } else if steam_input.available {
                "warning".to_string()
            } else {
                "pending".to_string()
            },
            detail: if steam_pending {
                "Steam Input discovery is warming in the background".to_string()
            } else if steam_input.running {
                format!(
                    "Steam is running; {} local controller layout(s) discovered",
                    steam_input.layouts.len()
                )
            } else if steam_input.available {
                "Steam is installed but not currently running".to_string()
            } else {
                "Steam install not found in standard locations".to_string()
            },
        });
        for runtime in inner.adapter_runtimes.values() {
            checks.push(adapter_runtime_health_check(runtime, Some(game_detection)));
        }
        checks.extend(inner.controllers.health_checks());

        DiagnosticsResponse {
            loopback_only: !hardware_output_enabled,
            hardware_required: hardware_output_enabled,
            checks,
        }
    }

    async fn snapshot(&self) -> AgentSnapshotResponse {
        let game_detection = self.cached_game_detection().await;
        let steam_input = self.cached_steam_input_status_or_refresh().await;
        let hardware_output_enabled = self.hardware_output_enabled();
        let inner = self.inner.read().await;
        let diagnostics = self.diagnostics_from_inner(
            &inner,
            &steam_input,
            &game_detection,
            hardware_output_enabled,
        );
        let status = self.status_from_inner(&inner, Some(&game_detection));
        let profile_resolution = profile_resolution(&inner, Some(&game_detection));
        let effect_state = self.current_effect_response_cached(
            &inner,
            Some(&game_detection),
            hardware_output_enabled,
            EffectEnginePurpose::Preview,
        );
        AgentSnapshotResponse {
            status,
            app_settings: self.app_settings_response(&inner.app_settings),
            controllers: apply_controller_names(
                inner.controllers.summaries(),
                &inner.controller_names,
            ),
            profiles: inner.profiles.clone(),
            adapters: materialized_adapters(&inner, Some(&game_detection)),
            modules: module_summaries(),
            steam_input,
            game_detection: game_detection.clone(),
            profile_resolution,
            effect_state,
            telemetry: materialized_telemetry_response(&inner, Some(&game_detection)),
            logs: inner
                .logs
                .iter()
                .rev()
                .take(32)
                .cloned()
                .collect::<Vec<_>>(),
            diagnostics,
            partial_errors: Vec::new(),
        }
    }

    pub async fn support_bundle(&self) -> SupportBundleResponse {
        let game_detection = self.cached_game_detection().await;
        let steam_input = self.cached_steam_input_status_or_refresh().await;
        let hardware_output_enabled = self.hardware_output_enabled();
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
                ],
            },
            environment: support_environment(),
            status: sanitize_status_response(status),
            paths: support_paths(),
            controllers: apply_controller_names(
                inner.controllers.summaries(),
                &inner.controller_names,
            ),
            diagnostics: sanitize_diagnostics_response(diagnostics),
            profile_resolution: profile_resolution(&inner, Some(&game_detection)),
            game_detection: support_game_detection_summary(&game_detection),
            adapters: support_adapter_summaries(&inner, Some(&game_detection)),
            telemetry: support_telemetry_summary(&inner, Some(&game_detection)),
            steam_input: support_steam_input_summary(&steam_input),
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

fn sanitize_diagnostics_response(mut diagnostics: DiagnosticsResponse) -> DiagnosticsResponse {
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
    materialized_adapters(inner, game_detection)
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

fn support_telemetry_summary(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> SupportTelemetrySummary {
    let telemetry = materialized_telemetry_response(inner, game_detection);
    let source_id = inner.telemetry.text("source.id").map(str::to_string);
    let live = game_detection
        .and_then(|detection| detection.adapter_id.as_deref())
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

fn support_steam_input_summary(status: &SteamInputStatus) -> SupportSteamInputSummary {
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

fn sanitize_support_text(value: &str) -> String {
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
    roots.sort_by_key(|root| std::cmp::Reverse(root.0.len()));
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

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

impl DiscoveredController {
    pub fn new(info: ControllerInfo, state: ControllerState) -> Self {
        Self {
            info,
            state,
            raw_device_id: None,
            name: None,
            transport_label: None,
            permission: ControllerPermissionState::Granted,
            diagnostics: Vec::new(),
        }
    }

    pub fn with_raw_device_id(mut self, raw_device_id: RawDeviceId) -> Self {
        self.raw_device_id = Some(raw_device_id);
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_transport_label(mut self, transport_label: impl Into<String>) -> Self {
        self.transport_label = Some(transport_label.into());
        self
    }

    pub fn with_permission(mut self, permission: ControllerPermissionState) -> Self {
        self.permission = permission;
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: ControllerDiagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }
}

impl ControllerConfig {
    fn default_for(controller_id: impl Into<String>, model: impl Into<String>) -> Self {
        let controller_id = controller_id.into();
        let model = model.into();
        let edge = model == "DualSense Edge";

        Self {
            controller_id,
            model,
            input_mode: ControllerInputMode::NativeDualSense,
            trigger: TriggerConfig::default(),
            lightbar: LightbarConfig::default(),
            forza: ForzaTelemetryConfig::default(),
            sticks: StickConfig::default(),
            buttons: default_button_assignments(edge),
            profile_assignments: default_profile_assignments(edge),
        }
    }

    fn from_update(
        controller_id: impl Into<String>,
        model: impl Into<String>,
        request: UpdateControllerConfigRequest,
    ) -> Self {
        let model = model.into();
        let edge = model == "DualSense Edge";
        Self {
            controller_id: controller_id.into(),
            model,
            input_mode: request.input_mode,
            trigger: request.trigger.normalized(),
            lightbar: request.lightbar.normalized(),
            forza: request.forza.normalized(),
            sticks: request.sticks.normalized(),
            buttons: normalize_controller_button_assignments(request.buttons, edge),
            profile_assignments: normalize_profile_assignments(request.profile_assignments),
        }
    }

    fn normalized(mut self) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.forza = self.forza.normalized();
        self.sticks = self.sticks.normalized();
        self.input_mode = match self.input_mode {
            ControllerInputMode::NativeDualSense => ControllerInputMode::NativeDualSense,
            ControllerInputMode::SteamInputCompanion => ControllerInputMode::SteamInputCompanion,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, self.model == "DualSense Edge");
        self.profile_assignments = normalize_profile_assignments(self.profile_assignments);
        self
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::from_controller_config(&ControllerConfig::default_for("", "DualSense"))
    }
}

impl ProfileConfig {
    fn from_controller_config(config: &ControllerConfig) -> Self {
        Self {
            input_mode: config.input_mode,
            trigger: config.trigger.clone(),
            lightbar: config.lightbar.clone(),
            forza: config.forza.clone(),
            sticks: config.sticks.clone(),
            buttons: config.buttons.clone(),
        }
        .normalized_for_model(&config.model)
    }

    fn normalized_for_model(mut self, model: &str) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.forza = self.forza.normalized();
        self.sticks = self.sticks.normalized();
        self.input_mode = match self.input_mode {
            ControllerInputMode::NativeDualSense => ControllerInputMode::NativeDualSense,
            ControllerInputMode::SteamInputCompanion => ControllerInputMode::SteamInputCompanion,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, model == "DualSense Edge");
        self
    }

    fn apply_to_controller_config(&self, config: &mut ControllerConfig) {
        let profile_config = self.clone().normalized_for_model(&config.model);
        config.input_mode = profile_config.input_mode;
        config.trigger = profile_config.trigger;
        config.lightbar = profile_config.lightbar;
        config.forza = profile_config.forza;
        config.sticks = profile_config.sticks;
        config.buttons = profile_config.buttons;
    }
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            same_range: false,
            l2_from: 20,
            l2_to: 100,
            r2_from: 0,
            r2_to: 100,
            l2_curve: TriggerCurve::default_l2(),
            r2_curve: TriggerCurve::default_r2(),
            l2_curve_points: default_l2_trigger_curve_points(),
            r2_curve_points: default_r2_trigger_curve_points(),
            effect: "Adaptive resistance".to_string(),
            intensity: "Strong (Standard)".to_string(),
            vibration: "Medium".to_string(),
            vibration_mode: "Balanced".to_string(),
        }
    }
}

impl TriggerConfig {
    fn normalized(mut self) -> Self {
        self.l2_from = self.l2_from.min(100);
        self.l2_to = self.l2_to.clamp(self.l2_from, 100);
        self.r2_from = self.r2_from.min(100);
        self.r2_to = self.r2_to.clamp(self.r2_from, 100);
        if self.same_range {
            self.r2_from = self.l2_from;
            self.r2_to = self.l2_to;
        }
        self.l2_curve = self.l2_curve.normalized();
        self.r2_curve = self.r2_curve.normalized();
        self.l2_curve_points = normalize_trigger_curve_points(self.l2_curve_points, self.l2_curve);
        self.r2_curve_points = normalize_trigger_curve_points(self.r2_curve_points, self.r2_curve);
        if !["Adaptive resistance", "Pulse", "Wall", "Wall pulse", "Off"]
            .contains(&self.effect.as_str())
        {
            self.effect = "Adaptive resistance".to_string();
        }
        if !["Off", "Weak", "Medium", "Strong (Standard)"].contains(&self.intensity.as_str()) {
            self.intensity = "Medium".to_string();
        }
        if !["Off", "Low", "Medium", "High"].contains(&self.vibration.as_str()) {
            self.vibration = "Medium".to_string();
        }
        if !["Balanced", "Deep thump", "Fine buzz"].contains(&self.vibration_mode.as_str()) {
            self.vibration_mode = "Balanced".to_string();
        }
        self
    }
}

impl Default for ForzaTelemetryConfig {
    fn default() -> Self {
        Self {
            body_rumble_mode: default_forza_body_rumble_mode(),
            effects: default_forza_effect_configs(),
        }
    }
}

impl ForzaTelemetryConfig {
    fn normalized(self) -> Self {
        let body_rumble_mode =
            if forza_body_rumble_modes().contains(&self.body_rumble_mode.as_str()) {
                self.body_rumble_mode
            } else {
                default_forza_body_rumble_mode()
            };
        let mut provided = self
            .effects
            .into_iter()
            .map(|effect| (effect.id.clone(), effect))
            .collect::<BTreeMap<_, _>>();
        let mut effects = Vec::new();

        for default in default_forza_effect_configs() {
            let effect = provided
                .remove(&default.id)
                .unwrap_or_else(|| default.clone())
                .normalized_with_default(&default);
            effects.push(effect);
        }

        for (_, effect) in provided {
            if !effect.id.trim().is_empty() {
                let default = ForzaEffectConfig {
                    id: effect.id.clone(),
                    enabled: true,
                    intensity: 100,
                    route: "body_both".to_string(),
                };
                effects.push(effect.normalized_with_default(&default));
            }
        }

        Self {
            body_rumble_mode,
            effects,
        }
    }

    fn effect(&self, id: &str) -> ForzaEffectConfig {
        let default = default_forza_effect(id);
        self.effects
            .iter()
            .find(|effect| effect.id == id)
            .cloned()
            .unwrap_or_else(|| default.clone())
            .normalized_with_default(&default)
    }
}

impl ForzaEffectConfig {
    fn normalized_with_default(mut self, default: &ForzaEffectConfig) -> Self {
        if self.id.trim().is_empty() {
            self.id = default.id.clone();
        }
        if !forza_effect_routes().contains(&self.route.as_str()) {
            self.route = default.route.clone();
        }
        self
    }

    fn scalar(&self) -> f64 {
        if self.enabled {
            f64::from(self.intensity) / 100.0
        } else {
            0.0
        }
    }
}

fn default_forza_effect_enabled() -> bool {
    true
}

fn default_forza_effect_intensity() -> u8 {
    100
}

fn default_forza_effect_route() -> String {
    "body_both".to_string()
}

fn default_forza_body_rumble_mode() -> String {
    "native_passthrough".to_string()
}

fn forza_body_rumble_modes() -> &'static [&'static str] {
    &["native_passthrough", "dscc_full_control"]
}

fn default_forza_effect(id: &str) -> ForzaEffectConfig {
    default_forza_effect_configs()
        .into_iter()
        .find(|effect| effect.id == id)
        .unwrap_or_else(|| ForzaEffectConfig {
            id: id.to_string(),
            enabled: true,
            intensity: 100,
            route: "body_both".to_string(),
        })
}

fn default_forza_effect_configs() -> Vec<ForzaEffectConfig> {
    [
        ("brake_resistance", 100, "l2"),
        ("abs_slip_pulse", 100, "l2"),
        ("handbrake_wall", 100, "l2"),
        ("throttle_resistance", 100, "r2"),
        (
            "gear_shift_thump",
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("rev_limiter_buzz", 120, "r2"),
        ("road_texture", 60, "body_both"),
        ("rumble_strip", 72, "body_both"),
        ("tire_slip", 95, "body_right"),
        ("puddle_drag", 75, "body_left"),
        ("suspension_impact", 115, "body_both"),
        ("rpm_leds", 100, "light_led"),
    ]
    .into_iter()
    .map(|(id, intensity, route)| ForzaEffectConfig {
        id: id.to_string(),
        enabled: true,
        intensity,
        route: route.to_string(),
    })
    .collect()
}

fn forza_effect_routes() -> &'static [&'static str] {
    &[
        "body_both",
        "body_left",
        "body_right",
        "l2",
        "r2",
        "both_triggers",
        "body_and_triggers",
        "r2_and_body",
        "light_led",
    ]
}

impl Default for LightbarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            color: "#4cc9f0".to_string(),
            rpm_color: default_rpm_color(),
            brightness: 72,
        }
    }
}

impl LightbarConfig {
    fn normalized(mut self) -> Self {
        self.color = normalize_hex_color(&self.color);
        self.rpm_color = normalize_hex_color_or(&self.rpm_color, "#ff3a2e");
        self.brightness = self.brightness.min(100);
        self
    }

    fn rgb(&self) -> RgbColor {
        let normalized = normalize_hex_color(&self.color);
        let value = normalized.trim_start_matches('#');
        RgbColor {
            red: u8::from_str_radix(&value[0..2], 16).unwrap_or(0x4c),
            green: u8::from_str_radix(&value[2..4], 16).unwrap_or(0xc9),
            blue: u8::from_str_radix(&value[4..6], 16).unwrap_or(0xf0),
        }
    }

    fn rpm_rgb(&self) -> RgbColor {
        let normalized = normalize_hex_color_or(&self.rpm_color, "#ff3a2e");
        let value = normalized.trim_start_matches('#');
        RgbColor {
            red: u8::from_str_radix(&value[0..2], 16).unwrap_or(0xff),
            green: u8::from_str_radix(&value[2..4], 16).unwrap_or(0x3a),
            blue: u8::from_str_radix(&value[4..6], 16).unwrap_or(0x2e),
        }
    }
}

fn normalize_hex_color(value: &str) -> String {
    normalize_hex_color_or(value, "#4cc9f0")
}

fn normalize_hex_color_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        format!("#{hex}").to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

fn rgb_from_hex(value: &str) -> Option<RgbColor> {
    let normalized = normalize_hex_color_or(value, "");
    let value = normalized.strip_prefix('#')?;
    Some(RgbColor {
        red: u8::from_str_radix(&value[0..2], 16).ok()?,
        green: u8::from_str_radix(&value[2..4], 16).ok()?,
        blue: u8::from_str_radix(&value[4..6], 16).ok()?,
    })
}

fn default_rpm_color() -> String {
    "#ff3a2e".to_string()
}

impl Default for StickConfig {
    fn default() -> Self {
        Self {
            left_curve: "Quick".to_string(),
            left_curve_amount: 48,
            left_deadzone: 4,
            right_curve: "Default".to_string(),
            right_curve_amount: 62,
            right_deadzone: 8,
        }
    }
}

impl StickConfig {
    fn normalized(mut self) -> Self {
        for curve in [&mut self.left_curve, &mut self.right_curve] {
            if ![
                "Default", "Quick", "Precise", "Steady", "Digital", "Dynamic",
            ]
            .contains(&curve.as_str())
            {
                *curve = "Default".to_string();
            }
        }
        self.left_curve_amount = self.left_curve_amount.min(100);
        self.right_curve_amount = self.right_curve_amount.min(100);
        self.left_deadzone = self.left_deadzone.min(40);
        self.right_deadzone = self.right_deadzone.min(40);
        self
    }
}

impl ButtonAssignmentConfig {
    fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

impl EdgeProfileStore {
    fn normalized(mut self) -> Self {
        self.slots = self
            .slots
            .into_iter()
            .map(|(slot, config)| (slot, config.normalized()))
            .collect();
        self
    }
}

impl EdgeProfileSlotConfig {
    fn normalized(mut self) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.sticks = self.sticks.normalized();
        self.buttons = normalize_controller_button_assignments(self.buttons, true);
        self
    }
}

impl EdgeProfilesResponse {
    fn for_controller(detail: &ControllerDetail, store: Option<&EdgeProfileStore>) -> Self {
        if detail.model != "DualSense Edge" {
            return Self {
                controller_id: detail.id.clone(),
                support_state: EdgeProfileSupportState::Unsupported,
                warning:
                    "Onboard profile read/write is only planned for DualSense Edge controllers."
                        .to_string(),
                slots: Vec::new(),
            };
        }

        Self {
            controller_id: detail.id.clone(),
            support_state: EdgeProfileSupportState::Unknown,
            warning: "Edge onboard slot editing is available as DSCC staged configuration. Hardware profile sync is attempted over USB or Bluetooth when this host exposes safe HID feature-report access.".to_string(),
            slots: edge_profile_slots(store),
        }
    }

    fn for_controller_with_hardware(
        detail: &ControllerDetail,
        store: Option<&EdgeProfileStore>,
        hardware_profiles: Option<&[EdgeOnboardProfile]>,
        hardware_warning: Option<String>,
        hardware_writes_enabled: bool,
    ) -> Self {
        if detail.model != "DualSense Edge" {
            return Self::for_controller(detail, store);
        }

        let Some(hardware_profiles) = hardware_profiles else {
            let warning = hardware_warning.unwrap_or_else(|| {
                "Edge onboard slots can be staged locally. Connect the DualSense Edge over USB or Bluetooth and refresh to read or write controller memory.".to_string()
            });
            return Self {
                controller_id: detail.id.clone(),
                support_state: EdgeProfileSupportState::Unknown,
                warning,
                slots: edge_profile_slots(store),
            };
        };

        let (support_state, warning) = if hardware_writes_enabled {
            (
                EdgeProfileSupportState::ReadWrite,
                format!("DualSense Edge onboard slots were read over {}. DSCC can write static onboard trigger, stick, and button settings; live telemetry effects still require DSCC to be running.", detail.transport),
            )
        } else {
            (
                EdgeProfileSupportState::ReadOnly,
                format!("DualSense Edge onboard slots were read over {}, but hardware writes are disabled by DSCC output mode. Slot changes will be staged locally.", detail.transport),
            )
        };

        Self {
            controller_id: detail.id.clone(),
            support_state,
            warning,
            slots: edge_profile_slots_with_hardware(store, hardware_profiles),
        }
    }
}

fn edge_profile_slots(store: Option<&EdgeProfileStore>) -> Vec<EdgeProfileSlot> {
    let staged = |slot: &str| store.and_then(|store| store.slots.get(slot)).cloned();
    let slot_state = |slot: &str| {
        if staged(slot).is_some() {
            EdgeProfileSlotState::Assigned
        } else {
            EdgeProfileSlotState::Unknown
        }
    };

    vec![
        EdgeProfileSlot {
            slot_id: "default".to_string(),
            shortcut: "Fn + Triangle".to_string(),
            name: Some("Default Profile".to_string()),
            state: EdgeProfileSlotState::Default,
            editable: false,
            hardware_synced: true,
            staged: None,
        },
        EdgeProfileSlot {
            slot_id: "circle".to_string(),
            shortcut: "Fn + Circle".to_string(),
            name: staged("circle").map(|profile| profile.name),
            state: slot_state("circle"),
            editable: true,
            hardware_synced: false,
            staged: staged("circle"),
        },
        EdgeProfileSlot {
            slot_id: "cross".to_string(),
            shortcut: "Fn + Cross".to_string(),
            name: staged("cross").map(|profile| profile.name),
            state: slot_state("cross"),
            editable: true,
            hardware_synced: false,
            staged: staged("cross"),
        },
        EdgeProfileSlot {
            slot_id: "square".to_string(),
            shortcut: "Fn + Square".to_string(),
            name: staged("square").map(|profile| profile.name),
            state: slot_state("square"),
            editable: true,
            hardware_synced: false,
            staged: staged("square"),
        },
    ]
}

fn edge_profile_slots_with_hardware(
    store: Option<&EdgeProfileStore>,
    hardware_profiles: &[EdgeOnboardProfile],
) -> Vec<EdgeProfileSlot> {
    [
        EdgeOnboardSlotId::Default,
        EdgeOnboardSlotId::Square,
        EdgeOnboardSlotId::Cross,
        EdgeOnboardSlotId::Circle,
    ]
    .into_iter()
    .map(|slot| {
        let slot_id = slot.as_str();
        let staged = store.and_then(|store| store.slots.get(slot_id)).cloned();
        let hardware = hardware_profiles
            .iter()
            .find(|profile| profile.slot == slot)
            .cloned();

        if slot == EdgeOnboardSlotId::Default {
            return EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: hardware
                    .as_ref()
                    .filter(|profile| profile.assigned && !profile.name.trim().is_empty())
                    .map(|profile| profile.name.clone())
                    .or_else(|| Some("Default Profile".to_string())),
                state: EdgeProfileSlotState::Default,
                editable: false,
                hardware_synced: true,
                staged: hardware.as_ref().map(edge_profile_config_from_hardware),
            };
        }

        if let Some(staged) = staged.filter(|profile| !profile.hardware_synced) {
            return EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: Some(staged.name.clone()),
                state: EdgeProfileSlotState::Assigned,
                editable: true,
                hardware_synced: false,
                staged: Some(staged),
            };
        }

        match hardware {
            Some(profile) if profile.assigned => {
                let config = edge_profile_config_from_hardware(&profile);
                EdgeProfileSlot {
                    slot_id: slot_id.to_string(),
                    shortcut: slot.shortcut().to_string(),
                    name: Some(config.name.clone()),
                    state: EdgeProfileSlotState::Assigned,
                    editable: true,
                    hardware_synced: true,
                    staged: Some(config),
                }
            }
            Some(_) => EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: None,
                state: EdgeProfileSlotState::Empty,
                editable: true,
                hardware_synced: true,
                staged: None,
            },
            None => EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: store
                    .and_then(|store| store.slots.get(slot_id))
                    .map(|profile| profile.name.clone()),
                state: EdgeProfileSlotState::Unknown,
                editable: true,
                hardware_synced: false,
                staged: store.and_then(|store| store.slots.get(slot_id)).cloned(),
            },
        }
    })
    .collect()
}

fn default_profile_assignments(edge: bool) -> Vec<ProfileAssignmentConfig> {
    vec![
        ProfileAssignmentConfig {
            game_id: "forza-horizon-6".to_string(),
            game_name: "Forza Horizon 6".to_string(),
            profile_id: IMMERSIVE_PROFILE_ID.to_string(),
            profile_name: "Immersive".to_string(),
            state: "ready".to_string(),
            detail: if edge {
                "Throttle, brake, slip, road texture (Edge)"
            } else {
                "Throttle, brake, slip, road texture"
            }
            .to_string(),
        },
        ProfileAssignmentConfig {
            game_id: "forza-horizon-5".to_string(),
            game_name: "Forza Horizon 5".to_string(),
            profile_id: IMMERSIVE_PROFILE_ID.to_string(),
            profile_name: "Immersive".to_string(),
            state: "ready".to_string(),
            detail: "Horizon 5-compatible Data Out signals".to_string(),
        },
        ProfileAssignmentConfig {
            game_id: "assetto-corsa-rally".to_string(),
            game_name: "Assetto Corsa Rally".to_string(),
            profile_id: ASSETTO_CORSA_RALLY_PROFILE_ID.to_string(),
            profile_name: "Rally".to_string(),
            state: "ready".to_string(),
            detail: "Shared-memory rally telemetry".to_string(),
        },
    ]
}

fn default_button_assignments(edge: bool) -> Vec<ButtonAssignmentConfig> {
    let mut buttons = vec![
        ButtonAssignmentConfig::new("Cross", "Cross"),
        ButtonAssignmentConfig::new("Circle", "Circle"),
        ButtonAssignmentConfig::new("Square", "Square"),
        ButtonAssignmentConfig::new("Triangle", "Triangle"),
        ButtonAssignmentConfig::new("D-Pad", "D-Pad"),
        ButtonAssignmentConfig::new("L1", "L1"),
        ButtonAssignmentConfig::new("R1", "R1"),
        ButtonAssignmentConfig::new("L2", "L2"),
        ButtonAssignmentConfig::new("R2", "R2"),
        ButtonAssignmentConfig::new("L3", "L3"),
        ButtonAssignmentConfig::new("R3", "R3"),
        ButtonAssignmentConfig::new("Create", "Create"),
        ButtonAssignmentConfig::new("Options", "Options"),
        ButtonAssignmentConfig::new("Touch Pad", "Touch Pad Press"),
        ButtonAssignmentConfig::new("Mute", "Mute"),
    ];
    if edge {
        buttons.extend([
            ButtonAssignmentConfig::new("Back Left", "L3"),
            ButtonAssignmentConfig::new("Back Right", "R3"),
            ButtonAssignmentConfig::new("Fn Left", "Previous DSCC Profile"),
            ButtonAssignmentConfig::new("Fn Right", "Next DSCC Profile"),
        ]);
    }
    buttons
}

fn normalize_controller_button_assignments(
    buttons: Vec<ButtonAssignmentConfig>,
    edge: bool,
) -> Vec<ButtonAssignmentConfig> {
    let mut normalized = normalize_button_assignments(buttons);
    let defaults = default_button_assignments(edge);
    let mut ordered = Vec::with_capacity(defaults.len().max(normalized.len()).min(24));

    for default in defaults {
        if let Some(index) = normalized
            .iter()
            .position(|button| button.key == default.key)
        {
            ordered.push(normalized.remove(index));
        } else {
            ordered.push(default);
        }
    }

    let remaining = 24_usize.saturating_sub(ordered.len());
    ordered.extend(normalized.into_iter().take(remaining));
    ordered
}

fn model_hint_for_profile_buttons(buttons: &[ButtonAssignmentConfig]) -> &'static str {
    if buttons.iter().any(|button| {
        matches!(
            button.key.as_str(),
            "Back Left" | "Back Right" | "Fn Left" | "Fn Right"
        )
    }) {
        "DualSense Edge"
    } else {
        "DualSense"
    }
}

fn normalize_button_assignments(
    buttons: Vec<ButtonAssignmentConfig>,
) -> Vec<ButtonAssignmentConfig> {
    buttons
        .into_iter()
        .filter(|button| !button.key.trim().is_empty())
        .map(normalize_button_assignment)
        .take(24)
        .collect()
}

fn normalize_button_assignment(button: ButtonAssignmentConfig) -> ButtonAssignmentConfig {
    let key = normalize_button_key(&button.key);
    let label = normalize_button_label(&key, &button.label);
    ButtonAssignmentConfig { key, label }
}

fn normalize_button_key(key: &str) -> String {
    match key.trim() {
        "" => "Unassigned".to_string(),
        other => other.chars().take(24).collect(),
    }
}

fn normalize_button_label(key: &str, label: &str) -> String {
    let trimmed = label.trim();
    let normalized = if trimmed.is_empty() {
        default_assignment_for_key(key)
    } else {
        trimmed.to_string()
    };

    if is_supported_assignment_label(&normalized) {
        normalized
    } else {
        default_assignment_for_key(key)
    }
}

fn default_assignment_for_key(key: &str) -> String {
    match key {
        "Back Left" => "L3",
        "Back Right" => "R3",
        "Fn Left" => "Previous DSCC Profile",
        "Fn Right" => "Next DSCC Profile",
        "Touch Pad" => "Touch Pad Press",
        other if is_supported_assignment_label(other) => other,
        _ => "Unassigned",
    }
    .to_string()
}

fn is_supported_assignment_label(label: &str) -> bool {
    matches!(
        label,
        "Unassigned"
            | "Cross"
            | "Circle"
            | "Square"
            | "Triangle"
            | "D-Pad"
            | "D-Pad Up"
            | "D-Pad Down"
            | "D-Pad Left"
            | "D-Pad Right"
            | "L1"
            | "R1"
            | "L2"
            | "R2"
            | "L3"
            | "R3"
            | "Create"
            | "Options"
            | "Touch Pad Press"
            | "Mute"
            | "Previous DSCC Profile"
            | "Next DSCC Profile"
            | "Toggle Telemetry Overlay"
            | "Toggle Effect Preview"
    )
}

fn normalize_profile_assignments(
    assignments: Vec<ProfileAssignmentConfig>,
) -> Vec<ProfileAssignmentConfig> {
    assignments
        .into_iter()
        .filter(|assignment| {
            !assignment.game_id.trim().is_empty() && !assignment.profile_id.trim().is_empty()
        })
        .take(12)
        .collect()
}

fn normalize_existing_profile_assignments(
    assignments: Vec<ProfileAssignmentConfig>,
    persisted_profiles: &[ProfileSummary],
) -> Vec<ProfileAssignmentConfig> {
    normalize_profile_assignments(assignments)
        .into_iter()
        .filter(|assignment| {
            profile_exists_in_defaults_or_persisted(&assignment.profile_id, persisted_profiles)
        })
        .collect()
}

fn edge_profile_config_from_request(request: UpdateEdgeProfileRequest) -> EdgeProfileSlotConfig {
    EdgeProfileSlotConfig {
        name: if request.name.trim().is_empty() {
            "Untitled Edge Profile".to_string()
        } else {
            request.name.trim().chars().take(64).collect()
        },
        trigger: request.trigger.normalized(),
        lightbar: request.lightbar.normalized(),
        sticks: request.sticks.normalized(),
        buttons: normalize_controller_button_assignments(request.buttons, true),
        updated_at: current_timestamp(),
        hardware_synced: false,
    }
}

fn edge_slot_id_from_api(slot: &str) -> Option<EdgeOnboardSlotId> {
    match slot {
        "default" | "triangle" => Some(EdgeOnboardSlotId::Default),
        "square" => Some(EdgeOnboardSlotId::Square),
        "cross" => Some(EdgeOnboardSlotId::Cross),
        "circle" => Some(EdgeOnboardSlotId::Circle),
        _ => None,
    }
}

fn edge_profile_config_from_hardware(profile: &EdgeOnboardProfile) -> EdgeProfileSlotConfig {
    let mut trigger = TriggerConfig::default();
    trigger.same_range = profile.trigger_deadzone.unified;
    trigger.l2_from = profile.trigger_deadzone.left[0].min(100);
    trigger.l2_to = profile.trigger_deadzone.left[1].clamp(trigger.l2_from, 100);
    trigger.r2_from = profile.trigger_deadzone.right[0].min(100);
    trigger.r2_to = profile.trigger_deadzone.right[1].clamp(trigger.r2_from, 100);
    trigger.intensity = edge_trigger_intensity_label(profile.trigger_effect_intensity).to_string();
    trigger.vibration = edge_vibration_label(profile.vibration_intensity).to_string();

    EdgeProfileSlotConfig {
        name: if profile.name.trim().is_empty() {
            profile.slot.shortcut().to_string()
        } else {
            profile.name.trim().chars().take(64).collect()
        },
        trigger: trigger.normalized(),
        lightbar: LightbarConfig::default(),
        sticks: StickConfig {
            left_curve: profile.left_stick.preset.as_str().to_string(),
            left_curve_amount: 50,
            left_deadzone: 0,
            right_curve: profile.right_stick.preset.as_str().to_string(),
            right_curve_amount: 50,
            right_deadzone: 0,
        }
        .normalized(),
        buttons: edge_button_assignments_from_hardware(&profile.button_mappings),
        updated_at: edge_profile_updated_at(profile.updated_at_ms),
        hardware_synced: true,
    }
}

fn edge_profile_from_slot_config(
    slot: EdgeOnboardSlotId,
    config: &EdgeProfileSlotConfig,
) -> EdgeOnboardProfile {
    let config = config.clone().normalized();
    let mut profile = EdgeOnboardProfile::new(slot, config.name.clone());
    profile.trigger_deadzone = EdgeTriggerDeadzone {
        left: [config.trigger.l2_from, config.trigger.l2_to],
        right: [config.trigger.r2_from, config.trigger.r2_to],
        unified: config.trigger.same_range,
    };
    profile.left_stick = EdgeStickProfile {
        preset: EdgeStickPreset::from_label(&config.sticks.left_curve),
        ..EdgeStickProfile::default()
    };
    profile.right_stick = EdgeStickProfile {
        preset: EdgeStickPreset::from_label(&config.sticks.right_curve),
        ..EdgeStickProfile::default()
    };
    profile.trigger_effect_intensity =
        edge_profile_intensity_from_trigger(&config.trigger.intensity);
    profile.vibration_intensity = edge_profile_intensity_from_vibration(&config.trigger.vibration);
    profile.button_mappings = edge_button_mappings_from_config(&config.buttons);
    profile.updated_at_ms = current_timestamp_millis();
    profile
}

fn edge_button_assignments_from_hardware(
    mappings: &[EdgeButtonMapping],
) -> Vec<ButtonAssignmentConfig> {
    let mut assignments: Vec<_> = mappings
        .iter()
        .filter(|mapping| mapping.source != mapping.target)
        .map(|mapping| ButtonAssignmentConfig {
            key: mapping.source.label().to_string(),
            label: mapping.target.label().to_string(),
        })
        .collect();

    if assignments.is_empty() {
        assignments = default_button_assignments(true);
    }

    normalize_controller_button_assignments(assignments, true)
}

fn edge_button_mappings_from_config(buttons: &[ButtonAssignmentConfig]) -> Vec<EdgeButtonMapping> {
    let mut mappings = dscc_device::default_button_mappings().to_vec();
    for button in buttons {
        let Some(source) = EdgeButton::from_label(&button.key) else {
            continue;
        };
        let Some(target) = EdgeButton::from_label(&button.label) else {
            continue;
        };
        if let Some(mapping) = mappings.iter_mut().find(|mapping| mapping.source == source) {
            mapping.target = target;
        } else {
            mappings.push(EdgeButtonMapping { source, target });
        }
    }
    mappings
}

fn edge_trigger_intensity_label(value: EdgeProfileIntensity) -> &'static str {
    match value {
        EdgeProfileIntensity::Off => "Off",
        EdgeProfileIntensity::Weak => "Weak",
        EdgeProfileIntensity::Medium => "Medium",
        EdgeProfileIntensity::Strong => "Strong (Standard)",
    }
}

fn edge_vibration_label(value: EdgeProfileIntensity) -> &'static str {
    match value {
        EdgeProfileIntensity::Off => "Off",
        EdgeProfileIntensity::Weak => "Low",
        EdgeProfileIntensity::Medium => "Medium",
        EdgeProfileIntensity::Strong => "High",
    }
}

fn edge_profile_intensity_from_trigger(value: &str) -> EdgeProfileIntensity {
    EdgeProfileIntensity::from_label(value.trim().strip_suffix(" (Standard)").unwrap_or(value))
}

fn edge_profile_intensity_from_vibration(value: &str) -> EdgeProfileIntensity {
    EdgeProfileIntensity::from_label(match value.trim() {
        "Low" => "Weak",
        "High" => "Strong",
        other => other,
    })
}

fn edge_profile_updated_at(updated_at_ms: u64) -> String {
    if updated_at_ms == 0 {
        return current_timestamp();
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at_ms as i64)
        .map(|timestamp| timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(current_timestamp)
}

fn profile_override_key(controller_id: Option<&str>, game_id: Option<&str>) -> String {
    format!(
        "{}:{}",
        controller_id.unwrap_or("*"),
        game_id.unwrap_or("*")
    )
}

fn normalize_controller_display_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.chars().take(64).collect())
}

fn apply_controller_names(
    mut controllers: Vec<ControllerSummary>,
    names: &BTreeMap<String, String>,
) -> Vec<ControllerSummary> {
    for controller in &mut controllers {
        if let Some(name) = names.get(&controller.id) {
            controller.name = name.clone();
        }
    }
    controllers
}

fn apply_controller_name(
    mut detail: ControllerDetail,
    names: &BTreeMap<String, String>,
) -> ControllerDetail {
    if let Some(name) = names.get(&detail.id) {
        detail.name = name.clone();
    }
    detail
}

impl ControllerDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Info,
            message: message.into(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default)]
struct ControllerRegistry {
    controllers: BTreeMap<String, ControllerRecord>,
    global_diagnostics: Vec<ControllerDiagnostic>,
}

impl ControllerRegistry {
    fn apply(&mut self, event: ControllerDiscoveryEvent) {
        match event {
            ControllerDiscoveryEvent::Attached(controller) => {
                if self.should_skip_attach(&controller) {
                    return;
                }
                self.remove_redundant_windows_pnp(&controller);
                let id = controller.info.id.0.clone();
                self.remove_disconnected_duplicate(&id, &controller.info);
                self.controllers
                    .insert(id, ControllerRecord::from_discovered(controller));
            }
            ControllerDiscoveryEvent::Detached(id) => {
                if let Some(record) = self.controllers.get_mut(&id.0) {
                    record.mark_disconnected();
                } else {
                    self.global_diagnostics.push(ControllerDiagnostic::warning(
                        "controller_detached_unknown",
                        format!("Received detach event for unknown controller {}", id.0),
                    ));
                }
            }
            ControllerDiscoveryEvent::StatusChanged(state) => {
                if let Some(record) = self.controllers.get_mut(&state.id.0) {
                    record.update_state(state);
                } else {
                    self.global_diagnostics.push(ControllerDiagnostic::warning(
                        "controller_status_unknown",
                        format!(
                            "Received status update for unknown controller {}",
                            state.id.0
                        ),
                    ));
                }
            }
            ControllerDiscoveryEvent::PermissionDenied(problem) => {
                let diagnostic =
                    ControllerDiagnostic::error("controller_permission_denied", problem.message);
                if let Some(id) = problem.id {
                    if let Some(record) = self.controllers.get_mut(&id.0) {
                        record.mark_permission_denied(diagnostic);
                    } else {
                        self.global_diagnostics.push(ControllerDiagnostic::error(
                            "controller_permission_denied",
                            format!("Permission denied for unknown controller {}", id.0),
                        ));
                    }
                } else {
                    self.global_diagnostics.push(diagnostic);
                }
            }
            ControllerDiscoveryEvent::Faulted { id, message } => {
                let diagnostic = ControllerDiagnostic::error("controller_faulted", message);
                if let Some(id) = id {
                    if let Some(record) = self.controllers.get_mut(&id.0) {
                        record.mark_faulted(diagnostic);
                    } else {
                        self.global_diagnostics.push(ControllerDiagnostic::error(
                            "controller_faulted",
                            format!("Fault event for unknown controller {}", id.0),
                        ));
                    }
                } else {
                    self.global_diagnostics.push(diagnostic);
                }
            }
        }
    }

    fn is_redundant_attach(&self, event: &ControllerDiscoveryEvent) -> bool {
        let ControllerDiscoveryEvent::Attached(controller) = event else {
            return false;
        };
        self.controllers
            .get(&controller.info.id.0)
            .is_some_and(|record| {
                record.state.connection == ConnectionState::Connected
                    && record.matches_identity(&controller.info)
            })
    }

    fn should_skip_attach(&self, controller: &DiscoveredController) -> bool {
        is_windows_pnp_controller_id(&controller.info.id.0)
            && self.controllers.values().any(|record| {
                !is_windows_pnp_controller_id(&record.info.id.0)
                    && record.state.connection == ConnectionState::Connected
            })
    }

    fn remove_redundant_windows_pnp(&mut self, controller: &DiscoveredController) {
        if is_windows_pnp_controller_id(&controller.info.id.0) {
            return;
        }
        self.controllers.retain(|id, record| {
            !(is_windows_pnp_controller_id(id)
                && record.state.connection == ConnectionState::Connected)
        });
    }

    fn remove_disconnected_duplicate(&mut self, attached_id: &str, attached_info: &ControllerInfo) {
        let duplicate_id = self
            .controllers
            .iter()
            .find(|(id, record)| {
                id.as_str() != attached_id
                    && record.state.connection != ConnectionState::Connected
                    && record.matches_identity(attached_info)
            })
            .map(|(id, _)| id.clone());

        if let Some(id) = duplicate_id {
            self.controllers.remove(&id);
        }
    }

    fn detail(&self, id: &str) -> Option<ControllerDetail> {
        self.controllers.get(id).map(ControllerRecord::detail)
    }

    fn output_target(&self, id: &str) -> Option<ControllerOutputTarget> {
        self.controllers
            .get(id)
            .and_then(ControllerRecord::output_target)
    }

    fn summaries(&self) -> Vec<ControllerSummary> {
        self.controllers
            .values()
            .map(ControllerRecord::summary)
            .collect()
    }

    fn summary_for(&self, id: &ControllerId) -> Option<ControllerSummary> {
        self.controllers.get(&id.0).map(ControllerRecord::summary)
    }

    fn realtime_message_for(&self, event: &ControllerDiscoveryEvent) -> RealtimeMessage {
        match event {
            ControllerDiscoveryEvent::Attached(controller) => RealtimeMessage {
                kind: "controller_attached".to_string(),
                controller: self.summary_for(&controller.info.id),
                message: None,
            },
            ControllerDiscoveryEvent::Detached(id) => RealtimeMessage {
                kind: "controller_detached".to_string(),
                controller: self.summary_for(id),
                message: None,
            },
            ControllerDiscoveryEvent::StatusChanged(state) => RealtimeMessage {
                kind: "controller_status".to_string(),
                controller: self.summary_for(&state.id),
                message: None,
            },
            ControllerDiscoveryEvent::PermissionDenied(problem) => RealtimeMessage {
                kind: "controller_permission_denied".to_string(),
                controller: problem.id.as_ref().and_then(|id| self.summary_for(id)),
                message: Some(problem.message.clone()),
            },
            ControllerDiscoveryEvent::Faulted { id, message } => RealtimeMessage {
                kind: "controller_faulted".to_string(),
                controller: id.as_ref().and_then(|id| self.summary_for(id)),
                message: Some(message.clone()),
            },
        }
    }

    fn health_checks(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();
        if self.controllers.is_empty() {
            checks.push(HealthCheck {
                name: "controller-discovery".to_string(),
                status: "warning".to_string(),
                detail: "No supported controllers are known to the agent".to_string(),
            });
        }

        for record in self.controllers.values() {
            checks.push(record.health_check());
        }

        checks.extend(
            self.global_diagnostics
                .iter()
                .map(|diagnostic| HealthCheck {
                    name: diagnostic.code.clone(),
                    status: severity_status(diagnostic.severity).to_string(),
                    detail: diagnostic.message.clone(),
                }),
        );

        checks
    }
}

#[derive(Debug, Clone)]
struct ControllerRecord {
    info: ControllerInfo,
    state: ControllerState,
    raw_device_id: Option<RawDeviceId>,
    name: String,
    transport: String,
    permission: ControllerPermissionState,
    diagnostic_state: ControllerDiagnosticState,
    diagnostics: Vec<ControllerDiagnostic>,
}

impl ControllerRecord {
    fn from_discovered(controller: DiscoveredController) -> Self {
        let diagnostic_state =
            diagnostic_state_for(controller.permission, controller.state.connection);
        let name = controller
            .name
            .unwrap_or_else(|| family_label(controller.info.family).to_string());
        let transport = controller
            .transport_label
            .unwrap_or_else(|| transport_label(controller.info.transport).to_string());

        Self {
            info: controller.info,
            state: controller.state,
            raw_device_id: controller.raw_device_id,
            name,
            transport,
            permission: controller.permission,
            diagnostic_state,
            diagnostics: controller.diagnostics,
        }
    }

    fn update_state(&mut self, state: ControllerState) {
        self.state = state;
        if self.permission != ControllerPermissionState::Denied {
            self.diagnostic_state = diagnostic_state_for(self.permission, self.state.connection);
        }
    }

    fn mark_disconnected(&mut self) {
        self.state.connection = ConnectionState::Disconnected;
        self.diagnostic_state = ControllerDiagnosticState::Disconnected;
        self.diagnostics.push(ControllerDiagnostic::warning(
            "controller_disconnected",
            "Controller was detached from the device backend",
        ));
    }

    fn mark_permission_denied(&mut self, diagnostic: ControllerDiagnostic) {
        self.permission = ControllerPermissionState::Denied;
        self.diagnostic_state = ControllerDiagnosticState::PermissionDenied;
        self.diagnostics.push(diagnostic);
    }

    fn mark_faulted(&mut self, diagnostic: ControllerDiagnostic) {
        self.diagnostic_state = ControllerDiagnosticState::Faulted;
        self.diagnostics.push(diagnostic);
    }

    fn summary(&self) -> ControllerSummary {
        ControllerSummary {
            id: self.info.id.0.clone(),
            name: self.name.clone(),
            model: family_label(self.info.family).to_string(),
            transport: self.transport.clone(),
            connected: self.state.connection == ConnectionState::Connected,
            connection_state: self.state.connection,
            battery_percent: battery_percent_for(&self.state),
            battery_state: self.state.battery_state,
            permission: self.permission,
            diagnostic_state: self.diagnostic_state,
        }
    }

    fn detail(&self) -> ControllerDetail {
        let summary = self.summary();
        ControllerDetail {
            id: summary.id,
            name: summary.name,
            model: summary.model,
            transport: summary.transport,
            connected: summary.connected,
            connection_state: summary.connection_state,
            battery_percent: summary.battery_percent,
            battery_state: summary.battery_state,
            permission: summary.permission,
            diagnostic_state: summary.diagnostic_state,
            vendor_id: self.info.vendor_id,
            product_id: self.info.product_id,
            capabilities: self.info.capabilities.clone(),
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn output_target(&self) -> Option<ControllerOutputTarget> {
        if self.state.connection != ConnectionState::Connected
            || self.permission != ControllerPermissionState::Granted
            || !self.info.capabilities.adaptive_triggers
        {
            return None;
        }

        Some(ControllerOutputTarget {
            raw_device_id: self.raw_device_id.clone()?,
            transport: device_transport_from_core(self.info.transport),
        })
    }

    fn health_check(&self) -> HealthCheck {
        let status = match self.diagnostic_state {
            ControllerDiagnosticState::Ok => "ok",
            ControllerDiagnosticState::Disconnected => "warning",
            ControllerDiagnosticState::PermissionDenied => "blocked",
            ControllerDiagnosticState::CannotOpen => "error",
            ControllerDiagnosticState::Unsupported => "warning",
            ControllerDiagnosticState::Faulted => "error",
            ControllerDiagnosticState::Unknown => "warning",
        };

        HealthCheck {
            name: format!("controller:{}", self.info.id.0),
            status: status.to_string(),
            detail: match self.diagnostic_state {
                ControllerDiagnosticState::Ok => {
                    format!("{} connected over {}", self.name, self.transport)
                }
                ControllerDiagnosticState::Disconnected => {
                    format!("{} is known but currently disconnected", self.name)
                }
                ControllerDiagnosticState::PermissionDenied => self
                    .diagnostics
                    .last()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| format!("Permission denied for {}", self.name)),
                ControllerDiagnosticState::CannotOpen => {
                    format!("{} is present but cannot be opened", self.name)
                }
                ControllerDiagnosticState::Unsupported => {
                    format!("{} is not a supported controller variant", self.name)
                }
                ControllerDiagnosticState::Faulted => self
                    .diagnostics
                    .last()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| format!("{} reported a transport fault", self.name)),
                ControllerDiagnosticState::Unknown => {
                    format!("{} has unknown controller status", self.name)
                }
            },
        }
    }

    fn matches_identity(&self, info: &ControllerInfo) -> bool {
        self.info.family == info.family
            && self.info.transport == info.transport
            && self.info.vendor_id == info.vendor_id
            && self.info.product_id == info.product_id
    }
}

fn battery_percent_for(state: &ControllerState) -> Option<u8> {
    state
        .battery_percent
        .map(|percent| percent.min(100))
        .or(match state.battery_state {
            BatteryState::Full => Some(100),
            _ => None,
        })
}

fn mock_device_manager() -> DeviceManager<MockTransport> {
    let transport = MockTransport::with_devices(vec![
        RawHidDevice::mock("mock://dualsense-primary")
            .with_family_hint(DeviceFamily::DualSense)
            .with_transport_hint(DeviceTransportKind::Usb)
            .with_battery(DeviceBatteryInfo::new(
                Some(88),
                DeviceBatteryState::Discharging,
            ))
            .with_product("Mock DualSense"),
        RawHidDevice::mock("mock://dualsense-edge-secondary")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Bluetooth)
            .with_battery(DeviceBatteryInfo::new(
                Some(62),
                DeviceBatteryState::Discharging,
            ))
            .with_product("Mock DualSense Edge"),
    ]);

    DeviceManager::with_default_config(transport)
}

fn controller_events_from_device_manager<T>(
    manager: &mut DeviceManager<T>,
) -> Result<Vec<ControllerDiscoveryEvent>, dscc_device::DeviceError>
where
    T: DeviceTransport,
{
    let device_events = manager.poll_once()?;
    let states = manager
        .registry()
        .entries()
        .map(|entry| (entry.info.id.as_str().to_string(), entry.state.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut events = device_events
        .into_iter()
        .map(|event| controller_event_from_device_event(event, &states))
        .collect::<Vec<_>>();

    if states.is_empty() {
        events.extend(windows_pnp_controller_events());
    }

    Ok(events)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct WindowsPnpControllerCache {
    events: Vec<ControllerDiscoveryEvent>,
    refreshed_at: Option<Instant>,
}

#[cfg(target_os = "windows")]
struct SetupDiInfoSet(windows_sys::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO);

#[cfg(target_os = "windows")]
impl Drop for SetupDiInfoSet {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Devices::DeviceAndDriverInstallation::SetupDiDestroyDeviceInfoList(
                self.0,
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    static CACHE: std::sync::OnceLock<Mutex<WindowsPnpControllerCache>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(WindowsPnpControllerCache::default()));
    let now = Instant::now();
    {
        let cache = match cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.refreshed_at.is_some_and(|refreshed_at| {
            now.duration_since(refreshed_at) < WINDOWS_PNP_CONTROLLER_CACHE_TTL
        }) {
            return cache.events.clone();
        }
    }

    let events = discover_windows_pnp_controller_events();
    let mut cache = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.events = events.clone();
    cache.refreshed_at = Some(Instant::now());
    events
}

#[cfg(target_os = "windows")]
fn discover_windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    let records = windows_setupapi_present_controller_records();
    if records.is_empty() {
        return Vec::new();
    }

    windows_pnp_controller_events_from_text(&records.join("\n"))
}

#[cfg(target_os = "windows")]
fn windows_setupapi_present_controller_records() -> Vec<String> {
    use windows_sys::Win32::{
        Devices::DeviceAndDriverInstallation::{
            SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
            SetupDiGetDeviceRegistryPropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO,
            SETUP_DI_REGISTRY_PROPERTY, SPDRP_DEVICEDESC, SPDRP_FRIENDLYNAME, SPDRP_HARDWAREID,
            SP_DEVINFO_DATA,
        },
        Foundation::{
            GetLastError, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE,
        },
    };

    fn registry_property_text(
        info_set: HDEVINFO,
        device_info: &SP_DEVINFO_DATA,
        property: SETUP_DI_REGISTRY_PROPERTY,
    ) -> Option<String> {
        let mut data_type = 0u32;
        let mut required_size = 0u32;
        let first_result = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                info_set,
                device_info,
                property,
                &mut data_type,
                std::ptr::null_mut(),
                0,
                &mut required_size,
            )
        };
        if first_result == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_INSUFFICIENT_BUFFER || required_size == 0 {
                return None;
            }
        }

        let mut buffer = vec![0u8; required_size as usize];
        let second_result = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                info_set,
                device_info,
                property,
                &mut data_type,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut required_size,
            )
        };
        if second_result == 0 {
            return None;
        }

        let valid_len = (required_size as usize).min(buffer.len());
        windows_utf16_bytes_to_search_text(&buffer[..valid_len])
    }

    fn instance_id_text(info_set: HDEVINFO, device_info: &SP_DEVINFO_DATA) -> Option<String> {
        let mut required_chars = 0u32;
        let first_result = unsafe {
            SetupDiGetDeviceInstanceIdW(
                info_set,
                device_info,
                std::ptr::null_mut(),
                0,
                &mut required_chars,
            )
        };
        if first_result == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_INSUFFICIENT_BUFFER || required_chars == 0 {
                return None;
            }
        }

        let mut buffer = vec![0u16; required_chars as usize];
        let second_result = unsafe {
            SetupDiGetDeviceInstanceIdW(
                info_set,
                device_info,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut required_chars,
            )
        };
        if second_result == 0 {
            return None;
        }

        windows_utf16_units_to_search_text(&buffer)
    }

    let info_set = unsafe {
        SetupDiGetClassDevsW(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        )
    };
    if info_set == INVALID_HANDLE_VALUE as HDEVINFO {
        return Vec::new();
    }
    let info_set = SetupDiInfoSet(info_set);

    let mut records = Vec::new();
    let mut index = 0u32;
    loop {
        let mut device_info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        let enum_result = unsafe { SetupDiEnumDeviceInfo(info_set.0, index, &mut device_info) };
        if enum_result == 0 {
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                break;
            }
            index += 1;
            continue;
        }

        let mut fields = Vec::new();
        for property in [SPDRP_FRIENDLYNAME, SPDRP_DEVICEDESC, SPDRP_HARDWAREID] {
            if let Some(value) = registry_property_text(info_set.0, &device_info, property) {
                fields.push(value);
            }
        }
        if let Some(value) = instance_id_text(info_set.0, &device_info) {
            fields.push(value);
        }

        let record = fields.join("\t");
        if windows_pnp_candidate_text_is_controller(&record) {
            records.push(record);
        }
        index += 1;
    }
    records
}

#[cfg(target_os = "windows")]
fn windows_utf16_bytes_to_search_text(bytes: &[u8]) -> Option<String> {
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    windows_utf16_units_to_search_text(&units)
}

#[cfg(target_os = "windows")]
fn windows_utf16_units_to_search_text(units: &[u16]) -> Option<String> {
    let parts = units
        .split(|unit| *unit == 0)
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let text = String::from_utf16_lossy(part);
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(target_os = "windows")]
fn windows_pnp_candidate_text_is_controller(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("dualsense")
        || lower.contains("wireless controller")
        || lower.contains("playstation")
        || lower.contains("vid_054c")
        || lower.contains("vid&054c")
        || lower.contains("pid_0df2")
        || lower.contains("pid&0df2")
        || lower.contains("pid_0ce6")
        || lower.contains("pid&0ce6")
}

#[cfg(not(target_os = "windows"))]
fn windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn windows_pnp_controller_events_from_text(text: &str) -> Vec<ControllerDiscoveryEvent> {
    let mut found_edge = false;
    let mut found_explicit_standard = false;
    let mut found_generic_standard = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("dualsense edge")
            || lower.contains("pid&0df2")
            || lower.contains("pid_0df2")
        {
            found_edge = true;
            continue;
        }
        if lower.contains("pid&0ce6") || lower.contains("pid_0ce6") {
            found_explicit_standard = true;
            continue;
        }
        if lower.contains("dualsense") || lower.contains("wireless controller") {
            found_generic_standard = true;
        }
    }

    let found_standard = found_explicit_standard || (!found_edge && found_generic_standard);
    let mut events = Vec::new();
    if found_edge {
        events.push(windows_pnp_controller_event(
            "windows-pnp-dualsense-edge",
            "DualSense Edge",
            ControllerFamily::DualSenseEdge,
            0x0df2,
        ));
    }
    if found_standard {
        events.push(windows_pnp_controller_event(
            "windows-pnp-dualsense",
            "DualSense",
            ControllerFamily::DualSense,
            0x0ce6,
        ));
    }
    events
}

#[cfg(target_os = "windows")]
fn windows_pnp_controller_event(
    id: &str,
    name: &str,
    family: ControllerFamily,
    product_id: u16,
) -> ControllerDiscoveryEvent {
    let info = ControllerInfo {
        id: ControllerId(id.to_string()),
        vendor_id: 0x054c,
        product_id,
        family,
        transport: ControllerTransportKind::Bluetooth,
        connection: ConnectionState::Connected,
        capabilities: ControllerCapabilities {
            adaptive_triggers: true,
            lightbar: true,
            player_leds: true,
            rumble: true,
            microphone_led: true,
            edge_buttons: family == ControllerFamily::DualSenseEdge,
        },
    };
    let state = ControllerState {
        id: info.id.clone(),
        connection: ConnectionState::Connected,
        battery_percent: None,
        battery_state: BatteryState::Unknown,
    };

    ControllerDiscoveryEvent::Attached(
        DiscoveredController::new(info, state)
            .with_name(name)
            .with_transport_label("bluetooth")
            .with_diagnostic(ControllerDiagnostic::warning(
                "windows_pnp_fallback",
                "Controller is present in Windows PnP, but hidapi did not expose an open HID handle; configuration is available and hardware output is disabled.",
            )),
    )
}

fn is_windows_pnp_controller_id(id: &str) -> bool {
    id.starts_with("windows-pnp-")
}

fn controller_event_from_device_event(
    event: DeviceEvent,
    states: &BTreeMap<String, DeviceControllerState>,
) -> ControllerDiscoveryEvent {
    match event {
        DeviceEvent::Attached(info) => {
            let raw_device_id = info.raw_device_id.clone();
            let state = states
                .get(info.id.as_str())
                .cloned()
                .unwrap_or_else(|| connected_state_for(&info));
            let controller_info = core_controller_info(&info, state.connection);
            let controller_state = core_controller_state(&state);
            ControllerDiscoveryEvent::Attached(
                DiscoveredController::new(controller_info, controller_state)
                    .with_raw_device_id(raw_device_id)
                    .with_transport_label(device_transport_label(info.transport))
                    .with_diagnostic(ControllerDiagnostic::info(
                        "device_backend",
                        "Controller discovered through dscc-device",
                    )),
            )
        }
        DeviceEvent::Detached(id) => ControllerDiscoveryEvent::Detached(core_controller_id(id)),
        DeviceEvent::StatusChanged(state) => {
            ControllerDiscoveryEvent::StatusChanged(core_controller_state(&state))
        }
        DeviceEvent::PermissionDenied(path_hint) => ControllerDiscoveryEvent::PermissionDenied(
            DevicePermissionProblem::global(permission_denied_message(&path_hint)),
        ),
        DeviceEvent::Faulted { id, message } => ControllerDiscoveryEvent::Faulted {
            id: id.map(core_controller_id),
            message,
        },
    }
}

fn connected_state_for(info: &DeviceControllerInfo) -> DeviceControllerState {
    DeviceControllerState {
        id: info.id.clone(),
        connection: DeviceConnectionState::Connected,
        battery: DeviceBatteryInfo::UNKNOWN,
    }
}

fn core_controller_id(id: DeviceControllerId) -> ControllerId {
    ControllerId(id.as_str().to_string())
}

fn core_controller_info(
    info: &DeviceControllerInfo,
    connection: DeviceConnectionState,
) -> ControllerInfo {
    ControllerInfo {
        id: ControllerId(info.id.as_str().to_string()),
        vendor_id: info.vendor_id.unwrap_or(0),
        product_id: info.product_id.unwrap_or(0),
        family: core_family(info.family),
        transport: core_transport(info.transport),
        connection: core_connection(connection),
        capabilities: core_capabilities(&info.capabilities),
    }
}

fn core_controller_state(state: &DeviceControllerState) -> ControllerState {
    ControllerState {
        id: ControllerId(state.id.as_str().to_string()),
        connection: core_connection(state.connection),
        battery_percent: state.battery.percent,
        battery_state: core_battery_state(state.battery.state),
    }
}

fn core_capabilities(capabilities: &DeviceControllerCapabilities) -> ControllerCapabilities {
    ControllerCapabilities {
        adaptive_triggers: capabilities.adaptive_triggers,
        lightbar: capabilities.lightbar,
        player_leds: capabilities.player_leds,
        rumble: capabilities.rumble,
        microphone_led: capabilities.microphone_led,
        edge_buttons: capabilities.edge_buttons,
    }
}

fn core_family(family: DeviceFamily) -> ControllerFamily {
    match family {
        DeviceFamily::DualSense => ControllerFamily::DualSense,
        DeviceFamily::DualSenseEdge => ControllerFamily::DualSenseEdge,
        DeviceFamily::UnknownSony | DeviceFamily::Unknown => ControllerFamily::UnknownSony,
    }
}

fn core_transport(transport: DeviceTransportKind) -> ControllerTransportKind {
    match transport {
        DeviceTransportKind::Usb => ControllerTransportKind::Usb,
        DeviceTransportKind::Bluetooth => ControllerTransportKind::Bluetooth,
        DeviceTransportKind::Unknown => ControllerTransportKind::Unknown,
    }
}

fn device_transport_from_core(transport: ControllerTransportKind) -> DeviceTransportKind {
    match transport {
        ControllerTransportKind::Usb => DeviceTransportKind::Usb,
        ControllerTransportKind::Bluetooth => DeviceTransportKind::Bluetooth,
        ControllerTransportKind::Unknown => DeviceTransportKind::Unknown,
    }
}

fn core_connection(connection: DeviceConnectionState) -> ConnectionState {
    match connection {
        DeviceConnectionState::Connected => ConnectionState::Connected,
        DeviceConnectionState::Disconnected => ConnectionState::Disconnected,
        DeviceConnectionState::Unknown => ConnectionState::Unknown,
    }
}

fn core_battery_state(state: DeviceBatteryState) -> BatteryState {
    match state {
        DeviceBatteryState::Unknown => BatteryState::Unknown,
        DeviceBatteryState::Discharging => BatteryState::Discharging,
        DeviceBatteryState::Charging => BatteryState::Charging,
        DeviceBatteryState::Full => BatteryState::Full,
    }
}

fn device_transport_label(transport: DeviceTransportKind) -> &'static str {
    match transport {
        DeviceTransportKind::Usb => "usb",
        DeviceTransportKind::Bluetooth => "bluetooth",
        DeviceTransportKind::Unknown => "unknown",
    }
}

fn permission_denied_message(path_hint: &DevicePathHint) -> String {
    format!("Permission denied while opening controller candidate at {path_hint}")
}

fn family_label(family: ControllerFamily) -> &'static str {
    match family {
        ControllerFamily::DualSense => "DualSense",
        ControllerFamily::DualSenseEdge => "DualSense Edge",
        ControllerFamily::UnknownSony => "Unknown Sony Controller",
    }
}

fn transport_label(transport: ControllerTransportKind) -> &'static str {
    match transport {
        ControllerTransportKind::Usb => "usb",
        ControllerTransportKind::Bluetooth => "bluetooth",
        ControllerTransportKind::Unknown => "unknown",
    }
}

fn diagnostic_state_for(
    permission: ControllerPermissionState,
    connection: ConnectionState,
) -> ControllerDiagnosticState {
    if permission == ControllerPermissionState::Denied {
        return ControllerDiagnosticState::PermissionDenied;
    }

    match connection {
        ConnectionState::Connected => ControllerDiagnosticState::Ok,
        ConnectionState::Disconnected => ControllerDiagnosticState::Disconnected,
        ConnectionState::Unknown => ControllerDiagnosticState::Unknown,
    }
}

fn severity_status(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    }
}

fn default_adapters() -> Vec<AdapterSummary> {
    built_in_adapters()
        .iter()
        .map(|adapter| {
            let enabled = adapter.enabled_by_default;
            AdapterSummary {
                id: adapter.id.to_string(),
                name: adapter.display_name.to_string(),
                enabled,
                state: adapter_state_label(&initial_detection(adapter, enabled)).to_string(),
                packet_rate_hz: None,
                protocol: format!("{:?}", adapter.protocol).to_ascii_lowercase(),
                setup_hint: adapter.setup_hint.to_string(),
                setup_url: adapter.setup_url.map(str::to_string),
            }
        })
        .collect()
}

fn set_adapter_running(adapters: &mut [AdapterSummary], adapter_id: &str, running: bool) {
    if let Some(adapter) = adapters.iter_mut().find(|adapter| adapter.id == adapter_id) {
        if running && !adapter.enabled {
            adapter.enabled = true;
        }
        let state = if running {
            "connected"
        } else if adapter.enabled {
            "ready"
        } else {
            "disabled"
        };
        if adapter.state != state {
            adapter.state = state.to_string();
        }
        let packet_rate_hz = running.then_some(60);
        if adapter.packet_rate_hz != packet_rate_hz {
            adapter.packet_rate_hz = packet_rate_hz;
        }
    }
}

fn materialized_adapters(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> Vec<AdapterSummary> {
    let now = Instant::now();
    let mut adapters = inner.adapters.clone();
    for adapter in &mut adapters {
        if let Some(runtime) = inner.adapter_runtime(&adapter.id) {
            apply_adapter_runtime_summary(adapter, runtime, game_detection, now);
        }
    }
    adapters
}

fn apply_adapter_runtime_summary(
    adapter: &mut AdapterSummary,
    runtime: &AdapterRuntime,
    game_detection: Option<&GameDetectionResponse>,
    now: Instant,
) {
    let bind_addr = runtime
        .bind_addr
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| default_adapter_bind_addr(runtime));
    let detected_game = detected_adapter_game_name(game_detection, &runtime.adapter_id);

    if !runtime.listener_bound {
        if let Some(error) = runtime.last_error.as_ref() {
            adapter.enabled = true;
            adapter.state = "faulted".to_string();
            adapter.packet_rate_hz = None;
            adapter.setup_hint = if runtime.protocol == AdapterProtocol::SharedMemory {
                format!(
                    "DSCC could not start the {} reader: {error}",
                    runtime.display_name
                )
            } else {
                format!(
                    "DSCC could not bind the {} UDP listener on {bind_addr}: {error}",
                    runtime.display_name
                )
            };
        }
        return;
    }

    adapter.enabled = true;
    if runtime.has_recent_packet(now) {
        adapter.state = "connected".to_string();
        adapter.packet_rate_hz = runtime.packet_rate_hz;
        let packet_len = runtime.last_packet_len.unwrap_or_default();
        adapter.setup_hint = format!(
            "Receiving {} via {}; last packet was {packet_len} bytes {}.",
            runtime.display_name,
            runtime_transport_label(runtime, &bind_addr),
            runtime
                .last_packet_at
                .map(|last| format_elapsed_brief(now.duration_since(last)))
                .unwrap_or_else(|| "just now".to_string())
        );
        return;
    }

    adapter.packet_rate_hz = Some(0);
    if runtime.packet_count > 0 {
        adapter.state = "needs_setup".to_string();
        adapter.setup_hint = format!(
            "{} is ready via {}, but the stream is stale; last packet arrived {}.",
            runtime.display_name,
            runtime_transport_label(runtime, &bind_addr),
            runtime
                .last_packet_at
                .map(|last| format_elapsed_brief(now.duration_since(last)))
                .unwrap_or_else(|| "earlier".to_string())
        );
    } else if runtime.adapter_id == FORZA_DATA_OUT_ADAPTER_ID {
        if let Some(game_name) = detected_game {
            adapter.state = "needs_setup".to_string();
            adapter.setup_hint = format!(
                "{game_name} is running and DSCC is listening on {bind_addr}, but no Data Out packets have arrived. Enable UDP Race Telemetry in-game, set target IP to 127.0.0.1, use port 5300, then enter a driving session."
            );
        } else {
            adapter.state = "ready".to_string();
            adapter.setup_hint = format!(
                "DSCC is listening on {bind_addr}; launch a supported Forza title and enable UDP Race Telemetry."
            );
        }
    } else if runtime.adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
        if let Some(game_name) = detected_game {
            adapter.state = "needs_setup".to_string();
            adapter.setup_hint = format!(
                "{game_name} is running and DSCC is watching Assetto shared memory, but no live physics page is available yet. Load into a driving session and make sure the game is not paused."
            );
        } else {
            adapter.state = "ready".to_string();
            adapter.setup_hint = "DSCC is watching Assetto shared memory; launch Assetto Corsa Rally and enter a driving session.".to_string();
        }
    } else {
        adapter.state = "needs_setup".to_string();
        adapter.setup_hint = format!(
            "DSCC is listening on {bind_addr}; configure {} to send UDP telemetry to this adapter.",
            runtime.display_name
        );
    }
}

fn default_adapter_bind_addr(runtime: &AdapterRuntime) -> String {
    match runtime.default_port {
        Some(port) => format!("127.0.0.1:{port}"),
        None if runtime.protocol == AdapterProtocol::SharedMemory => "shared-memory".to_string(),
        None => "127.0.0.1".to_string(),
    }
}

fn runtime_transport_label<'a>(runtime: &AdapterRuntime, bind_addr: &'a str) -> &'a str {
    match runtime.protocol {
        AdapterProtocol::SharedMemory => "shared memory",
        _ => bind_addr,
    }
}

fn detected_adapter_game_name<'a>(
    game_detection: Option<&'a GameDetectionResponse>,
    adapter_id: &str,
) -> Option<&'a str> {
    game_detection.and_then(|detection| {
        (detection.adapter_id.as_deref() == Some(adapter_id))
            .then_some(detection.active_game_name.as_deref())
            .flatten()
    })
}

fn format_elapsed_brief(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds == 0 {
        "just now".to_string()
    } else if seconds < 60 {
        format!("{seconds}s ago")
    } else {
        format!("{}m {}s ago", seconds / 60, seconds % 60)
    }
}

fn adapter_runtime_health_check(
    runtime: &AdapterRuntime,
    game_detection: Option<&GameDetectionResponse>,
) -> HealthCheck {
    let now = Instant::now();
    let bind_addr = runtime
        .bind_addr
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| default_adapter_bind_addr(runtime));

    if !runtime.listener_bound {
        return HealthCheck {
            name: runtime.adapter_id.clone(),
            status: if runtime.last_error.is_some() {
                "blocked".to_string()
            } else {
                "pending".to_string()
            },
            detail: runtime.last_error.clone().unwrap_or_else(|| {
                format!(
                    "{} listener has not reported ready on {bind_addr}",
                    runtime.display_name
                )
            }),
        };
    }

    if runtime.has_recent_packet(now) {
        return HealthCheck {
            name: runtime.adapter_id.clone(),
            status: "ok".to_string(),
            detail: format!(
                "Receiving {} byte packets on {bind_addr} at {} Hz",
                runtime.last_packet_len.unwrap_or_default(),
                runtime.packet_rate_hz.unwrap_or_default()
            ),
        };
    }

    let detected_game = detected_adapter_game_name(game_detection, &runtime.adapter_id);
    let status = if detected_game.is_some() {
        "warning"
    } else {
        "ok"
    };
    let mut detail = if let Some(game_name) = detected_game {
        if runtime.protocol == AdapterProtocol::SharedMemory {
            format!(
                "{game_name} is running; shared-memory reader is ready, but no live physics page is available"
            )
        } else {
            format!(
                "{game_name} is running; listener is ready on {bind_addr}, but no live Data Out packets are arriving"
            )
        }
    } else if runtime.adapter_id == FORZA_DATA_OUT_ADAPTER_ID {
        format!(
            "Listener is ready on {bind_addr}; telemetry will activate when a supported Forza title is running"
        )
    } else if runtime.adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
        "Shared-memory reader is ready; telemetry will activate when Assetto Corsa Rally is running in a driving session".to_string()
    } else {
        format!(
            "Listener is ready on {bind_addr}; telemetry will activate when a supported source sends packets"
        )
    };
    if let Some(last_packet_at) = runtime.last_packet_at {
        detail = format!(
            "{detail}; last valid packet arrived {}",
            format_elapsed_brief(now.duration_since(last_packet_at))
        );
    }
    if let Some(last_parse_error) = runtime.last_parse_error.as_ref() {
        detail = format!("{detail}; {last_parse_error}");
    }

    HealthCheck {
        name: runtime.adapter_id.clone(),
        status: status.to_string(),
        detail,
    }
}

fn adapter_state_label(detection: &AdapterDetection) -> &'static str {
    match detection {
        AdapterDetection::Unavailable { .. } => "disabled",
        AdapterDetection::NeedsSetup { .. } => "needs_setup",
        AdapterDetection::Ready => "ready",
        AdapterDetection::Running => "connected",
        AdapterDetection::Faulted { .. } => "faulted",
    }
}

fn module_summaries() -> Vec<ModuleSummary> {
    let mut summaries: Vec<ModuleSummary> = built_in_adapters()
        .iter()
        .map(|adapter| ModuleSummary {
            id: adapter.id.to_string(),
            name: adapter.display_name.to_string(),
            kind: "adapter".to_string(),
            version: "builtin".to_string(),
            source: "built_in".to_string(),
            trusted: true,
            protocol: format!("{:?}", adapter.protocol).to_ascii_lowercase(),
            setup_hint: adapter.setup_hint.to_string(),
            setup_url: adapter.setup_url.map(str::to_string),
            profile_templates: Vec::new(),
        })
        .collect();
    summaries.extend(game_module_summaries());
    summaries
}

#[cfg(test)]
async fn detect_running_game(
    _user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    no_game_detection("none")
}

#[cfg(not(test))]
async fn detect_running_game(
    user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    if let Ok(fixture) = std::env::var("DSCC_PROCESS_SCAN_FIXTURE") {
        return detect_running_game_from_processes_with_user_games(
            fixture
                .split(';')
                .map(str::trim)
                .filter(|process| !process.is_empty()),
            user_games,
        );
    }

    if std::env::var_os("DSCC_DISABLE_PROCESS_SCAN").is_some() {
        return no_game_detection("process_scan_disabled");
    }

    match current_process_names().await {
        Ok(processes) => detect_running_game_from_processes_with_user_games(
            processes.iter().map(String::as_str),
            user_games,
        ),
        Err(error) => GameDetectionResponse {
            active_game_id: None,
            active_game_name: None,
            source: "process_scan_unavailable".to_string(),
            confidence: 0,
            process_name: None,
            module_id: None,
            adapter_id: None,
            profile_id: None,
            candidates: Vec::new(),
            supported_games: Vec::new(),
            selected_game: None,
        }
        .with_source_detail(error.to_string()),
    }
}

#[cfg(not(test))]
trait GameDetectionSourceDetail {
    fn with_source_detail(self, detail: String) -> Self;
}

#[cfg(not(test))]
impl GameDetectionSourceDetail for GameDetectionResponse {
    fn with_source_detail(mut self, detail: String) -> Self {
        self.source = format!("{}:{detail}", self.source);
        self
    }
}

#[cfg(not(test))]
async fn current_process_names() -> io::Result<Vec<String>> {
    #[cfg(target_os = "windows")]
    {
        windows_process_names()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = tokio::process::Command::new("ps")
            .args(["-eo", "comm="])
            .output()
            .await?;
        if !output.status.success() {
            return Err(io::Error::other("ps did not complete successfully"));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect())
    }
}

fn profile_resolution(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> ProfileResolutionResponse {
    let controller_id = inner
        .controllers
        .summaries()
        .into_iter()
        .find(|controller| controller.connected)
        .map(|controller| controller.id);
    let detected_game_id = game_detection.and_then(|detection| detection.active_game_id.clone());
    let detected_adapter_id = game_detection.and_then(|detection| {
        detection
            .active_game_id
            .as_ref()
            .and_then(|_| detection.adapter_id.clone())
    });
    let active_adapter_id = detected_adapter_id
        .clone()
        .or_else(|| inner.active_adapter_id.clone())
        .or_else(|| inner.telemetry.text("source.id").map(str::to_string));
    let override_key = profile_override_key(controller_id.as_deref(), detected_game_id.as_deref());
    let fallback_override_key = profile_override_key(None, detected_game_id.as_deref());
    let controller_global_override_key = profile_override_key(controller_id.as_deref(), None);
    let global_override_key = profile_override_key(None, None);
    let override_profile = inner
        .profile_overrides
        .get(&override_key)
        .or_else(|| inner.profile_overrides.get(&fallback_override_key))
        .or_else(|| inner.profile_overrides.get(&controller_global_override_key))
        .or_else(|| inner.profile_overrides.get(&global_override_key));

    let assigned_profile_id = controller_id.as_deref().and_then(|id| {
        if let Some(config) = inner.controller_configs.get(id) {
            assigned_profile_for(config, detected_game_id.as_deref())
        } else {
            inner.controllers.detail(id).and_then(|detail| {
                let config = ControllerConfig::default_for(id, detail.model);
                assigned_profile_for(&config, detected_game_id.as_deref())
            })
        }
    });
    let module_profile_id = game_detection.and_then(|detection| detection.profile_id.clone());
    let selected_profile_id = override_profile
        .map(|profile| profile.profile_id.clone())
        .or_else(|| assigned_profile_id.clone())
        .or_else(|| module_profile_id.clone())
        .or_else(|| inner.active_profile_id.clone());
    let validation = if selected_profile_id
        .as_deref()
        .is_some_and(|id| inner.profiles.iter().any(|profile| profile.id == id))
    {
        "valid"
    } else {
        "missing_profile"
    };

    ProfileResolutionResponse {
        controller_id,
        detected_game_id,
        active_adapter_id,
        selected_profile_id,
        reason: if override_profile.is_some() {
            "manual_override".to_string()
        } else if game_detection.is_some_and(|detection| detection.profile_id.is_some()) {
            "foreground_game".to_string()
        } else if assigned_profile_id.is_some() {
            "telemetry_source".to_string()
        } else if module_profile_id.is_some() {
            "module_template".to_string()
        } else if inner.active_adapter_id.is_some() {
            "active_telemetry_source".to_string()
        } else {
            "global_default".to_string()
        },
        override_profile_id: override_profile.map(|profile| profile.profile_id.clone()),
        validation: validation.to_string(),
    }
}

fn assigned_profile_for(config: &ControllerConfig, game_id: Option<&str>) -> Option<String> {
    let game_id = game_id?;
    config
        .profile_assignments
        .iter()
        .find(|assignment| profile_assignment_matches(&assignment.game_id, game_id))
        .map(|assignment| assignment.profile_id.clone())
}

fn profile_assignment_matches(assignment_game_id: &str, detected_game_id: &str) -> bool {
    assignment_game_id == detected_game_id
}

fn update_number(updates: &[SignalUpdate], name: &str) -> Option<f64> {
    updates
        .iter()
        .find(|update| update.name.as_str() == name)
        .and_then(|update| update.value.as_number())
}

fn update_text<'a>(updates: &'a [SignalUpdate], name: &str) -> Option<&'a str> {
    updates
        .iter()
        .find(|update| update.name.as_str() == name)
        .and_then(|update| update.value.as_text())
}

fn racing_shift_adapter(adapter_id: &str) -> bool {
    matches!(
        adapter_id,
        FORZA_DATA_OUT_ADAPTER_ID | ASSETTO_SHARED_MEMORY_ADAPTER_ID
    )
}

#[derive(Debug, Clone, Copy)]
struct RacingEffectToggles {
    shift_thump: bool,
    suspension_impact: bool,
}

fn racing_effect_toggles(inner: &AgentStateInner) -> RacingEffectToggles {
    let mut toggles = RacingEffectToggles {
        shift_thump: false,
        suspension_impact: false,
    };
    let mut saw_connected = false;
    for controller in inner
        .controllers
        .summaries()
        .into_iter()
        .filter(|controller| controller.connected)
    {
        saw_connected = true;
        let default_config;
        let config = match inner.controller_configs.get(&controller.id) {
            Some(config) => config,
            None => {
                default_config = ControllerConfig::default_for(&controller.id, controller.model);
                &default_config
            }
        };
        toggles.shift_thump |= forza_effect_enabled(config, "gear_shift_thump");
        toggles.suspension_impact |= forza_effect_enabled(config, "suspension_impact");
        if toggles.shift_thump && toggles.suspension_impact {
            break;
        }
    }
    if !saw_connected {
        return RacingEffectToggles {
            shift_thump: true,
            suspension_impact: true,
        };
    }
    toggles
}

fn forza_effect_enabled(config: &ControllerConfig, effect_id: &str) -> bool {
    let default = default_forza_effect(effect_id);
    config
        .forza
        .effects
        .iter()
        .find(|effect| effect.id == effect_id)
        .cloned()
        .unwrap_or_else(|| default.clone())
        .normalized_with_default(&default)
        .scalar()
        > 0.0
}

fn telemetry_response(snapshot: &SignalSnapshot) -> Vec<TelemetrySignalResponse> {
    snapshot
        .signals()
        .iter()
        .map(|(name, value)| TelemetrySignalResponse {
            name: name.as_str().to_string(),
            value: signal_value_json(value),
            unit: signal_unit(name.as_str()).map(str::to_string),
            updated_ms_ago: 0,
        })
        .collect()
}

fn materialized_telemetry_response(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> Vec<TelemetrySignalResponse> {
    let now = Instant::now();
    if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
        let source_id = inner.telemetry.text("source.id");
        let Some(runtime) = inner.adapter_runtime(adapter_id) else {
            return telemetry_response(&inner.telemetry);
        };
        if source_id != Some(adapter_id) || !runtime.has_recent_packet(now) {
            return waiting_telemetry_response(runtime, adapter_id, game_id, game_name, now);
        }
        let mut response = telemetry_response(&inner.telemetry);
        upsert_telemetry_signal(&mut response, telemetry_signal("game.id", game_id, None, 0));
        upsert_telemetry_signal(
            &mut response,
            telemetry_signal("game.name", game_name, None, 0),
        );
        return response;
    }

    telemetry_response(&inner.telemetry)
}

fn detected_telemetry_game(
    game_detection: Option<&GameDetectionResponse>,
) -> Option<(&str, &str, &str)> {
    let detection = game_detection?;
    let adapter_id = detection.adapter_id.as_deref()?;
    let game_id = detection.active_game_id.as_deref()?;
    Some((
        adapter_id,
        game_id,
        detection.active_game_name.as_deref().unwrap_or(game_id),
    ))
}

fn hardware_output_runtime_allowed_for_resolution(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    let Some(detection) = game_detection else {
        return false;
    };
    if detection.active_game_id.is_none() {
        return false;
    }
    let Some(adapter_id) = detection.adapter_id.as_deref() else {
        return false;
    };
    if resolution.controller_id.is_none()
        || resolution.selected_profile_id.is_none()
        || resolution.validation != "valid"
    {
        return false;
    }
    let Some(runtime) = inner.adapter_runtime(adapter_id) else {
        return false;
    };
    runtime.has_recent_packet(Instant::now())
        && inner.telemetry.text("source.id") == Some(adapter_id)
}

fn hardware_output_detection_lightbar_allowed_for_resolution(
    _inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    let Some(detection) = game_detection else {
        return false;
    };
    if detection.active_game_id.is_none()
        || detection.adapter_id.is_none()
        || detection.profile_id.is_none()
    {
        return false;
    }
    resolution.controller_id.is_some()
        && resolution.selected_profile_id.is_some()
        && resolution.validation == "valid"
        && detection_game_module(detection).is_some()
}

fn hardware_output_global_lightbar_allowed_for_resolution(
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    if game_detection.is_some_and(|detection| detection.profile_id.is_some()) {
        return false;
    }

    resolution.controller_id.is_some() && resolution.validation == "valid"
}

fn hardware_output_any_allowed(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> bool {
    let resolution = profile_resolution(inner, game_detection);
    hardware_output_runtime_allowed_for_resolution(inner, game_detection, &resolution)
        || hardware_output_detection_lightbar_allowed_for_resolution(
            inner,
            game_detection,
            &resolution,
        )
        || (hardware_output_global_lightbar_allowed_for_resolution(game_detection, &resolution)
            && global_lightbar_output(inner, &resolution).is_some())
}

fn detection_game_module(detection: &GameDetectionResponse) -> Option<&'static GameModule> {
    let module_id = detection.module_id.as_deref()?;
    built_in_game_modules()
        .iter()
        .find(|game| game.id == module_id)
}

fn detection_lightbar_output(detection: &GameDetectionResponse) -> Option<LightbarOutput> {
    let game = detection_game_module(detection)?;
    let color = rgb_from_hex(game.detection_lightbar_color)?;
    Some(LightbarOutput {
        color,
        brightness: clamp_unit(f64::from(game.detection_lightbar_brightness.min(100)) / 100.0),
    })
}

fn global_lightbar_output(
    inner: &AgentStateInner,
    resolution: &ProfileResolutionResponse,
) -> Option<ControllerOutputFrame> {
    let config = controller_config_for_resolution(inner, resolution)?;
    let lightbar = config.lightbar.normalized();
    let lightbar = lightbar.enabled.then(|| LightbarOutput {
        color: lightbar.rgb(),
        brightness: clamp_unit(f64::from(lightbar.brightness) / 100.0),
    });
    Some(ControllerOutputFrame {
        lightbar,
        ..ControllerOutputFrame::default()
    })
}

fn upsert_telemetry_signal(
    signals: &mut Vec<TelemetrySignalResponse>,
    signal: TelemetrySignalResponse,
) {
    if let Some(existing) = signals.iter_mut().find(|item| item.name == signal.name) {
        *existing = signal;
    } else {
        signals.push(signal);
    }
}

fn waiting_telemetry_response(
    runtime: &AdapterRuntime,
    adapter_id: &str,
    game_id: &str,
    game_name: &str,
    now: Instant,
) -> Vec<TelemetrySignalResponse> {
    let age_ms = runtime
        .last_packet_at
        .map(|last| {
            now.duration_since(last)
                .as_millis()
                .min(u128::from(u64::MAX)) as u64
        })
        .unwrap_or_default();
    vec![
        telemetry_signal("source.id", adapter_id, None, 0),
        telemetry_signal("source.connected", runtime.has_recent_packet(now), None, 0),
        telemetry_signal(
            "source.packet_rate_hz",
            if runtime.has_recent_packet(now) {
                f64::from(runtime.packet_rate_hz.unwrap_or_default())
            } else {
                0.0
            },
            Some("Hz"),
            age_ms,
        ),
        telemetry_signal(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
            Some("bytes"),
            age_ms,
        ),
        telemetry_signal("game.id", game_id, None, 0),
        telemetry_signal("game.name", game_name, None, 0),
        telemetry_signal(
            "game.state",
            if runtime.packet_count > 0 {
                "telemetry_stale"
            } else if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "awaiting_shared_memory"
            } else {
                "awaiting_data_out"
            },
            None,
            age_ms,
        ),
        telemetry_signal("input.throttle", 0.0, None, age_ms),
        telemetry_signal("input.brake", 0.0, None, age_ms),
        telemetry_signal("input.handbrake", 0.0, None, age_ms),
        telemetry_signal("vehicle.rpm_ratio", 0.0, None, age_ms),
        telemetry_signal("vehicle.speed_kmh", 0.0, Some("km/h"), age_ms),
        telemetry_signal("wheel.slip.max", 0.0, None, age_ms),
        telemetry_signal("wheel.slip.front_max", 0.0, None, age_ms),
        telemetry_signal("wheel.slip.rear_max", 0.0, None, age_ms),
        telemetry_signal("tire.slip_ratio.max", 0.0, None, age_ms),
        telemetry_signal("surface.rumble.max", 0.0, None, age_ms),
        telemetry_signal("surface.rumble_strip.max", 0.0, None, age_ms),
        telemetry_signal("surface.puddle.max", 0.0, None, age_ms),
        telemetry_signal("suspension.travel.max", 0.0, None, age_ms),
        telemetry_signal("suspension.impact_pulse", 0.0, None, age_ms),
        telemetry_signal("vehicle.acceleration.magnitude", 0.0, Some("m/s^2"), age_ms),
        telemetry_signal("drivetrain.shift_event", "none", None, age_ms),
        telemetry_signal("drivetrain.shift_pulse", 0.0, None, age_ms),
    ]
}

fn waiting_signal_snapshot(
    runtime: &AdapterRuntime,
    adapter_id: &str,
    game_id: &str,
    game_name: &str,
    now: Instant,
) -> SignalSnapshot {
    SignalSnapshot::from_updates([
        signal_update("source.id", adapter_id),
        signal_update("source.connected", runtime.has_recent_packet(now)),
        signal_update("source.packet_rate_hz", 0.0),
        signal_update(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
        ),
        signal_update("game.id", game_id),
        signal_update("game.name", game_name),
        signal_update(
            "game.state",
            if runtime.packet_count > 0 {
                "telemetry_stale"
            } else if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "awaiting_shared_memory"
            } else {
                "awaiting_data_out"
            },
        ),
        signal_update("input.throttle", 0.0),
        signal_update("input.brake", 0.0),
        signal_update("input.handbrake", 0.0),
        signal_update("vehicle.rpm_ratio", 0.0),
        signal_update("vehicle.speed_kmh", 0.0),
        signal_update("wheel.slip.max", 0.0),
        signal_update("wheel.slip.front_max", 0.0),
        signal_update("wheel.slip.rear_max", 0.0),
        signal_update("tire.slip_ratio.max", 0.0),
        signal_update("surface.rumble.max", 0.0),
        signal_update("surface.rumble_strip.max", 0.0),
        signal_update("surface.puddle.max", 0.0),
        signal_update("suspension.travel.max", 0.0),
        signal_update("suspension.impact_pulse", 0.0),
        signal_update("vehicle.acceleration.magnitude", 0.0),
        signal_update("drivetrain.shift_event", "none"),
        signal_update("drivetrain.shift_pulse", 0.0),
    ])
}

fn forza_inactive_signal_snapshot(
    runtime: &AdapterRuntime,
    now: Instant,
    game_id: Option<&str>,
    game_name: Option<&str>,
) -> SignalSnapshot {
    let mut updates = vec![
        signal_update("source.id", "none"),
        signal_update("source.connected", false),
        signal_update("source.packet_rate_hz", 0.0),
        signal_update(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
        ),
        signal_update(
            "source.packet_age_ms",
            runtime
                .last_packet_at
                .map(|last| {
                    now.duration_since(last)
                        .as_millis()
                        .min(u128::from(u64::MAX)) as f64
                })
                .unwrap_or_default(),
        ),
    ];
    if let Some(game_id) = game_id {
        updates.push(signal_update("game.id", game_id));
    }
    if let Some(game_name) = game_name {
        updates.push(signal_update("game.name", game_name));
    }
    SignalSnapshot::from_updates(updates)
}

fn current_effect_snapshot(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> (SignalSnapshot, bool) {
    let now = Instant::now();
    if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
        let source_id = inner.telemetry.text("source.id");
        let Some(runtime) = inner.adapter_runtime(adapter_id) else {
            return (inner.telemetry.clone(), false);
        };
        if source_id != Some(adapter_id) || !runtime.has_recent_packet(now) {
            return (
                waiting_signal_snapshot(runtime, adapter_id, game_id, game_name, now),
                false,
            );
        }

        let mut snapshot = inner.telemetry.clone();
        if let Some(shift_event) = inner.forza_effect_runtime.latched_shift_event(now) {
            snapshot.apply_update(signal_update("drivetrain.shift_event", shift_event));
            snapshot.apply_update(signal_update("drivetrain.shift_pulse", 1.0));
        } else {
            snapshot.apply_update(signal_update("drivetrain.shift_event", "none"));
            snapshot.apply_update(signal_update("drivetrain.shift_pulse", 0.0));
        }
        snapshot.apply_update(signal_update(
            "suspension.impact_pulse",
            inner.forza_effect_runtime.latched_suspension_impact(now),
        ));
        return (snapshot, true);
    }

    if let Some(source_id) = inner.telemetry.text("source.id") {
        if let Some(runtime) = inner
            .adapter_runtime(source_id)
            .filter(|runtime| !runtime.has_recent_packet(now))
        {
            return (
                forza_inactive_signal_snapshot(
                    runtime,
                    now,
                    inner.telemetry.text("game.id"),
                    inner.telemetry.text("game.name"),
                ),
                false,
            );
        }
    }

    (inner.telemetry.clone(), true)
}

fn telemetry_signal(
    name: &str,
    value: impl Serialize,
    unit: Option<&str>,
    updated_ms_ago: u64,
) -> TelemetrySignalResponse {
    TelemetrySignalResponse {
        name: name.to_string(),
        value: serde_json::to_value(value).expect("telemetry signal value is serializable"),
        unit: unit.map(str::to_string),
        updated_ms_ago,
    }
}

fn signal_update(name: &str, value: impl Into<SignalValue>) -> SignalUpdate {
    SignalUpdate::new(
        SignalName::new(name).expect("internal telemetry signal name is valid"),
        value,
    )
}

fn signal_value_json(value: &SignalValue) -> serde_json::Value {
    match value {
        SignalValue::Number(value) => serde_json::json!(value),
        SignalValue::Bool(value) => serde_json::json!(value),
        SignalValue::Text(value) => serde_json::json!(value),
    }
}

fn signal_unit(name: &str) -> Option<&'static str> {
    match name {
        "vehicle.speed_kmh" => Some("km/h"),
        "vehicle.rpm" | "vehicle.max_rpm" => Some("rpm"),
        "vehicle.acceleration.x"
        | "vehicle.acceleration.y"
        | "vehicle.acceleration.z"
        | "vehicle.acceleration.magnitude" => Some("m/s^2"),
        "source.packet_rate_hz" => Some("Hz"),
        "source.packet_size" => Some("bytes"),
        _ => None,
    }
}

#[cfg(test)]
fn current_effect_response(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    hardware_output_enabled: bool,
) -> CurrentEffectResponse {
    let resolution = profile_resolution(inner, game_detection);
    let config = controller_config_for_resolution(inner, &resolution);
    let (snapshot, telemetry_live) = current_effect_snapshot(inner, game_detection);
    let profile_id = resolution
        .selected_profile_id
        .clone()
        .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
    let profile_name = profile_name_by_id(inner, &profile_id).unwrap_or_else(|| profile_id.clone());
    let profile = runtime_profile_for(&profile_id, &profile_name, config.as_ref(), &snapshot);
    let mut output = EffectEngine::new().evaluate(&profile, &snapshot);
    apply_runtime_output_enhancements(
        &profile_id,
        config.as_ref(),
        &snapshot,
        telemetry_live,
        &mut output,
    );
    apply_detection_lightbar_preview(game_detection, telemetry_live, &mut output);
    current_effect_response_from_parts(
        resolution,
        profile,
        config.as_ref(),
        snapshot,
        telemetry_live,
        output,
        game_detection,
        hardware_output_enabled,
    )
}

#[allow(clippy::too_many_arguments)]
fn current_effect_response_from_parts(
    resolution: ProfileResolutionResponse,
    profile: Profile,
    config: Option<&ControllerConfig>,
    snapshot: SignalSnapshot,
    telemetry_live: bool,
    output: ControllerOutputFrame,
    game_detection: Option<&GameDetectionResponse>,
    hardware_output_enabled: bool,
) -> CurrentEffectResponse {
    let mut warnings = Vec::new();

    if hardware_output_enabled {
        warnings.push(
            "Hardware output is enabled. DSCC keeps trigger and rumble output neutral until supported-game telemetry is live or during a manual effect test; idle lightbar follows the Global profile."
                .to_string(),
        );
    } else {
        warnings.push(
                "Hardware output is disabled; this frame is the validated target state, not a raw hardware write."
                .to_string(),
        );
    }
    if resolution
        .controller_id
        .as_deref()
        .is_some_and(is_windows_pnp_controller_id)
    {
        warnings.push(
            "Windows currently exposes this Edge only through the PnP fallback, so live battery and lightbar writes require the Sony HID interface to become visible to DSCC."
                .to_string(),
        );
    }
    if is_forza_runtime_profile(&profile.id, &snapshot) && !telemetry_live {
        if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
            let source_label = if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "shared-memory telemetry"
            } else {
                "Data Out telemetry"
            };
            warnings.push(
                format!(
                    "{game_name} ({game_id}) is detected, but {source_label} is not live; trigger output stays neutral until fresh telemetry arrives."
                ),
            );
        } else {
            warnings.push(
                "Racing telemetry is stale and no supported process is detected; trigger output is neutral."
                    .to_string(),
            );
        }
    }

    CurrentEffectResponse {
        controller_id: resolution.controller_id,
        selected_profile_id: Some(profile.id),
        selected_profile_name: Some(profile.name),
        reason: resolution.reason,
        dry_run: !hardware_output_enabled,
        hardware_output_enabled,
        output,
        parity_effects: effect_mapping_statuses(&snapshot, config),
        warnings,
    }
}

fn apply_runtime_output_enhancements(
    profile_id: &str,
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if is_forza_runtime_profile(profile_id, snapshot) {
        apply_forza_output_enhancements(config, snapshot, telemetry_live, output);
    }
}

fn apply_detection_lightbar_preview(
    game_detection: Option<&GameDetectionResponse>,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if telemetry_live {
        return;
    }
    let Some(detection) = game_detection else {
        return;
    };
    if detection.profile_id.is_none() {
        return;
    }
    if let Some(lightbar) = detection_lightbar_output(detection) {
        output.lightbar = Some(lightbar);
    }
}

fn apply_forza_output_enhancements(
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if !telemetry_live || snapshot.text("game.state") != Some("driving") {
        output.rumble = None;
        output.player_leds = None;
        return;
    }

    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let trigger = config.map(|config| &config.trigger);
    let vibration = trigger_vibration_scalar(trigger);
    if vibration <= 0.0 {
        output.rumble = None;
    } else {
        output.rumble = forza_rumble_output(
            &forza,
            snapshot,
            vibration,
            trigger.map_or("Balanced", |trigger| trigger.vibration_mode.as_str()),
        );
    }

    if config.map(|config| config.lightbar.enabled).unwrap_or(true) {
        let rpm_leds = forza.effect("rpm_leds");
        let rpm_led_scalar = if rpm_leds.route == "light_led" {
            rpm_leds.scalar()
        } else {
            0.0
        };
        output.lightbar = Some(forza_lightbar_output(config, snapshot, rpm_led_scalar));
        output.player_leds = if rpm_led_scalar > 0.0 {
            Some(PlayerLedsOutput {
                count: forza_gear_player_led_count(snapshot),
            })
        } else {
            None
        };
    }
}

fn forza_rumble_output(
    forza: &ForzaTelemetryConfig,
    snapshot: &SignalSnapshot,
    vibration: f64,
    vibration_mode: &str,
) -> Option<RumbleOutput> {
    let throttle = signal_unit_value(snapshot, "input.throttle");
    let brake = signal_unit_value(snapshot, "input.brake");
    let handbrake = signal_unit_value(snapshot, "input.handbrake");
    let rpm = signal_unit_value(snapshot, "vehicle.rpm_ratio");
    let speed = signal_scaled(snapshot, "vehicle.speed_kmh", 12.0, 280.0);
    let rolling = signal_scaled(snapshot, "vehicle.speed_kmh", 3.0, 38.0);
    let rolling_texture = rolling.sqrt();
    let surface = signal_unit_value(snapshot, "surface.rumble.max");
    let strip = signal_unit_value(snapshot, "surface.rumble_strip.max");
    let puddle = signal_unit_value(snapshot, "surface.puddle.max");
    let slip = signal_scaled(snapshot, "wheel.slip.max", 0.16, 1.10);
    let front_slip = signal_scaled(snapshot, "wheel.slip.front_max", 0.14, 1.0);
    let rear_slip = signal_scaled(snapshot, "wheel.slip.rear_max", 0.14, 1.0);
    let slip_ratio = signal_scaled(snapshot, "tire.slip_ratio.max", 0.12, 1.0);
    let slip_angle = signal_scaled(snapshot, "tire.slip_angle.max", 0.22, 1.05);
    let shift = signal_unit_value(snapshot, "drivetrain.shift_pulse");
    let suspension_impact = signal_unit_value(snapshot, "suspension.impact_pulse");
    let rev_limiter = signal_scaled(snapshot, "vehicle.rpm_ratio", 0.93, 1.0);
    let native_passthrough = forza.body_rumble_mode == default_forza_body_rumble_mode();

    let road_texture = surface.max(strip * 0.95) * rolling_texture * (0.35 + speed * 0.65);
    let strip_feedback = strip * rolling_texture;
    let puddle_feedback = puddle * rolling_texture;
    let pedal_load = throttle.max(brake).max(handbrake);
    let steering_slip_feedback = slip_angle * (0.12 + pedal_load * 0.38);
    let tire_feedback = slip.max(slip_ratio * 0.85).max(steering_slip_feedback);
    let brake_feedback = if brake > 0.08 {
        front_slip.max(tire_feedback * brake)
    } else {
        0.0
    };
    let traction_feedback = if throttle > 0.12 {
        rear_slip.max(tire_feedback * throttle)
    } else {
        0.0
    };
    let drivetrain = (rpm * rpm * (0.35 + throttle * 0.65)).clamp(0.0, 1.0);

    let mut low = 0.0;
    let mut high = 0.0;
    if !native_passthrough {
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("road_texture"),
            road_texture,
            0.46,
            0.58,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("rumble_strip"),
            strip_feedback,
            0.26,
            0.52,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("tire_slip"),
            tire_feedback.max(brake_feedback).max(traction_feedback),
            0.16,
            0.56,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("puddle_drag"),
            puddle_feedback,
            0.34,
            0.24,
        );
    }
    add_forza_rumble_component(
        &mut low,
        &mut high,
        &forza.effect("suspension_impact"),
        suspension_impact,
        0.98,
        0.42,
    );
    add_forza_rumble_component(
        &mut low,
        &mut high,
        &forza.effect("gear_shift_thump"),
        shift,
        0.92,
        0.84,
    );
    if !native_passthrough {
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("rev_limiter_buzz"),
            rev_limiter,
            0.20,
            0.80,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("throttle_resistance"),
            drivetrain,
            0.32,
            0.12,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("brake_resistance"),
            brake,
            0.14,
            0.08,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("handbrake_wall"),
            handbrake,
            0.30,
            0.12,
        );
    }

    low = clamp_unit(low * vibration);
    high = clamp_unit(high * vibration);
    (low, high) = apply_vibration_mode(vibration_mode, low, high);

    if low < 0.025 && high < 0.025 {
        None
    } else {
        Some(RumbleOutput {
            low_frequency: clamp_unit(low),
            high_frequency: clamp_unit(high),
        })
    }
}

fn add_forza_rumble_component(
    low: &mut f64,
    high: &mut f64,
    tuning: &ForzaEffectConfig,
    value: f64,
    low_weight: f64,
    high_weight: f64,
) {
    if tuning.scalar() <= 0.0 || !route_has_body(&tuning.route) {
        return;
    }

    let (low_mix, high_mix) = route_body_mix(&tuning.route);
    let signal = clamp_unit(value) * tuning.scalar();
    *low += signal * low_weight * low_mix;
    *high += signal * high_weight * high_mix;
}

fn forza_lightbar_output(
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    rpm_led_scalar: f64,
) -> LightbarOutput {
    let configured = config
        .map(|config| config.lightbar.clone().normalized())
        .unwrap_or_default();
    let rpm = signal_unit_value(snapshot, "vehicle.rpm_ratio");
    let base = configured.rgb();
    let redline = configured.rpm_rgb();
    let rpm_blend = clamp_unit(rpm * rpm_led_scalar);
    let color = blend_rgb(base, redline, rpm_blend);
    let brightness =
        clamp_unit(f64::from(configured.brightness) / 100.0 + rpm * 0.12 * rpm_led_scalar);

    LightbarOutput { color, brightness }
}

fn blend_rgb(from: RgbColor, to: RgbColor, amount: f64) -> RgbColor {
    fn blend_channel(from: u8, to: u8, amount: f64) -> u8 {
        (f64::from(from) + (f64::from(to) - f64::from(from)) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    }

    let amount = clamp_unit(amount);
    RgbColor {
        red: blend_channel(from.red, to.red, amount),
        green: blend_channel(from.green, to.green, amount),
        blue: blend_channel(from.blue, to.blue, amount),
    }
}

fn forza_gear_player_led_count(snapshot: &SignalSnapshot) -> u8 {
    snapshot
        .number("drivetrain.gear")
        .and_then(signal_gear_to_u8)
        .unwrap_or_default()
        .clamp(0, 5)
}

fn signal_unit_value(snapshot: &SignalSnapshot, name: &str) -> f64 {
    clamp_unit(snapshot.number(name).unwrap_or_default())
}

fn signal_scaled(snapshot: &SignalSnapshot, name: &str, min: f64, max: f64) -> f64 {
    if min >= max {
        return 0.0;
    }

    let value = snapshot.number(name).unwrap_or_default();
    clamp_unit((value - min) / (max - min))
}

fn suspension_impact_strength(
    suspension_travel: Option<f64>,
    acceleration_magnitude: Option<f64>,
    speed_kmh: Option<f64>,
) -> f64 {
    let suspension = signal_scaled_value(suspension_travel.unwrap_or_default(), 0.10, 0.30);
    let acceleration = signal_scaled_value(acceleration_magnitude.unwrap_or_default(), 18.0, 38.0);
    let speed_gate = signal_scaled_value(speed_kmh.unwrap_or_default(), 8.0, 24.0);
    let mut impact = (acceleration * 0.75 + suspension * 0.45).clamp(0.0, 1.0) * speed_gate;

    if suspension < 0.18 {
        impact *= 0.35;
    }

    clamp_unit(impact)
}

fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn controller_output_target_or_reason(
    inner: &AgentStateInner,
    controller_id: &str,
) -> Result<ControllerOutputTarget, String> {
    if let Some(target) = inner.controllers.output_target(controller_id) {
        return Ok(target);
    }

    let Some(detail) = inner.controllers.detail(controller_id) else {
        return Err(format!("Controller {controller_id} is not known to DSCC"));
    };
    if is_windows_pnp_controller_id(controller_id) {
        return Err(
            "Controller is visible only through the Windows PnP fallback; no HID output handle is available"
                .to_string(),
        );
    }
    if !detail.connected {
        return Err(format!("Controller {controller_id} is disconnected"));
    }
    if detail.permission != ControllerPermissionState::Granted {
        return Err(format!(
            "Controller {controller_id} does not have HID permission"
        ));
    }
    if !detail.capabilities.adaptive_triggers {
        return Err(format!(
            "Controller {controller_id} does not advertise adaptive trigger support"
        ));
    }

    Err(format!(
        "Controller {controller_id} has no openable raw HID output target"
    ))
}

fn controller_config_for_resolution(
    inner: &AgentStateInner,
    resolution: &ProfileResolutionResponse,
) -> Option<ControllerConfig> {
    let controller_id = resolution.controller_id.as_deref()?;
    inner
        .controller_configs
        .get(controller_id)
        .cloned()
        .or_else(|| {
            inner
                .controllers
                .detail(controller_id)
                .map(|detail| ControllerConfig::default_for(controller_id, detail.model))
        })
}

fn profile_name_by_id(inner: &AgentStateInner, profile_id: &str) -> Option<String> {
    inner
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| profile.name.clone())
}

fn runtime_profile_for(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
) -> Profile {
    if profile_id == GLOBAL_PROFILE_ID {
        return global_runtime_profile(profile_id, profile_name, config);
    }

    if is_forza_runtime_profile(profile_id, snapshot) {
        forza_runtime_profile(profile_id, profile_name, config)
    } else {
        generic_runtime_profile(profile_id, profile_name, config)
    }
}

fn is_forza_runtime_profile(profile_id: &str, snapshot: &SignalSnapshot) -> bool {
    profile_id.contains("forza")
        || profile_id == ASSETTO_CORSA_RALLY_PROFILE_ID
        || snapshot.text("source.id").is_some_and(|source| {
            matches!(
                source,
                FORZA_DATA_OUT_ADAPTER_ID | ASSETTO_SHARED_MEMORY_ADAPTER_ID
            )
        })
        || snapshot
            .text("game.id")
            .is_some_and(|game| game.starts_with("forza") || game == "assetto-corsa-rally")
}

fn global_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::Disabled,
        rules: lightbar_rules(config.map(|config| &config.lightbar)),
    }
}

fn forza_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    let trigger = config.map(|config| &config.trigger);
    let lightbar = config.map(|config| &config.lightbar);
    // The resolver materializes the selected profile into this cloned config
    // before evaluation, so automatic game detection can use the right preset
    // without requiring the UI to save/apply it first.
    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let intensity = trigger.map_or(0.82, trigger_intensity_scalar);
    if trigger.is_some_and(|trigger| trigger.effect == "Off") || intensity <= 0.0 {
        return Profile {
            id: profile_id.to_string(),
            name: profile_name.to_string(),
            version: 1,
            rumble_policy: RumblePolicy::FullControl,
            rules: lightbar_rules(lightbar),
        };
    }

    let l2_start = trigger.map_or(0.18, |trigger| f64::from(trigger.l2_from.min(100)) / 100.0);
    let r2_start = trigger.map_or(0.10, |trigger| f64::from(trigger.r2_from.min(100)) / 100.0);
    let l2_end = trigger.map_or(FORZA_BRAKE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.l2_from, trigger.l2_to)
    });
    let r2_end = trigger.map_or(FORZA_THROTTLE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.r2_from, trigger.r2_to)
    });
    let l2_has_overtravel_guard = brake_overtravel_guard_active(l2_end);
    let l2_endstop_wall = brake_overtravel_wall_position(l2_start, l2_end);
    let l2_overtravel_ramp_start = brake_overtravel_ramp_start(l2_start, l2_endstop_wall);
    let r2_has_overtravel_guard = throttle_overtravel_guard_active(r2_end);
    let r2_endstop_wall = throttle_overtravel_wall_position(r2_start, r2_end);
    let r2_overtravel_ramp_start = throttle_overtravel_ramp_start(r2_start, r2_endstop_wall);
    let l2_normal_end = if l2_has_overtravel_guard && l2_overtravel_ramp_start < l2_endstop_wall {
        l2_overtravel_ramp_start
    } else {
        l2_endstop_wall
    }
    .max(l2_start + 0.01);
    let r2_normal_end = if r2_has_overtravel_guard && r2_overtravel_ramp_start < r2_endstop_wall {
        r2_overtravel_ramp_start
    } else {
        r2_endstop_wall
    }
    .max(r2_start + 0.01);
    let abs_brake_threshold = abs_brake_threshold_for_range(l2_start, l2_end);
    let l2_curve_points = trigger
        .map(|trigger| trigger_curve_value_points(&trigger.l2_curve_points))
        .unwrap_or_else(|| trigger_curve_value_points(&default_l2_trigger_curve_points()));
    let r2_curve_points = trigger
        .map(|trigger| trigger_curve_value_points(&trigger.r2_curve_points))
        .unwrap_or_else(|| trigger_curve_value_points(&default_r2_trigger_curve_points()));
    let brake = forza.effect("brake_resistance");
    let abs = forza.effect("abs_slip_pulse");
    let handbrake = forza.effect("handbrake_wall");
    let throttle = forza.effect("throttle_resistance");
    let shift = forza.effect("gear_shift_thump");
    let rev = forza.effect("rev_limiter_buzz");
    let trigger_scalar = intensity.clamp(0.0, 1.0);
    let brake_baseline_force =
        scaled_unit(FORZA_BRAKE_BASELINE_FORCE, brake.scalar() * trigger_scalar);
    let brake_normal_force = scaled_unit(FORZA_BRAKE_NORMAL_FORCE, brake.scalar() * trigger_scalar);
    let brake_endstop_force = scaled_unit(
        FORZA_BRAKE_ENDSTOP_FORCE,
        brake.scalar() * trigger_scalar * FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
    );
    let throttle_baseline_force = scaled_unit(
        FORZA_THROTTLE_BASELINE_FORCE,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_normal_force = scaled_unit(
        FORZA_THROTTLE_NORMAL_FORCE,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_endstop_scalar =
        throttle.scalar() * trigger_scalar * FORZA_THROTTLE_ENDSTOP_FORCE_BOOST;
    let throttle_endstop_force = scaled_unit(FORZA_THROTTLE_ENDSTOP_FORCE, throttle_endstop_scalar);
    let abs_amplitude = scaled_unit(FORZA_ABS_PULSE_AMPLITUDE, abs.scalar());
    let rev_amplitude = scaled_unit(10.0 / 63.0, rev.scalar() * trigger_scalar);
    let shift_amplitude = scaled_unit(1.0, shift.scalar());

    let baseline_condition = forza_baseline_trigger_condition();
    let mut rules = Vec::new();

    if abs.scalar() > 0.0 && route_has_l2(&abs.route) {
        rules.push(EffectRule {
            id: "forza-l2-abs-slip-pulse".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 60,
            condition: RuleCondition::All {
                conditions: vec![
                    number_condition(
                        "input.brake",
                        ComparisonOp::GreaterOrEqual,
                        abs_brake_threshold,
                    ),
                    number_condition(
                        "vehicle.speed_kmh",
                        ComparisonOp::GreaterOrEqual,
                        FORZA_ABS_MIN_SPEED_KMH,
                    ),
                    RuleCondition::Any {
                        conditions: vec![
                            number_condition(
                                "tire.slip_ratio.max",
                                ComparisonOp::GreaterOrEqual,
                                FORZA_ABS_SLIP_THRESHOLD,
                            ),
                            number_condition(
                                "wheel.slip.max",
                                ComparisonOp::GreaterOrEqual,
                                FORZA_ABS_SLIP_THRESHOLD,
                            ),
                        ],
                    },
                ],
            },
            effect: EffectTemplate::Pulse {
                amplitude: ValueSource::constant(abs_amplitude),
                frequency_hz: ValueSource::constant(FORZA_ABS_PULSE_FREQUENCY_HZ),
            },
        });
    }

    if handbrake.scalar() > 0.0 && route_has_l2(&handbrake.route) {
        rules.push(EffectRule {
            id: "forza-l2-handbrake-wall".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 45,
            condition: number_condition("input.handbrake", ComparisonOp::GreaterThan, 0.05),
            effect: EffectTemplate::Wall {
                position: ValueSource::constant((l2_start + 0.12).clamp(0.0, 0.86)),
                strength: ValueSource::constant(scaled_unit(
                    FORZA_HANDBRAKE_FORCE,
                    handbrake.scalar() * trigger_scalar,
                )),
            },
        });
    }

    if brake.scalar() > 0.0 && route_has_l2(&brake.route) {
        rules.push(EffectRule {
            id: "forza-l2-brake-full-force".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 12,
            condition: number_condition(
                "input.brake",
                ComparisonOp::GreaterOrEqual,
                l2_endstop_wall,
            ),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(l2_endstop_wall),
                strength: ValueSource::constant(brake_endstop_force),
            },
        });
        if l2_has_overtravel_guard && l2_overtravel_ramp_start < l2_endstop_wall {
            rules.push(EffectRule {
                id: "forza-l2-brake-overtravel-ramp".to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::L2,
                priority: 11,
                condition: number_condition(
                    "input.brake",
                    ComparisonOp::GreaterOrEqual,
                    l2_overtravel_ramp_start,
                ),
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(l2_overtravel_ramp_start),
                    strength: ValueSource::signal_curve(
                        "input.brake",
                        l2_overtravel_ramp_start,
                        l2_endstop_wall,
                        brake_normal_force,
                        brake_endstop_force,
                        FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
                    ),
                },
            });
        }
        rules.push(EffectRule {
            id: "forza-l2-brake-resistance".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 10,
            condition: baseline_condition.clone(),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(l2_start),
                strength: ValueSource::signal_points(
                    "input.brake",
                    l2_start,
                    l2_normal_end,
                    brake_baseline_force,
                    brake_normal_force,
                    l2_curve_points.clone(),
                ),
            },
        });
    }

    push_routed_pulse_rule(
        &mut rules,
        &rev,
        "forza-rev-limiter-buzz",
        55,
        number_condition(
            "vehicle.rpm_ratio",
            ComparisonOp::GreaterOrEqual,
            FORZA_REV_LIMIT_RATIO,
        ),
        ValueSource::constant(rev_amplitude),
        ValueSource::constant(30.0),
    );
    push_shift_thump_rules(&mut rules, &shift, shift_amplitude);

    if throttle.scalar() > 0.0 && route_has_r2(&throttle.route) {
        rules.push(EffectRule {
            id: "forza-r2-throttle-full-force".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: 12,
            condition: number_condition(
                "input.throttle",
                ComparisonOp::GreaterOrEqual,
                r2_endstop_wall,
            ),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(r2_endstop_wall),
                strength: ValueSource::constant(throttle_endstop_force),
            },
        });
        if r2_has_overtravel_guard && r2_overtravel_ramp_start < r2_endstop_wall {
            rules.push(EffectRule {
                id: "forza-r2-throttle-overtravel-ramp".to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::R2,
                priority: 11,
                condition: number_condition(
                    "input.throttle",
                    ComparisonOp::GreaterOrEqual,
                    r2_overtravel_ramp_start,
                ),
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(r2_overtravel_ramp_start),
                    strength: ValueSource::signal_curve(
                        "input.throttle",
                        r2_overtravel_ramp_start,
                        r2_endstop_wall,
                        throttle_normal_force,
                        throttle_endstop_force,
                        FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE,
                    ),
                },
            });
        }
        rules.push(EffectRule {
            id: "forza-r2-throttle-resistance".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: 10,
            condition: baseline_condition,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(r2_start),
                strength: ValueSource::signal_points(
                    "input.throttle",
                    r2_start,
                    r2_normal_end,
                    throttle_baseline_force,
                    throttle_normal_force,
                    r2_curve_points.clone(),
                ),
            },
        });
    }

    rules.extend(lightbar_rules(lightbar));

    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::FullControl,
        rules,
    }
}

fn forza_baseline_trigger_condition() -> RuleCondition {
    text_condition("game.state", ComparisonOp::Eq, "driving")
}

fn push_routed_pulse_rule(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    id: &str,
    priority: i32,
    condition: RuleCondition,
    amplitude: ValueSource,
    frequency_hz: ValueSource,
) {
    if tuning.scalar() <= 0.0 {
        return;
    }

    for target in routed_trigger_targets(&tuning.route) {
        rules.push(EffectRule {
            id: format!("{id}-{}", trigger_target_label(target)),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority,
            condition: condition.clone(),
            effect: EffectTemplate::Pulse {
                amplitude: amplitude.clone(),
                frequency_hz: frequency_hz.clone(),
            },
        });
    }
}

fn push_shift_thump_rules(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    shift_amplitude: f64,
) {
    if tuning.scalar() <= 0.0 {
        return;
    }

    for (target, pedal_signal) in [
        (EffectTarget::L2, "input.brake"),
        (EffectTarget::R2, "input.throttle"),
    ] {
        if !route_targets_trigger(&tuning.route, target) {
            continue;
        }

        let target_label = trigger_target_label(target);
        rules.push(EffectRule {
            id: format!("forza-gear-shift-thump-{target_label}-pulse-ab"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: 70,
            condition: shift_thump_condition(pedal_signal, ComparisonOp::GreaterOrEqual),
            effect: EffectTemplate::PulseAb {
                strength: ValueSource::constant(shift_amplitude),
                frequency_hz: ValueSource::constant(FORZA_SHIFT_FREQUENCY_HZ),
                wall_zones: ValueSource::constant(FORZA_SHIFT_WALL_ZONES),
            },
        });
        rules.push(EffectRule {
            id: format!("forza-gear-shift-thump-{target_label}-pulse"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: 70,
            condition: shift_thump_condition(pedal_signal, ComparisonOp::LessThan),
            effect: EffectTemplate::Pulse {
                amplitude: ValueSource::constant(shift_amplitude),
                frequency_hz: ValueSource::constant(FORZA_SHIFT_FREQUENCY_HZ),
            },
        });
    }
}

fn shift_thump_condition(pedal_signal: &str, pedal_op: ComparisonOp) -> RuleCondition {
    RuleCondition::All {
        conditions: vec![
            text_condition("drivetrain.shift_event", ComparisonOp::NotEq, "none"),
            number_condition(pedal_signal, pedal_op, FORZA_SHIFT_WALL_FORM_AT),
        ],
    }
}

fn routed_trigger_targets(route: &str) -> Vec<EffectTarget> {
    match route {
        "l2" => vec![EffectTarget::L2],
        "r2" => vec![EffectTarget::R2],
        "both_triggers" | "body_and_triggers" => vec![EffectTarget::L2, EffectTarget::R2],
        "r2_and_body" => vec![EffectTarget::R2],
        _ => Vec::new(),
    }
}

fn route_targets_trigger(route: &str, target: EffectTarget) -> bool {
    match target {
        EffectTarget::L2 => route_has_l2(route),
        EffectTarget::R2 => route_has_r2(route),
        _ => false,
    }
}

fn trigger_target_label(target: EffectTarget) -> &'static str {
    match target {
        EffectTarget::L2 => "l2",
        EffectTarget::R2 => "r2",
        _ => "other",
    }
}

fn route_has_l2(route: &str) -> bool {
    matches!(route, "l2" | "both_triggers" | "body_and_triggers")
}

fn route_has_r2(route: &str) -> bool {
    matches!(
        route,
        "r2" | "both_triggers" | "body_and_triggers" | "r2_and_body"
    )
}

fn route_has_body(route: &str) -> bool {
    matches!(
        route,
        "body_both" | "body_left" | "body_right" | "body_and_triggers" | "r2_and_body"
    )
}

fn route_body_mix(route: &str) -> (f64, f64) {
    match route {
        "body_left" => (1.0, 0.25),
        "body_right" => (0.25, 1.0),
        "body_both" | "body_and_triggers" => (1.0, 1.0),
        "r2_and_body" => (0.70, 0.70),
        _ => (0.0, 0.0),
    }
}

fn scaled_unit(value: f64, scalar: f64) -> f64 {
    clamp_unit(value * scalar)
}

fn generic_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    let trigger = config.map(|config| &config.trigger);
    let intensity = trigger.map_or(0.62, trigger_intensity_scalar);
    let mode = trigger.map_or("Adaptive resistance", |trigger| trigger.effect.as_str());
    let effect = match mode {
        "Off" => EffectTemplate::Off,
        "Pulse" => EffectTemplate::Pulse {
            amplitude: ValueSource::constant(intensity),
            frequency_hz: ValueSource::constant(36.0),
        },
        "Wall pulse" => EffectTemplate::PulseAb {
            strength: ValueSource::constant(intensity),
            frequency_hz: ValueSource::constant(36.0),
            wall_zones: ValueSource::constant(2.0),
        },
        "Wall" => EffectTemplate::Wall {
            position: ValueSource::constant(0.32),
            strength: ValueSource::constant(intensity),
        },
        _ => EffectTemplate::AdaptiveResistance {
            start_position: ValueSource::constant(0.16),
            strength: ValueSource::constant(intensity),
        },
    };
    let mut rules = vec![
        EffectRule {
            id: "generic-l2-preview".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: effect.clone(),
        },
        EffectRule {
            id: "generic-r2-preview".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect,
        },
    ];
    rules.extend(lightbar_rules(config.map(|config| &config.lightbar)));

    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::TriggerOverlay,
        rules,
    }
}

fn lightbar_rules(config: Option<&LightbarConfig>) -> Vec<EffectRule> {
    let config = config.cloned().unwrap_or_default().normalized();
    if !config.enabled {
        return vec![EffectRule {
            id: "lightbar-disabled".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::Lightbar,
            priority: 1,
            condition: RuleCondition::Always,
            effect: EffectTemplate::Off,
        }];
    }

    vec![EffectRule {
        id: "lightbar-user-color".to_string(),
        smoothing: None,
        hysteresis: None,
        timeout: None,
        target: EffectTarget::Lightbar,
        priority: 1,
        condition: RuleCondition::Always,
        effect: EffectTemplate::Lightbar {
            color: config.rgb(),
            brightness: ValueSource::constant(f64::from(config.brightness) / 100.0),
        },
    }]
}

fn trigger_intensity_scalar(trigger: &TriggerConfig) -> f64 {
    match trigger.intensity.as_str() {
        "Off" => 0.0,
        "Weak" => 0.38,
        "Medium" => 0.62,
        "Strong (Standard)" => 0.86,
        _ => 0.62,
    }
}

fn trigger_vibration_scalar(trigger: Option<&TriggerConfig>) -> f64 {
    match trigger.map(|trigger| trigger.vibration.as_str()) {
        Some("Off") => 0.0,
        Some("Low") => 0.48,
        Some("High") => 1.0,
        Some("Medium") | None => 0.82,
        _ => 0.82,
    }
}

fn apply_vibration_mode(mode: &str, low: f64, high: f64) -> (f64, f64) {
    match mode {
        "Deep thump" | "deep_thump" => (clamp_unit(low.max(high * 0.28)), clamp_unit(high * 0.42)),
        "Fine buzz" | "fine_buzz" => (clamp_unit(low * 0.42), clamp_unit(high.max(low * 0.28))),
        _ => (clamp_unit(low), clamp_unit(high)),
    }
}

fn number_condition(signal: &str, op: ComparisonOp, value: f64) -> RuleCondition {
    RuleCondition::Signal {
        signal: signal.to_string(),
        op,
        value: ComparableValue::Number(value),
    }
}

fn text_condition(signal: &str, op: ComparisonOp, value: &str) -> RuleCondition {
    RuleCondition::Signal {
        signal: signal.to_string(),
        op,
        value: ComparableValue::Text(value.to_string()),
    }
}

fn effect_mapping_statuses(
    snapshot: &SignalSnapshot,
    config: Option<&ControllerConfig>,
) -> Vec<EffectMappingStatus> {
    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let brake = snapshot.number("input.brake").unwrap_or_default();
    let throttle = snapshot.number("input.throttle").unwrap_or_default();
    let speed_kmh = snapshot.number("vehicle.speed_kmh").unwrap_or_default();
    let moving = speed_kmh > 3.0;
    let slip = snapshot.number("wheel.slip.max").unwrap_or_default();
    let front_slip = snapshot.number("wheel.slip.front_max").unwrap_or_default();
    let handbrake = snapshot.number("input.handbrake").unwrap_or_default();
    let gear = snapshot.number("drivetrain.gear").unwrap_or_default();
    let rpm_ratio = snapshot.number("vehicle.rpm_ratio").unwrap_or_default();
    let shift = snapshot.text("drivetrain.shift_event").unwrap_or("none");
    let rumble_strip = snapshot
        .number("surface.rumble_strip.max")
        .unwrap_or_default();
    let puddle = snapshot.number("surface.puddle.max").unwrap_or_default();
    let suspension_impact = snapshot
        .number("suspension.impact_pulse")
        .unwrap_or_default();
    vec![
        mapping_status(
            "brake_resistance",
            "L2",
            "Brake resistance",
            "input.brake",
            brake > 0.02,
            &forza,
        ),
        mapping_status(
            "abs_slip_pulse",
            "L2",
            "ABS / tire slip pulse",
            "wheel.slip.max",
            brake > 0.10 && slip.max(front_slip) > 0.20,
            &forza,
        ),
        mapping_status(
            "handbrake_wall",
            "L2",
            "Handbrake resistance",
            "input.handbrake",
            handbrake > 0.05,
            &forza,
        ),
        mapping_status(
            "throttle_resistance",
            "R2",
            "Throttle resistance",
            "input.throttle",
            throttle > 0.02,
            &forza,
        ),
        mapping_status(
            "gear_shift_thump",
            "R2",
            "Gear shift thump",
            "drivetrain.shift_event",
            shift != "none",
            &forza,
        ),
        mapping_status(
            "rev_limiter_buzz",
            "R2",
            "Rev limiter buzz",
            "vehicle.rpm_ratio",
            rpm_ratio >= 0.965,
            &forza,
        ),
        mapping_status(
            "road_texture",
            "HD",
            "Road texture rumble",
            "surface.rumble.max",
            moving && snapshot.number("surface.rumble.max").unwrap_or_default() > 0.08,
            &forza,
        ),
        mapping_status(
            "rumble_strip",
            "HD",
            "Rumble strip pulse",
            "surface.rumble_strip.max",
            moving && rumble_strip > 0.0,
            &forza,
        ),
        mapping_status(
            "tire_slip",
            "HD",
            "Tire slip rumble",
            "wheel.slip.max",
            moving && slip > 0.20,
            &forza,
        ),
        mapping_status(
            "puddle_drag",
            "HD",
            "Puddle drag",
            "surface.puddle.max",
            moving && puddle > 0.08,
            &forza,
        ),
        mapping_status(
            "suspension_impact",
            "HD",
            "Suspension / impact thump",
            "suspension.impact_pulse",
            moving && suspension_impact > 0.05,
            &forza,
        ),
        mapping_status(
            "rpm_leds",
            "LED",
            "Gear LEDs / RPM lightbar",
            "drivetrain.gear + vehicle.rpm_ratio",
            gear > 0.0 || rpm_ratio > 0.20,
            &forza,
        ),
    ]
}

fn mapping_status(
    id: &str,
    target: &str,
    label: &str,
    signal: &str,
    active: bool,
    forza: &ForzaTelemetryConfig,
) -> EffectMappingStatus {
    let enabled = forza.effect(id).enabled;
    EffectMappingStatus {
        id: id.to_string(),
        target: target.to_string(),
        label: label.to_string(),
        signal: signal.to_string(),
        state: if !enabled {
            "disabled"
        } else if active {
            "active"
        } else {
            "ready"
        }
        .to_string(),
    }
}

fn effect_test_output_frame(request: &EffectTestRequest) -> ControllerOutputFrame {
    let target = request.target.as_deref().unwrap_or("r2");
    let mode = request.mode.as_deref().unwrap_or("adaptive_resistance");
    let intensity = f64::from(request.intensity.unwrap_or(65).min(100)) / 100.0;
    let start_position = request.start_position.unwrap_or(0.16).clamp(0.0, 1.0);
    let mut frame = ControllerOutputFrame::default();

    match target {
        "base_feel" => {
            return base_feel_test_output_frame(
                request.trigger.clone().unwrap_or_default(),
                request.l2_position,
                request.r2_position,
            )
        }
        "l2" => frame.l2 = trigger_for_mode(mode, intensity, start_position),
        "r2" => frame.r2 = trigger_for_mode(mode, intensity, start_position),
        "lightbar" => {
            frame.lightbar = Some(LightbarOutput {
                color: LightbarConfig {
                    enabled: true,
                    color: mode.to_string(),
                    rpm_color: default_rpm_color(),
                    brightness: request.intensity.unwrap_or(65).min(100),
                }
                .normalized()
                .rgb(),
                brightness: intensity,
            });
        }
        "rumble" => {
            frame.rumble = Some(rumble_for_mode(mode, intensity));
        }
        _ => frame.r2 = trigger_for_mode(mode, intensity, start_position),
    }

    frame
}

fn base_feel_test_output_frame(
    trigger: TriggerConfig,
    l2_position: Option<f64>,
    r2_position: Option<f64>,
) -> ControllerOutputFrame {
    let trigger = trigger.normalized();
    ControllerOutputFrame {
        l2: base_feel_trigger_output(
            &trigger.effect,
            &trigger.intensity,
            trigger.l2_from,
            trigger.l2_to,
            &trigger.l2_curve_points,
            l2_position,
        ),
        r2: base_feel_trigger_output(
            &trigger.effect,
            &trigger.intensity,
            trigger.r2_from,
            trigger.r2_to,
            &trigger.r2_curve_points,
            r2_position,
        ),
        ..Default::default()
    }
}

fn base_feel_trigger_output(
    effect: &str,
    intensity_label: &str,
    from: u8,
    to: u8,
    curve_points: &[TriggerCurvePoint],
    position: Option<f64>,
) -> TriggerOutput {
    let strength = position.map_or_else(
        || {
            (trigger_strength_for_label(intensity_label) * (f64::from(to.min(100)) / 100.0))
                .clamp(0.0, 1.0)
        },
        |position| trigger_curve_strength(position, from, to, curve_points, intensity_label),
    );
    if effect == "Off" || strength <= f64::EPSILON {
        return TriggerOutput::Off;
    }
    let mode = effect.to_ascii_lowercase().replace(' ', "_");
    trigger_for_mode(&mode, strength, f64::from(from.min(100)) / 100.0)
}

fn trigger_range_end_position(from: u8, to: u8) -> f64 {
    let start_percent = from.min(100);
    let start = f64::from(start_percent) / 100.0;
    let end = f64::from(to.clamp(start_percent, 100)) / 100.0;
    end.max(start + 0.01)
}

fn endstop_wall_position(start: f64, end: f64) -> f64 {
    (end - FORZA_ENDSTOP_WALL_OFFSET).clamp(start, end)
}

fn brake_overtravel_guard_active(end: f64) -> bool {
    end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION
}

fn brake_overtravel_wall_position(start: f64, end: f64) -> f64 {
    if brake_overtravel_guard_active(end) {
        return (end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET)
            .max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION)
            .clamp(start, end);
    }

    endstop_wall_position(start, end)
}

fn brake_overtravel_ramp_start(start: f64, wall: f64) -> f64 {
    (wall - FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH).clamp(start, wall)
}

fn throttle_overtravel_guard_active(end: f64) -> bool {
    end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION
}

fn throttle_overtravel_wall_position(start: f64, end: f64) -> f64 {
    if throttle_overtravel_guard_active(end) {
        return end
            .min(FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION)
            .clamp(start, end);
    }

    endstop_wall_position(start, end)
}

fn throttle_overtravel_ramp_start(start: f64, wall: f64) -> f64 {
    let ramp_start = wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH;
    ((ramp_start * 1000.0).round() / 1000.0).clamp(start, wall)
}

fn abs_brake_threshold_for_range(start: f64, end: f64) -> f64 {
    let threshold = start + (end - start) * FORZA_ABS_RANGE_START_RATIO;
    threshold.clamp(start, end)
}

fn trigger_curve_strength(
    position: f64,
    from: u8,
    to: u8,
    curve_points: &[TriggerCurvePoint],
    intensity_label: &str,
) -> f64 {
    let strength = trigger_strength_for_label(intensity_label);
    if strength <= f64::EPSILON {
        return 0.0;
    }

    let start = f64::from(from.min(100)) / 100.0;
    let end = trigger_range_end_position(from, to);
    let x = clamp_unit(position);
    if x <= start {
        return 0.0;
    }

    let active = trigger_curve_point_output(curve_points, clamp_unit((x - start) / (end - start)));
    clamp_unit(active * strength)
}

fn trigger_curve_point_output(points: &[TriggerCurvePoint], active: f64) -> f64 {
    let points = normalize_trigger_curve_points(points.to_vec(), TriggerCurve::default_l2());
    let active = clamp_unit(active);
    for window in points.windows(2) {
        let left = window[0];
        let right = window[1];
        let left_input = f64::from(left.input) / 100.0;
        let right_input = f64::from(right.input) / 100.0;
        if active >= left_input && active <= right_input {
            if (right_input - left_input).abs() < f64::EPSILON {
                return f64::from(right.output) / 100.0;
            }
            let ratio = (active - left_input) / (right_input - left_input);
            let left_output = f64::from(left.output) / 100.0;
            let right_output = f64::from(right.output) / 100.0;
            return left_output + (right_output - left_output) * ratio;
        }
    }

    points
        .last()
        .map(|point| f64::from(point.output) / 100.0)
        .unwrap_or(0.0)
}

fn trigger_strength_for_label(intensity_label: &str) -> f64 {
    match intensity_label {
        "Off" => 0.0,
        "Weak" => 0.36,
        "Medium" => 0.68,
        _ => 1.0,
    }
}

fn trigger_for_mode(mode: &str, intensity: f64, start_position: f64) -> TriggerOutput {
    match mode {
        "off" => TriggerOutput::Off,
        "wall" => TriggerOutput::Wall {
            position: (start_position + intensity * 0.34).clamp(0.0, 1.0),
            strength: intensity,
        },
        "pulse" => TriggerOutput::Pulse {
            amplitude: intensity,
            frequency_hz: 18.0 + intensity * 42.0,
        },
        "pulse_ab" | "wall_pulse" => TriggerOutput::PulseAb {
            strength: intensity,
            frequency_hz: 18.0 + intensity * 42.0,
            wall_zones: 2,
        },
        _ => TriggerOutput::AdaptiveResistance {
            start_position,
            strength: intensity,
        },
    }
}

fn rumble_for_mode(mode: &str, intensity: f64) -> RumbleOutput {
    let intensity = clamp_unit(intensity);
    let (low, high) = match mode {
        "deep_thump" | "low" => (intensity, intensity * 0.18),
        "fine_buzz" | "high" => (intensity * 0.18, intensity),
        _ => apply_vibration_mode(mode, intensity, intensity * 0.82),
    };
    RumbleOutput {
        low_frequency: clamp_unit(low),
        high_frequency: clamp_unit(high),
    }
}

async fn fetch_latest_release_update_check() -> anyhow::Result<UpdateCheckResponse> {
    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .user_agent(format!(
            "DualSenseCommandCenter/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;
    let response = client
        .get(UPDATE_CHECK_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("GitHub Releases request failed with HTTP {status}");
    }
    let release = response.json::<GithubReleaseResponse>().await?;
    Ok(update_check_from_release(
        env!("CARGO_PKG_VERSION"),
        release,
        current_timestamp(),
    ))
}

fn update_check_from_release(
    current_version: &str,
    release: GithubReleaseResponse,
    checked_at: String,
) -> UpdateCheckResponse {
    let latest_version = normalize_release_version(&release.tag_name);
    let state = match compare_release_versions(current_version, &latest_version) {
        VersionOrdering::Older => "update_available",
        VersionOrdering::SameOrNewer => "up_to_date",
        VersionOrdering::Unknown => "unknown",
    };
    UpdateCheckResponse {
        current_version: current_version.to_string(),
        latest_version: Some(latest_version),
        release_url: Some(release.html_url),
        release_name: release.name,
        published_at: release.published_at,
        state: state.to_string(),
        checked_at: Some(checked_at),
        error: None,
        cached: false,
    }
}

fn unavailable_update_check(error: String) -> UpdateCheckResponse {
    UpdateCheckResponse {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: None,
        release_url: None,
        release_name: None,
        published_at: None,
        state: "unavailable".to_string(),
        checked_at: Some(current_timestamp()),
        error: Some(error),
        cached: false,
    }
}

fn normalize_release_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

fn compare_release_versions(current: &str, latest: &str) -> VersionOrdering {
    let Some(current) = parse_version_core(current) else {
        return VersionOrdering::Unknown;
    };
    let Some(latest) = parse_version_core(latest) else {
        return VersionOrdering::Unknown;
    };
    if current < latest {
        VersionOrdering::Older
    } else {
        VersionOrdering::SameOrNewer
    }
}

fn parse_version_core(version: &str) -> Option<Vec<u64>> {
    let normalized = normalize_release_version(version);
    let core = normalized
        .split_once(['-', '+'])
        .map(|(core, _)| core)
        .unwrap_or(normalized.as_str());
    let parts: Option<Vec<u64>> = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect();
    let mut parts = parts?;
    if parts.is_empty() {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

impl DevicePermissionProblem {
    pub fn for_controller(
        id: ControllerId,
        transport: ControllerTransportKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Some(id),
            transport: Some(transport),
            message: message.into(),
        }
    }

    pub fn global(message: impl Into<String>) -> Self {
        Self {
            id: None,
            transport: None,
            message: message.into(),
        }
    }
}

pub fn app(state: AgentState) -> Router {
    let dist = web_dist_dir();
    let static_assets =
        ServeDir::new(&dist).not_found_service(ServeFile::new(dist.join("index.html")));

    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/update-check", get(get_update_check))
        .route(
            "/api/app-settings",
            get(get_app_settings).put(update_app_settings),
        )
        .route("/api/snapshot", get(get_snapshot))
        .route("/api/controllers", get(list_controllers))
        .route(
            "/api/controllers/:id",
            get(get_controller).put(update_controller),
        )
        .route(
            "/api/controllers/:id/config",
            get(get_controller_config).put(update_controller_config),
        )
        .route("/api/controllers/:id/input", get(get_controller_input))
        .route("/api/controllers/:id/edge-profiles", get(get_edge_profiles))
        .route(
            "/api/controllers/:id/edge-profiles/:slot",
            put(write_edge_profile),
        )
        .route("/api/controllers/:id/test-effect", post(test_effect))
        .route(
            "/api/controllers/current/test-effect",
            post(test_current_effect),
        )
        .route(
            "/api/controllers/current/input",
            get(get_current_controller_input),
        )
        .route("/api/profiles", get(list_profiles).post(create_profile))
        .route("/api/profiles/import", post(import_profile))
        .route(
            "/api/profiles/:id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .route("/api/profiles/:id/config", put(update_profile_config))
        .route("/api/profiles/:id/export", get(export_profile))
        .route("/api/profiles/:id/activate", post(activate_profile))
        .route("/api/adapters", get(list_adapters))
        .route("/api/adapters/:id", put(update_adapter))
        .route("/api/steam-input", get(get_steam_input_status))
        .route(
            "/api/steam-input/bindings",
            post(update_steam_input_binding),
        )
        .route("/api/modules", get(list_modules))
        .route("/api/games/detected", get(get_detected_game))
        .route("/api/games/art/:game_id/:kind", get(get_game_art))
        .route("/api/games/steam-art/:app_id/:kind", get(get_steam_app_art))
        .route("/api/games/steam-library", get(list_steam_library))
        .route("/api/games/steam-library/browse", get(browse_steam_library))
        .route("/api/games/custom", post(add_custom_game))
        .route("/api/games/custom/:game_id", delete(remove_custom_game))
        .route("/api/effects/current", get(get_current_effect))
        .route("/api/profile-resolution", get(get_profile_resolution))
        .route(
            "/api/profile-resolution/override",
            put(set_profile_override).delete(clear_profile_override),
        )
        .route("/api/telemetry", get(list_telemetry))
        .route("/api/logs", get(list_logs))
        .route("/api/diagnostics", get(get_diagnostics))
        .route("/api/diagnostics/support-bundle", get(get_support_bundle))
        .route("/api/support-bundle", get(get_support_bundle))
        .route("/api/ws", get(ws_handler))
        .layer(middleware::from_fn(reject_cross_origin_mutations))
        .fallback_service(static_assets)
        .with_state(state)
}

pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    init_tracing();
    let listener = TcpListener::bind(addr).await?;
    let state = hid_agent_state().with_bind_addr(addr);
    for adapter in built_in_udp_adapters() {
        let bind_addr = udp_adapter_bind_addr(adapter);
        tokio::spawn(udp_telemetry_adapter_loop(
            state.clone(),
            *adapter,
            bind_addr,
        ));
    }
    #[cfg(target_os = "windows")]
    tokio::spawn(assetto_shared_memory_adapter_loop(state.clone()));
    #[cfg(not(target_os = "windows"))]
    mark_assetto_shared_memory_unavailable(&state).await;
    tokio::spawn(output_watchdog_loop(
        state.clone(),
        Duration::from_millis(250),
    ));
    tokio::spawn(hardware_output_loop(
        state.clone(),
        HARDWARE_OUTPUT_INTERVAL,
    ));
    info!(%addr, "dscc-agent listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

fn hid_agent_state() -> AgentState {
    match HidApiTransport::new() {
        Ok(transport) => {
            let output_mode = configured_output_mode();
            let transport =
                transport.with_hardware_writes_enabled(output_mode.hardware_writes_enabled());
            let output_manager =
                Arc::new(ControllerOutputManager::new(transport.clone(), output_mode));
            let config = DeviceConfig {
                output_mode,
                open_sessions: true,
                ..DeviceConfig::default()
            };
            let mut manager = DeviceManager::new(transport, config);
            match AgentState::from_device_manager_with_backend_and_storage(
                &mut manager,
                DeviceBackendSummary::hid(output_mode),
                PersistenceStore::default(),
            ) {
                Ok(state) => {
                    let state = state.with_output_manager(output_manager);
                    tokio::spawn(device_scan_loop(
                        state.clone(),
                        manager,
                        Duration::from_millis(1_000),
                    ));
                    state
                }
                Err(error) => AgentState::from_controller_events_with_backend(
                    [ControllerDiscoveryEvent::Faulted {
                        id: None,
                        message: format!("initial HID scan failed: {error}"),
                    }],
                    DeviceBackendSummary::unavailable(format!("Initial HID scan failed: {error}")),
                ),
            }
        }
        Err(error) => AgentState::from_controller_events_with_backend(
            [ControllerDiscoveryEvent::Faulted {
                id: None,
                message: format!("hidapi backend unavailable: {error}"),
            }],
            DeviceBackendSummary::unavailable(format!("hidapi backend unavailable: {error}")),
        ),
    }
}

async fn device_scan_loop<T>(
    state: AgentState,
    mut manager: DeviceManager<T>,
    scan_interval: Duration,
) where
    T: DeviceTransport,
{
    let mut interval = tokio::time::interval(scan_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match controller_events_from_device_manager(&mut manager) {
            Ok(events) => {
                for event in events {
                    state.apply_controller_event(event).await;
                }
            }
            Err(error) => {
                state
                    .apply_controller_event(ControllerDiscoveryEvent::Faulted {
                        id: None,
                        message: format!("HID scan failed: {error}"),
                    })
                    .await;
            }
        }
    }
}

async fn output_watchdog_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        if !state.hardware_output_enabled()
            || state.manual_output_override_active()
            || !state.has_non_neutral_output_frames()
        {
            continue;
        }

        let game_detection = state.cached_hardware_game_detection().await;
        let should_neutralize = {
            let inner = state.inner.read().await;
            !hardware_output_any_allowed(&inner, Some(&game_detection))
        };

        if should_neutralize {
            state
                .neutralize_active_output_and_release("the supported-game telemetry gate closed")
                .await;
        }
    }
}

async fn hardware_output_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut game_detection = state.cached_hardware_game_detection().await;
    let mut next_detection_refresh = Instant::now() + HARDWARE_GAME_DETECTION_INTERVAL;
    loop {
        interval.tick().await;
        if !state.hardware_output_enabled() || state.manual_output_override_active() {
            continue;
        }

        let now = Instant::now();
        if now >= next_detection_refresh {
            game_detection = state.cached_hardware_game_detection().await;
            next_detection_refresh = now + HARDWARE_GAME_DETECTION_INTERVAL;
        }

        if let Err(error) = state
            .write_current_output_frame_if_due(Some(&game_detection))
            .await
        {
            state
                .note_hardware_output_error(format!(
                    "Hardware trigger output write failed: {error}"
                ))
                .await;
        }
    }
}

fn udp_adapter_bind_addr(adapter: &UdpTelemetryAdapter) -> SocketAddr {
    match adapter.id {
        FORZA_DATA_OUT_ADAPTER_ID => resolve_forza_bind_addr(),
        _ => SocketAddr::from(([127, 0, 0, 1], adapter.default_port)),
    }
}

#[cfg(any(target_os = "windows", test))]
const ASSETTO_PHYSICS_MIN_LEN: usize = 120;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_GRAPHICS_MIN_LEN: usize = 12;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_STATIC_MAX_RPM_OFFSET: usize = 412;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_STATIC_MIN_LEN: usize = ASSETTO_STATIC_MAX_RPM_OFFSET + 4;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_AC_LIVE: i32 = 2;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_AC_PAUSE: i32 = 3;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_DEFAULT_MAX_RPM: f64 = 8_000.0;
#[cfg(any(target_os = "windows", test))]
const STANDARD_GRAVITY_MS2: f64 = 9.80665;

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy)]
struct AssettoSharedMemoryPages<'a> {
    physics: &'a [u8],
    graphics: Option<&'a [u8]>,
    static_page: Option<&'a [u8]>,
}

#[cfg(any(target_os = "windows", test))]
fn parse_assetto_shared_memory_pages(
    pages: AssettoSharedMemoryPages<'_>,
    sequence: u64,
) -> Option<(usize, Vec<SignalUpdate>)> {
    if pages.physics.len() < ASSETTO_PHYSICS_MIN_LEN {
        return None;
    }

    let packet_id = read_le_i32(pages.physics, 0)?;
    let throttle = finite_unit(read_le_f32(pages.physics, 4)?);
    let brake = finite_unit(read_le_f32(pages.physics, 8)?);
    let raw_gear = read_le_i32(pages.physics, 16)?;
    let rpm = finite_non_negative(f64::from(read_le_i32(pages.physics, 20)?));
    let steer_angle = finite_f64(f64::from(read_le_f32(pages.physics, 24)?));
    let speed_kmh = finite_non_negative(read_le_f32_f64(pages.physics, 28)?);
    let acceleration_x = finite_f64(read_le_f32_f64(pages.physics, 44)? * STANDARD_GRAVITY_MS2);
    let acceleration_y = finite_f64(read_le_f32_f64(pages.physics, 48)? * STANDARD_GRAVITY_MS2);
    let acceleration_z = finite_f64(read_le_f32_f64(pages.physics, 52)? * STANDARD_GRAVITY_MS2);
    let acceleration_magnitude = finite_f64(
        acceleration_x
            .mul_add(
                acceleration_x,
                acceleration_y.mul_add(acceleration_y, acceleration_z * acceleration_z),
            )
            .sqrt(),
    );
    let wheel_slip = read_f32_array_abs(pages.physics, 56, 4)?;
    let front_slip = wheel_slip[0].max(wheel_slip[1]);
    let rear_slip = wheel_slip[2].max(wheel_slip[3]);
    let wheel_slip_max = front_slip.max(rear_slip);
    let suspension_signal = signal_scaled_value(acceleration_magnitude, 2.0, 16.0);
    let surface_grip = read_le_f32(pages.physics, 116)
        .map(finite_unit)
        .filter(|value| *value > 0.0);
    let loose_surface = surface_grip.map_or(0.0, |grip| (1.0 - grip).clamp(0.0, 1.0));
    let surface_rumble = loose_surface
        .max(signal_scaled_value(acceleration_magnitude, 3.0, 22.0) * 0.55)
        .max((wheel_slip_max - 0.12).clamp(0.0, 1.0) * 0.35)
        .clamp(0.0, 1.0);
    let max_rpm = pages
        .static_page
        .and_then(|static_page| read_le_i32(static_page, ASSETTO_STATIC_MAX_RPM_OFFSET))
        .map(f64::from)
        .filter(|value| value.is_finite() && *value >= 1_000.0)
        .unwrap_or(ASSETTO_DEFAULT_MAX_RPM);
    let rpm_ratio = if max_rpm > 0.0 {
        (rpm / max_rpm).clamp(0.0, 1.25)
    } else {
        0.0
    };
    let graphics_status = pages.graphics.and_then(|graphics| read_le_i32(graphics, 4));
    let game_state =
        assetto_game_state(graphics_status, speed_kmh, rpm, throttle, brake, packet_id);

    let updates = vec![
        sequenced_signal_update("source.id", ASSETTO_SHARED_MEMORY_ADAPTER_ID, sequence),
        sequenced_signal_update("source.connected", true, sequence),
        sequenced_signal_update("source.packet_size", pages.physics.len() as f64, sequence),
        sequenced_signal_update("game.state", game_state, sequence),
        sequenced_signal_update("vehicle.max_rpm", max_rpm, sequence),
        sequenced_signal_update("vehicle.rpm", rpm, sequence),
        sequenced_signal_update("vehicle.rpm_ratio", rpm_ratio, sequence),
        sequenced_signal_update("vehicle.speed_kmh", speed_kmh, sequence),
        sequenced_signal_update("vehicle.acceleration.x", acceleration_x, sequence),
        sequenced_signal_update("vehicle.acceleration.y", acceleration_y, sequence),
        sequenced_signal_update("vehicle.acceleration.z", acceleration_z, sequence),
        sequenced_signal_update(
            "vehicle.acceleration.magnitude",
            acceleration_magnitude,
            sequence,
        ),
        sequenced_signal_update("input.throttle", throttle, sequence),
        sequenced_signal_update("input.brake", brake, sequence),
        sequenced_signal_update("input.clutch", 0.0, sequence),
        sequenced_signal_update("input.handbrake", 0.0, sequence),
        sequenced_signal_update("input.steer", assetto_steer_unit(steer_angle), sequence),
        sequenced_signal_update("drivetrain.gear", assetto_display_gear(raw_gear), sequence),
        sequenced_signal_update("wheel.slip.front_left", wheel_slip[0], sequence),
        sequenced_signal_update("wheel.slip.front_right", wheel_slip[1], sequence),
        sequenced_signal_update("wheel.slip.rear_left", wheel_slip[2], sequence),
        sequenced_signal_update("wheel.slip.rear_right", wheel_slip[3], sequence),
        sequenced_signal_update("wheel.slip.front_max", front_slip, sequence),
        sequenced_signal_update("wheel.slip.rear_max", rear_slip, sequence),
        sequenced_signal_update("wheel.slip.max", wheel_slip_max, sequence),
        sequenced_signal_update("tire.slip_ratio.max", wheel_slip_max, sequence),
        sequenced_signal_update("tire.slip_angle.max", wheel_slip_max * 0.65, sequence),
        sequenced_signal_update("surface.rumble.max", surface_rumble, sequence),
        sequenced_signal_update("surface.rumble_strip.max", surface_rumble * 0.35, sequence),
        sequenced_signal_update("surface.puddle.max", 0.0, sequence),
        sequenced_signal_update("suspension.travel.max", suspension_signal, sequence),
    ];

    Some((pages.physics.len(), updates))
}

#[cfg(any(target_os = "windows", test))]
fn assetto_game_state(
    graphics_status: Option<i32>,
    speed_kmh: f64,
    rpm: f64,
    throttle: f64,
    brake: f64,
    packet_id: i32,
) -> &'static str {
    match graphics_status {
        Some(ASSETTO_AC_LIVE) => "driving",
        Some(ASSETTO_AC_PAUSE) => "paused",
        Some(_) => "menu",
        None if speed_kmh > 1.0 || rpm > 500.0 || throttle > 0.01 || brake > 0.01 => "driving",
        None if packet_id > 0 => "menu",
        None => "menu",
    }
}

#[cfg(any(target_os = "windows", test))]
fn assetto_display_gear(raw_gear: i32) -> f64 {
    f64::from(raw_gear.saturating_sub(1).max(0))
}

#[cfg(any(target_os = "windows", test))]
fn assetto_steer_unit(steer_angle: f64) -> f64 {
    (steer_angle / 0.75).clamp(-1.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn read_f32_array_abs(packet: &[u8], offset: usize, count: usize) -> Option<Vec<f64>> {
    (0..count)
        .map(|index| read_le_f32_f64(packet, offset + index * 4).map(|value| value.abs()))
        .collect()
}

fn signal_scaled_value(value: f64, input_min: f64, input_max: f64) -> f64 {
    if input_min >= input_max {
        return 0.0;
    }
    ((value - input_min) / (input_max - input_min)).clamp(0.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_unit(value: f32) -> f64 {
    finite_f64(f64::from(value)).clamp(0.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_non_negative(value: f64) -> f64 {
    finite_f64(value).max(0.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_f64(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(any(target_os = "windows", test))]
fn read_le_bytes<const N: usize>(packet: &[u8], offset: usize) -> Option<[u8; N]> {
    packet.get(offset..offset + N)?.try_into().ok()
}

#[cfg(any(target_os = "windows", test))]
fn read_le_i32(packet: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(read_le_bytes(packet, offset)?))
}

#[cfg(any(target_os = "windows", test))]
fn read_le_f32(packet: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(read_le_bytes(packet, offset)?))
}

#[cfg(any(target_os = "windows", test))]
fn read_le_f32_f64(packet: &[u8], offset: usize) -> Option<f64> {
    Some(finite_f64(f64::from(read_le_f32(packet, offset)?)))
}

fn sequenced_signal_update(
    name: &str,
    value: impl Into<SignalValue>,
    sequence: u64,
) -> SignalUpdate {
    signal_update(name, value).with_sequence(sequence)
}

#[cfg(target_os = "windows")]
type AssettoSharedMemoryPageBuffers = (Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>);

#[cfg(target_os = "windows")]
fn read_assetto_shared_memory_snapshot(
    sequence: u64,
) -> io::Result<Option<(usize, Vec<SignalUpdate>)>> {
    let Some((physics, graphics, static_page)) = read_assetto_shared_memory_pages()? else {
        return Ok(None);
    };
    Ok(parse_assetto_shared_memory_pages(
        AssettoSharedMemoryPages {
            physics: &physics,
            graphics: graphics.as_deref(),
            static_page: static_page.as_deref(),
        },
        sequence,
    ))
}

#[cfg(target_os = "windows")]
fn read_assetto_shared_memory_pages() -> io::Result<Option<AssettoSharedMemoryPageBuffers>> {
    let page_sets = [
        (
            "Local\\acpmf_physics",
            "Local\\acpmf_graphics",
            "Local\\acpmf_static",
        ),
        (
            "Local\\acevo_pmf_physics",
            "Local\\acevo_pmf_graphics",
            "Local\\acevo_pmf_static",
        ),
    ];

    for (physics_name, graphics_name, static_name) in page_sets {
        let Some(physics) = read_windows_shared_memory_page(physics_name, ASSETTO_PHYSICS_MIN_LEN)?
        else {
            continue;
        };
        let graphics = read_windows_shared_memory_page(graphics_name, ASSETTO_GRAPHICS_MIN_LEN)?;
        let static_page = read_windows_shared_memory_page(static_name, ASSETTO_STATIC_MIN_LEN)?;
        return Ok(Some((physics, graphics, static_page)));
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
fn read_windows_shared_memory_page(
    name: &str,
    bytes_to_read: usize,
) -> io::Result<Option<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, HANDLE},
        System::Memory::{
            MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
            MEMORY_MAPPED_VIEW_ADDRESS,
        },
    };

    struct MappingHandle(HANDLE);

    impl Drop for MappingHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct MappingView(MEMORY_MAPPED_VIEW_ADDRESS);

    impl Drop for MappingView {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(self.0);
            }
        }
    }

    let mut wide = name.encode_utf16().collect::<Vec<_>>();
    wide.push(0);

    let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, wide.as_ptr()) };
    if handle.is_null() {
        let error = unsafe { GetLastError() };
        if error == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let handle = MappingHandle(handle);

    let view = unsafe { MapViewOfFile(handle.0, FILE_MAP_READ, 0, 0, bytes_to_read) };
    if view.Value.is_null() {
        let error = unsafe { GetLastError() };
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let view = MappingView(view);

    let bytes = unsafe { std::slice::from_raw_parts(view.0.Value.cast::<u8>(), bytes_to_read) };
    let mut owned = vec![0_u8; bytes_to_read];
    owned.copy_from_slice(bytes);
    Ok(Some(owned))
}

#[cfg(target_os = "windows")]
async fn assetto_shared_memory_adapter_loop(state: AgentState) {
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .mark_ready();
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: "Assetto shared-memory reader ready".to_string(),
            timestamp: current_timestamp(),
        });
    }

    let mut interval = tokio::time::interval(SHARED_MEMORY_TELEMETRY_PROCESS_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sequence = 0_u64;
    loop {
        interval.tick().await;
        sequence = sequence.saturating_add(1);
        let result =
            tokio::task::spawn_blocking(move || read_assetto_shared_memory_snapshot(sequence))
                .await;
        match result {
            Ok(Ok(Some((packet_len, updates)))) => {
                state
                    .apply_adapter_packet(
                        ASSETTO_SHARED_MEMORY_ADAPTER_ID,
                        packet_len,
                        sequence,
                        updates,
                    )
                    .await;
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let mut inner = state.inner.write().await;
                inner
                    .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                    .last_error = Some(error.to_string());
            }
            Err(error) => {
                let mut inner = state.inner.write().await;
                inner
                    .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                    .last_error = Some(error.to_string());
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
async fn mark_assetto_shared_memory_unavailable(state: &AgentState) {
    let mut inner = state.inner.write().await;
    inner
        .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        .mark_bind_error(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            "Assetto shared-memory telemetry is currently available on Windows only.",
        );
}

async fn udp_telemetry_adapter_loop(
    state: AgentState,
    adapter: UdpTelemetryAdapter,
    bind_addr: SocketAddr,
) {
    let socket = match UdpSocket::bind(bind_addr).await {
        Ok(socket) => socket,
        Err(error) => {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(adapter.id)
                .mark_bind_error(bind_addr, error.to_string());
            inner.logs.push(LogEntry {
                level: "warn".to_string(),
                message: format!(
                    "{} listener could not bind {bind_addr}: {error}",
                    adapter.display_name
                ),
                timestamp: current_timestamp(),
            });
            return;
        }
    };

    {
        let mut inner = state.inner.write().await;
        inner.adapter_runtime_mut(adapter.id).mark_bound(bind_addr);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("{} listener ready on {bind_addr}", adapter.display_name),
            timestamp: current_timestamp(),
        });
    }

    let mut sequence = 0_u64;
    let mut buffer = [0_u8; 512];
    let mut last_processed_at: Option<Instant> = None;
    loop {
        match socket.recv_from(&mut buffer).await {
            Ok((len, _source)) => {
                sequence = sequence.saturating_add(1);
                let now = Instant::now();
                if last_processed_at
                    .is_some_and(|last| now.duration_since(last) < UDP_TELEMETRY_PROCESS_INTERVAL)
                {
                    continue;
                }
                last_processed_at = Some(now);
                if let Some(parsed) =
                    parse_udp_telemetry_packet(adapter.id, &buffer[..len], sequence)
                {
                    state
                        .apply_adapter_packet(
                            parsed.adapter_id,
                            parsed.packet_len,
                            sequence,
                            parsed.updates,
                        )
                        .await;
                } else {
                    let mut inner = state.inner.write().await;
                    inner
                        .adapter_runtime_mut(adapter.id)
                        .mark_parse_error(len, sequence);
                }
            }
            Err(error) => {
                let mut inner = state.inner.write().await;
                inner.adapter_runtime_mut(adapter.id).last_error = Some(error.to_string());
                inner.logs.push(LogEntry {
                    level: "warn".to_string(),
                    message: format!("{} listener read failed: {error}", adapter.display_name),
                    timestamp: current_timestamp(),
                });
            }
        }
    }
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dscc_agent=info,tower_http=info".into()),
        )
        .try_init();
}

pub fn app_paths() -> Option<AppPaths> {
    ProjectDirs::from("dev", "DualSenseCommand", "DualSenseCommandCenter").map(|dirs| AppPaths {
        config_dir: dirs.config_dir().display().to_string(),
        data_dir: dirs.data_dir().display().to_string(),
        log_dir: dirs.cache_dir().join("logs").display().to_string(),
    })
}

fn web_dist_dir() -> PathBuf {
    let current_exe = std::env::current_exe().ok();
    let current_dir = std::env::current_dir().ok();
    web_dist_dir_from_parts(
        configured_web_dist_dir(),
        current_exe.as_deref(),
        current_dir.as_deref(),
    )
}

fn configured_web_dist_dir() -> Option<PathBuf> {
    std::env::var_os("DSCC_WEB_DIST")
        .or_else(|| std::env::var_os("DSCC_WEB_DIST_DIR"))
        .map(PathBuf::from)
}

fn web_dist_dir_from_parts(
    configured: Option<PathBuf>,
    executable: Option<&FsPath>,
    current_dir: Option<&FsPath>,
) -> PathBuf {
    if let Some(path) = configured {
        return path;
    }

    let candidates = web_dist_candidates(executable, current_dir);
    candidates
        .iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .cloned()
        .unwrap_or_else(|| {
            candidates
                .into_iter()
                .next()
                .unwrap_or_else(|| PathBuf::from("web/dist"))
        })
}

fn web_dist_candidates(executable: Option<&FsPath>, current_dir: Option<&FsPath>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(current_dir) = current_dir {
        candidates.push(current_dir.join("web").join("dist"));
    }
    if let Some(executable_dir) = executable.and_then(FsPath::parent) {
        candidates.push(executable_dir.join("web").join("dist"));
        candidates.push(executable_dir.join("dist"));
        if let Some(install_parent) = executable_dir.parent() {
            candidates.push(install_parent.join("web").join("dist"));
        }
    }
    candidates.push(PathBuf::from("web/dist"));
    candidates
}

async fn get_status(State(state): State<AgentState>) -> Json<StatusResponse> {
    let game_detection = state.cached_game_detection().await;
    Json(state.status_with_detection(Some(&game_detection)).await)
}

async fn get_update_check(State(state): State<AgentState>) -> Json<UpdateCheckResponse> {
    Json(state.update_check().await)
}

async fn get_app_settings(State(state): State<AgentState>) -> Json<AppSettingsResponse> {
    let inner = state.inner.read().await;
    Json(state.app_settings_response(&inner.app_settings))
}

async fn update_app_settings(
    State(state): State<AgentState>,
    Json(request): Json<UpdateAppSettingsRequest>,
) -> Result<Json<AppSettingsResponse>, (StatusCode, String)> {
    if request.listen_on_all_interfaces == Some(true) && !lan_api_enabled() {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "LAN API access requires explicit opt-in. Set {LAN_API_ENABLE_ENV}=1 before enabling all-interface binding."
            ),
        ));
    }

    let glyph_result = if let Some(glyphs) = request.forza_playstation_glyphs.clone() {
        let persisted_install_path = {
            let inner = state.inner.read().await;
            inner
                .app_settings
                .forza_playstation_glyphs
                .install_path
                .clone()
        };
        let configured_path = glyphs
            .install_path
            .as_deref()
            .or(persisted_install_path.as_deref())
            .map(|path| resolve_forza_horizon6_install_path(Some(path)));
        let steam_path = supported_game_install_path(
            &state.cached_steam_game_catalog().await,
            "forza-horizon-6",
        );
        let install_path = trusted_forza_horizon6_install_path(configured_path, steam_path);
        let requested_enabled = glyphs.enabled;
        let path_for_task = install_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            if requested_enabled {
                install_forza_playstation_glyphs(path_for_task)
            } else {
                restore_forza_original_glyphs(path_for_task)
            }
        })
        .await
        .map_err(|error| format!("glyph installer task failed: {error}"))
        .and_then(|result| result.map_err(|error| error.to_string()));
        Some((requested_enabled, install_path, result))
    } else {
        None
    };

    let (response, to_save) = {
        let mut inner = state.inner.write().await;
        let mut settings = inner.app_settings.clone();
        if let Some(listen) = request.listen_on_all_interfaces {
            settings.listen_on_all_interfaces = listen;
        }
        if let Some((requested_enabled, install_path, result)) = glyph_result {
            settings.forza_playstation_glyphs.install_path =
                Some(install_path.display().to_string());
            match result {
                Ok(message) => {
                    settings.forza_playstation_glyphs.enabled = requested_enabled;
                    settings.forza_playstation_glyphs.last_status = if requested_enabled {
                        "installed".to_string()
                    } else {
                        "restored".to_string()
                    };
                    settings.forza_playstation_glyphs.last_message = message;
                }
                Err(message) => {
                    settings.forza_playstation_glyphs.last_status = "error".to_string();
                    settings.forza_playstation_glyphs.last_message = message;
                }
            }
        }
        inner.app_settings = settings.clone();
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: "Application settings updated".to_string(),
            timestamp: current_timestamp(),
        });
        (
            state.app_settings_response(&settings),
            build_persist_snapshot(&inner),
        )
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("app-settings-updated".to_string()),
    });
    Ok(Json(response))
}

async fn get_snapshot(State(state): State<AgentState>) -> Json<AgentSnapshotResponse> {
    Json(state.snapshot().await)
}

async fn list_controllers(State(state): State<AgentState>) -> Json<Vec<ControllerSummary>> {
    let inner = state.inner.read().await;
    Json(apply_controller_names(
        inner.controllers.summaries(),
        &inner.controller_names,
    ))
}

async fn get_controller(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerDetail>, StatusCode> {
    let inner = state.inner.read().await;
    inner
        .controllers
        .detail(&id)
        .map(|detail| apply_controller_name(detail, &inner.controller_names))
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_controller(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateControllerRequest>,
) -> Result<Json<ControllerDetail>, StatusCode> {
    let name = normalize_controller_display_name(&request.name).ok_or(StatusCode::BAD_REQUEST)?;
    let (detail, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        inner.controller_names.insert(id.clone(), name.clone());
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Controller {id} renamed to {name}"),
            timestamp: current_timestamp(),
        });
        (
            apply_controller_name(detail, &inner.controller_names),
            build_persist_snapshot(&inner),
        )
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("controller-renamed".to_string()),
    });
    Ok(Json(detail))
}

async fn get_controller_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerConfig>, StatusCode> {
    let (config, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let active_profile_config = inner
            .active_profile_id
            .as_deref()
            .and_then(|profile_id| inner.profile_configs.get(profile_id))
            .cloned();
        let model = detail.model;
        let config = inner
            .controller_configs
            .entry(id.clone())
            .or_insert_with(|| {
                let mut config = ControllerConfig::default_for(id, model);
                if let Some(profile_config) = active_profile_config.as_ref() {
                    profile_config.apply_to_controller_config(&mut config);
                }
                config
            })
            .clone()
            .normalized();
        inner
            .controller_configs
            .insert(config.controller_id.clone(), config.clone());
        (config, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(config))
}

async fn update_controller_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateControllerConfigRequest>,
) -> Result<Json<ControllerConfig>, StatusCode> {
    let (config, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let config = ControllerConfig::from_update(id.clone(), detail.model, request);
        inner.controller_configs.insert(id.clone(), config.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Configuration saved for controller {id}"),
            timestamp: current_timestamp(),
        });
        (config, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(config))
}

struct EdgeHardwareProfilesRead {
    profiles: Vec<EdgeOnboardProfile>,
    hardware_writes_enabled: bool,
}

enum EdgeHardwareProfileWriteResult {
    Written,
    StagedOnly(String),
    Failed(String),
}

async fn read_edge_profiles_from_hardware(
    state: &AgentState,
    controller_id: &str,
) -> Result<EdgeHardwareProfilesRead, String> {
    let manager = state
        .output_manager
        .clone()
        .ok_or_else(|| "HID output manager is unavailable".to_string())?;
    let target = {
        let inner = state.inner.read().await;
        controller_output_target_or_reason(&inner, controller_id)?
    };
    if !edge_onboard_transport_supported(target.transport) {
        return Err(
            "DualSense Edge onboard profile reads require USB or Bluetooth HID feature report access"
                .to_string(),
        );
    }

    let hardware_writes_enabled = manager.hardware_writes_enabled();
    let profiles = tokio::task::spawn_blocking(move || manager.read_edge_onboard_profiles(&target))
        .await
        .map_err(|error| format!("DualSense Edge profile read task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    Ok(EdgeHardwareProfilesRead {
        profiles,
        hardware_writes_enabled,
    })
}

async fn write_edge_profile_to_hardware(
    state: &AgentState,
    controller_id: &str,
    profile: EdgeOnboardProfile,
) -> EdgeHardwareProfileWriteResult {
    let Some(manager) = state.output_manager.clone() else {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "HID output manager is unavailable".to_string(),
        );
    };
    if !manager.hardware_writes_enabled() {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "hardware writes are disabled by DSCC output mode".to_string(),
        );
    }

    let target = {
        let inner = state.inner.read().await;
        match controller_output_target_or_reason(&inner, controller_id) {
            Ok(target) => target,
            Err(error) => return EdgeHardwareProfileWriteResult::StagedOnly(error),
        }
    };
    if !edge_onboard_transport_supported(target.transport) {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "DualSense Edge onboard profile writes require USB or Bluetooth HID feature report access"
                .to_string(),
        );
    }

    match tokio::task::spawn_blocking(move || manager.write_edge_onboard_profile(&target, &profile))
        .await
    {
        Ok(Ok(())) => EdgeHardwareProfileWriteResult::Written,
        Ok(Err(error)) => EdgeHardwareProfileWriteResult::Failed(error.to_string()),
        Err(error) => EdgeHardwareProfileWriteResult::Failed(format!(
            "DualSense Edge profile write task failed: {error}"
        )),
    }
}

async fn get_edge_profiles(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<EdgeProfilesResponse>, StatusCode> {
    let (detail, store, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let snapshot = if let Some(store) = inner.edge_profiles.remove(&id) {
            inner.edge_profiles.insert(id.clone(), store.normalized());
            build_persist_snapshot(&inner)
        } else {
            None
        };
        (detail, inner.edge_profiles.get(&id).cloned(), snapshot)
    };
    persist_snapshot(&state, to_save).await;

    if detail.model != "DualSense Edge" {
        return Ok(Json(EdgeProfilesResponse::for_controller(
            &detail,
            store.as_ref(),
        )));
    }

    let response = match read_edge_profiles_from_hardware(&state, &id).await {
        Ok(hardware) => EdgeProfilesResponse::for_controller_with_hardware(
            &detail,
            store.as_ref(),
            Some(&hardware.profiles),
            None,
            hardware.hardware_writes_enabled,
        ),
        Err(error) => EdgeProfilesResponse::for_controller_with_hardware(
            &detail,
            store.as_ref(),
            None,
            Some(error),
            false,
        ),
    };
    Ok(Json(response))
}

async fn write_edge_profile(
    Path((id, slot)): Path<(String, String)>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateEdgeProfileRequest>,
) -> Result<(StatusCode, Json<ActionAccepted>), StatusCode> {
    let Some(slot_id) = edge_slot_id_from_api(&slot).filter(|slot| slot.assignable()) else {
        return Ok((
            StatusCode::CONFLICT,
            Json(ActionAccepted {
                accepted: false,
                message: "Only Fn + Circle, Fn + Cross, and Fn + Square are editable Edge slots."
                    .to_string(),
                dry_run: Some(true),
            }),
        ));
    };

    let detail = {
        let inner = state.inner.read().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?
    };
    let response = EdgeProfilesResponse::for_controller(&detail, None);
    if response.support_state == EdgeProfileSupportState::Unsupported {
        return Ok((
            StatusCode::CONFLICT,
            Json(ActionAccepted {
                accepted: false,
                message: format!(
                    "Edge onboard profile slot {slot} was not written. {}",
                    response.warning
                ),
                dry_run: Some(true),
            }),
        ));
    }

    let mut config = edge_profile_config_from_request(request);
    let hardware_profile = edge_profile_from_slot_config(slot_id, &config);
    let write_result = write_edge_profile_to_hardware(&state, &id, hardware_profile).await;
    let (status, message, dry_run, level) = match write_result {
        EdgeHardwareProfileWriteResult::Written => {
            config.hardware_synced = true;
            (
                StatusCode::ACCEPTED,
                format!("Wrote Edge slot {slot} to controller memory for {id}"),
                false,
                "info",
            )
        }
        EdgeHardwareProfileWriteResult::StagedOnly(reason) => {
            config.hardware_synced = false;
            (
                StatusCode::ACCEPTED,
                format!("Staged Edge slot {slot} for controller {id}; no hardware write was attempted: {reason}"),
                true,
                "info",
            )
        }
        EdgeHardwareProfileWriteResult::Failed(error) => {
            config.hardware_synced = false;
            (
                StatusCode::CONFLICT,
                format!("Staged Edge slot {slot} locally, but the hardware write failed: {error}"),
                false,
                "warn",
            )
        }
    };

    let to_save = {
        let mut inner = state.inner.write().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        inner
            .edge_profiles
            .entry(id.clone())
            .or_default()
            .slots
            .insert(slot.clone(), config);
        inner.logs.push(LogEntry {
            level: level.to_string(),
            message: message.clone(),
            timestamp: current_timestamp(),
        });
        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;

    Ok((
        status,
        Json(ActionAccepted {
            accepted: status == StatusCode::ACCEPTED,
            message,
            dry_run: Some(dry_run),
        }),
    ))
}

async fn get_controller_input(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerInputResponse>, StatusCode> {
    Ok(Json(read_controller_input_state(id, state).await?))
}

async fn get_current_controller_input(
    State(state): State<AgentState>,
) -> Result<Json<ControllerInputResponse>, StatusCode> {
    let id = {
        let inner = state.inner.read().await;
        inner
            .controllers
            .summaries()
            .into_iter()
            .find(|controller| controller.connected)
            .map(|controller| controller.id)
            .ok_or(StatusCode::NOT_FOUND)?
    };

    Ok(Json(read_controller_input_state(id, state).await?))
}

async fn read_controller_input_state(
    id: String,
    state: AgentState,
) -> Result<ControllerInputResponse, StatusCode> {
    {
        let inner = state.inner.read().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
    }

    match state.read_input_state_for_controller(&id).await {
        Ok(Some(input)) => Ok(controller_input_available(id, input)),
        Ok(None) => Ok(controller_input_unavailable(
            id,
            "hid",
            "No fresh DualSense input report was available".to_string(),
        )),
        Err(error) => Ok(controller_input_unavailable(
            id,
            "hid",
            format!("DualSense input read failed: {error}"),
        )),
    }
}

fn controller_input_available(
    controller_id: String,
    input: ControllerInputState,
) -> ControllerInputResponse {
    ControllerInputResponse {
        controller_id,
        available: true,
        source: "hid".to_string(),
        message: "Live DualSense trigger input is available".to_string(),
        l2: input.l2,
        r2: input.r2,
    }
}

fn controller_input_unavailable(
    controller_id: String,
    source: &str,
    message: String,
) -> ControllerInputResponse {
    ControllerInputResponse {
        controller_id,
        available: false,
        source: source.to_string(),
        message,
        l2: 0.0,
        r2: 0.0,
    }
}

async fn test_effect(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    run_effect_test_for_controller(id, state, request).await
}

async fn test_current_effect(
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    let id = {
        let inner = state.inner.read().await;
        inner
            .controllers
            .summaries()
            .into_iter()
            .find(|controller| controller.connected)
            .map(|controller| controller.id)
            .ok_or(StatusCode::NOT_FOUND)?
    };

    run_effect_test_for_controller(id, state, request).await
}

async fn run_effect_test_for_controller(
    id: String,
    state: AgentState,
    request: EffectTestRequest,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;

        if detail.permission == ControllerPermissionState::Denied {
            return Ok((
                StatusCode::CONFLICT,
                Json(EffectTestResponse {
                    accepted: false,
                    message: format!(
                        "Controller {id} requires device permission before effect tests"
                    ),
                    dry_run: true,
                    duration_ms: 0,
                    output: ControllerOutputFrame::default(),
                }),
            ));
        }
    }

    let target = request.target.as_deref().unwrap_or("r2").to_string();
    let mode = request
        .mode
        .as_deref()
        .unwrap_or("adaptive_resistance")
        .to_string();
    let stop_manual_override = target == "base_feel" && mode == "off";
    let duration_ms = if stop_manual_override {
        0
    } else if target == "base_feel" {
        request
            .duration_ms
            .unwrap_or(DEFAULT_BASE_FEEL_TEST_DURATION_MS)
            .clamp(500, MAX_BASE_FEEL_TEST_DURATION_MS)
    } else {
        request
            .duration_ms
            .unwrap_or(DEFAULT_EFFECT_TEST_DURATION_MS)
            .clamp(100, MAX_EFFECT_TEST_DURATION_MS)
    };
    let output = if stop_manual_override {
        ControllerOutputFrame::default()
    } else {
        effect_test_output_frame(&request)
    };
    let base_feel_trigger = if target == "base_feel" && !stop_manual_override {
        Some(request.trigger.clone().unwrap_or_default())
    } else {
        None
    };
    let hardware_output_enabled = state.hardware_output_enabled();
    let mut accepted = true;
    let mut status = StatusCode::ACCEPTED;
    let mut message = if hardware_output_enabled {
        if stop_manual_override {
            state.clear_manual_output_override();
        }
        let generation = if stop_manual_override {
            None
        } else {
            Some(state.begin_manual_output_override(Duration::from_millis(duration_ms)))
        };
        match state.write_output_frame_to_controller(&id, &output).await {
            Ok(write) => {
                if let Some(generation) = generation {
                    let state_for_reset = state.clone();
                    let id_for_reset = id.clone();
                    let output_for_refresh = output.clone();
                    let base_feel_trigger = base_feel_trigger.clone();
                    tokio::spawn(async move {
                        let deadline = Instant::now() + Duration::from_millis(duration_ms);
                        let refresh_interval = if base_feel_trigger.is_some() {
                            BASE_FEEL_OUTPUT_REFRESH_INTERVAL
                        } else {
                            MANUAL_OUTPUT_REFRESH_INTERVAL
                        };
                        loop {
                            let now = Instant::now();
                            if now >= deadline {
                                break;
                            }
                            let sleep_for =
                                refresh_interval.min(deadline.saturating_duration_since(now));
                            tokio::time::sleep(sleep_for).await;
                            if !state_for_reset.manual_output_override_active_for(generation) {
                                if !state_for_reset
                                    .manual_output_override_generation_matches(generation)
                                {
                                    return;
                                }
                                break;
                            }
                            if Instant::now() >= deadline {
                                break;
                            }
                            let output_for_refresh = if let Some(trigger_config) =
                                base_feel_trigger.as_ref()
                            {
                                match state_for_reset
                                    .read_input_state_for_controller(&id_for_reset)
                                    .await
                                {
                                    Ok(Some(input)) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        Some(input.l2),
                                        Some(input.r2),
                                    ),
                                    Ok(None) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        None,
                                        None,
                                    ),
                                    Err(error) => {
                                        state_for_reset
                                                .note_hardware_output_error(format!(
                                                    "Hardware effect test input read for controller {id_for_reset} failed: {error}"
                                                ))
                                                .await;
                                        output_for_refresh.clone()
                                    }
                                }
                            } else {
                                output_for_refresh.clone()
                            };
                            if let Err(error) = state_for_reset
                                .write_output_frame_to_controller(
                                    &id_for_reset,
                                    &output_for_refresh,
                                )
                                .await
                            {
                                state_for_reset
                                    .note_hardware_output_error(format!(
                                        "Hardware effect test refresh for controller {id_for_reset} failed: {error}"
                                    ))
                                    .await;
                                break;
                            }
                        }

                        if state_for_reset.manual_output_override_generation_matches(generation) {
                            let _ = state_for_reset
                                .write_output_frame_to_controller(
                                    &id_for_reset,
                                    &ControllerOutputFrame::default(),
                                )
                                .await;
                            state_for_reset
                                .release_output_session_for_controller(&id_for_reset)
                                .await;
                            state_for_reset.clear_manual_output_override_if_generation(generation);
                        }
                    });
                    format!(
                        "Queued hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                } else {
                    state.release_output_session_for_controller(&id).await;
                    format!(
                        "Stopped hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                }
            }
            Err(error) => {
                if !stop_manual_override {
                    state.clear_manual_output_override();
                } else {
                    state.release_output_session_for_controller(&id).await;
                }
                accepted = false;
                status = StatusCode::CONFLICT;
                format!("Hardware effect test for controller {id} was blocked: {error}")
            }
        }
    } else {
        format!("Queued effect test preview for controller {id}")
    };

    {
        let mut inner = state.inner.write().await;
        inner.logs.push(LogEntry {
            level: if accepted { "info" } else { "warn" }.to_string(),
            message: format!("{}: target={} mode={}", message, target, mode),
            timestamp: current_timestamp(),
        });
    }

    if !accepted && message.is_empty() {
        message = format!("Hardware effect test for controller {id} was blocked");
    }

    Ok((
        status,
        Json(EffectTestResponse {
            accepted,
            message,
            dry_run: !hardware_output_enabled,
            duration_ms,
            output,
        }),
    ))
}

async fn list_profiles(State(state): State<AgentState>) -> Json<Vec<ProfileSummary>> {
    let inner = state.inner.read().await;
    Json(inner.profiles.clone())
}

async fn create_profile(
    State(state): State<AgentState>,
    Json(request): Json<CreateProfileRequest>,
) -> impl IntoResponse {
    let (profile, to_save) = {
        let mut inner = state.inner.write().await;
        let id = slugify(&request.name);
        let game_id = normalize_optional_profile_game_id(request.game_id);
        if inner.profiles.iter().any(|profile| profile.id == id) {
            return (
                StatusCode::CONFLICT,
                Json(ProfileSummary {
                    id,
                    name: request.name,
                    built_in: false,
                    active: false,
                    game_id,
                }),
            );
        }

        let profile = ProfileSummary {
            id,
            name: request.name,
            built_in: false,
            active: false,
            game_id,
        };
        inner.profiles.push(profile.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        (profile, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    (StatusCode::CREATED, Json(profile))
}

async fn get_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ProfileSummary>, StatusCode> {
    let inner = state.inner.read().await;
    inner
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn export_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ExportedProfile>, StatusCode> {
    let inner = state.inner.read().await;
    let profile = inner
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ExportedProfile {
        schema: "dev.dscc.profile.v1".to_string(),
        config: inner.profile_configs.get(&profile.id).cloned(),
        id: profile.id,
        name: profile.name,
        built_in: profile.built_in,
        active: profile.active,
        game_id: profile.game_id,
    }))
}

async fn import_profile(
    State(state): State<AgentState>,
    Json(request): Json<ImportProfileRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if request.schema != "dev.dscc.profile.v1" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (profile, to_save) = {
        let mut inner = state.inner.write().await;
        let mut id = request.id.unwrap_or_else(|| slugify(&request.name));
        let game_id = normalize_optional_profile_game_id(request.game_id);
        if id.trim().is_empty() {
            id = slugify(&request.name);
        }
        if inner.profiles.iter().any(|profile| profile.id == id) {
            return Ok((
                StatusCode::CONFLICT,
                Json(ProfileSummary {
                    id,
                    name: request.name,
                    built_in: false,
                    active: false,
                    game_id,
                }),
            ));
        }

        let profile = ProfileSummary {
            id,
            name: request.name,
            built_in: false,
            active: false,
            game_id,
        };
        if let Some(config) = request.config {
            inner.profile_configs.insert(profile.id.clone(), config);
        }
        inner.profiles.push(profile.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        (profile, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<ProfileSummary>, StatusCode> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (updated, to_save) = {
        let mut inner = state.inner.write().await;
        let profile_index = inner
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        if inner.profiles[profile_index].built_in {
            return Err(StatusCode::FORBIDDEN);
        }

        if inner
            .profiles
            .iter()
            .any(|profile| profile.id != id && profile.name.trim().eq_ignore_ascii_case(name))
        {
            return Err(StatusCode::CONFLICT);
        }

        inner.profiles[profile_index].name = name.to_string();
        let updated = inner.profiles[profile_index].clone();
        for config in inner.controller_configs.values_mut() {
            for assignment in &mut config.profile_assignments {
                if assignment.profile_id == id {
                    assignment.profile_name = updated.name.clone();
                }
            }
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Renamed profile {}", updated.name),
            timestamp: current_timestamp(),
        });
        (updated, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-renamed".to_string()),
    });
    Ok(Json(updated))
}

async fn update_profile_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateProfileConfigRequest>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let model_hint = request
        .model
        .clone()
        .unwrap_or_else(|| model_hint_for_profile_buttons(&request.buttons).to_string());
    let profile_config = ProfileConfig {
        input_mode: request.input_mode,
        trigger: request.trigger,
        lightbar: request.lightbar,
        forza: request.forza,
        sticks: request.sticks,
        buttons: request.buttons,
    }
    .normalized_for_model(&model_hint);
    let (profile_name, to_save) = {
        let mut inner = state.inner.write().await;
        let profile_name = inner
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| {
                if profile.built_in {
                    None
                } else {
                    Some(profile.name.clone())
                }
            })
            .ok_or(StatusCode::NOT_FOUND)?;
        let Some(profile_name) = profile_name else {
            return Err(StatusCode::FORBIDDEN);
        };

        inner
            .profile_configs
            .insert(id.clone(), profile_config.clone());
        if inner.active_profile_id.as_deref() == Some(id.as_str())
            || inner.auto_loaded_profile_id.as_deref() == Some(id.as_str())
        {
            apply_profile_config_to_controllers(
                &mut inner,
                &SelectedProfileConfig::Full(profile_config.clone()),
            );
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Profile settings saved for {profile_name}"),
            timestamp: current_timestamp(),
        });
        (profile_name, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-config-saved".to_string()),
    });

    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Saved profile {profile_name}"),
        dry_run: None,
    }))
}

async fn delete_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let (deleted_name, to_save) = {
        let mut inner = state.inner.write().await;
        let profile = inner
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        if profile.built_in {
            return Err(StatusCode::FORBIDDEN);
        }
        let deleted_name = profile.name.clone();

        inner.profiles.retain(|profile| profile.id != id);
        inner.profile_configs.remove(&id);
        inner
            .profile_overrides
            .retain(|_, override_profile| override_profile.profile_id != id);
        for config in inner.controller_configs.values_mut() {
            config
                .profile_assignments
                .retain(|assignment| assignment.profile_id != id);
        }
        if inner.active_profile_id.as_deref() == Some(id.as_str()) {
            inner.active_profile_id = Some(DEFAULT_PROFILE_ID.to_string());
            apply_profile_selection_config(&mut inner, DEFAULT_PROFILE_ID);
        }
        if inner.auto_loaded_profile_id.as_deref() == Some(id.as_str()) {
            inner.auto_loaded_profile_id = None;
        }
        let active_profile_id = inner.active_profile_id.clone();
        for profile in &mut inner.profiles {
            profile.active = active_profile_id.as_deref() == Some(profile.id.as_str());
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Deleted profile {deleted_name}"),
            timestamp: current_timestamp(),
        });
        (deleted_name, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-deleted".to_string()),
    });
    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Deleted profile {deleted_name}"),
        dry_run: None,
    }))
}

async fn activate_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let to_save = {
        let mut inner = state.inner.write().await;
        if !inner.profiles.iter().any(|profile| profile.id == id) {
            return Err(StatusCode::NOT_FOUND);
        }

        for profile in &mut inner.profiles {
            profile.active = profile.id == id;
        }
        inner.active_profile_id = Some(id.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);

        apply_profile_selection_config(&mut inner, &id);

        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;

    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Activated profile {id}"),
        dry_run: None,
    }))
}

async fn list_adapters(State(state): State<AgentState>) -> Json<Vec<AdapterSummary>> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(materialized_adapters(&inner, Some(&game_detection)))
}

async fn update_adapter(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateAdapterRequest>,
) -> Result<Json<AdapterSummary>, StatusCode> {
    let game_detection = state.cached_game_detection().await;
    let (updated, to_save) = {
        let mut inner = state.inner.write().await;
        let adapter = inner
            .adapters
            .iter_mut()
            .find(|adapter| adapter.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        adapter.enabled = request.enabled;
        adapter.state = if request.enabled {
            "needs_setup".to_string()
        } else {
            "disabled".to_string()
        };
        let mut updated = adapter.clone();
        if let Some(runtime) = inner.adapter_runtime(&updated.id) {
            apply_adapter_runtime_summary(
                &mut updated,
                runtime,
                Some(&game_detection),
                Instant::now(),
            );
        }
        (updated, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(updated))
}

async fn get_steam_input_status(State(state): State<AgentState>) -> Json<SteamInputStatus> {
    Json(state.cached_steam_input_status().await)
}

async fn update_steam_input_binding(
    State(state): State<AgentState>,
    Json(request): Json<SteamInputBindingWriteRequest>,
) -> Result<Json<SteamInputBindingWriteResponse>, (StatusCode, String)> {
    let dry_run = request.dry_run;
    let response = tokio::task::spawn_blocking(move || write_steam_input_binding(request))
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Steam Input writer task failed: {error}"),
            )
        })?
        .map_err(|error| (error.status, error.message))?;

    if !dry_run {
        state.spawn_steam_input_refresh();
    }

    Ok(Json(response))
}

async fn list_modules() -> Json<Vec<ModuleSummary>> {
    Json(module_summaries())
}

async fn get_profile_resolution(
    State(state): State<AgentState>,
) -> Json<ProfileResolutionResponse> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(profile_resolution(&inner, Some(&game_detection)))
}

async fn get_detected_game(State(state): State<AgentState>) -> Json<GameDetectionResponse> {
    Json(state.cached_game_detection().await)
}

async fn get_game_art(
    Path((game_id, kind)): Path<(String, String)>,
    State(state): State<AgentState>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["icon", "banner", "hero", "capsule"].contains(&kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let catalog = state.cached_steam_game_catalog().await;
    let path = catalog
        .artwork_paths
        .get(&(game_id, kind))
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    if !steam_artwork_file_usable(&path) {
        return Err(StatusCode::NOT_FOUND);
    }

    let content_type = artwork_content_type(&path);
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes))
}

/// Serves a single artwork file out of Steam's local librarycache, keyed by the
/// numeric `app_id`. This is what `user_game_artwork_for_app` points its URLs
/// at — many Steam apps lack public CDN capsules but always have the local
/// renders Steam uses in-client.
async fn get_steam_app_art(
    Path((app_id, kind)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["icon", "banner", "hero", "capsule"].contains(&kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    // app_id must be purely digits — guards against path traversal and tells us
    // the request is for a real Steam manifest, not an arbitrary string.
    if app_id.is_empty() || !app_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = tokio::task::spawn_blocking(move || {
        steam_root_candidates()
            .into_iter()
            .find(|root| root.join("steamapps").is_dir() || root.join("steam.exe").is_file())
            .and_then(|root| {
                discover_steam_artwork_paths(&root, &app_id)
                    .remove(&kind)
                    .filter(|path| steam_artwork_file_usable(path))
            })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let content_type = artwork_content_type(&path);
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes))
}

async fn list_steam_library(State(state): State<AgentState>) -> Json<SteamLibraryListResponse> {
    let user_games = {
        let inner = state.inner.read().await;
        inner.user_games.clone()
    };
    let games = tokio::task::spawn_blocking(move || discover_steam_library_entries(&user_games))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "Steam library discovery task failed");
            Vec::new()
        });
    Json(SteamLibraryListResponse { games })
}

/// Maximum entries returned by a single browse request. Larger directories are
/// truncated and the response sets `truncated: true` so the UI can warn.
const STEAM_LIBRARY_BROWSE_LIMIT: usize = 400;

/// Sandboxed directory listing for a Steam-installed game. The endpoint never
/// resolves outside the game's install path; it confirms the resolved path is
/// a prefix-equal of the install root after canonicalisation, so symlinks and
/// `..` traversal can't escape the game folder.
async fn browse_steam_library(
    Query(params): Query<BrowseSteamLibraryParams>,
) -> Result<Json<SteamLibraryBrowseResponse>, (StatusCode, Json<serde_json::Value>)> {
    let app_id = params.app_id.trim().to_string();
    if app_id.is_empty() || !app_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId must be a numeric Steam app id"})),
        ));
    }

    let requested_rel = params.path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let Some(manifest) = locate_steam_manifest(&app_id) else {
            return Err(BrowseError::NotFound);
        };
        let install_root = manifest
            .install_path
            .canonicalize()
            .map_err(|_| BrowseError::NotFound)?;
        let target = resolve_browse_target(&install_root, &requested_rel)?;
        if !target.is_dir() {
            return Err(BrowseError::NotADirectory);
        }
        let (entries, truncated) = read_browse_entries(&target);
        let relative_path = path_relative_to(&install_root, &target);
        Ok(SteamLibraryBrowseResponse {
            app_id: manifest.app_id,
            install_path: install_root.display().to_string(),
            relative_path,
            entries,
            truncated,
        })
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "directory scan task failed"})),
        )
    })?;

    match result {
        Ok(response) => Ok(Json(response)),
        Err(BrowseError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Steam app manifest or install folder not found"})),
        )),
        Err(BrowseError::OutsideRoot) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "path escapes the install folder (rejected)"})),
        )),
        Err(BrowseError::NotADirectory) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "requested path is not a directory"})),
        )),
    }
}

enum BrowseError {
    NotFound,
    OutsideRoot,
    NotADirectory,
}

/// Resolves `relative` against `root` and asserts the canonical result is
/// under `root`. Empty/whitespace relative paths return `root` itself. Forward
/// or backward slashes are accepted; `..` segments are blocked.
fn resolve_browse_target(root: &FsPath, relative: &str) -> Result<PathBuf, BrowseError> {
    let trimmed = relative.trim().trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(root.to_path_buf());
    }
    let mut combined = root.to_path_buf();
    for segment in trimmed.split(['/', '\\']) {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains('\0') {
            return Err(BrowseError::OutsideRoot);
        }
        combined.push(segment);
    }
    let canonical = combined.canonicalize().map_err(|_| BrowseError::NotFound)?;
    if !canonical.starts_with(root) {
        return Err(BrowseError::OutsideRoot);
    }
    Ok(canonical)
}

/// Returns the forward-slashed path of `target` relative to `root`, or an
/// empty string if they are equal. Caller has already confirmed `target` is
/// inside `root`.
fn path_relative_to(root: &FsPath, target: &FsPath) -> String {
    target
        .strip_prefix(root)
        .map(|rel| {
            rel.components()
                .filter_map(|component| component.as_os_str().to_str().map(String::from))
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

fn read_browse_entries(dir: &FsPath) -> (Vec<SteamLibraryBrowseEntry>, bool) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return (Vec::new(), false);
    };

    let mut dirs: Vec<SteamLibraryBrowseEntry> = Vec::new();
    let mut exes: Vec<SteamLibraryBrowseEntry> = Vec::new();
    let mut truncated = false;

    for entry in read_dir.flatten() {
        if dirs.len() + exes.len() >= STEAM_LIBRARY_BROWSE_LIMIT {
            truncated = true;
            break;
        }
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        // Hide dotfiles and the Steam-managed sentinel files; the user is
        // never going to pick those as a launch executable.
        if name.starts_with('.') || name.eq_ignore_ascii_case("steam_appid.txt") {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.push(SteamLibraryBrowseEntry {
                name,
                kind: "dir".to_string(),
                size_bytes: None,
            });
        } else if file_type.is_file() && name.to_ascii_lowercase().ends_with(".exe") {
            let size_bytes = entry.metadata().ok().map(|metadata| metadata.len());
            exes.push(SteamLibraryBrowseEntry {
                name,
                kind: "exe".to_string(),
                size_bytes,
            });
        }
    }

    dirs.sort_by_key(|entry| entry.name.to_ascii_lowercase());
    exes.sort_by_key(|entry| entry.name.to_ascii_lowercase());

    let mut combined = dirs;
    combined.extend(exes);
    (combined, truncated)
}

async fn add_custom_game(
    State(state): State<AgentState>,
    Json(request): Json<AddUserGameRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let app_id = request.app_id.trim().to_string();
    if app_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId is required"})),
        ));
    }

    let game_id = user_game_id_for_app_id(&app_id);
    if built_in_game_modules()
        .iter()
        .any(|module| module.id == game_id)
    {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "A built-in module already covers this gameId",
                "gameId": game_id,
            })),
        ));
    }

    // Look up Steam manifest first (outside any lock; this hits the disk).
    let manifest_lookup_app_id = app_id.clone();
    let manifest =
        tokio::task::spawn_blocking(move || locate_steam_manifest(&manifest_lookup_app_id))
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Steam manifest lookup task failed");
                None
            });
    let Some(manifest) = manifest else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Steam app manifest not found",
                "appId": app_id,
            })),
        ));
    };

    // If the client supplied explicit process names, trust them; otherwise scan
    // the install dir. The override path is the escape hatch for games whose
    // .exe lives in a subfolder or is named oddly.
    let process_names = if !request.process_names.is_empty() {
        sanitize_user_game_process_names(&request.process_names)
    } else {
        let process_candidates_path = manifest.install_path.clone();
        tokio::task::spawn_blocking(move || {
            discover_user_game_process_candidates(&process_candidates_path)
        })
        .await
        .unwrap_or_default()
    };

    let added_at = current_timestamp();
    let new_game = UserGameConfig {
        game_id: game_id.clone(),
        app_id: manifest.app_id.clone(),
        name: manifest.name.clone(),
        install_dir: manifest.install_dir.clone(),
        install_path: manifest.install_path.display().to_string(),
        process_names,
        added_at,
    };

    let (summary, to_save) = {
        let mut inner = state.inner.write().await;
        if inner.user_games.contains_key(&game_id) {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "Game already registered",
                    "gameId": game_id,
                })),
            ));
        }
        inner.user_games.insert(game_id.clone(), new_game.clone());
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!(
                "Registered custom Steam game {} ({} processes)",
                new_game.name,
                new_game.process_names.len()
            ),
            timestamp: current_timestamp(),
        });
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let summary = user_game_to_supported_summary(&new_game, None, SteamGameStats::default());
        (summary, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    // Invalidate the detection cache so the new game shows up immediately.
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.value = None;
        cache.refreshed_at = None;
    }
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("user-game-added".to_string()),
    });

    Ok((
        StatusCode::CREATED,
        Json(AddUserGameResponse { game: summary }),
    ))
}

async fn remove_custom_game(
    Path(game_id): Path<String>,
    State(state): State<AgentState>,
) -> Result<StatusCode, StatusCode> {
    let to_save = {
        let mut inner = state.inner.write().await;
        if inner.user_games.remove(&game_id).is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Removed custom game {game_id}"),
            timestamp: current_timestamp(),
        });
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        if inner.auto_loaded_profile_id.is_some() {
            // The detection cache is invalidated below; auto-loaded profile
            // re-resolves on the next snapshot pass.
        }
        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.value = None;
        cache.refreshed_at = None;
    }
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("user-game-removed".to_string()),
    });
    Ok(StatusCode::NO_CONTENT)
}

fn artwork_content_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

async fn get_current_effect(State(state): State<AgentState>) -> Json<CurrentEffectResponse> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(state.current_effect_response_cached(
        &inner,
        Some(&game_detection),
        state.hardware_output_enabled(),
        EffectEnginePurpose::Preview,
    ))
}

async fn set_profile_override(
    State(state): State<AgentState>,
    Json(request): Json<ProfileOverride>,
) -> Result<Json<ProfileResolutionResponse>, StatusCode> {
    let game_detection = state.cached_game_detection().await;
    let (resolution, to_save) = {
        let mut inner = state.inner.write().await;
        if !inner
            .profiles
            .iter()
            .any(|profile| profile.id == request.profile_id)
        {
            return Err(StatusCode::NOT_FOUND);
        }

        inner.profile_overrides.insert(
            profile_override_key(request.controller_id.as_deref(), request.game_id.as_deref()),
            request,
        );
        sync_auto_loaded_profile_for_detection(&mut inner, &game_detection);
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let resolution = profile_resolution(&inner, Some(&game_detection));
        (resolution, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(resolution))
}

async fn clear_profile_override(
    State(state): State<AgentState>,
    Query(scope): Query<ProfileOverrideScope>,
) -> Json<ProfileResolutionResponse> {
    let game_detection = state.cached_game_detection().await;
    let (resolution, to_save) = {
        let mut inner = state.inner.write().await;
        let controller_id = scope.controller_id.as_deref().filter(|id| !id.is_empty());
        let game_id = scope.game_id.as_deref().filter(|id| !id.is_empty());
        if controller_id.is_some() || game_id.is_some() {
            inner
                .profile_overrides
                .remove(&profile_override_key(controller_id, game_id));
        } else {
            inner.profile_overrides.clear();
        }
        sync_auto_loaded_profile_for_detection(&mut inner, &game_detection);
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let resolution = profile_resolution(&inner, Some(&game_detection));
        (resolution, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Json(resolution)
}

async fn list_telemetry(State(state): State<AgentState>) -> Json<Vec<TelemetrySignalResponse>> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(materialized_telemetry_response(
        &inner,
        Some(&game_detection),
    ))
}

async fn list_logs(State(state): State<AgentState>) -> Json<Vec<LogEntry>> {
    let inner = state.inner.read().await;
    Json(inner.logs.clone())
}

async fn get_diagnostics(State(state): State<AgentState>) -> Json<DiagnosticsResponse> {
    Json(state.diagnostics().await)
}

async fn get_support_bundle(State(state): State<AgentState>) -> Json<SupportBundleResponse> {
    Json(state.support_bundle().await)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<AgentState>,
) -> impl IntoResponse {
    if !request_origin_matches_host(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(move |socket| websocket_session(socket, state))
        .into_response()
}

async fn websocket_session(mut socket: WebSocket, state: AgentState) {
    let mut events = state.subscribe_events();
    let payload = serde_json::json!({
        "type": "snapshot",
        "snapshot": state.snapshot().await
    });

    if socket
        .send(Message::Text(payload.to_string()))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            maybe_message = socket.recv() => {
                match maybe_message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        let Ok(text) = serde_json::to_string(&event) else {
                            continue;
                        };
                        if socket.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    let _ = socket.close().await;
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "untitled-profile".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_modules::{FORZA_HORIZON5_STEAM_APP_ID, FORZA_HORIZON6_STEAM_APP_ID};
    use axum::{
        body::{to_bytes, Body},
        http::{Method, Request},
    };
    use serde::de::DeserializeOwned;
    use std::sync::Mutex as StdMutex;
    use tower::ServiceExt;

    static TEST_ENV_LOCK: StdMutex<()> = StdMutex::new(());

    struct TestEnv {
        _lock: std::sync::MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl TestEnv {
        fn new(names: &[&'static str]) -> Self {
            let lock = TEST_ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let saved = names
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect();
            Self { _lock: lock, saved }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn web_dist_uses_configured_path_without_probing() {
        let configured = PathBuf::from("custom-web-dist");

        assert_eq!(
            web_dist_dir_from_parts(Some(configured.clone()), None, None),
            configured
        );
    }

    #[test]
    fn web_dist_finds_packaged_assets_next_to_binary() {
        let root = temp_test_dir("dscc-web-dist");
        let exe = root.join("dscc-cli");
        let web_dist = root.join("web").join("dist");
        fs::create_dir_all(&web_dist).expect("web dist fixture directory");
        fs::write(web_dist.join("index.html"), "<!doctype html>").expect("web dist fixture");

        let found = web_dist_dir_from_parts(None, Some(&exe), Some(&root.join("other-cwd")));

        assert_eq!(found, web_dist);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn web_dist_candidates_cover_repo_and_packaged_layouts() {
        let repo = PathBuf::from("repo-root");
        let exe = PathBuf::from("install-root").join("dscc-cli");
        let candidates = web_dist_candidates(Some(&exe), Some(&repo));

        assert!(candidates.contains(&repo.join("web").join("dist")));
        assert!(candidates.contains(&PathBuf::from("install-root").join("web").join("dist")));
        assert!(candidates.contains(&PathBuf::from("install-root").join("dist")));
    }

    fn test_udp_adapter_runtime() -> AdapterRuntime {
        let adapter = built_in_udp_adapters()
            .iter()
            .find(|adapter| adapter.id == FORZA_DATA_OUT_ADAPTER_ID)
            .copied()
            .expect("Forza UDP adapter is registered");
        AdapterRuntime::for_udp_adapter(adapter)
    }

    fn test_forza_effect_runtime() -> ForzaEffectRuntime {
        ForzaEffectRuntime::default()
    }

    fn test_game_module_by_id(id: &str) -> &'static GameModule {
        built_in_game_modules()
            .iter()
            .find(|game| game.id == id)
            .expect("built-in game module exists")
    }

    fn forza_horizon_controller_config() -> ControllerConfig {
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        config.trigger = forza_horizon_trigger_preset();
        config.forza = forza_horizon_preset();
        config
    }

    #[tokio::test]
    async fn status_reports_mock_active_state() {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let status: StatusResponse = serde_json::from_slice(&body).unwrap();
        assert!(status.healthy);
        assert_eq!(
            status.active_profile_id.as_deref(),
            Some(DEFAULT_PROFILE_ID)
        );
    }

    #[tokio::test]
    async fn support_bundle_route_returns_sanitized_shareable_payload() {
        let _env = TestEnv::new(&["USERPROFILE", "HOME", "DSCC_WEB_DIST"]);
        std::env::set_var("USERPROFILE", r"C:\Users\Kyle");
        std::env::set_var("HOME", "/home/kyle");
        std::env::set_var("DSCC_WEB_DIST", r"D:\PrivateLab\DSCC Secret Web Dist");
        let state = AgentState::mock();
        {
            let mut inner = state.inner.write().await;
            inner.app_settings.forza_playstation_glyphs.install_path =
                Some(r"C:\Users\Kyle\SteamLibrary\ForzaHorizon6".to_string());
            inner.app_settings.forza_playstation_glyphs.last_message =
                r"Installed from C:\Users\Kyle\SteamLibrary\ForzaHorizon6\userdata\123456789\config"
                    .to_string();
        }

        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/support-bundle")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let bundle: SupportBundleResponse = serde_json::from_slice(&body).unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();

        assert_eq!(bundle.schema, "dev.dscc.support-bundle.v1");
        assert!(bundle.privacy.sanitized);
        assert!(bundle
            .privacy
            .omitted
            .iter()
            .any(|item| item == "raw controller hardware IDs"));
        assert!(bundle.app_settings.forza_playstation_glyphs_path_configured);
        assert!(!body_text.contains(r"C:\Users\Kyle"));
        assert!(!body_text.contains(r"C:\\Users\\Kyle"));
        assert!(!body_text.contains("123456789"));
        assert!(!body_text.contains("SteamLibrary"));
        assert!(!body_text.contains("PrivateLab"));
        assert!(!body_text.contains("DSCC Secret Web Dist"));
        assert!(!body_text.contains("installPath"));
        assert!(!body_text.contains("steamPath"));
        assert!(!body_text.contains("rawBinding"));
    }

    #[tokio::test]
    async fn support_bundle_diagnostics_alias_matches_primary_route() {
        for uri in ["/api/support-bundle", "/api/diagnostics/support-bundle"] {
            let response = app(AgentState::mock())
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            let bundle: SupportBundleResponse = serde_json::from_slice(&body).unwrap();
            assert_eq!(bundle.schema, "dev.dscc.support-bundle.v1");
            assert!(bundle.privacy.sanitized);
        }
    }

    #[test]
    fn support_steam_input_summary_omits_raw_layout_details() {
        let status = SteamInputStatus {
            running: true,
            available: true,
            steam_path: Some(r"C:\Program Files (x86)\Steam".to_string()),
            layouts: vec![SteamInputLayout {
                app_id: Some("1551360".to_string()),
                title: "Forza Horizon 5".to_string(),
                controller_type: Some("dual_sense".to_string()),
                controller_label: Some("DualSense Edge".to_string()),
                source: r"steamapps/common/Steam Controller Configs/60706926/config/1551360/controller_edge.vdf"
                    .to_string(),
                binding_count: 1,
                bindings: vec![SteamInputBinding {
                    input: "Cross".to_string(),
                    input_id: "button_south".to_string(),
                    binding: "Secret binding".to_string(),
                    raw_binding: "key_press SECRET_VALUE".to_string(),
                    kind: "keyboard".to_string(),
                    source: Some("buttons".to_string()),
                    source_mode: Some("button".to_string()),
                    activator: Some("full_press".to_string()),
                    group_id: Some("0".to_string()),
                }],
            }],
            warnings: vec![
                r"Read warning in userdata\76561198000000000\config\controller.vdf".to_string(),
            ],
        };

        let summary = support_steam_input_summary(&status);
        let json = serde_json::to_string(&summary).unwrap();

        assert!(summary.install_detected);
        assert_eq!(summary.layout_count, 1);
        assert_eq!(summary.binding_count, 1);
        assert!(json.contains("<steam-user>"));
        assert!(!json.contains("60706926"));
        assert!(!json.contains("76561198000000000"));
        assert!(!json.contains("SECRET_VALUE"));
        assert!(!json.contains("rawBinding"));
        assert!(!json.contains("steamPath"));
    }

    #[test]
    fn support_sanitizer_redacts_absolute_paths_and_steam_ids() {
        let sanitized = sanitize_support_text(
            r"Installed at \\?\D:\SteamLibrary\steamapps\common\ForzaHorizon6. User path C:\Users\Kyle\Documents\dscc. Layout steamapps/common/Steam Controller Configs/60706926/config/controller.vdf and userdata\76561198000000000\config.",
        );

        assert!(sanitized.contains("[local-path]"));
        assert!(sanitized.contains("<steam-user>"));
        assert!(!sanitized.contains("D:\\"));
        assert!(!sanitized.contains("C:\\Users\\Kyle"));
        assert!(!sanitized.contains("SteamLibrary"));
        assert!(!sanitized.contains("60706926"));
        assert!(!sanitized.contains("76561198000000000"));
    }

    #[test]
    fn update_check_version_comparison_handles_tags_and_unknowns() {
        assert_eq!(
            compare_release_versions("0.2.0", "v0.3.0"),
            VersionOrdering::Older
        );
        assert_eq!(
            compare_release_versions("0.2.0", "0.2.0"),
            VersionOrdering::SameOrNewer
        );
        assert_eq!(
            compare_release_versions("0.2.1", "0.2.0"),
            VersionOrdering::SameOrNewer
        );
        assert_eq!(
            compare_release_versions("0.2.0", "preview-build"),
            VersionOrdering::Unknown
        );
    }

    #[test]
    fn update_check_release_payload_reports_available_update() {
        let response = update_check_from_release(
            "0.2.0",
            GithubReleaseResponse {
                tag_name: "v0.3.0".to_string(),
                html_url: "https://github.com/shiftedx/dualsense-command/releases/tag/v0.3.0"
                    .to_string(),
                name: Some("DualSense Command Center 0.3.0".to_string()),
                published_at: Some("2026-05-21T12:00:00Z".to_string()),
            },
            "2026-05-21T12:30:00Z".to_string(),
        );

        assert_eq!(response.current_version, "0.2.0");
        assert_eq!(response.latest_version.as_deref(), Some("0.3.0"));
        assert_eq!(response.state, "update_available");
        assert_eq!(response.error, None);
        assert!(!response.cached);
    }

    #[test]
    fn update_check_failure_payload_is_unavailable() {
        let response = unavailable_update_check("network unavailable".to_string());
        assert_eq!(response.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.latest_version, None);
        assert_eq!(response.release_url, None);
        assert_eq!(response.state, "unavailable");
        assert_eq!(response.error.as_deref(), Some("network unavailable"));
    }

    #[tokio::test]
    async fn update_check_route_returns_cached_response_without_network() {
        let state = AgentState::mock();
        {
            let mut cache = state.discovery_cache.update_check.lock().await;
            cache.store(
                UpdateCheckResponse {
                    current_version: env!("CARGO_PKG_VERSION").to_string(),
                    latest_version: Some("9.9.9".to_string()),
                    release_url: Some(
                        "https://github.com/shiftedx/dualsense-command/releases/tag/v9.9.9"
                            .to_string(),
                    ),
                    release_name: Some("Future release".to_string()),
                    published_at: Some("2026-05-21T12:00:00Z".to_string()),
                    state: "update_available".to_string(),
                    checked_at: Some("2026-05-21T12:30:00Z".to_string()),
                    error: None,
                    cached: false,
                },
                Instant::now(),
            );
        }

        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/update-check")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let update: UpdateCheckResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(update.latest_version.as_deref(), Some("9.9.9"));
        assert_eq!(update.release_name.as_deref(), Some("Future release"));
        assert_eq!(update.state, "update_available");
        assert!(update.cached);
    }

    #[tokio::test]
    async fn update_check_route_is_get_only() {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/update-check")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn cross_origin_mutations_are_rejected() {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/app-settings")
                    .header("host", "127.0.0.1:43473")
                    .header("origin", "http://evil.example")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"listenOnAllInterfaces":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn cross_origin_websocket_origin_guard_rejects_host_mismatch() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:43473".parse().unwrap());
        headers.insert(header::ORIGIN, "http://evil.example".parse().unwrap());

        assert!(!request_origin_matches_host(&headers));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lan_api_mode_requires_explicit_opt_in() {
        let _env = TestEnv::new(&[LAN_API_ENABLE_ENV]);
        std::env::remove_var(LAN_API_ENABLE_ENV);

        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/app-settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"listenOnAllInterfaces":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn agent_bind_addr_ignores_non_loopback_without_lan_opt_in() {
        let config_dir = temp_test_dir("dscc-agent-bind-config");
        fs::create_dir_all(&config_dir).expect("temp config dir");
        let _env = TestEnv::new(&["DSCC_AGENT_ADDR", "DSCC_CONFIG_DIR", LAN_API_ENABLE_ENV]);
        std::env::set_var("DSCC_AGENT_ADDR", "0.0.0.0:43474");
        std::env::set_var("DSCC_CONFIG_DIR", &config_dir);
        std::env::remove_var(LAN_API_ENABLE_ENV);

        assert_eq!(resolve_agent_bind_addr(), default_agent_bind_addr());

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn agent_bind_addr_allows_non_loopback_with_lan_opt_in() {
        let _env = TestEnv::new(&["DSCC_AGENT_ADDR", LAN_API_ENABLE_ENV]);
        std::env::set_var("DSCC_AGENT_ADDR", "0.0.0.0:43474");
        std::env::set_var(LAN_API_ENABLE_ENV, "1");

        assert_eq!(
            resolve_agent_bind_addr(),
            "0.0.0.0:43474".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn forza_bind_addr_ignores_non_loopback_without_lan_opt_in() {
        let _env = TestEnv::new(&[FORZA_BIND_ADDR_ENV, FORZA_LAN_ENABLE_ENV]);
        std::env::set_var(FORZA_BIND_ADDR_ENV, "0.0.0.0:5300");
        std::env::remove_var(FORZA_LAN_ENABLE_ENV);

        assert_eq!(
            resolve_forza_bind_addr(),
            DEFAULT_FORZA_BIND_ADDR.parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn forza_bind_addr_allows_non_loopback_with_lan_opt_in() {
        let _env = TestEnv::new(&[FORZA_BIND_ADDR_ENV, FORZA_LAN_ENABLE_ENV]);
        std::env::set_var(FORZA_BIND_ADDR_ENV, "0.0.0.0:5301");
        std::env::set_var(FORZA_LAN_ENABLE_ENV, "true");

        assert_eq!(
            resolve_forza_bind_addr(),
            "0.0.0.0:5301".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn hardware_output_mode_defaults_to_hardware_output() {
        let _env = TestEnv::new(&[
            "DSCC_DISABLE_HARDWARE_OUTPUT",
            "DSCC_ENABLE_HARDWARE_OUTPUT",
        ]);
        std::env::remove_var("DSCC_DISABLE_HARDWARE_OUTPUT");
        std::env::remove_var("DSCC_ENABLE_HARDWARE_OUTPUT");

        assert_eq!(configured_output_mode(), OutputMode::HardwareOutput);
    }

    #[test]
    fn hardware_output_mode_disable_env_wins_over_enable_env() {
        let _env = TestEnv::new(&[
            "DSCC_DISABLE_HARDWARE_OUTPUT",
            "DSCC_ENABLE_HARDWARE_OUTPUT",
        ]);
        std::env::set_var("DSCC_DISABLE_HARDWARE_OUTPUT", "1");
        std::env::set_var("DSCC_ENABLE_HARDWARE_OUTPUT", "1");

        assert_eq!(configured_output_mode(), OutputMode::DryRunHid);
    }

    #[test]
    fn hardware_output_mode_enable_zero_selects_dry_run() {
        let _env = TestEnv::new(&[
            "DSCC_DISABLE_HARDWARE_OUTPUT",
            "DSCC_ENABLE_HARDWARE_OUTPUT",
        ]);
        std::env::remove_var("DSCC_DISABLE_HARDWARE_OUTPUT");
        std::env::set_var("DSCC_ENABLE_HARDWARE_OUTPUT", "0");

        assert_eq!(configured_output_mode(), OutputMode::DryRunHid);
    }

    #[tokio::test]
    async fn profile_can_be_created_and_activated() {
        let router = app(AgentState::mock());
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Track Focus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/track-focus/activate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let status: StatusResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(status.active_profile_id.as_deref(), Some("track-focus"));
    }

    #[tokio::test]
    async fn profile_create_and_export_preserve_game_scope() {
        let router = app(AgentState::mock());
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"Horizon Rally","gameId":"forza-horizon-6"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let created: ProfileSummary = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.game_id.as_deref(), Some("forza-horizon-6"));

        let exported: ExportedProfile =
            get_json(router, "/api/profiles/horizon-rally/export", StatusCode::OK).await;
        assert_eq!(exported.game_id.as_deref(), Some("forza-horizon-6"));
    }

    #[tokio::test]
    async fn custom_profile_can_be_renamed() {
        let router = app(AgentState::mock());
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Track Focus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profiles/track-focus")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Endurance Focus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let renamed: ProfileSummary = serde_json::from_slice(&body).unwrap();
        assert_eq!(renamed.id, "track-focus");
        assert_eq!(renamed.name, "Endurance Focus");
        assert!(!renamed.built_in);

        let profile: ProfileSummary =
            get_json(router, "/api/profiles/track-focus", StatusCode::OK).await;
        assert_eq!(profile.name, "Endurance Focus");
    }

    #[tokio::test]
    async fn built_in_profile_cannot_be_renamed() {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profiles/forza-horizon")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Renamed Built In"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn custom_profile_can_be_deleted_and_active_profile_falls_back() {
        let router = app(AgentState::mock());
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Track Focus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/track-focus/activate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/profiles/track-focus")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let accepted: ActionAccepted = serde_json::from_slice(&body).unwrap();
        assert!(accepted.accepted);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let status: StatusResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            status.active_profile_id.as_deref(),
            Some(DEFAULT_PROFILE_ID)
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/profiles/track-focus")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn profiles_can_be_exported_and_imported() {
        let router = app(AgentState::mock());

        let exported: ExportedProfile = get_json(
            router.clone(),
            "/api/profiles/global/export",
            StatusCode::OK,
        )
        .await;
        assert_eq!(exported.schema, "dev.dscc.profile.v1");
        assert_eq!(exported.id, DEFAULT_PROFILE_ID);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/import")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"schema":"dev.dscc.profile.v1","id":"imported-road","name":"Imported Road"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let bad_schema = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/import")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"schema":"dev.dscc.profile.v0","id":"bad-road","name":"Bad Road"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_schema.status(), StatusCode::BAD_REQUEST);

        let imported: ProfileSummary =
            get_json(router, "/api/profiles/imported-road", StatusCode::OK).await;
        assert_eq!(imported.name, "Imported Road");
        assert!(!imported.built_in);
    }

    #[tokio::test]
    async fn controllers_are_served_from_multi_controller_registry() {
        let router = app(AgentState::from_controller_events([
            attach_event(
                "controller-a",
                ControllerFamily::DualSense,
                ControllerTransportKind::Usb,
                Some(91),
            ),
            attach_event(
                "controller-b",
                ControllerFamily::DualSenseEdge,
                ControllerTransportKind::Bluetooth,
                Some(54),
            ),
        ]));

        let controllers: Vec<ControllerSummary> =
            get_json(router, "/api/controllers", StatusCode::OK).await;

        assert_eq!(controllers.len(), 2);
        assert_eq!(controllers[0].id, "controller-a");
        assert_eq!(controllers[0].transport, "usb");
        assert_eq!(
            controllers[0].diagnostic_state,
            ControllerDiagnosticState::Ok
        );
        assert_eq!(controllers[1].id, "controller-b");
        assert_eq!(controllers[1].model, "DualSense Edge");
        assert_eq!(controllers[1].battery_percent, Some(54));
    }

    #[tokio::test]
    async fn controller_detail_includes_capabilities_and_diagnostics() {
        let router = app(AgentState::from_controller_events([attach_event(
            "edge-detail",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Usb,
            Some(100),
        )]));

        let detail: ControllerDetail =
            get_json(router, "/api/controllers/edge-detail", StatusCode::OK).await;

        assert_eq!(detail.id, "edge-detail");
        assert_eq!(detail.vendor_id, 0);
        assert_eq!(detail.product_id, 0);
        assert!(detail.capabilities.edge_buttons);
        assert_eq!(detail.permission, ControllerPermissionState::Granted);
        assert!(detail
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "test_fixture"));
    }

    #[tokio::test]
    async fn controller_can_be_renamed_without_changing_identity() {
        let router = app(AgentState::from_controller_events([attach_event(
            "edge-identity",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]));

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/controllers/edge-identity")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Rig Edge"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let renamed: ControllerDetail = serde_json::from_slice(&body).unwrap();
        assert_eq!(renamed.id, "edge-identity");
        assert_eq!(renamed.name, "Rig Edge");

        let controllers: Vec<ControllerSummary> =
            get_json(router, "/api/controllers", StatusCode::OK).await;
        assert_eq!(controllers[0].id, "edge-identity");
        assert_eq!(controllers[0].name, "Rig Edge");
    }

    #[tokio::test]
    async fn controller_config_can_be_read_and_updated() {
        let router = app(AgentState::from_controller_events([attach_event(
            "edge-config",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            None,
        )]));

        let config: ControllerConfig = get_json(
            router.clone(),
            "/api/controllers/edge-config/config",
            StatusCode::OK,
        )
        .await;
        assert_eq!(config.controller_id, "edge-config");
        assert_eq!(config.model, "DualSense Edge");
        assert!(config
            .buttons
            .iter()
            .any(|button| button.key == "Back Left"));

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/controllers/edge-config/config")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "inputMode":"steam_input_companion",
                            "trigger":{
                                "sameRange":true,
                                "l2From":12,
                                "l2To":88,
                                "r2From":0,
                                "r2To":100,
                                "effect":"Wall",
                                "intensity":"Medium",
                                "vibration":"High"
                            },
                            "sticks":{
                                "leftCurve":"Precise",
                                "leftCurveAmount":72,
                                "leftDeadzone":5,
                                "rightCurve":"Dynamic",
                                "rightCurveAmount":110,
                                "rightDeadzone":42
                            },
                            "buttons":[{"key":"Back Left","label":"Shift down"}],
                            "profileAssignments":[
                                {
                                    "gameId":"forza",
                                    "gameName":"Forza Horizon",
                                    "profileId":"edge-track-focus",
                                    "profileName":"Edge Track Focus",
                                    "state":"active",
                                    "detail":"Throttle and brake"
                                }
                            ]
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let updated: ControllerConfig = serde_json::from_slice(&body).unwrap();
        assert_eq!(updated.input_mode, ControllerInputMode::SteamInputCompanion);
        assert_eq!(updated.trigger.effect, "Wall");
        assert_eq!(updated.trigger.r2_from, 12);
        assert_eq!(updated.sticks.right_curve_amount, 100);
        assert_eq!(updated.sticks.right_deadzone, 40);
        assert!(updated
            .buttons
            .iter()
            .any(|button| button.key == "Cross" && button.label == "Cross"));
        assert!(updated
            .buttons
            .iter()
            .any(|button| button.key == "Back Left" && button.label == "L3"));
        assert!(updated
            .buttons
            .iter()
            .any(|button| button.key == "Back Right" && button.label == "R3"));
    }

    #[test]
    fn missing_button_assignments_normalize_to_defaults() {
        let mut controller_value = serde_json::to_value(ControllerConfig::default_for(
            "edge-defaults",
            "DualSense Edge",
        ))
        .expect("controller config serializes");
        controller_value
            .as_object_mut()
            .expect("controller config object")
            .remove("buttons");
        let controller_config: ControllerConfig =
            serde_json::from_value(controller_value).expect("controller config deserializes");
        let controller_config = controller_config.normalized();
        assert!(controller_config
            .buttons
            .iter()
            .any(|button| button.key == "Cross" && button.label == "Cross"));
        assert!(controller_config
            .buttons
            .iter()
            .any(|button| button.key == "Back Left" && button.label == "L3"));

        let mut profile_value =
            serde_json::to_value(ProfileConfig::default()).expect("profile config serializes");
        profile_value
            .as_object_mut()
            .expect("profile config object")
            .remove("buttons");
        let profile_config: ProfileConfig =
            serde_json::from_value(profile_value).expect("profile config deserializes");
        let profile_config = profile_config.normalized_for_model("DualSense Edge");
        assert!(profile_config
            .buttons
            .iter()
            .any(|button| button.key == "Cross" && button.label == "Cross"));
        assert!(profile_config
            .buttons
            .iter()
            .any(|button| button.key == "Back Left" && button.label == "L3"));
    }

    #[test]
    fn steam_input_layout_parser_extracts_readable_bindings() {
        let root = FsPath::new("C:/Program Files (x86)/Steam");
        let file = root.join("userdata/123456/1551360/remote/test_controller_config.vdf");
        let layout = parse_steam_input_layout(
            root,
            &file,
            r##""controller_mappings"
{
    "title" "Forza Layout"
    "controller_type" "controller_ps5"
    "group"
    {
        "ID" "1"
        "mode" "dpad"
        "inputs"
        {
            "dpad_north"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press UP_ARROW"
                        }
                    }
                }
            }
            "button_back_left"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "xinput_button LEFT_SHOULDER"
                        }
                    }
                }
            }
        }
    }
}"##,
        )
        .expect("layout parses");

        assert_eq!(layout.app_id.as_deref(), Some("1551360"));
        assert_eq!(layout.title, "Forza Layout");
        assert_eq!(layout.controller_type.as_deref(), Some("controller_ps5"));
        assert_eq!(layout.controller_label.as_deref(), Some("DualSense"));
        assert!(layout.source.contains("<steam-user>"));
        assert_eq!(layout.bindings[0].input, "D-Pad Up");
        assert_eq!(layout.bindings[0].binding, "Up Arrow Key");
        assert_eq!(layout.bindings[0].kind, "Key");
        assert_eq!(layout.bindings[1].input, "Back Left");
    }

    #[test]
    fn steam_input_layout_parser_keeps_input_id_for_non_full_activators() {
        let root = FsPath::new("C:/Program Files (x86)/Steam");
        let file = root.join("userdata/123456/1551360/remote/test_controller_config.vdf");
        let layout = parse_steam_input_layout(
            root,
            &file,
            r##""controller_mappings"
{
    "title" "Forza Layout"
    "controller_type" "controller_ps5"
    "group"
    {
        "ID" "1"
        "mode" "dpad"
        "inputs"
        {
            "dpad_north"
            {
                "activators"
                {
                    "Long_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press UP_ARROW"
                        }
                    }
                }
            }
        }
    }
}"##,
        )
        .expect("layout parses");

        assert_eq!(layout.bindings.len(), 1);
        assert_eq!(layout.bindings[0].input_id, "dpad_north");
        assert_eq!(layout.bindings[0].input, "D-Pad Up");
        assert_eq!(layout.bindings[0].activator.as_deref(), Some("Long Press"));
    }

    #[test]
    fn steam_input_layout_parser_mirrors_fh6_active_sources() {
        let root = FsPath::new("C:/Program Files (x86)/Steam");
        let file = root.join(
            "steamapps/common/Steam Controller Configs/123456/config/2483190/controller_ps5.vdf",
        );
        let layout = parse_steam_input_layout(
            root,
            &file,
            r##""controller_mappings"
{
    "title" "#Title"
    "controller_type" "controller_ps5_edge"
    "localization"
    {
        "english"
        {
            "title" "Gamepad"
        }
    }
    "group"
    {
        "id" "7"
        "mode" "switches"
        "inputs"
        {
            "button_menu"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "xinput_button select, , "
                        }
                    }
                }
            }
            "button_escape"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "xinput_button start, , "
                        }
                    }
                }
            }
            "button_back_left_upper"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press M, , "
                        }
                    }
                }
            }
            "button_back_left"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press Q, , "
                        }
                    }
                }
            }
        }
    }
    "group"
    {
        "id" "14"
        "mode" "2dscroll"
        "inputs"
        {
            "dpad_north"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press EQUALS, , "
                        }
                    }
                }
            }
            "dpad_south"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press DASH, , "
                        }
                    }
                }
            }
        }
    }
    "preset"
    {
        "id" "0"
        "name" "Default"
        "group_source_bindings"
        {
            "7" "switch active"
            "14" "center_trackpad active"
        }
    }
}"##,
        )
        .expect("layout parses");

        let find = |input_id: &str, group_id: &str| {
            layout
                .bindings
                .iter()
                .find(|binding| {
                    binding.input_id == input_id && binding.group_id.as_deref() == Some(group_id)
                })
                .expect("binding exists")
        };

        let create = find("button_menu", "7");
        assert_eq!(create.input, "Create");
        assert_eq!(create.binding, "Select");
        let options = find("button_escape", "7");
        assert_eq!(options.input, "Options");
        assert_eq!(options.binding, "Start");
        let fn_left = find("button_back_left_upper", "7");
        assert_eq!(fn_left.binding, "M Key");
        let swipe_up = find("dpad_north", "14");
        assert_eq!(swipe_up.input, "Swipe Up");
        assert_eq!(swipe_up.binding, "= Key");
        assert_eq!(swipe_up.source.as_deref(), Some("Center Trackpad"));
        assert_eq!(swipe_up.source_mode.as_deref(), Some("Directional Swipe"));
        let swipe_down = find("dpad_south", "14");
        assert_eq!(swipe_down.input, "Swipe Down");
        assert_eq!(swipe_down.binding, "- Key");
    }

    #[test]
    fn steam_input_writer_replaces_only_selected_binding() {
        let source = r##""controller_mappings"
{
    "title" "Forza Layout"
    "revision" "4"
    "controller_type" "controller_ps5_edge"
    "group"
    {
        "id" "7"
        "mode" "switches"
        "inputs"
        {
            "button_back_left"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press Q, , "
                        }
                    }
                }
            }
            "button_back_right"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press E, , "
                        }
                    }
                }
            }
        }
    }
}"##;
        let request = SteamInputBindingWriteRequest {
            layout_source:
                "steamapps/common/Steam Controller Configs/123/config/2483190/controller_ps5.vdf"
                    .to_string(),
            app_id: Some("2483190".to_string()),
            input_id: "button_back_left".to_string(),
            group_id: Some("7".to_string()),
            activator: Some("Full Press".to_string()),
            raw_binding: "key_press M, , ".to_string(),
            profile_name: Some("Immersive / active".to_string()),
            dry_run: true,
        };

        let updated = replace_steam_binding_value(source, &request, "key_press M, , ")
            .expect("binding can be replaced")
            .expect("source changes");
        let updated = mark_dscc_steam_profile_metadata(&updated, request.profile_name.as_deref());

        assert!(updated.contains(r#""binding" "key_press M, , ""#));
        assert!(updated.contains(r#""binding" "key_press E, , ""#));
        assert!(updated.contains(r#""title" "DSCC / Immersive / active""#));
        assert!(updated.contains(r#""revision" "5""#));
        assert!(!updated.contains(r#""binding" "key_press Q, , ""#));
    }

    #[test]
    fn steam_input_writer_updates_center_trackpad_without_touching_dpad() {
        let source = r##""controller_mappings"
{
    "title" "Forza Layout"
    "revision" "2"
    "controller_type" "controller_ps5_edge"
    "group"
    {
        "id" "9"
        "mode" "dpad"
        "inputs"
        {
            "dpad_north"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "xinput_button DPAD_UP, , "
                        }
                    }
                }
            }
        }
    }
    "group"
    {
        "id" "14"
        "mode" "2dscroll"
        "inputs"
        {
            "dpad_north"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press EQUALS, , "
                        }
                    }
                }
            }
        }
    }
}"##;
        let request = SteamInputBindingWriteRequest {
            layout_source:
                "steamapps/common/Steam Controller Configs/123/config/2483190/controller_ps5.vdf"
                    .to_string(),
            app_id: Some("2483190".to_string()),
            input_id: "dpad_north".to_string(),
            group_id: Some("14".to_string()),
            activator: Some("Full Press".to_string()),
            raw_binding: "key_press TAB, , ".to_string(),
            profile_name: Some("Immersive / active".to_string()),
            dry_run: true,
        };

        let updated = replace_steam_binding_value(source, &request, "key_press TAB, , ")
            .expect("binding can be replaced")
            .expect("source changes");

        assert!(updated.contains(r#""binding" "xinput_button DPAD_UP, , ""#));
        assert!(updated.contains(r#""binding" "key_press TAB, , ""#));
        assert!(!updated.contains(r#""binding" "key_press EQUALS, , ""#));
    }

    #[test]
    fn steam_input_writer_dry_run_uses_temp_steam_root_without_writing() {
        let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
        let root = temp_test_dir("dscc-steam-input-test");
        let layout_dir = root
            .join("steamapps")
            .join("common")
            .join("Steam Controller Configs")
            .join("123456")
            .join("config")
            .join("2483190");
        fs::create_dir_all(&layout_dir).expect("layout fixture directory");
        let layout_file = layout_dir.join("controller_ps5.vdf");
        let original = r##""controller_mappings"
{
    "title" "Gamepad"
    "controller_type" "controller_ps5_edge"
    "group"
    {
        "id" "7"
        "mode" "switches"
        "inputs"
        {
            "button_back_left"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press Q, , "
                        }
                    }
                }
            }
        }
    }
}"##;
        fs::write(&layout_file, original).expect("layout fixture");
        let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");

        std::env::set_var("DSCC_STEAM_ROOT", &root);
        let response = write_steam_input_binding(SteamInputBindingWriteRequest {
            layout_source: source,
            app_id: Some("2483190".to_string()),
            input_id: "button_back_left".to_string(),
            group_id: Some("7".to_string()),
            activator: Some("Full Press".to_string()),
            raw_binding: "key_press M".to_string(),
            profile_name: Some("Base".to_string()),
            dry_run: true,
        })
        .expect("dry run succeeds");

        assert!(response.accepted);
        assert!(response.dry_run);
        assert_eq!(response.backup_path, None);
        assert_eq!(response.binding.binding, "M Key");
        assert_eq!(
            fs::read_to_string(&layout_file).expect("layout still readable"),
            original
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn steam_input_writer_creates_backup_before_writing() {
        let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
        let root = temp_test_dir("dscc-steam-input-write-test");
        let layout_dir = root
            .join("steamapps")
            .join("common")
            .join("Steam Controller Configs")
            .join("123456")
            .join("config")
            .join("2483190");
        fs::create_dir_all(&layout_dir).expect("layout fixture directory");
        let layout_file = layout_dir.join("controller_ps5.vdf");
        let original = r##""controller_mappings"
{
    "title" "Gamepad"
    "revision" "1"
    "controller_type" "controller_ps5_edge"
    "group"
    {
        "id" "7"
        "mode" "switches"
        "inputs"
        {
            "button_back_left"
            {
                "activators"
                {
                    "Full_Press"
                    {
                        "bindings"
                        {
                            "binding" "key_press Q, , "
                        }
                    }
                }
            }
        }
    }
}"##;
        fs::write(&layout_file, original).expect("layout fixture");
        let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");

        std::env::set_var("DSCC_STEAM_ROOT", &root);
        let response = write_steam_input_binding(SteamInputBindingWriteRequest {
            layout_source: source,
            app_id: Some("2483190".to_string()),
            input_id: "button_back_left".to_string(),
            group_id: Some("7".to_string()),
            activator: Some("Full Press".to_string()),
            raw_binding: "key_press M".to_string(),
            profile_name: Some("Base".to_string()),
            dry_run: false,
        })
        .expect("write succeeds");

        assert!(response.accepted);
        assert!(!response.dry_run);
        assert_eq!(response.binding.binding, "M Key");
        let backup_path = response
            .backup_path
            .as_deref()
            .map(PathBuf::from)
            .expect("backup path is reported");
        assert_eq!(
            fs::read_to_string(&backup_path).expect("backup layout is readable"),
            original
        );
        let updated = fs::read_to_string(&layout_file).expect("updated layout is readable");
        assert!(updated.contains(r#""binding" "key_press M, , ""#));
        assert!(updated.contains(r#""title" "DSCC / Base""#));
        assert!(updated.contains(r#""revision" "2""#));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn steam_input_writer_rejects_layouts_outside_steam_root() {
        let root = temp_test_dir("dscc-steam-root-test");
        let outside_root = temp_test_dir("dscc-steam-outside-test");
        fs::create_dir_all(&root).expect("steam root fixture");
        fs::create_dir_all(&outside_root).expect("outside fixture");
        let outside_file = outside_root.join("controller_ps5.vdf");
        fs::write(&outside_file, "\"controller_mappings\"\n{}").expect("outside layout fixture");

        let error = validated_steam_input_layout_path(root.clone(), outside_file)
            .expect_err("outside layout should be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(
            error.message.contains("inside the Steam install path"),
            "unexpected message: {}",
            error.message
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn steam_input_writer_rejects_non_controller_layout_names() {
        let root = temp_test_dir("dscc-steam-name-test");
        let layout_dir = root.join("userdata").join("123456").join("config");
        fs::create_dir_all(&layout_dir).expect("layout fixture directory");
        let layout_file = layout_dir.join("controller_base.vdf");
        fs::write(&layout_file, "\"controller_mappings\"\n{}").expect("layout fixture");

        let error = validated_steam_input_layout_path(root.clone(), layout_file)
            .expect_err("base layout should be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(
            error.message.contains("controller_*.vdf"),
            "unexpected message: {}",
            error.message
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn steam_input_writer_rejects_layouts_over_guarded_size_limit() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let root = temp_test_dir("dscc-steam-large-test");
        let layout_dir = root
            .join("userdata")
            .join("123456")
            .join("2483190")
            .join("remote");
        fs::create_dir_all(&layout_dir).expect("layout fixture directory");
        let layout_file = layout_dir.join("controller_ps5.vdf");
        fs::write(&layout_file, vec![b'a'; 256 * 1024 + 1]).expect("large layout fixture");

        std::env::set_var("DSCC_STEAM_ROOT", &root);
        std::env::set_var("ProgramFiles(x86)", root.join("missing-pf86"));
        std::env::set_var("ProgramFiles", root.join("missing-pf"));
        std::env::set_var("LOCALAPPDATA", root.join("missing-local-app-data"));
        let error = write_steam_input_binding(SteamInputBindingWriteRequest {
            layout_source: layout_file.display().to_string(),
            app_id: Some("2483190".to_string()),
            input_id: "button_back_left".to_string(),
            group_id: None,
            activator: None,
            raw_binding: "key_press M".to_string(),
            profile_name: None,
            dry_run: false,
        })
        .expect_err("large layout should be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(
            error.message.contains("guarded write limit"),
            "unexpected message: {}",
            error.message
        );
        assert!(
            fs::read_dir(&layout_dir)
                .expect("layout directory is readable")
                .all(|entry| entry
                    .expect("layout entry is readable")
                    .file_name()
                    .to_string_lossy()
                    == "controller_ps5.vdf"),
            "large rejected layout should not create backups"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn steam_libraryfolders_parser_discovers_primary_and_secondary_libraries() {
        let libraries = parse_steam_library_folders(include_str!(
            "../tests/fixtures/steam/libraryfolders_fh5_fh6.vdf"
        ));

        assert!(libraries.contains(&PathBuf::from("C:\\Program Files (x86)\\Steam")));
        assert!(libraries.contains(&PathBuf::from("D:\\SteamLibrary")));
    }

    #[test]
    fn steam_appmanifests_match_fh5_and_fh6_supported_games() {
        let primary = FsPath::new("C:/Program Files (x86)/Steam");
        let secondary = FsPath::new("D:/SteamLibrary");
        let fh5_manifest = parse_steam_app_manifest(
            primary,
            include_str!("../tests/fixtures/steam/appmanifest_1551360.acf"),
        )
        .expect("FH5 manifest parses");
        let fh6_manifest = parse_steam_app_manifest(
            secondary,
            include_str!("../tests/fixtures/steam/appmanifest_2483190.acf"),
        )
        .expect("FH6 manifest parses");

        let catalog = build_supported_steam_game_catalog(
            primary,
            &[primary.to_path_buf(), secondary.to_path_buf()],
            &[fh5_manifest, fh6_manifest],
        );

        let fh5 = catalog
            .supported_games
            .iter()
            .find(|game| game.game_id == "forza-horizon-5")
            .expect("FH5 is discovered from Steam appmanifest");
        assert_eq!(fh5.app_id.as_deref(), Some(FORZA_HORIZON5_STEAM_APP_ID));
        assert!(fh5.install_path.as_deref().is_some_and(|path| {
            path.ends_with("steamapps\\common\\ForzaHorizon5")
                || path.ends_with("steamapps/common/ForzaHorizon5")
        }));

        let fh6 = catalog
            .supported_games
            .iter()
            .find(|game| game.game_id == "forza-horizon-6")
            .expect("FH6 is discovered from Steam appmanifest");
        assert_eq!(fh6.app_id.as_deref(), Some(FORZA_HORIZON6_STEAM_APP_ID));
        assert_eq!(fh6.support_level, "telemetry");
    }

    #[test]
    fn steam_appmanifest_matches_assetto_corsa_rally_supported_game() {
        let secondary = FsPath::new("D:/SteamLibrary");
        let acr_manifest = parse_steam_app_manifest(
            secondary,
            r#""AppState"
{
    "appid"     "3917090"
    "name"      "Assetto Corsa Rally"
    "StateFlags"    "4"
    "installdir"    "Assetto Corsa Rally"
}"#,
        )
        .expect("ACR manifest parses");

        let catalog = build_supported_steam_game_catalog(
            secondary,
            &[secondary.to_path_buf()],
            &[acr_manifest],
        );

        let acr = catalog
            .supported_games
            .iter()
            .find(|game| game.game_id == "assetto-corsa-rally")
            .expect("Assetto Corsa Rally is discovered from Steam appmanifest");
        assert_eq!(acr.app_id.as_deref(), Some("3917090"));
        assert!(acr.install_path.as_deref().is_some_and(|path| {
            path.ends_with("steamapps\\common\\Assetto Corsa Rally")
                || path.ends_with("steamapps/common/Assetto Corsa Rally")
        }));
        assert_eq!(acr.support_level, "telemetry");
    }

    #[test]
    fn built_in_game_modules_have_unique_game_ids_and_non_empty_core_ids() {
        let mut game_ids = std::collections::BTreeSet::new();
        let built_in_module_ids: std::collections::BTreeSet<_> =
            built_in_adapters().iter().map(|module| module.id).collect();
        let built_in_profile_ids: std::collections::BTreeSet<_> = default_profiles()
            .iter()
            .map(|profile| profile.id.clone())
            .collect();

        for game in built_in_game_modules() {
            assert!(
                !game.id.trim().is_empty(),
                "built-in game id must not be empty for {game:?}"
            );
            assert!(
                game_ids.insert(game.id),
                "duplicate built-in game id: {}",
                game.id
            );
            assert!(
                !game.adapter_id.trim().is_empty(),
                "module id must not be empty for {}",
                game.id
            );
            assert!(
                built_in_module_ids.contains(game.adapter_id),
                "{} references unknown module id {}",
                game.id,
                game.adapter_id
            );
            assert!(
                !game.default_profile_id.trim().is_empty(),
                "default profile id must not be empty for {}",
                game.id
            );
            assert!(
                built_in_profile_ids.contains(game.default_profile_id),
                "{} references unknown default profile id {}",
                game.id,
                game.default_profile_id
            );
        }
    }

    #[test]
    fn every_built_in_game_has_detection_metadata() {
        for game in built_in_game_modules() {
            assert!(
                !game.display_name.trim().is_empty(),
                "game name must not be empty for {}",
                game.id
            );
            assert!(
                !game.process_names.is_empty(),
                "{} must declare at least one process name",
                game.id
            );

            for process_name in game.process_names {
                assert!(
                    !process_name.trim().is_empty(),
                    "{} contains an empty process name",
                    game.id
                );
                let detection = detect_running_game_from_processes([*process_name]);
                assert_eq!(
                    detection.active_game_id.as_deref(),
                    Some(game.id),
                    "{} should detect from process {}",
                    game.id,
                    process_name
                );
                assert_eq!(
                    detection.module_id.as_deref(),
                    Some(game.id),
                    "{} should detect game module {}",
                    game.id,
                    game.id
                );
                assert_eq!(
                    detection.adapter_id.as_deref(),
                    Some(game.adapter_id),
                    "{} should detect adapter {}",
                    game.id,
                    game.adapter_id
                );
                assert_eq!(
                    detection.profile_id.as_deref(),
                    Some(game.default_profile_id),
                    "{} should detect default profile {}",
                    game.id,
                    game.default_profile_id
                );
                assert_eq!(detection.candidates.len(), 1);
            }
        }
    }

    #[test]
    fn forza_games_are_distinct_game_modules_sharing_forza_data_out() {
        let forza_games: Vec<_> = built_in_game_modules()
            .iter()
            .filter(|game| game.adapter_id == FORZA_DATA_OUT_ADAPTER_ID)
            .collect();
        let forza_game_ids: std::collections::BTreeSet<_> =
            forza_games.iter().map(|game| game.id).collect();

        assert!(forza_game_ids.contains("forza-horizon-5"));
        assert!(forza_game_ids.contains("forza-horizon-6"));
        assert!(
            forza_games.len() >= 2,
            "Forza titles should stay separate game entries"
        );
        assert_eq!(
            forza_game_ids.len(),
            forza_games.len(),
            "Forza titles must have distinct game ids"
        );
        assert!(
            forza_games
                .iter()
                .all(|game| game.adapter_id == FORZA_DATA_OUT_ADAPTER_ID),
            "Forza titles should share the Forza Data Out adapter id"
        );

        let fh5 = detect_running_game_from_processes(["ForzaHorizon5.exe"]);
        let fh6 = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        assert_ne!(fh5.active_game_id, fh6.active_game_id);
        assert_eq!(fh5.module_id.as_deref(), Some("forza-horizon-5"));
        assert_eq!(fh6.module_id.as_deref(), Some("forza-horizon-6"));
        assert_eq!(fh5.adapter_id.as_deref(), Some(FORZA_DATA_OUT_ADAPTER_ID));
        assert_eq!(fh6.adapter_id.as_deref(), Some(FORZA_DATA_OUT_ADAPTER_ID));
    }

    #[test]
    fn steam_local_stats_parser_extracts_playtime_and_achievements() {
        let local_stats = parse_steam_localconfig_stats(
            r#""UserLocalConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "apps"
                {
                    "2483190"
                    {
                        "LastPlayed" "1779141250"
                        "Playtime" "843"
                    }
                }
            }
        }
    }
}"#,
        );
        let game_stats = local_stats
            .get(FORZA_HORIZON6_STEAM_APP_ID)
            .expect("FH6 playtime parsed");
        assert_eq!(game_stats.playtime_minutes, Some(843));
        assert_eq!(game_stats.last_played_unix, Some(1779141250));

        let achievements = parse_steam_achievement_progress_cache(
            r#"{"mapCache":[[2483190,{"appid":2483190,"unlocked":29,"total":57}]]}"#,
        );
        assert_eq!(
            achievements.get(FORZA_HORIZON6_STEAM_APP_ID),
            Some(&SteamAchievementStats {
                unlocked: 29,
                total: 57
            })
        );
    }

    #[test]
    fn steam_game_artwork_uses_local_route_urls_for_discovered_cache_files() {
        let steam_root = std::env::temp_dir().join(format!(
            "dscc-agent-steam-art-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app_cache = steam_root
            .join("appcache")
            .join("librarycache")
            .join(FORZA_HORIZON6_STEAM_APP_ID);
        fs::create_dir_all(app_cache.join("hash-header")).unwrap();
        fs::create_dir_all(app_cache.join("hash-hero")).unwrap();
        fs::create_dir_all(app_cache.join("hash-capsule")).unwrap();
        fs::write(
            app_cache.join("hash-header").join("library_header.jpg"),
            [1_u8, 2, 3],
        )
        .unwrap();
        fs::write(
            app_cache.join("hash-hero").join("library_hero.jpg"),
            [1_u8, 2, 3],
        )
        .unwrap();
        fs::write(
            app_cache.join("hash-capsule").join("library_capsule.jpg"),
            [1_u8, 2, 3],
        )
        .unwrap();

        let manifest = SteamAppManifest {
            app_id: FORZA_HORIZON6_STEAM_APP_ID.to_string(),
            name: "Forza Horizon 6".to_string(),
            install_dir: "ForzaHorizon6".to_string(),
            install_path: steam_root
                .join("steamapps")
                .join("common")
                .join("ForzaHorizon6"),
        };
        let catalog = build_supported_steam_game_catalog(
            &steam_root,
            std::slice::from_ref(&steam_root),
            &[manifest],
        );
        let fh6 = catalog
            .supported_games
            .iter()
            .find(|game| game.game_id == "forza-horizon-6")
            .expect("FH6 is present in supported games");

        assert_eq!(
            fh6.artwork.banner_url.as_deref(),
            Some("/api/games/art/forza-horizon-6/banner")
        );
        assert_eq!(
            fh6.artwork.hero_url.as_deref(),
            Some("/api/games/art/forza-horizon-6/hero")
        );
        assert_eq!(
            fh6.artwork.capsule_url.as_deref(),
            Some("/api/games/art/forza-horizon-6/capsule")
        );
        assert_eq!(
            fh6.artwork.icon_url.as_deref(),
            Some("/api/games/art/forza-horizon-6/capsule")
        );
        assert!(catalog
            .artwork_paths
            .contains_key(&("forza-horizon-6".to_string(), "banner".to_string())));
    }

    #[test]
    fn game_detection_is_enriched_with_supported_steam_game_selection() {
        let fh5 = test_game_module_by_id("forza-horizon-5");
        let catalog = SteamGameCatalog {
            supported_games: vec![supported_game_summary(
                fh5,
                Some(FORZA_HORIZON5_STEAM_APP_ID.to_string()),
                Some(PathBuf::from(
                    "D:/SteamLibrary/steamapps/common/ForzaHorizon5",
                )),
                GameArtwork {
                    banner_url: Some("/api/games/art/forza-horizon-5/banner".to_string()),
                    ..GameArtwork::default()
                },
                SteamGameStats::default(),
            )],
            artwork_paths: BTreeMap::new(),
        };

        let detection = detect_running_game_from_processes(["ForzaHorizon5.exe"]);
        let enriched = enrich_game_detection(detection, &catalog);

        assert_eq!(enriched.active_game_id.as_deref(), Some("forza-horizon-5"));
        assert_eq!(
            enriched
                .selected_game
                .as_ref()
                .map(|game| game.game_id.as_str()),
            Some("forza-horizon-5")
        );
        assert!(enriched
            .supported_games
            .iter()
            .any(|game| game.game_id == "forza-horizon-5" && game.running));
    }

    #[test]
    fn installed_supported_games_do_not_become_selected_without_detection() {
        let fh6 = test_game_module_by_id("forza-horizon-6");
        let catalog = SteamGameCatalog {
            supported_games: vec![SupportedGameSummary {
                installed: true,
                ..supported_game_summary(
                    fh6,
                    Some(FORZA_HORIZON6_STEAM_APP_ID.to_string()),
                    None,
                    GameArtwork::default(),
                    SteamGameStats::default(),
                )
            }],
            artwork_paths: BTreeMap::new(),
        };

        let enriched = enrich_game_detection(no_game_detection("none"), &catalog);

        assert!(enriched.active_game_id.is_none());
        assert!(enriched.selected_game.is_none());
        assert_eq!(enriched.supported_games.len(), 1);
        assert!(!enriched.supported_games[0].running);
    }

    #[tokio::test]
    async fn edge_onboard_profiles_are_visible_and_stageable() {
        let router = app(AgentState::from_controller_events([attach_event(
            "edge-onboard",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            None,
        )]));

        let profiles: EdgeProfilesResponse = get_json(
            router.clone(),
            "/api/controllers/edge-onboard/edge-profiles",
            StatusCode::OK,
        )
        .await;
        assert_eq!(profiles.support_state, EdgeProfileSupportState::Unknown);
        assert_eq!(profiles.slots.len(), 4);
        assert!(profiles
            .slots
            .iter()
            .any(|slot| slot.slot_id == "circle" && slot.editable));

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/controllers/edge-onboard/edge-profiles/circle")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "name":"Track Focus",
                            "trigger":{
                                "sameRange":false,
                                "l2From":5,
                                "l2To":95,
                                "r2From":0,
                                "r2To":100,
                                "effect":"Adaptive resistance",
                                "intensity":"Medium",
                                "vibration":"Medium"
                            },
                            "sticks":{
                                "leftCurve":"Quick",
                                "leftCurveAmount":55,
                                "leftDeadzone":4,
                                "rightCurve":"Default",
                                "rightCurveAmount":60,
                                "rightDeadzone":8
                            },
                            "buttons":[{"key":"Back Left","label":"Shift down"}]
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let accepted: ActionAccepted = serde_json::from_slice(&body).unwrap();
        assert!(accepted.accepted);

        let profiles: EdgeProfilesResponse = get_json(
            router,
            "/api/controllers/edge-onboard/edge-profiles",
            StatusCode::OK,
        )
        .await;
        let circle = profiles
            .slots
            .iter()
            .find(|slot| slot.slot_id == "circle")
            .expect("circle slot exists");
        assert_eq!(circle.state, EdgeProfileSlotState::Assigned);
        assert_eq!(circle.name.as_deref(), Some("Track Focus"));
        assert!(!circle.hardware_synced);
    }

    #[tokio::test]
    async fn modules_and_profile_resolution_are_api_visible() {
        let router = app(AgentState::mock());

        let modules: Vec<ModuleSummary> =
            get_json(router.clone(), "/api/modules", StatusCode::OK).await;
        assert!(modules
            .iter()
            .any(|module| module.id == "forza-data-out" && module.trusted));

        let resolution: ProfileResolutionResponse =
            get_json(router.clone(), "/api/profile-resolution", StatusCode::OK).await;
        // Mock state has no active telemetry adapter (synthetic-lab removed
        // for production), so resolution falls through to the global default.
        assert_eq!(resolution.reason, "global_default");

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profile-resolution/override")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"controllerId":null,"gameId":null,"profileId":"global"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let resolution: ProfileResolutionResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resolution.reason, "manual_override");
        assert_eq!(
            resolution.override_profile_id.as_deref(),
            Some(DEFAULT_PROFILE_ID)
        );
    }

    #[tokio::test]
    async fn profile_override_delete_can_clear_one_game_scope() {
        let state = AgentState::mock();
        let router = app(state.clone());

        for body in [
            r#"{"controllerId":null,"gameId":null,"profileId":"forza-horizon"}"#,
            r#"{"controllerId":null,"gameId":"forza-horizon-6","profileId":"forza-horizon"}"#,
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/api/profile-resolution/override")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/profile-resolution/override?gameId=forza-horizon-6")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        assert!(!inner
            .profile_overrides
            .contains_key(&profile_override_key(None, Some("forza-horizon-6"))));
        assert!(inner
            .profile_overrides
            .contains_key(&profile_override_key(None, None)));
    }

    #[tokio::test]
    async fn controller_global_profile_override_resolves_for_selected_controller() {
        let router = app(AgentState::from_controller_events([attach_event(
            "edge-global",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]));

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profile-resolution/override")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"controllerId":"edge-global","gameId":null,"profileId":"forza-horizon-immersive"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let resolution: ProfileResolutionResponse =
            get_json(router, "/api/profile-resolution", StatusCode::OK).await;
        assert_eq!(resolution.reason, "manual_override");
        assert_eq!(
            resolution.selected_profile_id.as_deref(),
            Some(IMMERSIVE_PROFILE_ID)
        );
    }

    #[tokio::test]
    async fn process_detection_maps_forza_to_edge_profile() {
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
        assert_eq!(detection.profile_id.as_deref(), Some(IMMERSIVE_PROFILE_ID));
    }

    #[tokio::test]
    async fn cached_game_detection_keeps_standard_five_second_cache() {
        let state = AgentState::mock();
        let cached = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        let cached_game_id = cached.active_game_id.clone();
        let cached_at =
            Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

        {
            let mut cache = state.discovery_cache.game_detection.lock().await;
            cache.store(cached, cached_at);
        }

        let detection = state.cached_game_detection().await;
        let refreshed_at = {
            let cache = state.discovery_cache.game_detection.lock().await;
            cache.refreshed_at
        };

        assert_eq!(detection.active_game_id, cached_game_id);
        assert_eq!(refreshed_at, Some(cached_at));
    }

    #[tokio::test]
    async fn cached_hardware_game_detection_refreshes_after_hardware_interval() {
        let state = AgentState::mock();
        let cached = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        let cached_at =
            Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

        {
            let mut catalog = state.discovery_cache.steam_game_catalog.lock().await;
            catalog.store(SteamGameCatalog::default(), Instant::now());
        }
        {
            let mut cache = state.discovery_cache.game_detection.lock().await;
            cache.store(cached, cached_at);
        }

        let _detection = state.cached_hardware_game_detection().await;
        let refreshed_at = {
            let cache = state.discovery_cache.game_detection.lock().await;
            cache.refreshed_at
        };

        assert!(refreshed_at.is_some_and(|refreshed_at| refreshed_at > cached_at));
    }

    #[tokio::test]
    async fn detected_forza_game_materializes_listener_and_profile_resolution() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
        }

        let inner = state.inner.read().await;
        let adapters = materialized_adapters(&inner, Some(&detection));
        let forza = adapters
            .iter()
            .find(|adapter| adapter.id == "forza-data-out")
            .expect("Forza adapter exists");
        assert!(forza.enabled);
        assert_eq!(forza.state, "needs_setup");
        assert!(forza.setup_hint.contains("no Data Out packets"));

        let resolution = profile_resolution(&inner, Some(&detection));
        assert_eq!(
            resolution.active_adapter_id.as_deref(),
            Some("forza-data-out")
        );
        assert_eq!(
            resolution.selected_profile_id.as_deref(),
            Some(IMMERSIVE_PROFILE_ID)
        );
        assert_eq!(resolution.reason, "foreground_game");

        let telemetry = materialized_telemetry_response(&inner, Some(&detection));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "game.state" && signal.value == serde_json::json!("awaiting_data_out")
        }));
    }

    #[tokio::test]
    async fn detected_assetto_game_materializes_shared_memory_and_profile_resolution() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-assetto",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["acr.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                .mark_ready();
        }

        let inner = state.inner.read().await;
        let adapters = materialized_adapters(&inner, Some(&detection));
        let assetto = adapters
            .iter()
            .find(|adapter| adapter.id == ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .expect("Assetto shared-memory adapter exists");
        assert!(assetto.enabled);
        assert_eq!(assetto.state, "needs_setup");
        assert!(assetto.setup_hint.contains("shared memory"));

        let resolution = profile_resolution(&inner, Some(&detection));
        assert_eq!(
            resolution.active_adapter_id.as_deref(),
            Some(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        );
        assert_eq!(
            resolution.selected_profile_id.as_deref(),
            Some(ASSETTO_CORSA_RALLY_PROFILE_ID)
        );
        assert_eq!(resolution.reason, "foreground_game");

        let telemetry = materialized_telemetry_response(&inner, Some(&detection));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "game.state"
                && signal.value == serde_json::json!("awaiting_shared_memory")
        }));
    }

    #[tokio::test]
    async fn supported_game_detection_writes_only_lightbar_until_telemetry_is_live() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-assetto",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["acr.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                .mark_ready();
        }

        let inner = state.inner.read().await;
        let resolution = profile_resolution(&inner, Some(&detection));
        assert!(!hardware_output_runtime_allowed_for_resolution(
            &inner,
            Some(&detection),
            &resolution
        ));
        assert!(hardware_output_detection_lightbar_allowed_for_resolution(
            &inner,
            Some(&detection),
            &resolution
        ));

        let (controller_id, frame) = state
            .output_frame_for_current_resolution_cached(
                &inner,
                Some(&detection),
                EffectEnginePurpose::Hardware,
            )
            .expect("detection lightbar frame is produced");

        assert_eq!(controller_id, "edge-assetto");
        assert_eq!(frame.l2, TriggerOutput::Off);
        assert_eq!(frame.r2, TriggerOutput::Off);
        assert!(frame.rumble.is_none());
        assert!(frame.player_leds.is_none());
        assert_eq!(
            frame.lightbar,
            Some(LightbarOutput {
                color: RgbColor {
                    red: 0xff,
                    green: 0x3b,
                    blue: 0x30
                },
                brightness: 0.62,
            })
        );

        let preview = current_effect_response(&inner, Some(&detection), false);
        assert_eq!(preview.output.l2, TriggerOutput::Off);
        assert_eq!(preview.output.r2, TriggerOutput::Off);
        assert!(preview.output.rumble.is_none());
        assert_eq!(preview.output.lightbar, frame.lightbar);
    }

    #[test]
    fn assetto_shared_memory_prefix_normalizes_racing_signals() {
        let mut physics = vec![0_u8; ASSETTO_PHYSICS_MIN_LEN];
        write_i32(&mut physics, 0, 27);
        write_f32(&mut physics, 4, 0.65);
        write_f32(&mut physics, 8, 0.25);
        write_i32(&mut physics, 16, 4);
        write_i32(&mut physics, 20, 6_300);
        write_f32(&mut physics, 24, 0.30);
        write_f32(&mut physics, 28, 102.0);
        write_f32(&mut physics, 44, 0.20);
        write_f32(&mut physics, 48, 0.35);
        write_f32(&mut physics, 52, 0.10);
        write_f32(&mut physics, 56, 0.11);
        write_f32(&mut physics, 60, 0.18);
        write_f32(&mut physics, 64, 0.42);
        write_f32(&mut physics, 68, 0.36);

        let mut graphics = vec![0_u8; ASSETTO_GRAPHICS_MIN_LEN];
        write_i32(&mut graphics, 4, ASSETTO_AC_LIVE);

        let mut static_page = vec![0_u8; ASSETTO_STATIC_MIN_LEN];
        write_i32(&mut static_page, ASSETTO_STATIC_MAX_RPM_OFFSET, 9_000);

        let (_, updates) = parse_assetto_shared_memory_pages(
            AssettoSharedMemoryPages {
                physics: &physics,
                graphics: Some(&graphics),
                static_page: Some(&static_page),
            },
            42,
        )
        .expect("Assetto shared-memory prefix parses");
        let snapshot = SignalSnapshot::from_updates(updates);

        assert_eq!(
            snapshot.text("source.id"),
            Some(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        );
        assert_eq!(snapshot.text("game.state"), Some("driving"));
        assert_eq!(snapshot.number("vehicle.speed_kmh"), Some(102.0));
        assert_eq!(snapshot.number("drivetrain.gear"), Some(3.0));
        assert!(snapshot
            .number("input.throttle")
            .is_some_and(|value| (value - 0.65).abs() < 0.000_001));
        assert!(snapshot
            .number("vehicle.rpm_ratio")
            .is_some_and(|value| (value - 0.7).abs() < 0.000_001));
        assert!(snapshot
            .number("wheel.slip.max")
            .is_some_and(|value| (value - 0.42).abs() < 0.000_001));
    }

    #[test]
    fn idle_forza_listener_is_a_clear_diagnostic() {
        let mut runtime = test_udp_adapter_runtime();
        runtime.mark_bound("127.0.0.1:5300".parse().unwrap());

        let health = adapter_runtime_health_check(&runtime, Some(&no_game_detection("none")));

        assert_eq!(health.name, "forza-data-out");
        assert_eq!(health.status, "ok");
        assert!(health.detail.contains("telemetry will activate"));
        assert!(!health.detail.contains("waiting"));
    }

    #[test]
    fn detected_forza_without_packets_warns_in_diagnostics() {
        let mut runtime = test_udp_adapter_runtime();
        runtime.mark_bound("127.0.0.1:5300".parse().unwrap());
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

        let health = adapter_runtime_health_check(&runtime, Some(&detection));

        assert_eq!(health.name, "forza-data-out");
        assert_eq!(health.status, "warning");
        assert!(health.detail.contains("Forza Horizon 6 is running"));
        assert!(health.detail.contains("no live Data Out packets"));
    }

    #[tokio::test]
    async fn detected_forza_auto_loads_profile_without_ui_apply() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

        let mut inner = state.inner.write().await;
        inner.active_profile_id = Some(DEFAULT_PROFILE_ID.to_string());
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        for effect in &mut config.forza.effects {
            effect.enabled = false;
        }
        config.lightbar.color = "#f4a261".to_string();
        config.lightbar.brightness = 44;
        inner
            .controller_configs
            .insert("edge-forza".to_string(), config);

        assert!(sync_auto_loaded_profile_for_detection(
            &mut inner, &detection
        ));
        assert_eq!(
            inner.auto_loaded_profile_id.as_deref(),
            Some(IMMERSIVE_PROFILE_ID)
        );
        assert_eq!(inner.active_profile_id.as_deref(), Some(DEFAULT_PROFILE_ID));

        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("connected controller config was updated");
        let effect_enabled = |id: &str| -> bool {
            config
                .forza
                .effects
                .iter()
                .find(|effect| effect.id == id)
                .unwrap_or_else(|| panic!("preset contains '{id}'"))
                .enabled
        };

        assert!(effect_enabled("abs_slip_pulse"));
        assert!(effect_enabled("gear_shift_thump"));
        assert!(!effect_enabled("rpm_leds"));
        assert!(effect_enabled("road_texture"));
        assert_eq!(config.trigger.l2_from, 0);
        assert_eq!(config.trigger.r2_from, 4);
        assert_eq!(config.lightbar.color, "#f4a261");
        assert_eq!(config.lightbar.brightness, 44);

        let shift = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "gear_shift_thump")
            .expect("gear_shift_thump present after auto-load");
        assert_eq!(shift.route, "r2_and_body");
    }

    #[tokio::test]
    async fn cleared_forza_detection_unloads_auto_profile() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

        let mut inner = state.inner.write().await;
        inner.active_profile_id = Some(DEFAULT_PROFILE_ID.to_string());
        inner.controller_configs.insert(
            "edge-forza".to_string(),
            ControllerConfig::default_for("edge-forza", "DualSense Edge"),
        );

        assert!(sync_auto_loaded_profile_for_detection(
            &mut inner, &detection
        ));
        assert!(sync_auto_loaded_profile_for_detection(
            &mut inner,
            &no_game_detection("none")
        ));
        assert_eq!(inner.auto_loaded_profile_id, None);
        assert_eq!(inner.active_profile_id.as_deref(), Some(DEFAULT_PROFILE_ID));

        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("connected controller config was restored");
        let effect_enabled = |id: &str| -> bool {
            config
                .forza
                .effects
                .iter()
                .find(|effect| effect.id == id)
                .unwrap_or_else(|| panic!("preset contains '{id}'"))
                .enabled
        };

        assert!(effect_enabled("abs_slip_pulse"));
        assert!(effect_enabled("gear_shift_thump"));
        assert!(!effect_enabled("rpm_leds"));
        assert!(effect_enabled("road_texture"));
        assert!(effect_enabled("brake_resistance"));
    }

    #[tokio::test]
    async fn stale_forza_effects_keep_trigger_output_neutral_while_game_runs() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at =
                Some(Instant::now() - TELEMETRY_PACKET_STALE_AFTER - Duration::from_secs(1));
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "driving"),
                signal_update("input.brake", 0.95),
                signal_update("input.throttle", 0.80),
                signal_update("wheel.slip.max", 0.70),
            ]);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, Some(&detection), false);

        assert_eq!(response.output.l2, TriggerOutput::Off);
        assert_eq!(response.output.r2, TriggerOutput::Off);
        assert!(response
            .warnings
            .iter()
            .any(|warning| { warning.contains("trigger output stays neutral") }));
        assert!(response
            .parity_effects
            .iter()
            .all(|effect| effect.state == "ready"));
    }

    #[tokio::test]
    async fn stale_forza_effects_neutralize_after_game_exits() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at =
                Some(Instant::now() - TELEMETRY_PACKET_STALE_AFTER - Duration::from_secs(1));
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "driving"),
                signal_update("input.brake", 0.95),
                signal_update("input.throttle", 0.80),
                signal_update("drivetrain.shift_event", "none"),
            ]);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, None, false);

        assert_eq!(response.output.l2, TriggerOutput::Off);
        assert_eq!(response.output.r2, TriggerOutput::Off);
    }

    #[tokio::test]
    async fn forza_menu_effects_keep_trigger_output_neutral() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at = Some(Instant::now());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "menu"),
                signal_update("input.brake", 0.0),
                signal_update("input.throttle", 0.0),
                signal_update("input.handbrake", 0.0),
                signal_update("vehicle.rpm_ratio", 0.0),
                signal_update("vehicle.speed_kmh", 0.0),
                signal_update("wheel.slip.max", 0.0),
                signal_update("tire.slip_ratio.max", 0.0),
                signal_update("drivetrain.shift_event", "none"),
                signal_update("drivetrain.shift_pulse", 0.0),
            ]);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, Some(&detection), true);

        assert_eq!(response.output.l2, TriggerOutput::Off);
        assert_eq!(response.output.r2, TriggerOutput::Off);
    }

    #[tokio::test]
    async fn hardware_output_frame_uses_global_lightbar_without_game_and_waits_for_live_packets() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        {
            let mut inner = state.inner.write().await;
            let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
            config.lightbar.color = "#f4a261".to_string();
            config.lightbar.brightness = 44;
            inner
                .controller_configs
                .insert("edge-forza".to_string(), config);
        }

        let without_game = {
            let inner = state.inner.read().await;
            state.output_frame_for_current_resolution_cached(
                &inner,
                None,
                EffectEnginePurpose::Hardware,
            )
        };
        let (_, global_frame) =
            without_game.expect("idle hardware output keeps the global lightbar color");
        assert_eq!(global_frame.l2, TriggerOutput::Off);
        assert_eq!(global_frame.r2, TriggerOutput::Off);
        assert!(global_frame.rumble.is_none());
        assert!(global_frame.player_leds.is_none());
        assert_eq!(
            global_frame.lightbar,
            Some(LightbarOutput {
                color: RgbColor {
                    red: 0xf4,
                    green: 0xa2,
                    blue: 0x61
                },
                brightness: 0.44,
            })
        );

        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        let without_packets = {
            let inner = state.inner.read().await;
            state.output_frame_for_current_resolution_cached(
                &inner,
                Some(&detection),
                EffectEnginePurpose::Hardware,
            )
        };
        let (_, detection_frame) =
            without_packets.expect("supported-game detection emits a lightbar-only frame");
        assert_eq!(detection_frame.l2, TriggerOutput::Off);
        assert_eq!(detection_frame.r2, TriggerOutput::Off);
        assert!(detection_frame.lightbar.is_some());
        assert!(detection_frame.rumble.is_none());
        assert!(detection_frame.player_leds.is_none());

        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at = Some(Instant::now());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "driving"),
                signal_update("input.brake", 0.20),
                signal_update("input.throttle", 0.30),
                signal_update("vehicle.speed_kmh", 30.0),
                signal_update("drivetrain.shift_event", "none"),
            ]);
        }

        let with_live_packets = {
            let inner = state.inner.read().await;
            state.output_frame_for_current_resolution_cached(
                &inner,
                Some(&detection),
                EffectEnginePurpose::Hardware,
            )
        };
        assert!(with_live_packets.is_some());
    }

    #[tokio::test]
    async fn live_forza_effects_preserve_native_rumble_by_default_and_can_full_control_body() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at = Some(Instant::now());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "driving"),
                signal_update("input.brake", 0.30),
                signal_update("input.throttle", 0.82),
                signal_update("input.handbrake", 0.0),
                signal_update("wheel.slip.max", 0.58),
                signal_update("wheel.slip.front_max", 0.42),
                signal_update("wheel.slip.rear_max", 0.58),
                signal_update("tire.slip_ratio.max", 0.36),
                signal_update("tire.slip_angle.max", 0.28),
                signal_update("surface.rumble.max", 0.44),
                signal_update("surface.rumble_strip.max", 1.0),
                signal_update("surface.puddle.max", 0.18),
                signal_update("suspension.travel.max", 0.12),
                signal_update("vehicle.acceleration.magnitude", 16.0),
                signal_update("vehicle.rpm_ratio", 0.91),
                signal_update("vehicle.speed_kmh", 188.0),
                signal_update("drivetrain.gear", 4.0),
                signal_update("drivetrain.shift_event", "none"),
                signal_update("drivetrain.shift_pulse", 0.0),
            ]);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, Some(&detection), true);
        assert_eq!(
            response.output.rumble, None,
            "native passthrough should leave continuous Forza body rumble to the game"
        );
        assert!(response.output.lightbar.is_some());
        assert_eq!(
            response.output.player_leds,
            Some(PlayerLedsOutput { count: 4 })
        );
        assert!(response
            .parity_effects
            .iter()
            .any(|effect| { effect.id == "rumble_strip" && effect.state == "active" }));
        drop(inner);

        {
            let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
            config.forza.body_rumble_mode = "dscc_full_control".to_string();
            let mut inner = state.inner.write().await;
            inner
                .controller_configs
                .insert("edge-forza".to_string(), config);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, Some(&detection), true);

        let rumble = response
            .output
            .rumble
            .expect("DSCC full-control mode should drive telemetry body rumble");
        assert!(rumble.low_frequency > 0.20);
        assert!(rumble.high_frequency > 0.35);
    }

    #[test]
    fn forza_player_leds_follow_current_gear() {
        let third_gear = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 3.0)]);
        assert_eq!(forza_gear_player_led_count(&third_gear), 3);

        let sixth_gear = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 6.0)]);
        assert_eq!(forza_gear_player_led_count(&sixth_gear), 5);

        let neutral = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 0.0)]);
        assert_eq!(forza_gear_player_led_count(&neutral), 0);
    }

    #[test]
    fn forza_lightbar_blends_profile_color_toward_redline_with_rpm() {
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        config.lightbar.color = "#0044ff".to_string();
        config.lightbar.rpm_color = "#ffcc00".to_string();
        config.lightbar.brightness = 50;

        let idle = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.0)]);
        let mid = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.5)]);
        let redline = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 1.0)]);

        let idle_lightbar = forza_lightbar_output(Some(&config), &idle, 1.0);
        let mid_lightbar = forza_lightbar_output(Some(&config), &mid, 1.0);
        let redline_lightbar = forza_lightbar_output(Some(&config), &redline, 1.0);
        let disabled_rpm_leds = forza_lightbar_output(Some(&config), &redline, 0.0);

        assert_eq!(
            idle_lightbar.color,
            RgbColor {
                red: 0,
                green: 68,
                blue: 255,
            }
        );
        assert!(
            mid_lightbar.color.red > idle_lightbar.color.red,
            "mid-rpm lightbar should move toward red"
        );
        assert!(
            mid_lightbar.color.blue < idle_lightbar.color.blue,
            "mid-rpm lightbar should reduce blue while moving toward red"
        );
        assert_eq!(
            redline_lightbar.color,
            RgbColor {
                red: 255,
                green: 204,
                blue: 0,
            }
        );
        assert!(redline_lightbar.brightness > idle_lightbar.brightness);
        assert_eq!(disabled_rpm_leds.color, idle_lightbar.color);
    }

    #[tokio::test]
    async fn disabled_forza_effect_reports_disabled_and_suppresses_output() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
            for effect in &mut config.forza.effects {
                effect.enabled = false;
            }

            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count = 1;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_at = Some(Instant::now());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .last_packet_len = Some(324);
            inner.active_adapter_id = Some("forza-data-out".to_string());
            inner
                .controller_configs
                .insert("edge-forza".to_string(), config);
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", "forza-data-out"),
                signal_update("game.id", "forza-horizon-6"),
                signal_update("game.state", "driving"),
                signal_update("vehicle.speed_kmh", 84.0),
                signal_update("surface.rumble_strip.max", 1.0),
            ]);
        }

        let inner = state.inner.read().await;
        let response = current_effect_response(&inner, Some(&detection), true);
        let rumble_strip = response
            .parity_effects
            .iter()
            .find(|effect| effect.id == "rumble_strip")
            .expect("rumble strip status exists");

        assert_eq!(rumble_strip.state, "disabled");
        assert_eq!(
            response
                .parity_effects
                .iter()
                .filter(|effect| effect.state == "disabled")
                .count(),
            default_forza_effect_configs().len()
        );
        assert_eq!(response.output.rumble, None);
    }

    #[test]
    fn forza_tuning_routes_shift_thump_to_left_body() {
        let mut forza = ForzaTelemetryConfig::default().normalized();
        for effect in &mut forza.effects {
            effect.enabled = false;
        }
        let shift = forza
            .effects
            .iter_mut()
            .find(|effect| effect.id == "gear_shift_thump")
            .expect("default shift tuning exists");
        shift.enabled = true;
        shift.intensity = FORZA_SHIFT_THUMP_DEFAULT_INTENSITY;
        shift.route = "body_left".to_string();
        let snapshot = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.5),
            signal_update("vehicle.speed_kmh", 80.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 0.0),
            signal_update("surface.rumble.max", 0.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_pulse", 1.0),
        ]);

        let rumble =
            forza_rumble_output(&forza, &snapshot, 1.0, "Balanced").expect("shift should rumble");

        assert!(
            rumble.low_frequency > 0.95,
            "max shift thump should saturate the routed low motor, got {}",
            rumble.low_frequency
        );
        assert!(
            rumble.high_frequency < 0.65,
            "left-body route should still keep high motor secondary, got {}",
            rumble.high_frequency
        );
    }

    #[test]
    fn forza_shift_thump_intensity_scales_r2_and_reduced_body() {
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let shift = config
            .forza
            .effects
            .iter_mut()
            .find(|effect| effect.id == "gear_shift_thump")
            .expect("default shift tuning exists");
        shift.enabled = true;
        shift.intensity = 35;
        shift.route = "r2_and_body".to_string();

        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.5),
            signal_update("vehicle.speed_kmh", 80.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("surface.rumble.max", 0.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_event", "shift"),
            signal_update("drivetrain.shift_pulse", 1.0),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let mut frame = EffectEngine::new().evaluate(&profile, &snapshot);
        apply_forza_output_enhancements(Some(&config), &snapshot, true, &mut frame);

        match frame.r2 {
            TriggerOutput::Pulse {
                amplitude,
                frequency_hz,
            } => {
                assert!((frequency_hz - FORZA_SHIFT_FREQUENCY_HZ).abs() < f64::EPSILON);
                assert!(
                    (0.32..0.38).contains(&amplitude),
                    "35% shift thump should produce a scaled trigger pulse, got {amplitude}"
                );
            }
            other => panic!("expected scaled trigger shift pulse, got {other:?}"),
        }
        match frame.l2 {
            TriggerOutput::AdaptiveResistance { .. } => {}
            other => {
                panic!("R2 + body shift thump should leave L2 on brake baseline, got {other:?}")
            }
        }
        let rumble = frame
            .rumble
            .expect("body route should produce shift rumble");
        assert!(
            (0.18..0.20).contains(&rumble.low_frequency),
            "35% shift thump should produce reduced low rumble, got {}",
            rumble.low_frequency
        );
        assert!(
            (0.16..0.18).contains(&rumble.high_frequency),
            "35% shift thump should produce reduced high rumble, got {}",
            rumble.high_frequency
        );
    }

    #[test]
    fn forza_surface_rumble_is_suppressed_while_stationary() {
        let mut forza = ForzaTelemetryConfig::default().normalized();
        forza.body_rumble_mode = "dscc_full_control".to_string();
        for effect in &mut forza.effects {
            effect.enabled = false;
        }
        let road = forza
            .effects
            .iter_mut()
            .find(|effect| effect.id == "road_texture")
            .expect("default road tuning exists");
        road.enabled = true;
        road.intensity = 150;
        road.route = "body_both".to_string();
        let idle_on_dirt = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.25),
            signal_update("vehicle.speed_kmh", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 0.0),
            signal_update("surface.rumble.max", 1.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_pulse", 0.0),
        ]);

        assert_eq!(
            forza_rumble_output(&forza, &idle_on_dirt, 1.0, "Balanced"),
            None
        );

        let rolling_on_dirt = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.25),
            signal_update("vehicle.speed_kmh", 24.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 0.0),
            signal_update("surface.rumble.max", 1.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_pulse", 0.0),
        ]);
        let rumble = forza_rumble_output(&forza, &rolling_on_dirt, 1.0, "Balanced")
            .expect("dirt should rumble once the car is rolling");

        assert!(rumble.low_frequency > 0.20);
        assert!(rumble.high_frequency > 0.25);
    }

    #[test]
    fn forza_tuning_can_move_throttle_off_r2_trigger() {
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let throttle = config
            .forza
            .effects
            .iter_mut()
            .find(|effect| effect.id == "throttle_resistance")
            .expect("default throttle tuning exists");
        throttle.route = "body_both".to_string();
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 1.0),
            signal_update("input.brake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.4),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        assert_eq!(frame.r2, TriggerOutput::Off);
    }

    #[test]
    fn forza_trigger_resistance_uses_tensioned_throttle_curve() {
        let config = forza_horizon_controller_config();
        let idle_throttle = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 90.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let idle_frame = EffectEngine::new().evaluate(&profile, &idle_throttle);

        match idle_frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.04).abs() < f64::EPSILON);
                assert!(
                    (0.005..0.02).contains(&strength),
                    "idle throttle should stay light at the beginning of the pull, got {strength}"
                );
            }
            other => panic!("expected baseline throttle tension, got {other:?}"),
        }
        match idle_frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert_eq!(start_position, 0.0);
                assert!(
                    (0.13..0.16).contains(&strength),
                    "idle brake should still feel tensioned, got {strength}"
                );
            }
            other => panic!("expected baseline brake tension, got {other:?}"),
        }

        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.70),
            signal_update("input.brake", 0.80),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 90.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.r2 {
            TriggerOutput::AdaptiveResistance { strength, .. } => {
                assert!(
                    (0.23..0.32).contains(&strength),
                    "partial throttle should be hardening through the end-stop ramp, got {strength}"
                );
            }
            other => panic!("expected throttle resistance, got {other:?}"),
        }
        match frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.72).abs() < f64::EPSILON);
                assert!(
                    strength > 0.98 && strength <= 1.0,
                    "partial brake should be near the sustained lock-warning wall, got {strength}"
                );
            }
            other => panic!("expected brake resistance, got {other:?}"),
        }
    }

    #[test]
    fn forza_full_pedal_press_arms_end_stop_force() {
        let config = forza_horizon_controller_config();
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 1.0),
            signal_update("input.brake", 1.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 90.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.80).abs() < f64::EPSILON);
                assert!(
                    (0.99..=1.0).contains(&strength),
                    "full throttle should hold a max-resistance wall through the last travel, got {strength}"
                );
            }
            other => panic!("expected full throttle force, got {other:?}"),
        }
        match frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.72).abs() < f64::EPSILON);
                assert!(
                    strength > 0.98 && strength <= 1.0,
                    "full brake should create a hard lock-warning wall, got {strength}"
                );
            }
            other => panic!("expected full brake force, got {other:?}"),
        }
    }

    #[test]
    fn forza_throttle_endstop_progressively_hardens_near_high_end_point() {
        let config = forza_horizon_controller_config();
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));

        let snapshot = |throttle| {
            SignalSnapshot::from_updates([
                signal_update("game.state", "driving"),
                signal_update("input.throttle", throttle),
                signal_update("input.brake", 0.0),
                signal_update("input.handbrake", 0.0),
                signal_update("vehicle.rpm_ratio", 0.40),
                signal_update("vehicle.speed_kmh", 90.0),
                signal_update("tire.slip_ratio.max", 0.0),
                signal_update("wheel.slip.max", 0.0),
                signal_update("drivetrain.shift_event", "none"),
            ])
        };

        let below = EffectEngine::new().evaluate(&profile, &snapshot(0.59));
        match below.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.04).abs() < f64::EPSILON);
                assert!(
                    strength < 0.12,
                    "throttle should stay light before the end-stop ramp, got {strength}"
                );
            }
            other => panic!("expected light throttle ramp before guard, got {other:?}"),
        }

        let ramp_start = EffectEngine::new().evaluate(&profile, &snapshot(0.60));
        match ramp_start.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.60).abs() < 1e-9);
                assert!(
                    (0.08..0.12).contains(&strength),
                    "throttle guard should begin with a controlled ramp, got {strength}"
                );
            }
            other => panic!("expected throttle overtravel ramp to arm, got {other:?}"),
        }

        let mid_ramp = EffectEngine::new().evaluate(&profile, &snapshot(0.70));
        match mid_ramp.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.60).abs() < 1e-9);
                assert!(
                    (0.23..0.32).contains(&strength),
                    "throttle should build meaningfully through the ramp, got {strength}"
                );
            }
            other => panic!("expected progressive throttle guard in the ramp, got {other:?}"),
        }

        let near_wall = EffectEngine::new().evaluate(&profile, &snapshot(0.78));
        match near_wall.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.60).abs() < 1e-9);
                assert!(
                    (0.74..0.86).contains(&strength),
                    "throttle should get significantly harder near the wall, got {strength}"
                );
            }
            other => panic!("expected progressive throttle guard near the wall, got {other:?}"),
        }

        let frame = EffectEngine::new().evaluate(&profile, &snapshot(0.80));
        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.80).abs() < f64::EPSILON);
                assert!(
                    (0.99..=1.0).contains(&strength),
                    "throttle wall should hold max resistance through the final travel, got {strength}"
                );
            }
            other => panic!("expected throttle guard wall at full throttle, got {other:?}"),
        }
    }

    #[test]
    fn forza_brake_endstop_warns_before_high_end_point() {
        let mut config = forza_horizon_controller_config();
        config.trigger.l2_to = 90;
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));

        let snapshot = |brake| {
            SignalSnapshot::from_updates([
                signal_update("game.state", "driving"),
                signal_update("input.throttle", 0.0),
                signal_update("input.brake", brake),
                signal_update("input.handbrake", 0.0),
                signal_update("vehicle.rpm_ratio", 0.40),
                signal_update("vehicle.speed_kmh", 90.0),
                signal_update("tire.slip_ratio.max", 0.0),
                signal_update("wheel.slip.max", 0.0),
                signal_update("drivetrain.shift_event", "none"),
            ])
        };

        let below = EffectEngine::new().evaluate(&profile, &snapshot(0.69));
        match below.l2 {
            TriggerOutput::AdaptiveResistance { .. } => {}
            other => panic!("brake wall should wait until the warning point, got {other:?}"),
        }

        for brake in [0.70, 1.0] {
            let frame = EffectEngine::new().evaluate(&profile, &snapshot(brake));
            match frame.l2 {
                TriggerOutput::AdaptiveResistance {
                    start_position,
                    strength,
                } => {
                    assert!((start_position - 0.70).abs() < f64::EPSILON);
                    assert!(
                        strength > 0.98 && strength <= 1.0,
                        "brake wall should stay strong after the warning point, got {strength}"
                    );
                }
                other => panic!("expected hard brake warning wall at {brake}, got {other:?}"),
            }
        }
    }

    #[test]
    fn forza_trigger_range_end_controls_full_force_point() {
        let mut config = forza_horizon_controller_config();
        config.trigger.l2_from = 20;
        config.trigger.l2_to = 60;
        config.trigger.r2_from = 10;
        config.trigger.r2_to = 50;

        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.50),
            signal_update("input.brake", 0.60),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 90.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.57).abs() < f64::EPSILON);
                assert!(
                    strength > 0.98 && strength <= 1.0,
                    "custom brake end point should arm full force at 60%, got {strength}"
                );
            }
            other => panic!("expected brake end-stop force, got {other:?}"),
        }
        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.47).abs() < f64::EPSILON);
                assert!(
                    (0.99..=1.0).contains(&strength),
                    "custom throttle end point should arm max force at 50%, got {strength}"
                );
            }
            other => panic!("expected throttle end-stop force, got {other:?}"),
        }
    }

    #[test]
    fn forza_abs_pulse_uses_brake_speed_and_slip_thresholds() {
        let config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.50),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 55.0),
            signal_update("tire.slip_ratio.max", 1.15),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.l2 {
            TriggerOutput::Pulse {
                amplitude,
                frequency_hz,
            } => {
                assert!((frequency_hz - 10.0).abs() < f64::EPSILON);
                assert!(
                    (amplitude - FORZA_ABS_PULSE_AMPLITUDE).abs() < f64::EPSILON,
                    "ABS pulse should use the Horizon reference amplitude, got {amplitude}"
                );
            }
            other => panic!("expected ABS pulse, got {other:?}"),
        }
    }

    #[test]
    fn forza_abs_threshold_tracks_custom_brake_range() {
        let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        config.trigger.l2_from = 50;
        config.trigger.l2_to = 100;
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));

        let below_threshold = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.60),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 55.0),
            signal_update("tire.slip_ratio.max", 1.15),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let frame = EffectEngine::new().evaluate(&profile, &below_threshold);
        match frame.l2 {
            TriggerOutput::AdaptiveResistance { .. } => {}
            other => panic!("ABS should wait for the adjusted brake range, got {other:?}"),
        }

        let above_threshold = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.0),
            signal_update("input.brake", 0.70),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.40),
            signal_update("vehicle.speed_kmh", 55.0),
            signal_update("tire.slip_ratio.max", 1.15),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let frame = EffectEngine::new().evaluate(&profile, &above_threshold);
        match frame.l2 {
            TriggerOutput::Pulse { frequency_hz, .. } => {
                assert!((frequency_hz - FORZA_ABS_PULSE_FREQUENCY_HZ).abs() < f64::EPSILON);
            }
            other => panic!("expected ABS pulse after adjusted threshold, got {other:?}"),
        }
    }

    #[test]
    fn forza_rev_limiter_buzz_is_slightly_stronger() {
        let config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.95),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.95),
            signal_update("vehicle.speed_kmh", 95.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.r2 {
            TriggerOutput::Pulse {
                amplitude,
                frequency_hz,
            } => {
                assert!((frequency_hz - 30.0).abs() < f64::EPSILON);
                assert!(
                    (0.15..0.18).contains(&amplitude),
                    "rev limiter buzz should be slightly stronger, got {amplitude}"
                );
            }
            other => panic!("expected rev limiter buzz, got {other:?}"),
        }
    }

    #[test]
    fn forza_shift_detector_tracks_raw_direction_blind_gear_changes() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        assert_eq!(
            runtime.detect_shift_event(Some(3.0), true, true, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(0.0), true, true, now),
            Some("shift")
        );
        assert_eq!(runtime.latched_shift_event(now), Some("shift"));
        assert_eq!(
            runtime.detect_shift_event(Some(4.0), true, true, now),
            Some("shift")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(3.0), true, true, now),
            Some("shift")
        );
        assert_eq!(runtime.latched_shift_event(now), Some("shift"));
    }

    #[test]
    fn forza_shift_detector_suppresses_first_packet_and_hard_stops() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        assert_eq!(
            runtime.detect_shift_event(Some(3.0), true, true, now),
            Some("none")
        );
        assert_eq!(runtime.latched_shift_event(now), None);
        assert_eq!(
            runtime.detect_shift_event(Some(4.0), true, true, now),
            Some("shift")
        );
        assert_eq!(
            runtime.latched_shift_event(now + Duration::from_millis(189)),
            Some("shift")
        );
        assert_eq!(
            runtime.latched_shift_event(now + Duration::from_millis(190)),
            None
        );
    }

    #[test]
    fn forza_shift_detector_extends_without_stacking() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        assert_eq!(
            runtime.detect_shift_event(Some(3.0), true, true, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(4.0), true, true, now),
            Some("shift")
        );
        let second_shift = now + Duration::from_millis(50);
        assert_eq!(
            runtime.detect_shift_event(Some(5.0), true, true, second_shift),
            Some("shift")
        );
        assert_eq!(
            runtime.latched_shift_event(second_shift + Duration::from_millis(189)),
            Some("shift")
        );
        assert_eq!(
            runtime.latched_shift_event(second_shift + Duration::from_millis(190)),
            None
        );
    }

    #[test]
    fn forza_shift_detector_freezes_while_disabled_or_telemetry_off() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        assert_eq!(
            runtime.detect_shift_event(Some(3.0), true, false, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(4.0), true, false, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(5.0), true, true, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(6.0), true, true, now),
            Some("shift")
        );

        assert_eq!(
            runtime.detect_shift_event(Some(7.0), false, true, now),
            Some("none")
        );
        assert_eq!(
            runtime.detect_shift_event(Some(8.0), true, true, now),
            Some("shift")
        );
    }

    #[test]
    fn forza_suspension_impact_latches_landing_body_thump() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        assert_eq!(
            runtime.detect_suspension_impact(Some(0.06), Some(12.0), Some(80.0), true, true, now),
            0.0
        );

        let landing =
            runtime.detect_suspension_impact(Some(0.28), Some(34.0), Some(80.0), true, true, now);
        assert!(
            landing > 0.95,
            "hard landings should latch a full body thump, got {landing}"
        );
        assert!(
            runtime.latched_suspension_impact(now + Duration::from_millis(169)) > 0.95,
            "landing thump should hold briefly"
        );
        assert_eq!(
            runtime.latched_suspension_impact(now + Duration::from_millis(170)),
            0.0
        );
    }

    #[test]
    fn forza_suspension_impact_ignores_steering_acceleration_without_compression() {
        let mut runtime = test_forza_effect_runtime();
        let now = Instant::now();

        let steering =
            runtime.detect_suspension_impact(Some(0.03), Some(34.0), Some(96.0), true, true, now);
        assert_eq!(
            steering, 0.0,
            "lateral acceleration without suspension compression should not thump"
        );
        assert_eq!(runtime.latched_suspension_impact(now), 0.0);
    }

    #[test]
    fn forza_shift_thump_wins_over_rev_limiter_on_r2() {
        let config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 1.0),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.98),
            signal_update("vehicle.speed_kmh", 118.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "shift"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.r2 {
            TriggerOutput::PulseAb {
                strength,
                frequency_hz,
                wall_zones,
            } => {
                assert!((frequency_hz - FORZA_SHIFT_FREQUENCY_HZ).abs() < f64::EPSILON);
                assert_eq!(wall_zones, 4);
                assert!(
                    strength > 0.95,
                    "floored shift thump should use the full configured wall-form kick, got {strength}"
                );
            }
            other => panic!("expected shift wall pulse to override rev limiter, got {other:?}"),
        }
    }

    #[test]
    fn forza_shift_thump_uses_plain_pulse_near_idle() {
        let config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
        let snapshot = SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", 0.05),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.98),
            signal_update("vehicle.speed_kmh", 118.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "shift"),
        ]);
        let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        match frame.r2 {
            TriggerOutput::Pulse {
                amplitude,
                frequency_hz,
            } => {
                assert!((frequency_hz - FORZA_SHIFT_FREQUENCY_HZ).abs() < f64::EPSILON);
                assert!(
                    amplitude > 0.95,
                    "default shift thump should use the full configured kick, got {amplitude}"
                );
            }
            other => panic!("expected plain shift pulse below wall threshold, got {other:?}"),
        }
    }

    #[test]
    fn manual_trigger_test_uses_requested_start_position() {
        let request = EffectTestRequest {
            target: Some("r2".to_string()),
            mode: Some("adaptive_resistance".to_string()),
            intensity: Some(82),
            start_position: Some(0.37),
            l2_position: None,
            r2_position: None,
            duration_ms: Some(650),
            trigger: None,
        };

        let frame = effect_test_output_frame(&request);
        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.37).abs() < f64::EPSILON);
                assert!((strength - 0.82).abs() < f64::EPSILON);
            }
            other => panic!("expected adaptive resistance test output, got {other:?}"),
        }
    }

    #[test]
    fn base_feel_test_uses_current_l2_and_r2_settings() {
        let trigger = TriggerConfig {
            l2_from: 8,
            l2_to: 100,
            r2_from: 3,
            r2_to: 72,
            intensity: "Strong (Standard)".to_string(),
            ..Default::default()
        };

        let request = EffectTestRequest {
            target: Some("base_feel".to_string()),
            mode: Some("hold".to_string()),
            intensity: Some(100),
            start_position: None,
            l2_position: None,
            r2_position: None,
            duration_ms: Some(DEFAULT_BASE_FEEL_TEST_DURATION_MS),
            trigger: Some(trigger),
        };

        let frame = effect_test_output_frame(&request);
        match frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.08).abs() < f64::EPSILON);
                assert!((strength - 1.0).abs() < f64::EPSILON);
            }
            other => panic!("expected L2 base feel resistance, got {other:?}"),
        }
        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.03).abs() < f64::EPSILON);
                assert!((strength - 0.72).abs() < f64::EPSILON);
            }
            other => panic!("expected R2 base feel resistance, got {other:?}"),
        }
    }

    #[test]
    fn base_feel_test_uses_live_trigger_position_and_curve_math() {
        let trigger = TriggerConfig {
            l2_from: 20,
            l2_to: 80,
            l2_curve: TriggerCurve::from_ratio(2.0),
            l2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(2.0)),
            r2_from: 10,
            r2_to: 90,
            r2_curve: TriggerCurve::from_ratio(0.5),
            r2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(0.5)),
            intensity: "Strong (Standard)".to_string(),
            ..Default::default()
        };

        let request = EffectTestRequest {
            target: Some("base_feel".to_string()),
            mode: Some("hold".to_string()),
            intensity: Some(100),
            start_position: None,
            l2_position: Some(0.50),
            r2_position: Some(0.50),
            duration_ms: Some(DEFAULT_BASE_FEEL_TEST_DURATION_MS),
            trigger: Some(trigger),
        };

        let frame = effect_test_output_frame(&request);
        match frame.l2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.20).abs() < f64::EPSILON);
                assert!(
                    (strength - 0.25).abs() < 0.0001,
                    "L2 should match ((50-20)/(80-20))^2, got {strength}"
                );
            }
            other => panic!("expected L2 base feel resistance, got {other:?}"),
        }
        match frame.r2 {
            TriggerOutput::AdaptiveResistance {
                start_position,
                strength,
            } => {
                assert!((start_position - 0.10).abs() < f64::EPSILON);
                assert!(
                    (strength - 0.71).abs() < 0.0001,
                    "R2 should match the generated point curve for sqrt((50-10)/(90-10)), got {strength}"
                );
            }
            other => panic!("expected R2 base feel resistance, got {other:?}"),
        }
    }

    #[test]
    fn legacy_trigger_config_deserializes_points_from_saved_curves() {
        let trigger: TriggerConfig = serde_json::from_value(serde_json::json!({
            "sameRange": false,
            "l2From": 20,
            "l2To": 100,
            "r2From": 0,
            "r2To": 100,
            "l2Curve": 2.0,
            "r2Curve": 0.5,
            "effect": "Adaptive resistance",
            "intensity": "Strong (Standard)",
            "vibration": "Medium",
            "vibrationMode": "Balanced"
        }))
        .expect("legacy trigger config without point arrays should deserialize");

        let trigger = trigger.normalized();

        assert_eq!(
            trigger.l2_curve_points,
            trigger_curve_points_from_curve(TriggerCurve::from_ratio(2.0))
        );
        assert_eq!(
            trigger.r2_curve_points,
            trigger_curve_points_from_curve(TriggerCurve::from_ratio(0.5))
        );
    }

    #[test]
    fn base_feel_test_uses_custom_trigger_curve_points() {
        let trigger = TriggerConfig {
            l2_from: 0,
            l2_to: 100,
            l2_curve_points: vec![
                TriggerCurvePoint {
                    input: 0,
                    output: 0,
                },
                TriggerCurvePoint {
                    input: 35,
                    output: 8,
                },
                TriggerCurvePoint {
                    input: 50,
                    output: 80,
                },
                TriggerCurvePoint {
                    input: 100,
                    output: 100,
                },
            ],
            intensity: "Strong (Standard)".to_string(),
            ..Default::default()
        };
        let frame = base_feel_test_output_frame(trigger, Some(0.50), Some(0.0));

        match frame.l2 {
            TriggerOutput::AdaptiveResistance { strength, .. } => {
                assert!(
                    (0.79..0.81).contains(&strength),
                    "custom L2 point curve should shape base feel output, got {strength}"
                );
            }
            other => panic!("expected L2 point-curve resistance, got {other:?}"),
        }
    }

    #[test]
    fn base_feel_test_exposes_wall_pulse_pattern() {
        let trigger = TriggerConfig {
            l2_from: 12,
            r2_from: 7,
            effect: "Wall pulse".to_string(),
            intensity: "Strong (Standard)".to_string(),
            ..Default::default()
        };

        let request = EffectTestRequest {
            target: Some("base_feel".to_string()),
            mode: Some("hold".to_string()),
            intensity: Some(100),
            start_position: None,
            l2_position: None,
            r2_position: None,
            duration_ms: Some(DEFAULT_BASE_FEEL_TEST_DURATION_MS),
            trigger: Some(trigger),
        };

        let frame = effect_test_output_frame(&request);
        match frame.l2 {
            TriggerOutput::PulseAb {
                strength,
                frequency_hz,
                wall_zones,
            } => {
                assert!((strength - 1.0).abs() < f64::EPSILON);
                assert!((frequency_hz - 60.0).abs() < f64::EPSILON);
                assert_eq!(wall_zones, 2);
            }
            other => panic!("expected L2 wall pulse, got {other:?}"),
        }
        match frame.r2 {
            TriggerOutput::PulseAb {
                strength,
                frequency_hz,
                wall_zones,
            } => {
                assert!((strength - 1.0).abs() < f64::EPSILON);
                assert!((frequency_hz - 60.0).abs() < f64::EPSILON);
                assert_eq!(wall_zones, 2);
            }
            other => panic!("expected R2 wall pulse, got {other:?}"),
        }
    }

    #[test]
    fn rumble_test_honors_body_haptic_character() {
        let deep = effect_test_output_frame(&EffectTestRequest {
            target: Some("rumble".to_string()),
            mode: Some("deep_thump".to_string()),
            intensity: Some(80),
            start_position: None,
            l2_position: None,
            r2_position: None,
            duration_ms: Some(DEFAULT_EFFECT_TEST_DURATION_MS),
            trigger: None,
        })
        .rumble
        .expect("deep thump should produce rumble");
        assert!((deep.low_frequency - 0.80).abs() < f64::EPSILON);
        assert!(deep.high_frequency < 0.20);

        let fine = effect_test_output_frame(&EffectTestRequest {
            target: Some("rumble".to_string()),
            mode: Some("fine_buzz".to_string()),
            intensity: Some(80),
            start_position: None,
            l2_position: None,
            r2_position: None,
            duration_ms: Some(DEFAULT_EFFECT_TEST_DURATION_MS),
            trigger: None,
        })
        .rumble
        .expect("fine buzz should produce rumble");
        assert!(fine.low_frequency < 0.20);
        assert!((fine.high_frequency - 0.80).abs() < f64::EPSILON);
    }

    #[test]
    fn forza_horizon_preset_preserves_native_body_rumble_by_default() {
        // The "Base" built-in preset is designed to be
        // battery-conscious and game-friendly: adaptive triggers stay on,
        // native body rumble remains the continuous surface/engine layer,
        // and DSCC only adds short event-driven thumps by default.
        let preset =
            forza_preset_for_profile("forza-horizon").expect("forza-horizon is a built-in preset");
        assert_eq!(preset.body_rumble_mode, "native_passthrough");

        let road = preset
            .effects
            .iter()
            .find(|effect| effect.id == "road_texture")
            .expect("preset must contain 'road_texture'");
        assert!(road.enabled, "road texture should be enabled by default");
        assert_eq!(road.intensity, 40);
        assert_eq!(road.route, "body_both");

        for id in [
            "rumble_strip",
            "tire_slip",
            "puddle_drag",
            "suspension_impact",
        ] {
            let effect = preset
                .effects
                .iter()
                .find(|effect| effect.id == id)
                .unwrap_or_else(|| panic!("preset must contain '{id}'"));
            assert!(
                !effect.enabled,
                "heavy continuous-rumble effect '{id}' must default to disabled in the \
                 Base preset (got enabled={})",
                effect.enabled,
            );
        }

        // Adaptive-trigger effects should be enabled and route to the
        // natural trigger side for the effect.
        let trigger_effects: &[(&str, &str)] = &[
            ("brake_resistance", "l2"),
            ("throttle_resistance", "r2"),
            ("abs_slip_pulse", "l2"),
            ("handbrake_wall", "l2"),
            ("rev_limiter_buzz", "r2"),
        ];
        for (id, expected_route) in trigger_effects {
            let effect = preset
                .effects
                .iter()
                .find(|effect| effect.id == *id)
                .unwrap_or_else(|| panic!("preset must contain '{id}'"));
            assert!(
                effect.enabled,
                "adaptive-trigger effect '{id}' should stay enabled in the \
                 Base preset"
            );
            assert_eq!(effect.route, *expected_route, "route for '{id}'");
        }

        let abs = preset
            .effects
            .iter()
            .find(|effect| effect.id == "abs_slip_pulse")
            .expect("preset must contain 'abs_slip_pulse'");
        assert_eq!(
            abs.intensity, 100,
            "ABS preset intensity should preserve the 20-unit reference pulse"
        );

        let shift = preset
            .effects
            .iter()
            .find(|effect| effect.id == "gear_shift_thump")
            .expect("preset must contain 'gear_shift_thump'");
        assert!(shift.enabled);
        assert_eq!(shift.intensity, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY);
        assert_eq!(shift.route, "r2_and_body");

        let rpm_leds = preset
            .effects
            .iter()
            .find(|effect| effect.id == "rpm_leds")
            .expect("preset must contain 'rpm_leds'");
        assert!(
            !rpm_leds.enabled,
            "Base should leave gear LEDs disabled and keep only the user lightbar color"
        );

        // Unknown profile ids have no preset — activation is a no-op for
        // controller config (so user-created profiles never overwrite the
        // user's tuning).
        assert!(forza_preset_for_profile("user-created-profile").is_none());
        assert!(forza_preset_for_profile("some-unrecognized-id").is_none());
    }

    #[test]
    fn forza_horizon_immersive_preset_layers_detail_without_stealing_core_cues() {
        let preset = forza_preset_for_profile(IMMERSIVE_PROFILE_ID)
            .expect("immersive Horizon is a built-in preset");
        assert_eq!(preset.body_rumble_mode, "native_passthrough");
        let effect = |id: &str| {
            preset
                .effects
                .iter()
                .find(|effect| effect.id == id)
                .unwrap_or_else(|| panic!("preset must contain '{id}'"))
        };

        for (id, route) in [
            ("brake_resistance", "l2"),
            ("throttle_resistance", "r2"),
            ("abs_slip_pulse", "l2"),
            ("handbrake_wall", "l2"),
            ("rev_limiter_buzz", "r2"),
        ] {
            let tuning = effect(id);
            assert!(
                tuning.enabled,
                "core trigger cue '{id}' should stay enabled"
            );
            assert_eq!(tuning.route, route, "route for '{id}'");
        }

        let shift = effect("gear_shift_thump");
        assert!(shift.enabled);
        assert_eq!(shift.intensity, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY);
        assert_eq!(shift.route, "r2_and_body");

        for (id, intensity, route) in [
            ("road_texture", 35, "body_both"),
            ("rumble_strip", 38, "body_both"),
            ("tire_slip", 30, "body_right"),
            ("puddle_drag", 32, "body_left"),
            ("suspension_impact", 82, "body_both"),
        ] {
            let tuning = effect(id);
            assert!(tuning.enabled, "immersive layer '{id}' should be enabled");
            assert_eq!(tuning.intensity, intensity, "intensity for '{id}'");
            assert_eq!(tuning.route, route, "route for '{id}'");
            assert!(
                tuning.intensity < shift.intensity,
                "immersive layer '{id}' should stay below the shift thump"
            );
        }

        let rpm_leds = effect("rpm_leds");
        assert!(
            !rpm_leds.enabled,
            "Immersive should keep gear LEDs and the RPM bar disabled by default"
        );
        assert_eq!(rpm_leds.intensity, 100);
        assert_eq!(rpm_leds.route, "light_led");
    }

    #[test]
    fn forza_immersive_preset_keeps_slip_below_landing_thumps() {
        let preset = forza_preset_for_profile(IMMERSIVE_PROFILE_ID)
            .expect("immersive Horizon is a built-in preset");

        let heavy_slip = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.85),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.55),
            signal_update("vehicle.speed_kmh", 96.0),
            signal_update("wheel.slip.max", 1.10),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 1.10),
            signal_update("tire.slip_ratio.max", 1.0),
            signal_update("tire.slip_angle.max", 0.85),
            signal_update("surface.rumble.max", 0.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_pulse", 0.0),
        ]);
        assert_eq!(
            forza_rumble_output(&preset, &heavy_slip, 1.0, "Balanced"),
            None,
            "native passthrough should not replace Forza's own continuous tire/body rumble"
        );

        let mut full_control_preset = preset.clone();
        full_control_preset.body_rumble_mode = "dscc_full_control".to_string();
        let slip = forza_rumble_output(&full_control_preset, &heavy_slip, 1.0, "Balanced")
            .expect("heavy slip should still produce readable feedback");
        assert!(
            slip.high_frequency < 0.19,
            "immersive slip should be readable without becoming constant buzz, got {slip:?}"
        );

        let landing = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.20),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.35),
            signal_update("vehicle.speed_kmh", 96.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 0.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("tire.slip_angle.max", 0.0),
            signal_update("surface.rumble.max", 0.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.28),
            signal_update("suspension.impact_pulse", 1.0),
            signal_update("vehicle.acceleration.magnitude", 34.0),
            signal_update("drivetrain.shift_pulse", 0.0),
        ]);
        let landing = forza_rumble_output(&preset, &landing, 1.0, "Balanced")
            .expect("hard landing should produce a body thump");
        assert!(
            landing.low_frequency > 0.75,
            "hard landings should have real low-frequency thump, got {landing:?}"
        );
        assert!(
            landing.high_frequency > slip.high_frequency * 1.5,
            "landing thumps should stand above sustained tire slip, slip={slip:?}, landing={landing:?}"
        );
    }

    #[test]
    fn forza_immersive_preset_filters_gentle_steering_slip_angle_feedback() {
        let preset = forza_preset_for_profile(IMMERSIVE_PROFILE_ID)
            .expect("immersive Horizon is a built-in preset");
        let lane_change = SignalSnapshot::from_updates([
            signal_update("input.throttle", 0.20),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.35),
            signal_update("vehicle.speed_kmh", 96.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("wheel.slip.front_max", 0.0),
            signal_update("wheel.slip.rear_max", 0.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("tire.slip_angle.max", 0.45),
            signal_update("surface.rumble.max", 0.0),
            signal_update("surface.rumble_strip.max", 0.0),
            signal_update("surface.puddle.max", 0.0),
            signal_update("suspension.travel.max", 0.0),
            signal_update("vehicle.acceleration.magnitude", 0.0),
            signal_update("drivetrain.shift_pulse", 0.0),
        ]);

        assert_eq!(
            forza_rumble_output(&preset, &lane_change, 1.0, "Balanced"),
            None,
            "gentle lane-change slip angle should not create noticeable body vibration"
        );
    }

    #[tokio::test]
    async fn activating_forza_profile_writes_preset_into_controller_config() {
        // Activating "forza-horizon" should rewrite every saved
        // controller's Forza config to the preset, so the UI re-reads the
        // new values on the next /api/controllers/{id} fetch.
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let router = app(state.clone());

        // Touch the controller's config once so the lazy-created default
        // config exists in `controller_configs` (the API materializes it on
        // first GET).
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/controllers/edge-forza/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Sanity: the controller starts with the engineering default config
        // (which enables surface effects).
        {
            let inner = state.inner.read().await;
            let config = inner
                .controller_configs
                .get("edge-forza")
                .expect("controller config materialized by GET");
            let road = config
                .forza
                .effects
                .iter()
                .find(|effect| effect.id == "road_texture")
                .expect("road_texture in default config");
            assert!(
                road.enabled,
                "the engineering default config should enable road_texture so we \
                 can verify activation actually changes it"
            );
        }

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/forza-horizon/activate")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        assert_eq!(inner.active_profile_id.as_deref(), Some("forza-horizon"));
        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("controller config still present");

        // The Base preset enables road texture but leaves heavier
        // continuous-rumble effects disabled on the saved config.
        let road = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "road_texture")
            .expect("road_texture present after activation");
        assert!(
            road.enabled,
            "activating the Base preset should enable road_texture on the saved \
             controller config"
        );
        assert_eq!(road.intensity, 40);
        assert_eq!(road.route, "body_both");
        let rumble = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "rumble_strip")
            .expect("rumble_strip present after activation");
        assert!(!rumble.enabled);

        // Trigger effects remain enabled with the preset's intensities.
        let brake = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "brake_resistance")
            .expect("brake_resistance present after activation");
        assert!(brake.enabled);
        assert_eq!(brake.intensity, 100);
        assert_eq!(brake.route, "l2");

        let shift = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "gear_shift_thump")
            .expect("gear_shift_thump present after activation");
        assert!(shift.enabled);
        assert_eq!(shift.intensity, FORZA_SHIFT_THUMP_DEFAULT_INTENSITY);
        assert_eq!(shift.route, "r2_and_body");

        let rpm_leds = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "rpm_leds")
            .expect("rpm_leds present after activation");
        assert!(!rpm_leds.enabled);
        assert_eq!(config.trigger.l2_from, 0);
        assert_eq!(config.trigger.r2_from, 4);
        assert_eq!(config.trigger.l2_to, 100);
        assert_eq!(config.trigger.r2_to, 100);
        assert!((config.trigger.l2_curve.as_f64() - FORZA_BRAKE_CURVE).abs() < f64::EPSILON);
        assert!((config.trigger.r2_curve.as_f64() - FORZA_THROTTLE_CURVE).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn activating_immersive_forza_profile_writes_layered_preset() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let router = app(state.clone());

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/controllers/edge-forza/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/forza-horizon-immersive/activate")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        assert_eq!(
            inner.active_profile_id.as_deref(),
            Some(IMMERSIVE_PROFILE_ID)
        );
        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("controller config still present");
        let effect = |id: &str| {
            config
                .forza
                .effects
                .iter()
                .find(|effect| effect.id == id)
                .unwrap_or_else(|| panic!("immersive preset contains '{id}'"))
        };

        assert!(effect("tire_slip").enabled);
        assert_eq!(effect("tire_slip").intensity, 30);
        assert_eq!(effect("tire_slip").route, "body_right");
        assert!(effect("suspension_impact").enabled);
        assert_eq!(effect("suspension_impact").intensity, 82);
        assert_eq!(effect("suspension_impact").route, "body_both");
        assert!(effect("puddle_drag").enabled);
        assert_eq!(effect("puddle_drag").route, "body_left");
        assert!(
            !effect("rpm_leds").enabled,
            "Immersive should leave gear LEDs and the RPM bar disabled"
        );
        assert_eq!(config.trigger.l2_from, 0);
        assert_eq!(config.trigger.r2_from, 4);
        assert_eq!(config.trigger.l2_to, 100);
        assert_eq!(config.trigger.r2_to, 100);
        assert!((config.trigger.l2_curve.as_f64() - FORZA_BRAKE_CURVE).abs() < f64::EPSILON);
        assert!((config.trigger.r2_curve.as_f64() - FORZA_THROTTLE_CURVE).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn activating_user_profile_leaves_controller_config_alone() {
        // User-created profiles have no preset (`forza_preset_for_profile`
        // returns None), so activating one must NOT touch the user's
        // current Forza tuning.
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let router = app(state.clone());

        // Materialize the controller config and seed a user-created
        // profile by writing it into state directly (the public API is
        // `POST /api/profiles` — exercised elsewhere).
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/controllers/edge-forza/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        {
            let mut inner = state.inner.write().await;
            inner.profiles.push(ProfileSummary {
                id: "my-custom-profile".to_string(),
                name: "My Custom Profile".to_string(),
                built_in: false,
                active: false,
                game_id: None,
            });
        }

        let baseline_forza = {
            let inner = state.inner.read().await;
            inner
                .controller_configs
                .get("edge-forza")
                .map(|config| config.forza.clone())
                .expect("controller config materialized by GET")
        };

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/my-custom-profile/activate")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        assert_eq!(
            inner.active_profile_id.as_deref(),
            Some("my-custom-profile")
        );
        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("controller config still present");
        assert_eq!(
            config.forza, baseline_forza,
            "activating a user-created profile must not rewrite saved Forza config"
        );
    }

    #[tokio::test]
    async fn saved_profile_config_persists_and_reapplies_on_activation() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let router = app(state.clone());

        let config: ControllerConfig = get_json(
            router.clone(),
            "/api/controllers/edge-forza/config",
            StatusCode::OK,
        )
        .await;
        assert!(
            !config.profile_assignments.is_empty(),
            "profile assignments should start materialized"
        );
        {
            let mut inner = state.inner.write().await;
            inner.profiles.push(ProfileSummary {
                id: "track-focus".to_string(),
                name: "Track Focus".to_string(),
                built_in: false,
                active: false,
                game_id: None,
            });
        }

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profiles/track-focus/config")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{
                            "inputMode":"native_dualsense",
                            "trigger":{
                                "sameRange":false,
                                "l2From":12,
                                "l2To":82,
                                "r2From":9,
                                "r2To":94,
                                "effect":"Pulse",
                                "intensity":"Weak",
                                "vibration":"High"
                            },
                            "lightbar":{
                                "enabled":true,
                                "color":"#ff8800",
                                "brightness":33
                            },
                            "forza":{
                                "effects":[
                                    {
                                        "id":"road_texture",
                                        "enabled":true,
                                        "intensity":143,
                                        "route":"body_left"
                                    }
                                ]
                            },
                            "sticks":{
                                "leftCurve":"Precise",
                                "leftCurveAmount":41,
                                "leftDeadzone":3,
                                "rightCurve":"Steady",
                                "rightCurveAmount":58,
                                "rightDeadzone":7
                            },
                            "buttons":[
                                {"key":"Back Left","label":"Previous DSCC Profile"}
                            ]
                        }"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/track-focus/activate")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("controller config still present");
        assert_eq!(config.trigger.effect, "Pulse");
        assert_eq!(config.trigger.intensity, "Weak");
        assert_eq!(config.lightbar.color, "#ff8800");
        assert_eq!(config.lightbar.brightness, 33);
        assert_eq!(config.sticks.left_curve, "Precise");
        assert!(
            !config.profile_assignments.is_empty(),
            "applying a saved profile config must preserve controller profile assignments"
        );

        let road = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "road_texture")
            .expect("road_texture present after activation");
        assert!(road.enabled);
        assert_eq!(road.intensity, 143);
        assert_eq!(road.route, "body_left");

        let persisted = PersistedAgentState::from_inner(&inner);
        assert!(persisted.profile_configs.contains_key("track-focus"));
        let reloaded = persisted.normalized();
        let reloaded_config = reloaded
            .profile_configs
            .get("track-focus")
            .expect("profile config survives normalization");
        assert_eq!(reloaded_config.lightbar.color, "#ff8800");
        assert_eq!(reloaded_config.trigger.intensity, "Weak");
    }

    #[tokio::test]
    async fn built_in_forza_profile_cannot_be_overwritten() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-forza",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(84),
        )]);
        let router = app(state.clone());

        let _: ControllerConfig = get_json(
            router.clone(),
            "/api/controllers/edge-forza/config",
            StatusCode::OK,
        )
        .await;

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profiles/forza-horizon/config")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{
                            "inputMode":"native_dualsense",
                            "trigger":{
                                "sameRange":false,
                                "l2From":12,
                                "l2To":82,
                                "r2From":9,
                                "r2To":94,
                                "effect":"Pulse",
                                "intensity":"Weak",
                                "vibration":"High"
                            },
                            "lightbar":{
                                "enabled":true,
                                "color":"#ff8800",
                                "brightness":33
                            },
                            "forza":{
                                "effects":[
                                    {
                                        "id":"road_texture",
                                        "enabled":true,
                                        "intensity":150,
                                        "route":"body_both"
                                    }
                                ]
                            },
                            "sticks":{
                                "leftCurve":"Precise",
                                "leftCurveAmount":41,
                                "leftDeadzone":3,
                                "rightCurve":"Steady",
                                "rightCurveAmount":58,
                                "rightDeadzone":7
                            },
                            "buttons":[]
                        }"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        {
            let mut stale = ProfileConfig::from_controller_config(&ControllerConfig::default_for(
                "",
                "DualSense Edge",
            ));
            let road = stale
                .forza
                .effects
                .iter_mut()
                .find(|effect| effect.id == "road_texture")
                .expect("road_texture exists");
            road.enabled = true;
            road.intensity = 150;
            road.route = "body_both".to_string();

            let mut inner = state.inner.write().await;
            inner
                .profile_configs
                .insert("forza-horizon".to_string(), stale);
        }

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/profiles/forza-horizon/activate")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let inner = state.inner.read().await;
        let config = inner
            .controller_configs
            .get("edge-forza")
            .expect("controller config still present");
        let road = config
            .forza
            .effects
            .iter()
            .find(|effect| effect.id == "road_texture")
            .expect("road_texture present after activation");
        assert!(
            road.enabled,
            "activating the Base profile must use the built-in preset, not a stale saved override"
        );
        assert_eq!(road.intensity, 40);

        let persisted = PersistedAgentState::from_inner(&inner);
        assert!(
            !persisted.profile_configs.contains_key("forza-horizon"),
            "stock built-in profile configs should never be persisted"
        );
        assert!(!persisted
            .normalized()
            .profile_configs
            .contains_key("forza-horizon"));
    }

    #[tokio::test]
    async fn valid_forza_packet_switches_runtime_to_connected() {
        let state = AgentState::mock();
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
        }
        let mut packet = vec![0_u8; 324];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 204;
        let parsed = parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 7)
            .expect("packet parses");

        state
            .apply_adapter_packet(parsed.adapter_id, parsed.packet_len, 7, parsed.updates)
            .await;

        let inner = state.inner.read().await;
        assert_eq!(inner.active_adapter_id.as_deref(), Some("forza-data-out"));
        assert_eq!(
            inner
                .require_adapter_runtime(FORZA_DATA_OUT_ADAPTER_ID)
                .packet_count,
            1
        );
        let adapters = materialized_adapters(&inner, None);
        let forza = adapters
            .iter()
            .find(|adapter| adapter.id == "forza-data-out")
            .expect("Forza adapter exists");
        assert_eq!(forza.state, "connected");

        let telemetry = materialized_telemetry_response(&inner, Some(&detection));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "source.id" && signal.value == serde_json::json!("forza-data-out")
        }));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "game.id" && signal.value == serde_json::json!("forza-horizon-6")
        }));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "vehicle.speed_kmh" && signal.value == serde_json::json!(108.0)
        }));
    }

    #[tokio::test]
    async fn forza_packet_rate_is_materialized_from_runtime_packets() {
        let state = AgentState::mock();
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_bound("127.0.0.1:5300".parse().unwrap());
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .rate_window_started_at = Some(Instant::now() - Duration::from_secs(2));
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .rate_window_packet_count = 119;
        }
        let mut packet = vec![0_u8; 324];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 204;
        let parsed = parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 9)
            .expect("packet parses");

        state
            .apply_adapter_packet(parsed.adapter_id, parsed.packet_len, 9, parsed.updates)
            .await;

        let inner = state.inner.read().await;
        let adapters = materialized_adapters(&inner, Some(&detection));
        let forza = adapters
            .iter()
            .find(|adapter| adapter.id == "forza-data-out")
            .expect("Forza adapter exists");
        let packet_rate_hz = forza.packet_rate_hz.expect("packet rate is materialized");
        assert!((59..=60).contains(&packet_rate_hz));

        let telemetry = materialized_telemetry_response(&inner, Some(&detection));
        assert!(telemetry.iter().any(|signal| {
            signal.name == "source.packet_rate_hz"
                && signal.value.as_f64() == Some(f64::from(packet_rate_hz))
        }));
    }

    #[tokio::test]
    async fn short_forza_horizon_packet_gear_change_latches_shift_thump() {
        let state = AgentState::mock();
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

        for (sequence, gear) in [(11, 3_u8), (12, 4_u8)] {
            let mut packet = vec![0_u8; 323];
            write_i32(&mut packet, 0, 1);
            write_f32(&mut packet, 8, 8_000.0);
            write_f32(&mut packet, 16, 6_000.0);
            write_f32(&mut packet, 244 + 12, 30.0);
            packet[244 + 71] = 255;
            packet[244 + 75] = gear;

            let parsed = parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, sequence)
                .expect("packet parses");
            state
                .apply_adapter_packet(
                    parsed.adapter_id,
                    parsed.packet_len,
                    sequence,
                    parsed.updates,
                )
                .await;
        }

        let inner = state.inner.read().await;
        assert_eq!(inner.telemetry.number("drivetrain.gear"), Some(4.0));
        let response = current_effect_response(&inner, Some(&detection), false);

        assert!(response
            .parity_effects
            .iter()
            .any(|effect| effect.id == "gear_shift_thump" && effect.state == "active"));
        match response.output.r2 {
            TriggerOutput::PulseAb {
                strength,
                frequency_hz,
                wall_zones,
            } => {
                assert!((frequency_hz - FORZA_SHIFT_FREQUENCY_HZ).abs() < f64::EPSILON);
                assert_eq!(wall_zones, 4);
                assert!(
                    strength > 0.95,
                    "shift thump should use the full configured wall-form kick, got {strength}"
                );
            }
            other => {
                panic!(
                    "expected short Horizon gear change to drive R2 wall-form shift thump, got {other:?}"
                )
            }
        }
    }

    #[tokio::test]
    async fn real_controller_replaces_windows_pnp_fallback() {
        let state = AgentState::from_controller_events([attach_event(
            "windows-pnp-dualsense-edge",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            None,
        )]);

        state
            .apply_controller_event(attach_event(
                "controller-0001",
                ControllerFamily::DualSenseEdge,
                ControllerTransportKind::Bluetooth,
                Some(100),
            ))
            .await;

        let controllers = state.inner.read().await.controllers.summaries();
        assert_eq!(controllers.len(), 1);
        assert_eq!(controllers[0].id, "controller-0001");

        state
            .apply_controller_event(attach_event(
                "windows-pnp-dualsense-edge",
                ControllerFamily::DualSenseEdge,
                ControllerTransportKind::Bluetooth,
                None,
            ))
            .await;
        let controllers = state.inner.read().await.controllers.summaries();
        assert_eq!(controllers.len(), 1);
        assert_eq!(controllers[0].id, "controller-0001");
    }

    #[tokio::test]
    async fn reattach_replaces_disconnected_duplicate_identity() {
        let state = AgentState::from_controller_events([attach_event(
            "edge-old",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Bluetooth,
            Some(80),
        )]);

        state
            .apply_controller_event(ControllerDiscoveryEvent::Detached(ControllerId(
                "edge-old".to_string(),
            )))
            .await;
        state
            .apply_controller_event(attach_event(
                "edge-new",
                ControllerFamily::DualSenseEdge,
                ControllerTransportKind::Bluetooth,
                Some(79),
            ))
            .await;

        let controllers = state.inner.read().await.controllers.summaries();
        assert_eq!(controllers.len(), 1);
        assert_eq!(controllers[0].id, "edge-new");
        assert!(controllers[0].connected);
    }

    #[tokio::test]
    async fn full_battery_state_reports_full_percent() {
        let mut event = attach_event(
            "edge-full",
            ControllerFamily::DualSenseEdge,
            ControllerTransportKind::Usb,
            None,
        );
        let ControllerDiscoveryEvent::Attached(controller) = &mut event else {
            panic!("test fixture should create an attach event");
        };
        controller.state.battery_state = BatteryState::Full;

        let router = app(AgentState::from_controller_events([event]));
        let controllers: Vec<ControllerSummary> =
            get_json(router, "/api/controllers", StatusCode::OK).await;

        assert_eq!(controllers[0].battery_percent, Some(100));
        assert_eq!(controllers[0].battery_state, BatteryState::Full);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_pnp_edge_suppresses_generic_wireless_controller_alias() {
        let events = windows_pnp_controller_events_from_text(
            "DualSense Edge Wireless Controller\tHID\\VID_054C&PID_0DF2\nWireless Controller\tBTHENUM\\PRIVATE",
        );

        assert_eq!(events.len(), 1);
        let ControllerDiscoveryEvent::Attached(controller) = &events[0] else {
            panic!("Windows PnP fallback should create attach events");
        };
        assert_eq!(controller.info.family, ControllerFamily::DualSenseEdge);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_setupapi_multisz_hardware_id_feeds_pnp_classifier() {
        let mut units = Vec::new();
        for part in [
            "HID\\VID_054C&PID_0DF2&REV_0100",
            "DualSense Edge Wireless Controller",
        ] {
            units.extend(part.encode_utf16());
            units.push(0);
        }
        units.push(0);
        let bytes = units
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();

        let text = windows_utf16_bytes_to_search_text(&bytes)
            .expect("SetupAPI UTF-16 text should be decoded");

        assert!(windows_pnp_candidate_text_is_controller(&text));
        let events = windows_pnp_controller_events_from_text(&text);
        assert_eq!(events.len(), 1);
        let ControllerDiscoveryEvent::Attached(controller) = &events[0] else {
            panic!("Windows PnP fallback should create attach events");
        };
        assert_eq!(controller.info.family, ControllerFamily::DualSenseEdge);
    }

    #[test]
    fn forza_trusted_install_path_ignores_untrusted_configured_path_without_steam_catalog() {
        let _env = TestEnv::new(&["DSCC_FORZA_HORIZON6_INSTALL_DIR"]);
        let default_root = temp_test_dir("dscc-forza-default-root");
        let configured_root = temp_test_dir("dscc-forza-configured-root");
        fs::create_dir_all(&default_root).expect("default root fixture");
        fs::create_dir_all(&configured_root).expect("configured root fixture");
        std::env::set_var("DSCC_FORZA_HORIZON6_INSTALL_DIR", &default_root);

        let trusted = trusted_forza_horizon6_install_path(Some(configured_root.clone()), None);

        assert_eq!(
            fs::canonicalize(trusted).expect("trusted path canonicalizes"),
            fs::canonicalize(&default_root).expect("default path canonicalizes")
        );
        let _ = fs::remove_dir_all(default_root);
        let _ = fs::remove_dir_all(configured_root);
    }

    #[test]
    fn forza_trusted_install_path_prefers_discovered_steam_path() {
        let _env = TestEnv::new(&["DSCC_FORZA_HORIZON6_INSTALL_DIR"]);
        let default_root = temp_test_dir("dscc-forza-default-root");
        let steam_root = temp_test_dir("dscc-forza-steam-root");
        fs::create_dir_all(&default_root).expect("default root fixture");
        fs::create_dir_all(&steam_root).expect("steam root fixture");
        std::env::set_var("DSCC_FORZA_HORIZON6_INSTALL_DIR", &default_root);

        let trusted = trusted_forza_horizon6_install_path(None, Some(steam_root.clone()));

        assert_eq!(trusted, steam_root);
        let _ = fs::remove_dir_all(default_root);
        let _ = fs::remove_dir_all(steam_root);
    }

    #[test]
    fn forza_icon_target_guard_rejects_paths_outside_install_root() {
        let root = temp_test_dir("dscc-forza-safe-root");
        let outside_root = temp_test_dir("dscc-forza-outside-root");
        fs::create_dir_all(&root).expect("root fixture");
        fs::create_dir_all(&outside_root).expect("outside fixture");
        let outside_target = outside_root.join("ControllerIcons.zip");

        let error = ensure_forza_icon_target_is_safe(&root, &outside_target)
            .expect_err("outside target should be rejected");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn forza_glyph_installer_backs_up_and_restores_controller_icons() {
        let root =
            std::env::temp_dir().join(format!("dscc-forza-glyph-test-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp glyph test dir should be removable");
        }

        let targets = forza_controller_icon_targets(&root);
        for (index, target) in targets.iter().enumerate() {
            fs::create_dir_all(target.parent().expect("target has parent"))
                .expect("target parent should be creatable");
            fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
        }

        install_forza_playstation_glyphs(root.clone()).expect("glyph install should succeed");
        for (index, target) in targets.iter().enumerate() {
            assert_eq!(
                fs::read(target).expect("installed icon should be readable"),
                FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP
            );
            assert!(
                forza_controller_icon_backup_path(target).exists(),
                "original icon should be backed up"
            );
            assert_eq!(
                fs::read_to_string(forza_controller_icon_backup_path(target))
                    .expect("backup icon should be readable"),
                format!("xbox-icons-{index}")
            );
        }

        restore_forza_original_glyphs(root.clone()).expect("glyph restore should succeed");
        for (index, target) in forza_controller_icon_targets(&root).iter().enumerate() {
            assert_eq!(
                fs::read_to_string(target).expect("restored icon should be readable"),
                format!("xbox-icons-{index}")
            );
        }

        fs::remove_dir_all(&root).expect("temp glyph test dir should be removable");
    }

    #[test]
    fn forza_glyph_installer_refuses_to_install_without_originals() {
        let root = std::env::temp_dir().join(format!(
            "dscc-forza-glyph-missing-originals-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp missing originals dir should be removable");
        }
        fs::create_dir_all(&root).expect("temp missing originals root should be creatable");

        let error = install_forza_playstation_glyphs(root.clone())
            .expect_err("glyph install should refuse missing original icon files");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);

        for target in forza_controller_icon_targets(&root) {
            assert!(
                !target.exists(),
                "installer should not create unbacked PlayStation icon files"
            );
            assert!(
                !forza_controller_icon_backup_path(&target).exists(),
                "installer should not create backups when originals are missing"
            );
        }

        fs::remove_dir_all(&root).expect("temp missing originals dir should be removable");
    }

    #[test]
    fn forza_glyph_installer_recovers_bad_playstation_backups_after_verify() {
        let root = std::env::temp_dir().join(format!(
            "dscc-forza-glyph-recovery-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp glyph recovery dir should be removable");
        }

        let targets = forza_controller_icon_targets(&root);
        for (index, target) in targets.iter().enumerate() {
            fs::create_dir_all(target.parent().expect("target has parent"))
                .expect("target parent should be creatable");
            fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
            fs::write(
                forza_controller_icon_backup_path(target),
                FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP,
            )
            .expect("stale PlayStation backup should be writable");
        }

        install_forza_playstation_glyphs(root.clone()).expect("glyph install should succeed");
        restore_forza_original_glyphs(root.clone()).expect("glyph restore should succeed");

        for (index, target) in targets.iter().enumerate() {
            assert_eq!(
                fs::read_to_string(target).expect("restored icon should be readable"),
                format!("xbox-icons-{index}")
            );
        }

        fs::remove_dir_all(&root).expect("temp glyph recovery dir should be removable");
    }

    #[test]
    fn forza_glyph_restore_succeeds_when_defaults_are_already_present() {
        let root = std::env::temp_dir().join(format!(
            "dscc-forza-glyph-defaults-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp defaults dir should be removable");
        }

        let targets = forza_controller_icon_targets(&root);
        for (index, target) in targets.iter().enumerate() {
            fs::create_dir_all(target.parent().expect("target has parent"))
                .expect("target parent should be creatable");
            fs::write(target, format!("xbox-icons-{index}")).expect("seed icon should be writable");
        }

        let message = restore_forza_original_glyphs(root.clone())
            .expect("restore should no-op when defaults are present");
        assert!(
            message.contains("already using the game defaults"),
            "restore should report a successful no-op"
        );
        for (index, target) in targets.iter().enumerate() {
            assert_eq!(
                fs::read_to_string(target).expect("default icon should remain readable"),
                format!("xbox-icons-{index}")
            );
        }

        fs::remove_dir_all(&root).expect("temp defaults dir should be removable");
    }

    #[test]
    fn forza_glyph_restore_refuses_unbacked_playstation_files() {
        let root = std::env::temp_dir().join(format!(
            "dscc-forza-glyph-unbacked-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp unbacked dir should be removable");
        }

        let targets = forza_controller_icon_targets(&root);
        for target in &targets {
            fs::create_dir_all(target.parent().expect("target has parent"))
                .expect("target parent should be creatable");
            fs::write(target, FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP)
                .expect("PlayStation icon should be writable");
        }

        let error = restore_forza_original_glyphs(root.clone())
            .expect_err("restore should refuse PlayStation icons without backups");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        for target in &targets {
            assert_eq!(
                fs::read(target).expect("PlayStation icon should remain readable"),
                FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP
            );
        }

        fs::remove_dir_all(&root).expect("temp unbacked dir should be removable");
    }

    #[test]
    fn forza_glyph_restore_validates_every_target_before_replacing_files() {
        let root = std::env::temp_dir().join(format!(
            "dscc-forza-glyph-partial-restore-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp partial restore dir should be removable");
        }

        let targets = forza_controller_icon_targets(&root);
        for target in &targets {
            fs::create_dir_all(target.parent().expect("target has parent"))
                .expect("target parent should be creatable");
            fs::write(target, FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP)
                .expect("PlayStation icon should be writable");
        }
        fs::write(
            forza_controller_icon_backup_path(&targets[0]),
            "xbox-icons-restorable",
        )
        .expect("backup icon should be writable");

        let error = restore_forza_original_glyphs(root.clone())
            .expect_err("restore should refuse a partial restore with one unbacked target");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        for target in &targets {
            assert_eq!(
                fs::read(target).expect("PlayStation icon should remain readable"),
                FORZA_PLAYSTATION_CONTROLLER_ICONS_ZIP
            );
        }

        fs::remove_dir_all(&root).expect("temp partial restore dir should be removable");
    }

    #[tokio::test]
    async fn telemetry_endpoint_returns_empty_list_in_mock_state() {
        let router = app(AgentState::mock());

        let signals: Vec<TelemetrySignalResponse> =
            get_json(router, "/api/telemetry", StatusCode::OK).await;

        // Mock state has no active telemetry adapter; real adapters (e.g. Forza
        // Data Out) populate this list once they receive packets.
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn adapters_include_first_wave_catalog() {
        let router = app(AgentState::mock());

        let adapters: Vec<AdapterSummary> = get_json(router, "/api/adapters", StatusCode::OK).await;
        let ids = adapters
            .iter()
            .map(|adapter| adapter.id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"forza-data-out"));
        assert!(ids.contains(&"ea-f1-udp"));
        assert!(ids.contains(&"beamng"));
        assert!(adapters
            .iter()
            .find(|adapter| adapter.id == "forza-data-out")
            .is_some_and(|adapter| adapter.setup_url.is_some()));
    }

    #[tokio::test]
    async fn current_controller_effect_test_returns_dry_run_output() {
        let router = app(AgentState::mock());

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/controllers/current/test-effect")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"target":"r2","mode":"wall","intensity":72,"durationMs":500}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let effect: EffectTestResponse = serde_json::from_slice(&body).unwrap();
        assert!(effect.accepted);
        assert!(effect.dry_run);
        assert!(matches!(effect.output.r2, TriggerOutput::Wall { .. }));
    }

    #[tokio::test]
    async fn permission_denied_event_is_actionable_controller_state() {
        let state = AgentState::from_controller_events([attach_event(
            "locked-pad",
            ControllerFamily::DualSense,
            ControllerTransportKind::Usb,
            Some(72),
        )]);
        state
            .apply_controller_event(ControllerDiscoveryEvent::PermissionDenied(
                DevicePermissionProblem::for_controller(
                    ControllerId("locked-pad".to_string()),
                    ControllerTransportKind::Usb,
                    "udev rules do not allow opening this controller",
                ),
            ))
            .await;
        let router = app(state);

        let detail: ControllerDetail = get_json(
            router.clone(),
            "/api/controllers/locked-pad",
            StatusCode::OK,
        )
        .await;
        assert!(detail.connected);
        assert_eq!(detail.permission, ControllerPermissionState::Denied);
        assert_eq!(
            detail.diagnostic_state,
            ControllerDiagnosticState::PermissionDenied
        );

        let diagnostics: DiagnosticsResponse =
            get_json(router.clone(), "/api/diagnostics", StatusCode::OK).await;
        assert!(diagnostics.checks.iter().any(|check| {
            check.name == "controller:locked-pad"
                && check.status == "blocked"
                && check.detail.contains("udev rules")
        }));

        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/controllers/locked-pad/test-effect")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"target":"r2","mode":"wall","intensity":80}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn detach_event_keeps_known_controller_with_disconnected_diagnostic() {
        let state = AgentState::from_controller_events([attach_event(
            "usb-pad",
            ControllerFamily::DualSense,
            ControllerTransportKind::Usb,
            Some(12),
        )]);
        state
            .apply_controller_event(ControllerDiscoveryEvent::Detached(ControllerId(
                "usb-pad".to_string(),
            )))
            .await;
        let router = app(state);

        let controllers: Vec<ControllerSummary> =
            get_json(router.clone(), "/api/controllers", StatusCode::OK).await;
        assert_eq!(controllers.len(), 1);
        assert!(!controllers[0].connected);
        assert_eq!(
            controllers[0].diagnostic_state,
            ControllerDiagnosticState::Disconnected
        );

        let detail: ControllerDetail =
            get_json(router, "/api/controllers/usb-pad", StatusCode::OK).await;
        assert!(detail
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "controller_disconnected"));
    }

    #[tokio::test]
    async fn unknown_controller_detail_returns_not_found() {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/controllers/no-such-controller")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    fn make_user_game(
        app_id: &str,
        name: &str,
        install_path: &str,
        processes: &[&str],
    ) -> UserGameConfig {
        UserGameConfig {
            game_id: user_game_id_for_app_id(app_id),
            app_id: app_id.to_string(),
            name: name.to_string(),
            install_dir: name.replace(' ', ""),
            install_path: install_path.to_string(),
            process_names: processes.iter().map(|s| s.to_string()).collect(),
            added_at: current_timestamp(),
        }
    }

    fn make_test_steam_root(prefix: &str) -> PathBuf {
        let root = temp_test_dir(prefix);
        let steamapps = root.join("steamapps");
        let common = steamapps.join("common");
        fs::create_dir_all(&common).expect("steam common");
        root
    }

    fn install_test_steam_manifest(
        steam_root: &FsPath,
        app_id: &str,
        name: &str,
        install_dir: &str,
        exe_names: &[&str],
    ) {
        let steamapps = steam_root.join("steamapps");
        let manifest_path = steamapps.join(format!("appmanifest_{app_id}.acf"));
        let manifest = format!(
            r#""AppState"
{{
    "appid"        "{app_id}"
    "name"        "{name}"
    "installdir"        "{install_dir}"
    "Universe"        "1"
}}
"#
        );
        fs::write(&manifest_path, manifest).expect("write appmanifest");
        let install_path = steamapps.join("common").join(install_dir);
        fs::create_dir_all(&install_path).expect("create install dir");
        for exe in exe_names {
            fs::write(install_path.join(exe), [0_u8; 4]).expect("write fake exe");
        }
    }

    #[test]
    fn user_game_id_uses_custom_prefix() {
        assert_eq!(user_game_id_for_app_id("12345"), "custom-12345");
    }

    #[test]
    fn user_game_process_candidates_filter_known_uninstaller_patterns() {
        let install_dir = temp_test_dir("dscc-user-game-procs");
        fs::create_dir_all(&install_dir).expect("install dir");
        for name in [
            "Game.exe",
            "GameLauncher.exe",
            "UnityCrashHandler.exe",
            "uninstall.exe",
            "setup.exe",
            "vcredist_x64.exe",
            "EasyAntiCheat.exe",
            "readme.txt",
        ] {
            fs::write(install_dir.join(name), [0_u8; 4]).expect("touch file");
        }
        let candidates = discover_user_game_process_candidates(&install_dir);
        assert!(candidates.iter().any(|n| n == "Game.exe"));
        assert!(candidates.iter().any(|n| n == "GameLauncher.exe"));
        assert!(!candidates
            .iter()
            .any(|n| n.eq_ignore_ascii_case("UnityCrashHandler.exe")));
        assert!(!candidates
            .iter()
            .any(|n| n.eq_ignore_ascii_case("uninstall.exe")));
        assert!(!candidates
            .iter()
            .any(|n| n.eq_ignore_ascii_case("setup.exe")));
        assert!(!candidates
            .iter()
            .any(|n| n.eq_ignore_ascii_case("vcredist_x64.exe")));
        assert!(!candidates
            .iter()
            .any(|n| n.eq_ignore_ascii_case("EasyAntiCheat.exe")));
        let _ = fs::remove_dir_all(install_dir);
    }

    #[tokio::test]
    async fn steam_library_endpoint_lists_installed_games() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-steam-lib-list");
        install_test_steam_manifest(
            &steam_root,
            FORZA_HORIZON5_STEAM_APP_ID,
            "Forza Horizon 5",
            "ForzaHorizon5",
            &["ForzaHorizon5.exe"],
        );
        install_test_steam_manifest(
            &steam_root,
            "987654321",
            "Imaginary Indie Racer",
            "ImaginaryRacer",
            &["ImaginaryRacer.exe", "uninstall.exe"],
        );
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/games/steam-library")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let games = parsed
            .get("games")
            .and_then(|games| games.as_array())
            .expect("games array");
        let fh5 = games
            .iter()
            .find(|game| {
                game.get("appId").and_then(|app_id| app_id.as_str())
                    == Some(FORZA_HORIZON5_STEAM_APP_ID)
            })
            .expect("FH5 present");
        assert_eq!(
            fh5.get("alreadyInCatalog")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            fh5.get("suggestedGameId").and_then(|value| value.as_str()),
            Some(format!("custom-{FORZA_HORIZON5_STEAM_APP_ID}").as_str())
        );
        let indie = games
            .iter()
            .find(|game| game.get("appId").and_then(|value| value.as_str()) == Some("987654321"))
            .expect("imaginary indie present");
        assert_eq!(
            indie
                .get("alreadyInCatalog")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        let process_candidates = indie
            .get("processCandidates")
            .and_then(|value| value.as_array())
            .expect("process candidates");
        let process_names: Vec<&str> = process_candidates
            .iter()
            .filter_map(|value| value.as_str())
            .collect();
        assert!(process_names.contains(&"ImaginaryRacer.exe"));
        assert!(!process_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case("uninstall.exe")));
        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn add_custom_game_rejects_unknown_app_id() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-add-custom-404");
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/games/custom")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"appId":"42"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn add_custom_game_rejects_duplicate_registration() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-add-custom-dup");
        install_test_steam_manifest(
            &steam_root,
            "555000",
            "Sample Racer",
            "SampleRacer",
            &["SampleRacer.exe"],
        );
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        let router = app(AgentState::mock());
        let first = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/games/custom")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"appId":"555000"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let body = to_bytes(first.into_body(), 64 * 1024).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let game = parsed.get("game").expect("game key");
        assert_eq!(
            game.get("gameId").and_then(|value| value.as_str()),
            Some("custom-555000")
        );
        assert_eq!(
            game.get("supportLevel").and_then(|value| value.as_str()),
            Some("custom")
        );

        let duplicate = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/games/custom")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"appId":"555000"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);
        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn add_and_remove_custom_game_round_trips() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-add-remove-custom");
        install_test_steam_manifest(
            &steam_root,
            "777111",
            "Removable Racer",
            "RemovableRacer",
            &["RemovableRacer.exe"],
        );
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        let state = AgentState::mock();
        let router = app(state.clone());

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/games/custom")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"appId":"777111"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        {
            let inner = state.inner.read().await;
            assert!(inner.user_games.contains_key("custom-777111"));
        }

        let delete_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/games/custom/custom-777111")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let missing_delete = router
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/api/games/custom/custom-777111")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_delete.status(), StatusCode::NOT_FOUND);

        {
            let inner = state.inner.read().await;
            assert!(!inner.user_games.contains_key("custom-777111"));
        }
        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn browse_steam_library_lists_root_entries() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-browse-root");
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        install_test_steam_manifest(
            &steam_root,
            "9911",
            "Browse Test Game",
            "BrowseTestGame",
            &["LauncherA.exe", "GameB.exe"],
        );
        // Add a nested directory so we can confirm directories are surfaced.
        let install_path = steam_root
            .join("steamapps")
            .join("common")
            .join("BrowseTestGame");
        fs::create_dir_all(install_path.join("Binaries").join("Win64")).expect("nested dirs");
        fs::write(
            install_path
                .join("Binaries")
                .join("Win64")
                .join("Game-Shipping.exe"),
            [0_u8; 4],
        )
        .expect("nested exe");

        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/games/steam-library/browse?appId=9911&path=")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: SteamLibraryBrowseResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.app_id, "9911");
        assert_eq!(payload.relative_path, "");
        assert!(!payload.truncated);
        let names: Vec<_> = payload.entries.iter().map(|e| e.name.as_str()).collect();
        // Directories sort first, then exes — alphabetical within each group.
        assert_eq!(names, vec!["Binaries", "GameB.exe", "LauncherA.exe"]);
        let kinds: Vec<_> = payload.entries.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, vec!["dir", "exe", "exe"]);

        // Walk into the nested directory.
        let nested = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/games/steam-library/browse?appId=9911&path=Binaries/Win64")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(nested.status(), StatusCode::OK);
        let body = to_bytes(nested.into_body(), 1024 * 1024).await.unwrap();
        let payload: SteamLibraryBrowseResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.relative_path, "Binaries/Win64");
        let names: Vec<_> = payload.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Game-Shipping.exe"]);

        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn browse_steam_library_blocks_path_traversal() {
        let _env = TestEnv::new(&[
            "DSCC_STEAM_ROOT",
            "ProgramFiles(x86)",
            "ProgramFiles",
            "LOCALAPPDATA",
        ]);
        let steam_root = make_test_steam_root("dscc-browse-traversal");
        std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
        std::env::remove_var("ProgramFiles(x86)");
        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");

        install_test_steam_manifest(
            &steam_root,
            "9912",
            "Traversal Test",
            "TraversalTest",
            &["GameOnly.exe"],
        );

        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .uri("/api/games/steam-library/browse?appId=9912&path=../..")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let _ = fs::remove_dir_all(&steam_root);
    }

    #[tokio::test]
    async fn snapshot_supported_games_includes_user_games_with_custom_support_level() {
        let state = AgentState::mock();
        {
            let mut inner = state.inner.write().await;
            inner.user_games.insert(
                "custom-12345".to_string(),
                make_user_game(
                    "12345",
                    "Test Custom Game",
                    "C:/dscc/fake/install",
                    &["TestCustomGame.exe"],
                ),
            );
        }
        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let snapshot: AgentSnapshotResponse = serde_json::from_slice(&body).unwrap();
        let custom = snapshot
            .game_detection
            .supported_games
            .iter()
            .find(|game| game.game_id == "custom-12345")
            .expect("custom game appears in supported_games");
        assert_eq!(custom.support_level, "custom");
        assert_eq!(custom.app_id.as_deref(), Some("12345"));
        assert_eq!(custom.name, "Test Custom Game");
    }

    #[test]
    fn persisted_state_round_trips_user_games() {
        let mut inner_user_games = BTreeMap::new();
        inner_user_games.insert(
            "custom-99887".to_string(),
            make_user_game(
                "99887",
                "Round Trip Racer",
                "C:/dscc/round-trip",
                &["RoundTrip.exe"],
            ),
        );
        let persisted = PersistedAgentState {
            version: PERSISTED_STATE_VERSION,
            user_games: inner_user_games.clone(),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&persisted).expect("serialize");
        let deserialized: PersistedAgentState =
            serde_json::from_str(&serialized).expect("deserialize");
        let normalized = deserialized.normalized();
        assert!(normalized.user_games.contains_key("custom-99887"));
        let restored = normalized
            .user_games
            .get("custom-99887")
            .expect("user game survives");
        assert_eq!(restored.name, "Round Trip Racer");
        assert_eq!(restored.process_names, vec!["RoundTrip.exe".to_string()]);
    }

    #[test]
    fn normalization_drops_user_games_that_collide_with_built_in_modules() {
        let mut user_games = BTreeMap::new();
        // Use a built-in module id (forza-horizon-5) as the user game id;
        // normalization should drop it.
        user_games.insert(
            "forza-horizon-5".to_string(),
            UserGameConfig {
                game_id: "forza-horizon-5".to_string(),
                app_id: FORZA_HORIZON5_STEAM_APP_ID.to_string(),
                name: "Bad clone".to_string(),
                install_dir: "FH5".to_string(),
                install_path: "C:/whatever".to_string(),
                process_names: vec!["ForzaHorizon5.exe".to_string()],
                added_at: current_timestamp(),
            },
        );
        let persisted = PersistedAgentState {
            version: PERSISTED_STATE_VERSION,
            user_games,
            ..Default::default()
        };
        let normalized = persisted.normalized();
        assert!(normalized.user_games.is_empty());
    }

    #[test]
    fn process_detection_matches_user_game_process_name() {
        let mut user_games = BTreeMap::new();
        user_games.insert(
            "custom-99887".to_string(),
            make_user_game(
                "99887",
                "Round Trip Racer",
                "C:/dscc/round-trip",
                &["RoundTrip.exe"],
            ),
        );
        let detection =
            detect_running_game_from_processes_with_user_games(["RoundTrip.exe"], &user_games);
        assert_eq!(detection.active_game_id.as_deref(), Some("custom-99887"));
        assert_eq!(detection.module_id.as_deref(), Some("custom-99887"));
        // Custom games do not have a telemetry adapter; the response omits
        // the adapter id and profile id.
        assert!(detection.adapter_id.is_none());
        assert!(detection.profile_id.is_none());
        assert_eq!(detection.candidates.len(), 1);
    }

    #[test]
    fn user_game_detection_keeps_global_profile_until_supported_module_matches() {
        let mut user_games = BTreeMap::new();
        user_games.insert(
            "custom-99887".to_string(),
            make_user_game(
                "99887",
                "Round Trip Racer",
                "C:/dscc/round-trip",
                &["RoundTrip.exe"],
            ),
        );
        let detection =
            detect_running_game_from_processes_with_user_games(["RoundTrip.exe"], &user_games);
        let mut state = AgentStateInner {
            controllers: ControllerRegistry::default(),
            controller_names: BTreeMap::new(),
            profiles: profiles_with_active(
                default_profiles(),
                &Some(DEFAULT_PROFILE_ID.to_string()),
            ),
            adapters: default_adapters(),
            telemetry: SignalSnapshot::default(),
            logs: Vec::new(),
            device_backend: DeviceBackendSummary::mock(),
            storage: None,
            controller_configs: BTreeMap::new(),
            profile_configs: BTreeMap::new(),
            profile_overrides: BTreeMap::new(),
            edge_profiles: BTreeMap::new(),
            app_settings: AppSettings::default(),
            active_profile_id: Some(DEFAULT_PROFILE_ID.to_string()),
            active_adapter_id: None,
            auto_loaded_profile_id: None,
            adapter_runtimes: default_adapter_runtimes(),
            forza_effect_runtime: ForzaEffectRuntime::default(),
            effect_revision: 0,
            user_games,
        };

        assert!(!sync_auto_loaded_profile_for_detection(
            &mut state, &detection
        ));
        assert_eq!(state.active_profile_id.as_deref(), Some(DEFAULT_PROFILE_ID));
        assert_eq!(state.auto_loaded_profile_id, None);
    }

    async fn get_json<T>(router: Router, uri: &str, expected_status: StatusCode) -> T
    where
        T: DeserializeOwned,
    {
        let response = router
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn attach_event(
        id: &str,
        family: ControllerFamily,
        transport: ControllerTransportKind,
        battery_percent: Option<u8>,
    ) -> ControllerDiscoveryEvent {
        let info = ControllerInfo {
            id: ControllerId(id.to_string()),
            vendor_id: 0,
            product_id: 0,
            family,
            transport,
            connection: ConnectionState::Connected,
            capabilities: ControllerCapabilities {
                adaptive_triggers: true,
                lightbar: true,
                player_leds: true,
                rumble: true,
                microphone_led: true,
                edge_buttons: family == ControllerFamily::DualSenseEdge,
            },
        };
        let state = ControllerState {
            id: info.id.clone(),
            connection: ConnectionState::Connected,
            battery_percent,
            battery_state: BatteryState::Discharging,
        };

        ControllerDiscoveryEvent::Attached(
            DiscoveredController::new(info, state)
                .with_name(format!("{id} test controller"))
                .with_diagnostic(ControllerDiagnostic::info(
                    "test_fixture",
                    "controller added by test fixture",
                )),
        )
    }

    fn write_i32(packet: &mut [u8], offset: usize, value: i32) {
        packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f32(packet: &mut [u8], offset: usize, value: f32) {
        packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
