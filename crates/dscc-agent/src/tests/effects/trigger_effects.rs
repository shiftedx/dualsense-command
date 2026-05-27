use super::*;

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
