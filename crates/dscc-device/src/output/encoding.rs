use dscc_core::{ControllerOutputFrame, PlayerLedsOutput, RumbleOutput, TriggerOutput};

use crate::{error::DeviceError, status::DeviceTransportKind};

pub(super) const USB_REPORT_ID: u8 = 0x02;
pub(super) const BT_REPORT_ID: u8 = 0x31;
pub(super) const BT_OUTPUT_TAG: u8 = 0x10;
const OUTPUT_CRC32_SEED: u8 = 0xa2;

pub(super) const USB_REPORT_LEN: usize = 63;
pub(super) const BT_REPORT_LEN: usize = 78;
pub(super) const BT_CRC_OFFSET: usize = BT_REPORT_LEN - 4;
const USB_COMMON_OFFSET: usize = 1;
const BT_COMMON_OFFSET: usize = 3;

const FLAG0_ENABLE_RUMBLE_EMULATION: u8 = 0x01;
const FLAG0_USE_RUMBLE_NOT_HAPTICS: u8 = 0x02;
pub(super) const FLAG0_ALLOW_RIGHT_TRIGGER: u8 = 0x04;
pub(super) const FLAG0_ALLOW_LEFT_TRIGGER: u8 = 0x08;
pub(super) const FLAG1_ALLOW_LIGHTBAR: u8 = 0x04;
pub(super) const FLAG1_ALLOW_PLAYER_LEDS: u8 = 0x10;

const COMMON_VALID_FLAG0: usize = 0;
const COMMON_VALID_FLAG1: usize = 1;
const COMMON_RUMBLE_RIGHT: usize = 2;
const COMMON_RUMBLE_LEFT: usize = 3;
const COMMON_RIGHT_TRIGGER: usize = 10;
const COMMON_LEFT_TRIGGER: usize = 21;
const COMMON_PLAYER_LEDS: usize = 43;
const COMMON_LIGHTBAR_RED: usize = 44;
const COMMON_LIGHTBAR_GREEN: usize = 45;
const COMMON_LIGHTBAR_BLUE: usize = 46;
const TRIGGER_EFFECT_LEN: usize = 11;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputReportKind {
    Usb,
    Bluetooth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedOutputReport {
    pub kind: OutputReportKind,
    pub bytes: Vec<u8>,
}

pub(super) struct EncodedOutputReportBuffer {
    pub(super) kind: OutputReportKind,
    pub(super) len: usize,
    bytes: [u8; BT_REPORT_LEN],
}

impl EncodedOutputReportBuffer {
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub fn encode_controller_output_frame(
    frame: &ControllerOutputFrame,
    transport: DeviceTransportKind,
    sequence: u8,
) -> Result<EncodedOutputReport, DeviceError> {
    let report = encode_controller_output_frame_buffer(frame, transport, sequence)?;
    Ok(EncodedOutputReport {
        kind: report.kind,
        bytes: report.as_slice().to_vec(),
    })
}

pub(super) fn encode_controller_output_frame_buffer(
    frame: &ControllerOutputFrame,
    transport: DeviceTransportKind,
    sequence: u8,
) -> Result<EncodedOutputReportBuffer, DeviceError> {
    match transport {
        DeviceTransportKind::Usb => Ok(EncodedOutputReportBuffer {
            kind: OutputReportKind::Usb,
            len: USB_REPORT_LEN,
            bytes: encode_usb_output_report_buffer(frame),
        }),
        DeviceTransportKind::Bluetooth => Ok(EncodedOutputReportBuffer {
            kind: OutputReportKind::Bluetooth,
            len: BT_REPORT_LEN,
            bytes: encode_bluetooth_output_report_buffer(frame, sequence),
        }),
        DeviceTransportKind::Unknown => Err(DeviceError::TransportFault(
            "cannot encode DualSense output report for unknown transport".to_string(),
        )),
    }
}

fn encode_usb_output_report_buffer(frame: &ControllerOutputFrame) -> [u8; BT_REPORT_LEN] {
    let mut report = [0; BT_REPORT_LEN];
    report[0] = USB_REPORT_ID;
    fill_common_output(&mut report[..USB_REPORT_LEN], USB_COMMON_OFFSET, frame);
    report
}

fn encode_bluetooth_output_report_buffer(
    frame: &ControllerOutputFrame,
    sequence: u8,
) -> [u8; BT_REPORT_LEN] {
    let mut report = [0; BT_REPORT_LEN];
    report[0] = BT_REPORT_ID;
    report[1] = (sequence & 0x0f) << 4;
    report[2] = BT_OUTPUT_TAG;
    fill_common_output(&mut report, BT_COMMON_OFFSET, frame);

    let crc = dualsense_output_crc32(&report[..BT_CRC_OFFSET]);
    report[BT_CRC_OFFSET..].copy_from_slice(&crc.to_le_bytes());
    report
}

fn fill_common_output(report: &mut [u8], common_offset: usize, frame: &ControllerOutputFrame) {
    report[common_offset + COMMON_VALID_FLAG0] |=
        FLAG0_ALLOW_RIGHT_TRIGGER | FLAG0_ALLOW_LEFT_TRIGGER;
    write_trigger(report, common_offset + COMMON_RIGHT_TRIGGER, &frame.r2);
    write_trigger(report, common_offset + COMMON_LEFT_TRIGGER, &frame.l2);

    if let Some(rumble) = frame.rumble {
        report[common_offset + COMMON_VALID_FLAG0] |=
            FLAG0_ENABLE_RUMBLE_EMULATION | FLAG0_USE_RUMBLE_NOT_HAPTICS;
        write_rumble(report, common_offset, rumble);
    }

    if let Some(lightbar) = frame.lightbar {
        report[common_offset + COMMON_VALID_FLAG1] |= FLAG1_ALLOW_LIGHTBAR;
        let brightness = normalized(lightbar.brightness);
        report[common_offset + COMMON_LIGHTBAR_RED] =
            brightness_scaled(lightbar.color.red, brightness);
        report[common_offset + COMMON_LIGHTBAR_GREEN] =
            brightness_scaled(lightbar.color.green, brightness);
        report[common_offset + COMMON_LIGHTBAR_BLUE] =
            brightness_scaled(lightbar.color.blue, brightness);
    }

    if let Some(player_leds) = frame.player_leds {
        report[common_offset + COMMON_VALID_FLAG1] |= FLAG1_ALLOW_PLAYER_LEDS;
        report[common_offset + COMMON_PLAYER_LEDS] = player_led_mask(player_leds);
    }
}

fn write_trigger(report: &mut [u8], offset: usize, trigger: &TriggerOutput) {
    let encoded = encode_trigger(trigger);
    report[offset..offset + TRIGGER_EFFECT_LEN].copy_from_slice(&encoded);
}

fn encode_trigger(trigger: &TriggerOutput) -> [u8; TRIGGER_EFFECT_LEN] {
    let mut encoded = [0; TRIGGER_EFFECT_LEN];
    match trigger {
        TriggerOutput::Off => {
            // Mode 0x05 retracts the actuator and clears the programmed effect.
            encoded[0] = 0x05;
        }
        TriggerOutput::AdaptiveResistance {
            start_position,
            strength,
        } => {
            encoded[0] = 0x01;
            encoded[1] = resistance_position(*start_position);
            encoded[2] = force_byte(*strength);
        }
        TriggerOutput::Wall { position, strength } => {
            encoded[0] = 0x02;
            let start = normalized(*position);
            encoded[1] = resistance_position(start);
            encoded[2] = resistance_position(1.0);
            encoded[3] = force_byte(*strength);
        }
        TriggerOutput::Pulse {
            amplitude,
            frequency_hz,
        } => {
            encoded[0] = 0x06;
            encoded[1] = frequency_byte(*frequency_hz);
            encoded[2] = trigger_motor_strength(*amplitude);
            encoded[3] = vibration_start_position(0.0);
        }
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            encoded = encode_pulse_ab_trigger(*strength, *frequency_hz, *wall_zones);
        }
    }
    encoded
}

fn encode_pulse_ab_trigger(
    strength: f64,
    frequency_hz: f64,
    wall_zones: u8,
) -> [u8; TRIGGER_EFFECT_LEN] {
    let zone_strength = pulse_ab_zone_strength(strength);
    let top_zones = wall_zones.clamp(1, 9) as usize;
    let mut zones = [zone_strength; 10];
    for zone in &mut zones[(10 - top_zones)..] {
        *zone = 8;
    }

    let mut active: u16 = 0;
    let mut packed_strength: u32 = 0;
    for (index, strength) in zones.iter().enumerate() {
        active |= 1_u16 << index;
        packed_strength |= (u32::from(strength.saturating_sub(1)) & 0x07) << (3 * index);
    }

    [
        0x26,
        (active & 0xff) as u8,
        ((active >> 8) & 0xff) as u8,
        (packed_strength & 0xff) as u8,
        ((packed_strength >> 8) & 0xff) as u8,
        ((packed_strength >> 16) & 0xff) as u8,
        ((packed_strength >> 24) & 0xff) as u8,
        frequency_byte(frequency_hz),
        0,
        0,
        0,
    ]
}

pub(super) fn pulse_ab_zone_strength(value: f64) -> u8 {
    let amp = force_byte(value);
    ((amp / 32).saturating_add(1)).clamp(1, 8)
}

fn write_rumble(report: &mut [u8], common_offset: usize, rumble: RumbleOutput) {
    report[common_offset + COMMON_RUMBLE_RIGHT] = force_byte(rumble.high_frequency);
    report[common_offset + COMMON_RUMBLE_LEFT] = force_byte(rumble.low_frequency);
}

fn player_led_mask(player_leds: PlayerLedsOutput) -> u8 {
    match player_leds.count.clamp(0, 5) {
        0 => 0x00,
        1 => 0x04,
        2 => 0x06,
        3 => 0x15,
        4 => 0x1b,
        _ => 0x1f,
    }
}

pub(super) fn resistance_position(value: f64) -> u8 {
    (30.0 + normalized(value) * 142.0).round() as u8
}

fn vibration_start_position(value: f64) -> u8 {
    (normalized(value) * 137.0).round() as u8
}

pub(super) fn force_byte(value: f64) -> u8 {
    (normalized(value) * 255.0).round() as u8
}

fn trigger_motor_strength(value: f64) -> u8 {
    (normalized(value) * 63.0).round() as u8
}

fn frequency_byte(frequency_hz: f64) -> u8 {
    if frequency_hz.is_finite() {
        frequency_hz.round().clamp(1.0, 255.0) as u8
    } else {
        1
    }
}

fn brightness_scaled(value: u8, brightness: f64) -> u8 {
    (f64::from(value) * brightness).round().clamp(0.0, 255.0) as u8
}

fn normalized(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn dualsense_output_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    crc = crc32_le_update(crc, OUTPUT_CRC32_SEED);
    for byte in data {
        crc = crc32_le_update(crc, *byte);
    }
    !crc
}

fn crc32_le_update(mut crc: u32, byte: u8) -> u32 {
    crc ^= u32::from(byte);
    for _ in 0..8 {
        if crc & 1 == 1 {
            crc = (crc >> 1) ^ 0xedb8_8320;
        } else {
            crc >>= 1;
        }
    }
    crc
}
