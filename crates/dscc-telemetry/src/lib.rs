//! Normalized telemetry types and adapter contracts for DualSense Command Center.
//!
//! Adapters publish small, typed updates into a common signal namespace. The
//! runtime can fold those updates into a [`SignalSnapshot`] and pass the
//! snapshot to the effect engine without knowing which game or source produced
//! the values.

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A normalized telemetry signal name such as `input.brake`.
///
/// The constructor enforces the public naming convention used by first-party
/// adapters: lowercase path segments separated by dots. Adapter-specific debug
/// values may still use the `raw.<adapter_id>.*` namespace if needed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignalName(String);

impl SignalName {
    /// Creates a signal name after validating the normalized dotted path.
    pub fn new(value: impl Into<String>) -> Result<Self, SignalNameError> {
        let value = value.into();
        validate_signal_name(&value)?;
        Ok(Self(value))
    }

    /// Creates a signal name without validation.
    ///
    /// This is intended for static, audited constants and tests. Dynamic input
    /// should use [`SignalName::new`].
    pub const fn new_unchecked(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SignalName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SignalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<SignalName> for String {
    fn from(value: SignalName) -> Self {
        value.0
    }
}

impl TryFrom<&str> for SignalName {
    type Error = SignalNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

fn validate_signal_name(value: &str) -> Result<(), SignalNameError> {
    if value.is_empty() {
        return Err(SignalNameError::Empty);
    }

    for segment in value.split('.') {
        if segment.is_empty() {
            return Err(SignalNameError::EmptySegment);
        }

        let valid = segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if !valid {
            return Err(SignalNameError::InvalidCharacters(value.to_owned()));
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SignalNameError {
    #[error("signal name cannot be empty")]
    Empty,
    #[error("signal name cannot contain empty path segments")]
    EmptySegment,
    #[error("signal name contains unsupported characters: {0}")]
    InvalidCharacters(String),
}

/// The type family for a normalized signal value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Number,
    Bool,
    Text,
}

/// A normalized telemetry value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SignalValue {
    Number(f64),
    Bool(bool),
    Text(String),
}

impl SignalValue {
    pub fn kind(&self) -> SignalKind {
        match self {
            SignalValue::Number(_) => SignalKind::Number,
            SignalValue::Bool(_) => SignalKind::Bool,
            SignalValue::Text(_) => SignalKind::Text,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            SignalValue::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SignalValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            SignalValue::Text(value) => Some(value),
            _ => None,
        }
    }
}

impl From<f64> for SignalValue {
    fn from(value: f64) -> Self {
        SignalValue::Number(value)
    }
}

impl From<bool> for SignalValue {
    fn from(value: bool) -> Self {
        SignalValue::Bool(value)
    }
}

impl From<String> for SignalValue {
    fn from(value: String) -> Self {
        SignalValue::Text(value)
    }
}

impl From<&str> for SignalValue {
    fn from(value: &str) -> Self {
        SignalValue::Text(value.to_owned())
    }
}

/// A single normalized signal write from an adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalUpdate {
    pub name: SignalName,
    pub value: SignalValue,
    /// Monotonic sequence assigned by the adapter when available.
    pub sequence: Option<u64>,
}

impl SignalUpdate {
    pub fn new(name: SignalName, value: impl Into<SignalValue>) -> Self {
        Self {
            name,
            value: value.into(),
            sequence: None,
        }
    }

    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }
}

/// The latest known values for a telemetry source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalSnapshot {
    signals: BTreeMap<SignalName, SignalValue>,
    last_sequence: Option<u64>,
}

impl SignalSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_updates(updates: impl IntoIterator<Item = SignalUpdate>) -> Self {
        let mut snapshot = Self::new();
        snapshot.apply_updates(updates);
        snapshot
    }

    pub fn apply_update(&mut self, update: SignalUpdate) {
        if let Some(sequence) = update.sequence {
            self.last_sequence = Some(
                self.last_sequence
                    .map_or(sequence, |last| last.max(sequence)),
            );
        }
        self.signals.insert(update.name, update.value);
    }

    pub fn apply_updates(&mut self, updates: impl IntoIterator<Item = SignalUpdate>) {
        for update in updates {
            self.apply_update(update);
        }
    }

    pub fn get(&self, name: &SignalName) -> Option<&SignalValue> {
        self.signals.get(name)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&SignalValue> {
        self.signals.get(name)
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        self.get_by_name(name).and_then(SignalValue::as_number)
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get_by_name(name).and_then(SignalValue::as_bool)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.get_by_name(name).and_then(SignalValue::as_text)
    }

    pub fn signals(&self) -> &BTreeMap<SignalName, SignalValue> {
        &self.signals
    }

    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
}

/// Adapter features advertised to the agent and UI.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    pub udp_listener: bool,
    pub shared_memory: bool,
    pub requires_setup: bool,
    pub supports_auto_detect: bool,
    pub packet_formats: Vec<String>,
}

/// User-configurable adapter settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub enabled: bool,
    pub auto_detect: bool,
    pub bind_address: Option<String>,
    pub port: Option<u16>,
    pub packet_format: Option<String>,
    pub setup_url: Option<String>,
    pub setup_text: Option<String>,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: true,
            bind_address: None,
            port: None,
            packet_format: None,
            setup_url: None,
            setup_text: None,
        }
    }
}

/// Current adapter detection or runtime state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterDetection {
    Unavailable { reason: Option<String> },
    NeedsSetup { instructions: Option<String> },
    Ready,
    Running,
    Faulted { reason: String },
}

/// A UI/API friendly status record for one telemetry adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdapterStatus {
    pub id: String,
    pub display_name: String,
    pub detection: AdapterDetection,
    pub enabled: bool,
    pub packet_rate_hz: Option<f64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("adapter is disabled")]
    Disabled,
    #[error("adapter configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("adapter I/O failed: {0}")]
    Io(String),
    #[error("adapter failed: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_signal_names() {
        assert!(SignalName::new("").is_err());
        assert!(SignalName::new("input..brake").is_err());
        assert!(SignalName::new("Input.Brake").is_err());
    }

    #[test]
    fn folds_updates_into_snapshot() {
        let updates = vec![
            SignalUpdate::new(SignalName::new("input.brake").unwrap(), 0.25).with_sequence(7),
            SignalUpdate::new(SignalName::new("source.connected").unwrap(), true).with_sequence(8),
        ];

        let snapshot = SignalSnapshot::from_updates(updates);

        assert_eq!(snapshot.number("input.brake"), Some(0.25));
        assert_eq!(snapshot.bool("source.connected"), Some(true));
        assert_eq!(snapshot.last_sequence(), Some(8));
    }

    #[test]
    fn lookup_by_str_matches_signal_name_lookup_for_value_kinds() {
        let number_name = SignalName::new("input.brake").unwrap();
        let bool_name = SignalName::new("source.connected").unwrap();
        let text_name = SignalName::new("session.mode").unwrap();
        let snapshot = SignalSnapshot::from_updates(vec![
            SignalUpdate::new(number_name.clone(), 0.25),
            SignalUpdate::new(bool_name.clone(), true),
            SignalUpdate::new(text_name.clone(), "race"),
        ]);

        assert_eq!(
            snapshot.get_by_name(number_name.as_str()),
            snapshot.get(&number_name)
        );
        assert_eq!(snapshot.number(number_name.as_str()), Some(0.25));

        assert_eq!(
            snapshot.get_by_name(bool_name.as_str()),
            snapshot.get(&bool_name)
        );
        assert_eq!(snapshot.bool(bool_name.as_str()), Some(true));

        assert_eq!(
            snapshot.get_by_name(text_name.as_str()),
            snapshot.get(&text_name)
        );
        assert_eq!(snapshot.text(text_name.as_str()), Some("race"));
    }

    #[test]
    fn lookup_by_str_returns_none_for_missing_names() {
        let snapshot = SignalSnapshot::from_updates(vec![SignalUpdate::new(
            SignalName::new("input.brake").unwrap(),
            0.25,
        )]);

        assert_eq!(snapshot.get_by_name("input.throttle"), None);
        assert_eq!(snapshot.number("input.throttle"), None);
        assert_eq!(snapshot.bool("input.throttle"), None);
        assert_eq!(snapshot.text("input.throttle"), None);
    }
}
