use super::*;

/// Latched shift/clutch/suspension state for the Forza Runtime Live Effects.
/// Pure decision logic: telemetry signals in, latched effect events out.
#[derive(Debug, Clone, Default)]
pub(crate) struct ForzaEffectRuntime {
    prev_shift_gear: Option<u8>,
    latched_shift_event: Option<&'static str>,
    latched_shift_pulse: f64,
    latched_shift_until: Option<Instant>,
    clutch_seen: bool,
    prev_suspension_impact: f64,
    latched_suspension_impact: f64,
    latched_suspension_impact_until: Option<Instant>,
}

impl ForzaEffectRuntime {
    fn latch_shift_event(
        &mut self,
        event: &'static str,
        pulse: f64,
        duration: Duration,
        now: Instant,
    ) {
        if event == "none" {
            return;
        }

        self.latched_shift_event = Some(event);
        self.latched_shift_pulse = clamp_unit(pulse);
        self.latched_shift_until = Some(now + duration);
    }

    pub(crate) fn detect_shift_event(
        &mut self,
        current_gear: Option<f64>,
        clutch: Option<f64>,
        telemetry_on: bool,
        shift_enabled: bool,
        shift_tuning: &ForzaShiftTuningConfig,
        now: Instant,
    ) -> Option<&'static str> {
        if !telemetry_on || !shift_enabled {
            return Some("none");
        }

        let shift_tuning = shift_tuning.clone().normalized();
        let clutch = clutch.map(clamp_unit);
        if clutch.is_some_and(|value| value >= shift_tuning.clutch_threshold) {
            self.clutch_seen = true;
        }

        let current_gear = signal_gear_to_u8(current_gear?)?;
        let event = match self.prev_shift_gear {
            Some(previous_gear) if previous_gear != current_gear => {
                self.shift_event_for_clutch(clutch, &shift_tuning)
            }
            _ => "none",
        };

        self.prev_shift_gear = Some(current_gear);
        let (pulse, duration) = self.shift_pulse_and_duration(event, &shift_tuning);
        self.latch_shift_event(event, pulse, duration, now);
        Some(event)
    }

    pub(crate) fn latched_shift_event(&self, now: Instant) -> Option<&'static str> {
        self.latched_shift_event
            .filter(|_| self.latched_shift_until.is_some_and(|until| now < until))
    }

    pub(crate) fn latched_shift_pulse(&self, now: Instant) -> f64 {
        self.latched_shift_event(now)
            .map(|_| clamp_unit(self.latched_shift_pulse))
            .unwrap_or_default()
    }

    fn shift_event_for_clutch(
        &self,
        clutch: Option<f64>,
        shift_tuning: &ForzaShiftTuningConfig,
    ) -> &'static str {
        match shift_tuning.clutch_mode.as_str() {
            "off" => "shift",
            "manual_clutch" => clutch_quality_event(clutch, shift_tuning.clutch_threshold),
            _ if self.clutch_seen => clutch_quality_event(clutch, shift_tuning.clutch_threshold),
            _ => "shift",
        }
    }

    fn shift_pulse_and_duration(
        &self,
        event: &str,
        shift_tuning: &ForzaShiftTuningConfig,
    ) -> (f64, Duration) {
        match event {
            "smooth_shift" => (
                shift_tuning.with_clutch_strength,
                duration_from_millis(shift_tuning.with_clutch_duration_ms),
            ),
            "rough_shift" => (
                shift_tuning.without_clutch_strength,
                duration_from_millis(shift_tuning.without_clutch_duration_ms),
            ),
            _ => (1.0, FORZA_SHIFT_EVENT_HOLD),
        }
    }

    fn latch_suspension_impact(&mut self, strength: f64, now: Instant) {
        self.latched_suspension_impact = clamp_unit(strength);
        self.latched_suspension_impact_until = Some(now + FORZA_SUSPENSION_IMPACT_HOLD);
    }

    pub(crate) fn detect_suspension_impact(
        &mut self,
        suspension_travel: Option<f64>,
        acceleration_magnitude: Option<f64>,
        speed_kmh: Option<f64>,
        telemetry_on: bool,
        impact_enabled: bool,
        now: Instant,
    ) -> f64 {
        if !telemetry_on || !impact_enabled {
            self.prev_suspension_impact = 0.0;
            self.latched_suspension_impact = 0.0;
            self.latched_suspension_impact_until = None;
            return 0.0;
        }

        let impact =
            suspension_impact_strength(suspension_travel, acceleration_magnitude, speed_kmh);
        let rising_impact = impact >= FORZA_SUSPENSION_IMPACT_TRIGGER_AT
            && self.prev_suspension_impact <= FORZA_SUSPENSION_IMPACT_RESET_AT;
        self.prev_suspension_impact = impact;

        if rising_impact
            || (self
                .latched_suspension_impact_until
                .is_some_and(|until| now < until)
                && impact > self.latched_suspension_impact)
        {
            self.latch_suspension_impact(impact, now);
        }

        self.latched_suspension_impact(now)
    }

    pub(crate) fn latched_suspension_impact(&self, now: Instant) -> f64 {
        if self
            .latched_suspension_impact_until
            .is_some_and(|until| now < until)
        {
            self.latched_suspension_impact
        } else {
            0.0
        }
    }
}

fn clutch_quality_event(clutch: Option<f64>, threshold: f64) -> &'static str {
    if clutch.unwrap_or_default() >= threshold {
        "smooth_shift"
    } else {
        "rough_shift"
    }
}

fn duration_from_millis(ms: f64) -> Duration {
    Duration::from_millis(ms.round().clamp(1.0, u64::MAX as f64) as u64)
}
