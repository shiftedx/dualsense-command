//! Core domain model and deterministic effect evaluation for DualSense Command Center.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub mod input_bridge;

use dscc_telemetry::SignalSnapshot;

/// Stable runtime identifier for a controller.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControllerId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerFamily {
    DualSense,
    DualSenseEdge,
    UnknownSony,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerTransportKind {
    Usb,
    Bluetooth,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerCapabilities {
    pub adaptive_triggers: bool,
    pub lightbar: bool,
    pub player_leds: bool,
    pub rumble: bool,
    pub microphone_led: bool,
    pub edge_buttons: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerInfo {
    pub id: ControllerId,
    pub vendor_id: u16,
    pub product_id: u16,
    pub family: ControllerFamily,
    pub transport: ControllerTransportKind,
    pub connection: ConnectionState,
    pub capabilities: ControllerCapabilities,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerState {
    pub id: ControllerId,
    pub connection: ConnectionState,
    pub battery_percent: Option<u8>,
    pub battery_state: BatteryState,
}

/// Profile-level policy for standard rumble writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RumblePolicy {
    TriggerOverlay,
    FullControl,
    Disabled,
}

/// A user or built-in profile containing declarative effect rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub rumble_policy: RumblePolicy,
    pub rules: Vec<EffectRule>,
}

impl Profile {
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: 1,
            rumble_policy: RumblePolicy::TriggerOverlay,
            rules: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectRule {
    pub id: String,
    pub target: EffectTarget,
    pub priority: i32,
    pub condition: RuleCondition,
    pub effect: EffectTemplate,
    /// Optional exponential low-pass filter applied to the rule's primary
    /// numeric source. Skipped on the first evaluation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smoothing: Option<Smoothing>,
    /// Optional hysteresis on the rule's primary numeric source. While the
    /// rule is "inactive" because of hysteresis, the engine falls through to
    /// the next-priority rule for the same target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hysteresis: Option<Hysteresis>,
    /// Optional stale-telemetry fallback. If the rule has not produced an
    /// active effect within `stale_after_ms`, `fallback` is emitted instead
    /// of the rule's normal effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<TimeoutFallback>,
}

/// Exponential low-pass filter parameters for a rule's primary numeric source.
///
/// `alpha = 1 - exp(-dt_ms / time_constant_ms)` and
/// `filtered = prev + alpha * (raw - prev)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Smoothing {
    pub time_constant_ms: u32,
}

/// Schmitt-trigger style hysteresis on the rule's primary numeric source.
///
/// The rule is considered "active" only after the value crosses `enter`,
/// and stays active until the value falls below `exit`. If `enter <= exit`
/// the band collapses to a single hard threshold at `enter`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hysteresis {
    pub enter: f64,
    pub exit: f64,
}

/// Stale-telemetry fallback. If the rule has not been "fresh" for at least
/// `stale_after_ms`, the engine substitutes `fallback` for the rule's effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeoutFallback {
    pub stale_after_ms: u32,
    pub fallback: EffectTemplate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTarget {
    L2,
    R2,
    Lightbar,
    PlayerLeds,
    Rumble,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleCondition {
    Always,
    Signal {
        signal: String,
        op: ComparisonOp,
        value: ComparableValue,
    },
    All {
        conditions: Vec<RuleCondition>,
    },
    Any {
        conditions: Vec<RuleCondition>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOp {
    Eq,
    NotEq,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ComparableValue {
    Number(f64),
    Bool(bool),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EffectTemplate {
    Off,
    AdaptiveResistance {
        start_position: ValueSource,
        strength: ValueSource,
    },
    Pulse {
        amplitude: ValueSource,
        frequency_hz: ValueSource,
    },
    PulseAb {
        strength: ValueSource,
        frequency_hz: ValueSource,
        wall_zones: ValueSource,
    },
    Wall {
        position: ValueSource,
        strength: ValueSource,
    },
    Lightbar {
        color: RgbColor,
        brightness: ValueSource,
    },
    PlayerLeds {
        count: ValueSource,
    },
    Rumble {
        low_frequency: ValueSource,
        high_frequency: ValueSource,
    },
}

/// A numeric value in an effect, either fixed or scaled from a signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueSource {
    Constant {
        value: f64,
    },
    SignalScale {
        signal: String,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
        clamp: bool,
    },
    SignalCurve {
        signal: String,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
        exponent: f64,
        clamp: bool,
    },
    SignalPoints {
        signal: String,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
        points: Vec<ValuePoint>,
        clamp: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValuePoint {
    pub input: f64,
    pub output: f64,
}

impl ValueSource {
    pub fn constant(value: f64) -> Self {
        Self::Constant { value }
    }

    pub fn signal_scale(
        signal: impl Into<String>,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
    ) -> Self {
        Self::SignalScale {
            signal: signal.into(),
            input_min,
            input_max,
            output_min,
            output_max,
            clamp: true,
        }
    }

    pub fn signal_curve(
        signal: impl Into<String>,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
        exponent: f64,
    ) -> Self {
        Self::SignalCurve {
            signal: signal.into(),
            input_min,
            input_max,
            output_min,
            output_max,
            exponent,
            clamp: true,
        }
    }

    pub fn signal_points(
        signal: impl Into<String>,
        input_min: f64,
        input_max: f64,
        output_min: f64,
        output_max: f64,
        points: Vec<ValuePoint>,
    ) -> Self {
        Self::SignalPoints {
            signal: signal.into(),
            input_min,
            input_max,
            output_min,
            output_max,
            points,
            clamp: true,
        }
    }

    fn evaluate(&self, snapshot: &SignalSnapshot) -> Option<f64> {
        match self {
            ValueSource::Constant { value } => Some(*value),
            ValueSource::SignalScale {
                signal,
                input_min,
                input_max,
                output_min,
                output_max,
                clamp,
            } => {
                if input_min == input_max {
                    return None;
                }

                let input = snapshot.number(signal)?;
                let ratio = (input - input_min) / (input_max - input_min);
                let ratio = if *clamp { ratio.clamp(0.0, 1.0) } else { ratio };
                Some(output_min + (output_max - output_min) * ratio)
            }
            ValueSource::SignalCurve {
                signal,
                input_min,
                input_max,
                output_min,
                output_max,
                exponent,
                clamp,
            } => {
                if input_min == input_max || *exponent <= 0.0 {
                    return None;
                }

                let input = snapshot.number(signal)?;
                let ratio = (input - input_min) / (input_max - input_min);
                let ratio = if *clamp { ratio.clamp(0.0, 1.0) } else { ratio };
                let curved = if ratio >= 0.0 {
                    ratio.powf(*exponent)
                } else {
                    -(-ratio).powf(*exponent)
                };
                Some(output_min + (output_max - output_min) * curved)
            }
            ValueSource::SignalPoints {
                signal,
                input_min,
                input_max,
                output_min,
                output_max,
                points,
                clamp,
            } => {
                if input_min == input_max {
                    return None;
                }

                let input = snapshot.number(signal)?;
                let ratio = (input - input_min) / (input_max - input_min);
                let ratio = if *clamp { ratio.clamp(0.0, 1.0) } else { ratio };
                let curved = evaluate_value_points(points, ratio)?;
                Some(output_min + (output_max - output_min) * curved)
            }
        }
    }
}

fn evaluate_value_points(points: &[ValuePoint], input: f64) -> Option<f64> {
    if points.is_empty() || !input.is_finite() {
        return None;
    }

    let mut normalized: Vec<ValuePoint> = points
        .iter()
        .copied()
        .filter(|point| point.input.is_finite() && point.output.is_finite())
        .map(|point| ValuePoint {
            input: point.input.clamp(0.0, 1.0),
            output: point.output.clamp(0.0, 1.0),
        })
        .collect();
    if normalized.is_empty() {
        return None;
    }

    normalized.sort_by(|a, b| a.input.total_cmp(&b.input));
    normalized.dedup_by(|a, b| {
        if (a.input - b.input).abs() < f64::EPSILON {
            b.output = a.output;
            true
        } else {
            false
        }
    });

    let x = input.clamp(0.0, 1.0);
    if x <= normalized[0].input {
        return Some(normalized[0].output);
    }
    if let Some(last) = normalized.last() {
        if x >= last.input {
            return Some(last.output);
        }
    }

    for window in normalized.windows(2) {
        let left = window[0];
        let right = window[1];
        if x >= left.input && x <= right.input {
            let width = right.input - left.input;
            if width <= f64::EPSILON {
                return Some(right.output);
            }
            let ratio = (x - left.input) / width;
            return Some(left.output + (right.output - left.output) * ratio);
        }
    }

    normalized.last().map(|point| point.output)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControllerOutputFrame {
    pub l2: TriggerOutput,
    pub r2: TriggerOutput,
    pub lightbar: Option<LightbarOutput>,
    pub player_leds: Option<PlayerLedsOutput>,
    pub rumble: Option<RumbleOutput>,
}

impl Default for ControllerOutputFrame {
    fn default() -> Self {
        Self {
            l2: TriggerOutput::Off,
            r2: TriggerOutput::Off,
            lightbar: None,
            player_leds: None,
            rumble: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerOutput {
    Off,
    AdaptiveResistance {
        start_position: f64,
        strength: f64,
    },
    Pulse {
        amplitude: f64,
        frequency_hz: f64,
    },
    PulseAb {
        strength: f64,
        frequency_hz: f64,
        wall_zones: u8,
    },
    Wall {
        position: f64,
        strength: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightbarOutput {
    pub color: RgbColor,
    pub brightness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerLedsOutput {
    pub count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RumbleOutput {
    pub low_frequency: f64,
    pub high_frequency: f64,
}

/// Per-rule mutable state for temporal effects (smoothing, hysteresis,
/// timeout-fallback freshness tracking).
#[derive(Clone, Debug, Default)]
struct RuleState {
    /// Last smoothed value of the rule's primary numeric source, if any.
    smoothed: Option<f64>,
    /// Timestamp of the previous smoothing evaluation, used to compute `dt`.
    smoothed_at: Option<Instant>,
    /// Whether the rule is currently "active" under its hysteresis band.
    hysteresis_active: bool,
    /// Wall-clock instant at which this rule last produced a fresh active
    /// effect. Used by `TimeoutFallback`.
    last_fresh_at: Option<Instant>,
}

/// Deterministic evaluator for profile rules.
///
/// The engine keeps a small amount of per-rule state (keyed by `EffectRule.id`)
/// to support smoothing, hysteresis, and timeout fallback. State is wrapped
/// in a `Mutex` (which preserves `Send + Sync`) so callers can use the
/// convenient `evaluate(&self, ...)` entry point, while
/// `evaluate_at(&mut self, ...)` offers an explicit `Instant` for
/// deterministic testing.
#[derive(Debug, Default)]
pub struct EffectEngine {
    state: Mutex<BTreeMap<String, RuleState>>,
}

impl Clone for EffectEngine {
    fn clone(&self) -> Self {
        let snapshot = self
            .state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        Self {
            state: Mutex::new(snapshot),
        }
    }
}

impl EffectEngine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(BTreeMap::new()),
        }
    }

    /// Convenience entry point. Uses `Instant::now()` as the evaluation time
    /// and the engine's internal state map.
    pub fn evaluate(&self, profile: &Profile, snapshot: &SignalSnapshot) -> ControllerOutputFrame {
        self.evaluate_inner(profile, snapshot, Instant::now())
    }

    /// Explicit-time entry point. Required for deterministic tests of
    /// smoothing and timeout behaviour, and preferred for production
    /// callers that already track frame times.
    pub fn evaluate_at(
        &mut self,
        profile: &Profile,
        snapshot: &SignalSnapshot,
        now: Instant,
    ) -> ControllerOutputFrame {
        self.evaluate_inner(profile, snapshot, now)
    }

    fn evaluate_inner(
        &self,
        profile: &Profile,
        snapshot: &SignalSnapshot,
        now: Instant,
    ) -> ControllerOutputFrame {
        let mut frame = ControllerOutputFrame::default();
        // If the mutex was poisoned by a prior panic mid-evaluation, recover
        // the inner map. The engine's state is purely advisory (smoothing
        // history etc.) so dropping a partial mutation is safe.
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for target in [
            EffectTarget::L2,
            EffectTarget::R2,
            EffectTarget::Lightbar,
            EffectTarget::PlayerLeds,
            EffectTarget::Rumble,
        ] {
            let Some(rule) = pick_rule_for_target(profile, snapshot, target, &mut state, now)
            else {
                continue;
            };

            // Choose effect: stale -> fallback, otherwise the rule's normal
            // effect. Freshness is updated by `pick_rule_for_target` whenever
            // the rule's primary source resolves to a finite value.
            let effect = match rule.timeout.as_ref() {
                Some(timeout) if is_stale(state.get(&rule.id), now, timeout.stale_after_ms) => {
                    &timeout.fallback
                }
                _ => &rule.effect,
            };

            apply_rule(
                &mut frame,
                target,
                effect,
                profile.rumble_policy,
                snapshot,
                state.get_mut(&rule.id),
                now,
                rule.smoothing,
            );
        }

        frame
    }
}

/// Returns whether the rule's last "fresh" timestamp is older than
/// `stale_after_ms` relative to `now`. A rule that has never been fresh is
/// also considered stale.
fn is_stale(state: Option<&RuleState>, now: Instant, stale_after_ms: u32) -> bool {
    let limit = Duration::from_millis(stale_after_ms as u64);
    match state.and_then(|s| s.last_fresh_at) {
        Some(ts) => now.duration_since(ts) >= limit,
        None => true,
    }
}

/// Choose the rule that "wins" for `target`, considering both rule
/// conditions and hysteresis activation. Rules whose hysteresis band is
/// currently inactive are skipped exactly as if their condition returned
/// `false`, allowing the next-priority rule to take over.
///
/// This also performs the hysteresis state update for every candidate it
/// touches (so rules that lose the priority race still track their band).
fn pick_rule_for_target<'a>(
    profile: &'a Profile,
    snapshot: &SignalSnapshot,
    target: EffectTarget,
    state: &mut BTreeMap<String, RuleState>,
    now: Instant,
) -> Option<&'a EffectRule> {
    let mut candidates: Vec<&EffectRule> = profile
        .rules
        .iter()
        .filter(|rule| rule.target == target && condition_matches(&rule.condition, snapshot))
        .collect();

    // Sort by (priority desc, id desc) — matches the previous max_by_key
    // ordering. We then walk in order so hysteresis can demote a high
    // priority rule to inactive without losing the next candidate.
    candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| b.id.cmp(&a.id)));

    for rule in candidates {
        let raw = primary_source(&rule.effect).and_then(|src| src.evaluate(snapshot));

        // Update hysteresis activation if configured. A rule with no
        // primary source (e.g. EffectTemplate::Off) or with a missing
        // signal cannot trigger hysteresis, so we treat it as inactive
        // for hysteresis purposes — the rule still wins if it has no
        // hysteresis configured at all.
        let hysteresis_ok = match (rule.hysteresis, raw) {
            (None, _) => true,
            (Some(band), Some(value)) => {
                let entry = state.entry(rule.id.clone()).or_default();
                entry.hysteresis_active = update_hysteresis(entry.hysteresis_active, band, value);
                entry.hysteresis_active
            }
            (Some(_), None) => {
                // Source unavailable: keep prior activation state but treat
                // as inactive for this tick so we can fall through.
                false
            }
        };

        if !hysteresis_ok {
            continue;
        }

        // Refresh "last fresh" timestamp whenever the primary source
        // resolves to a finite number — that's the engine's signal that
        // upstream telemetry is alive.
        if let Some(value) = raw {
            if value.is_finite() {
                let entry = state.entry(rule.id.clone()).or_default();
                entry.last_fresh_at = Some(now);
            }
        }

        return Some(rule);
    }

    None
}

/// Returns the new hysteresis-active state given the previous state, the
/// configured band, and the current raw value. If `enter <= exit` the band
/// collapses to a single hard threshold at `enter`.
fn update_hysteresis(was_active: bool, band: Hysteresis, value: f64) -> bool {
    if band.enter <= band.exit {
        return value >= band.enter;
    }
    if was_active {
        value > band.exit
    } else {
        value >= band.enter
    }
}

/// The "primary" numeric source of an effect template — the source that is
/// smoothed and gates hysteresis. We pick the most semantically meaningful
/// channel per effect:
///   * AdaptiveResistance / Wall: `strength`
///   * Pulse: `amplitude`
///   * PulseAb: `strength`
///   * Lightbar: `brightness`
///   * PlayerLeds: `count`
///   * Rumble: `low_frequency`
///   * Off: none
fn primary_source(effect: &EffectTemplate) -> Option<&ValueSource> {
    match effect {
        EffectTemplate::Off => None,
        EffectTemplate::AdaptiveResistance { strength, .. } => Some(strength),
        EffectTemplate::Pulse { amplitude, .. } => Some(amplitude),
        EffectTemplate::PulseAb { strength, .. } => Some(strength),
        EffectTemplate::Wall { strength, .. } => Some(strength),
        EffectTemplate::Lightbar { brightness, .. } => Some(brightness),
        EffectTemplate::PlayerLeds { count } => Some(count),
        EffectTemplate::Rumble { low_frequency, .. } => Some(low_frequency),
    }
}

fn condition_matches(condition: &RuleCondition, snapshot: &SignalSnapshot) -> bool {
    match condition {
        RuleCondition::Always => true,
        RuleCondition::Signal { signal, op, value } => compare_signal(snapshot, signal, *op, value),
        RuleCondition::All { conditions } => conditions
            .iter()
            .all(|condition| condition_matches(condition, snapshot)),
        RuleCondition::Any { conditions } => conditions
            .iter()
            .any(|condition| condition_matches(condition, snapshot)),
    }
}

fn compare_signal(
    snapshot: &SignalSnapshot,
    signal: &str,
    op: ComparisonOp,
    expected: &ComparableValue,
) -> bool {
    match expected {
        ComparableValue::Number(expected) => snapshot
            .number(signal)
            .is_some_and(|actual| compare_numbers(actual, op, *expected)),
        ComparableValue::Bool(expected) => snapshot
            .bool(signal)
            .is_some_and(|actual| compare_ord(actual, op, *expected)),
        ComparableValue::Text(expected) => snapshot
            .text(signal)
            .is_some_and(|actual| compare_ord(actual, op, expected.as_str())),
    }
}

fn compare_numbers(actual: f64, op: ComparisonOp, expected: f64) -> bool {
    match op {
        ComparisonOp::Eq => actual == expected,
        ComparisonOp::NotEq => actual != expected,
        ComparisonOp::GreaterThan => actual > expected,
        ComparisonOp::GreaterOrEqual => actual >= expected,
        ComparisonOp::LessThan => actual < expected,
        ComparisonOp::LessOrEqual => actual <= expected,
    }
}

fn compare_ord<T: PartialOrd + PartialEq>(actual: T, op: ComparisonOp, expected: T) -> bool {
    match op {
        ComparisonOp::Eq => actual == expected,
        ComparisonOp::NotEq => actual != expected,
        ComparisonOp::GreaterThan => actual > expected,
        ComparisonOp::GreaterOrEqual => actual >= expected,
        ComparisonOp::LessThan => actual < expected,
        ComparisonOp::LessOrEqual => actual <= expected,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_rule(
    frame: &mut ControllerOutputFrame,
    target: EffectTarget,
    effect: &EffectTemplate,
    rumble_policy: RumblePolicy,
    snapshot: &SignalSnapshot,
    rule_state: Option<&mut RuleState>,
    now: Instant,
    smoothing: Option<Smoothing>,
) {
    let mut resolved = evaluate_effect(effect, snapshot);

    // Apply smoothing to the resolved effect's primary numeric channel.
    if let (Some(smoothing), Some(state), Some(resolved_ref)) =
        (smoothing, rule_state, resolved.as_mut())
    {
        if let Some(raw) = resolved_primary(resolved_ref) {
            let smoothed = apply_smoothing(state, smoothing, raw, now);
            set_resolved_primary(resolved_ref, smoothed);
        }
    }

    match (target, resolved) {
        (EffectTarget::L2, Some(ResolvedEffect::Trigger(trigger))) => frame.l2 = trigger,
        (EffectTarget::R2, Some(ResolvedEffect::Trigger(trigger))) => frame.r2 = trigger,
        (EffectTarget::Lightbar, Some(ResolvedEffect::Lightbar(lightbar))) => {
            frame.lightbar = Some(lightbar);
        }
        (EffectTarget::PlayerLeds, Some(ResolvedEffect::PlayerLeds(player_leds))) => {
            frame.player_leds = Some(player_leds);
        }
        (EffectTarget::Rumble, Some(ResolvedEffect::Rumble(rumble)))
            if rumble_policy == RumblePolicy::FullControl =>
        {
            frame.rumble = Some(rumble);
        }
        (EffectTarget::Rumble, Some(ResolvedEffect::Rumble(_))) => {}
        (_, Some(ResolvedEffect::Off)) => match target {
            EffectTarget::L2 => frame.l2 = TriggerOutput::Off,
            EffectTarget::R2 => frame.r2 = TriggerOutput::Off,
            EffectTarget::Lightbar => frame.lightbar = None,
            EffectTarget::PlayerLeds => frame.player_leds = None,
            EffectTarget::Rumble => frame.rumble = None,
        },
        _ => {}
    }
}

/// Read the "primary" numeric value from a resolved effect — must mirror
/// `primary_source` in choice of channel.
fn resolved_primary(resolved: &ResolvedEffect) -> Option<f64> {
    match resolved {
        ResolvedEffect::Off => None,
        ResolvedEffect::Trigger(TriggerOutput::Off) => None,
        ResolvedEffect::Trigger(TriggerOutput::AdaptiveResistance { strength, .. }) => {
            Some(*strength)
        }
        ResolvedEffect::Trigger(TriggerOutput::Pulse { amplitude, .. }) => Some(*amplitude),
        ResolvedEffect::Trigger(TriggerOutput::PulseAb { strength, .. }) => Some(*strength),
        ResolvedEffect::Trigger(TriggerOutput::Wall { strength, .. }) => Some(*strength),
        ResolvedEffect::Lightbar(out) => Some(out.brightness),
        ResolvedEffect::PlayerLeds(out) => Some(out.count as f64),
        ResolvedEffect::Rumble(out) => Some(out.low_frequency),
    }
}

/// Write a (smoothed) primary value back into a resolved effect.
fn set_resolved_primary(resolved: &mut ResolvedEffect, value: f64) {
    match resolved {
        ResolvedEffect::Off => {}
        ResolvedEffect::Trigger(TriggerOutput::Off) => {}
        ResolvedEffect::Trigger(TriggerOutput::AdaptiveResistance { strength, .. }) => {
            *strength = normalized(value);
        }
        ResolvedEffect::Trigger(TriggerOutput::Pulse { amplitude, .. }) => {
            *amplitude = normalized(value);
        }
        ResolvedEffect::Trigger(TriggerOutput::PulseAb { strength, .. }) => {
            *strength = normalized(value);
        }
        ResolvedEffect::Trigger(TriggerOutput::Wall { strength, .. }) => {
            *strength = normalized(value);
        }
        ResolvedEffect::Lightbar(out) => {
            out.brightness = normalized(value);
        }
        ResolvedEffect::PlayerLeds(out) => {
            out.count = value.round().clamp(0.0, 5.0) as u8;
        }
        ResolvedEffect::Rumble(out) => {
            out.low_frequency = normalized(value);
        }
    }
}

/// Exponential low-pass filter step. Returns the smoothed value and
/// updates the per-rule state in place. The very first call seeds the
/// filter with `raw` and returns `raw` unchanged.
fn apply_smoothing(state: &mut RuleState, smoothing: Smoothing, raw: f64, now: Instant) -> f64 {
    let tc_ms = smoothing.time_constant_ms.max(1) as f64;

    match (state.smoothed, state.smoothed_at) {
        (Some(prev), Some(prev_at)) => {
            let dt_ms = now.duration_since(prev_at).as_secs_f64() * 1000.0;
            let alpha = 1.0 - (-dt_ms / tc_ms).exp();
            let alpha = alpha.clamp(0.0, 1.0);
            let next = prev + alpha * (raw - prev);
            state.smoothed = Some(next);
            state.smoothed_at = Some(now);
            next
        }
        _ => {
            state.smoothed = Some(raw);
            state.smoothed_at = Some(now);
            raw
        }
    }
}

enum ResolvedEffect {
    Off,
    Trigger(TriggerOutput),
    Lightbar(LightbarOutput),
    PlayerLeds(PlayerLedsOutput),
    Rumble(RumbleOutput),
}

fn evaluate_effect(effect: &EffectTemplate, snapshot: &SignalSnapshot) -> Option<ResolvedEffect> {
    match effect {
        EffectTemplate::Off => Some(ResolvedEffect::Off),
        EffectTemplate::AdaptiveResistance {
            start_position,
            strength,
        } => Some(ResolvedEffect::Trigger(TriggerOutput::AdaptiveResistance {
            start_position: normalized(start_position.evaluate(snapshot)?),
            strength: normalized(strength.evaluate(snapshot)?),
        })),
        EffectTemplate::Pulse {
            amplitude,
            frequency_hz,
        } => Some(ResolvedEffect::Trigger(TriggerOutput::Pulse {
            amplitude: normalized(amplitude.evaluate(snapshot)?),
            frequency_hz: non_negative(frequency_hz.evaluate(snapshot)?),
        })),
        EffectTemplate::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => Some(ResolvedEffect::Trigger(TriggerOutput::PulseAb {
            strength: normalized(strength.evaluate(snapshot)?),
            frequency_hz: non_negative(frequency_hz.evaluate(snapshot)?),
            wall_zones: wall_zones.evaluate(snapshot)?.round().clamp(1.0, 9.0) as u8,
        })),
        EffectTemplate::Wall { position, strength } => {
            Some(ResolvedEffect::Trigger(TriggerOutput::Wall {
                position: normalized(position.evaluate(snapshot)?),
                strength: normalized(strength.evaluate(snapshot)?),
            }))
        }
        EffectTemplate::Lightbar { color, brightness } => {
            Some(ResolvedEffect::Lightbar(LightbarOutput {
                color: *color,
                brightness: normalized(brightness.evaluate(snapshot)?),
            }))
        }
        EffectTemplate::PlayerLeds { count } => {
            Some(ResolvedEffect::PlayerLeds(PlayerLedsOutput {
                count: count.evaluate(snapshot)?.round().clamp(0.0, 5.0) as u8,
            }))
        }
        EffectTemplate::Rumble {
            low_frequency,
            high_frequency,
        } => Some(ResolvedEffect::Rumble(RumbleOutput {
            low_frequency: normalized(low_frequency.evaluate(snapshot)?),
            high_frequency: normalized(high_frequency.evaluate(snapshot)?),
        })),
    }
}

fn normalized(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dscc_telemetry::{SignalName, SignalUpdate};

    #[test]
    fn maps_brake_and_throttle_to_trigger_resistance() {
        let profile = sample_driving_profile();
        let snapshot = SignalSnapshot::from_updates([
            SignalUpdate::new(SignalName::new("input.brake").unwrap(), 0.75),
            SignalUpdate::new(SignalName::new("input.throttle").unwrap(), 0.50),
        ]);

        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        assert_trigger_resistance(&frame.l2, 0.20, 0.775);
        assert_trigger_resistance(&frame.r2, 0.10, 0.45);
        assert_eq!(frame.rumble, None);
    }

    #[test]
    fn signal_points_interpolate_custom_trigger_response() {
        let profile = Profile {
            id: "point-curve".to_string(),
            name: "Point Curve".to_string(),
            version: 1,
            rumble_policy: RumblePolicy::Disabled,
            rules: vec![EffectRule {
                id: "r2-points".to_string(),
                target: EffectTarget::R2,
                priority: 10,
                condition: RuleCondition::Always,
                effect: EffectTemplate::AdaptiveResistance {
                    start_position: ValueSource::constant(0.0),
                    strength: ValueSource::signal_points(
                        "input.throttle",
                        0.0,
                        1.0,
                        0.10,
                        0.90,
                        vec![
                            ValuePoint {
                                input: 0.0,
                                output: 0.0,
                            },
                            ValuePoint {
                                input: 0.50,
                                output: 0.20,
                            },
                            ValuePoint {
                                input: 0.75,
                                output: 0.80,
                            },
                            ValuePoint {
                                input: 1.0,
                                output: 1.0,
                            },
                        ],
                    ),
                },
                smoothing: None,
                hysteresis: None,
                timeout: None,
            }],
        };
        let snapshot = SignalSnapshot::from_updates([SignalUpdate::new(
            SignalName::new("input.throttle").unwrap(),
            0.625,
        )]);

        let frame = EffectEngine::new().evaluate(&profile, &snapshot);

        assert_trigger_resistance(&frame.r2, 0.0, 0.50);
    }

    #[test]
    fn falls_back_to_off_when_required_signal_is_missing() {
        let profile = sample_driving_profile();
        let frame = EffectEngine::new().evaluate(&profile, &SignalSnapshot::new());

        assert_eq!(frame.l2, TriggerOutput::Off);
        assert_eq!(frame.r2, TriggerOutput::Off);
    }

    fn sample_driving_profile() -> Profile {
        Profile {
            id: "sample-driving".to_owned(),
            name: "Sample Driving".to_owned(),
            version: 1,
            rumble_policy: RumblePolicy::TriggerOverlay,
            rules: vec![
                EffectRule {
                    id: "brake-resistance".to_owned(),
                    target: EffectTarget::L2,
                    priority: 10,
                    condition: RuleCondition::Always,
                    effect: EffectTemplate::AdaptiveResistance {
                        start_position: ValueSource::constant(0.20),
                        strength: ValueSource::signal_scale("input.brake", 0.0, 1.0, 0.10, 1.0),
                    },
                    smoothing: None,
                    hysteresis: None,
                    timeout: None,
                },
                EffectRule {
                    id: "throttle-resistance".to_owned(),
                    target: EffectTarget::R2,
                    priority: 10,
                    condition: RuleCondition::Always,
                    effect: EffectTemplate::AdaptiveResistance {
                        start_position: ValueSource::constant(0.10),
                        strength: ValueSource::signal_scale("input.throttle", 0.0, 1.0, 0.20, 0.70),
                    },
                    smoothing: None,
                    hysteresis: None,
                    timeout: None,
                },
            ],
        }
    }

    fn assert_trigger_resistance(trigger: &TriggerOutput, start_position: f64, strength: f64) {
        match trigger {
            TriggerOutput::AdaptiveResistance {
                start_position: actual_start,
                strength: actual_strength,
            } => {
                assert!((actual_start - start_position).abs() < f64::EPSILON);
                assert!((actual_strength - strength).abs() < 0.000_001);
            }
            other => panic!("expected adaptive resistance, got {other:?}"),
        }
    }

    fn throttle_snapshot(value: f64) -> SignalSnapshot {
        SignalSnapshot::from_updates([SignalUpdate::new(
            SignalName::new("input.throttle").unwrap(),
            value,
        )])
    }

    fn single_rule_profile(rule: EffectRule) -> Profile {
        Profile {
            id: "temporal-test".to_owned(),
            name: "Temporal Test".to_owned(),
            version: 1,
            rumble_policy: RumblePolicy::TriggerOverlay,
            rules: vec![rule],
        }
    }

    fn resistance_strength(trigger: &TriggerOutput) -> f64 {
        match trigger {
            TriggerOutput::AdaptiveResistance { strength, .. } => *strength,
            other => panic!("expected adaptive resistance, got {other:?}"),
        }
    }

    #[test]
    fn smoothing_low_passes_step_change() {
        let rule = EffectRule {
            id: "smoothed-throttle".to_owned(),
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(0.0),
                strength: ValueSource::signal_scale("input.throttle", 0.0, 1.0, 0.0, 1.0),
            },
            smoothing: Some(Smoothing {
                time_constant_ms: 100,
            }),
            hysteresis: None,
            timeout: None,
        };
        let profile = single_rule_profile(rule);
        let mut engine = EffectEngine::new();

        let t0 = Instant::now();
        let frame0 = engine.evaluate_at(&profile, &throttle_snapshot(1.0), t0);
        // First sample seeds the filter, so the smoothed value equals raw.
        assert!((resistance_strength(&frame0.r2) - 1.0).abs() < 1e-9);

        let t1 = t0 + Duration::from_millis(50);
        let frame1 = engine.evaluate_at(&profile, &throttle_snapshot(0.0), t1);
        let smoothed = resistance_strength(&frame1.r2);

        // alpha = 1 - exp(-50/100) ~= 0.3935 => filtered = 1 + 0.3935*(0-1) ~= 0.6065
        assert!(
            smoothed > 0.0 && smoothed < 1.0,
            "smoothed value {smoothed} should be strictly between 0 and 1"
        );
        // Bracket around the analytic expectation (~0.6065). The task asks
        // for "closer to 0.4 ish" measured from below (i.e. 1 - 0.6 ≈ 0.4
        // worth of decay) — assert generously around the math.
        assert!(
            (0.55..=0.66).contains(&smoothed),
            "smoothed value {smoothed} outside expected band [0.55, 0.66]"
        );
    }

    #[test]
    fn hysteresis_gates_rule_activation() {
        // Two rules on R2: the high-priority one has hysteresis on a
        // 0.7/0.3 band; the low-priority fallback is always-on Off, so we
        // can observe whether the gated rule "fires" by checking for the
        // adaptive resistance vs. an Off trigger.
        let gated = EffectRule {
            id: "gated-throttle".to_owned(),
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(0.0),
                strength: ValueSource::signal_scale("input.throttle", 0.0, 1.0, 0.0, 1.0),
            },
            smoothing: None,
            hysteresis: Some(Hysteresis {
                enter: 0.7,
                exit: 0.3,
            }),
            timeout: None,
        };
        let fallback = EffectRule {
            id: "off-fallback".to_owned(),
            target: EffectTarget::R2,
            priority: 0,
            condition: RuleCondition::Always,
            effect: EffectTemplate::Off,
            smoothing: None,
            hysteresis: None,
            timeout: None,
        };
        let profile = Profile {
            id: "hyst".to_owned(),
            name: "Hyst".to_owned(),
            version: 1,
            rumble_policy: RumblePolicy::TriggerOverlay,
            rules: vec![gated, fallback],
        };
        let mut engine = EffectEngine::new();
        let base = Instant::now();

        // 0.5 — below enter, inactive, fallback Off wins.
        let f = engine.evaluate_at(&profile, &throttle_snapshot(0.5), base);
        assert_eq!(
            f.r2,
            TriggerOutput::Off,
            "0.5 below enter should be inactive"
        );

        // 0.8 — crosses enter, gated rule activates.
        let f = engine.evaluate_at(
            &profile,
            &throttle_snapshot(0.8),
            base + Duration::from_millis(10),
        );
        assert!(
            matches!(f.r2, TriggerOutput::AdaptiveResistance { .. }),
            "0.8 should activate the gated rule, got {:?}",
            f.r2
        );

        // 0.5 — between exit and enter, should stay active (latched).
        let f = engine.evaluate_at(
            &profile,
            &throttle_snapshot(0.5),
            base + Duration::from_millis(20),
        );
        assert!(
            matches!(f.r2, TriggerOutput::AdaptiveResistance { .. }),
            "0.5 inside band should remain active, got {:?}",
            f.r2
        );

        // 0.2 — falls below exit, deactivates.
        let f = engine.evaluate_at(
            &profile,
            &throttle_snapshot(0.2),
            base + Duration::from_millis(30),
        );
        assert_eq!(f.r2, TriggerOutput::Off, "0.2 below exit should deactivate");
    }

    #[test]
    fn timeout_substitutes_fallback_when_stale() {
        let rule = EffectRule {
            id: "stale-throttle".to_owned(),
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(0.0),
                strength: ValueSource::signal_scale("input.throttle", 0.0, 1.0, 0.0, 1.0),
            },
            smoothing: None,
            hysteresis: None,
            timeout: Some(TimeoutFallback {
                stale_after_ms: 500,
                fallback: EffectTemplate::Off,
            }),
        };
        let profile = single_rule_profile(rule);
        let mut engine = EffectEngine::new();
        let t0 = Instant::now();

        // Initial fresh evaluation produces the normal effect.
        let frame0 = engine.evaluate_at(&profile, &throttle_snapshot(0.6), t0);
        assert!(
            matches!(frame0.r2, TriggerOutput::AdaptiveResistance { .. }),
            "fresh evaluation should emit normal effect, got {:?}",
            frame0.r2
        );

        // Advance the clock past the staleness window. Use an empty
        // snapshot so the primary source can't resolve and freshness is
        // not refreshed. The engine should emit the fallback (Off).
        let stale_at = t0 + Duration::from_millis(1000);
        let frame1 = engine.evaluate_at(&profile, &SignalSnapshot::new(), stale_at);
        assert_eq!(
            frame1.r2,
            TriggerOutput::Off,
            "stale rule should emit fallback Off effect"
        );
    }

    #[test]
    fn evaluation_is_deterministic_for_identical_inputs() {
        // Determinism guarantee from the spec: given identical (profile,
        // snapshot stream, time stream), output must be reproducible across
        // independent engine instances.
        let rule = EffectRule {
            id: "deterministic-throttle".to_owned(),
            target: EffectTarget::R2,
            priority: 10,
            condition: RuleCondition::Always,
            effect: EffectTemplate::AdaptiveResistance {
                start_position: ValueSource::constant(0.0),
                strength: ValueSource::signal_scale("input.throttle", 0.0, 1.0, 0.0, 1.0),
            },
            smoothing: Some(Smoothing {
                time_constant_ms: 80,
            }),
            hysteresis: Some(Hysteresis {
                enter: 0.6,
                exit: 0.2,
            }),
            timeout: Some(TimeoutFallback {
                stale_after_ms: 400,
                fallback: EffectTemplate::Off,
            }),
        };
        let profile = single_rule_profile(rule);

        // Two engines, identical input sequence, identical time stream.
        let mut engine_a = EffectEngine::new();
        let mut engine_b = EffectEngine::new();
        let t0 = Instant::now();
        let times: [Duration; 4] = [
            Duration::from_millis(0),
            Duration::from_millis(33),
            Duration::from_millis(66),
            Duration::from_millis(99),
        ];
        let throttles: [f64; 4] = [0.75, 0.8, 0.45, 0.1];

        for (dt, value) in times.iter().zip(throttles.iter()) {
            let snap = throttle_snapshot(*value);
            let now = t0 + *dt;
            let frame_a = engine_a.evaluate_at(&profile, &snap, now);
            let frame_b = engine_b.evaluate_at(&profile, &snap, now);
            assert_eq!(
                frame_a, frame_b,
                "engines diverged at t={dt:?}, throttle={value}"
            );
        }
    }

    #[test]
    fn minimal_profile_json_uses_default_temporal_fields() {
        // Module/profile authors should not have to spell out temporal fields
        // for rules that do not need smoothing, hysteresis, or timeout fallback.
        let json = r#"{
            "id": "minimal",
            "name": "Minimal",
            "version": 1,
            "rumble_policy": "trigger_overlay",
            "rules": [{
                "id": "r1",
                "target": "r2",
                "priority": 5,
                "condition": { "type": "always" },
                "effect": {
                    "type": "adaptive_resistance",
                    "start_position": { "type": "constant", "value": 0.1 },
                    "strength": { "type": "constant", "value": 0.5 }
                }
            }]
        }"#;
        let profile: Profile =
            serde_json::from_str(json).expect("minimal profile must deserialize");
        let rule = &profile.rules[0];
        assert!(rule.smoothing.is_none());
        assert!(rule.hysteresis.is_none());
        assert!(rule.timeout.is_none());
    }
}
