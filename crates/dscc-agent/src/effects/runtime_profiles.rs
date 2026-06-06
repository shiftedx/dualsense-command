use super::*;

pub(crate) fn controller_output_target_or_reason(
    inner: &AgentStateInner,
    controller_id: &str,
) -> Result<ControllerOutputTarget, String> {
    if let Some(target) = inner.controllers.output_target(controller_id) {
        return Ok(target);
    }

    let Some(detail) = inner.controllers.detail(controller_id) else {
        return Err(format!("Controller {controller_id} is not known to DSCC"));
    };
    if is_windows_pnp_controller_id(controller_id) {
        return Err(
            "Controller is visible only through the Windows PnP fallback; no HID output handle is available"
                .to_string(),
        );
    }
    if !detail.connected {
        return Err(format!("Controller {controller_id} is disconnected"));
    }
    if detail.permission != ControllerPermissionState::Granted {
        return Err(format!(
            "Controller {controller_id} does not have HID permission"
        ));
    }
    if !detail.capabilities.adaptive_triggers {
        return Err(format!(
            "Controller {controller_id} does not advertise adaptive trigger support"
        ));
    }

    Err(format!(
        "Controller {controller_id} has no openable raw HID output target"
    ))
}

pub(crate) fn controller_config_for_resolution(
    inner: &AgentStateInner,
    resolution: &ProfileResolutionResponse,
) -> Option<ControllerConfig> {
    let controller_id = resolution.controller_id.as_deref()?;
    inner
        .controller_configs
        .get(controller_id)
        .cloned()
        .or_else(|| {
            inner
                .controllers
                .detail(controller_id)
                .map(|detail| ControllerConfig::default_for(controller_id, detail.model))
        })
}

pub(crate) fn profile_name_by_id(inner: &AgentStateInner, profile_id: &str) -> Option<String> {
    inner
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| profile.name.clone())
}

pub(crate) fn runtime_profile_for(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
) -> Profile {
    if profile_id == GLOBAL_PROFILE_ID {
        return global_runtime_profile(profile_id, profile_name, config);
    }

    if is_forza_runtime_profile(profile_id, snapshot) {
        forza_runtime_profile(profile_id, profile_name, config)
    } else {
        generic_runtime_profile(profile_id, profile_name, config)
    }
}

pub(crate) fn is_forza_runtime_profile(profile_id: &str, snapshot: &SignalSnapshot) -> bool {
    profile_id.contains("forza")
        || profile_id == ASSETTO_CORSA_RALLY_PROFILE_ID
        || snapshot.text("source.id").is_some_and(|source| {
            matches!(
                source,
                FORZA_DATA_OUT_ADAPTER_ID | ASSETTO_SHARED_MEMORY_ADAPTER_ID
            )
        })
        || snapshot
            .text("game.id")
            .is_some_and(|game| game.starts_with("forza") || game == "assetto-corsa-rally")
}

fn global_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::Disabled,
        rules: lightbar_rules(config.map(|config| &config.lightbar)),
    }
}

// Effect-rule priority ladder for Forza trigger layering (higher wins). Named
// here so the precedence contract is explicit at every rule-push site instead
// of living as bare integer literals.
const ABS_FRONT_SLIP_PRIORITY: i32 = 62;
const ABS_TIRE_SLIP_PRIORITY: i32 = 61;
const ABS_WHEEL_SLIP_PRIORITY: i32 = 60;
const SHIFT_THUMP_PRIORITY: i32 = 70;
const REV_LIMITER_PRIORITY: i32 = 55;
const HANDBRAKE_WALL_PRIORITY: i32 = 45;
const TRIGGER_ENDSTOP_PRIORITY: i32 = 12;
const TRIGGER_OVERTRAVEL_RAMP_PRIORITY: i32 = 11;
const TRIGGER_BASELINE_PRIORITY: i32 = 10;

/// Derived adaptive-trigger geometry for a Forza profile: the start/end points,
/// overtravel knees, end-stop walls, ramp starts and ABS threshold that the
/// effect-rule builders consume. All fields are in 0..=1 trigger-travel units.
///
/// Computed purely from already-normalized config, so it can be exercised
/// directly in unit tests instead of read back out of an assembled `Profile`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TriggerPositions {
    pub l2_start: f64,
    pub l2_end: f64,
    pub l2_force_knee: f64,
    pub l2_final_stop_input: f64,
    pub l2_final_stop_position: f64,
    pub l2_normal_end: f64,
    pub l2_has_overtravel_guard: bool,
    pub r2_start: f64,
    pub r2_end: f64,
    pub r2_endstop_wall: f64,
    pub r2_overtravel_ramp_start: f64,
    pub r2_normal_end: f64,
    pub r2_has_overtravel_guard: bool,
    pub abs_brake_threshold: f64,
}

/// Derive the Forza trigger geometry from a (raw) trigger config plus the
/// already-normalized brake/throttle/abs tuning.
///
/// Invariants on the result (all guaranteed by the helpers below):
/// `l2_start <= l2_force_knee <= l2_end`, `l2_final_stop_position` and
/// `abs_brake_threshold` lie in `[l2_start, l2_end]`, `l2_normal_end >=
/// l2_start + 0.01`, `r2_start <= r2_overtravel_ramp_start <= r2_endstop_wall
/// <= r2_end`, and `r2_normal_end >= r2_start + 0.01`. When `trigger` is `None`
/// the start/end points fall back to the Forza defaults. Total: no panics, no
/// `Result` — coherence is already guaranteed by the callers' `normalized()`.
pub(crate) fn compute_trigger_positions(
    trigger: Option<&TriggerConfig>,
    brake_tuning: &ForzaBrakeTuningConfig,
    throttle_tuning: &ForzaThrottleTuningConfig,
    abs_tuning: &ForzaAbsTuningConfig,
) -> TriggerPositions {
    let l2_start = trigger.map_or(0.18, |trigger| f64::from(trigger.l2_from.min(100)) / 100.0);
    let r2_start = trigger.map_or(0.10, |trigger| f64::from(trigger.r2_from.min(100)) / 100.0);
    let l2_end = trigger.map_or(FORZA_BRAKE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.l2_from, trigger.l2_to)
    });
    let r2_end = trigger.map_or(FORZA_THROTTLE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.r2_from, trigger.r2_to)
    });
    let l2_has_overtravel_guard = brake_overtravel_guard_active(l2_end, brake_tuning.guard_min_end);
    let l2_force_knee = brake_overtravel_wall_position(
        l2_start,
        l2_end,
        brake_tuning.wall_position,
        brake_tuning.guard_min_end,
    );
    let r2_has_overtravel_guard =
        throttle_overtravel_guard_active(r2_end, throttle_tuning.guard_min_end);
    let r2_endstop_wall = throttle_overtravel_wall_position(
        r2_start,
        r2_end,
        throttle_tuning.wall_position,
        throttle_tuning.guard_min_end,
    );
    let r2_overtravel_ramp_start =
        throttle_overtravel_ramp_start(r2_start, r2_endstop_wall, throttle_tuning.ramp_width);
    let l2_final_stop_input =
        brake_full_force_position(l2_force_knee, l2_end, brake_tuning.full_force_at);
    let l2_final_stop_position = l2_final_stop_input.clamp(l2_start, l2_end);
    let l2_normal_end = if l2_has_overtravel_guard {
        l2_force_knee
    } else {
        l2_force_knee.min(l2_end)
    }
    .max(l2_start + 0.01);
    let r2_normal_end = if r2_has_overtravel_guard && r2_overtravel_ramp_start < r2_endstop_wall {
        r2_overtravel_ramp_start
    } else {
        r2_endstop_wall
    }
    .max(r2_start + 0.01);
    let abs_brake_threshold =
        abs_brake_threshold_for_range(l2_start, l2_end, abs_tuning.brake_threshold_ratio);

    TriggerPositions {
        l2_start,
        l2_end,
        l2_force_knee,
        l2_final_stop_input,
        l2_final_stop_position,
        l2_normal_end,
        l2_has_overtravel_guard,
        r2_start,
        r2_end,
        r2_endstop_wall,
        r2_overtravel_ramp_start,
        r2_normal_end,
        r2_has_overtravel_guard,
        abs_brake_threshold,
    }
}

pub(crate) fn forza_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    let trigger = config.map(|config| &config.trigger);
    let lightbar = config.map(|config| &config.lightbar);
    // The resolver materializes the selected profile into this cloned config
    // before evaluation, so automatic game detection can use the right preset
    // without requiring the UI to save/apply it first.
    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let intensity = trigger.map_or(0.82, trigger_intensity_scalar);
    if trigger.is_some_and(|trigger| trigger.effect == "Off") || intensity <= 0.0 {
        return Profile {
            id: profile_id.to_string(),
            name: profile_name.to_string(),
            version: 1,
            rumble_policy: RumblePolicy::FullControl,
            rules: lightbar_rules(lightbar),
        };
    }

    let brake_tuning = forza.brake.clone().normalized();
    let throttle_tuning = forza.throttle.clone().normalized();
    let abs_tuning = forza.abs.clone().normalized();
    let shift_tuning = forza.shift.clone().normalized();
    let rev_tuning = forza.rev_limiter.clone().normalized();
    // Derived trigger geometry now lives behind one pure, testable interface;
    // destructure it back into the local names the rule builders already use.
    let TriggerPositions {
        l2_start,
        l2_end,
        l2_force_knee,
        l2_final_stop_input,
        l2_final_stop_position,
        l2_normal_end,
        l2_has_overtravel_guard,
        r2_start,
        r2_endstop_wall,
        r2_overtravel_ramp_start,
        r2_normal_end,
        r2_has_overtravel_guard,
        abs_brake_threshold,
        ..
    } = compute_trigger_positions(trigger, &brake_tuning, &throttle_tuning, &abs_tuning);
    let l2_curve_points = trigger
        .map(|trigger| trigger_curve_value_points(&trigger.l2_curve_points))
        .unwrap_or_else(|| trigger_curve_value_points(&default_l2_trigger_curve_points()));
    let r2_curve_points = trigger
        .map(|trigger| trigger_curve_value_points(&trigger.r2_curve_points))
        .unwrap_or_else(|| trigger_curve_value_points(&default_r2_trigger_curve_points()));
    let brake = forza.effect("brake_resistance");
    let abs = forza.effect("abs_slip_pulse");
    let handbrake = forza.effect("handbrake_wall");
    let throttle = forza.effect("throttle_resistance");
    let shift = forza.effect("gear_shift_thump");
    let rev = forza.effect("rev_limiter_buzz");
    let trigger_scalar = intensity.clamp(0.0, 1.0);
    let brake_baseline_force =
        scaled_unit(brake_tuning.baseline_force, brake.scalar() * trigger_scalar);
    let brake_normal_force =
        scaled_unit(brake_tuning.normal_force, brake.scalar() * trigger_scalar);
    let brake_endstop_force = scaled_unit(
        brake_tuning.endstop_force,
        brake.scalar() * trigger_scalar * brake_tuning.endstop_boost,
    );
    let throttle_baseline_force = scaled_unit(
        throttle_tuning.baseline_force,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_normal_force = scaled_unit(
        throttle_tuning.normal_force,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_endstop_scalar =
        throttle.scalar() * trigger_scalar * throttle_tuning.endstop_boost;
    let throttle_endstop_force =
        scaled_unit(throttle_tuning.endstop_force, throttle_endstop_scalar);
    let abs_min_amplitude = scaled_unit(abs_tuning.min_strength, abs.scalar());
    let abs_max_amplitude = scaled_unit(abs_tuning.max_strength, abs.scalar());
    let rev_min_amplitude = scaled_unit(rev_tuning.min_strength, rev.scalar() * trigger_scalar);
    let rev_max_amplitude = scaled_unit(rev_tuning.max_strength, rev.scalar() * trigger_scalar);
    let shift_amplitude = scaled_unit(1.0, shift.scalar());

    let baseline_condition = forza_baseline_trigger_condition();
    let mut rules = Vec::new();

    if abs.scalar() > 0.0 && route_has_l2(&abs.route) {
        let abs_sources: Vec<(&str, &str, i32)> = match abs_tuning.slip_source.as_str() {
            "front" => vec![(
                "forza-l2-abs-front-slip-pulse",
                "wheel.slip.front_max",
                ABS_FRONT_SLIP_PRIORITY,
            )],
            "tire" => vec![(
                "forza-l2-abs-tire-slip-pulse",
                "tire.slip_ratio.max",
                ABS_TIRE_SLIP_PRIORITY,
            )],
            "wheel" => vec![(
                "forza-l2-abs-wheel-slip-pulse",
                "wheel.slip.max",
                ABS_WHEEL_SLIP_PRIORITY,
            )],
            _ => vec![
                (
                    "forza-l2-abs-front-slip-pulse",
                    "wheel.slip.front_max",
                    ABS_FRONT_SLIP_PRIORITY,
                ),
                (
                    "forza-l2-abs-tire-slip-pulse",
                    "tire.slip_ratio.max",
                    ABS_TIRE_SLIP_PRIORITY,
                ),
                (
                    "forza-l2-abs-wheel-slip-pulse",
                    "wheel.slip.max",
                    ABS_WHEEL_SLIP_PRIORITY,
                ),
            ],
        };

        for (id, signal, priority) in abs_sources {
            let strength = ValueSource::signal_curve(
                signal,
                abs_tuning.slip_threshold,
                abs_tuning.slip_threshold + FORZA_ABS_SLIP_RANGE_WIDTH,
                abs_min_amplitude,
                abs_max_amplitude,
                abs_tuning.curve,
            );
            let frequency_hz = ValueSource::constant(abs_tuning.frequency_hz);
            let effect = if abs_tuning.mode == "fine_flutter" {
                EffectTemplate::PulseAb {
                    strength,
                    frequency_hz,
                    wall_zones: ValueSource::constant(FORZA_ABS_FINE_FLUTTER_WALL_ZONES),
                }
            } else {
                EffectTemplate::Pulse {
                    amplitude: strength,
                    frequency_hz,
                }
            };

            rules.push(EffectRule {
                id: id.to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::L2,
                priority,
                condition: RuleCondition::All {
                    conditions: vec![
                        number_condition(
                            "input.brake",
                            ComparisonOp::GreaterOrEqual,
                            abs_brake_threshold,
                        ),
                        number_condition(
                            "vehicle.speed_kmh",
                            ComparisonOp::GreaterOrEqual,
                            abs_tuning.min_speed_kmh,
                        ),
                        number_condition(
                            signal,
                            ComparisonOp::GreaterOrEqual,
                            abs_tuning.slip_threshold,
                        ),
                    ],
                },
                effect,
            });
        }
    }

    if handbrake.scalar() > 0.0 && route_has_l2(&handbrake.route) {
        rules.push(EffectRule {
            id: "forza-l2-handbrake-wall".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: HANDBRAKE_WALL_PRIORITY,
            condition: number_condition("input.handbrake", ComparisonOp::GreaterThan, 0.05),
            effect: EffectTemplate::Wall {
                position: ValueSource::constant((l2_start + 0.12).clamp(0.0, 0.86)),
                strength: ValueSource::constant(scaled_unit(
                    FORZA_HANDBRAKE_FORCE,
                    handbrake.scalar() * trigger_scalar,
                )),
            },
        });
    }

    if brake.scalar() > 0.0 && route_has_l2(&brake.route) {
        rules.push(EffectRule {
            id: "forza-l2-brake-full-force".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: TRIGGER_ENDSTOP_PRIORITY,
            condition: number_condition(
                "input.brake",
                ComparisonOp::GreaterOrEqual,
                l2_final_stop_input,
            ),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(l2_final_stop_position),
                strength: ValueSource::constant(brake_endstop_force),
            },
        });
        if l2_has_overtravel_guard && l2_force_knee < l2_end {
            rules.push(EffectRule {
                id: "forza-l2-brake-overtravel-ramp".to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::L2,
                priority: TRIGGER_OVERTRAVEL_RAMP_PRIORITY,
                condition: number_condition(
                    "input.brake",
                    ComparisonOp::GreaterOrEqual,
                    l2_force_knee,
                ),
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(l2_start),
                    strength: ValueSource::signal_curve(
                        "input.brake",
                        l2_force_knee,
                        l2_final_stop_input,
                        brake_normal_force,
                        brake_endstop_force,
                        brake_tuning.ramp_curve,
                    ),
                },
            });
        }
        rules.push(EffectRule {
            id: "forza-l2-brake-resistance".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: TRIGGER_BASELINE_PRIORITY,
            condition: baseline_condition.clone(),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(l2_start),
                strength: ValueSource::signal_points(
                    "input.brake",
                    l2_start,
                    l2_normal_end,
                    brake_baseline_force,
                    brake_normal_force,
                    l2_curve_points.clone(),
                ),
            },
        });
    }

    push_rev_limiter_rules(
        &mut rules,
        &rev,
        RevLimiterRuleShape {
            id: "forza-rev-limiter-buzz",
            priority: REV_LIMITER_PRIORITY,
            condition: number_condition(
                "vehicle.rpm_ratio",
                ComparisonOp::GreaterOrEqual,
                rev_tuning.threshold_ratio,
            ),
            amplitude: ValueSource::signal_curve(
                "vehicle.rpm_ratio",
                rev_tuning.threshold_ratio,
                1.0,
                rev_min_amplitude,
                rev_max_amplitude,
                rev_tuning.curve,
            ),
            frequency_hz: ValueSource::constant(rev_tuning.frequency_hz),
        },
        &rev_tuning,
    );
    push_shift_thump_rules(&mut rules, &shift, shift_amplitude, &shift_tuning);

    if throttle.scalar() > 0.0 && route_has_r2(&throttle.route) {
        rules.push(EffectRule {
            id: "forza-r2-throttle-full-force".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: TRIGGER_ENDSTOP_PRIORITY,
            condition: number_condition(
                "input.throttle",
                ComparisonOp::GreaterOrEqual,
                r2_endstop_wall,
            ),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(r2_endstop_wall),
                strength: ValueSource::constant(throttle_endstop_force),
            },
        });
        if r2_has_overtravel_guard && r2_overtravel_ramp_start < r2_endstop_wall {
            rules.push(EffectRule {
                id: "forza-r2-throttle-overtravel-ramp".to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::R2,
                priority: TRIGGER_OVERTRAVEL_RAMP_PRIORITY,
                condition: number_condition(
                    "input.throttle",
                    ComparisonOp::GreaterOrEqual,
                    r2_overtravel_ramp_start,
                ),
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(r2_overtravel_ramp_start),
                    strength: ValueSource::signal_curve(
                        "input.throttle",
                        r2_overtravel_ramp_start,
                        r2_endstop_wall,
                        throttle_normal_force,
                        throttle_endstop_force,
                        throttle_tuning.ramp_curve,
                    ),
                },
            });
        }
        rules.push(EffectRule {
            id: "forza-r2-throttle-resistance".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: TRIGGER_BASELINE_PRIORITY,
            condition: baseline_condition,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(r2_start),
                strength: ValueSource::signal_points(
                    "input.throttle",
                    r2_start,
                    r2_normal_end,
                    throttle_baseline_force,
                    throttle_normal_force,
                    r2_curve_points.clone(),
                ),
            },
        });
    }

    rules.extend(lightbar_rules(lightbar));

    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::FullControl,
        rules,
    }
}

fn forza_baseline_trigger_condition() -> RuleCondition {
    text_condition("game.state", ComparisonOp::Eq, "driving")
}

struct RevLimiterRuleShape {
    id: &'static str,
    priority: i32,
    condition: RuleCondition,
    amplitude: ValueSource,
    frequency_hz: ValueSource,
}

fn push_rev_limiter_rules(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    shape: RevLimiterRuleShape,
    rev_tuning: &ForzaRevLimiterTuningConfig,
) {
    if tuning.scalar() <= 0.0 {
        return;
    }

    for target in routed_trigger_targets(&tuning.route) {
        let target_label = trigger_target_label(target);
        rules.push(EffectRule {
            id: format!("{}-{target_label}-wall-form", shape.id),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: shape.priority,
            condition: RuleCondition::All {
                conditions: vec![
                    shape.condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::GreaterOrEqual,
                        rev_tuning.wall_form_throttle_at,
                    ),
                ],
            },
            effect: EffectTemplate::PulseAb {
                strength: shape.amplitude.clone(),
                frequency_hz: shape.frequency_hz.clone(),
                wall_zones: ValueSource::constant(rev_tuning.wall_zones),
            },
        });
        rules.push(EffectRule {
            id: format!("{}-{target_label}-pulse", shape.id),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: shape.priority,
            condition: RuleCondition::All {
                conditions: vec![
                    shape.condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::LessThan,
                        rev_tuning.wall_form_throttle_at,
                    ),
                ],
            },
            effect: EffectTemplate::Pulse {
                amplitude: shape.amplitude.clone(),
                frequency_hz: shape.frequency_hz.clone(),
            },
        });
    }
}

fn push_shift_thump_rules(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    shift_amplitude: f64,
    shift_tuning: &ForzaShiftTuningConfig,
) {
    if tuning.scalar() <= 0.0 {
        return;
    }

    for (target, pedal_signal) in [
        (EffectTarget::L2, "input.brake"),
        (EffectTarget::R2, "input.throttle"),
    ] {
        if !route_targets_trigger(&tuning.route, target) {
            continue;
        }

        let target_label = trigger_target_label(target);
        rules.push(EffectRule {
            id: format!("forza-gear-shift-thump-{target_label}-pulse-ab"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: SHIFT_THUMP_PRIORITY,
            condition: shift_thump_condition(
                pedal_signal,
                ComparisonOp::GreaterOrEqual,
                shift_tuning.wall_form_at,
            ),
            effect: EffectTemplate::PulseAb {
                strength: ValueSource::signal_scale(
                    "drivetrain.shift_pulse",
                    0.0,
                    1.0,
                    0.0,
                    shift_amplitude,
                ),
                frequency_hz: ValueSource::constant(shift_tuning.frequency_hz),
                wall_zones: ValueSource::constant(shift_tuning.wall_zones),
            },
        });
        rules.push(EffectRule {
            id: format!("forza-gear-shift-thump-{target_label}-pulse"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: SHIFT_THUMP_PRIORITY,
            condition: shift_thump_condition(
                pedal_signal,
                ComparisonOp::LessThan,
                shift_tuning.wall_form_at,
            ),
            effect: EffectTemplate::Pulse {
                amplitude: ValueSource::signal_scale(
                    "drivetrain.shift_pulse",
                    0.0,
                    1.0,
                    0.0,
                    shift_amplitude,
                ),
                frequency_hz: ValueSource::constant(shift_tuning.frequency_hz),
            },
        });
    }
}

fn shift_thump_condition(
    pedal_signal: &str,
    pedal_op: ComparisonOp,
    wall_form_at: f64,
) -> RuleCondition {
    RuleCondition::All {
        conditions: vec![
            text_condition("drivetrain.shift_event", ComparisonOp::NotEq, "none"),
            number_condition(pedal_signal, pedal_op, wall_form_at),
        ],
    }
}

fn routed_trigger_targets(route: &str) -> Vec<EffectTarget> {
    match route {
        "l2" => vec![EffectTarget::L2],
        "r2" => vec![EffectTarget::R2],
        "both_triggers" | "body_and_triggers" => vec![EffectTarget::L2, EffectTarget::R2],
        "r2_and_body" => vec![EffectTarget::R2],
        _ => Vec::new(),
    }
}

fn route_targets_trigger(route: &str, target: EffectTarget) -> bool {
    match target {
        EffectTarget::L2 => route_has_l2(route),
        EffectTarget::R2 => route_has_r2(route),
        _ => false,
    }
}

fn trigger_target_label(target: EffectTarget) -> &'static str {
    match target {
        EffectTarget::L2 => "l2",
        EffectTarget::R2 => "r2",
        _ => "other",
    }
}

fn route_has_l2(route: &str) -> bool {
    matches!(route, "l2" | "both_triggers" | "body_and_triggers")
}

fn route_has_r2(route: &str) -> bool {
    matches!(
        route,
        "r2" | "both_triggers" | "body_and_triggers" | "r2_and_body"
    )
}

pub(crate) fn route_has_body(route: &str) -> bool {
    matches!(
        route,
        "body_both" | "body_left" | "body_right" | "body_and_triggers" | "r2_and_body"
    )
}

pub(crate) fn route_body_mix(route: &str) -> (f64, f64) {
    match route {
        "body_left" => (1.0, 0.25),
        "body_right" => (0.25, 1.0),
        "body_both" | "body_and_triggers" => (1.0, 1.0),
        "r2_and_body" => (0.70, 0.70),
        _ => (0.0, 0.0),
    }
}

fn scaled_unit(value: f64, scalar: f64) -> f64 {
    clamp_unit(value * scalar)
}

fn generic_runtime_profile(
    profile_id: &str,
    profile_name: &str,
    config: Option<&ControllerConfig>,
) -> Profile {
    let trigger = config.map(|config| &config.trigger);
    let intensity = trigger.map_or(0.62, trigger_intensity_scalar);
    let mode = trigger.map_or("Adaptive resistance", |trigger| trigger.effect.as_str());
    let effect = match mode {
        "Off" => EffectTemplate::Off,
        "Pulse" => EffectTemplate::Pulse {
            amplitude: ValueSource::constant(intensity),
            frequency_hz: ValueSource::constant(36.0),
        },
        "Wall pulse" => EffectTemplate::PulseAb {
            strength: ValueSource::constant(intensity),
            frequency_hz: ValueSource::constant(36.0),
            wall_zones: ValueSource::constant(2.0),
        },
        "Wall" => EffectTemplate::Wall {
            position: ValueSource::constant(0.32),
            strength: ValueSource::constant(intensity),
        },
        _ => EffectTemplate::AdaptiveResistance {
            start_position: ValueSource::constant(0.16),
            strength: ValueSource::constant(intensity),
        },
    };
    let mut rules = vec![
        EffectRule {
            id: "generic-l2-preview".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: effect.clone(),
        },
        EffectRule {
            id: "generic-r2-preview".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect,
        },
    ];
    rules.extend(lightbar_rules(config.map(|config| &config.lightbar)));

    Profile {
        id: profile_id.to_string(),
        name: profile_name.to_string(),
        version: 1,
        rumble_policy: RumblePolicy::TriggerOverlay,
        rules,
    }
}

fn lightbar_rules(config: Option<&LightbarConfig>) -> Vec<EffectRule> {
    let config = config.cloned().unwrap_or_default().normalized();
    if !config.enabled {
        return vec![EffectRule {
            id: "lightbar-disabled".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::Lightbar,
            priority: 1,
            condition: RuleCondition::Always,
            effect: EffectTemplate::Off,
        }];
    }

    vec![EffectRule {
        id: "lightbar-user-color".to_string(),
        smoothing: None,
        hysteresis: None,
        timeout: None,
        target: EffectTarget::Lightbar,
        priority: 1,
        condition: RuleCondition::Always,
        effect: EffectTemplate::Lightbar {
            color: config.rgb(),
            brightness: ValueSource::constant(f64::from(config.brightness) / 100.0),
        },
    }]
}

fn trigger_intensity_scalar(trigger: &TriggerConfig) -> f64 {
    match trigger.intensity.as_str() {
        "Off" => 0.0,
        "Weak" => 0.38,
        "Medium" => 0.62,
        "Strong (Standard)" => 0.86,
        _ => 0.62,
    }
}

pub(crate) fn trigger_vibration_scalar(trigger: Option<&TriggerConfig>) -> f64 {
    match trigger.map(|trigger| trigger.vibration.as_str()) {
        Some("Off") => 0.0,
        Some("Low") => 0.48,
        Some("High") => 1.0,
        Some("Medium") | None => 0.82,
        _ => 0.82,
    }
}

pub(crate) fn apply_vibration_mode(mode: &str, low: f64, high: f64) -> (f64, f64) {
    match mode {
        "Deep thump" | "deep_thump" => (clamp_unit(low.max(high * 0.28)), clamp_unit(high * 0.42)),
        "Fine buzz" | "fine_buzz" => (clamp_unit(low * 0.42), clamp_unit(high.max(low * 0.28))),
        _ => (clamp_unit(low), clamp_unit(high)),
    }
}

fn number_condition(signal: &str, op: ComparisonOp, value: f64) -> RuleCondition {
    RuleCondition::Signal {
        signal: signal.to_string(),
        op,
        value: ComparableValue::Number(value),
    }
}

fn text_condition(signal: &str, op: ComparisonOp, value: &str) -> RuleCondition {
    RuleCondition::Signal {
        signal: signal.to_string(),
        op,
        value: ComparableValue::Text(value.to_string()),
    }
}

pub(crate) fn effect_mapping_statuses(
    snapshot: &SignalSnapshot,
    config: Option<&ControllerConfig>,
) -> Vec<EffectMappingStatus> {
    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let brake = snapshot.number("input.brake").unwrap_or_default();
    let throttle = snapshot.number("input.throttle").unwrap_or_default();
    let speed_kmh = snapshot.number("vehicle.speed_kmh").unwrap_or_default();
    let moving = speed_kmh > 3.0;
    let slip = snapshot.number("wheel.slip.max").unwrap_or_default();
    let front_slip = snapshot.number("wheel.slip.front_max").unwrap_or_default();
    let tire_slip = snapshot.number("tire.slip_ratio.max").unwrap_or_default();
    let abs_tuning = forza.abs.clone().normalized();
    let abs_signal = match abs_tuning.slip_source.as_str() {
        "front" => front_slip,
        "tire" => tire_slip,
        "wheel" => slip,
        _ => front_slip.max(tire_slip).max(slip),
    };
    let handbrake = snapshot.number("input.handbrake").unwrap_or_default();
    let rpm_ratio = snapshot.number("vehicle.rpm_ratio").unwrap_or_default();
    let rev_tuning = forza.rev_limiter.clone().normalized();
    let shift = snapshot.text("drivetrain.shift_event").unwrap_or("none");
    let rumble_strip = snapshot
        .number("surface.rumble_strip.max")
        .unwrap_or_default();
    let puddle = snapshot.number("surface.puddle.max").unwrap_or_default();
    let suspension_impact = snapshot
        .number("suspension.impact_pulse")
        .unwrap_or_default();
    vec![
        mapping_status(
            "brake_resistance",
            "L2",
            "Brake resistance",
            "input.brake",
            brake > 0.02,
            &forza,
        ),
        mapping_status(
            "abs_slip_pulse",
            "L2",
            "ABS / tire slip pulse",
            "wheel.slip.max",
            brake >= abs_tuning.brake_threshold_ratio
                && speed_kmh >= abs_tuning.min_speed_kmh
                && abs_signal >= abs_tuning.slip_threshold,
            &forza,
        ),
        mapping_status(
            "handbrake_wall",
            "L2",
            "Handbrake resistance",
            "input.handbrake",
            handbrake > 0.05,
            &forza,
        ),
        mapping_status(
            "throttle_resistance",
            "R2",
            "Throttle resistance",
            "input.throttle",
            throttle > 0.02,
            &forza,
        ),
        mapping_status(
            "gear_shift_thump",
            "R2",
            "Gear shift thump",
            "drivetrain.shift_event",
            shift != "none",
            &forza,
        ),
        mapping_status(
            "rev_limiter_buzz",
            "R2",
            "Rev limiter buzz",
            "vehicle.rpm_ratio",
            rpm_ratio >= rev_tuning.threshold_ratio,
            &forza,
        ),
        mapping_status(
            "road_texture",
            "HD",
            "Road texture rumble",
            "surface.rumble.max",
            moving && snapshot.number("surface.rumble.max").unwrap_or_default() > 0.08,
            &forza,
        ),
        mapping_status(
            "rumble_strip",
            "HD",
            "Rumble strip pulse",
            "surface.rumble_strip.max",
            moving && rumble_strip > 0.0,
            &forza,
        ),
        mapping_status(
            "tire_slip",
            "HD",
            "Tire slip rumble",
            "wheel.slip.max",
            moving && slip > 0.20,
            &forza,
        ),
        mapping_status(
            "puddle_drag",
            "HD",
            "Puddle drag",
            "surface.puddle.max",
            moving && puddle > 0.08,
            &forza,
        ),
        mapping_status(
            "suspension_impact",
            "HD",
            "Suspension / impact thump",
            "suspension.impact_pulse",
            moving && suspension_impact > 0.05,
            &forza,
        ),
        mapping_status(
            "rpm_leds",
            "LED",
            "Redline ramp",
            "vehicle.rpm_ratio",
            rpm_ratio >= rev_tuning.threshold_ratio,
            &forza,
        ),
    ]
}

fn mapping_status(
    id: &str,
    target: &str,
    label: &str,
    signal: &str,
    active: bool,
    forza: &ForzaTelemetryConfig,
) -> EffectMappingStatus {
    let enabled = forza.effect(id).enabled;
    EffectMappingStatus {
        id: id.to_string(),
        target: target.to_string(),
        label: label.to_string(),
        signal: signal.to_string(),
        state: if !enabled {
            "disabled"
        } else if active {
            "active"
        } else {
            "ready"
        }
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized_tuning() -> (
        ForzaBrakeTuningConfig,
        ForzaThrottleTuningConfig,
        ForzaAbsTuningConfig,
    ) {
        (
            ForzaBrakeTuningConfig::default().normalized(),
            ForzaThrottleTuningConfig::default().normalized(),
            ForzaAbsTuningConfig::default().normalized(),
        )
    }

    fn trigger_with_range(l2_from: u8, l2_to: u8, r2_from: u8, r2_to: u8) -> TriggerConfig {
        TriggerConfig {
            l2_from,
            l2_to,
            r2_from,
            r2_to,
            ..TriggerConfig::default()
        }
    }

    #[test]
    fn compute_trigger_positions_defaults_when_trigger_none() {
        let (brake, throttle, abs) = normalized_tuning();
        let pos = compute_trigger_positions(None, &brake, &throttle, &abs);
        assert_eq!(pos.l2_start, 0.18);
        assert_eq!(pos.r2_start, 0.10);
        assert_eq!(pos.l2_end, FORZA_BRAKE_FULL_FORCE_AT);
        assert_eq!(pos.r2_end, FORZA_THROTTLE_FULL_FORCE_AT);
    }

    #[test]
    fn compute_trigger_positions_keeps_derived_points_within_range() {
        let trigger = trigger_with_range(20, 80, 10, 90);
        let (brake, throttle, abs) = normalized_tuning();

        let pos = compute_trigger_positions(Some(&trigger), &brake, &throttle, &abs);

        // L2 (brake) geometry stays ordered inside the configured travel range.
        assert!(pos.l2_start <= pos.l2_force_knee);
        assert!(pos.l2_force_knee <= pos.l2_end);
        assert!(pos.l2_final_stop_position >= pos.l2_start);
        assert!(pos.l2_final_stop_position <= pos.l2_end);
        assert!(pos.l2_normal_end >= pos.l2_start + 0.01);
        assert!(pos.abs_brake_threshold >= pos.l2_start);
        assert!(pos.abs_brake_threshold <= pos.l2_end);

        // R2 (throttle) geometry likewise.
        assert!(pos.r2_start <= pos.r2_overtravel_ramp_start);
        assert!(pos.r2_overtravel_ramp_start <= pos.r2_endstop_wall);
        assert!(pos.r2_endstop_wall <= pos.r2_end);
        assert!(pos.r2_normal_end >= pos.r2_start + 0.01);
    }

    #[test]
    fn compute_trigger_positions_overtravel_guard_tracks_guard_min_end() {
        let trigger = trigger_with_range(20, 80, 10, 90);
        let (brake, throttle, abs) = normalized_tuning();

        let pos = compute_trigger_positions(Some(&trigger), &brake, &throttle, &abs);

        assert_eq!(
            pos.l2_has_overtravel_guard,
            pos.l2_end >= brake.guard_min_end.clamp(0.0, 1.0)
        );
        assert_eq!(
            pos.r2_has_overtravel_guard,
            pos.r2_end >= throttle.guard_min_end.clamp(0.0, 1.0)
        );
    }
}
