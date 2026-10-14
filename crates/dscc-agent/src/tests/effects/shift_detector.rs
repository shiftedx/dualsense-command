use super::*;

fn detect_shift_event(
    runtime: &mut ForzaEffectRuntime,
    current_gear: Option<f64>,
    clutch: Option<f64>,
    telemetry_on: bool,
    shift_enabled: bool,
    now: Instant,
) -> Option<&'static str> {
    detect_shift_event_with_tuning(
        runtime,
        current_gear,
        clutch,
        telemetry_on,
        shift_enabled,
        &ForzaShiftTuningConfig::default(),
        now,
    )
}

fn detect_shift_event_with_tuning(
    runtime: &mut ForzaEffectRuntime,
    current_gear: Option<f64>,
    clutch: Option<f64>,
    telemetry_on: bool,
    shift_enabled: bool,
    shift_tuning: &ForzaShiftTuningConfig,
    now: Instant,
) -> Option<&'static str> {
    runtime.detect_shift_event(
        current_gear,
        clutch,
        telemetry_on,
        shift_enabled,
        shift_tuning,
        now,
    )
}

#[test]
fn forza_shift_detector_tracks_raw_direction_blind_gear_changes() {
    let mut runtime = test_forza_effect_runtime();
    let now = Instant::now();

    assert_eq!(
        detect_shift_event(&mut runtime, Some(3.0), None, true, true, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(0.0), None, true, true, now),
        Some("shift")
    );
    assert_eq!(runtime.latched_shift_event(now), Some("shift"));
    assert_eq!(
        detect_shift_event(&mut runtime, Some(4.0), None, true, true, now),
        Some("shift")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(3.0), None, true, true, now),
        Some("shift")
    );
    assert_eq!(runtime.latched_shift_event(now), Some("shift"));
}

#[test]
fn forza_shift_detector_suppresses_first_packet_and_hard_stops() {
    let mut runtime = test_forza_effect_runtime();
    let now = Instant::now();

    assert_eq!(
        detect_shift_event(&mut runtime, Some(3.0), None, true, true, now),
        Some("none")
    );
    assert_eq!(runtime.latched_shift_event(now), None);
    assert_eq!(
        detect_shift_event(&mut runtime, Some(4.0), None, true, true, now),
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
        detect_shift_event(&mut runtime, Some(3.0), None, true, true, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(4.0), None, true, true, now),
        Some("shift")
    );
    let second_shift = now + Duration::from_millis(50);
    assert_eq!(
        detect_shift_event(&mut runtime, Some(5.0), None, true, true, second_shift),
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
        detect_shift_event(&mut runtime, Some(3.0), None, true, false, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(4.0), None, true, false, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(5.0), None, true, true, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(6.0), None, true, true, now),
        Some("shift")
    );

    assert_eq!(
        detect_shift_event(&mut runtime, Some(7.0), None, false, true, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(8.0), None, true, true, now),
        Some("shift")
    );
}

#[test]
fn forza_shift_detector_auto_mode_uses_clutch_after_it_is_seen() {
    let mut runtime = test_forza_effect_runtime();
    let now = Instant::now();

    assert_eq!(
        detect_shift_event(&mut runtime, Some(3.0), Some(0.8), true, true, now),
        Some("none")
    );
    assert_eq!(
        detect_shift_event(&mut runtime, Some(4.0), Some(0.8), true, true, now),
        Some("smooth_shift")
    );
    assert_eq!(runtime.latched_shift_event(now), Some("smooth_shift"));
    assert!(
        (runtime.latched_shift_pulse(now) - default_forza_shift_with_clutch_strength()).abs()
            < f64::EPSILON
    );
    assert_eq!(
        runtime.latched_shift_event(now + Duration::from_millis(129)),
        Some("smooth_shift")
    );
    assert_eq!(
        runtime.latched_shift_event(now + Duration::from_millis(130)),
        None
    );

    let rough_shift = now + Duration::from_millis(160);
    assert_eq!(
        detect_shift_event(&mut runtime, Some(5.0), Some(0.0), true, true, rough_shift,),
        Some("rough_shift")
    );
    assert_eq!(
        runtime.latched_shift_event(rough_shift + Duration::from_millis(239)),
        Some("rough_shift")
    );
    assert_eq!(
        runtime.latched_shift_event(rough_shift + Duration::from_millis(240)),
        None
    );
}

#[test]
fn forza_shift_detector_manual_clutch_mode_punishes_missing_clutch_without_prior_seen() {
    let mut runtime = test_forza_effect_runtime();
    let now = Instant::now();
    let shift_tuning = ForzaShiftTuningConfig {
        clutch_mode: "manual_clutch".to_string(),
        ..ForzaShiftTuningConfig::default()
    };

    assert_eq!(
        detect_shift_event_with_tuning(
            &mut runtime,
            Some(2.0),
            Some(0.0),
            true,
            true,
            &shift_tuning,
            now,
        ),
        Some("none")
    );
    assert_eq!(
        detect_shift_event_with_tuning(
            &mut runtime,
            Some(3.0),
            Some(0.0),
            true,
            true,
            &shift_tuning,
            now,
        ),
        Some("rough_shift")
    );
    assert_eq!(runtime.latched_shift_pulse(now), 1.0);
}
