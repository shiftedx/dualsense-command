use super::*;

pub(crate) fn update_number(updates: &[SignalUpdate], name: &str) -> Option<f64> {
    updates
        .iter()
        .find(|update| update.name.as_str() == name)
        .and_then(|update| update.value.as_number())
}

pub(crate) fn update_text<'a>(updates: &'a [SignalUpdate], name: &str) -> Option<&'a str> {
    updates
        .iter()
        .find(|update| update.name.as_str() == name)
        .and_then(|update| update.value.as_text())
}

pub(crate) fn sequenced_signal_update(
    name: &str,
    value: impl Into<SignalValue>,
    sequence: u64,
) -> SignalUpdate {
    signal_update(name, value).with_sequence(sequence)
}

pub(crate) fn racing_shift_adapter(adapter_id: &str) -> bool {
    matches!(
        adapter_id,
        FORZA_DATA_OUT_ADAPTER_ID | ASSETTO_SHARED_MEMORY_ADAPTER_ID
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RacingEffectToggles {
    pub(crate) shift_thump: bool,
    pub(crate) suspension_impact: bool,
}

pub(crate) fn racing_effect_toggles(inner: &AgentStateInner) -> RacingEffectToggles {
    let mut toggles = RacingEffectToggles {
        shift_thump: false,
        suspension_impact: false,
    };
    let mut saw_connected = false;
    for controller in inner
        .controllers
        .summaries()
        .into_iter()
        .filter(|controller| controller.connected)
    {
        saw_connected = true;
        let default_config;
        let config = match inner.controller_configs.get(&controller.id) {
            Some(config) => config,
            None => {
                default_config = ControllerConfig::default_for(&controller.id, controller.model);
                &default_config
            }
        };
        toggles.shift_thump |= forza_effect_enabled(config, "gear_shift_thump");
        toggles.suspension_impact |= forza_effect_enabled(config, "suspension_impact");
        if toggles.shift_thump && toggles.suspension_impact {
            break;
        }
    }
    if !saw_connected {
        return RacingEffectToggles {
            shift_thump: true,
            suspension_impact: true,
        };
    }
    toggles
}

fn forza_effect_enabled(config: &ControllerConfig, effect_id: &str) -> bool {
    let default = default_forza_effect(effect_id);
    config
        .forza
        .effects
        .iter()
        .find(|effect| effect.id == effect_id)
        .cloned()
        .unwrap_or_else(|| default.clone())
        .normalized_with_default(&default)
        .scalar()
        > 0.0
}

fn telemetry_response(snapshot: &SignalSnapshot) -> Vec<TelemetrySignalResponse> {
    snapshot
        .signals()
        .iter()
        .map(|(name, value)| TelemetrySignalResponse {
            name: name.as_str().to_string(),
            value: signal_value_json(value),
            unit: signal_unit(name.as_str()).map(str::to_string),
            updated_ms_ago: 0,
        })
        .collect()
}

pub(crate) fn materialized_telemetry_response(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> Vec<TelemetrySignalResponse> {
    let now = Instant::now();
    if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
        let source_id = inner.telemetry.text("source.id");
        let Some(runtime) = inner.adapter_runtime(adapter_id) else {
            return telemetry_response(&inner.telemetry);
        };
        if source_id != Some(adapter_id) || !runtime.has_recent_packet(now) {
            return waiting_telemetry_response(runtime, adapter_id, game_id, game_name, now);
        }
        let mut response = telemetry_response(&inner.telemetry);
        upsert_telemetry_signal(&mut response, telemetry_signal("game.id", game_id, None, 0));
        upsert_telemetry_signal(
            &mut response,
            telemetry_signal("game.name", game_name, None, 0),
        );
        return response;
    }

    telemetry_response(&inner.telemetry)
}

fn detected_telemetry_game(
    game_detection: Option<&GameDetectionResponse>,
) -> Option<(&str, &str, &str)> {
    let detection = game_detection?;
    let adapter_id = detection.adapter_id.as_deref()?;
    let game_id = detection.active_game_id.as_deref()?;
    Some((
        adapter_id,
        game_id,
        detection.active_game_name.as_deref().unwrap_or(game_id),
    ))
}

pub(crate) fn hardware_output_runtime_allowed_for_resolution(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    let Some(detection) = game_detection else {
        return false;
    };
    if detection.active_game_id.is_none() {
        return false;
    }
    let Some(adapter_id) = detection.adapter_id.as_deref() else {
        return false;
    };
    if resolution.controller_id.is_none()
        || resolution.selected_profile_id.is_none()
        || resolution.validation != "valid"
    {
        return false;
    }
    let Some(runtime) = inner.adapter_runtime(adapter_id) else {
        return false;
    };
    runtime.has_recent_packet(Instant::now())
        && inner.telemetry.text("source.id") == Some(adapter_id)
}

pub(crate) fn hardware_output_detection_lightbar_allowed_for_resolution(
    _inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    let Some(detection) = game_detection else {
        return false;
    };
    if detection.active_game_id.is_none()
        || detection.adapter_id.is_none()
        || detection.profile_id.is_none()
    {
        return false;
    }
    resolution.controller_id.is_some()
        && resolution.selected_profile_id.is_some()
        && resolution.validation == "valid"
        && detection_game_module(detection).is_some()
}

pub(crate) fn hardware_output_global_lightbar_allowed_for_resolution(
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> bool {
    if game_detection.is_some_and(|detection| detection.profile_id.is_some()) {
        return false;
    }

    resolution.controller_id.is_some() && resolution.validation == "valid"
}

pub(crate) fn hardware_output_any_allowed(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> bool {
    let resolution = profile_resolution(inner, game_detection);
    hardware_output_runtime_allowed_for_resolution(inner, game_detection, &resolution)
        || hardware_output_detection_lightbar_allowed_for_resolution(
            inner,
            game_detection,
            &resolution,
        )
        || (hardware_output_global_lightbar_allowed_for_resolution(game_detection, &resolution)
            && global_lightbar_output(inner, &resolution).is_some())
}

fn detection_game_module(detection: &GameDetectionResponse) -> Option<&'static GameModule> {
    let module_id = detection.module_id.as_deref()?;
    built_in_game_modules()
        .iter()
        .find(|game| game.id == module_id)
}

pub(crate) fn detection_lightbar_output(
    detection: &GameDetectionResponse,
) -> Option<LightbarOutput> {
    let game = detection_game_module(detection)?;
    let color = rgb_from_hex(game.detection_lightbar_color)?;
    Some(LightbarOutput {
        color,
        brightness: clamp_unit(f64::from(game.detection_lightbar_brightness.min(100)) / 100.0),
    })
}

pub(crate) fn global_lightbar_output(
    inner: &AgentStateInner,
    resolution: &ProfileResolutionResponse,
) -> Option<ControllerOutputFrame> {
    let config = controller_config_for_resolution(inner, resolution)?;
    let lightbar = config.lightbar.normalized();
    let lightbar = lightbar.enabled.then(|| LightbarOutput {
        color: lightbar.rgb(),
        brightness: clamp_unit(f64::from(lightbar.brightness) / 100.0),
    });
    Some(ControllerOutputFrame {
        lightbar,
        ..ControllerOutputFrame::default()
    })
}

fn upsert_telemetry_signal(
    signals: &mut Vec<TelemetrySignalResponse>,
    signal: TelemetrySignalResponse,
) {
    if let Some(existing) = signals.iter_mut().find(|item| item.name == signal.name) {
        *existing = signal;
    } else {
        signals.push(signal);
    }
}

fn waiting_telemetry_response(
    runtime: &AdapterRuntime,
    adapter_id: &str,
    game_id: &str,
    game_name: &str,
    now: Instant,
) -> Vec<TelemetrySignalResponse> {
    let age_ms = runtime
        .last_packet_at
        .map(|last| {
            now.duration_since(last)
                .as_millis()
                .min(u128::from(u64::MAX)) as u64
        })
        .unwrap_or_default();
    vec![
        telemetry_signal("source.id", adapter_id, None, 0),
        telemetry_signal("source.connected", runtime.has_recent_packet(now), None, 0),
        telemetry_signal(
            "source.packet_rate_hz",
            if runtime.has_recent_packet(now) {
                f64::from(runtime.packet_rate_hz.unwrap_or_default())
            } else {
                0.0
            },
            Some("Hz"),
            age_ms,
        ),
        telemetry_signal(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
            Some("bytes"),
            age_ms,
        ),
        telemetry_signal("game.id", game_id, None, 0),
        telemetry_signal("game.name", game_name, None, 0),
        telemetry_signal(
            "game.state",
            if runtime.packet_count > 0 {
                "telemetry_stale"
            } else if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "awaiting_shared_memory"
            } else {
                "awaiting_data_out"
            },
            None,
            age_ms,
        ),
        telemetry_signal("input.throttle", 0.0, None, age_ms),
        telemetry_signal("input.brake", 0.0, None, age_ms),
        telemetry_signal("input.handbrake", 0.0, None, age_ms),
        telemetry_signal("vehicle.rpm_ratio", 0.0, None, age_ms),
        telemetry_signal("vehicle.speed_kmh", 0.0, Some("km/h"), age_ms),
        telemetry_signal("wheel.slip.max", 0.0, None, age_ms),
        telemetry_signal("wheel.slip.front_max", 0.0, None, age_ms),
        telemetry_signal("wheel.slip.rear_max", 0.0, None, age_ms),
        telemetry_signal("tire.slip_ratio.max", 0.0, None, age_ms),
        telemetry_signal("surface.rumble.max", 0.0, None, age_ms),
        telemetry_signal("surface.rumble_strip.max", 0.0, None, age_ms),
        telemetry_signal("surface.puddle.max", 0.0, None, age_ms),
        telemetry_signal("suspension.travel.max", 0.0, None, age_ms),
        telemetry_signal("suspension.impact_pulse", 0.0, None, age_ms),
        telemetry_signal("vehicle.acceleration.magnitude", 0.0, Some("m/s^2"), age_ms),
        telemetry_signal("drivetrain.shift_event", "none", None, age_ms),
        telemetry_signal("drivetrain.shift_pulse", 0.0, None, age_ms),
    ]
}

fn waiting_signal_snapshot(
    runtime: &AdapterRuntime,
    adapter_id: &str,
    game_id: &str,
    game_name: &str,
    now: Instant,
) -> SignalSnapshot {
    SignalSnapshot::from_updates([
        signal_update("source.id", adapter_id),
        signal_update("source.connected", runtime.has_recent_packet(now)),
        signal_update("source.packet_rate_hz", 0.0),
        signal_update(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
        ),
        signal_update("game.id", game_id),
        signal_update("game.name", game_name),
        signal_update(
            "game.state",
            if runtime.packet_count > 0 {
                "telemetry_stale"
            } else if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "awaiting_shared_memory"
            } else {
                "awaiting_data_out"
            },
        ),
        signal_update("input.throttle", 0.0),
        signal_update("input.brake", 0.0),
        signal_update("input.handbrake", 0.0),
        signal_update("vehicle.rpm_ratio", 0.0),
        signal_update("vehicle.speed_kmh", 0.0),
        signal_update("wheel.slip.max", 0.0),
        signal_update("wheel.slip.front_max", 0.0),
        signal_update("wheel.slip.rear_max", 0.0),
        signal_update("tire.slip_ratio.max", 0.0),
        signal_update("surface.rumble.max", 0.0),
        signal_update("surface.rumble_strip.max", 0.0),
        signal_update("surface.puddle.max", 0.0),
        signal_update("suspension.travel.max", 0.0),
        signal_update("suspension.impact_pulse", 0.0),
        signal_update("vehicle.acceleration.magnitude", 0.0),
        signal_update("drivetrain.shift_event", "none"),
        signal_update("drivetrain.shift_pulse", 0.0),
    ])
}

fn forza_inactive_signal_snapshot(
    runtime: &AdapterRuntime,
    now: Instant,
    game_id: Option<&str>,
    game_name: Option<&str>,
) -> SignalSnapshot {
    let mut updates = vec![
        signal_update("source.id", "none"),
        signal_update("source.connected", false),
        signal_update("source.packet_rate_hz", 0.0),
        signal_update(
            "source.packet_size",
            runtime.last_packet_len.unwrap_or_default() as f64,
        ),
        signal_update(
            "source.packet_age_ms",
            runtime
                .last_packet_at
                .map(|last| {
                    now.duration_since(last)
                        .as_millis()
                        .min(u128::from(u64::MAX)) as f64
                })
                .unwrap_or_default(),
        ),
    ];
    if let Some(game_id) = game_id {
        updates.push(signal_update("game.id", game_id));
    }
    if let Some(game_name) = game_name {
        updates.push(signal_update("game.name", game_name));
    }
    SignalSnapshot::from_updates(updates)
}

pub(crate) fn current_effect_snapshot(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> (SignalSnapshot, bool) {
    let now = Instant::now();
    if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
        let source_id = inner.telemetry.text("source.id");
        let Some(runtime) = inner.adapter_runtime(adapter_id) else {
            return (inner.telemetry.clone(), false);
        };
        if source_id != Some(adapter_id) || !runtime.has_recent_packet(now) {
            return (
                waiting_signal_snapshot(runtime, adapter_id, game_id, game_name, now),
                false,
            );
        }

        let mut snapshot = inner.telemetry.clone();
        if let Some(shift_event) = inner.forza_effect_runtime.latched_shift_event(now) {
            snapshot.apply_update(signal_update("drivetrain.shift_event", shift_event));
            snapshot.apply_update(signal_update("drivetrain.shift_pulse", 1.0));
        } else {
            snapshot.apply_update(signal_update("drivetrain.shift_event", "none"));
            snapshot.apply_update(signal_update("drivetrain.shift_pulse", 0.0));
        }
        snapshot.apply_update(signal_update(
            "suspension.impact_pulse",
            inner.forza_effect_runtime.latched_suspension_impact(now),
        ));
        return (snapshot, true);
    }

    if let Some(source_id) = inner.telemetry.text("source.id") {
        if let Some(runtime) = inner
            .adapter_runtime(source_id)
            .filter(|runtime| !runtime.has_recent_packet(now))
        {
            return (
                forza_inactive_signal_snapshot(
                    runtime,
                    now,
                    inner.telemetry.text("game.id"),
                    inner.telemetry.text("game.name"),
                ),
                false,
            );
        }
    }

    (inner.telemetry.clone(), true)
}

fn telemetry_signal(
    name: &str,
    value: impl Serialize,
    unit: Option<&str>,
    updated_ms_ago: u64,
) -> TelemetrySignalResponse {
    TelemetrySignalResponse {
        name: name.to_string(),
        value: serde_json::to_value(value).expect("telemetry signal value is serializable"),
        unit: unit.map(str::to_string),
        updated_ms_ago,
    }
}

pub(crate) fn signal_update(name: &str, value: impl Into<SignalValue>) -> SignalUpdate {
    SignalUpdate::new(
        SignalName::new(name).expect("internal telemetry signal name is valid"),
        value,
    )
}

fn signal_value_json(value: &SignalValue) -> serde_json::Value {
    match value {
        SignalValue::Number(value) => serde_json::json!(value),
        SignalValue::Bool(value) => serde_json::json!(value),
        SignalValue::Text(value) => serde_json::json!(value),
    }
}

fn signal_unit(name: &str) -> Option<&'static str> {
    match name {
        "vehicle.speed_kmh" => Some("km/h"),
        "vehicle.rpm" | "vehicle.max_rpm" => Some("rpm"),
        "vehicle.acceleration.x"
        | "vehicle.acceleration.y"
        | "vehicle.acceleration.z"
        | "vehicle.acceleration.magnitude" => Some("m/s^2"),
        "source.packet_rate_hz" => Some("Hz"),
        "source.packet_size" => Some("bytes"),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn current_effect_response(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    hardware_output_enabled: bool,
) -> CurrentEffectResponse {
    let resolution = profile_resolution(inner, game_detection);
    let config = controller_config_for_resolution(inner, &resolution);
    let (snapshot, telemetry_live) = current_effect_snapshot(inner, game_detection);
    let profile_id = resolution
        .selected_profile_id
        .clone()
        .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
    let profile_name = profile_name_by_id(inner, &profile_id).unwrap_or_else(|| profile_id.clone());
    let profile = runtime_profile_for(&profile_id, &profile_name, config.as_ref(), &snapshot);
    let mut output = EffectEngine::new().evaluate(&profile, &snapshot);
    apply_runtime_output_enhancements(
        &profile_id,
        config.as_ref(),
        &snapshot,
        telemetry_live,
        &mut output,
    );
    apply_detection_lightbar_preview(game_detection, telemetry_live, &mut output);
    current_effect_response_from_parts(
        resolution,
        profile,
        config.as_ref(),
        snapshot,
        telemetry_live,
        output,
        game_detection,
        hardware_output_enabled,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn current_effect_response_from_parts(
    resolution: ProfileResolutionResponse,
    profile: Profile,
    config: Option<&ControllerConfig>,
    snapshot: SignalSnapshot,
    telemetry_live: bool,
    output: ControllerOutputFrame,
    game_detection: Option<&GameDetectionResponse>,
    hardware_output_enabled: bool,
) -> CurrentEffectResponse {
    let mut warnings = Vec::new();

    if hardware_output_enabled {
        warnings.push(
            "Hardware output is enabled. DSCC keeps trigger and rumble output neutral until supported-game telemetry is live or during a manual effect test; idle lightbar follows the Global profile."
                .to_string(),
        );
    } else {
        warnings.push(
                "Hardware output is disabled; this frame is the validated target state, not a raw hardware write."
                .to_string(),
        );
    }
    if resolution
        .controller_id
        .as_deref()
        .is_some_and(is_windows_pnp_controller_id)
    {
        warnings.push(
            "Windows currently exposes this Edge only through the PnP fallback, so live battery and lightbar writes require the Sony HID interface to become visible to DSCC."
                .to_string(),
        );
    }
    if is_forza_runtime_profile(&profile.id, &snapshot) && !telemetry_live {
        if let Some((adapter_id, game_id, game_name)) = detected_telemetry_game(game_detection) {
            let source_label = if adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
                "shared-memory telemetry"
            } else {
                "Data Out telemetry"
            };
            warnings.push(
                format!(
                    "{game_name} ({game_id}) is detected, but {source_label} is not live; trigger output stays neutral until fresh telemetry arrives."
                ),
            );
        } else {
            warnings.push(
                "Racing telemetry is stale and no supported process is detected; trigger output is neutral."
                    .to_string(),
            );
        }
    }

    CurrentEffectResponse {
        controller_id: resolution.controller_id,
        selected_profile_id: Some(profile.id),
        selected_profile_name: Some(profile.name),
        reason: resolution.reason,
        dry_run: !hardware_output_enabled,
        hardware_output_enabled,
        output,
        parity_effects: effect_mapping_statuses(&snapshot, config),
        warnings,
    }
}

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
        output.player_leds = None;
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
        let rpm_leds = forza.effect("rpm_leds");
        let rpm_led_scalar = if rpm_leds.route == "light_led" {
            rpm_leds.scalar()
        } else {
            0.0
        };
        output.lightbar = Some(forza_lightbar_output(config, snapshot, rpm_led_scalar));
        output.player_leds = if rpm_led_scalar > 0.0 {
            Some(PlayerLedsOutput {
                count: forza_gear_player_led_count(snapshot),
            })
        } else {
            None
        };
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
    let rev_limiter = signal_scaled(snapshot, "vehicle.rpm_ratio", 0.93, 1.0);
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
    let drivetrain = (rpm * rpm * (0.35 + throttle * 0.65)).clamp(0.0, 1.0);

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
        0.92,
        0.84,
    );
    if !native_passthrough {
        add_forza_rumble_component(
            &mut low,
            &mut high,
            &forza.effect("rev_limiter_buzz"),
            rev_limiter,
            0.20,
            0.80,
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

pub(crate) fn forza_lightbar_output(
    config: Option<&ControllerConfig>,
    snapshot: &SignalSnapshot,
    rpm_led_scalar: f64,
) -> LightbarOutput {
    let configured = config
        .map(|config| config.lightbar.clone().normalized())
        .unwrap_or_default();
    let rpm = signal_unit_value(snapshot, "vehicle.rpm_ratio");
    let base = configured.rgb();
    let redline = configured.rpm_rgb();
    let rpm_blend = clamp_unit(rpm * rpm_led_scalar);
    let color = blend_rgb(base, redline, rpm_blend);
    let brightness =
        clamp_unit(f64::from(configured.brightness) / 100.0 + rpm * 0.12 * rpm_led_scalar);

    LightbarOutput { color, brightness }
}

fn blend_rgb(from: RgbColor, to: RgbColor, amount: f64) -> RgbColor {
    fn blend_channel(from: u8, to: u8, amount: f64) -> u8 {
        (f64::from(from) + (f64::from(to) - f64::from(from)) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    }

    let amount = clamp_unit(amount);
    RgbColor {
        red: blend_channel(from.red, to.red, amount),
        green: blend_channel(from.green, to.green, amount),
        blue: blend_channel(from.blue, to.blue, amount),
    }
}

pub(crate) fn forza_gear_player_led_count(snapshot: &SignalSnapshot) -> u8 {
    snapshot
        .number("drivetrain.gear")
        .and_then(signal_gear_to_u8)
        .unwrap_or_default()
        .clamp(0, 5)
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

fn is_forza_runtime_profile(profile_id: &str, snapshot: &SignalSnapshot) -> bool {
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

    let l2_start = trigger.map_or(0.18, |trigger| f64::from(trigger.l2_from.min(100)) / 100.0);
    let r2_start = trigger.map_or(0.10, |trigger| f64::from(trigger.r2_from.min(100)) / 100.0);
    let l2_end = trigger.map_or(FORZA_BRAKE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.l2_from, trigger.l2_to)
    });
    let r2_end = trigger.map_or(FORZA_THROTTLE_FULL_FORCE_AT, |trigger| {
        trigger_range_end_position(trigger.r2_from, trigger.r2_to)
    });
    let l2_has_overtravel_guard = brake_overtravel_guard_active(l2_end);
    let l2_endstop_wall = brake_overtravel_wall_position(l2_start, l2_end);
    let l2_overtravel_ramp_start = brake_overtravel_ramp_start(l2_start, l2_endstop_wall);
    let r2_has_overtravel_guard = throttle_overtravel_guard_active(r2_end);
    let r2_endstop_wall = throttle_overtravel_wall_position(r2_start, r2_end);
    let r2_overtravel_ramp_start = throttle_overtravel_ramp_start(r2_start, r2_endstop_wall);
    let l2_normal_end = if l2_has_overtravel_guard && l2_overtravel_ramp_start < l2_endstop_wall {
        l2_overtravel_ramp_start
    } else {
        l2_endstop_wall
    }
    .max(l2_start + 0.01);
    let r2_normal_end = if r2_has_overtravel_guard && r2_overtravel_ramp_start < r2_endstop_wall {
        r2_overtravel_ramp_start
    } else {
        r2_endstop_wall
    }
    .max(r2_start + 0.01);
    let abs_brake_threshold = abs_brake_threshold_for_range(l2_start, l2_end);
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
        scaled_unit(FORZA_BRAKE_BASELINE_FORCE, brake.scalar() * trigger_scalar);
    let brake_normal_force = scaled_unit(FORZA_BRAKE_NORMAL_FORCE, brake.scalar() * trigger_scalar);
    let brake_endstop_force = scaled_unit(
        FORZA_BRAKE_ENDSTOP_FORCE,
        brake.scalar() * trigger_scalar * FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
    );
    let throttle_baseline_force = scaled_unit(
        FORZA_THROTTLE_BASELINE_FORCE,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_normal_force = scaled_unit(
        FORZA_THROTTLE_NORMAL_FORCE,
        throttle.scalar() * trigger_scalar,
    );
    let throttle_endstop_scalar =
        throttle.scalar() * trigger_scalar * FORZA_THROTTLE_ENDSTOP_FORCE_BOOST;
    let throttle_endstop_force = scaled_unit(FORZA_THROTTLE_ENDSTOP_FORCE, throttle_endstop_scalar);
    let abs_amplitude = scaled_unit(FORZA_ABS_PULSE_AMPLITUDE, abs.scalar());
    let rev_amplitude = scaled_unit(
        FORZA_REV_LIMITER_PULSE_AMPLITUDE,
        rev.scalar() * trigger_scalar,
    );
    let shift_amplitude = scaled_unit(1.0, shift.scalar());

    let baseline_condition = forza_baseline_trigger_condition();
    let mut rules = Vec::new();

    if abs.scalar() > 0.0 && route_has_l2(&abs.route) {
        rules.push(EffectRule {
            id: "forza-l2-abs-slip-pulse".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 60,
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
                        FORZA_ABS_MIN_SPEED_KMH,
                    ),
                    RuleCondition::Any {
                        conditions: vec![
                            number_condition(
                                "tire.slip_ratio.max",
                                ComparisonOp::GreaterOrEqual,
                                FORZA_ABS_SLIP_THRESHOLD,
                            ),
                            number_condition(
                                "wheel.slip.max",
                                ComparisonOp::GreaterOrEqual,
                                FORZA_ABS_SLIP_THRESHOLD,
                            ),
                        ],
                    },
                ],
            },
            effect: EffectTemplate::Pulse {
                amplitude: ValueSource::constant(abs_amplitude),
                frequency_hz: ValueSource::constant(FORZA_ABS_PULSE_FREQUENCY_HZ),
            },
        });
    }

    if handbrake.scalar() > 0.0 && route_has_l2(&handbrake.route) {
        rules.push(EffectRule {
            id: "forza-l2-handbrake-wall".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::L2,
            priority: 45,
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
            priority: 12,
            condition: number_condition(
                "input.brake",
                ComparisonOp::GreaterOrEqual,
                l2_endstop_wall,
            ),
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(l2_endstop_wall),
                strength: ValueSource::constant(brake_endstop_force),
            },
        });
        if l2_has_overtravel_guard && l2_overtravel_ramp_start < l2_endstop_wall {
            rules.push(EffectRule {
                id: "forza-l2-brake-overtravel-ramp".to_string(),
                smoothing: None,
                hysteresis: None,
                timeout: None,
                target: EffectTarget::L2,
                priority: 11,
                condition: number_condition(
                    "input.brake",
                    ComparisonOp::GreaterOrEqual,
                    l2_overtravel_ramp_start,
                ),
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(l2_overtravel_ramp_start),
                    strength: ValueSource::signal_curve(
                        "input.brake",
                        l2_overtravel_ramp_start,
                        l2_endstop_wall,
                        brake_normal_force,
                        brake_endstop_force,
                        FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
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
            priority: 10,
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
        "forza-rev-limiter-buzz",
        55,
        number_condition(
            "vehicle.rpm_ratio",
            ComparisonOp::GreaterOrEqual,
            FORZA_REV_LIMIT_RATIO,
        ),
        ValueSource::constant(rev_amplitude),
        ValueSource::constant(FORZA_REV_LIMITER_FREQUENCY_HZ),
    );
    push_shift_thump_rules(&mut rules, &shift, shift_amplitude);

    if throttle.scalar() > 0.0 && route_has_r2(&throttle.route) {
        rules.push(EffectRule {
            id: "forza-r2-throttle-full-force".to_string(),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target: EffectTarget::R2,
            priority: 12,
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
                priority: 11,
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
                        FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE,
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
            priority: 10,
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

fn push_rev_limiter_rules(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    id: &str,
    priority: i32,
    condition: RuleCondition,
    amplitude: ValueSource,
    frequency_hz: ValueSource,
) {
    if tuning.scalar() <= 0.0 {
        return;
    }

    for target in routed_trigger_targets(&tuning.route) {
        let target_label = trigger_target_label(target);
        rules.push(EffectRule {
            id: format!("{id}-{target_label}-wall-form"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority,
            condition: RuleCondition::All {
                conditions: vec![
                    condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::GreaterOrEqual,
                        FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT,
                    ),
                ],
            },
            effect: EffectTemplate::PulseAb {
                strength: amplitude.clone(),
                frequency_hz: frequency_hz.clone(),
                wall_zones: ValueSource::constant(FORZA_REV_LIMITER_WALL_ZONES),
            },
        });
        rules.push(EffectRule {
            id: format!("{id}-{target_label}-pulse"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority,
            condition: RuleCondition::All {
                conditions: vec![
                    condition.clone(),
                    number_condition(
                        "input.throttle",
                        ComparisonOp::LessThan,
                        FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT,
                    ),
                ],
            },
            effect: EffectTemplate::Pulse {
                amplitude: amplitude.clone(),
                frequency_hz: frequency_hz.clone(),
            },
        });
    }
}

fn push_shift_thump_rules(
    rules: &mut Vec<EffectRule>,
    tuning: &ForzaEffectConfig,
    shift_amplitude: f64,
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
            priority: 70,
            condition: shift_thump_condition(pedal_signal, ComparisonOp::GreaterOrEqual),
            effect: EffectTemplate::PulseAb {
                strength: ValueSource::constant(shift_amplitude),
                frequency_hz: ValueSource::constant(FORZA_SHIFT_FREQUENCY_HZ),
                wall_zones: ValueSource::constant(FORZA_SHIFT_WALL_ZONES),
            },
        });
        rules.push(EffectRule {
            id: format!("forza-gear-shift-thump-{target_label}-pulse"),
            smoothing: None,
            hysteresis: None,
            timeout: None,
            target,
            priority: 70,
            condition: shift_thump_condition(pedal_signal, ComparisonOp::LessThan),
            effect: EffectTemplate::Pulse {
                amplitude: ValueSource::constant(shift_amplitude),
                frequency_hz: ValueSource::constant(FORZA_SHIFT_FREQUENCY_HZ),
            },
        });
    }
}

fn shift_thump_condition(pedal_signal: &str, pedal_op: ComparisonOp) -> RuleCondition {
    RuleCondition::All {
        conditions: vec![
            text_condition("drivetrain.shift_event", ComparisonOp::NotEq, "none"),
            number_condition(pedal_signal, pedal_op, FORZA_SHIFT_WALL_FORM_AT),
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

fn route_has_body(route: &str) -> bool {
    matches!(
        route,
        "body_both" | "body_left" | "body_right" | "body_and_triggers" | "r2_and_body"
    )
}

fn route_body_mix(route: &str) -> (f64, f64) {
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

fn trigger_vibration_scalar(trigger: Option<&TriggerConfig>) -> f64 {
    match trigger.map(|trigger| trigger.vibration.as_str()) {
        Some("Off") => 0.0,
        Some("Low") => 0.48,
        Some("High") => 1.0,
        Some("Medium") | None => 0.82,
        _ => 0.82,
    }
}

fn apply_vibration_mode(mode: &str, low: f64, high: f64) -> (f64, f64) {
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

fn effect_mapping_statuses(
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
    let handbrake = snapshot.number("input.handbrake").unwrap_or_default();
    let gear = snapshot.number("drivetrain.gear").unwrap_or_default();
    let rpm_ratio = snapshot.number("vehicle.rpm_ratio").unwrap_or_default();
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
            brake > 0.10 && slip.max(front_slip) > 0.20,
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
            rpm_ratio >= 0.965,
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
            "Gear LEDs / RPM lightbar",
            "drivetrain.gear + vehicle.rpm_ratio",
            gear > 0.0 || rpm_ratio > 0.20,
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

fn trigger_range_end_position(from: u8, to: u8) -> f64 {
    let start_percent = from.min(100);
    let start = f64::from(start_percent) / 100.0;
    let end = f64::from(to.clamp(start_percent, 100)) / 100.0;
    end.max(start + 0.01)
}

fn endstop_wall_position(start: f64, end: f64) -> f64 {
    (end - FORZA_ENDSTOP_WALL_OFFSET).clamp(start, end)
}

fn brake_overtravel_guard_active(end: f64) -> bool {
    end >= FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION
}

fn brake_overtravel_wall_position(start: f64, end: f64) -> f64 {
    if brake_overtravel_guard_active(end) {
        return (end - FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET)
            .max(FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION)
            .clamp(start, end);
    }

    endstop_wall_position(start, end)
}

fn brake_overtravel_ramp_start(start: f64, wall: f64) -> f64 {
    (wall - FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH).clamp(start, wall)
}

fn throttle_overtravel_guard_active(end: f64) -> bool {
    end >= FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION
}

fn throttle_overtravel_wall_position(start: f64, end: f64) -> f64 {
    if throttle_overtravel_guard_active(end) {
        return end
            .min(FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION)
            .clamp(start, end);
    }

    endstop_wall_position(start, end)
}

fn throttle_overtravel_ramp_start(start: f64, wall: f64) -> f64 {
    let ramp_start = wall - FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH;
    ((ramp_start * 1000.0).round() / 1000.0).clamp(start, wall)
}

fn abs_brake_threshold_for_range(start: f64, end: f64) -> f64 {
    let threshold = start + (end - start) * FORZA_ABS_RANGE_START_RATIO;
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
