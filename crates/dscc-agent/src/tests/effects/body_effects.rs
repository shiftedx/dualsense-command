use super::*;

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
fn forza_shift_advanced_tuning_moves_wall_form_threshold_and_frequency() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    let shift = config
        .forza
        .effects
        .iter_mut()
        .find(|effect| effect.id == "gear_shift_thump")
        .expect("default shift tuning exists");
    shift.enabled = true;
    shift.intensity = 100;
    shift.route = "r2".to_string();
    config.forza.shift.wall_form_at = 0.80;
    config.forza.shift.frequency_hz = 48.0;
    config.forza.shift.wall_zones = 6.0;
    let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));

    let snapshot = |throttle| {
        SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", throttle),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", 0.70),
            signal_update("vehicle.speed_kmh", 118.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "shift"),
        ])
    };

    let below_wall = EffectEngine::new().evaluate(&profile, &snapshot(0.70));
    match below_wall.r2 {
        TriggerOutput::Pulse {
            amplitude,
            frequency_hz,
        } => {
            assert!((frequency_hz - 48.0).abs() < f64::EPSILON);
            assert!((0.99..=1.0).contains(&amplitude));
        }
        other => panic!("expected tuned plain shift pulse below wall threshold, got {other:?}"),
    }

    let above_wall = EffectEngine::new().evaluate(&profile, &snapshot(0.85));
    match above_wall.r2 {
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            assert!((frequency_hz - 48.0).abs() < f64::EPSILON);
            assert_eq!(wall_zones, 6);
            assert!((0.99..=1.0).contains(&strength));
        }
        other => panic!("expected tuned wall-form shift pulse above threshold, got {other:?}"),
    }
}
