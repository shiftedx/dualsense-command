use super::*;

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
