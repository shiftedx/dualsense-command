use super::support::*;
use super::*;

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
