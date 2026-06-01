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
            assert!((start_position - 0.06).abs() < f64::EPSILON);
            assert!(
                (0.24..0.27).contains(&strength),
                "idle brake should have a firm preload before pedal travel builds, got {strength}"
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
            assert!((start_position - 0.80).abs() < f64::EPSILON);
            assert!(
                (0.99..=1.0).contains(&strength),
                "partial brake should hold a throttle-like wall through the final travel, got {strength}"
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
            assert!((start_position - 0.80).abs() < f64::EPSILON);
            assert!(
                strength > 0.98 && strength <= 1.0,
                "full brake should create a throttle-wall-level end stop, got {strength}"
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
fn forza_throttle_advanced_tuning_moves_ramp_and_force_levels() {
    let mut config = forza_horizon_controller_config();
    config.forza.throttle.baseline_force = 0.05;
    config.forza.throttle.normal_force = 0.20;
    config.forza.throttle.endstop_force = 0.50;
    config.forza.throttle.endstop_boost = 1.50;
    config.forza.throttle.guard_min_end = 0.60;
    config.forza.throttle.wall_position = 0.70;
    config.forza.throttle.ramp_width = 0.10;
    config.forza.throttle.ramp_curve = 1.0;
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

    let ramp_mid = EffectEngine::new().evaluate(&profile, &snapshot(0.65));
    match ramp_mid.r2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.60).abs() < f64::EPSILON);
            assert!(
                (0.40..0.42).contains(&strength),
                "custom throttle ramp should interpolate from normal force to boosted end stop, got {strength}"
            );
        }
        other => panic!("expected custom throttle ramp, got {other:?}"),
    }

    let wall = EffectEngine::new().evaluate(&profile, &snapshot(0.72));
    match wall.r2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.70).abs() < f64::EPSILON);
            assert!(
                (0.64..0.66).contains(&strength),
                "custom throttle wall should use the configured max force and boost, got {strength}"
            );
        }
        other => panic!("expected custom throttle wall, got {other:?}"),
    }
}

#[test]
fn forza_brake_load_uses_global_wall_in_final_travel() {
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

    let below = EffectEngine::new().evaluate(&profile, &snapshot(0.47));
    match below.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.06).abs() < f64::EPSILON);
            assert!(
                (0.84..0.87).contains(&strength),
                "brake should already have clear mid-pedal force before the end-load ramp, got {strength}"
            );
        }
        other => panic!("expected continuous brake load before the end-load ramp, got {other:?}"),
    }

    let high = EffectEngine::new().evaluate(&profile, &snapshot(0.84));
    match high.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.80).abs() < f64::EPSILON);
            assert!(
                (0.99..=1.0).contains(&strength),
                "brake should hold a throttle-like wall through the final travel, got {strength}"
            );
        }
        other => panic!("expected brake final-travel wall, got {other:?}"),
    }

    let full = EffectEngine::new().evaluate(&profile, &snapshot(0.94));
    match full.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.80).abs() < f64::EPSILON);
            assert!(
                strength > 0.98 && strength <= 1.0,
                "brake should reach throttle-wall-level force near the configured end point, got {strength}"
            );
        }
        other => panic!("expected max brake force at the configured end point, got {other:?}"),
    }
}

#[test]
fn forza_brake_advanced_tuning_moves_wall_and_force_levels() {
    let mut config = forza_horizon_controller_config();
    config.forza.brake.baseline_force = 0.40;
    config.forza.brake.normal_force = 0.70;
    config.forza.brake.endstop_force = 0.80;
    config.forza.brake.endstop_boost = 1.20;
    config.forza.brake.guard_min_end = 0.50;
    config.forza.brake.wall_position = 0.58;
    config.forza.brake.full_force_at = 0.86;
    config.forza.brake.ramp_curve = 1.0;
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

    let before_wall = EffectEngine::new().evaluate(&profile, &snapshot(0.55));
    match before_wall.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.06).abs() < f64::EPSILON);
            assert!(
                (0.59..0.61).contains(&strength),
                "custom brake curve should hold the configured pedal force before the wall, got {strength}"
            );
        }
        other => panic!("expected custom brake load before the wall, got {other:?}"),
    }

    let ramp = EffectEngine::new().evaluate(&profile, &snapshot(0.72));
    match ramp.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.06).abs() < f64::EPSILON);
            assert!(
                (0.70..0.73).contains(&strength),
                "custom brake wall should begin ramping toward boosted force, got {strength}"
            );
        }
        other => panic!("expected custom brake wall ramp, got {other:?}"),
    }

    let full = EffectEngine::new().evaluate(&profile, &snapshot(0.88));
    match full.l2 {
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            assert!((start_position - 0.86).abs() < f64::EPSILON);
            assert!(
                (0.82..0.84).contains(&strength),
                "custom brake full-force point should use the configured boosted force, got {strength}"
            );
        }
        other => panic!("expected custom brake full-force point, got {other:?}"),
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
            assert!((start_position - 0.60).abs() < f64::EPSILON);
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
            assert!((frequency_hz - FORZA_ABS_PULSE_FREQUENCY_HZ).abs() < f64::EPSILON);
            assert!(
                (0.99..=1.0).contains(&amplitude),
                "ABS pulse should be impossible to miss when slip is high, got {amplitude}"
            );
        }
        other => panic!("expected strong ABS trigger pulse, got {other:?}"),
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
        other => panic!("expected ABS trigger pulse after adjusted threshold, got {other:?}"),
    }
}

#[test]
fn forza_abs_advanced_tuning_can_select_fine_flutter_mode() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    config.forza.abs.mode = "fine_flutter".to_string();
    config.forza.abs.slip_source = "front".to_string();
    config.forza.abs.brake_threshold_ratio = 0.20;
    config.forza.abs.slip_threshold = 0.50;
    config.forza.abs.min_strength = 0.35;
    config.forza.abs.max_strength = 0.75;
    config.forza.abs.frequency_hz = 42.0;
    config.forza.abs.curve = 1.20;
    let snapshot = SignalSnapshot::from_updates([
        signal_update("game.state", "driving"),
        signal_update("input.throttle", 0.0),
        signal_update("input.brake", 0.40),
        signal_update("input.handbrake", 0.0),
        signal_update("vehicle.rpm_ratio", 0.40),
        signal_update("vehicle.speed_kmh", 55.0),
        signal_update("wheel.slip.front_max", 1.0),
        signal_update("tire.slip_ratio.max", 0.0),
        signal_update("wheel.slip.max", 0.0),
        signal_update("drivetrain.shift_event", "none"),
    ]);
    let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));
    let frame = EffectEngine::new().evaluate(&profile, &snapshot);

    match frame.l2 {
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            assert!((strength - 0.75).abs() < f64::EPSILON);
            assert!((frequency_hz - 42.0).abs() < f64::EPSILON);
            assert_eq!(wall_zones, FORZA_ABS_FINE_FLUTTER_WALL_ZONES as u8);
        }
        other => panic!("expected custom fine-flutter ABS output, got {other:?}"),
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
fn forza_rev_limiter_advanced_tuning_controls_threshold_strength_and_wall_form() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    let rev = config
        .forza
        .effects
        .iter_mut()
        .find(|effect| effect.id == "rev_limiter_buzz")
        .expect("default rev limiter tuning exists");
    rev.enabled = true;
    rev.intensity = 100;
    rev.route = "r2".to_string();
    config.forza.rev_limiter.threshold_ratio = 0.98;
    config.forza.rev_limiter.min_strength = 0.10;
    config.forza.rev_limiter.max_strength = 0.50;
    config.forza.rev_limiter.frequency_hz = 55.0;
    config.forza.rev_limiter.wall_form_throttle_at = 0.90;
    config.forza.rev_limiter.wall_zones = 7.0;
    config.forza.rev_limiter.curve = 1.0;
    let profile = forza_runtime_profile("forza-horizon", "Forza", Some(&config));

    let snapshot = |rpm, throttle| {
        SignalSnapshot::from_updates([
            signal_update("game.state", "driving"),
            signal_update("input.throttle", throttle),
            signal_update("input.brake", 0.0),
            signal_update("input.handbrake", 0.0),
            signal_update("vehicle.rpm_ratio", rpm),
            signal_update("vehicle.speed_kmh", 95.0),
            signal_update("tire.slip_ratio.max", 0.0),
            signal_update("wheel.slip.max", 0.0),
            signal_update("drivetrain.shift_event", "none"),
        ])
    };

    let below_threshold = EffectEngine::new().evaluate(&profile, &snapshot(0.95, 1.0));
    match below_threshold.r2 {
        TriggerOutput::AdaptiveResistance { .. } => {}
        other => panic!("rev limiter should wait for custom RPM threshold, got {other:?}"),
    }

    let limiter = EffectEngine::new().evaluate(&profile, &snapshot(0.99, 0.95));
    match limiter.r2 {
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            assert!((frequency_hz - 55.0).abs() < f64::EPSILON);
            assert_eq!(wall_zones, 7);
            assert!(
                (0.25..0.27).contains(&strength),
                "custom rev limiter should interpolate strength by RPM, got {strength}"
            );
        }
        other => panic!("expected custom rev limiter wall-form buzz, got {other:?}"),
    }
}
