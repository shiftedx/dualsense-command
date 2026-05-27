use super::*;

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
