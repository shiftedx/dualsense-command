use super::*;

pub(crate) fn apply_runtime_output_enhancements(
    profile_id: &str,
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if is_forza_runtime_profile(profile_id, snapshot) {
        apply_forza_output_enhancements(config, snapshot, telemetry_live, output);
    }
}

pub(crate) fn apply_detection_lightbar_preview(
    game_detection: Option<&GameDetectionResponse>,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if telemetry_live {
        return;
    }
    let Some(detection) = game_detection else {
        return;
    };
    if detection.profile_id.is_none() {
        return;
    }
    if let Some(lightbar) = detection_lightbar_output(detection) {
        output.lightbar = Some(lightbar);
    }
}

pub(crate) fn apply_forza_output_enhancements(
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    telemetry_live: bool,
    output: &mut ControllerOutputFrame,
) {
    if !telemetry_live || snapshot.text("game.state") != Some("driving") {
        output.rumble = None;
        output.player_leds = Some(PlayerLedsOutput { count: 0 });
        return;
    }

    let forza = config
        .map(|config| config.forza.clone().normalized())
        .unwrap_or_default();
    let trigger = config.map(|config| &config.trigger);
    let vibration = trigger_vibration_scalar(trigger);
    if vibration <= 0.0 {
        output.rumble = None;
    } else {
        output.rumble = forza_rumble_output(
            &forza,
            snapshot,
            vibration,
            trigger.map_or("Balanced", |trigger| trigger.vibration_mode.as_str()),
        );
    }

    if config.map(|config| config.lightbar.enabled).unwrap_or(true) {
        let redline_blink = forza.effect("rpm_leds");
        let redline_blink_scalar = if redline_blink.route == "light_led" {
            redline_blink.scalar()
        } else {
            0.0
        };
        let redline = forza_redline_light_output(
            config,
            snapshot,
            redline_blink_scalar,
            forza.rev_limiter.normalized().threshold_ratio,
            forza_redline_blink_on(current_timestamp_millis()),
        );
        output.lightbar = Some(redline.lightbar);
        output.player_leds = redline.player_leds;
    } else {
        output.player_leds = Some(PlayerLedsOutput { count: 0 });
    }
}

pub(crate) fn forza_rumble_output(
    forza: &ForzaTelemetryConfig,
    snapshot: &SignalSnapshot,
    vibration: f64,
    vibration_mode: &str,
) -> Option<RumbleOutput> {
    let throttle = signal_unit_value(snapshot, "input.throttle");
    let brake = signal_unit_value(snapshot, "input.brake");
    let clutch = signal_unit_value(snapshot, "input.clutch");
    let handbrake = signal_unit_value(snapshot, "input.handbrake");
    let rpm = signal_unit_value(snapshot, "vehicle.rpm_ratio");
    let speed = signal_scaled(snapshot, "vehicle.speed_kmh", 12.0, 280.0);
    let rolling = signal_scaled(snapshot, "vehicle.speed_kmh", 3.0, 38.0);
    let rolling_texture = rolling.sqrt();
    let surface = signal_unit_value(snapshot, "surface.rumble.max");
    let strip = signal_unit_value(snapshot, "surface.rumble_strip.max");
    let puddle = signal_unit_value(snapshot, "surface.puddle.max");
    let slip = signal_scaled(snapshot, "wheel.slip.max", 0.16, 1.10);
    let front_slip = signal_scaled(snapshot, "wheel.slip.front_max", 0.14, 1.0);
    let rear_slip = signal_scaled(snapshot, "wheel.slip.rear_max", 0.14, 1.0);
    let slip_ratio = signal_scaled(snapshot, "tire.slip_ratio.max", 0.12, 1.0);
    let slip_angle = signal_scaled(snapshot, "tire.slip_angle.max", 0.22, 1.05);
    let shift = signal_unit_value(snapshot, "drivetrain.shift_pulse");
    let suspension_impact = signal_unit_value(snapshot, "suspension.impact_pulse");
    let shift_tuning = forza.shift.clone().normalized();
    let rev_tuning = forza.rev_limiter.clone().normalized();
    let rev_limiter = signal_scaled(
        snapshot,
        "vehicle.rpm_ratio",
        rev_tuning.threshold_ratio,
        1.0,
    )
    .powf(rev_tuning.curve);
    let native_passthrough = forza.body_rumble_mode == default_forza_body_rumble_mode();

    let road_texture = surface.max(strip * 0.95) * rolling_texture * (0.35 + speed * 0.65);
    let strip_feedback = strip * rolling_texture;
    let puddle_feedback = puddle * rolling_texture;
    let pedal_load = throttle.max(brake).max(handbrake);
    let steering_slip_feedback = slip_angle * (0.12 + pedal_load * 0.38);
    let tire_feedback = slip.max(slip_ratio * 0.85).max(steering_slip_feedback);
    let brake_feedback = if brake > 0.08 {
        front_slip.max(tire_feedback * brake)
    } else {
        0.0
    };
    let traction_feedback = if throttle > 0.12 {
        rear_slip.max(tire_feedback * throttle)
    } else {
        0.0
    };
    let clutch_uncouple = 1.0 - clutch * 0.78;
    let drivetrain = (rpm * rpm * (0.35 + throttle * 0.65) * clutch_uncouple).clamp(0.0, 1.0);

    let mut low = 0.0;
    let mut high = 0.0;
    if !native_passthrough {
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("road_texture"),
            road_texture,
            0.46,
            0.58,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("rumble_strip"),
            strip_feedback,
            0.26,
            0.52,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("tire_slip"),
            tire_feedback.max(brake_feedback).max(traction_feedback),
            0.16,
            0.56,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("puddle_drag"),
            puddle_feedback,
            0.34,
            0.24,
        );
    }
    add_forza_rumble_component(
        &mut low,
        &mut high,
        &forza.effect("suspension_impact"),
        suspension_impact,
        0.98,
        0.42,
    );
    add_forza_rumble_component(
        &mut low,
        &mut high,
        &forza.effect("gear_shift_thump"),
        shift,
        shift_tuning.body_low_weight,
        shift_tuning.body_high_weight,
    );
    if !native_passthrough {
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("rev_limiter_buzz"),
            rev_limiter,
            rev_tuning.body_low_weight,
            rev_tuning.body_high_weight,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("throttle_resistance"),
            drivetrain,
            0.32,
            0.12,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("brake_resistance"),
            brake,
            0.14,
            0.08,
        );
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("handbrake_wall"),
            handbrake,
            0.30,
            0.12,
        );
    }

    low = clamp_unit(low * vibration);
    high = clamp_unit(high * vibration);
    (low, high) = apply_vibration_mode(vibration_mode, low, high);

    if low < 0.025 && high < 0.025 {
        None
    } else {
        Some(RumbleOutput {
            low_frequency: clamp_unit(low),
            high_frequency: clamp_unit(high),
        })
    }
}

fn add_forza_rumble_component(
    low: &mut f64,
    high: &mut f64,
    tuning: &ForzaEffectConfig,
    value: f64,
    low_weight: f64,
    high_weight: f64,
) {
    if tuning.scalar() <= 0.0 || !route_has_body(&tuning.route) {
        return;
    }

    let (low_mix, high_mix) = route_body_mix(&tuning.route);
    let signal = clamp_unit(value) * tuning.scalar();
    *low += signal * low_weight * low_mix;
    *high += signal * high_weight * high_mix;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ForzaRedlineLightOutput {
    pub(crate) lightbar: LightbarOutput,
    pub(crate) player_leds: Option<PlayerLedsOutput>,
}

pub(crate) fn forza_lightbar_output(config: Option<&ControllerConfig>) -> LightbarOutput {
    let configured = config
        .map(|config| config.lightbar.clone().normalized())
        .unwrap_or_default();
    LightbarOutput {
        color: configured.rgb(),
        brightness: clamp_unit(f64::from(configured.brightness) / 100.0),
    }
}

pub(crate) fn forza_redline_light_output(
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    redline_blink_scalar: f64,
    redline_threshold: f64,
    blink_on: bool,
) -> ForzaRedlineLightOutput {
    let configured = config
        .map(|config| config.lightbar.clone().normalized())
        .unwrap_or_default();
    let rpm = signal_unit_value(snapshot, "vehicle.rpm_ratio");
    let mut lightbar = forza_lightbar_output(config);

    let redline_active =
        redline_blink_scalar > 0.0 && rpm >= redline_threshold.clamp(0.0, 1.0) && blink_on;
    if redline_active {
        lightbar.color = configured.rpm_rgb();
    }

    ForzaRedlineLightOutput {
        lightbar,
        player_leds: Some(PlayerLedsOutput {
            count: if redline_active {
                FORZA_REDLINE_PLAYER_LED_COUNT
            } else {
                0
            },
        }),
    }
}

pub(crate) fn forza_redline_blink_on(now_ms: u64) -> bool {
    (now_ms / FORZA_REDLINE_BLINK_HALF_PERIOD_MS) % 2 == 0
}

fn signal_unit_value(snapshot: &SignalSnapshot, name: &str) -> f64 {
    clamp_unit(snapshot.number(name).unwrap_or_default())
}

fn signal_scaled(snapshot: &SignalSnapshot, name: &str, min: f64, max: f64) -> f64 {
    if min >= max {
        return 0.0;
    }

    let value = snapshot.number(name).unwrap_or_default();
    clamp_unit((value - min) / (max - min))
}

fn signal_scaled_value(value: f64, input_min: f64, input_max: f64) -> f64 {
    if input_min >= input_max {
        return 0.0;
    }
    ((value - input_min) / (input_max - input_min)).clamp(0.0, 1.0)
}

pub(crate) fn suspension_impact_strength(
    suspension_travel: Option<f64>,
    acceleration_magnitude: Option<f64>,
    speed_kmh: Option<f64>,
) -> f64 {
    let suspension = signal_scaled_value(suspension_travel.unwrap_or_default(), 0.10, 0.30);
    let acceleration = signal_scaled_value(acceleration_magnitude.unwrap_or_default(), 18.0, 38.0);
    let speed_gate = signal_scaled_value(speed_kmh.unwrap_or_default(), 8.0, 24.0);
    let mut impact = (acceleration * 0.75 + suspension * 0.45).clamp(0.0, 1.0) * speed_gate;

    if suspension < 0.18 {
        impact *= 0.35;
    }

    clamp_unit(impact)
}

pub(crate) fn clamp_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
