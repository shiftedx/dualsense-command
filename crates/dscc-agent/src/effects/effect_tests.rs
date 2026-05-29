use super::*;

pub(crate) fn effect_test_output_frame(request: &EffectTestRequest) -> ControllerOutputFrame {
    let target = request.target.as_deref().unwrap_or("r2");
    let mode = request.mode.as_deref().unwrap_or("adaptive_resistance");
    let intensity = f64::from(request.intensity.unwrap_or(65).min(100)) / 100.0;
    let start_position = request.start_position.unwrap_or(0.16).clamp(0.0, 1.0);
    let mut frame = ControllerOutputFrame::default();

    match target {
        "base_feel" => {
            return base_feel_test_output_frame(
                request.trigger.clone().unwrap_or_default(),
                request.l2_position,
                request.r2_position,
            )
        }
        "l2" => frame.l2 = trigger_for_mode(mode, intensity, start_position),
        "r2" => frame.r2 = trigger_for_mode(mode, intensity, start_position),
        "lightbar" => {
            frame.lightbar = Some(LightbarOutput {
                color: LightbarConfig {
                    enabled: true,
                    color: mode.to_string(),
                    rpm_color: default_rpm_color(),
                    brightness: request.intensity.unwrap_or(65).min(100),
                }
                .normalized()
                .rgb(),
                brightness: intensity,
            });
        }
        "rumble" => {
            frame.rumble = Some(rumble_for_mode(mode, intensity));
        }
        _ => frame.r2 = trigger_for_mode(mode, intensity, start_position),
    }

    frame
}

pub(crate) fn base_feel_test_output_frame(
    trigger: TriggerConfig,
    l2_position: Option<f64>,
    r2_position: Option<f64>,
) -> ControllerOutputFrame {
    let trigger = trigger.normalized();
    ControllerOutputFrame {
        l2: base_feel_trigger_output(
            &trigger.effect,
            &trigger.intensity,
            trigger.l2_from,
            trigger.l2_to,
            &trigger.l2_curve_points,
            l2_position,
        ),
        r2: base_feel_trigger_output(
            &trigger.effect,
            &trigger.intensity,
            trigger.r2_from,
            trigger.r2_to,
            &trigger.r2_curve_points,
            r2_position,
        ),
        ..Default::default()
    }
}

fn base_feel_trigger_output(
    effect: &str,
    intensity_label: &str,
    from: u8,
    to: u8,
    curve_points: &[TriggerCurvePoint],
    position: Option<f64>,
) -> TriggerOutput {
    let strength = position.map_or_else(
        || {
            (trigger_strength_for_label(intensity_label) * (f64::from(to.min(100)) / 100.0))
                .clamp(0.0, 1.0)
        },
        |position| trigger_curve_strength(position, from, to, curve_points, intensity_label),
    );
    if effect == "Off" || strength <= f64::EPSILON {
        return TriggerOutput::Off;
    }
    let mode = effect.to_ascii_lowercase().replace(' ', "_");
    trigger_for_mode(&mode, strength, f64::from(from.min(100)) / 100.0)
}

pub(crate) fn trigger_range_end_position(from: u8, to: u8) -> f64 {
    let start_percent = from.min(100);
    let start = f64::from(start_percent) / 100.0;
    let end = f64::from(to.clamp(start_percent, 100)) / 100.0;
    end.max(start + 0.01)
}

fn endstop_wall_position(start: f64, end: f64) -> f64 {
    (end - FORZA_ENDSTOP_WALL_OFFSET).clamp(start, end)
}

pub(crate) fn brake_overtravel_guard_active(end: f64) -> bool {
    end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION
}

pub(crate) fn brake_overtravel_wall_position(start: f64, end: f64) -> f64 {
    if brake_overtravel_guard_active(end) {
        return (end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET)
            .max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION)
            .clamp(start, end);
    }

    endstop_wall_position(start, end)
}

pub(crate) fn throttle_overtravel_guard_active(end: f64, guard_min_end: f64) -> bool {
    end >= guard_min_end.clamp(0.0, 1.0)
}

pub(crate) fn throttle_overtravel_wall_position(
    start: f64,
    end: f64,
    wall_position: f64,
    guard_min_end: f64,
) -> f64 {
    if throttle_overtravel_guard_active(end, guard_min_end) {
        return end.min(wall_position.clamp(0.0, 1.0)).clamp(start, end);
    }

    endstop_wall_position(start, end)
}

pub(crate) fn throttle_overtravel_ramp_start(start: f64, wall: f64, ramp_width: f64) -> f64 {
    let ramp_start = wall - ramp_width.clamp(0.01, 0.80);
    ((ramp_start * 1000.0).round() / 1000.0).clamp(start, wall)
}

pub(crate) fn abs_brake_threshold_for_range(start: f64, end: f64, ratio: f64) -> f64 {
    let threshold = start + (end - start) * ratio.clamp(0.0, 1.0);
    threshold.clamp(start, end)
}

fn trigger_curve_strength(
    position: f64,
    from: u8,
    to: u8,
    curve_points: &[TriggerCurvePoint],
    intensity_label: &str,
) -> f64 {
    let strength = trigger_strength_for_label(intensity_label);
    if strength <= f64::EPSILON {
        return 0.0;
    }

    let start = f64::from(from.min(100)) / 100.0;
    let end = trigger_range_end_position(from, to);
    let x = clamp_unit(position);
    if x <= start {
        return 0.0;
    }

    let active = trigger_curve_point_output(curve_points, clamp_unit((x - start) / (end - start)));
    clamp_unit(active * strength)
}

fn trigger_curve_point_output(points: &[TriggerCurvePoint], active: f64) -> f64 {
    let points = normalize_trigger_curve_points(points.to_vec(), TriggerCurve::default_l2());
    let active = clamp_unit(active);
    for window in points.windows(2) {
        let left = window[0];
        let right = window[1];
        let left_input = f64::from(left.input) / 100.0;
        let right_input = f64::from(right.input) / 100.0;
        if active >= left_input && active <= right_input {
            if (right_input - left_input).abs() < f64::EPSILON {
                return f64::from(right.output) / 100.0;
            }
            let ratio = (active - left_input) / (right_input - left_input);
            let left_output = f64::from(left.output) / 100.0;
            let right_output = f64::from(right.output) / 100.0;
            return left_output + (right_output - left_output) * ratio;
        }
    }

    points
        .last()
        .map(|point| f64::from(point.output) / 100.0)
        .unwrap_or(0.0)
}

fn trigger_strength_for_label(intensity_label: &str) -> f64 {
    match intensity_label {
        "Off" => 0.0,
        "Weak" => 0.36,
        "Medium" => 0.68,
        _ => 1.0,
    }
}

fn trigger_for_mode(mode: &str, intensity: f64, start_position: f64) -> TriggerOutput {
    match mode {
        "off" => TriggerOutput::Off,
        "wall" => TriggerOutput::Wall {
            position: (start_position + intensity * 0.34).clamp(0.0, 1.0),
            strength: intensity,
        },
        "pulse" => TriggerOutput::Pulse {
            amplitude: intensity,
            frequency_hz: 18.0 + intensity * 42.0,
        },
        "pulse_ab" | "wall_pulse" => TriggerOutput::PulseAb {
            strength: intensity,
            frequency_hz: 18.0 + intensity * 42.0,
            wall_zones: 2,
        },
        _ => TriggerOutput::AdaptiveResistance {
            start_position,
            strength: intensity,
        },
    }
}

fn rumble_for_mode(mode: &str, intensity: f64) -> RumbleOutput {
    let intensity = clamp_unit(intensity);
    let (low, high) = match mode {
        "deep_thump" | "low" => (intensity, intensity * 0.18),
        "fine_buzz" | "high" => (intensity * 0.18, intensity),
        _ => apply_vibration_mode(mode, intensity, intensity * 0.82),
    };
    RumbleOutput {
        low_frequency: clamp_unit(low),
        high_frequency: clamp_unit(high),
    }
}
