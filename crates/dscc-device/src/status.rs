use std::fmt;

/// Stable runtime identifier assigned by DSCC.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerId(String);

impl ControllerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ControllerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque transport-local identifier for a raw HID record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawDeviceId(String);

impl RawDeviceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn from_stable_source(source: &str) -> Self {
        Self(redacted_hash(source))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RawDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Redacted hint for diagnostics and sanitized listings.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DevicePathHint {
    backend_path_hash: String,
}

impl DevicePathHint {
    pub fn from_backend_path(path: &str) -> Self {
        Self {
            backend_path_hash: redacted_hash(path),
        }
    }

    pub fn from_hash(hash: impl Into<String>) -> Self {
        Self {
            backend_path_hash: hash.into(),
        }
    }

    pub fn backend_path_hash(&self) -> &str {
        &self.backend_path_hash
    }
}

impl fmt::Display for DevicePathHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hid-path-hash:{}", self.backend_path_hash)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceFamily {
    DualSense,
    DualSenseEdge,
    UnknownSony,
    Unknown,
}

impl DeviceFamily {
    pub fn is_supported_controller(self) -> bool {
        matches!(self, Self::DualSense | Self::DualSenseEdge)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceTransportKind {
    Usb,
    Bluetooth,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BatteryState {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatteryInfo {
    pub percent: Option<u8>,
    pub state: BatteryState,
}

impl BatteryInfo {
    pub const UNKNOWN: Self = Self {
        percent: None,
        state: BatteryState::Unknown,
    };

    pub fn new(percent: Option<u8>, state: BatteryState) -> Self {
        Self {
            percent: percent.map(|value| value.min(100)),
            state,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerCapabilities {
    pub adaptive_triggers: bool,
    pub lightbar: bool,
    pub player_leds: bool,
    pub rumble: bool,
    pub microphone_led: bool,
    pub edge_buttons: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerInfo {
    pub id: ControllerId,
    pub raw_device_id: RawDeviceId,
    pub path_hint: DevicePathHint,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub family: DeviceFamily,
    pub transport: DeviceTransportKind,
    pub capabilities: ControllerCapabilities,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerState {
    pub id: ControllerId,
    pub connection: ConnectionState,
    pub battery: BatteryInfo,
}

pub(crate) fn redacted_hash(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_hint_does_not_expose_backend_path() {
        let raw_path = "/dev/hidraw7/device-with-serial";
        let hint = DevicePathHint::from_backend_path(raw_path);

        assert_ne!(hint.backend_path_hash(), raw_path);
        assert!(!hint.to_string().contains(raw_path));
    }

    #[test]
    fn battery_percent_is_clamped_to_valid_range() {
        let battery = BatteryInfo::new(Some(250), BatteryState::Charging);

        assert_eq!(battery.percent, Some(100));
        assert_eq!(battery.state, BatteryState::Charging);
    }
}
