use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde::de::DeserializeOwned;
use std::sync::Mutex as StdMutex;
use tower::ServiceExt;

static TEST_ENV_LOCK: StdMutex<()> = StdMutex::new(());

pub(super) struct TestEnv {
    _lock: std::sync::MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl TestEnv {
    pub(super) fn new(names: &[&'static str]) -> Self {
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

pub(super) fn temp_test_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}

pub(super) async fn seed_active_local_game(state: &AgentState, game_id: &str) {
    let mut detection = no_game_detection("process_scan");
    detection.active_game_id = Some(game_id.to_string());
    detection.active_game_name = Some("Night Drive Lab".to_string());
    detection.source = "process_scan".to_string();
    detection.confidence = 100;
    detection.process_name = Some("NightDriveLab.exe".to_string());
    detection.selected_game = Some(SupportedGameSummary {
        game_id: game_id.to_string(),
        name: "Night Drive Lab".to_string(),
        source: "local_app".to_string(),
        input_provider: "dscc_input_bridge".to_string(),
        app_id: Some("local:test".to_string()),
        install_path: None,
        process_names: vec!["NightDriveLab.exe".to_string()],
        executable_name: Some("NightDriveLab.exe".to_string()),
        installed: true,
        running: true,
        support_level: "custom".to_string(),
        artwork: GameArtwork::default(),
        stats: SteamGameStats::default(),
    });
    let mut cache = state.discovery_cache.game_detection.lock().await;
    cache.store(detection, Instant::now());
}

pub(super) fn test_udp_adapter_runtime() -> AdapterRuntime {
    let adapter = built_in_udp_adapters()
        .iter()
        .find(|adapter| adapter.id == FORZA_DATA_OUT_ADAPTER_ID)
        .copied()
        .expect("Forza UDP adapter is registered");
    AdapterRuntime::for_udp_adapter(adapter)
}

pub(super) fn test_forza_effect_runtime() -> ForzaEffectRuntime {
    ForzaEffectRuntime::default()
}

pub(super) fn test_game_module_by_id(id: &str) -> &'static GameModule {
    built_in_game_modules()
        .iter()
        .find(|game| game.id == id)
        .expect("built-in game module exists")
}

pub(super) fn forza_horizon_controller_config() -> ControllerConfig {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    config.trigger = forza_horizon_trigger_preset();
    config.forza = forza_horizon_preset();
    config
}

pub(super) fn make_user_game(
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

pub(super) fn make_test_steam_root(prefix: &str) -> PathBuf {
    let root = temp_test_dir(prefix);
    let steamapps = root.join("steamapps");
    let common = steamapps.join("common");
    fs::create_dir_all(&common).expect("steam common");
    root
}

pub(super) fn install_test_steam_manifest(
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

pub(super) async fn get_json<T>(router: Router, uri: &str, expected_status: StatusCode) -> T
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

pub(super) fn attach_event(
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

pub(super) fn sample_controller_input() -> ControllerInputState {
    ControllerInputState {
        left_stick: dscc_device::ControllerInputStickState {
            x: 0.25,
            y: 0.5,
            magnitude: 0.559_016_994,
        },
        right_stick: dscc_device::ControllerInputStickState {
            x: -0.25,
            y: -0.75,
            magnitude: 0.790_569_415,
        },
        l2: 0.4,
        r2: 0.8,
        buttons: vec![
            dscc_device::ControllerInputButtonState {
                id: "cross",
                label: "Cross",
                pressed: true,
                value: 1.0,
            },
            dscc_device::ControllerInputButtonState {
                id: "r2",
                label: "R2",
                pressed: true,
                value: 0.8,
            },
        ],
    }
}

pub(super) fn write_i32(packet: &mut [u8], offset: usize, value: i32) {
    packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn write_f32(packet: &mut [u8], offset: usize, value: f32) {
    packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
