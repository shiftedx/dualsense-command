use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EffectEnginePurpose {
    Preview,
    Hardware,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EffectEngineKey {
    purpose: EffectEnginePurpose,
    controller_id: String,
    profile_id: String,
    revision: u64,
}

#[derive(Debug, Default)]
pub(crate) struct EffectRuntimeCache {
    engines: BTreeMap<EffectEngineKey, EffectEngine>,
}

impl EffectRuntimeCache {
    fn evaluate(
        &mut self,
        key: EffectEngineKey,
        profile: &Profile,
        snapshot: &SignalSnapshot,
    ) -> ControllerOutputFrame {
        if self.engines.len() > 16 {
            self.engines
                .retain(|existing, _| existing.revision == key.revision);
        }
        self.engines
            .entry(key)
            .or_default()
            .evaluate(profile, snapshot)
    }
}

pub(crate) struct RuntimeLiveEffectMaterializer<'a, 'cache> {
    inner: &'a AgentStateInner,
    game_detection: Option<&'a GameDetectionResponse>,
    purpose: EffectEnginePurpose,
    cache: &'cache mut EffectRuntimeCache,
}

struct MaterializedRuntimeLiveEffect {
    resolution: ProfileResolutionResponse,
    profile: Profile,
    config: Option<ControllerConfig>,
    snapshot: SignalSnapshot,
    telemetry_live: bool,
    output: ControllerOutputFrame,
}

enum HardwareOutputFramePlan {
    RuntimeLiveEffect,
    DetectionLightbar,
    GlobalLightbar,
    Suppressed,
}

impl<'a, 'cache> RuntimeLiveEffectMaterializer<'a, 'cache> {
    pub(crate) fn new(
        inner: &'a AgentStateInner,
        game_detection: Option<&'a GameDetectionResponse>,
        purpose: EffectEnginePurpose,
        cache: &'cache mut EffectRuntimeCache,
    ) -> Self {
        Self {
            inner,
            game_detection,
            purpose,
            cache,
        }
    }

    pub(crate) fn current_response(
        &mut self,
        hardware_output_enabled: bool,
    ) -> CurrentEffectResponse {
        let resolution = profile_resolution(self.inner, self.game_detection);
        let cache_controller_id = resolution.controller_id.clone();
        let effect = self.materialize(resolution, cache_controller_id.as_deref());
        current_effect_response_from_parts(
            effect.resolution,
            effect.profile,
            effect.config.as_ref(),
            effect.snapshot,
            effect.telemetry_live,
            effect.output,
            self.game_detection,
            hardware_output_enabled,
        )
    }

    pub(crate) fn output_frame_for_current_resolution(
        &mut self,
    ) -> Option<(String, ControllerOutputFrame)> {
        let resolution = profile_resolution(self.inner, self.game_detection);
        let controller_id = resolution.controller_id.clone()?;

        if self.purpose == EffectEnginePurpose::Hardware {
            match hardware_output_frame_plan(self.inner, self.game_detection, &resolution) {
                HardwareOutputFramePlan::RuntimeLiveEffect => {}
                HardwareOutputFramePlan::DetectionLightbar => {
                    let detection = self.game_detection?;
                    let output = ControllerOutputFrame {
                        lightbar: detection_lightbar_output(detection),
                        ..ControllerOutputFrame::default()
                    };
                    return Some((controller_id, output));
                }
                HardwareOutputFramePlan::GlobalLightbar => {
                    if let Some(output) = global_lightbar_output(self.inner, &resolution) {
                        return Some((controller_id, output));
                    }
                    return None;
                }
                HardwareOutputFramePlan::Suppressed => return None,
            }
        }

        let effect = self.materialize(resolution, Some(&controller_id));
        Some((controller_id, effect.output))
    }

    fn materialize(
        &mut self,
        resolution: ProfileResolutionResponse,
        cache_controller_id: Option<&str>,
    ) -> MaterializedRuntimeLiveEffect {
        let config = controller_config_for_resolution(self.inner, &resolution);
        let (snapshot, telemetry_live) = current_effect_snapshot(self.inner, self.game_detection);
        let profile_id = resolution
            .selected_profile_id
            .clone()
            .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
        let profile_name =
            profile_name_by_id(self.inner, &profile_id).unwrap_or_else(|| profile_id.clone());
        let profile = runtime_profile_for(&profile_id, &profile_name, config.as_ref(), &snapshot);
        let mut output =
            self.evaluate_runtime_profile(cache_controller_id, &profile_id, &profile, &snapshot);
        apply_runtime_output_enhancements(
            &profile_id,
            config.as_ref(),
            &snapshot,
            telemetry_live,
            &mut output,
        );
        apply_detection_lightbar_preview(self.game_detection, telemetry_live, &mut output);

        MaterializedRuntimeLiveEffect {
            resolution,
            profile,
            config,
            snapshot,
            telemetry_live,
            output,
        }
    }

    fn evaluate_runtime_profile(
        &mut self,
        controller_id: Option<&str>,
        profile_id: &str,
        profile: &Profile,
        snapshot: &SignalSnapshot,
    ) -> ControllerOutputFrame {
        let key = EffectEngineKey {
            purpose: self.purpose,
            controller_id: controller_id.unwrap_or("none").to_string(),
            profile_id: profile_id.to_string(),
            revision: self.inner.effect_revision,
        };
        self.cache.evaluate(key, profile, snapshot)
    }
}

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

pub(crate) fn racing_shift_tuning(inner: &AgentStateInner) -> ForzaShiftTuningConfig {
    inner
        .controllers
        .summaries()
        .into_iter()
        .find(|controller| controller.connected)
        .and_then(|controller| inner.controller_configs.get(&controller.id))
        .map(|config| config.forza.shift.clone().normalized())
        .unwrap_or_default()
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
    !matches!(
        hardware_output_frame_plan(inner, game_detection, &resolution),
        HardwareOutputFramePlan::Suppressed
    )
}

fn hardware_output_frame_plan(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
    resolution: &ProfileResolutionResponse,
) -> HardwareOutputFramePlan {
    if hardware_output_runtime_allowed_for_resolution(inner, game_detection, resolution) {
        return HardwareOutputFramePlan::RuntimeLiveEffect;
    }
    if hardware_output_detection_lightbar_allowed_for_resolution(inner, game_detection, resolution)
    {
        return HardwareOutputFramePlan::DetectionLightbar;
    }
    if hardware_output_global_lightbar_allowed_for_resolution(game_detection, resolution)
        && global_lightbar_output(inner, resolution).is_some()
    {
        return HardwareOutputFramePlan::GlobalLightbar;
    }
    HardwareOutputFramePlan::Suppressed
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
        telemetry_signal("input.clutch", 0.0, None, age_ms),
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
        signal_update("input.clutch", 0.0),
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
            snapshot.apply_update(signal_update(
                "drivetrain.shift_pulse",
                inner.forza_effect_runtime.latched_shift_pulse(now),
            ));
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
    let mut cache = EffectRuntimeCache::default();
    RuntimeLiveEffectMaterializer::new(
        inner,
        game_detection,
        EffectEnginePurpose::Preview,
        &mut cache,
    )
    .current_response(hardware_output_enabled)
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
