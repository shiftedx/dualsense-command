use super::support::*;
use super::*;

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
async fn controller_api_includes_sanitized_power_diagnostics() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-output-api",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Usb,
        Some(100),
    )]);
    let frame = ControllerOutputFrame {
        r2: TriggerOutput::AdaptiveResistance {
            start_position: 0.42,
            strength: 0.20,
        },
        ..ControllerOutputFrame::default()
    };
    let now = Instant::now();
    state.record_output_frame_write("edge-output-api", &frame, DeviceTransportKind::Usb, now);
    assert!(!state.output_frame_write_due(
        "edge-output-api",
        &frame,
        DeviceTransportKind::Usb,
        now + Duration::from_millis(33),
    ));
    let router = app(state);

    let controllers: Vec<ControllerSummary> =
        get_json(router.clone(), "/api/controllers", StatusCode::OK).await;
    assert_eq!(controllers[0].power_diagnostics.written_reports, 1);
    assert_eq!(
        controllers[0]
            .power_diagnostics
            .suppressed_redundant_reports,
        1
    );
    assert_eq!(
        controllers[0].power_diagnostics.keepalive_interval_ms,
        HARDWARE_OUTPUT_KEEPALIVE_INTERVAL.as_millis() as u64
    );
    assert!(controllers[0].power_diagnostics.last_write_age_ms.is_some());
    assert!(controllers[0]
        .power_diagnostics
        .last_suppressed_age_ms
        .is_some());
    assert!(controllers[0].power_diagnostics.native_rumble_passthrough);
    assert!(controllers[0].power_diagnostics.adaptive_triggers_retained);

    let controllers_json: serde_json::Value =
        get_json(router.clone(), "/api/controllers", StatusCode::OK).await;
    let first_controller = &controllers_json
        .as_array()
        .expect("controllers response is an array")[0];
    assert!(first_controller.get("power_diagnostics").is_some());
    assert!(first_controller.get("output_diagnostics").is_none());
    assert!(first_controller
        .get("power_diagnostics")
        .and_then(|value| value.get("suppressedRedundantReports"))
        .is_some());

    let detail: ControllerDetail =
        get_json(router, "/api/controllers/edge-output-api", StatusCode::OK).await;
    assert_eq!(
        detail.power_diagnostics.written_reports,
        controllers[0].power_diagnostics.written_reports
    );
    assert_eq!(
        detail.power_diagnostics.suppressed_redundant_reports,
        controllers[0]
            .power_diagnostics
            .suppressed_redundant_reports
    );
    assert_eq!(
        detail.power_diagnostics.keepalive_interval_ms,
        controllers[0].power_diagnostics.keepalive_interval_ms
    );
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
