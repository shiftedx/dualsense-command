#[cfg(any(test, not(target_os = "windows")))]
use std::collections::BTreeSet;
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
    AdapterProtocol, UdpTelemetryAdapter,
};
#[cfg(test)]
use dscc_core::ControllerFamily;
use dscc_core::{
    input_bridge::{
        InputBridgeBindingConfig, InputBridgeConfig, InputBridgeSource, InputBridgeTarget,
        VirtualAxis, VirtualButton,
    },
    BatteryState, ComparableValue, ComparisonOp, ConnectionState, ControllerCapabilities,
    ControllerId, ControllerInfo, ControllerOutputFrame, ControllerState, ControllerTransportKind,
    EffectEngine, EffectRule, EffectTarget, EffectTemplate, LightbarOutput, PlayerLedsOutput,
    Profile, RgbColor, RuleCondition, RumbleOutput, RumblePolicy, TriggerOutput, ValuePoint,
    ValueSource,
};
use dscc_device::{
    edge_onboard_transport_supported, edge_onboard_write_transport_supported,
    ControllerInputReadOptions, ControllerInputState, ControllerOutputManager,
    ControllerOutputTarget, ControllerOutputWrite, DeviceConfig, DeviceManager, DeviceTransport,
    DeviceTransportKind, EdgeButton, EdgeButtonMapping, EdgeOnboardProfile, EdgeOnboardSlotId,
    EdgeProfileIntensity, EdgeStickPreset, EdgeStickProfile, EdgeTriggerDeadzone, HidApiTransport,
    OutputMode, RawDeviceId,
};
use dscc_telemetry::{SignalName, SignalSnapshot, SignalUpdate, SignalValue};
use dscc_virtual_output::VirtualOutputKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::{broadcast, Mutex as AsyncMutex, RwLock},
};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

mod adapter_runtime;
mod bind_addr;
mod controller_registry;
mod edge_profiles;
mod env_policy;
mod forza_glyphs;
mod game_detection;
mod game_modules;
mod http_security;
mod input_bridge;
mod persistence;
mod profiles;
mod routes;
mod steam_input;
mod support_bundle;
mod update_check;

pub use bind_addr::{
    resolve_agent_bind_addr, DEFAULT_BIND_ADDR, DEFAULT_FORZA_BIND_ADDR, FORZA_BIND_ADDR_ENV,
    FORZA_LAN_ENABLE_ENV, LAN_API_ENABLE_ENV,
};

#[cfg(test)]
pub(crate) use adapter_runtime::TELEMETRY_PACKET_STALE_AFTER;
pub(crate) use adapter_runtime::{
    adapter_runtime_health_check, adapter_state_label, apply_adapter_runtime_summary,
    default_adapter_runtimes, materialized_adapters, AdapterRuntime,
};
pub(crate) use bind_addr::{
    default_agent_bind_addr, desired_agent_bind_addr, lan_api_enabled, resolve_forza_bind_addr,
};
#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
pub(crate) use controller_registry::mock_device_manager;
pub(crate) use controller_registry::{
    controller_events_from_device_manager, is_windows_pnp_controller_id, ControllerRegistry,
};
#[cfg(all(test, target_os = "windows"))]
pub(crate) use controller_registry::{
    windows_pnp_candidate_text_is_controller, windows_pnp_controller_events_from_text,
    windows_utf16_bytes_to_search_text,
};
pub(crate) use edge_profiles::{get_edge_profiles, write_edge_profile};
pub use edge_profiles::{
    EdgeProfileSlot, EdgeProfileSlotConfig, EdgeProfileSlotState, EdgeProfileStore,
    EdgeProfileSupportState, EdgeProfilesResponse, UpdateEdgeProfileRequest,
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
#[cfg(all(test, target_os = "windows"))]
pub(crate) use game_detection::local_app_process_path_allowed;
#[cfg(target_os = "windows")]
pub(crate) use game_detection::windows_process_running;
pub(crate) use game_detection::{
    add_custom_game, add_local_game, append_user_games_to_detection, browse_steam_library,
    detect_running_game, detection_allows_input_bridge, discover_steam_game_catalog,
    enrich_game_detection, get_detected_game, get_game_art, get_steam_app_art, list_steam_library,
    local_app_execution_verified_for_input_bridge, remove_custom_game,
    steam_root_and_stats_for_user_games, supported_game_install_path, telemetry_game_detection,
    unsupported_steam_game_catalog, validate_local_game, SteamGameCatalog,
};
#[cfg(test)]
pub(crate) use game_detection::{
    build_supported_steam_game_catalog, discover_user_game_process_candidates,
    parse_steam_achievement_progress_cache, parse_steam_app_manifest, parse_steam_library_folders,
    parse_steam_localconfig_stats, parse_unix_process_names, user_game_id_for_app_id,
    SteamAppManifest, USER_GAME_PROCESS_CANDIDATE_LIMIT,
};
pub use game_detection::{
    AddLocalGameRequest, AddUserGameRequest, AddUserGameResponse, BrowseSteamLibraryParams,
    GameArtwork, GameDetectionCandidate, GameDetectionResponse, SteamAchievementStats,
    SteamGameStats, SteamLibraryBrowseEntry, SteamLibraryBrowseResponse, SteamLibraryEntry,
    SteamLibraryListResponse, SupportedGameSummary, UserGameConfig, ValidateLocalGameRequest,
    ValidateLocalGameResponse,
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
pub(crate) use input_bridge::{
    InputBridgeService, InputBridgeSessionState, InputBridgeSessionSummary,
    InputBridgeStatusResponse,
};
#[cfg(test)]
pub(crate) use persistence::PERSISTED_STATE_VERSION;
pub(crate) use persistence::{
    build_persist_snapshot, persist_snapshot, PersistedAgentState, PersistenceStore,
};
#[cfg(test)]
pub(crate) use profiles::default_profiles;
pub(crate) use profiles::{
    apply_profile_config_to_controllers, apply_profile_selection_config,
    default_profile_assignments, is_default_profile_id, merge_profiles,
    model_hint_for_profile_buttons, normalize_existing_profile_assignments,
    normalize_optional_profile_game_id, normalize_profile_assignments,
    profile_exists_in_defaults_or_persisted, profile_override_key, profile_resolution,
    profiles_with_active, slugify, sync_auto_loaded_profile_for_detection, SelectedProfileConfig,
};
pub use routes::app;
pub(crate) use routes::{configured_web_dist_dir, web_dist_dir};
#[cfg(test)]
pub(crate) use routes::{web_dist_candidates, web_dist_dir_from_parts};
pub(crate) use steam_input::{
    discover_steam_input_status_async, numeric_child_dirs, pending_steam_input_status,
    quoted_tokens, steam_input_discovery_pending, steam_root_candidates, write_steam_input_binding,
    write_steam_input_paddle_preset,
};
#[cfg(test)]
pub(crate) use steam_input::{
    ensure_dualsense_edge_steam_layout, mark_dscc_steam_profile_metadata, parse_steam_input_layout,
    replace_steam_binding_value, sanitized_steam_path, steam_edge_paddle_binding,
    validated_steam_input_layout_path, STEAM_EDGE_BACK_RIGHT_INPUT_ID,
};
pub use steam_input::{
    SteamInputBinding, SteamInputBindingWriteRequest, SteamInputBindingWriteResponse,
    SteamInputLayout, SteamInputPaddlePresetPaddleResult, SteamInputPaddlePresetRequest,
    SteamInputPaddlePresetResponse, SteamInputStatus,
};
pub(crate) use support_bundle::sanitize_diagnostics_response;
#[cfg(test)]
pub(crate) use support_bundle::{
    sanitize_support_text, support_steam_input_summary, support_telemetry_summary,
};
pub use support_bundle::{
    SupportAdapterSummary, SupportAppSettingsSummary, SupportBundleResponse, SupportEnvironment,
    SupportGameDetectionSummary, SupportGameSummary, SupportInputBridgeSummary, SupportPaths,
    SupportPrivacy, SupportSafetySummary, SupportSteamInputLayoutSummary, SupportSteamInputSummary,
    SupportTelemetrySignalSummary, SupportTelemetrySummary,
};
#[cfg(test)]
pub(crate) use update_check::{
    compare_release_versions, update_check_from_release, GithubReleaseResponse, VersionOrdering,
};
pub(crate) use update_check::{fetch_latest_release_update_check, unavailable_update_check};

const GLOBAL_PROFILE_ID: &str = "global";
const DEFAULT_PROFILE_ID: &str = GLOBAL_PROFILE_ID;
const IMMERSIVE_PROFILE_ID: &str = FORZA_HORIZON_IMMERSIVE_PROFILE_ID;

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn current_timestamp_millis() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

const HARDWARE_OUTPUT_INTERVAL: Duration = Duration::from_millis(33);
const INPUT_BRIDGE_PROCESS_INTERVAL: Duration = Duration::from_millis(8);
const INPUT_BRIDGE_CONFIG_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const INPUT_BRIDGE_STALE_AFTER: Duration = Duration::from_millis(250);
const CONTROLLER_INPUT_UI_CACHE_TTL: Duration = Duration::from_millis(75);
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
const TELEMETRY_WS_INVALIDATION_INTERVAL: Duration = Duration::from_millis(500);
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
const FORZA_REV_LIMITER_PULSE_AMPLITUDE: f64 = 18.0 / 63.0;
const FORZA_REV_LIMITER_FREQUENCY_HZ: f64 = 42.0;
const FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT: f64 = 0.60;
const FORZA_REV_LIMITER_WALL_ZONES: f64 = 4.0;
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
        ("rev_limiter_buzz", true, 85, "r2"),
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
        ("rev_limiter_buzz", true, 95, "r2"),
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
        ("rev_limiter_buzz", true, 90, "r2"),
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
    #[cfg(test)]
    input_overrides: Arc<Mutex<BTreeMap<String, ControllerInputState>>>,
    output_runtime: Arc<Mutex<HardwareOutputRuntime>>,
    discovery_cache: Arc<DiscoveryCache>,
    realtime_runtime: Arc<Mutex<RealtimeRuntime>>,
    effect_runtime: Arc<Mutex<EffectRuntimeCache>>,
    input_runtime: Arc<Mutex<InputRuntimeCache>>,
    input_bridge: InputBridgeService,
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

#[derive(Debug, Default)]
struct InputRuntimeCache {
    latest: BTreeMap<String, LatestControllerInput>,
    read_locks: BTreeMap<String, Arc<AsyncMutex<()>>>,
    next_sequence: u64,
}

#[derive(Clone, Debug)]
struct LatestControllerInput {
    state: ControllerInputState,
    sampled_at: Instant,
    sampled_at_ms: u64,
    sequence: u64,
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

#[derive(Debug, Clone, Default)]
struct ForzaEffectRuntime {
    prev_shift_gear: Option<u8>,
    latched_shift_event: Option<&'static str>,
    latched_shift_until: Option<Instant>,
    prev_suspension_impact: f64,
    latched_suspension_impact: f64,
    latched_suspension_impact_until: Option<Instant>,
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
struct DeviceBackendSummary {
    status: String,
    detail: String,
}

impl DeviceBackendSummary {
    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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

fn input_bridge_service_for_device_backend(
    device_backend: &DeviceBackendSummary,
) -> InputBridgeService {
    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
    if device_backend.status == "mock" {
        return InputBridgeService::mock();
    }

    let _ = device_backend;
    InputBridgeService::production()
}

impl AgentState {
    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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

    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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

    #[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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
            #[cfg(test)]
            input_overrides: Arc::new(Mutex::new(BTreeMap::new())),
            output_runtime: Arc::new(Mutex::new(HardwareOutputRuntime::default())),
            discovery_cache: Arc::new(DiscoveryCache::default()),
            realtime_runtime: Arc::new(Mutex::new(RealtimeRuntime::default())),
            effect_runtime: Arc::new(Mutex::new(EffectRuntimeCache::default())),
            input_runtime: Arc::new(Mutex::new(InputRuntimeCache::default())),
            input_bridge: input_bridge_service_for_device_backend(&device_backend),
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
            if matches!(
                &event,
                ControllerDiscoveryEvent::Detached(_)
                    | ControllerDiscoveryEvent::PermissionDenied(_)
                    | ControllerDiscoveryEvent::Faulted { .. }
            ) {
                self.clear_cached_input_for_event(&event);
            }
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

    fn clear_cached_input_for_event(&self, event: &ControllerDiscoveryEvent) {
        let clear_id = match event {
            ControllerDiscoveryEvent::Detached(id) => Some(id.0.as_str()),
            ControllerDiscoveryEvent::PermissionDenied(problem) => {
                problem.id.as_ref().map(|id| id.0.as_str())
            }
            ControllerDiscoveryEvent::Faulted { id, .. } => id.as_ref().map(|id| id.0.as_str()),
            ControllerDiscoveryEvent::Attached(_) | ControllerDiscoveryEvent::StatusChanged(_) => {
                None
            }
        };
        if let Some(id) = clear_id {
            let mut runtime = self
                .input_runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.latest.remove(id);
            runtime.read_locks.remove(id);
        }
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
        Ok(self
            .read_cached_or_live_input_state_for_controller(
                controller_id,
                ControllerInputReadOptions::default(),
                Duration::ZERO,
            )
            .await?
            .map(|sample| sample.state))
    }

    async fn read_cached_or_live_input_state_for_controller(
        &self,
        controller_id: &str,
        options: ControllerInputReadOptions,
        cache_ttl: Duration,
    ) -> Result<Option<LatestControllerInput>, String> {
        if let Some(sample) = self.cached_input_state(controller_id, cache_ttl) {
            return Ok(Some(sample));
        }
        let read_lock = self.input_read_lock(controller_id);
        let _guard = read_lock.lock().await;
        if let Some(sample) = self.cached_input_state(controller_id, cache_ttl) {
            return Ok(Some(sample));
        }
        self.read_live_input_state_for_controller_with_options(controller_id, options)
            .await
    }

    async fn read_live_input_state_for_controller_with_options(
        &self,
        controller_id: &str,
        options: ControllerInputReadOptions,
    ) -> Result<Option<LatestControllerInput>, String> {
        #[cfg(test)]
        {
            let input = self
                .input_overrides
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get(controller_id)
                .cloned();
            if let Some(input) = input {
                return Ok(Some(self.record_cached_input(controller_id, input)));
            }
        }

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

        let input = tokio::task::spawn_blocking(move || {
            manager.read_input_state_with_options(&target, options)
        })
        .await
        .map_err(|error| format!("HID input task failed: {error}"))?
        .map_err(|error| error.to_string())?;
        Ok(input.map(|input| self.record_cached_input(controller_id, input)))
    }

    fn cached_input_state(
        &self,
        controller_id: &str,
        max_age: Duration,
    ) -> Option<LatestControllerInput> {
        if max_age.is_zero() {
            return None;
        }
        self.input_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest
            .get(controller_id)
            .filter(|sample| sample.sampled_at.elapsed() <= max_age)
            .cloned()
    }

    fn input_read_lock(&self, controller_id: &str) -> Arc<AsyncMutex<()>> {
        let mut runtime = self
            .input_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime
            .read_locks
            .entry(controller_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    fn record_cached_input(
        &self,
        controller_id: &str,
        state: ControllerInputState,
    ) -> LatestControllerInput {
        let sampled_at = Instant::now();
        let sampled_at_ms = current_timestamp_millis();
        let mut runtime = self
            .input_runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.next_sequence = runtime.next_sequence.saturating_add(1).max(1);
        let sample = LatestControllerInput {
            state,
            sampled_at,
            sampled_at_ms,
            sequence: runtime.next_sequence,
        };
        runtime
            .latest
            .insert(controller_id.to_string(), sample.clone());
        sample
    }

    #[cfg(test)]
    fn with_input_override(
        self,
        controller_id: impl Into<String>,
        input: ControllerInputState,
    ) -> Self {
        self.input_overrides
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(controller_id.into(), input);
        self
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
        if detection.active_game_id.is_none() {
            let inner = self.inner.read().await;
            if let Some(telemetry_detection) = telemetry_game_detection(&inner, &catalog) {
                detection = enrich_game_detection(telemetry_detection, &catalog);
                append_user_games_to_detection(
                    &mut detection,
                    &user_games,
                    steam_root.as_deref(),
                    &steam_stats,
                );
            }
        }
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
        let bridge = self.input_bridge.status_response();
        checks.push(HealthCheck {
            name: "input-bridge".to_string(),
            status: if bridge.available {
                "pending".to_string()
            } else {
                "warning".to_string()
            },
            detail: format!(
                "{} provider {}: {}",
                bridge.provider, bridge.backend_id, bridge.message
            ),
        });
        checks.push(HealthCheck {
            name: "local-apps".to_string(),
            status: if inner
                .user_games
                .values()
                .any(|game| game.game_id.starts_with("local-"))
            {
                "ok".to_string()
            } else {
                "pending".to_string()
            },
            detail: format!(
                "{} local app profile(s) registered",
                inner
                    .user_games
                    .values()
                    .filter(|game| game.game_id.starts_with("local-"))
                    .count()
            ),
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
            adapters: materialized_adapters(
                &inner.adapters,
                &inner.adapter_runtimes,
                Some(&game_detection),
            ),
            modules: module_summaries(),
            steam_input,
            input_bridge: self.input_bridge.status_response(),
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
            diagnostics: sanitize_diagnostics_response(diagnostics),
            partial_errors: Vec::new(),
        }
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
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
            input_bridge: InputBridgeConfig::default(),
            profile_assignments: default_profile_assignments(edge),
        }
    }

    fn from_update(
        controller_id: impl Into<String>,
        model: impl Into<String>,
        request: UpdateControllerConfigRequest,
        existing_input_bridge: Option<InputBridgeConfig>,
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
            input_bridge: request
                .input_bridge
                .or(existing_input_bridge)
                .unwrap_or_default()
                .normalized(),
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
            ControllerInputMode::DsccInputBridge => ControllerInputMode::DsccInputBridge,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, self.model == "DualSense Edge");
        self.input_bridge = self.input_bridge.normalized();
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
            input_bridge: config.input_bridge.clone(),
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
            ControllerInputMode::DsccInputBridge => ControllerInputMode::DsccInputBridge,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, model == "DualSense Edge");
        self.input_bridge = self.input_bridge.normalized();
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
        config.input_bridge = profile_config.input_bridge;
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
    let rev_amplitude = scaled_unit(
        FORZA_REV_LIMITER_PULSE_AMPLITUDE,
        rev.scalar() * trigger_scalar,
    );
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

    push_rev_limiter_rules(
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
        ValueSource::constant(FORZA_REV_LIMITER_FREQUENCY_HZ),
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

fn push_rev_limiter_rules(
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
        let target_label = trigger_target_label(target);
        rules.push(EffectRule {
            id: format!("{id}-{target_label}-wall-form"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority,
            condition: RuleCondition::All {
                conditions: vec![
                    condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::GreaterOrEqual,
                        FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT,
                    ),
                ],
            },
            effect: EffectTemplate::PulseAb {
                strength: amplitude.clone(),
                frequency_hz: frequency_hz.clone(),
                wall_zones: ValueSource::constant(FORZA_REV_LIMITER_WALL_ZONES),
            },
        });
        rules.push(EffectRule {
            id: format!("{id}-{target_label}-pulse"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority,
            condition: RuleCondition::All {
                conditions: vec![
                    condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::LessThan,
                        FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT,
                    ),
                ],
            },
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
        let existing_input_bridge = inner
            .controller_configs
            .get(&id)
            .map(|config| config.input_bridge.clone());
        let config =
            ControllerConfig::from_update(id.clone(), detail.model, request, existing_input_bridge);
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

    if state.input_bridge.is_active(&id) {
        return match state.cached_input_state(&id, INPUT_BRIDGE_STALE_AFTER) {
            Some(sample) => Ok(controller_input_available(id, sample)),
            None => Ok(controller_input_unavailable(
                id,
                "hid",
                "Waiting for a fresh DSCC Input Bridge input sample".to_string(),
            )),
        };
    }

    match state
        .read_cached_or_live_input_state_for_controller(
            &id,
            ControllerInputReadOptions::bridge_poll(),
            CONTROLLER_INPUT_UI_CACHE_TTL,
        )
        .await
    {
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
    sample: LatestControllerInput,
) -> ControllerInputResponse {
    let age_ms = input_sample_age_ms(&sample);
    let input = sample.state;
    ControllerInputResponse {
        controller_id,
        available: true,
        source: "hid".to_string(),
        message: "Live DualSense input is available".to_string(),
        sampled_at_ms: Some(sample.sampled_at_ms),
        age_ms: Some(age_ms),
        axes: ControllerInputAxesResponse {
            left_stick: ControllerInputStickResponse {
                x: input.left_stick.x,
                y: input.left_stick.y,
                magnitude: input.left_stick.magnitude,
            },
            right_stick: ControllerInputStickResponse {
                x: input.right_stick.x,
                y: input.right_stick.y,
                magnitude: input.right_stick.magnitude,
            },
        },
        triggers: ControllerInputTriggersResponse {
            l2: input.l2,
            r2: input.r2,
        },
        buttons: input
            .buttons
            .into_iter()
            .map(|button| ControllerInputButtonResponse {
                id: button.id.to_string(),
                label: button.label.to_string(),
                pressed: button.pressed,
                value: button.value,
            })
            .collect(),
    }
}

fn input_sample_age_ms(sample: &LatestControllerInput) -> u64 {
    sample
        .sampled_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
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
        sampled_at_ms: None,
        age_ms: None,
        axes: ControllerInputAxesResponse {
            left_stick: ControllerInputStickResponse::default(),
            right_stick: ControllerInputStickResponse::default(),
        },
        triggers: ControllerInputTriggersResponse::default(),
        buttons: Vec::new(),
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
        let existing_input_bridge = inner
            .profile_configs
            .get(&id)
            .map(|config| config.input_bridge.clone())
            .unwrap_or_default();
        let profile_config = ProfileConfig {
            input_mode: request.input_mode,
            trigger: request.trigger,
            lightbar: request.lightbar,
            forza: request.forza,
            sticks: request.sticks,
            buttons: request.buttons,
            input_bridge: request.input_bridge.unwrap_or(existing_input_bridge),
        }
        .normalized_for_model(&model_hint);

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
    Json(materialized_adapters(
        &inner.adapters,
        &inner.adapter_runtimes,
        Some(&game_detection),
    ))
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

async fn apply_steam_input_paddle_preset(
    State(state): State<AgentState>,
    Json(request): Json<SteamInputPaddlePresetRequest>,
) -> Result<Json<SteamInputPaddlePresetResponse>, (StatusCode, String)> {
    let dry_run = request.dry_run;
    let mut response =
        tokio::task::spawn_blocking(move || write_steam_input_paddle_preset(request))
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Steam Input paddle preset task failed: {error}"),
                )
            })?
            .map_err(|error| (error.status, error.message))?;

    if !dry_run {
        let steam_input = state.cached_steam_input_status_or_refresh().await;
        if !steam_input.running {
            response.warnings.push(
                "Steam is not currently running; restart Steam or reopen the game if the layout is not picked up immediately."
                    .to_string(),
            );
        }
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

async fn get_input_bridge_status(
    State(state): State<AgentState>,
) -> Json<InputBridgeStatusResponse> {
    Json(state.input_bridge.status_response())
}

async fn get_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Json<InputBridgeSessionSummary> {
    Json(state.input_bridge.session_summary(&controller_id))
}

async fn start_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<InputBridgeSessionSummary>, (StatusCode, Json<serde_json::Value>)> {
    {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&controller_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "controller not found"})),
            )
        })?;
        if !detail.connected {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "controller is not connected"})),
            ));
        }
        let config = inner
            .controller_configs
            .get(&controller_id)
            .cloned()
            .unwrap_or_else(|| ControllerConfig::default_for(&controller_id, detail.model));
        if config.input_mode != ControllerInputMode::DsccInputBridge || !config.input_bridge.enabled
        {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "DSCC Input Bridge must be explicitly enabled for this controller"
                })),
            ));
        }
    }
    let detection = state
        .cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
        .await;
    if !detection_allows_input_bridge(&detection) {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "DSCC Input Bridge can only start while a local app is active"
            })),
        ));
    }
    if !local_app_execution_verified_for_input_bridge(&state, &detection).await {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "DSCC Input Bridge can only start while the registered local app executable is running"
            })),
        ));
    }
    let existing = state.input_bridge.session_summary(&controller_id);
    if existing.state == InputBridgeSessionState::Active {
        return Ok(Json(existing));
    }
    let summary = state
        .input_bridge
        .start_session(
            &controller_id,
            VirtualOutputKind::Xbox360,
            current_timestamp_millis(),
        )
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": error})),
            )
        })?;
    let loop_state = state.clone();
    let loop_controller_id = controller_id.clone();
    tokio::spawn(async move {
        run_input_bridge_session_loop(loop_state, loop_controller_id).await;
    });
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("input-bridge-started".to_string()),
    });
    Ok(Json(summary))
}

async fn stop_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Json<InputBridgeSessionSummary> {
    let summary = state
        .input_bridge
        .stop_session(&controller_id, current_timestamp_millis());
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("input-bridge-stopped".to_string()),
    });
    Json(summary)
}

async fn run_input_bridge_session_loop(state: AgentState, controller_id: String) {
    let mut last_input_at = Instant::now();
    let mut last_submitted_sequence: Option<u64> = None;
    let mut last_game_check_at = Instant::now();
    let mut last_config_check_at: Option<Instant> = None;
    let mut active_config: Option<InputBridgeConfig> = None;
    let mut process_interval = tokio::time::interval(INPUT_BRIDGE_PROCESS_INTERVAL);
    process_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        process_interval.tick().await;
        if !state.input_bridge.is_active(&controller_id) {
            break;
        }

        if last_config_check_at
            .map(|checked_at| checked_at.elapsed() >= INPUT_BRIDGE_CONFIG_REFRESH_INTERVAL)
            .unwrap_or(true)
        {
            let inner = state.inner.read().await;
            let Some(detail) = inner.controllers.detail(&controller_id) else {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-controller-disconnected");
                break;
            };
            if !detail.connected {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-controller-disconnected");
                break;
            }
            let config = inner
                .controller_configs
                .get(&controller_id)
                .cloned()
                .unwrap_or_else(|| ControllerConfig::default_for(&controller_id, detail.model));
            if config.input_mode != ControllerInputMode::DsccInputBridge
                || !config.input_bridge.enabled
            {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-config-disabled");
                break;
            }
            active_config = Some(config.input_bridge);
            last_config_check_at = Some(Instant::now());
        }
        let Some(config) = active_config.as_ref() else {
            continue;
        };

        if last_game_check_at.elapsed() >= HARDWARE_GAME_DETECTION_INTERVAL {
            let detection = state
                .cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
                .await;
            if !detection_allows_input_bridge(&detection) {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-local-app-inactive");
                break;
            }
            if !local_app_execution_verified_for_input_bridge(&state, &detection).await {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-local-app-unverified");
                break;
            }
            last_game_check_at = Instant::now();
        }

        match state
            .read_cached_or_live_input_state_for_controller(
                &controller_id,
                ControllerInputReadOptions::bridge_poll(),
                INPUT_BRIDGE_PROCESS_INTERVAL,
            )
            .await
        {
            Ok(Some(sample)) => {
                last_input_at = sample.sampled_at;
                if last_submitted_sequence == Some(sample.sequence) {
                    continue;
                }
                last_submitted_sequence = Some(sample.sequence);
                if state
                    .input_bridge
                    .submit_controller_input(
                        &controller_id,
                        &sample.state,
                        config,
                        current_timestamp_millis(),
                    )
                    .is_err()
                {
                    state.input_bridge.neutralize_session(
                        &controller_id,
                        InputBridgeSessionState::Faulted,
                        "DSCC Input Bridge backend fault; virtual output was neutralized.",
                        current_timestamp_millis(),
                    );
                    tracing::warn!(controller_id = %controller_id, "DSCC Input Bridge backend fault");
                    send_input_bridge_invalidation(&state, "input-bridge-backend-fault");
                    break;
                }
            }
            Ok(None) => {
                if last_input_at.elapsed() >= INPUT_BRIDGE_STALE_AFTER {
                    state.input_bridge.neutralize_session(
                        &controller_id,
                        InputBridgeSessionState::Stale,
                        "DSCC Input Bridge neutralized output after stale controller input.",
                        current_timestamp_millis(),
                    );
                    send_input_bridge_invalidation(&state, "input-bridge-input-stale");
                    last_input_at = Instant::now();
                }
            }
            Err(_) => {
                state.input_bridge.neutralize_session(
                    &controller_id,
                    InputBridgeSessionState::Faulted,
                    "DSCC Input Bridge input read failed; virtual output was neutralized.",
                    current_timestamp_millis(),
                );
                tracing::warn!(controller_id = %controller_id, "DSCC Input Bridge input read failed");
                send_input_bridge_invalidation(&state, "input-bridge-input-fault");
                break;
            }
        }
    }
}

fn send_input_bridge_invalidation(state: &AgentState, message: &str) {
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some(message.to_string()),
    });
}

async fn write_input_bridge_binding(
    State(state): State<AgentState>,
    Json(request): Json<InputBridgeBindingWriteRequest>,
) -> Result<Json<InputBridgeBindingWriteResponse>, (StatusCode, Json<serde_json::Value>)> {
    let input_id = request.input_id.trim();
    let target = request.target.trim();
    if input_id.is_empty() || target.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "inputId and target are required"})),
        ));
    }
    let source = bridge_source_from_input_id(input_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unsupported DSCC Input Bridge source"})),
        )
    })?;
    let target = bridge_target_from_raw(target).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unsupported DSCC Input Bridge target"})),
        )
    })?;
    let binding = InputBridgeBindingConfig { source, target };
    let mut warnings = Vec::new();
    if !request.dry_run {
        let to_save = {
            let mut inner = state.inner.write().await;
            let profile_id = request.profile_id.as_deref().map(str::trim).unwrap_or("");
            let wrote_profile = if !profile_id.is_empty()
                && inner
                    .profiles
                    .iter()
                    .any(|profile| profile.id == profile_id && !profile.built_in)
            {
                let config = inner
                    .profile_configs
                    .entry(profile_id.to_string())
                    .or_insert_with(ProfileConfig::default);
                upsert_input_bridge_binding(&mut config.input_bridge, binding.clone());
                true
            } else {
                false
            };

            if !wrote_profile {
                let controller_id = request
                    .controller_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("");
                if controller_id.is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "controllerId is required when no writable profileId is provided"
                        })),
                    ));
                }
                let model = inner
                    .controllers
                    .detail(controller_id)
                    .map(|detail| detail.model)
                    .ok_or_else(|| {
                        (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"error": "controller not found"})),
                        )
                    })?;
                let config = inner
                    .controller_configs
                    .entry(controller_id.to_string())
                    .or_insert_with(|| ControllerConfig::default_for(controller_id, model));
                upsert_input_bridge_binding(&mut config.input_bridge, binding.clone());
                warnings.push(
                    "Wrote bridge binding to controller config because the profile is built-in or absent."
                        .to_string(),
                );
            }
            inner.effect_revision = inner.effect_revision.saturating_add(1);
            build_persist_snapshot(&inner)
        };
        persist_snapshot(&state, to_save).await;
        let _ = state.event_tx.send(RealtimeMessage {
            kind: "snapshot_invalidated".to_string(),
            controller: None,
            message: Some("input-bridge-binding-updated".to_string()),
        });
    }
    Ok(Json(InputBridgeBindingWriteResponse {
        accepted: true,
        message: if request.dry_run {
            format!("Validated DSCC Input Bridge binding for {input_id}.")
        } else {
            format!("Saved DSCC Input Bridge binding for {input_id}.")
        },
        dry_run: request.dry_run,
        warnings,
    }))
}

fn upsert_input_bridge_binding(config: &mut InputBridgeConfig, binding: InputBridgeBindingConfig) {
    config
        .bindings
        .retain(|existing| existing.source != binding.source);
    config.bindings.push(binding);
    *config = config.clone().normalized();
}

fn bridge_source_from_input_id(input_id: &str) -> Option<InputBridgeSource> {
    let normalized = input_id.trim().to_ascii_lowercase();
    let source = match normalized.as_str() {
        "button_a" => InputBridgeSource::Button("cross".to_string()),
        "button_b" => InputBridgeSource::Button("circle".to_string()),
        "button_x" => InputBridgeSource::Button("square".to_string()),
        "button_y" => InputBridgeSource::Button("triangle".to_string()),
        "dpad_north" | "dpad_up" => InputBridgeSource::Button("dpad_up".to_string()),
        "dpad_south" | "dpad_down" => InputBridgeSource::Button("dpad_down".to_string()),
        "dpad_west" | "dpad_left" => InputBridgeSource::Button("dpad_left".to_string()),
        "dpad_east" | "dpad_right" => InputBridgeSource::Button("dpad_right".to_string()),
        "left_bumper" | "button_should_left" => InputBridgeSource::Button("l1".to_string()),
        "right_bumper" | "button_should_right" => InputBridgeSource::Button("r1".to_string()),
        "click:left_trigger" | "left_trigger:click" => InputBridgeSource::Axis("l2".to_string()),
        "click:right_trigger" | "right_trigger:click" => InputBridgeSource::Axis("r2".to_string()),
        "button_menu" => InputBridgeSource::Button("create".to_string()),
        "button_escape" => InputBridgeSource::Button("options".to_string()),
        "click:left_joystick" | "left_joystick:click" | "click:joystick" | "joystick:click" => {
            InputBridgeSource::Button("l3".to_string())
        }
        "click:right_joystick" | "right_joystick:click" => {
            InputBridgeSource::Button("r3".to_string())
        }
        "click:left_trackpad" | "left_trackpad:click" => {
            InputBridgeSource::Button("touchpad".to_string())
        }
        "click:right_trackpad" | "right_trackpad:click" => {
            InputBridgeSource::Button("touchpad".to_string())
        }
        "button_back_left" => InputBridgeSource::Button("edge_back_left".to_string()),
        "button_back_right" => InputBridgeSource::Button("edge_back_right".to_string()),
        "button_back_left_upper" => InputBridgeSource::Button("edge_fn_left".to_string()),
        "button_back_right_upper" => InputBridgeSource::Button("edge_fn_right".to_string()),
        _ if normalized.contains("center_trackpad") || normalized.contains("gyro") => return None,
        _ => return None,
    };
    Some(source)
}

fn bridge_target_from_raw(raw: &str) -> Option<InputBridgeTarget> {
    let command = raw.split(',').next()?.trim();
    let mut parts = command.split_whitespace();
    let kind = parts.next()?.to_ascii_lowercase();
    let param = parts.next().unwrap_or("").to_ascii_lowercase();
    if kind != "xinput_button" {
        return None;
    }
    match param.as_str() {
        "a" => Some(InputBridgeTarget::Button(VirtualButton::A)),
        "b" => Some(InputBridgeTarget::Button(VirtualButton::B)),
        "x" => Some(InputBridgeTarget::Button(VirtualButton::X)),
        "y" => Some(InputBridgeTarget::Button(VirtualButton::Y)),
        "dpad_up" => Some(InputBridgeTarget::Button(VirtualButton::DpadUp)),
        "dpad_down" => Some(InputBridgeTarget::Button(VirtualButton::DpadDown)),
        "dpad_left" => Some(InputBridgeTarget::Button(VirtualButton::DpadLeft)),
        "dpad_right" => Some(InputBridgeTarget::Button(VirtualButton::DpadRight)),
        "shoulder_left" => Some(InputBridgeTarget::Button(VirtualButton::LeftShoulder)),
        "shoulder_right" => Some(InputBridgeTarget::Button(VirtualButton::RightShoulder)),
        "trigger_left" => Some(InputBridgeTarget::Axis(VirtualAxis::LeftTrigger)),
        "trigger_right" => Some(InputBridgeTarget::Axis(VirtualAxis::RightTrigger)),
        "joystick_left" => Some(InputBridgeTarget::Button(VirtualButton::LeftThumb)),
        "joystick_right" => Some(InputBridgeTarget::Button(VirtualButton::RightThumb)),
        "select" | "back" => Some(InputBridgeTarget::Button(VirtualButton::Back)),
        "start" => Some(InputBridgeTarget::Button(VirtualButton::Start)),
        "guide" => Some(InputBridgeTarget::Button(VirtualButton::Guide)),
        _ => None,
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

#[cfg(test)]
mod tests;
