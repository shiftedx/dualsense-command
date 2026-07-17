use super::support::*;
use super::*;

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
async fn log_history_is_capped_and_drops_oldest_entries() {
    let state = AgentState::mock();
    {
        let mut inner = state.inner.write().await;
        let seeded = inner.logs.len();
        for index in 0..(MAX_LOGS + 40) {
            inner.push_log(LogEntry {
                level: "info".to_string(),
                message: format!("bounded log entry {index}"),
                timestamp: current_timestamp(),
            });
        }
        assert!(seeded > 0, "mock state seeds at least one log entry");
        assert_eq!(inner.logs.len(), MAX_LOGS);
        assert_eq!(
            inner.logs.last().map(|entry| entry.message.as_str()),
            Some(format!("bounded log entry {}", MAX_LOGS + 39).as_str())
        );
        assert!(!inner
            .logs
            .iter()
            .any(|entry| entry.message == "bounded log entry 0"));
    }

    let logs: Vec<LogEntry> = get_json(app(state), "/api/logs", StatusCode::OK).await;
    assert_eq!(logs.len(), MAX_LOGS);
}

#[tokio::test]
async fn support_bundle_route_returns_sanitized_shareable_payload() {
    let _env = TestEnv::new(&["USERPROFILE", "HOME", "DSCC_WEB_DIST"]);
    std::env::set_var("USERPROFILE", r"C:\Users\ExampleUser");
    std::env::set_var("HOME", "/home/example-user");
    std::env::set_var("DSCC_WEB_DIST", r"D:\PrivateLab\DSCC Redaction Fixture");
    let state = AgentState::mock();
    {
        let mut inner = state.inner.write().await;
        inner.app_settings.forza_playstation_glyphs.install_path =
            Some(r"C:\Users\ExampleUser\SteamLibrary\ForzaHorizon6".to_string());
        inner.app_settings.forza_playstation_glyphs.last_message =
            r"Installed from C:\Users\ExampleUser\SteamLibrary\ForzaHorizon6\userdata\123456789\config"
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
    assert!(!body_text.contains(r"C:\Users\ExampleUser"));
    assert!(!body_text.contains(r"C:\\Users\\ExampleUser"));
    assert!(!body_text.contains("123456789"));
    assert!(!body_text.contains("SteamLibrary"));
    assert!(!body_text.contains("PrivateLab"));
    assert!(!body_text.contains("DSCC Redaction Fixture"));
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
        r"Installed at \\?\D:\SteamLibrary\steamapps\common\ForzaHorizon6. User path C:\Users\ExampleUser\Documents\dscc. Layout steamapps/common/Steam Controller Configs/60706926/config/controller.vdf and userdata\76561198000000000\config.",
    );

    assert!(sanitized.contains("[local-path]"));
    assert!(sanitized.contains("<steam-user>"));
    assert!(!sanitized.contains("D:\\"));
    assert!(!sanitized.contains("C:\\Users\\ExampleUser"));
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

#[tokio::test]
async fn sensitive_mutation_routes_reject_cross_origin_requests() {
    let routes = [
        (Method::PUT, "/api/app-settings"),
        (Method::PUT, "/api/controllers/controller-0001"),
        (Method::PUT, "/api/controllers/controller-0001/config"),
        (
            Method::PUT,
            "/api/controllers/controller-0001/edge-profiles/fn_triangle",
        ),
        (Method::POST, "/api/controllers/controller-0001/test-effect"),
        (Method::POST, "/api/controllers/current/test-effect"),
        (Method::POST, "/api/profiles"),
        (Method::POST, "/api/profiles/import"),
        (Method::PUT, "/api/profiles/global/config"),
        (Method::POST, "/api/profiles/global/activate"),
        (Method::PUT, "/api/adapters/forza-data-out"),
        (Method::POST, "/api/steam-input/bindings"),
        (Method::POST, "/api/steam-input/paddle-preset"),
        (Method::POST, "/api/input-bridge/bindings"),
        (
            Method::POST,
            "/api/input-bridge/sessions/controller-0001/start",
        ),
        (
            Method::POST,
            "/api/input-bridge/sessions/controller-0001/stop",
        ),
        (Method::POST, "/api/games/local/validate"),
        (Method::POST, "/api/games/local"),
        (Method::POST, "/api/games/custom"),
        (Method::DELETE, "/api/games/custom/custom-game"),
        (Method::PUT, "/api/profile-resolution/override"),
        (Method::DELETE, "/api/profile-resolution/override"),
    ];

    for (method, uri) in routes {
        let response = app(AgentState::mock())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("host", "127.0.0.1:43473")
                    .header("origin", "http://evil.example")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
}

#[test]
fn cross_origin_websocket_origin_guard_rejects_host_mismatch() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "127.0.0.1:43473".parse().unwrap());
    headers.insert(header::ORIGIN, "http://evil.example".parse().unwrap());

    assert!(!request_origin_matches_host(&headers));
}

#[test]
fn absent_origin_is_allowed_for_loopback_local_tools() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "127.0.0.1:43473".parse().unwrap());

    assert!(request_origin_matches_host(&headers));
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
fn app_paths_honors_dscc_config_dir_override() {
    let _env = TestEnv::new(&["DSCC_CONFIG_DIR"]);
    let config_dir = temp_test_dir("dscc-paths-config");
    std::env::set_var("DSCC_CONFIG_DIR", &config_dir);

    let paths = app_paths().expect("app paths resolve with override set");
    assert_eq!(paths.config_dir, config_dir.display().to_string());
    // Single source of truth: paths must report the directory persistence uses.
    let store = PersistenceStore::default().expect("persistence store resolves");
    assert_eq!(store.state_file, config_dir.join("state.json"));

    std::env::remove_var("DSCC_CONFIG_DIR");
    let default_paths = app_paths().expect("app paths resolve without override");
    assert_ne!(default_paths.config_dir, config_dir.display().to_string());
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
