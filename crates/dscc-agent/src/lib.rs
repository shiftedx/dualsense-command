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
    encode_controller_output_frame, ControllerInputReadOptions, ControllerInputState,
    ControllerOutputManager, ControllerOutputTarget, ControllerOutputWrite, DeviceConfig,
    DeviceManager, DeviceTransport, DeviceTransportKind, EdgeButton, EdgeButtonMapping,
    EdgeOnboardProfile, EdgeOnboardSlotId, EdgeProfileIntensity, EdgeStickPreset, EdgeStickProfile,
    EdgeTriggerDeadzone, HidApiTransport, OutputMode, OutputReportKind, RawDeviceId,
};
use dscc_telemetry::{SignalName, SignalSnapshot, SignalUpdate, SignalValue};
use dscc_virtual_output::VirtualOutputKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::{broadcast, Mutex as AsyncMutex, RwLock},
};
use tower_http::services::ServeDir;
use tracing::info;

mod adapter_runtime;
mod agent_types;
mod api;
mod assetto_shared_memory;
mod bind_addr;
mod built_in_presets;
mod config_model;
mod controller_registry;
mod edge_profiles;
mod effects;
mod env_policy;
mod forza_glyphs;
mod game_detection;
mod game_detection_cache;
mod game_modules;
mod http_security;
mod input_bridge;
mod persistence;
mod profiles;
mod routes;
mod runtime;
mod runtime_constants;
mod runtime_paths;
mod steam_input;
mod support_bundle;
mod telemetry_runtime;
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
pub use agent_types::*;
pub(crate) use api::*;
#[cfg(target_os = "windows")]
pub(crate) use assetto_shared_memory::assetto_shared_memory_adapter_loop;
#[cfg(not(target_os = "windows"))]
pub(crate) use assetto_shared_memory::mark_assetto_shared_memory_unavailable;
#[cfg(test)]
pub(crate) use assetto_shared_memory::{
    parse_assetto_shared_memory_pages, AssettoSharedMemoryPages, ASSETTO_AC_LIVE,
    ASSETTO_GRAPHICS_MIN_LEN, ASSETTO_PHYSICS_MIN_LEN, ASSETTO_STATIC_MAX_RPM_OFFSET,
    ASSETTO_STATIC_MIN_LEN,
};
pub(crate) use bind_addr::{
    default_agent_bind_addr, desired_agent_bind_addr, lan_api_enabled, resolve_forza_bind_addr,
};
pub(crate) use built_in_presets::*;
pub(crate) use config_model::*;
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
pub(crate) use effects::*;
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
pub(crate) use game_detection_cache::*;
#[cfg(test)]
use game_modules::detect_running_game_from_processes;
use game_modules::{
    built_in_game_modules, detect_running_game_from_processes_with_user_games,
    game_executable_exists, game_module_summaries, no_game_detection, supported_game_summary,
    GameModule, ASSETTO_CORSA_RALLY_PROFILE_ID, ASSETTO_SHARED_MEMORY_ADAPTER_ID,
    FORZA_DATA_OUT_ADAPTER_ID, FORZA_HORIZON_PROFILE_ID,
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
pub(crate) use runtime::*;
pub(crate) use runtime_constants::*;
pub use runtime_paths::{app_paths, init_tracing};
pub(crate) use steam_input::{
    discover_steam_input_status_async, pending_steam_input_status, steam_input_discovery_pending,
    steam_root_candidates, write_steam_input_binding, write_steam_input_paddle_preset,
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
pub(crate) use telemetry_runtime::{udp_adapter_bind_addr, udp_telemetry_adapter_loop};
#[cfg(test)]
pub(crate) use update_check::{
    compare_release_versions, update_check_from_release, GithubReleaseResponse, VersionOrdering,
};
pub(crate) use update_check::{fetch_latest_release_update_check, unavailable_update_check};

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
    diagnostics: BTreeMap<String, ControllerOutputDiagnostics>,
}

#[derive(Debug, Clone, Default)]
struct ControllerOutputDiagnostics {
    written_reports: u64,
    suppressed_redundant_reports: u64,
    first_written_at: Option<Instant>,
    previous_written_at: Option<Instant>,
    last_written_at: Option<Instant>,
    last_suppressed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct LastHardwareOutputFrame {
    frame: ControllerOutputFrame,
    fingerprint: Option<StableOutputFingerprint>,
    written_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableOutputFingerprint {
    report_kind: OutputReportKind,
    len: usize,
    hash: u128,
}

fn stable_output_fingerprint(
    frame: &ControllerOutputFrame,
    transport: DeviceTransportKind,
) -> Result<StableOutputFingerprint, String> {
    let report = encode_controller_output_frame(frame, transport, 0).map_err(|error| {
        format!("could not encode controller output fingerprint for {transport:?}: {error}")
    })?;
    Ok(StableOutputFingerprint {
        report_kind: report.kind,
        len: report.bytes.len(),
        hash: stable_output_hash(&report.bytes),
    })
}

fn stable_output_hash(bytes: &[u8]) -> u128 {
    let mut low = 0xcbf2_9ce4_8422_2325_u64;
    let mut high = 0x8422_2325_cbf2_9ce4_u64;
    for byte in bytes {
        low ^= u64::from(*byte);
        low = low.wrapping_mul(0x0000_0100_0000_01b3);
        high ^= u64::from(*byte).rotate_left(1);
        high = high.wrapping_mul(0x0000_0100_0000_01d3);
    }
    (u128::from(high) << 64) | u128::from(low)
}

fn controller_power_diagnostics(
    diagnostics: Option<&ControllerOutputDiagnostics>,
    config: Option<&ControllerConfig>,
    now: Instant,
) -> ControllerPowerDiagnostics {
    let native_rumble_passthrough = config
        .is_none_or(|config| config.forza.body_rumble_mode == default_forza_body_rumble_mode());
    let adaptive_triggers_retained = config.is_none_or(|config| {
        !config.trigger.effect.eq_ignore_ascii_case("off") && config.trigger.intensity != "Off"
    });
    let Some(diagnostics) = diagnostics else {
        return ControllerPowerDiagnostics {
            keepalive_interval_ms: duration_millis_u64(HARDWARE_OUTPUT_KEEPALIVE_INTERVAL),
            native_rumble_passthrough,
            adaptive_triggers_retained,
            ..ControllerPowerDiagnostics::default()
        };
    };

    ControllerPowerDiagnostics {
        written_reports: diagnostics.written_reports,
        suppressed_redundant_reports: diagnostics.suppressed_redundant_reports,
        output_write_rate_hz: output_write_rate_hz(diagnostics),
        output_cadence_ms: output_cadence_ms(diagnostics),
        keepalive_interval_ms: duration_millis_u64(HARDWARE_OUTPUT_KEEPALIVE_INTERVAL),
        last_write_age_ms: diagnostics
            .last_written_at
            .map(|written_at| duration_millis_u64(now.saturating_duration_since(written_at))),
        last_suppressed_age_ms: diagnostics
            .last_suppressed_at
            .map(|suppressed_at| duration_millis_u64(now.saturating_duration_since(suppressed_at))),
        native_rumble_passthrough,
        adaptive_triggers_retained,
    }
}

fn output_cadence_ms(diagnostics: &ControllerOutputDiagnostics) -> Option<u64> {
    Some(duration_millis_u64(
        diagnostics
            .last_written_at?
            .saturating_duration_since(diagnostics.previous_written_at?),
    ))
}

fn output_write_rate_hz(diagnostics: &ControllerOutputDiagnostics) -> Option<u16> {
    let first = diagnostics.first_written_at?;
    let last = diagnostics.last_written_at?;
    if diagnostics.written_reports < 2 || last <= first {
        return None;
    }

    let elapsed_ms = last.duration_since(first).as_millis().max(1);
    let rate = u128::from(diagnostics.written_reports.saturating_sub(1)) * 1_000 / elapsed_ms;
    Some(rate.min(u128::from(u16::MAX)) as u16)
}

fn duration_millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[derive(Debug, Default)]
struct RealtimeRuntime {
    last_telemetry_event_at: Option<Instant>,
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
        transport: DeviceTransportKind,
        now: Instant,
    ) -> bool {
        let fingerprint = stable_output_fingerprint(frame, transport).ok();
        let mut runtime = self.lock_output_runtime();
        match runtime.last_output_frames.get(controller_id) {
            Some(last) => {
                let same_output = match (last.fingerprint, fingerprint) {
                    (Some(last_fingerprint), Some(next_fingerprint)) => {
                        last_fingerprint == next_fingerprint
                    }
                    _ => last.frame == *frame,
                };
                let keepalive_due =
                    now.duration_since(last.written_at) >= HARDWARE_OUTPUT_KEEPALIVE_INTERVAL;
                if same_output && !keepalive_due {
                    let diagnostics = runtime
                        .diagnostics
                        .entry(controller_id.to_string())
                        .or_default();
                    diagnostics.suppressed_redundant_reports =
                        diagnostics.suppressed_redundant_reports.saturating_add(1);
                    diagnostics.last_suppressed_at = Some(now);
                    return false;
                }
                true
            }
            None => true,
        }
    }

    fn record_output_frame_write(
        &self,
        controller_id: &str,
        frame: &ControllerOutputFrame,
        transport: DeviceTransportKind,
        written_at: Instant,
    ) {
        let mut runtime = self.lock_output_runtime();
        let diagnostics = runtime
            .diagnostics
            .entry(controller_id.to_string())
            .or_default();
        diagnostics.written_reports = diagnostics.written_reports.saturating_add(1);
        if diagnostics.first_written_at.is_none() {
            diagnostics.first_written_at = Some(written_at);
        }
        diagnostics.previous_written_at = diagnostics.last_written_at;
        diagnostics.last_written_at = Some(written_at);
        runtime.last_output_frames.insert(
            controller_id.to_string(),
            LastHardwareOutputFrame {
                frame: frame.clone(),
                fingerprint: stable_output_fingerprint(frame, transport).ok(),
                written_at,
            },
        );
    }

    pub(crate) fn output_diagnostics_snapshot(
        &self,
    ) -> BTreeMap<String, ControllerOutputDiagnostics> {
        self.lock_output_runtime().diagnostics.clone()
    }

    pub(crate) fn apply_power_diagnostics_to_controllers(
        &self,
        mut controllers: Vec<ControllerSummary>,
        diagnostics: &BTreeMap<String, ControllerOutputDiagnostics>,
        configs: &BTreeMap<String, ControllerConfig>,
    ) -> Vec<ControllerSummary> {
        let now = Instant::now();
        for controller in &mut controllers {
            controller.power_diagnostics = controller_power_diagnostics(
                diagnostics.get(&controller.id),
                configs.get(&controller.id),
                now,
            );
        }
        controllers
    }

    pub(crate) fn apply_power_diagnostics_to_controller_detail(
        &self,
        mut detail: ControllerDetail,
        diagnostics: &BTreeMap<String, ControllerOutputDiagnostics>,
        configs: &BTreeMap<String, ControllerConfig>,
    ) -> ControllerDetail {
        detail.power_diagnostics = controller_power_diagnostics(
            diagnostics.get(&detail.id),
            configs.get(&detail.id),
            Instant::now(),
        );
        detail
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
        let transport = target.transport;
        let frame_for_write = frame.clone();
        let write =
            tokio::task::spawn_blocking(move || manager.write_frame(&target, &frame_for_write))
                .await
                .map_err(|error| format!("HID output task failed: {error}"))?
                .map_err(|error| error.to_string())?;
        self.record_output_frame_write(controller_id, frame, transport, Instant::now());
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
        let transport = {
            let inner = self.inner.read().await;
            controller_output_target_or_reason(&inner, &controller_id)?.transport
        };
        if !self.output_frame_write_due(&controller_id, &frame, transport, Instant::now()) {
            return Ok(None);
        }
        self.write_output_frame_to_controller(&controller_id, &frame)
            .await
            .map(Some)
    }

    fn current_effect_response_cached(
        &self,
        inner: &AgentStateInner,
        game_detection: Option<&GameDetectionResponse>,
        hardware_output_enabled: bool,
        purpose: EffectEnginePurpose,
    ) -> CurrentEffectResponse {
        let mut runtime = match self.effect_runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        RuntimeLiveEffectMaterializer::new(inner, game_detection, purpose, &mut runtime)
            .current_response(hardware_output_enabled)
    }

    fn output_frame_for_current_resolution_cached(
        &self,
        inner: &AgentStateInner,
        game_detection: Option<&GameDetectionResponse>,
        purpose: EffectEnginePurpose,
    ) -> Option<(String, ControllerOutputFrame)> {
        let mut runtime = match self.effect_runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        RuntimeLiveEffectMaterializer::new(inner, game_detection, purpose, &mut runtime)
            .output_frame_for_current_resolution()
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
                let clutch = update_number(&updates, "input.clutch");
                let telemetry_on = update_text(&updates, "game.state") == Some("driving");
                let effect_toggles = racing_effect_toggles(&inner);
                let shift_tuning = racing_shift_tuning(&inner);
                let suspension_travel = update_number(&updates, "suspension.travel.max");
                let acceleration_magnitude =
                    update_number(&updates, "vehicle.acceleration.magnitude");
                let speed_kmh = update_number(&updates, "vehicle.speed_kmh");
                let now = Instant::now();
                if let Some(shift_event) = inner.forza_effect_runtime.detect_shift_event(
                    current_gear,
                    clutch,
                    telemetry_on,
                    effect_toggles.shift_thump,
                    &shift_tuning,
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
                updates.push(sequenced_signal_update(
                    "drivetrain.shift_pulse",
                    inner.forza_effect_runtime.latched_shift_pulse(now),
                    sequence,
                ));
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
        let output_diagnostics = self.output_diagnostics_snapshot();
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
            controllers: self.apply_power_diagnostics_to_controllers(
                apply_controller_names(inner.controllers.summaries(), &inner.controller_names),
                &output_diagnostics,
                &inner.controller_configs,
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

#[cfg(test)]
mod tests;
