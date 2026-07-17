use super::*;

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn is_edge_model(model: &str) -> bool {
    model == "DualSense Edge"
}

struct SavedTuningProjection {
    input_mode: ControllerInputMode,
    trigger: TriggerConfig,
    lightbar: LightbarConfig,
    forza: ForzaTelemetryConfig,
    sticks: StickConfig,
    buttons: Vec<ButtonAssignmentConfig>,
    input_bridge: InputBridgeConfig,
}

impl SavedTuningProjection {
    fn from_controller(config: &ControllerConfig) -> Self {
        Self {
            input_mode: config.input_mode,
            trigger: config.trigger.clone(),
            lightbar: config.lightbar.clone(),
            forza: config.forza.clone(),
            sticks: config.sticks.clone(),
            buttons: config.buttons.clone(),
            input_bridge: config.input_bridge.clone(),
        }
    }

    fn from_profile(config: ProfileConfig) -> Self {
        Self {
            input_mode: config.input_mode,
            trigger: config.trigger,
            lightbar: config.lightbar,
            forza: config.forza,
            sticks: config.sticks,
            buttons: config.buttons,
            input_bridge: config.input_bridge,
        }
    }

    fn normalized_for_model(mut self, model: &str) -> Self {
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.forza = self.forza.normalized();
        self.sticks = self.sticks.normalized();
        self.buttons = normalize_controller_button_assignments(self.buttons, is_edge_model(model));
        self.input_bridge = self.input_bridge.normalized();
        self
    }

    fn apply_to_controller(self, config: &mut ControllerConfig) {
        config.input_mode = self.input_mode;
        config.trigger = self.trigger;
        config.lightbar = self.lightbar;
        config.forza = self.forza;
        config.sticks = self.sticks;
        config.buttons = self.buttons;
        config.input_bridge = self.input_bridge;
    }

    fn into_profile(self) -> ProfileConfig {
        ProfileConfig {
            input_mode: self.input_mode,
            trigger: self.trigger,
            lightbar: self.lightbar,
            forza: self.forza,
            sticks: self.sticks,
            buttons: self.buttons,
            input_bridge: self.input_bridge,
        }
    }
}

impl ControllerConfig {
    pub(crate) fn default_for(controller_id: impl Into<String>, model: impl Into<String>) -> Self {
        let controller_id = controller_id.into();
        let model = model.into();
        let edge = is_edge_model(&model);

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
        let edge = is_edge_model(&model);
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
        let projection =
            SavedTuningProjection::from_controller(&self).normalized_for_model(&self.model);
        projection.apply_to_controller(&mut self);
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
        SavedTuningProjection::from_controller(config)
            .normalized_for_model(&config.model)
            .into_profile()
    }

    pub(crate) fn normalized_for_model(self, model: &str) -> Self {
        SavedTuningProjection::from_profile(self)
            .normalized_for_model(model)
            .into_profile()
    }

    pub(crate) fn apply_to_controller_config(&self, config: &mut ControllerConfig) {
        SavedTuningProjection::from_profile(self.clone())
            .normalized_for_model(&config.model)
            .apply_to_controller(config);
    }
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            same_range: false,
            l2_from: 0,
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
        if self.l2_curve_points == previous_soft_l2_trigger_curve_points() {
            self.l2_curve = TriggerCurve::default_l2();
            self.l2_curve_points = default_l2_trigger_curve_points();
        }
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

mod forza_tuning {
    use super::*;

    #[derive(Clone, Copy)]
    struct RangeRule {
        min: f64,
        max: f64,
        fallback: f64,
    }

    impl RangeRule {
        const fn new(min: f64, max: f64, fallback: f64) -> Self {
            Self { min, max, fallback }
        }

        fn clamp(self, value: f64) -> f64 {
            if value.is_finite() {
                value.clamp(self.min, self.max)
            } else {
                self.fallback.clamp(self.min, self.max)
            }
        }

        const fn with_min(self, min: f64) -> Self {
            Self { min, ..self }
        }
    }

    const fn range(min: f64, max: f64, fallback: f64) -> RangeRule {
        RangeRule::new(min, max, fallback)
    }

    const fn unit_range(fallback: f64) -> RangeRule {
        RangeRule::new(0.0, 1.0, fallback)
    }

    pub(super) fn default_telemetry() -> ForzaTelemetryConfig {
        ForzaTelemetryConfig {
            body_rumble_mode: default_body_rumble_mode(),
            effects: default_effect_configs(),
            brake: brake::default(),
            abs: abs::default(),
            throttle: throttle::default(),
            shift: shift::default(),
            rev_limiter: rev_limiter::default(),
        }
    }

    pub(super) fn normalize_telemetry(config: ForzaTelemetryConfig) -> ForzaTelemetryConfig {
        let body_rumble_mode = if body_rumble_modes().contains(&config.body_rumble_mode.as_str()) {
            config.body_rumble_mode
        } else {
            default_body_rumble_mode()
        };

        ForzaTelemetryConfig {
            body_rumble_mode,
            effects: normalize_effects(config.effects),
            brake: brake::normalize(config.brake),
            abs: abs::normalize(config.abs),
            throttle: throttle::normalize(config.throttle),
            shift: shift::normalize(config.shift),
            rev_limiter: rev_limiter::normalize(config.rev_limiter),
        }
    }

    fn normalize_effects(effects: Vec<ForzaEffectConfig>) -> Vec<ForzaEffectConfig> {
        let mut provided = effects
            .into_iter()
            .map(|effect| (effect.id.clone(), effect))
            .collect::<BTreeMap<_, _>>();
        let mut normalized = Vec::new();

        for default in default_effect_configs() {
            let effect = provided
                .remove(&default.id)
                .unwrap_or_else(|| default.clone())
                .normalized_with_default(&default);
            normalized.push(effect);
        }

        for (_, effect) in provided {
            if !effect.id.trim().is_empty() {
                let default = ForzaEffectConfig {
                    id: effect.id.clone(),
                    enabled: true,
                    intensity: 100,
                    route: "body_both".to_string(),
                };
                normalized.push(effect.normalized_with_default(&default));
            }
        }

        normalized
    }

    pub(super) fn default_effect(id: &str) -> ForzaEffectConfig {
        default_effect_configs()
            .into_iter()
            .find(|effect| effect.id == id)
            .unwrap_or_else(|| ForzaEffectConfig {
                id: id.to_string(),
                enabled: true,
                intensity: 100,
                route: "body_both".to_string(),
            })
    }

    pub(super) fn default_effect_configs() -> Vec<ForzaEffectConfig> {
        [
            ("brake_resistance", 77, "l2"),
            ("abs_slip_pulse", 26, "l2"),
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

    pub(super) fn default_body_rumble_mode() -> String {
        "native_passthrough".to_string()
    }

    pub(super) fn body_rumble_modes() -> &'static [&'static str] {
        &["native_passthrough", "dscc_full_control"]
    }

    pub(super) fn effect_routes() -> &'static [&'static str] {
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

    pub(crate) mod brake {
        use super::super::*;
        use super::{range, unit_range};

        pub(crate) fn default() -> ForzaBrakeTuningConfig {
            ForzaBrakeTuningConfig {
                baseline_force: FORZA_BRAKE_BASELINE_FORCE,
                normal_force: FORZA_BRAKE_NORMAL_FORCE,
                endstop_force: FORZA_BRAKE_ENDSTOP_FORCE,
                endstop_boost: FORZA_BRAKE_ENDSTOP_FORCE_BOOST,
                wall_position: FORZA_BRAKE_OVERTRAVEL_WALL_POSITION,
                guard_min_end: FORZA_BRAKE_OVERTRAVEL_MIN_POSITION,
                full_force_at: FORZA_BRAKE_FULL_FORCE_INPUT,
                ramp_curve: FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE,
            }
        }

        pub(crate) fn normalize(config: ForzaBrakeTuningConfig) -> ForzaBrakeTuningConfig {
            let defaults = default();
            let baseline_force = unit_range(defaults.baseline_force).clamp(config.baseline_force);

            ForzaBrakeTuningConfig {
                baseline_force,
                normal_force: unit_range(defaults.normal_force)
                    .with_min(baseline_force)
                    .clamp(config.normal_force),
                endstop_force: unit_range(defaults.endstop_force).clamp(config.endstop_force),
                endstop_boost: range(0.0, 5.0, defaults.endstop_boost).clamp(config.endstop_boost),
                wall_position: unit_range(defaults.wall_position).clamp(config.wall_position),
                guard_min_end: unit_range(defaults.guard_min_end).clamp(config.guard_min_end),
                full_force_at: unit_range(defaults.full_force_at).clamp(config.full_force_at),
                ramp_curve: range(0.4, 4.0, defaults.ramp_curve).clamp(config.ramp_curve),
            }
        }
    }

    pub(crate) mod abs {
        use super::super::*;
        use super::{range, unit_range};

        pub(crate) fn default() -> ForzaAbsTuningConfig {
            ForzaAbsTuningConfig {
                mode: default_mode(),
                slip_source: default_slip_source(),
                slip_threshold: FORZA_ABS_SLIP_THRESHOLD,
                brake_threshold_ratio: FORZA_ABS_RANGE_START_RATIO,
                min_speed_kmh: FORZA_ABS_MIN_SPEED_KMH,
                min_strength: FORZA_ABS_PULSE_MIN_AMPLITUDE,
                max_strength: FORZA_ABS_PULSE_MAX_AMPLITUDE,
                frequency_hz: FORZA_ABS_PULSE_FREQUENCY_HZ,
                curve: 1.0,
            }
        }

        pub(crate) fn normalize(config: ForzaAbsTuningConfig) -> ForzaAbsTuningConfig {
            let defaults = default();
            let mode = if modes().contains(&config.mode.as_str()) {
                config.mode
            } else {
                defaults.mode
            };
            let slip_source = if slip_sources().contains(&config.slip_source.as_str()) {
                config.slip_source
            } else {
                defaults.slip_source
            };
            let min_strength = unit_range(defaults.min_strength).clamp(config.min_strength);

            ForzaAbsTuningConfig {
                mode,
                slip_source,
                slip_threshold: range(0.05, 2.0, defaults.slip_threshold)
                    .clamp(config.slip_threshold),
                brake_threshold_ratio: unit_range(defaults.brake_threshold_ratio)
                    .clamp(config.brake_threshold_ratio),
                min_speed_kmh: range(0.0, 250.0, defaults.min_speed_kmh)
                    .clamp(config.min_speed_kmh),
                min_strength,
                max_strength: unit_range(defaults.max_strength)
                    .with_min(min_strength)
                    .clamp(config.max_strength),
                frequency_hz: range(1.0, 80.0, defaults.frequency_hz).clamp(config.frequency_hz),
                curve: range(0.4, 3.0, defaults.curve).clamp(config.curve),
            }
        }

        pub(crate) fn default_mode() -> String {
            "strong_pulse".to_string()
        }

        pub(crate) fn default_slip_source() -> String {
            "auto_front_first".to_string()
        }

        pub(crate) fn modes() -> &'static [&'static str] {
            &["strong_pulse", "fine_flutter"]
        }

        pub(crate) fn slip_sources() -> &'static [&'static str] {
            &["auto_front_first", "front", "tire", "wheel"]
        }
    }

    pub(crate) mod throttle {
        use super::super::*;
        use super::{range, unit_range};

        pub(crate) fn default() -> ForzaThrottleTuningConfig {
            ForzaThrottleTuningConfig {
                baseline_force: FORZA_THROTTLE_BASELINE_FORCE,
                normal_force: FORZA_THROTTLE_NORMAL_FORCE,
                endstop_force: FORZA_THROTTLE_ENDSTOP_FORCE,
                endstop_boost: FORZA_THROTTLE_ENDSTOP_FORCE_BOOST,
                wall_position: FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION,
                guard_min_end: FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION,
                ramp_width: FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH,
                ramp_curve: FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE,
            }
        }

        pub(crate) fn normalize(config: ForzaThrottleTuningConfig) -> ForzaThrottleTuningConfig {
            let defaults = default();
            let baseline_force = unit_range(defaults.baseline_force).clamp(config.baseline_force);

            ForzaThrottleTuningConfig {
                baseline_force,
                normal_force: unit_range(defaults.normal_force)
                    .with_min(baseline_force)
                    .clamp(config.normal_force),
                endstop_force: unit_range(defaults.endstop_force).clamp(config.endstop_force),
                endstop_boost: range(0.0, 5.0, defaults.endstop_boost).clamp(config.endstop_boost),
                wall_position: unit_range(defaults.wall_position).clamp(config.wall_position),
                guard_min_end: unit_range(defaults.guard_min_end).clamp(config.guard_min_end),
                ramp_width: range(0.01, 0.80, defaults.ramp_width).clamp(config.ramp_width),
                ramp_curve: range(0.4, 4.0, defaults.ramp_curve).clamp(config.ramp_curve),
            }
        }
    }

    pub(crate) mod shift {
        use super::super::*;
        use super::{range, unit_range};

        pub(crate) fn default() -> ForzaShiftTuningConfig {
            ForzaShiftTuningConfig {
                wall_form_at: FORZA_SHIFT_WALL_FORM_AT,
                frequency_hz: FORZA_SHIFT_FREQUENCY_HZ,
                wall_zones: FORZA_SHIFT_WALL_ZONES,
                body_low_weight: 0.92,
                body_high_weight: 0.84,
                clutch_mode: default_clutch_mode(),
                clutch_threshold: FORZA_SHIFT_CLUTCH_THRESHOLD,
                with_clutch_strength: FORZA_SHIFT_WITH_CLUTCH_STRENGTH,
                without_clutch_strength: FORZA_SHIFT_WITHOUT_CLUTCH_STRENGTH,
                with_clutch_duration_ms: FORZA_SHIFT_WITH_CLUTCH_DURATION_MS,
                without_clutch_duration_ms: FORZA_SHIFT_WITHOUT_CLUTCH_DURATION_MS,
                clutch_body_cut: FORZA_SHIFT_CLUTCH_BODY_CUT,
            }
        }

        pub(crate) fn normalize(config: ForzaShiftTuningConfig) -> ForzaShiftTuningConfig {
            let defaults = default();

            ForzaShiftTuningConfig {
                wall_form_at: unit_range(defaults.wall_form_at).clamp(config.wall_form_at),
                frequency_hz: range(1.0, 80.0, defaults.frequency_hz).clamp(config.frequency_hz),
                wall_zones: range(1.0, 8.0, defaults.wall_zones).clamp(config.wall_zones),
                body_low_weight: range(0.0, 1.5, defaults.body_low_weight)
                    .clamp(config.body_low_weight),
                body_high_weight: range(0.0, 1.5, defaults.body_high_weight)
                    .clamp(config.body_high_weight),
                clutch_mode: normalize_clutch_mode(&config.clutch_mode).to_string(),
                clutch_threshold: unit_range(defaults.clutch_threshold)
                    .clamp(config.clutch_threshold),
                with_clutch_strength: unit_range(defaults.with_clutch_strength)
                    .clamp(config.with_clutch_strength),
                without_clutch_strength: unit_range(defaults.without_clutch_strength)
                    .clamp(config.without_clutch_strength),
                with_clutch_duration_ms: range(40.0, 400.0, defaults.with_clutch_duration_ms)
                    .clamp(config.with_clutch_duration_ms),
                without_clutch_duration_ms: range(40.0, 500.0, defaults.without_clutch_duration_ms)
                    .clamp(config.without_clutch_duration_ms),
                clutch_body_cut: unit_range(defaults.clutch_body_cut).clamp(config.clutch_body_cut),
            }
        }

        pub(crate) fn default_clutch_mode() -> String {
            "auto".to_string()
        }

        pub(crate) fn normalize_clutch_mode(mode: &str) -> &'static str {
            match mode {
                "off" => "off",
                "manual_clutch" => "manual_clutch",
                _ => "auto",
            }
        }
    }

    pub(crate) mod rev_limiter {
        use super::super::*;
        use super::{range, unit_range};

        pub(crate) fn default() -> ForzaRevLimiterTuningConfig {
            ForzaRevLimiterTuningConfig {
                threshold_ratio: FORZA_REV_LIMIT_RATIO,
                min_strength: FORZA_REV_LIMITER_PULSE_AMPLITUDE,
                max_strength: FORZA_REV_LIMITER_PULSE_AMPLITUDE,
                frequency_hz: FORZA_REV_LIMITER_FREQUENCY_HZ,
                wall_form_throttle_at: FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT,
                wall_zones: FORZA_REV_LIMITER_WALL_ZONES,
                curve: 1.0,
                body_low_weight: 0.20,
                body_high_weight: 0.80,
            }
        }

        pub(crate) fn normalize(
            config: ForzaRevLimiterTuningConfig,
        ) -> ForzaRevLimiterTuningConfig {
            let defaults = default();
            let min_strength = unit_range(defaults.min_strength).clamp(config.min_strength);

            ForzaRevLimiterTuningConfig {
                threshold_ratio: range(0.5, 1.0, defaults.threshold_ratio)
                    .clamp(config.threshold_ratio),
                min_strength,
                max_strength: unit_range(defaults.max_strength)
                    .with_min(min_strength)
                    .clamp(config.max_strength),
                frequency_hz: range(1.0, 80.0, defaults.frequency_hz).clamp(config.frequency_hz),
                wall_form_throttle_at: unit_range(defaults.wall_form_throttle_at)
                    .clamp(config.wall_form_throttle_at),
                wall_zones: range(1.0, 8.0, defaults.wall_zones).clamp(config.wall_zones),
                curve: range(0.4, 4.0, defaults.curve).clamp(config.curve),
                body_low_weight: range(0.0, 1.5, defaults.body_low_weight)
                    .clamp(config.body_low_weight),
                body_high_weight: range(0.0, 1.5, defaults.body_high_weight)
                    .clamp(config.body_high_weight),
            }
        }
    }
}

impl Default for ForzaTelemetryConfig {
    fn default() -> Self {
        forza_tuning::default_telemetry()
    }
}

impl ForzaTelemetryConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::normalize_telemetry(self)
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

impl Default for ForzaBrakeTuningConfig {
    fn default() -> Self {
        forza_tuning::brake::default()
    }
}

impl ForzaBrakeTuningConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::brake::normalize(self)
    }
}

pub(crate) fn default_forza_brake_baseline_force() -> f64 {
    forza_tuning::brake::default().baseline_force
}

pub(crate) fn default_forza_brake_normal_force() -> f64 {
    forza_tuning::brake::default().normal_force
}

pub(crate) fn default_forza_brake_endstop_force() -> f64 {
    forza_tuning::brake::default().endstop_force
}

pub(crate) fn default_forza_brake_endstop_boost() -> f64 {
    forza_tuning::brake::default().endstop_boost
}

pub(crate) fn default_forza_brake_wall_position() -> f64 {
    forza_tuning::brake::default().wall_position
}

pub(crate) fn default_forza_brake_guard_min_end() -> f64 {
    forza_tuning::brake::default().guard_min_end
}

pub(crate) fn default_forza_brake_full_force_at() -> f64 {
    forza_tuning::brake::default().full_force_at
}

pub(crate) fn default_forza_brake_ramp_curve() -> f64 {
    forza_tuning::brake::default().ramp_curve
}

impl Default for ForzaAbsTuningConfig {
    fn default() -> Self {
        forza_tuning::abs::default()
    }
}

impl ForzaAbsTuningConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::abs::normalize(self)
    }
}

pub(crate) fn default_forza_abs_mode() -> String {
    forza_tuning::abs::default().mode
}

pub(crate) fn default_forza_abs_slip_source() -> String {
    forza_tuning::abs::default().slip_source
}

pub(crate) fn default_forza_abs_slip_threshold() -> f64 {
    forza_tuning::abs::default().slip_threshold
}

pub(crate) fn default_forza_abs_brake_threshold_ratio() -> f64 {
    forza_tuning::abs::default().brake_threshold_ratio
}

pub(crate) fn default_forza_abs_min_speed_kmh() -> f64 {
    forza_tuning::abs::default().min_speed_kmh
}

pub(crate) fn default_forza_abs_min_strength() -> f64 {
    forza_tuning::abs::default().min_strength
}

pub(crate) fn default_forza_abs_max_strength() -> f64 {
    forza_tuning::abs::default().max_strength
}

pub(crate) fn default_forza_abs_frequency_hz() -> f64 {
    forza_tuning::abs::default().frequency_hz
}

pub(crate) fn default_forza_abs_curve() -> f64 {
    forza_tuning::abs::default().curve
}

impl Default for ForzaThrottleTuningConfig {
    fn default() -> Self {
        forza_tuning::throttle::default()
    }
}

impl ForzaThrottleTuningConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::throttle::normalize(self)
    }
}

pub(crate) fn default_forza_throttle_baseline_force() -> f64 {
    forza_tuning::throttle::default().baseline_force
}

pub(crate) fn default_forza_throttle_normal_force() -> f64 {
    forza_tuning::throttle::default().normal_force
}

pub(crate) fn default_forza_throttle_endstop_force() -> f64 {
    forza_tuning::throttle::default().endstop_force
}

pub(crate) fn default_forza_throttle_endstop_boost() -> f64 {
    forza_tuning::throttle::default().endstop_boost
}

pub(crate) fn default_forza_throttle_wall_position() -> f64 {
    forza_tuning::throttle::default().wall_position
}

pub(crate) fn default_forza_throttle_guard_min_end() -> f64 {
    forza_tuning::throttle::default().guard_min_end
}

pub(crate) fn default_forza_throttle_ramp_width() -> f64 {
    forza_tuning::throttle::default().ramp_width
}

pub(crate) fn default_forza_throttle_ramp_curve() -> f64 {
    forza_tuning::throttle::default().ramp_curve
}

impl Default for ForzaShiftTuningConfig {
    fn default() -> Self {
        forza_tuning::shift::default()
    }
}

impl ForzaShiftTuningConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::shift::normalize(self)
    }
}

pub(crate) fn default_forza_shift_wall_form_at() -> f64 {
    forza_tuning::shift::default().wall_form_at
}

pub(crate) fn default_forza_shift_frequency_hz() -> f64 {
    forza_tuning::shift::default().frequency_hz
}

pub(crate) fn default_forza_shift_wall_zones() -> f64 {
    forza_tuning::shift::default().wall_zones
}

pub(crate) fn default_forza_shift_body_low_weight() -> f64 {
    forza_tuning::shift::default().body_low_weight
}

pub(crate) fn default_forza_shift_body_high_weight() -> f64 {
    forza_tuning::shift::default().body_high_weight
}

pub(crate) fn default_forza_shift_clutch_mode() -> String {
    forza_tuning::shift::default().clutch_mode
}

pub(crate) fn default_forza_shift_clutch_threshold() -> f64 {
    forza_tuning::shift::default().clutch_threshold
}

pub(crate) fn default_forza_shift_with_clutch_strength() -> f64 {
    forza_tuning::shift::default().with_clutch_strength
}

pub(crate) fn default_forza_shift_without_clutch_strength() -> f64 {
    forza_tuning::shift::default().without_clutch_strength
}

pub(crate) fn default_forza_shift_with_clutch_duration_ms() -> f64 {
    forza_tuning::shift::default().with_clutch_duration_ms
}

pub(crate) fn default_forza_shift_without_clutch_duration_ms() -> f64 {
    forza_tuning::shift::default().without_clutch_duration_ms
}

pub(crate) fn default_forza_shift_clutch_body_cut() -> f64 {
    forza_tuning::shift::default().clutch_body_cut
}

impl Default for ForzaRevLimiterTuningConfig {
    fn default() -> Self {
        forza_tuning::rev_limiter::default()
    }
}

impl ForzaRevLimiterTuningConfig {
    pub(crate) fn normalized(self) -> Self {
        forza_tuning::rev_limiter::normalize(self)
    }
}

pub(crate) fn default_forza_rev_limiter_threshold_ratio() -> f64 {
    forza_tuning::rev_limiter::default().threshold_ratio
}

pub(crate) fn default_forza_rev_limiter_min_strength() -> f64 {
    forza_tuning::rev_limiter::default().min_strength
}

pub(crate) fn default_forza_rev_limiter_max_strength() -> f64 {
    forza_tuning::rev_limiter::default().max_strength
}

pub(crate) fn default_forza_rev_limiter_frequency_hz() -> f64 {
    forza_tuning::rev_limiter::default().frequency_hz
}

pub(crate) fn default_forza_rev_limiter_wall_form_throttle_at() -> f64 {
    forza_tuning::rev_limiter::default().wall_form_throttle_at
}

pub(crate) fn default_forza_rev_limiter_wall_zones() -> f64 {
    forza_tuning::rev_limiter::default().wall_zones
}

pub(crate) fn default_forza_rev_limiter_curve() -> f64 {
    forza_tuning::rev_limiter::default().curve
}

pub(crate) fn default_forza_rev_limiter_body_low_weight() -> f64 {
    forza_tuning::rev_limiter::default().body_low_weight
}

pub(crate) fn default_forza_rev_limiter_body_high_weight() -> f64 {
    forza_tuning::rev_limiter::default().body_high_weight
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
    forza_tuning::default_body_rumble_mode()
}

pub(crate) fn default_forza_effect(id: &str) -> ForzaEffectConfig {
    forza_tuning::default_effect(id)
}

#[cfg(test)]
pub(crate) fn default_forza_effect_configs() -> Vec<ForzaEffectConfig> {
    forza_tuning::default_effect_configs()
}

pub(crate) fn forza_effect_routes() -> &'static [&'static str] {
    forza_tuning::effect_routes()
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
            // DSCC always starts listeners for built-in UDP adapters, so they
            // are enabled out of the box until the user disables them.
            let enabled = adapter.enabled_by_default
                || built_in_udp_adapters()
                    .iter()
                    .any(|udp_adapter| udp_adapter.id == adapter.id);
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

pub(crate) fn adapters_with_persisted_state(
    persisted: &BTreeMap<String, PersistedAdapterState>,
) -> Vec<AdapterSummary> {
    let mut adapters = default_adapters();
    for adapter in &mut adapters {
        if let Some(saved) = persisted.get(&adapter.id) {
            if adapter.enabled != saved.enabled {
                adapter.enabled = saved.enabled;
                if let Some(built_in) = adapter_by_id(&adapter.id) {
                    adapter.state =
                        adapter_state_label(&initial_detection(built_in, saved.enabled))
                            .to_string();
                }
            }
        }
    }
    adapters
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
