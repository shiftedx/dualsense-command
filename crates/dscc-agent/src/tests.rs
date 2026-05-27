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

async fn seed_active_local_game(state: &AgentState, game_id: &str) {
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
                    "https://github.com/shiftedx/dualsense-command/releases/tag/v9.9.9".to_string(),
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

#[tokio::test]
async fn input_bridge_status_route_reports_mock_backend() {
    let router = app(AgentState::mock());

    let status: InputBridgeStatusResponse =
        get_json(router, "/api/input-bridge", StatusCode::OK).await;

    assert!(status.available);
    assert_eq!(status.provider, "mock");
    assert_eq!(status.supported_kinds, vec!["xbox360".to_string()]);
}

#[test]
fn hid_agent_state_uses_hidmaestro_bridge_provider_not_mock() {
    let service = input_bridge_service_for_device_backend(&DeviceBackendSummary {
        status: "hidapi".to_string(),
        detail: "test hid backend".to_string(),
    });
    let status = service.status_response();

    assert_eq!(status.provider, "hidmaestro");
    assert_eq!(status.backend_id, "hidmaestro");
    assert!(!status.available);
    assert_ne!(status.provider, "mock");
}

#[tokio::test]
async fn input_bridge_start_refuses_unknown_and_unconfigured_controllers() {
    let router = app(AgentState::from_controller_events([attach_event(
        "edge-bridge-refused",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(90),
    )]));

    let missing = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/sessions/missing/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let unconfigured = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/sessions/edge-bridge-refused/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unconfigured.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn input_bridge_start_requires_active_local_bridge_app() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-bridge-no-app",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(90),
    )]);
    {
        let mut inner = state.inner.write().await;
        let mut config = ControllerConfig::default_for("edge-bridge-no-app", "DualSense Edge");
        config.input_mode = ControllerInputMode::DsccInputBridge;
        config.input_bridge = InputBridgeConfig {
            enabled: true,
            ..InputBridgeConfig::default()
        }
        .normalized();
        inner
            .controller_configs
            .insert("edge-bridge-no-app".to_string(), config);
    }
    let router = app(state);

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/sessions/edge-bridge-no-app/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[cfg(target_os = "windows")]
#[test]
fn input_bridge_local_app_path_check_requires_registered_root() {
    let root = temp_test_dir("dscc-local-app-root");
    let outside = temp_test_dir("dscc-local-app-outside");
    fs::create_dir_all(&root).expect("local app root");
    fs::create_dir_all(&outside).expect("outside root");
    let registered_exe = root.join("NightDriveLab.exe");
    let outside_exe = outside.join("NightDriveLab.exe");
    fs::write(&registered_exe, b"fixture").expect("registered exe");
    fs::write(&outside_exe, b"fixture").expect("outside exe");
    let install_root = root.canonicalize().expect("canonical root");
    let game = UserGameConfig {
        game_id: "local-night-drive-lab-test".to_string(),
        app_id: "local:test".to_string(),
        name: "Night Drive Lab".to_string(),
        install_dir: "NightDriveLab".to_string(),
        install_path: install_root.display().to_string(),
        process_names: vec!["NightDriveLab.exe".to_string()],
        added_at: current_timestamp(),
    };

    assert!(local_app_process_path_allowed(
        &game,
        &install_root,
        &registered_exe
    ));
    assert!(!local_app_process_path_allowed(
        &game,
        &install_root,
        &outside_exe
    ));
}

#[tokio::test]
async fn input_bridge_session_can_start_and_stop_when_explicitly_enabled_for_local_app() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-bridge",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(90),
    )])
    .with_input_override("edge-bridge", sample_controller_input());
    {
        let mut inner = state.inner.write().await;
        let mut config = ControllerConfig::default_for("edge-bridge", "DualSense Edge");
        config.input_mode = ControllerInputMode::DsccInputBridge;
        config.input_bridge = InputBridgeConfig {
            enabled: true,
            ..InputBridgeConfig::default()
        }
        .normalized();
        inner
            .controller_configs
            .insert("edge-bridge".to_string(), config);
    }
    seed_active_local_game(&state, "local-night-drive-lab-test").await;
    let router = app(state);

    let start = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/sessions/edge-bridge/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::OK);
    let body = to_bytes(start.into_body(), 1024 * 1024).await.unwrap();
    let summary: InputBridgeSessionSummary = serde_json::from_slice(&body).unwrap();
    assert_eq!(summary.state, InputBridgeSessionState::Active);

    let stop = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/sessions/edge-bridge/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop.status(), StatusCode::OK);
    let body = to_bytes(stop.into_body(), 1024 * 1024).await.unwrap();
    let summary: InputBridgeSessionSummary = serde_json::from_slice(&body).unwrap();
    assert_eq!(summary.state, InputBridgeSessionState::Disabled);
}

#[tokio::test]
async fn input_bridge_binding_write_persists_typed_controller_config() {
    let router = app(AgentState::from_controller_events([attach_event(
        "edge-bridge-write",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(90),
    )]));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/input-bridge/bindings")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "controllerId":"edge-bridge-write",
                        "inputId":"button_a",
                        "target":"xinput_button b, , B"
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let config: ControllerConfig = get_json(
        router,
        "/api/controllers/edge-bridge-write/config",
        StatusCode::OK,
    )
    .await;
    assert!(config.input_bridge.bindings.iter().any(|binding| {
        binding.source == InputBridgeSource::Button("cross".to_string())
            && binding.target == InputBridgeTarget::Button(VirtualButton::B)
    }));
    assert!(!config.input_bridge.bindings.iter().any(|binding| {
        binding.source == InputBridgeSource::Button("cross".to_string())
            && binding.target == InputBridgeTarget::Button(VirtualButton::A)
    }));
}

#[tokio::test]
async fn new_input_bridge_mutations_reject_cross_origin_requests() {
    let router = app(AgentState::mock());
    for (uri, body) in [
        (
            "/api/input-bridge/bindings",
            r#"{"inputId":"cross","target":"xinput_button a, , A"}"#,
        ),
        ("/api/input-bridge/sessions/mock/start", "{}"),
        ("/api/input-bridge/sessions/mock/stop", "{}"),
        (
            "/api/games/local/validate",
            r#"{"name":"Night Drive Lab","executablePath":"C:\\Games\\NightDriveLab.exe"}"#,
        ),
        (
            "/api/games/local",
            r#"{"name":"Night Drive Lab","executablePath":"C:\\Games\\NightDriveLab.exe"}"#,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .header("host", "127.0.0.1:43473")
                    .header("origin", "http://evil.example")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test]
async fn local_app_routes_validate_add_and_reject_duplicate_executable() {
    let root = temp_test_dir("dscc-local-app");
    fs::create_dir_all(&root).expect("local app temp dir");
    let exe = root.join("NightDriveLab.exe");
    fs::write(&exe, b"mock exe").expect("local app exe fixture");
    let router = app(AgentState::mock());

    let validate = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/local/validate")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Night Drive Lab",
                        "executablePath": exe.display().to_string()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);
    let body = to_bytes(validate.into_body(), 1024 * 1024).await.unwrap();
    let validation_value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(validation_value.get("installPath").is_none());
    let validation: ValidateLocalGameResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(validation.executable_name, "NightDriveLab.exe");
    assert_eq!(
        validation.process_names,
        vec!["NightDriveLab.exe".to_string()]
    );

    for index in 0..USER_GAME_PROCESS_CANDIDATE_LIMIT {
        fs::write(root.join(format!("Aux{index}.exe")), b"mock exe")
            .expect("local app auxiliary exe fixture");
    }
    let process_names: Vec<String> = (0..USER_GAME_PROCESS_CANDIDATE_LIMIT)
        .map(|index| format!("Aux{index}.exe"))
        .collect();
    let capped_validate = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/local/validate")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Night Drive Lab",
                        "executablePath": exe.display().to_string(),
                        "processNames": process_names
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(capped_validate.status(), StatusCode::OK);
    let body = to_bytes(capped_validate.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let capped_validation: ValidateLocalGameResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        capped_validation.process_names.len(),
        USER_GAME_PROCESS_CANDIDATE_LIMIT
    );
    assert_eq!(capped_validation.process_names[0], "NightDriveLab.exe");
    assert!(!capped_validation
        .process_names
        .iter()
        .any(|process| process == "Aux7.exe"));

    let add = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/local")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Night Drive Lab",
                        "executablePath": exe.display().to_string()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add.status(), StatusCode::CREATED);
    let body = to_bytes(add.into_body(), 1024 * 1024).await.unwrap();
    let added: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let game = added.get("game").expect("response includes game");
    assert!(game
        .get("gameId")
        .and_then(|value| value.as_str())
        .is_some_and(|game_id| game_id.starts_with("local-night-drive-lab-")));
    assert!(game.get("installPath").is_none());

    let duplicate = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/local")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Renamed Lab",
                        "executablePath": exe.display().to_string()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let protected_process = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/local/validate")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Night Drive Lab",
                        "executablePath": exe.display().to_string(),
                        "processNames": ["explorer.exe"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(protected_process.status(), StatusCode::BAD_REQUEST);

    let _ = fs::remove_dir_all(root);
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
    let file = root
        .join("steamapps/common/Steam Controller Configs/123456/config/2483190/controller_ps5.vdf");
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
fn steam_input_paddle_preset_writes_only_edge_back_paddles_and_creates_backup() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-paddle-preset");
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
"title" "Forza Layout"
"revision" "5"
"controller_type" "controller_ps5_edge"
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
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button joystick_left, , "
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
                        "binding" "xinput_button joystick_right, , "
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
    }
}
}"##;
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");
    std::env::set_var("DSCC_STEAM_ROOT", &root);

    let response = write_steam_input_paddle_preset(SteamInputPaddlePresetRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        left_key: None,
        right_key: None,
        profile_name: Some("Forza Paddle Shift".to_string()),
        dry_run: false,
    })
    .expect("paddle preset writes");

    assert!(response.accepted);
    assert!(!response.dry_run);
    assert_eq!(response.paddles.len(), 2);
    assert_eq!(response.paddles[0].input_id, "button_back_left");
    assert_eq!(response.paddles[0].key, "Q");
    assert_eq!(response.paddles[0].binding.binding, "Q Key");
    assert_eq!(response.paddles[1].input_id, "button_back_right");
    assert_eq!(response.paddles[1].key, "E");
    assert_eq!(response.paddles[1].binding.binding, "E Key");

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
    assert!(updated.contains(r#""binding" "key_press Q, , ""#));
    assert!(updated.contains(r#""binding" "key_press E, , ""#));
    assert!(updated.contains(r#""binding" "xinput_button select, , ""#));
    assert!(updated.contains(r#""binding" "key_press M, , ""#));
    assert!(updated.contains(r#""title" "DSCC / Forza Paddle Shift""#));
    assert!(updated.contains(r#""revision" "6""#));
    assert!(!updated.contains(r#""binding" "xinput_button joystick_left, , ""#));
    assert!(!updated.contains(r#""binding" "xinput_button joystick_right, , ""#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_paddle_preset_uses_configurable_keys_in_dry_run() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-paddle-preset-dry");
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
"title" "Forza Layout"
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
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");
    std::env::set_var("DSCC_STEAM_ROOT", &root);

    let response = write_steam_input_paddle_preset(SteamInputPaddlePresetRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        left_key: Some("page up".to_string()),
        right_key: Some("page_down".to_string()),
        profile_name: Some("Forza Paddle Shift".to_string()),
        dry_run: true,
    })
    .expect("paddle preset dry run validates");

    assert!(response.dry_run);
    assert_eq!(response.backup_path, None);
    assert_eq!(response.paddles[0].key, "PAGE_UP");
    assert_eq!(response.paddles[0].binding.binding, "Page Up Key");
    assert_eq!(response.paddles[1].key, "PAGE_DOWN");
    assert_eq!(response.paddles[1].binding.binding, "Page Down Key");
    assert_eq!(
        fs::read_to_string(&layout_file).expect("layout still readable"),
        original
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_paddle_preset_rejects_missing_or_non_edge_bindings_cleanly() {
    let non_edge = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: Some("controller_ps5".to_string()),
        controller_label: Some("DualSense".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 0,
        bindings: Vec::new(),
    };
    let error = ensure_dualsense_edge_steam_layout(&non_edge)
        .expect_err("non-Edge layout should be rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("DualSense Edge"),
        "unexpected message: {}",
        error.message
    );

    let inferred_edge = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: None,
        controller_label: Some("DualSense Edge".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 2,
        bindings: vec![
            SteamInputBinding {
                input: "Back Left".to_string(),
                input_id: "button_back_left".to_string(),
                binding: "L3".to_string(),
                raw_binding: "xinput_button joystick_left, , ".to_string(),
                kind: "Gamepad".to_string(),
                source: Some("Switches".to_string()),
                source_mode: Some("Switches".to_string()),
                activator: Some("Full Press".to_string()),
                group_id: Some("7".to_string()),
            },
            SteamInputBinding {
                input: "Back Right".to_string(),
                input_id: "button_back_right".to_string(),
                binding: "R3".to_string(),
                raw_binding: "xinput_button joystick_right, , ".to_string(),
                kind: "Gamepad".to_string(),
                source: Some("Switches".to_string()),
                source_mode: Some("Switches".to_string()),
                activator: Some("Full Press".to_string()),
                group_id: Some("7".to_string()),
            },
        ],
    };
    ensure_dualsense_edge_steam_layout(&inferred_edge)
        .expect("layouts with both Edge back paddles are accepted");

    let edge_missing_right = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: Some("controller_ps5_edge".to_string()),
        controller_label: Some("DualSense Edge".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 1,
        bindings: vec![SteamInputBinding {
            input: "Back Left".to_string(),
            input_id: "button_back_left".to_string(),
            binding: "Q Key".to_string(),
            raw_binding: "key_press Q, , ".to_string(),
            kind: "Key".to_string(),
            source: Some("Switches".to_string()),
            source_mode: Some("Switches".to_string()),
            activator: Some("Full Press".to_string()),
            group_id: Some("7".to_string()),
        }],
    };
    let error = steam_edge_paddle_binding(&edge_missing_right, STEAM_EDGE_BACK_RIGHT_INPUT_ID)
        .expect_err("missing right paddle should be rejected");
    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert!(
        error.message.contains("Back Right"),
        "unexpected message: {}",
        error.message
    );
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

    let catalog =
        build_supported_steam_game_catalog(secondary, &[secondary.to_path_buf()], &[acr_manifest]);

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

#[test]
fn unix_process_scan_extracts_proton_windows_executable_names() {
    let names = parse_unix_process_names(
        "ForzaHorizon6.e /home/user/.steam/steamapps/common/ForzaHorizon6/ForzaHorizon6.exe -windowed\n\
         pressure-vessel pressure-vessel-wrap C:\\\\SteamLibrary\\\\ForzaHorizon5\\\\ForzaHorizon5.exe",
    );

    assert!(names.iter().any(|name| name == "ForzaHorizon6.exe"));
    assert!(names.iter().any(|name| name == "ForzaHorizon5.exe"));

    let detection = detect_running_game_from_processes(names.iter().map(String::as_str));
    assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
}

#[test]
fn telemetry_source_detection_recovers_forza_when_process_scan_misses_proton() {
    let state = AgentState::from_controller_events([attach_event(
        "linux-dualsense",
        ControllerFamily::DualSense,
        ControllerTransportKind::Usb,
        Some(25),
    )]);
    {
        let mut inner = state.inner.blocking_write();
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_packet(324, 1);
        inner.active_adapter_id = Some(FORZA_DATA_OUT_ADAPTER_ID.to_string());
        inner.telemetry = SignalSnapshot::from_updates([
            signal_update("source.id", FORZA_DATA_OUT_ADAPTER_ID),
            signal_update("game.state", "driving"),
            signal_update("input.brake", 0.20),
            signal_update("input.throttle", 0.45),
            signal_update("vehicle.speed_kmh", 80.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
    }

    let inner = state.inner.blocking_read();
    let detection = telemetry_game_detection(&inner, &SteamGameCatalog::default())
        .expect("live Forza Data Out packets should recover game detection");

    assert_eq!(detection.source, "telemetry_source");
    assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
    assert_eq!(
        detection.adapter_id.as_deref(),
        Some(FORZA_DATA_OUT_ADAPTER_ID)
    );
    assert_eq!(detection.profile_id.as_deref(), Some(IMMERSIVE_PROFILE_ID));

    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
    assert!(hardware_output_runtime_allowed_for_resolution(
        &inner,
        Some(&detection),
        &resolution
    ));
    assert!(support_telemetry_summary(&inner, None).live);
}

#[test]
fn global_profile_override_does_not_block_detected_supported_game() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-forza",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]);
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    {
        let mut inner = state.inner.blocking_write();
        inner.profile_overrides.insert(
            profile_override_key(None, None),
            ProfileOverride {
                controller_id: None,
                game_id: None,
                profile_id: DEFAULT_PROFILE_ID.to_string(),
            },
        );
    }

    let inner = state.inner.blocking_read();
    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(resolution.reason, "foreground_game");
    assert_eq!(resolution.override_profile_id, None);
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
}

#[tokio::test]
async fn cached_game_detection_keeps_standard_five_second_cache() {
    let state = AgentState::mock();
    let cached = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    let cached_game_id = cached.active_game_id.clone();
    let cached_at = Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

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
    let cached_at = Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

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
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
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
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
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
        signal.name == "game.state" && signal.value == serde_json::json!("awaiting_shared_memory")
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
fn forza_rev_limiter_buzz_uses_wall_form_at_high_throttle() {
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
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            assert!((frequency_hz - FORZA_REV_LIMITER_FREQUENCY_HZ).abs() < f64::EPSILON);
            assert_eq!(wall_zones, FORZA_REV_LIMITER_WALL_ZONES as u8);
            assert!(
                (0.28..0.30).contains(&strength),
                "high-throttle rev limiter should use a stronger wall-form buzz, got {strength}"
            );
        }
        other => panic!("expected rev limiter wall-form buzz, got {other:?}"),
    }
}

#[test]
fn forza_rev_limiter_buzz_stays_plain_near_idle() {
    let config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    let snapshot = SignalSnapshot::from_updates([
        signal_update("game.state", "driving"),
        signal_update("input.throttle", 0.25),
        signal_update("input.brake", 0.0),
        signal_update("input.handbrake", 0.0),
        signal_update("vehicle.rpm_ratio", 0.95),
        signal_update("vehicle.speed_kmh", 0.0),
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
            assert!((frequency_hz - FORZA_REV_LIMITER_FREQUENCY_HZ).abs() < f64::EPSILON);
            assert!(
                (0.28..0.30).contains(&amplitude),
                "low-throttle limiter blip should stay a stronger plain buzz, got {amplitude}"
            );
        }
        other => panic!("expected plain rev limiter buzz near idle, got {other:?}"),
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
    let parsed =
        parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 7).expect("packet parses");

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
    let adapters = materialized_adapters(&inner.adapters, &inner.adapter_runtimes, None);
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
    let parsed =
        parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 9).expect("packet parses");

    state
        .apply_adapter_packet(parsed.adapter_id, parsed.packet_len, 9, parsed.updates)
        .await;

    let inner = state.inner.read().await;
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
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

    let text =
        windows_utf16_bytes_to_search_text(&bytes).expect("SetupAPI UTF-16 text should be decoded");

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
    let root = std::env::temp_dir().join(format!("dscc-forza-glyph-test-{}", std::process::id()));
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
async fn controller_input_endpoint_returns_live_nested_state() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-input",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(72),
    )])
    .with_input_override("edge-input", sample_controller_input());
    let router = app(state);

    let input: ControllerInputResponse =
        get_json(router, "/api/controllers/edge-input/input", StatusCode::OK).await;

    assert!(input.available);
    assert_eq!(input.controller_id, "edge-input");
    assert_eq!(input.source, "hid");
    assert!(input.sampled_at_ms.is_some());
    assert!(input.age_ms.is_some_and(|age| age < 1_000));
    assert!((input.axes.left_stick.x - 0.25).abs() < f64::EPSILON);
    assert!((input.axes.right_stick.y + 0.75).abs() < f64::EPSILON);
    assert!((input.triggers.l2 - 0.4).abs() < f64::EPSILON);
    assert!(input
        .buttons
        .iter()
        .any(|button| button.id == "cross" && button.pressed && button.value == 1.0));
}

#[tokio::test]
async fn controller_input_endpoint_reuses_active_bridge_sample() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-bridge-input",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(72),
    )]);
    let sample = state.record_cached_input("edge-bridge-input", sample_controller_input());
    state
        .input_bridge
        .start_session(
            "edge-bridge-input",
            VirtualOutputKind::Xbox360,
            current_timestamp_millis(),
        )
        .expect("mock bridge session should start");
    let router = app(state);

    let input: ControllerInputResponse = get_json(
        router,
        "/api/controllers/edge-bridge-input/input",
        StatusCode::OK,
    )
    .await;

    assert!(input.available);
    assert_eq!(input.source, "hid");
    assert_eq!(input.sampled_at_ms, Some(sample.sampled_at_ms));
    assert!(input.age_ms.is_some_and(|age| age < 1_000));
    assert!((input.triggers.r2 - 0.8).abs() < f64::EPSILON);
}

#[tokio::test]
async fn detached_controller_clears_latest_input_sample() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-detach-input",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(72),
    )]);
    state.record_cached_input("edge-detach-input", sample_controller_input());
    assert!(state
        .cached_input_state("edge-detach-input", CONTROLLER_INPUT_UI_CACHE_TTL)
        .is_some());

    state
        .apply_controller_event(ControllerDiscoveryEvent::Detached(ControllerId(
            "edge-detach-input".to_string(),
        )))
        .await;

    assert!(state
        .cached_input_state("edge-detach-input", CONTROLLER_INPUT_UI_CACHE_TTL)
        .is_none());
}

#[tokio::test]
async fn current_controller_input_uses_connected_controller() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-current",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(72),
    )])
    .with_input_override("edge-current", sample_controller_input());
    let router = app(state);

    let input: ControllerInputResponse =
        get_json(router, "/api/controllers/current/input", StatusCode::OK).await;

    assert!(input.available);
    assert_eq!(input.controller_id, "edge-current");
    assert!((input.triggers.r2 - 0.8).abs() < f64::EPSILON);
}

#[tokio::test]
async fn controller_input_endpoint_reports_unavailable_without_live_report() {
    let router = app(AgentState::from_controller_events([attach_event(
        "usb-pad",
        ControllerFamily::DualSense,
        ControllerTransportKind::Usb,
        Some(72),
    )]));

    let input: ControllerInputResponse =
        get_json(router, "/api/controllers/usb-pad/input", StatusCode::OK).await;

    assert!(!input.available);
    assert_eq!(input.source, "hid");
    assert_eq!(input.sampled_at_ms, None);
    assert_eq!(input.age_ms, None);
    assert_eq!(
        input.axes.left_stick,
        ControllerInputStickResponse::default()
    );
    assert!(input.buttons.is_empty());
}

#[tokio::test]
async fn unknown_controller_input_returns_not_found() {
    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/controllers/no-such-controller/input")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
    let deserialized: PersistedAgentState = serde_json::from_str(&serialized).expect("deserialize");
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
        profiles: profiles_with_active(default_profiles(), &Some(DEFAULT_PROFILE_ID.to_string())),
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

fn sample_controller_input() -> ControllerInputState {
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

fn write_i32(packet: &mut [u8], offset: usize, value: i32) {
    packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_f32(packet: &mut [u8], offset: usize, value: f32) {
    packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
