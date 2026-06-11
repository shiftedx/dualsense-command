use super::*;

/// Built-in Forza preset designed from first principles to be immersive
/// without draining battery. The product owner directive is:
///
/// - Adaptive triggers do the heavy lifting (the DualSense's adaptive
///   triggers are passive solenoid loads — they only draw current while a
///   trigger is being squeezed, so they are essentially free at idle).
/// - Continuous low-amplitude body rumble (the rotating-mass actuators) is
///   the dominant battery drain on a DualSense. Road texture is enabled as
///   the default surface cue, while heavier continuous effects such as
///   rumble strip, suspension impact, tire slip, and puddle drag stay off.
///   Event-driven thumps (gear-shift, handbrake) stay enabled because they
///   only fire for a fraction of a second at a time.
/// - Intensities for the enabled effects are tuned conservatively against
///   the existing first-principles baseline forces in this file
///   (`FORZA_BRAKE_*`, `FORZA_THROTTLE_*`, etc.). All values come from the
///   public DualSense HID spec (trigger force 0..=255, body rumble 0..=255)
///   and physics intuition (real-car ABS modulates ~10-15 Hz, comfortable
///   pulse haptics are 20-50 Hz). No values were taken from any external
///   implementation.
///
/// The preset is written into a controller's saved `ForzaTelemetryConfig`
/// at profile-activation time, so changing profiles immediately rewrites
/// the controller config and the UI re-reads the new values.
pub(crate) fn forza_preset_for_profile(profile_id: &str) -> Option<ForzaTelemetryConfig> {
    match profile_id {
        FORZA_HORIZON_PROFILE_ID => Some(forza_horizon_preset()),
        IMMERSIVE_PROFILE_ID => Some(forza_horizon_immersive_preset()),
        ASSETTO_CORSA_RALLY_PROFILE_ID => Some(assetto_corsa_rally_preset()),
        _ => None,
    }
}

/// Battery-conscious "Base" preset. Adaptive triggers do most of the work,
/// with road texture enabled as the default surface cue.
pub(crate) fn forza_horizon_preset() -> ForzaTelemetryConfig {
    // (id, enabled, intensity 0..=255, route)
    //
    // Routes follow the natural side of each effect:
    //   - Brake / ABS  -> L2 adaptive trigger (left).
    //   - Throttle / rev limiter -> R2 adaptive trigger (right).
    //   - Handbrake -> L2 (driver actuates it from the left side).
    //   - Shift thump -> R2 + reduced body thump (short event, no sustained rumble).
    //
    // Road texture is the stock surface cue. Heavier continuous-rumble
    // effects (rumble strip, suspension impact, tire slip, puddle drag)
    // stay disabled by default; users can opt in via the tuning UI.
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 77, "l2"),
        ("throttle_resistance", true, 100, "r2"),
        ("abs_slip_pulse", true, 26, "l2"),
        ("handbrake_wall", true, 100, "l2"),
        ("rev_limiter_buzz", true, 85, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("road_texture", true, 40, "body_both"),
        ("rumble_strip", false, 55, "body_both"),
        ("tire_slip", false, 65, "body_right"),
        ("puddle_drag", false, 50, "body_left"),
        ("suspension_impact", false, 70, "body_both"),
        ("rpm_leds", true, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
        brake: ForzaBrakeTuningConfig::default(),
        abs: forza_horizon_abs_tuning(),
        throttle: ForzaThrottleTuningConfig::default(),
        shift: ForzaShiftTuningConfig::default(),
        rev_limiter: ForzaRevLimiterTuningConfig::default(),
    }
    .normalized()
}

/// Richer "Immersive" preset. This keeps the same trigger language as the stock
/// preset, then adds low-to-mid body layers for slip, curbs, puddles, and
/// suspension. Sustained tire slip stays restrained so it does not blur the
/// controller, while suspension impact is treated as a stronger event cue for
/// landing thumps. The redline ramp stays on because it gives a low-cost shift
/// cue without bringing back the old constant gear/RPM light show.
pub(crate) fn forza_horizon_immersive_preset() -> ForzaTelemetryConfig {
    // (id, enabled, intensity 0..=255, route)
    //
    // Body routing is intentionally spatial:
    //   - Tire slip -> right grip, so traction loss lives on the throttle side.
    //   - Puddle drag -> left grip, so water feels different from throttle load.
    //   - Suspension -> both grips with enough headroom to stand out on landings.
    //   - Rumble strips -> both grips, but below shift and impact events.
    //   - Redline ramp -> enabled; it warms up near shift RPM and blinks at the limiter.
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 77, "l2"),
        ("throttle_resistance", true, 100, "r2"),
        ("abs_slip_pulse", true, 26, "l2"),
        ("handbrake_wall", true, 100, "l2"),
        ("rev_limiter_buzz", true, 95, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("road_texture", true, 35, "body_both"),
        ("rumble_strip", true, 38, "body_both"),
        ("tire_slip", true, 30, "body_right"),
        ("puddle_drag", true, 32, "body_left"),
        ("suspension_impact", true, 82, "body_both"),
        ("rpm_leds", true, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
        brake: ForzaBrakeTuningConfig::default(),
        abs: forza_horizon_immersive_abs_tuning(),
        throttle: ForzaThrottleTuningConfig::default(),
        shift: ForzaShiftTuningConfig::default(),
        rev_limiter: ForzaRevLimiterTuningConfig::default(),
    }
    .normalized()
}

/// Rally preset for Assetto Corsa Rally. It reuses DSCC's normalized racing
/// signal names, but tunes the surface and shift layers for a looser road feel.
pub(crate) fn assetto_corsa_rally_preset() -> ForzaTelemetryConfig {
    let entries: &[(&str, bool, u8, &str)] = &[
        ("brake_resistance", true, 100, "l2"),
        ("throttle_resistance", true, 92, "r2"),
        ("abs_slip_pulse", true, 95, "l2"),
        ("handbrake_wall", true, 115, "l2"),
        ("rev_limiter_buzz", true, 90, "r2"),
        (
            "gear_shift_thump",
            true,
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY.saturating_add(22),
            "r2_and_body",
        ),
        ("road_texture", true, 46, "body_both"),
        ("rumble_strip", true, 35, "body_both"),
        ("tire_slip", true, 62, "body_right"),
        ("puddle_drag", false, 28, "body_left"),
        ("suspension_impact", true, 64, "body_both"),
        ("rpm_leds", true, 100, "light_led"),
    ];

    let effects = entries
        .iter()
        .map(|(id, enabled, intensity, route)| ForzaEffectConfig {
            id: (*id).to_string(),
            enabled: *enabled,
            intensity: *intensity,
            route: (*route).to_string(),
        })
        .collect();

    ForzaTelemetryConfig {
        body_rumble_mode: default_forza_body_rumble_mode(),
        effects,
        brake: ForzaBrakeTuningConfig::default(),
        abs: ForzaAbsTuningConfig::default(),
        throttle: ForzaThrottleTuningConfig::default(),
        shift: ForzaShiftTuningConfig::default(),
        rev_limiter: ForzaRevLimiterTuningConfig::default(),
    }
    .normalized()
}

pub(crate) fn forza_horizon_trigger_preset() -> TriggerConfig {
    TriggerConfig {
        same_range: false,
        l2_from: 0,
        l2_to: 100,
        r2_from: 4,
        r2_to: 100,
        l2_curve: TriggerCurve::from_ratio(FORZA_BRAKE_CURVE),
        r2_curve: TriggerCurve::from_ratio(FORZA_THROTTLE_CURVE),
        l2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(
            FORZA_BRAKE_CURVE,
        )),
        r2_curve_points: trigger_curve_points_from_curve(TriggerCurve::from_ratio(
            FORZA_THROTTLE_CURVE,
        )),
        effect: "Adaptive resistance".to_string(),
        intensity: "Strong (Standard)".to_string(),
        vibration: "Medium".to_string(),
        vibration_mode: "Balanced".to_string(),
    }
    .normalized()
}

fn forza_horizon_abs_tuning() -> ForzaAbsTuningConfig {
    standard_forza_abs_tuning()
}

fn forza_horizon_immersive_abs_tuning() -> ForzaAbsTuningConfig {
    standard_forza_abs_tuning()
}

fn standard_forza_abs_tuning() -> ForzaAbsTuningConfig {
    ForzaAbsTuningConfig {
        mode: "strong_pulse".to_string(),
        slip_source: "auto_front_first".to_string(),
        slip_threshold: FORZA_ABS_SLIP_THRESHOLD,
        brake_threshold_ratio: FORZA_ABS_RANGE_START_RATIO,
        min_speed_kmh: FORZA_ABS_MIN_SPEED_KMH,
        min_strength: FORZA_ABS_PULSE_MIN_AMPLITUDE,
        max_strength: 1.0,
        frequency_hz: FORZA_ABS_PULSE_FREQUENCY_HZ,
        curve: 1.0,
    }
    .normalized()
}
