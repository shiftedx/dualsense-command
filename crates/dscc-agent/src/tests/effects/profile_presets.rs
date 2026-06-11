use super::*;

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
        abs.intensity, 26,
        "ABS preset intensity should match the promoted New Brakes FH6 setting"
    );
    assert_eq!(preset.abs.mode, "strong_pulse");
    assert!((preset.abs.slip_threshold - FORZA_ABS_SLIP_THRESHOLD).abs() < f64::EPSILON);
    assert!((preset.abs.brake_threshold_ratio - FORZA_ABS_RANGE_START_RATIO).abs() < f64::EPSILON);
    assert!((preset.abs.min_speed_kmh - FORZA_ABS_MIN_SPEED_KMH).abs() < f64::EPSILON);
    assert!((preset.abs.min_strength - FORZA_ABS_PULSE_MIN_AMPLITUDE).abs() < f64::EPSILON);
    assert!((preset.abs.frequency_hz - FORZA_ABS_PULSE_FREQUENCY_HZ).abs() < f64::EPSILON);
    assert!((preset.abs.curve - 1.0).abs() < f64::EPSILON);

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
        rpm_leds.enabled,
        "Base should enable the redline ramp without restoring the old constant RPM lighting"
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
        rpm_leds.enabled,
        "Immersive should enable the redline ramp by default"
    );
    assert_eq!(rpm_leds.intensity, 100);
    assert_eq!(rpm_leds.route, "light_led");
    assert_eq!(effect("brake_resistance").intensity, 77);
    assert_eq!(effect("abs_slip_pulse").intensity, 26);
    assert_eq!(preset.abs.mode, "strong_pulse");
    assert!((preset.abs.slip_threshold - FORZA_ABS_SLIP_THRESHOLD).abs() < f64::EPSILON);
    assert!((preset.abs.brake_threshold_ratio - FORZA_ABS_RANGE_START_RATIO).abs() < f64::EPSILON);
    assert!((preset.abs.min_speed_kmh - FORZA_ABS_MIN_SPEED_KMH).abs() < f64::EPSILON);
    assert!((preset.abs.min_strength - FORZA_ABS_PULSE_MIN_AMPLITUDE).abs() < f64::EPSILON);
    assert!((preset.abs.frequency_hz - FORZA_ABS_PULSE_FREQUENCY_HZ).abs() < f64::EPSILON);
    assert!((preset.abs.curve - 1.0).abs() < f64::EPSILON);
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
    assert_eq!(brake.intensity, 77);
    assert_eq!(brake.route, "l2");

    let abs = config
        .forza
        .effects
        .iter()
        .find(|effect| effect.id == "abs_slip_pulse")
        .expect("abs_slip_pulse present after activation");
    assert!(abs.enabled);
    assert_eq!(abs.intensity, 26);
    assert_eq!(abs.route, "l2");

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
    assert!(rpm_leds.enabled);
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
        effect("rpm_leds").enabled,
        "Immersive should enable the redline ramp"
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
