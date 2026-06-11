use super::*;

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
    assert!(effect_enabled("rpm_leds"));
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
    assert!(effect_enabled("rpm_leds"));
    assert!(effect_enabled("road_texture"));
    assert!(effect_enabled("brake_resistance"));
}
