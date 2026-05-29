use super::*;

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

impl ControllerConfig {
    pub(crate) fn default_for(controller_id: impl Into<String>, model: impl Into<String>) -> Self {
        let controller_id = controller_id.into();
        let model = model.into();
        let edge = model == "DualSense Edge";

        Self {
            controller_id,
            model,
            input_mode: ControllerInputMode::NativeDualSense,
            trigger: TriggerConfig::default(),
            lightbar: LightbarConfig::default(),
            forza: ForzaTelemetryConfig::default(),
            sticks: StickConfig::default(),
            buttons: default_button_assignments(edge),
            input_bridge: InputBridgeConfig::default(),
            profile_assignments: default_profile_assignments(edge),
        }
    }

    pub(crate) fn from_update(
        controller_id: impl Into<String>,
        model: impl Into<String>,
        request: UpdateControllerConfigRequest,
        existing_input_bridge: Option<InputBridgeConfig>,
    ) -> Self {
        let model = model.into();
        let edge = model == "DualSense Edge";
        Self {
            controller_id: controller_id.into(),
            model,
            input_mode: request.input_mode,
            trigger: request.trigger.normalized(),
            lightbar: request.lightbar.normalized(),
            forza: request.forza.normalized(),
            sticks: request.sticks.normalized(),
            buttons: normalize_controller_button_assignments(request.buttons, edge),
            input_bridge: request
                .input_bridge
                .or(existing_input_bridge)
                .unwrap_or_default()
                .normalized(),
            profile_assignments: normalize_profile_assignments(request.profile_assignments),
        }
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.forza = self.forza.normalized();
        self.sticks = self.sticks.normalized();
        self.input_mode = match self.input_mode {
            ControllerInputMode::NativeDualSense => ControllerInputMode::NativeDualSense,
            ControllerInputMode::SteamInputCompanion => ControllerInputMode::SteamInputCompanion,
            ControllerInputMode::DsccInputBridge => ControllerInputMode::DsccInputBridge,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, self.model == "DualSense Edge");
        self.input_bridge = self.input_bridge.normalized();
        self.profile_assignments = normalize_profile_assignments(self.profile_assignments);
        self
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::from_controller_config(&ControllerConfig::default_for("", "DualSense"))
    }
}

impl ProfileConfig {
    pub(crate) fn from_controller_config(config: &ControllerConfig) -> Self {
        Self {
            input_mode: config.input_mode,
            trigger: config.trigger.clone(),
            lightbar: config.lightbar.clone(),
            forza: config.forza.clone(),
            sticks: config.sticks.clone(),
            buttons: config.buttons.clone(),
            input_bridge: config.input_bridge.clone(),
        }
        .normalized_for_model(&config.model)
    }

    pub(crate) fn normalized_for_model(mut self, model: &str) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.forza = self.forza.normalized();
        self.sticks = self.sticks.normalized();
        self.input_mode = match self.input_mode {
            ControllerInputMode::NativeDualSense => ControllerInputMode::NativeDualSense,
            ControllerInputMode::SteamInputCompanion => ControllerInputMode::SteamInputCompanion,
            ControllerInputMode::DsccInputBridge => ControllerInputMode::DsccInputBridge,
        };
        self.buttons =
            normalize_controller_button_assignments(self.buttons, model == "DualSense Edge");
        self.input_bridge = self.input_bridge.normalized();
        self
    }

    pub(crate) fn apply_to_controller_config(&self, config: &mut ControllerConfig) {
        let profile_config = self.clone().normalized_for_model(&config.model);
        config.input_mode = profile_config.input_mode;
        config.trigger = profile_config.trigger;
        config.lightbar = profile_config.lightbar;
        config.forza = profile_config.forza;
        config.sticks = profile_config.sticks;
        config.buttons = profile_config.buttons;
        config.input_bridge = profile_config.input_bridge;
    }
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            same_range: false,
            l2_from: 6,
            l2_to: 100,
            r2_from: 0,
            r2_to: 100,
            l2_curve: TriggerCurve::default_l2(),
            r2_curve: TriggerCurve::default_r2(),
            l2_curve_points: default_l2_trigger_curve_points(),
            r2_curve_points: default_r2_trigger_curve_points(),
            effect: "Adaptive resistance".to_string(),
            intensity: "Strong (Standard)".to_string(),
            vibration: "Medium".to_string(),
            vibration_mode: "Balanced".to_string(),
        }
    }
}

impl TriggerConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.l2_from = self.l2_from.min(100);
        self.l2_to = self.l2_to.clamp(self.l2_from, 100);
        self.r2_from = self.r2_from.min(100);
        self.r2_to = self.r2_to.clamp(self.r2_from, 100);
        if self.same_range {
            self.r2_from = self.l2_from;
            self.r2_to = self.l2_to;
        }
        self.l2_curve = self.l2_curve.normalized();
        self.r2_curve = self.r2_curve.normalized();
        self.l2_curve_points = normalize_trigger_curve_points(self.l2_curve_points, self.l2_curve);
        self.r2_curve_points = normalize_trigger_curve_points(self.r2_curve_points, self.r2_curve);
        if !["Adaptive resistance", "Pulse", "Wall", "Wall pulse", "Off"]
            .contains(&self.effect.as_str())
        {
            self.effect = "Adaptive resistance".to_string();
        }
        if !["Off", "Weak", "Medium", "Strong (Standard)"].contains(&self.intensity.as_str()) {
            self.intensity = "Medium".to_string();
        }
        if !["Off", "Low", "Medium", "High"].contains(&self.vibration.as_str()) {
            self.vibration = "Medium".to_string();
        }
        if !["Balanced", "Deep thump", "Fine buzz"].contains(&self.vibration_mode.as_str()) {
            self.vibration_mode = "Balanced".to_string();
        }
        self
    }
}

impl Default for ForzaTelemetryConfig {
    fn default() -> Self {
        Self {
            body_rumble_mode: default_forza_body_rumble_mode(),
            effects: default_forza_effect_configs(),
            abs: ForzaAbsTuningConfig::default(),
            throttle: ForzaThrottleTuningConfig::default(),
            shift: ForzaShiftTuningConfig::default(),
            rev_limiter: ForzaRevLimiterTuningConfig::default(),
        }
    }
}

impl ForzaTelemetryConfig {
    pub(crate) fn normalized(self) -> Self {
        let body_rumble_mode =
            if forza_body_rumble_modes().contains(&self.body_rumble_mode.as_str()) {
                self.body_rumble_mode
            } else {
                default_forza_body_rumble_mode()
            };
        let mut provided = self
            .effects
            .into_iter()
            .map(|effect| (effect.id.clone(), effect))
            .collect::<BTreeMap<_, _>>();
        let mut effects = Vec::new();

        for default in default_forza_effect_configs() {
            let effect = provided
                .remove(&default.id)
                .unwrap_or_else(|| default.clone())
                .normalized_with_default(&default);
            effects.push(effect);
        }

        for (_, effect) in provided {
            if !effect.id.trim().is_empty() {
                let default = ForzaEffectConfig {
                    id: effect.id.clone(),
                    enabled: true,
                    intensity: 100,
                    route: "body_both".to_string(),
                };
                effects.push(effect.normalized_with_default(&default));
            }
        }

        Self {
            body_rumble_mode,
            effects,
            abs: self.abs.normalized(),
            throttle: self.throttle.normalized(),
            shift: self.shift.normalized(),
            rev_limiter: self.rev_limiter.normalized(),
        }
    }

    pub(crate) fn effect(&self, id: &str) -> ForzaEffectConfig {
        let default = default_forza_effect(id);
        self.effects
            .iter()
            .find(|effect| effect.id == id)
            .cloned()
            .unwrap_or_else(|| default.clone())
            .normalized_with_default(&default)
    }
}

impl Default for ForzaAbsTuningConfig {
    fn default() -> Self {
        Self {
            mode: default_forza_abs_mode(),
            slip_source: default_forza_abs_slip_source(),
            slip_threshold: default_forza_abs_slip_threshold(),
            brake_threshold_ratio: default_forza_abs_brake_threshold_ratio(),
            min_speed_kmh: default_forza_abs_min_speed_kmh(),
            min_strength: default_forza_abs_min_strength(),
            max_strength: default_forza_abs_max_strength(),
            frequency_hz: default_forza_abs_frequency_hz(),
            curve: default_forza_abs_curve(),
        }
    }
}

impl ForzaAbsTuningConfig {
    pub(crate) fn normalized(mut self) -> Self {
        if !forza_abs_modes().contains(&self.mode.as_str()) {
            self.mode = default_forza_abs_mode();
        }
        if !forza_abs_slip_sources().contains(&self.slip_source.as_str()) {
            self.slip_source = default_forza_abs_slip_source();
        }
        self.slip_threshold = finite_clamp(
            self.slip_threshold,
            0.05,
            2.0,
            default_forza_abs_slip_threshold(),
        );
        self.brake_threshold_ratio = finite_clamp(
            self.brake_threshold_ratio,
            0.0,
            1.0,
            default_forza_abs_brake_threshold_ratio(),
        );
        self.min_speed_kmh = finite_clamp(
            self.min_speed_kmh,
            0.0,
            250.0,
            default_forza_abs_min_speed_kmh(),
        );
        self.min_strength = finite_clamp(
            self.min_strength,
            0.0,
            1.0,
            default_forza_abs_min_strength(),
        );
        self.max_strength = finite_clamp(
            self.max_strength,
            self.min_strength,
            1.0,
            default_forza_abs_max_strength(),
        );
        self.frequency_hz = finite_clamp(
            self.frequency_hz,
            1.0,
            80.0,
            default_forza_abs_frequency_hz(),
        );
        self.curve = finite_clamp(self.curve, 0.4, 3.0, default_forza_abs_curve());
        self
    }
}

fn finite_clamp(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback.clamp(min, max)
    }
}

pub(crate) fn default_forza_abs_mode() -> String {
    "strong_pulse".to_string()
}

pub(crate) fn default_forza_abs_slip_source() -> String {
    "auto_front_first".to_string()
}

pub(crate) fn default_forza_abs_slip_threshold() -> f64 {
    FORZA_ABS_SLIP_THRESHOLD
}

pub(crate) fn default_forza_abs_brake_threshold_ratio() -> f64 {
    FORZA_ABS_RANGE_START_RATIO
}

pub(crate) fn default_forza_abs_min_speed_kmh() -> f64 {
    FORZA_ABS_MIN_SPEED_KMH
}

pub(crate) fn default_forza_abs_min_strength() -> f64 {
    FORZA_ABS_PULSE_MIN_AMPLITUDE
}

pub(crate) fn default_forza_abs_max_strength() -> f64 {
    FORZA_ABS_PULSE_MAX_AMPLITUDE
}

pub(crate) fn default_forza_abs_frequency_hz() -> f64 {
    FORZA_ABS_PULSE_FREQUENCY_HZ
}

pub(crate) fn default_forza_abs_curve() -> f64 {
    1.0
}

pub(crate) fn forza_abs_modes() -> &'static [&'static str] {
    &["strong_pulse", "fine_flutter"]
}

pub(crate) fn forza_abs_slip_sources() -> &'static [&'static str] {
    &["auto_front_first", "front", "tire", "wheel"]
}

impl Default for ForzaThrottleTuningConfig {
    fn default() -> Self {
        Self {
            baseline_force: default_forza_throttle_baseline_force(),
            normal_force: default_forza_throttle_normal_force(),
            endstop_force: default_forza_throttle_endstop_force(),
            endstop_boost: default_forza_throttle_endstop_boost(),
            wall_position: default_forza_throttle_wall_position(),
            guard_min_end: default_forza_throttle_guard_min_end(),
            ramp_width: default_forza_throttle_ramp_width(),
            ramp_curve: default_forza_throttle_ramp_curve(),
        }
    }
}

impl ForzaThrottleTuningConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.baseline_force = finite_clamp(
            self.baseline_force,
            0.0,
            1.0,
            default_forza_throttle_baseline_force(),
        );
        self.normal_force = finite_clamp(
            self.normal_force,
            self.baseline_force,
            1.0,
            default_forza_throttle_normal_force(),
        );
        self.endstop_force = finite_clamp(
            self.endstop_force,
            0.0,
            1.0,
            default_forza_throttle_endstop_force(),
        );
        self.endstop_boost = finite_clamp(
            self.endstop_boost,
            0.0,
            5.0,
            default_forza_throttle_endstop_boost(),
        );
        self.wall_position = finite_clamp(
            self.wall_position,
            0.0,
            1.0,
            default_forza_throttle_wall_position(),
        );
        self.guard_min_end = finite_clamp(
            self.guard_min_end,
            0.0,
            1.0,
            default_forza_throttle_guard_min_end(),
        );
        self.ramp_width = finite_clamp(
            self.ramp_width,
            0.01,
            0.80,
            default_forza_throttle_ramp_width(),
        );
        self.ramp_curve = finite_clamp(
            self.ramp_curve,
            0.4,
            4.0,
            default_forza_throttle_ramp_curve(),
        );
        self
    }
}

pub(crate) fn default_forza_throttle_baseline_force() -> f64 {
    FORZA_THROTTLE_BASELINE_FORCE
}

pub(crate) fn default_forza_throttle_normal_force() -> f64 {
    FORZA_THROTTLE_NORMAL_FORCE
}

pub(crate) fn default_forza_throttle_endstop_force() -> f64 {
    FORZA_THROTTLE_ENDSTOP_FORCE
}

pub(crate) fn default_forza_throttle_endstop_boost() -> f64 {
    FORZA_THROTTLE_ENDSTOP_FORCE_BOOST
}

pub(crate) fn default_forza_throttle_wall_position() -> f64 {
    FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION
}

pub(crate) fn default_forza_throttle_guard_min_end() -> f64 {
    FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION
}

pub(crate) fn default_forza_throttle_ramp_width() -> f64 {
    FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH
}

pub(crate) fn default_forza_throttle_ramp_curve() -> f64 {
    FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE
}

impl Default for ForzaShiftTuningConfig {
    fn default() -> Self {
        Self {
            wall_form_at: default_forza_shift_wall_form_at(),
            frequency_hz: default_forza_shift_frequency_hz(),
            wall_zones: default_forza_shift_wall_zones(),
            body_low_weight: default_forza_shift_body_low_weight(),
            body_high_weight: default_forza_shift_body_high_weight(),
        }
    }
}

impl ForzaShiftTuningConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.wall_form_at = finite_clamp(
            self.wall_form_at,
            0.0,
            1.0,
            default_forza_shift_wall_form_at(),
        );
        self.frequency_hz = finite_clamp(
            self.frequency_hz,
            1.0,
            80.0,
            default_forza_shift_frequency_hz(),
        );
        self.wall_zones = finite_clamp(self.wall_zones, 1.0, 8.0, default_forza_shift_wall_zones());
        self.body_low_weight = finite_clamp(
            self.body_low_weight,
            0.0,
            1.5,
            default_forza_shift_body_low_weight(),
        );
        self.body_high_weight = finite_clamp(
            self.body_high_weight,
            0.0,
            1.5,
            default_forza_shift_body_high_weight(),
        );
        self
    }
}

pub(crate) fn default_forza_shift_wall_form_at() -> f64 {
    FORZA_SHIFT_WALL_FORM_AT
}

pub(crate) fn default_forza_shift_frequency_hz() -> f64 {
    FORZA_SHIFT_FREQUENCY_HZ
}

pub(crate) fn default_forza_shift_wall_zones() -> f64 {
    FORZA_SHIFT_WALL_ZONES
}

pub(crate) fn default_forza_shift_body_low_weight() -> f64 {
    0.92
}

pub(crate) fn default_forza_shift_body_high_weight() -> f64 {
    0.84
}

impl Default for ForzaRevLimiterTuningConfig {
    fn default() -> Self {
        Self {
            threshold_ratio: default_forza_rev_limiter_threshold_ratio(),
            min_strength: default_forza_rev_limiter_min_strength(),
            max_strength: default_forza_rev_limiter_max_strength(),
            frequency_hz: default_forza_rev_limiter_frequency_hz(),
            wall_form_throttle_at: default_forza_rev_limiter_wall_form_throttle_at(),
            wall_zones: default_forza_rev_limiter_wall_zones(),
            curve: default_forza_rev_limiter_curve(),
            body_low_weight: default_forza_rev_limiter_body_low_weight(),
            body_high_weight: default_forza_rev_limiter_body_high_weight(),
        }
    }
}

impl ForzaRevLimiterTuningConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.threshold_ratio = finite_clamp(
            self.threshold_ratio,
            0.5,
            1.0,
            default_forza_rev_limiter_threshold_ratio(),
        );
        self.min_strength = finite_clamp(
            self.min_strength,
            0.0,
            1.0,
            default_forza_rev_limiter_min_strength(),
        );
        self.max_strength = finite_clamp(
            self.max_strength,
            self.min_strength,
            1.0,
            default_forza_rev_limiter_max_strength(),
        );
        self.frequency_hz = finite_clamp(
            self.frequency_hz,
            1.0,
            80.0,
            default_forza_rev_limiter_frequency_hz(),
        );
        self.wall_form_throttle_at = finite_clamp(
            self.wall_form_throttle_at,
            0.0,
            1.0,
            default_forza_rev_limiter_wall_form_throttle_at(),
        );
        self.wall_zones = finite_clamp(
            self.wall_zones,
            1.0,
            8.0,
            default_forza_rev_limiter_wall_zones(),
        );
        self.curve = finite_clamp(self.curve, 0.4, 4.0, default_forza_rev_limiter_curve());
        self.body_low_weight = finite_clamp(
            self.body_low_weight,
            0.0,
            1.5,
            default_forza_rev_limiter_body_low_weight(),
        );
        self.body_high_weight = finite_clamp(
            self.body_high_weight,
            0.0,
            1.5,
            default_forza_rev_limiter_body_high_weight(),
        );
        self
    }
}

pub(crate) fn default_forza_rev_limiter_threshold_ratio() -> f64 {
    FORZA_REV_LIMIT_RATIO
}

pub(crate) fn default_forza_rev_limiter_min_strength() -> f64 {
    FORZA_REV_LIMITER_PULSE_AMPLITUDE
}

pub(crate) fn default_forza_rev_limiter_max_strength() -> f64 {
    FORZA_REV_LIMITER_PULSE_AMPLITUDE
}

pub(crate) fn default_forza_rev_limiter_frequency_hz() -> f64 {
    FORZA_REV_LIMITER_FREQUENCY_HZ
}

pub(crate) fn default_forza_rev_limiter_wall_form_throttle_at() -> f64 {
    FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT
}

pub(crate) fn default_forza_rev_limiter_wall_zones() -> f64 {
    FORZA_REV_LIMITER_WALL_ZONES
}

pub(crate) fn default_forza_rev_limiter_curve() -> f64 {
    1.0
}

pub(crate) fn default_forza_rev_limiter_body_low_weight() -> f64 {
    0.20
}

pub(crate) fn default_forza_rev_limiter_body_high_weight() -> f64 {
    0.80
}

impl ForzaEffectConfig {
    pub(crate) fn normalized_with_default(mut self, default: &ForzaEffectConfig) -> Self {
        if self.id.trim().is_empty() {
            self.id = default.id.clone();
        }
        if !forza_effect_routes().contains(&self.route.as_str()) {
            self.route = default.route.clone();
        }
        self
    }

    pub(crate) fn scalar(&self) -> f64 {
        if self.enabled {
            f64::from(self.intensity) / 100.0
        } else {
            0.0
        }
    }
}

pub(crate) fn default_forza_effect_enabled() -> bool {
    true
}

pub(crate) fn default_forza_effect_intensity() -> u8 {
    100
}

pub(crate) fn default_forza_effect_route() -> String {
    "body_both".to_string()
}

pub(crate) fn default_forza_body_rumble_mode() -> String {
    "native_passthrough".to_string()
}

pub(crate) fn forza_body_rumble_modes() -> &'static [&'static str] {
    &["native_passthrough", "dscc_full_control"]
}

pub(crate) fn default_forza_effect(id: &str) -> ForzaEffectConfig {
    default_forza_effect_configs()
        .into_iter()
        .find(|effect| effect.id == id)
        .unwrap_or_else(|| ForzaEffectConfig {
            id: id.to_string(),
            enabled: true,
            intensity: 100,
            route: "body_both".to_string(),
        })
}

pub(crate) fn default_forza_effect_configs() -> Vec<ForzaEffectConfig> {
    [
        ("brake_resistance", 100, "l2"),
        ("abs_slip_pulse", 100, "l2"),
        ("handbrake_wall", 100, "l2"),
        ("throttle_resistance", 100, "r2"),
        (
            "gear_shift_thump",
            FORZA_SHIFT_THUMP_DEFAULT_INTENSITY,
            "r2_and_body",
        ),
        ("rev_limiter_buzz", 120, "r2"),
        ("road_texture", 60, "body_both"),
        ("rumble_strip", 72, "body_both"),
        ("tire_slip", 95, "body_right"),
        ("puddle_drag", 75, "body_left"),
        ("suspension_impact", 115, "body_both"),
        ("rpm_leds", 100, "light_led"),
    ]
    .into_iter()
    .map(|(id, intensity, route)| ForzaEffectConfig {
        id: id.to_string(),
        enabled: true,
        intensity,
        route: route.to_string(),
    })
    .collect()
}

pub(crate) fn forza_effect_routes() -> &'static [&'static str] {
    &[
        "body_both",
        "body_left",
        "body_right",
        "l2",
        "r2",
        "both_triggers",
        "body_and_triggers",
        "r2_and_body",
        "light_led",
    ]
}

impl Default for LightbarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            color: "#4cc9f0".to_string(),
            rpm_color: default_rpm_color(),
            brightness: 72,
        }
    }
}

impl LightbarConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.color = normalize_hex_color(&self.color);
        self.rpm_color = normalize_hex_color_or(&self.rpm_color, "#ff3a2e");
        self.brightness = self.brightness.min(100);
        self
    }

    pub(crate) fn rgb(&self) -> RgbColor {
        let normalized = normalize_hex_color(&self.color);
        let value = normalized.trim_start_matches('#');
        RgbColor {
            red: u8::from_str_radix(&value[0..2], 16).unwrap_or(0x4c),
            green: u8::from_str_radix(&value[2..4], 16).unwrap_or(0xc9),
            blue: u8::from_str_radix(&value[4..6], 16).unwrap_or(0xf0),
        }
    }

    pub(crate) fn rpm_rgb(&self) -> RgbColor {
        let normalized = normalize_hex_color_or(&self.rpm_color, "#ff3a2e");
        let value = normalized.trim_start_matches('#');
        RgbColor {
            red: u8::from_str_radix(&value[0..2], 16).unwrap_or(0xff),
            green: u8::from_str_radix(&value[2..4], 16).unwrap_or(0x3a),
            blue: u8::from_str_radix(&value[4..6], 16).unwrap_or(0x2e),
        }
    }
}

pub(crate) fn normalize_hex_color(value: &str) -> String {
    normalize_hex_color_or(value, "#4cc9f0")
}

pub(crate) fn normalize_hex_color_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        format!("#{hex}").to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

pub(crate) fn rgb_from_hex(value: &str) -> Option<RgbColor> {
    let normalized = normalize_hex_color_or(value, "");
    let value = normalized.strip_prefix('#')?;
    Some(RgbColor {
        red: u8::from_str_radix(&value[0..2], 16).ok()?,
        green: u8::from_str_radix(&value[2..4], 16).ok()?,
        blue: u8::from_str_radix(&value[4..6], 16).ok()?,
    })
}

pub(crate) fn default_rpm_color() -> String {
    "#ff3a2e".to_string()
}

impl Default for StickConfig {
    fn default() -> Self {
        Self {
            left_curve: "Quick".to_string(),
            left_curve_amount: 48,
            left_deadzone: 4,
            right_curve: "Default".to_string(),
            right_curve_amount: 62,
            right_deadzone: 8,
        }
    }
}

impl StickConfig {
    pub(crate) fn normalized(mut self) -> Self {
        for curve in [&mut self.left_curve, &mut self.right_curve] {
            if ![
                "Default", "Quick", "Precise", "Steady", "Digital", "Dynamic",
            ]
            .contains(&curve.as_str())
            {
                *curve = "Default".to_string();
            }
        }
        self.left_curve_amount = self.left_curve_amount.min(100);
        self.right_curve_amount = self.right_curve_amount.min(100);
        self.left_deadzone = self.left_deadzone.min(40);
        self.right_deadzone = self.right_deadzone.min(40);
        self
    }
}

impl ButtonAssignmentConfig {
    fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

pub(crate) fn default_button_assignments(edge: bool) -> Vec<ButtonAssignmentConfig> {
    let mut buttons = vec![
        ButtonAssignmentConfig::new("Cross", "Cross"),
        ButtonAssignmentConfig::new("Circle", "Circle"),
        ButtonAssignmentConfig::new("Square", "Square"),
        ButtonAssignmentConfig::new("Triangle", "Triangle"),
        ButtonAssignmentConfig::new("D-Pad", "D-Pad"),
        ButtonAssignmentConfig::new("L1", "L1"),
        ButtonAssignmentConfig::new("R1", "R1"),
        ButtonAssignmentConfig::new("L2", "L2"),
        ButtonAssignmentConfig::new("R2", "R2"),
        ButtonAssignmentConfig::new("L3", "L3"),
        ButtonAssignmentConfig::new("R3", "R3"),
        ButtonAssignmentConfig::new("Create", "Create"),
        ButtonAssignmentConfig::new("Options", "Options"),
        ButtonAssignmentConfig::new("Touch Pad", "Touch Pad Press"),
        ButtonAssignmentConfig::new("Mute", "Mute"),
    ];
    if edge {
        buttons.extend([
            ButtonAssignmentConfig::new("Back Left", "L3"),
            ButtonAssignmentConfig::new("Back Right", "R3"),
            ButtonAssignmentConfig::new("Fn Left", "Previous DSCC Profile"),
            ButtonAssignmentConfig::new("Fn Right", "Next DSCC Profile"),
        ]);
    }
    buttons
}

pub(crate) fn normalize_controller_button_assignments(
    buttons: Vec<ButtonAssignmentConfig>,
    edge: bool,
) -> Vec<ButtonAssignmentConfig> {
    let mut normalized = normalize_button_assignments(buttons);
    let defaults = default_button_assignments(edge);
    let mut ordered = Vec::with_capacity(defaults.len().max(normalized.len()).min(24));

    for default in defaults {
        if let Some(index) = normalized
            .iter()
            .position(|button| button.key == default.key)
        {
            ordered.push(normalized.remove(index));
        } else {
            ordered.push(default);
        }
    }

    let remaining = 24_usize.saturating_sub(ordered.len());
    ordered.extend(normalized.into_iter().take(remaining));
    ordered
}

pub(crate) fn normalize_button_assignments(
    buttons: Vec<ButtonAssignmentConfig>,
) -> Vec<ButtonAssignmentConfig> {
    buttons
        .into_iter()
        .filter(|button| !button.key.trim().is_empty())
        .map(normalize_button_assignment)
        .take(24)
        .collect()
}

pub(crate) fn normalize_button_assignment(
    button: ButtonAssignmentConfig,
) -> ButtonAssignmentConfig {
    let key = normalize_button_key(&button.key);
    let label = normalize_button_label(&key, &button.label);
    ButtonAssignmentConfig { key, label }
}

pub(crate) fn normalize_button_key(key: &str) -> String {
    match key.trim() {
        "" => "Unassigned".to_string(),
        other => other.chars().take(24).collect(),
    }
}

pub(crate) fn normalize_button_label(key: &str, label: &str) -> String {
    let trimmed = label.trim();
    let normalized = if trimmed.is_empty() {
        default_assignment_for_key(key)
    } else {
        trimmed.to_string()
    };

    if is_supported_assignment_label(&normalized) {
        normalized
    } else {
        default_assignment_for_key(key)
    }
}

pub(crate) fn default_assignment_for_key(key: &str) -> String {
    match key {
        "Back Left" => "L3",
        "Back Right" => "R3",
        "Fn Left" => "Previous DSCC Profile",
        "Fn Right" => "Next DSCC Profile",
        "Touch Pad" => "Touch Pad Press",
        other if is_supported_assignment_label(other) => other,
        _ => "Unassigned",
    }
    .to_string()
}

pub(crate) fn is_supported_assignment_label(label: &str) -> bool {
    matches!(
        label,
        "Unassigned"
            | "Cross"
            | "Circle"
            | "Square"
            | "Triangle"
            | "D-Pad"
            | "D-Pad Up"
            | "D-Pad Down"
            | "D-Pad Left"
            | "D-Pad Right"
            | "L1"
            | "R1"
            | "L2"
            | "R2"
            | "L3"
            | "R3"
            | "Create"
            | "Options"
            | "Touch Pad Press"
            | "Mute"
            | "Previous DSCC Profile"
            | "Next DSCC Profile"
            | "Toggle Telemetry Overlay"
            | "Toggle Effect Preview"
    )
}

pub(crate) fn normalize_controller_display_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.chars().take(64).collect())
}

pub(crate) fn apply_controller_names(
    mut controllers: Vec<ControllerSummary>,
    names: &BTreeMap<String, String>,
) -> Vec<ControllerSummary> {
    for controller in &mut controllers {
        if let Some(name) = names.get(&controller.id) {
            controller.name = name.clone();
        }
    }
    controllers
}

pub(crate) fn apply_controller_name(
    mut detail: ControllerDetail,
    names: &BTreeMap<String, String>,
) -> ControllerDetail {
    if let Some(name) = names.get(&detail.id) {
        detail.name = name.clone();
    }
    detail
}

pub(crate) fn default_adapters() -> Vec<AdapterSummary> {
    built_in_adapters()
        .iter()
        .map(|adapter| {
            let enabled = adapter.enabled_by_default;
            AdapterSummary {
                id: adapter.id.to_string(),
                name: adapter.display_name.to_string(),
                enabled,
                state: adapter_state_label(&initial_detection(adapter, enabled)).to_string(),
                packet_rate_hz: None,
                protocol: format!("{:?}", adapter.protocol).to_ascii_lowercase(),
                setup_hint: adapter.setup_hint.to_string(),
                setup_url: adapter.setup_url.map(str::to_string),
            }
        })
        .collect()
}

pub(crate) fn set_adapter_running(
    adapters: &mut [AdapterSummary],
    adapter_id: &str,
    running: bool,
) {
    if let Some(adapter) = adapters.iter_mut().find(|adapter| adapter.id == adapter_id) {
        if running && !adapter.enabled {
            adapter.enabled = true;
        }
        let state = if running {
            "connected"
        } else if adapter.enabled {
            "ready"
        } else {
            "disabled"
        };
        if adapter.state != state {
            adapter.state = state.to_string();
        }
        let packet_rate_hz = running.then_some(60);
        if adapter.packet_rate_hz != packet_rate_hz {
            adapter.packet_rate_hz = packet_rate_hz;
        }
    }
}

pub(crate) fn module_summaries() -> Vec<ModuleSummary> {
    let mut summaries: Vec<ModuleSummary> = built_in_adapters()
        .iter()
        .map(|adapter| ModuleSummary {
            id: adapter.id.to_string(),
            name: adapter.display_name.to_string(),
            kind: "adapter".to_string(),
            version: "builtin".to_string(),
            source: "built_in".to_string(),
            trusted: true,
            protocol: format!("{:?}", adapter.protocol).to_ascii_lowercase(),
            setup_hint: adapter.setup_hint.to_string(),
            setup_url: adapter.setup_url.map(str::to_string),
            profile_templates: Vec::new(),
        })
        .collect();
    summaries.extend(game_module_summaries());
    summaries
}
