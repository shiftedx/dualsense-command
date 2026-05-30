use super::*;

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
fn trigger_config_derives_point_arrays_from_ratio_curves() {
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
    .expect("trigger config with ratio curves should deserialize");

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
fn trigger_config_migrates_previous_soft_default_brake_curve() {
    let trigger = TriggerConfig {
        l2_curve: TriggerCurve::from_ratio(1.45),
        l2_curve_points: legacy_soft_l2_trigger_curve_points(),
        ..TriggerConfig::default()
    }
    .normalized();

    assert_eq!(trigger.l2_curve, TriggerCurve::default_l2());
    assert_eq!(trigger.l2_curve_points, default_l2_trigger_curve_points());
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
