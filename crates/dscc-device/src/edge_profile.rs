use crate::{
    error::DeviceError,
    status::DeviceTransportKind,
    transport::{DeviceHandle, DeviceTransport},
};

const EDGE_PROFILE_REPORT_LEN: usize = 64;
const EDGE_PROFILE_PAYLOAD_LEN: usize = EDGE_PROFILE_REPORT_LEN;
const EDGE_PROFILE_CHECKSUM_INPUT_LEN: usize = 170;
const EDGE_PROFILE_DATA_CHECKSUM_OFFSET: usize = 56;
const EDGE_PROFILE_READ_PAYLOAD_LEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeOnboardSlotId {
    Default,
    Square,
    Cross,
    Circle,
}

impl EdgeOnboardSlotId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Square => "square",
            Self::Cross => "cross",
            Self::Circle => "circle",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Default => "Fn + Triangle",
            Self::Square => "Fn + Square",
            Self::Cross => "Fn + Cross",
            Self::Circle => "Fn + Circle",
        }
    }

    pub fn read_report_ids(self) -> [u8; 3] {
        let start = match self {
            Self::Default => 0x70,
            Self::Square => 0x73,
            Self::Cross => 0x76,
            Self::Circle => 0x79,
        };
        [start, start + 1, start + 2]
    }

    pub fn write_report_id(self) -> Option<u8> {
        match self {
            Self::Default => None,
            Self::Square => Some(0x60),
            Self::Cross => Some(0x61),
            Self::Circle => Some(0x62),
        }
    }

    pub fn write_ack_report_id(self) -> Option<u8> {
        match self {
            Self::Default => None,
            Self::Square => Some(0x63),
            Self::Cross => Some(0x64),
            Self::Circle => Some(0x65),
        }
    }

    fn from_read_selector(value: u8) -> Option<Self> {
        match value {
            0x70 => Some(Self::Default),
            0x73 => Some(Self::Square),
            0x76 => Some(Self::Cross),
            0x79 => Some(Self::Circle),
            _ => None,
        }
    }

    fn selector_for_write(self) -> u8 {
        self.write_report_id().unwrap_or(0x63)
    }

    pub fn assignable(self) -> bool {
        self.write_report_id().is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeProfileIntensity {
    Off,
    Weak,
    Medium,
    Strong,
}

impl EdgeProfileIntensity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Weak => "Weak",
            Self::Medium => "Medium",
            Self::Strong => "Strong",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Self::Off,
            "weak" | "low" => Self::Weak,
            "medium" => Self::Medium,
            _ => Self::Strong,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeTriggerDeadzone {
    pub left: [u8; 2],
    pub right: [u8; 2],
    pub unified: bool,
}

impl Default for EdgeTriggerDeadzone {
    fn default() -> Self {
        Self {
            left: [0, 100],
            right: [0, 100],
            unified: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeStickPreset {
    Default,
    Quick,
    Precise,
    Steady,
    Digital,
    Dynamic,
    Custom,
}

impl EdgeStickPreset {
    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "quick" => Self::Quick,
            "precise" | "precision" => Self::Precise,
            "steady" => Self::Steady,
            "digital" => Self::Digital,
            "dynamic" => Self::Dynamic,
            _ => Self::Default,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Quick => "Quick",
            Self::Precise => "Precise",
            Self::Steady => "Steady",
            Self::Digital => "Digital",
            Self::Dynamic => "Dynamic",
            Self::Custom => "Custom",
        }
    }

    fn from_byte(value: u8) -> Self {
        match value {
            0x01 => Self::Quick,
            0x02 => Self::Precise,
            0x03 => Self::Steady,
            0x04 => Self::Digital,
            0x05 => Self::Dynamic,
            0xff => Self::Custom,
            _ => Self::Default,
        }
    }

    fn to_byte(self) -> u8 {
        match self {
            Self::Default => 0x00,
            Self::Quick => 0x01,
            Self::Precise => 0x02,
            Self::Steady => 0x03,
            Self::Digital => 0x04,
            Self::Dynamic => 0x05,
            Self::Custom => 0xff,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeStickProfile {
    pub preset: EdgeStickPreset,
    pub curve_points: [u8; 8],
}

impl Default for EdgeStickProfile {
    fn default() -> Self {
        Self {
            preset: EdgeStickPreset::Default,
            curve_points: [0, 0, 128, 128, 196, 196, 225, 225],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeButton {
    Up,
    Left,
    Down,
    Right,
    Circle,
    Cross,
    Square,
    Triangle,
    R1,
    R2,
    R3,
    L1,
    L2,
    L3,
    BackLeft,
    BackRight,
    Options,
    Touchpad,
}

impl EdgeButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Up => "D-pad Up",
            Self::Left => "D-pad Left",
            Self::Down => "D-pad Down",
            Self::Right => "D-pad Right",
            Self::Circle => "Circle",
            Self::Cross => "Cross",
            Self::Square => "Square",
            Self::Triangle => "Triangle",
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::BackLeft => "Back Left",
            Self::BackRight => "Back Right",
            Self::Options => "Options",
            Self::Touchpad => "Touchpad",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        let normalized: String = value
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        match normalized.as_str() {
            "up" | "dpadup" | "dpadnorth" => Some(Self::Up),
            "left" | "dpadleft" | "dpadwest" => Some(Self::Left),
            "down" | "dpaddown" | "dpadsouth" => Some(Self::Down),
            "right" | "dpadright" | "dpadeast" => Some(Self::Right),
            "circle" => Some(Self::Circle),
            "cross" | "x" => Some(Self::Cross),
            "square" => Some(Self::Square),
            "triangle" => Some(Self::Triangle),
            "r1" | "rightbumper" => Some(Self::R1),
            "r2" | "righttrigger" => Some(Self::R2),
            "r3" | "rightstick" => Some(Self::R3),
            "l1" | "leftbumper" => Some(Self::L1),
            "l2" | "lefttrigger" => Some(Self::L2),
            "l3" | "leftstick" => Some(Self::L3),
            "backleft" | "paddleleft" | "edgebackleft" => Some(Self::BackLeft),
            "backright" | "paddleright" | "edgebackright" => Some(Self::BackRight),
            "options" | "start" => Some(Self::Options),
            "touchpad" | "touchpadpress" | "trackpad" | "trackpadpress" => Some(Self::Touchpad),
            _ => None,
        }
    }

    fn from_mapping_code(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Up),
            0x01 => Some(Self::Left),
            0x02 => Some(Self::Down),
            0x03 => Some(Self::Right),
            0x04 => Some(Self::Circle),
            0x05 => Some(Self::Cross),
            0x06 => Some(Self::Square),
            0x07 => Some(Self::Triangle),
            0x08 => Some(Self::R1),
            0x09 => Some(Self::R2),
            0x0a => Some(Self::R3),
            0x0b => Some(Self::L1),
            0x0c => Some(Self::L2),
            0x0d => Some(Self::L3),
            0x0e => Some(Self::BackLeft),
            0x0f => Some(Self::BackRight),
            0x10 => Some(Self::Options),
            0x11 => Some(Self::Touchpad),
            _ => None,
        }
    }

    fn mapping_code(self) -> u8 {
        match self {
            Self::Up => 0x00,
            Self::Left => 0x01,
            Self::Down => 0x02,
            Self::Right => 0x03,
            Self::Circle => 0x04,
            Self::Cross => 0x05,
            Self::Square => 0x06,
            Self::Triangle => 0x07,
            Self::R1 => 0x08,
            Self::R2 => 0x09,
            Self::R3 => 0x0a,
            Self::L1 => 0x0b,
            Self::L2 => 0x0c,
            Self::L3 => 0x0d,
            Self::BackLeft => 0x0e,
            Self::BackRight => 0x0f,
            Self::Options => 0x10,
            Self::Touchpad => 0x11,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeButtonMapping {
    pub source: EdgeButton,
    pub target: EdgeButton,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeOnboardProfile {
    pub slot: EdgeOnboardSlotId,
    pub assigned: bool,
    pub name: String,
    pub trigger_deadzone: EdgeTriggerDeadzone,
    pub left_stick: EdgeStickProfile,
    pub right_stick: EdgeStickProfile,
    pub vibration_intensity: EdgeProfileIntensity,
    pub trigger_effect_intensity: EdgeProfileIntensity,
    pub button_mappings: Vec<EdgeButtonMapping>,
    pub updated_at_ms: u64,
}

impl EdgeOnboardProfile {
    pub fn new(slot: EdgeOnboardSlotId, name: impl Into<String>) -> Self {
        Self {
            slot,
            assigned: true,
            name: name.into(),
            trigger_deadzone: EdgeTriggerDeadzone::default(),
            left_stick: EdgeStickProfile::default(),
            right_stick: EdgeStickProfile::default(),
            vibration_intensity: EdgeProfileIntensity::Strong,
            trigger_effect_intensity: EdgeProfileIntensity::Strong,
            button_mappings: default_button_mappings().to_vec(),
            updated_at_ms: 0,
        }
    }
}

pub fn default_button_mappings() -> [EdgeButtonMapping; 16] {
    [
        identity(EdgeButton::Up),
        identity(EdgeButton::Left),
        identity(EdgeButton::Down),
        identity(EdgeButton::Right),
        identity(EdgeButton::Circle),
        identity(EdgeButton::Cross),
        identity(EdgeButton::Square),
        identity(EdgeButton::Triangle),
        identity(EdgeButton::R1),
        identity(EdgeButton::R2),
        identity(EdgeButton::R3),
        identity(EdgeButton::L1),
        identity(EdgeButton::L2),
        identity(EdgeButton::L3),
        identity(EdgeButton::BackLeft),
        identity(EdgeButton::BackRight),
    ]
}

const fn identity(button: EdgeButton) -> EdgeButtonMapping {
    EdgeButtonMapping {
        source: button,
        target: button,
    }
}

pub fn read_edge_onboard_profiles<T: DeviceTransport>(
    transport: &T,
    target_id: &crate::status::RawDeviceId,
    transport_kind: DeviceTransportKind,
) -> Result<Vec<EdgeOnboardProfile>, DeviceError> {
    if transport_kind != DeviceTransportKind::Usb {
        return Err(DeviceError::TransportFault(
            "DualSense Edge onboard profile reads require a USB connection".to_string(),
        ));
    }

    let mut handle = transport.open(target_id)?;
    read_edge_onboard_profiles_from_handle(handle.as_mut())
}

pub fn write_edge_onboard_profile<T: DeviceTransport>(
    transport: &T,
    target_id: &crate::status::RawDeviceId,
    transport_kind: DeviceTransportKind,
    profile: &EdgeOnboardProfile,
) -> Result<(), DeviceError> {
    if transport_kind != DeviceTransportKind::Usb {
        return Err(DeviceError::TransportFault(
            "DualSense Edge onboard profile writes require a USB connection".to_string(),
        ));
    }

    let mut handle = transport.open(target_id)?;
    write_edge_onboard_profile_to_handle(handle.as_mut(), profile)
}

pub fn read_edge_onboard_profiles_from_handle(
    handle: &mut dyn DeviceHandle,
) -> Result<Vec<EdgeOnboardProfile>, DeviceError> {
    [
        EdgeOnboardSlotId::Default,
        EdgeOnboardSlotId::Square,
        EdgeOnboardSlotId::Cross,
        EdgeOnboardSlotId::Circle,
    ]
    .into_iter()
    .map(|slot| {
        let reports = slot.read_report_ids();
        let first = handle.receive_feature_report(reports[0], EDGE_PROFILE_READ_PAYLOAD_LEN)?;
        let second = handle.receive_feature_report(reports[1], EDGE_PROFILE_READ_PAYLOAD_LEN)?;
        let third = handle.receive_feature_report(reports[2], EDGE_PROFILE_READ_PAYLOAD_LEN)?;
        decode_edge_onboard_profile([&first, &second, &third])
    })
    .collect()
}

pub fn write_edge_onboard_profile_to_handle(
    handle: &mut dyn DeviceHandle,
    profile: &EdgeOnboardProfile,
) -> Result<(), DeviceError> {
    let report_id = profile.slot.write_report_id().ok_or_else(|| {
        DeviceError::TransportFault(
            "the default Fn + Triangle profile cannot be overwritten".to_string(),
        )
    })?;
    let reports = encode_edge_onboard_profile(profile)?;
    for report in reports {
        let written = handle.send_feature_report(report_id, &report)?;
        if written < EDGE_PROFILE_PAYLOAD_LEN {
            return Err(DeviceError::TransportFault(format!(
                "short DualSense Edge profile feature report write: expected {EDGE_PROFILE_PAYLOAD_LEN} bytes, wrote {written}"
            )));
        }
    }
    if let Some(ack_report_id) = profile.slot.write_ack_report_id() {
        let _ = handle.receive_feature_report(ack_report_id, EDGE_PROFILE_READ_PAYLOAD_LEN);
    }
    Ok(())
}

pub fn decode_edge_onboard_profile(reports: [&[u8]; 3]) -> Result<EdgeOnboardProfile, DeviceError> {
    if reports
        .iter()
        .any(|report| report.len() < EDGE_PROFILE_REPORT_LEN)
    {
        return Err(DeviceError::TransportFault(
            "DualSense Edge profile feature report was shorter than 64 bytes".to_string(),
        ));
    }

    let slot = EdgeOnboardSlotId::from_read_selector(reports[0][0]).ok_or_else(|| {
        DeviceError::TransportFault("unknown DualSense Edge profile slot selector".to_string())
    })?;
    let assigned = reports[0][1] != 0x10;
    let name = if assigned {
        decode_profile_name(reports[0], reports[1])
    } else {
        String::new()
    };
    let unified = (reports[2][31] >> 7) & 1 == 1;
    let trigger_deadzone = EdgeTriggerDeadzone {
        left: [decode_percent(reports[2][4]), decode_percent(reports[2][5])],
        right: [decode_percent(reports[2][6]), decode_percent(reports[2][7])],
        unified,
    };

    Ok(EdgeOnboardProfile {
        slot,
        assigned,
        name,
        trigger_deadzone,
        left_stick: EdgeStickProfile {
            preset: EdgeStickPreset::from_byte(reports[2][30]),
            curve_points: reports[1][45..53].try_into().unwrap_or_default(),
        },
        right_stick: EdgeStickProfile {
            preset: EdgeStickPreset::from_byte(reports[2][32]),
            curve_points: [
                reports[1][54],
                reports[1][55],
                reports[1][56],
                reports[1][57],
                reports[1][58],
                reports[1][59],
                reports[2][2],
                reports[2][3],
            ],
        },
        vibration_intensity: decode_vibration_intensity(reports[2][8]),
        trigger_effect_intensity: decode_trigger_effect_intensity(reports[2][9]),
        button_mappings: decode_button_mappings(&reports[2][10..26]),
        updated_at_ms: decode_u48_le(&reports[2][34..40]),
    })
}

pub fn encode_edge_onboard_profile(
    profile: &EdgeOnboardProfile,
) -> Result<[[u8; EDGE_PROFILE_REPORT_LEN]; 3], DeviceError> {
    if !profile.slot.assignable() {
        return Err(DeviceError::TransportFault(
            "the default Fn + Triangle profile cannot be overwritten".to_string(),
        ));
    }

    let mut reports = [[0u8; EDGE_PROFILE_REPORT_LEN]; 3];
    let selector = profile.slot.selector_for_write();
    reports[0][0] = selector;
    reports[1][0] = selector;
    reports[2][0] = selector;
    reports[0][2] = 0x01;
    reports[1][1] = 0x01;
    reports[2][1] = 0x02;

    let (first_report, remaining_reports) = reports.split_at_mut(1);
    encode_profile_name(
        &profile.name,
        &mut first_report[0],
        &mut remaining_reports[0],
    );
    reports[2][4] = encode_percent(profile.trigger_deadzone.left[0]);
    reports[2][5] = encode_percent(profile.trigger_deadzone.left[1]);
    reports[2][6] = encode_percent(profile.trigger_deadzone.right[0]);
    reports[2][7] = encode_percent(profile.trigger_deadzone.right[1]);
    if profile.trigger_deadzone.unified {
        reports[2][31] |= 1 << 7;
    }

    reports[2][8] = encode_vibration_intensity(profile.vibration_intensity);
    reports[2][9] = encode_trigger_effect_intensity(profile.trigger_effect_intensity);
    reports[1][44] = 4;
    reports[1][53] = 4;
    reports[1][45..53].copy_from_slice(&profile.left_stick.curve_points);
    reports[1][54..60].copy_from_slice(&profile.right_stick.curve_points[..6]);
    reports[2][2] = profile.right_stick.curve_points[6];
    reports[2][3] = profile.right_stick.curve_points[7];
    reports[2][30] = profile.left_stick.preset.to_byte();
    reports[2][32] = profile.right_stick.preset.to_byte();
    encode_button_mappings(&profile.button_mappings, &mut reports[2]);
    encode_u48_le(profile.updated_at_ms, &mut reports[2][34..40]);
    reports[2][60..64].copy_from_slice(&[0, 0, 0, 0]);

    fill_profile_checksum(&mut reports);
    Ok(reports)
}

fn decode_profile_name(first: &[u8], second: &[u8]) -> String {
    let mut bytes = [0u8; 80];
    bytes[..54].copy_from_slice(&first[6..60]);
    bytes[54..].copy_from_slice(&second[2..28]);
    let mut units = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    String::from_utf16_lossy(&units).trim().to_string()
}

fn encode_profile_name(name: &str, first: &mut [u8], second: &mut [u8]) {
    let mut bytes = [0u8; 80];
    for (index, unit) in name.trim().encode_utf16().take(40).enumerate() {
        let offset = index * 2;
        bytes[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    first[6..60].copy_from_slice(&bytes[..54]);
    second[2..28].copy_from_slice(&bytes[54..]);
}

fn decode_button_mappings(bytes: &[u8]) -> Vec<EdgeButtonMapping> {
    default_button_mappings()
        .into_iter()
        .zip(bytes.iter().copied())
        .map(|(mapping, target)| EdgeButtonMapping {
            source: mapping.source,
            target: EdgeButton::from_mapping_code(target).unwrap_or(mapping.source),
        })
        .collect()
}

fn encode_button_mappings(mappings: &[EdgeButtonMapping], report: &mut [u8]) {
    let mut bytes = [0u8; 20];
    bytes[..16].copy_from_slice(&[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ]);
    bytes[18] = 0xc0;
    for mapping in mappings {
        let source = mapping.source.mapping_code();
        if source < 16 {
            bytes[source as usize] = mapping.target.mapping_code();
        }
    }
    report[10..30].copy_from_slice(&bytes);
}

fn decode_percent(value: u8) -> u8 {
    ((u16::from(value) * 100 + 127) / 255).min(100) as u8
}

fn encode_percent(value: u8) -> u8 {
    ((u16::from(value.min(100)) * 255 + 50) / 100) as u8
}

fn decode_vibration_intensity(value: u8) -> EdgeProfileIntensity {
    match value {
        0xff => EdgeProfileIntensity::Off,
        0x03 => EdgeProfileIntensity::Weak,
        0x02 => EdgeProfileIntensity::Medium,
        _ => EdgeProfileIntensity::Strong,
    }
}

fn encode_vibration_intensity(value: EdgeProfileIntensity) -> u8 {
    match value {
        EdgeProfileIntensity::Off => 0xff,
        EdgeProfileIntensity::Weak => 0x03,
        EdgeProfileIntensity::Medium => 0x02,
        EdgeProfileIntensity::Strong => 0x00,
    }
}

fn decode_trigger_effect_intensity(value: u8) -> EdgeProfileIntensity {
    match value {
        0xff => EdgeProfileIntensity::Off,
        0x09 => EdgeProfileIntensity::Weak,
        0x06 => EdgeProfileIntensity::Medium,
        _ => EdgeProfileIntensity::Strong,
    }
}

fn encode_trigger_effect_intensity(value: EdgeProfileIntensity) -> u8 {
    match value {
        EdgeProfileIntensity::Off => 0xff,
        EdgeProfileIntensity::Weak => 0x09,
        EdgeProfileIntensity::Medium => 0x06,
        EdgeProfileIntensity::Strong => 0x00,
    }
}

fn encode_u48_le(value: u64, out: &mut [u8]) {
    let bytes = value.to_le_bytes();
    out[..6].copy_from_slice(&bytes[..6]);
}

fn decode_u48_le(bytes: &[u8]) -> u64 {
    let mut padded = [0u8; 8];
    padded[..6].copy_from_slice(&bytes[..6]);
    u64::from_le_bytes(padded)
}

fn fill_profile_checksum(reports: &mut [[u8; EDGE_PROFILE_REPORT_LEN]; 3]) {
    let mut checksum_input = [0u8; EDGE_PROFILE_CHECKSUM_INPUT_LEN];
    checksum_input[0..58].copy_from_slice(&reports[0][2..60]);
    checksum_input[58..116].copy_from_slice(&reports[1][2..60]);
    checksum_input[116..170].copy_from_slice(&reports[2][2..56]);
    let crc = crc32_le(&checksum_input);
    reports[2][EDGE_PROFILE_DATA_CHECKSUM_OFFSET..EDGE_PROFILE_DATA_CHECKSUM_OFFSET + 4]
        .copy_from_slice(&crc.to_le_bytes());
}

fn crc32_le(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xedb8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_profile_round_trips_typed_settings() {
        let mut profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Road Tune");
        profile.trigger_deadzone = EdgeTriggerDeadzone {
            left: [5, 95],
            right: [4, 100],
            unified: false,
        };
        profile.vibration_intensity = EdgeProfileIntensity::Medium;
        profile.trigger_effect_intensity = EdgeProfileIntensity::Weak;
        profile.button_mappings = vec![EdgeButtonMapping {
            source: EdgeButton::BackLeft,
            target: EdgeButton::L1,
        }];
        profile.updated_at_ms = 1_779_400_000_000;

        let encoded = encode_edge_onboard_profile(&profile).unwrap();
        assert_eq!(encoded[0][0], 0x60);
        assert_eq!(encoded[2][10 + 14], 0x0b);
        assert_ne!(
            &encoded[2][EDGE_PROFILE_DATA_CHECKSUM_OFFSET..EDGE_PROFILE_DATA_CHECKSUM_OFFSET + 4],
            &[0, 0, 0, 0]
        );

        let mut read_encoded = encoded;
        read_encoded[0][0] = 0x73;
        read_encoded[1][0] = 0x73;
        read_encoded[2][0] = 0x73;
        let decoded =
            decode_edge_onboard_profile([&read_encoded[0], &read_encoded[1], &read_encoded[2]])
                .unwrap();

        assert_eq!(decoded.slot, EdgeOnboardSlotId::Square);
        assert_eq!(decoded.name, "Road Tune");
        assert_eq!(decoded.trigger_deadzone.left, [5, 95]);
        assert_eq!(decoded.vibration_intensity, EdgeProfileIntensity::Medium);
        assert_eq!(decoded.trigger_effect_intensity, EdgeProfileIntensity::Weak);
        assert!(decoded
            .button_mappings
            .iter()
            .any(|mapping| mapping.source == EdgeButton::BackLeft
                && mapping.target == EdgeButton::L1));
    }

    #[test]
    fn default_slot_cannot_be_encoded_for_write() {
        let profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Default, "Default");
        assert!(encode_edge_onboard_profile(&profile).is_err());
    }
}
