use super::support::*;
use super::*;
use dscc_core::input_bridge::DsccBridgeCommand;

async fn seed_cycle_profiles(state: &AgentState, ids: &[&str], active: Option<&str>) {
    let mut inner = state.inner.write().await;
    inner.profiles = ids
        .iter()
        .map(|id| ProfileSummary {
            id: id.to_string(),
            name: id.to_string(),
            built_in: false,
            active: Some(*id) == active,
            game_id: None,
        })
        .collect();
    inner.active_profile_id = active.map(str::to_string);
}

async fn active_profile_id(state: &AgentState) -> Option<String> {
    state.inner.read().await.active_profile_id.clone()
}

fn drain_saw_cycle_invalidation(events: &mut broadcast::Receiver<RealtimeMessage>) -> bool {
    let mut saw = false;
    while let Ok(message) = events.try_recv() {
        if message.message.as_deref() == Some("input-bridge-profile-cycled") {
            saw = true;
        }
    }
    saw
}

#[tokio::test]
async fn dispatch_bridge_command_profile_next_activates_next_profile() {
    let state = AgentState::mock();
    seed_cycle_profiles(&state, &["alpha", "bravo", "charlie"], Some("alpha")).await;
    let mut events = state.subscribe_events();

    dispatch_bridge_command(&state, DsccBridgeCommand::ProfileNext).await;

    assert_eq!(active_profile_id(&state).await.as_deref(), Some("bravo"));
    assert!(
        drain_saw_cycle_invalidation(&mut events),
        "profile cycle must broadcast an input-bridge-profile-cycled invalidation"
    );
}

#[tokio::test]
async fn dispatch_bridge_command_profile_previous_activates_previous_profile() {
    let state = AgentState::mock();
    seed_cycle_profiles(&state, &["alpha", "bravo", "charlie"], Some("alpha")).await;

    dispatch_bridge_command(&state, DsccBridgeCommand::ProfilePrevious).await;

    assert_eq!(
        active_profile_id(&state).await.as_deref(),
        Some("charlie"),
        "ProfilePrevious must cycle backward, not forward"
    );
}

#[tokio::test]
async fn dispatch_bridge_command_is_noop_with_single_profile() {
    let state = AgentState::mock();
    seed_cycle_profiles(&state, &["solo"], Some("solo")).await;
    let mut events = state.subscribe_events();

    dispatch_bridge_command(&state, DsccBridgeCommand::ProfileNext).await;

    assert_eq!(active_profile_id(&state).await.as_deref(), Some("solo"));
    assert!(!drain_saw_cycle_invalidation(&mut events));
}

#[tokio::test]
async fn dispatch_bridge_command_ignores_shift_layer() {
    let state = AgentState::mock();
    seed_cycle_profiles(&state, &["alpha", "bravo"], Some("alpha")).await;
    let mut events = state.subscribe_events();

    dispatch_bridge_command(&state, DsccBridgeCommand::ShiftLayer).await;

    assert_eq!(active_profile_id(&state).await.as_deref(), Some("alpha"));
    assert!(!drain_saw_cycle_invalidation(&mut events));
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
