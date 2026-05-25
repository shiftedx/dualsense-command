use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use dscc_core::{ControllerOutputFrame, PlayerLedsOutput, RumbleOutput, TriggerOutput};

use crate::{
    edge_profile::{
        edge_onboard_transport_supported, read_edge_onboard_profiles_from_handle,
        write_edge_onboard_profile_to_handle, EdgeOnboardProfile,
    },
    error::DeviceError,
    manager::OutputMode,
    status::{DeviceTransportKind, RawDeviceId},
    transport::{DeviceHandle, DeviceTransport},
};

const USB_REPORT_ID: u8 = 0x02;
const BT_REPORT_ID: u8 = 0x31;
const BT_OUTPUT_TAG: u8 = 0x10;
const OUTPUT_CRC32_SEED: u8 = 0xa2;

const USB_REPORT_LEN: usize = 63;
const BT_REPORT_LEN: usize = 78;
const BT_CRC_OFFSET: usize = BT_REPORT_LEN - 4;
const USB_COMMON_OFFSET: usize = 1;
const BT_COMMON_OFFSET: usize = 3;

const FLAG0_ENABLE_RUMBLE_EMULATION: u8 = 0x01;
const FLAG0_USE_RUMBLE_NOT_HAPTICS: u8 = 0x02;
const FLAG0_ALLOW_RIGHT_TRIGGER: u8 = 0x04;
const FLAG0_ALLOW_LEFT_TRIGGER: u8 = 0x08;
const FLAG1_ALLOW_LIGHTBAR: u8 = 0x04;
const FLAG1_ALLOW_PLAYER_LEDS: u8 = 0x10;

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
const INPUT_USB_COMMON_OFFSET: usize = 1;
const INPUT_BT_COMMON_OFFSET: usize = 2;
const INPUT_COMMON_L2: usize = 4;
const INPUT_COMMON_R2: usize = 5;
const INPUT_READ_ATTEMPTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerInputState {
    pub l2: f64,
    pub r2: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerOutputTarget {
    pub raw_device_id: RawDeviceId,
    pub transport: DeviceTransportKind,
}

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

struct EncodedOutputReportBuffer {
    kind: OutputReportKind,
    len: usize,
    bytes: [u8; BT_REPORT_LEN],
}

impl EncodedOutputReportBuffer {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerOutputWrite {
    pub bytes: usize,
    pub hardware_output: bool,
    pub report_kind: OutputReportKind,
}

pub struct ControllerOutputManager<T: DeviceTransport> {
    transport: T,
    output_mode: OutputMode,
    sessions: Mutex<BTreeMap<RawDeviceId, Arc<Mutex<OutputSession>>>>,
}

struct OutputSession {
    handle: Box<dyn DeviceHandle>,
    sequence: u8,
}

impl<T: DeviceTransport> ControllerOutputManager<T> {
    pub fn new(transport: T, output_mode: OutputMode) -> Self {
        Self {
            transport,
            output_mode,
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn output_mode(&self) -> OutputMode {
        self.output_mode
    }

    pub fn hardware_writes_enabled(&self) -> bool {
        self.output_mode.hardware_writes_enabled()
    }

    pub fn write_frame(
        &self,
        target: &ControllerOutputTarget,
        frame: &ControllerOutputFrame,
    ) -> Result<ControllerOutputWrite, DeviceError> {
        let session = self.session_for(target)?;
        let write_result = {
            let mut session = lock_session(&session);
            let report =
                encode_controller_output_frame_buffer(frame, target.transport, session.sequence)?;
            if report.kind == OutputReportKind::Bluetooth {
                session.sequence = (session.sequence + 1) & 0x0f;
            }

            let write_result = session.handle.write(report.as_slice());
            (report, write_result)
        };

        let (report, write_result) = write_result;
        let report_len = report.len;
        match write_result {
            Ok(backend_bytes) if backend_bytes >= report_len => Ok(ControllerOutputWrite {
                bytes: report_len,
                hardware_output: self.hardware_writes_enabled(),
                report_kind: report.kind,
            }),
            Ok(backend_bytes) => {
                self.release(target);
                Err(DeviceError::TransportFault(format!(
                    "short {:?} output report write: expected {} bytes, wrote {backend_bytes}",
                    report.kind, report_len
                )))
            }
            Err(error) => {
                self.release(target);
                Err(error)
            }
        }
    }

    pub fn read_input_state(
        &self,
        target: &ControllerOutputTarget,
    ) -> Result<Option<ControllerInputState>, DeviceError> {
        let session = self.session_for(target)?;
        let read_result = {
            let mut session = lock_session(&session);
            let mut buffer = [0_u8; 256];
            let mut input = None;
            let mut fault = None;
            for _ in 0..INPUT_READ_ATTEMPTS {
                match session
                    .handle
                    .read_timeout_into(&mut buffer, Duration::from_millis(3))
                {
                    Ok(Some(size)) => {
                        if let Some(parsed) = parse_dualsense_input_state(&buffer[..size]) {
                            input = Some(parsed);
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        fault = Some(error);
                        break;
                    }
                }
            }
            fault.map_or(Ok(input), Err)
        };

        if read_result.is_err() {
            self.release(target);
        }
        read_result
    }

    pub fn read_edge_onboard_profiles(
        &self,
        target: &ControllerOutputTarget,
    ) -> Result<Vec<EdgeOnboardProfile>, DeviceError> {
        if !edge_onboard_transport_supported(target.transport) {
            return Err(DeviceError::TransportFault(
                "DualSense Edge onboard profile reads require USB or Bluetooth HID feature report access"
                    .to_string(),
            ));
        }

        let session = self.session_for(target)?;
        let read_result = {
            let mut session = lock_session(&session);
            read_edge_onboard_profiles_from_handle(session.handle.as_mut())
        };

        if read_result.is_err() {
            self.release(target);
        }
        read_result
    }

    pub fn write_edge_onboard_profile(
        &self,
        target: &ControllerOutputTarget,
        profile: &EdgeOnboardProfile,
    ) -> Result<(), DeviceError> {
        if !edge_onboard_transport_supported(target.transport) {
            return Err(DeviceError::TransportFault(
                "DualSense Edge onboard profile writes require USB or Bluetooth HID feature report access"
                    .to_string(),
            ));
        }

        let session = self.session_for(target)?;
        let write_result = {
            let mut session = lock_session(&session);
            write_edge_onboard_profile_to_handle(session.handle.as_mut(), profile)
        };

        if write_result.is_err() {
            self.release(target);
        }
        write_result
    }

    pub fn release(&self, target: &ControllerOutputTarget) {
        self.lock_sessions().remove(&target.raw_device_id);
    }

    pub fn release_all(&self) {
        self.lock_sessions().clear();
    }

    fn session_for(
        &self,
        target: &ControllerOutputTarget,
    ) -> Result<Arc<Mutex<OutputSession>>, DeviceError> {
        {
            let sessions = self.lock_sessions();
            if let Some(session) = sessions.get(&target.raw_device_id) {
                return Ok(session.clone());
            }
        }

        let handle = self.transport.open(&target.raw_device_id)?;
        let mut sessions = self.lock_sessions();
        match sessions.entry(target.raw_device_id.clone()) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => Ok(entry
                .insert(Arc::new(Mutex::new(OutputSession {
                    handle,
                    sequence: 0,
                })))
                .clone()),
        }
    }

    fn lock_sessions(&self) -> MutexGuard<'_, BTreeMap<RawDeviceId, Arc<Mutex<OutputSession>>>> {
        match self.sessions.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn lock_session(session: &Mutex<OutputSession>) -> MutexGuard<'_, OutputSession> {
    match session.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn parse_dualsense_input_state(report: &[u8]) -> Option<ControllerInputState> {
    let common_offset = match report.first().copied()? {
        0x01 if report.len() > INPUT_USB_COMMON_OFFSET + INPUT_COMMON_R2 => INPUT_USB_COMMON_OFFSET,
        0x31 if report.len() > INPUT_BT_COMMON_OFFSET + INPUT_COMMON_R2 => INPUT_BT_COMMON_OFFSET,
        _ => return None,
    };

    Some(ControllerInputState {
        l2: f64::from(report[common_offset + INPUT_COMMON_L2]) / 255.0,
        r2: f64::from(report[common_offset + INPUT_COMMON_R2]) / 255.0,
    })
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

fn encode_controller_output_frame_buffer(
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

fn pulse_ab_zone_strength(value: f64) -> u8 {
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

fn resistance_position(value: f64) -> u8 {
    (30.0 + normalized(value) * 142.0).round() as u8
}

fn vibration_start_position(value: f64) -> u8 {
    (normalized(value) * 137.0).round() as u8
}

fn force_byte(value: f64) -> u8 {
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

fn dualsense_output_crc32(data: &[u8]) -> u32 {
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

#[cfg(test)]
mod tests {
    use dscc_core::{LightbarOutput, RgbColor};
    use std::{
        sync::{mpsc, Condvar},
        thread,
    };

    use super::*;
    use crate::{
        edge_profile::{encode_edge_onboard_profile, EdgeOnboardSlotId},
        enumeration::RawHidDevice,
        status::DeviceFamily,
        transport::{DeviceHandle, DeviceTransport, MockTransport},
    };

    #[test]
    fn usb_report_encodes_trigger_blocks_and_lightbar() {
        let frame = ControllerOutputFrame {
            l2: TriggerOutput::AdaptiveResistance {
                start_position: 0.2,
                strength: 0.5,
            },
            r2: TriggerOutput::Pulse {
                amplitude: 0.75,
                frequency_hz: 42.0,
            },
            lightbar: Some(LightbarOutput {
                color: RgbColor {
                    red: 100,
                    green: 50,
                    blue: 10,
                },
                brightness: 0.5,
            }),
            player_leds: Some(PlayerLedsOutput { count: 3 }),
            rumble: None,
        };

        let report = encode_controller_output_frame(&frame, DeviceTransportKind::Usb, 0).unwrap();

        assert_eq!(report.kind, OutputReportKind::Usb);
        assert_eq!(report.bytes.len(), USB_REPORT_LEN);
        assert_eq!(report.bytes[0], USB_REPORT_ID);
        assert_eq!(
            report.bytes[1],
            FLAG0_ALLOW_RIGHT_TRIGGER | FLAG0_ALLOW_LEFT_TRIGGER
        );
        assert_eq!(
            report.bytes[2],
            FLAG1_ALLOW_LIGHTBAR | FLAG1_ALLOW_PLAYER_LEDS
        );
        assert_eq!(report.bytes[11], 0x06);
        assert_eq!(report.bytes[12], 42);
        assert_eq!(report.bytes[13], 47);
        assert_eq!(report.bytes[22], 0x01);
        assert_eq!(report.bytes[23], 58);
        assert_eq!(report.bytes[24], 128);
        assert_eq!(report.bytes[44], 0x15);
        assert_eq!(report.bytes[45], 50);
        assert_eq!(report.bytes[46], 25);
        assert_eq!(report.bytes[47], 5);
    }

    #[test]
    fn bluetooth_report_has_header_sequence_and_crc() {
        let frame = ControllerOutputFrame {
            l2: TriggerOutput::Off,
            r2: TriggerOutput::Wall {
                position: 0.4,
                strength: 0.9,
            },
            ..ControllerOutputFrame::default()
        };

        let report =
            encode_controller_output_frame(&frame, DeviceTransportKind::Bluetooth, 7).unwrap();
        let crc = u32::from_le_bytes(report.bytes[BT_CRC_OFFSET..].try_into().unwrap());

        assert_eq!(report.kind, OutputReportKind::Bluetooth);
        assert_eq!(report.bytes.len(), BT_REPORT_LEN);
        assert_eq!(report.bytes[0], BT_REPORT_ID);
        assert_eq!(report.bytes[1], 0x70);
        assert_eq!(report.bytes[2], BT_OUTPUT_TAG);
        assert_eq!(
            report.bytes[3],
            FLAG0_ALLOW_RIGHT_TRIGGER | FLAG0_ALLOW_LEFT_TRIGGER
        );
        assert_eq!(report.bytes[13], 0x02);
        assert_eq!(report.bytes[14], resistance_position(0.4));
        assert_eq!(report.bytes[15], resistance_position(1.0));
        assert_eq!(report.bytes[16], force_byte(0.9));
        assert_eq!(report.bytes[24], 0x05);
        assert_eq!(crc, dualsense_output_crc32(&report.bytes[..BT_CRC_OFFSET]));
        assert_ne!(crc, 0);
    }

    #[test]
    fn usb_report_encodes_pulse_ab_wall_form_trigger() {
        let frame = ControllerOutputFrame {
            r2: TriggerOutput::PulseAb {
                strength: 1.0,
                frequency_hz: 20.0,
                wall_zones: 2,
            },
            ..ControllerOutputFrame::default()
        };

        let report = encode_controller_output_frame(&frame, DeviceTransportKind::Usb, 0).unwrap();
        let trigger = &report.bytes[11..22];

        assert_eq!(trigger[0], 0x26);
        assert_eq!(trigger[1], 0xff);
        assert_eq!(trigger[2], 0x03);
        assert_eq!(trigger[7], 20);
    }

    #[test]
    fn pulse_ab_zone_strength_matches_shift_thump_boundaries() {
        assert_eq!(pulse_ab_zone_strength(0.0), 1);
        assert_eq!(pulse_ab_zone_strength(31.0 / 255.0), 1);
        assert_eq!(pulse_ab_zone_strength(32.0 / 255.0), 2);
        assert_eq!(pulse_ab_zone_strength(1.0), 8);
    }

    #[test]
    fn dualsense_input_parser_reads_usb_and_bluetooth_trigger_axes() {
        let mut usb = vec![0; 54];
        usb[0] = 0x01;
        usb[5] = 128;
        usb[6] = 255;
        let usb_input = parse_dualsense_input_state(&usb).expect("usb input parses");
        assert!((usb_input.l2 - 128.0 / 255.0).abs() < f64::EPSILON);
        assert_eq!(usb_input.r2, 1.0);

        let mut bluetooth = vec![0; 78];
        bluetooth[0] = 0x31;
        bluetooth[6] = 64;
        bluetooth[7] = 192;
        let bluetooth_input =
            parse_dualsense_input_state(&bluetooth).expect("bluetooth input parses");
        assert!((bluetooth_input.l2 - 64.0 / 255.0).abs() < f64::EPSILON);
        assert!((bluetooth_input.r2 - 192.0 / 255.0).abs() < f64::EPSILON);
    }

    #[test]
    fn output_manager_reads_trigger_axes_from_existing_session() {
        let device = RawHidDevice::mock("mock://edge-input")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.push_read_report(raw_id.clone(), {
            let mut report = vec![0; 54];
            report[0] = 0x01;
            report[5] = 25;
            report[6] = 200;
            report
        });
        let manager = ControllerOutputManager::new(transport, OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id,
            transport: DeviceTransportKind::Usb,
        };

        let input = manager
            .read_input_state(&target)
            .unwrap()
            .expect("queued input report parses");

        assert!((input.l2 - 25.0 / 255.0).abs() < f64::EPSILON);
        assert!((input.r2 - 200.0 / 255.0).abs() < f64::EPSILON);
    }

    #[test]
    fn output_manager_records_dry_run_write_on_mock_transport() {
        let device = RawHidDevice::mock("mock://edge")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id.clone(),
            transport: DeviceTransportKind::Usb,
        };

        let write = manager
            .write_frame(
                &target,
                &ControllerOutputFrame {
                    r2: TriggerOutput::AdaptiveResistance {
                        start_position: 0.1,
                        strength: 0.8,
                    },
                    ..ControllerOutputFrame::default()
                },
            )
            .unwrap();

        let writes = transport.writes_for(&raw_id);
        assert_eq!(write.bytes, USB_REPORT_LEN);
        assert!(!write.hardware_output);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0][0], USB_REPORT_ID);
    }

    #[test]
    fn output_manager_reads_and_writes_edge_onboard_profile_feature_reports() {
        assert_edge_onboard_profile_feature_reports(DeviceTransportKind::Usb);
        assert_edge_onboard_profile_feature_reports(DeviceTransportKind::Bluetooth);
    }

    fn assert_edge_onboard_profile_feature_reports(transport_kind: DeviceTransportKind) {
        let device = RawHidDevice::mock("mock://edge-profile")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(transport_kind);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);

        let mut default_profile = [[0_u8; 64]; 3];
        default_profile[0][0] = 0x70;
        default_profile[0][1] = 0x10;
        default_profile[1][0] = 0x71;
        default_profile[2][0] = 0x72;
        queue_edge_profile_read(&transport, &raw_id, [0x70, 0x71, 0x72], default_profile);

        for (slot, read_reports) in [
            (EdgeOnboardSlotId::Square, [0x73, 0x74, 0x75]),
            (EdgeOnboardSlotId::Cross, [0x76, 0x77, 0x78]),
            (EdgeOnboardSlotId::Circle, [0x79, 0x7a, 0x7b]),
        ] {
            let mut profile = EdgeOnboardProfile::new(slot, format!("{} Tune", slot.as_str()));
            profile.trigger_deadzone.left = [6, 94];
            let mut reports = encode_edge_onboard_profile(&profile).unwrap();
            for (report, selector) in reports.iter_mut().zip(read_reports) {
                report[0] = selector;
            }
            queue_edge_profile_read(&transport, &raw_id, read_reports, reports);
        }

        let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id.clone(),
            transport: transport_kind,
        };

        let profiles = manager.read_edge_onboard_profiles(&target).unwrap();
        assert_eq!(profiles.len(), 4);
        assert!(profiles
            .iter()
            .any(|profile| profile.slot == EdgeOnboardSlotId::Square
                && profile.name == "square Tune"));

        let mut write_profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Road Tune");
        write_profile.trigger_deadzone.right = [3, 97];
        transport.push_feature_report(raw_id.clone(), 0x63, vec![0x63]);
        manager
            .write_edge_onboard_profile(&target, &write_profile)
            .unwrap();

        let writes = transport.feature_writes_for(&raw_id, 0x60);
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].len(), 64);
        assert_eq!(writes[0][0], 0x60);
    }

    #[test]
    fn output_manager_requires_edge_onboard_write_acknowledgement() {
        let device = RawHidDevice::mock("mock://edge-profile-no-ack")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        let manager = ControllerOutputManager::new(transport, OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id,
            transport: DeviceTransportKind::Usb,
        };
        let profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Road Tune");

        let error = manager
            .write_edge_onboard_profile(&target, &profile)
            .expect_err("missing Edge write acknowledgement should fail");

        assert!(error.to_string().contains("acknowledgement"));
    }

    #[test]
    fn output_manager_rejects_unknown_transport_for_edge_onboard_profiles() {
        let device = RawHidDevice::mock("mock://edge-profile-unknown")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Unknown);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        let manager = ControllerOutputManager::new(transport, OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id,
            transport: DeviceTransportKind::Unknown,
        };

        let error = manager
            .read_edge_onboard_profiles(&target)
            .expect_err("unknown transport should not attempt profile reads");

        assert!(error.to_string().contains("USB or Bluetooth"));
    }

    #[test]
    fn output_manager_rejects_partial_hid_write_and_releases_session() {
        let device = RawHidDevice::mock("mock://edge-partial")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.push_write_result(raw_id.clone(), Ok(USB_REPORT_LEN - 1));
        let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id.clone(),
            transport: DeviceTransportKind::Usb,
        };

        let error = manager
            .write_frame(
                &target,
                &ControllerOutputFrame {
                    r2: TriggerOutput::AdaptiveResistance {
                        start_position: 0.1,
                        strength: 0.8,
                    },
                    ..ControllerOutputFrame::default()
                },
            )
            .expect_err("short write should be rejected");

        assert!(matches!(error, DeviceError::TransportFault(_)));
        assert!(error.to_string().contains("expected 63 bytes"));
        transport.fail_open(
            raw_id,
            DeviceError::TransportFault("session reopened after short write".to_string()),
        );
        let reopen_error = manager
            .write_frame(&target, &ControllerOutputFrame::default())
            .expect_err("short write should have released the failed session");
        assert!(reopen_error
            .to_string()
            .contains("session reopened after short write"));
    }

    fn queue_edge_profile_read(
        transport: &MockTransport,
        raw_id: &RawDeviceId,
        report_ids: [u8; 3],
        reports: [[u8; 64]; 3],
    ) {
        for (report_id, report) in report_ids.into_iter().zip(reports) {
            transport.push_feature_report(raw_id.clone(), report_id, report.to_vec());
        }
    }

    #[test]
    fn output_manager_accepts_backend_write_counts_above_report_length() {
        let device = RawHidDevice::mock("mock://edge-bt-wide")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Bluetooth);
        let raw_id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.push_write_result(raw_id.clone(), Ok(547));
        let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
        let target = ControllerOutputTarget {
            raw_device_id: raw_id.clone(),
            transport: DeviceTransportKind::Bluetooth,
        };

        let write = manager
            .write_frame(
                &target,
                &ControllerOutputFrame {
                    lightbar: Some(LightbarOutput {
                        color: RgbColor {
                            red: 60,
                            green: 140,
                            blue: 220,
                        },
                        brightness: 0.7,
                    }),
                    ..ControllerOutputFrame::default()
                },
            )
            .expect("oversized backend byte count still represents a completed write");

        let writes = transport.writes_for(&raw_id);
        assert_eq!(write.bytes, BT_REPORT_LEN);
        assert_eq!(write.report_kind, OutputReportKind::Bluetooth);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].len(), BT_REPORT_LEN);
        assert_eq!(writes[0][0], BT_REPORT_ID);
    }

    #[derive(Clone)]
    struct BlockingWriteTransport {
        devices: Vec<RawHidDevice>,
        blocked_id: RawDeviceId,
        state: Arc<(Mutex<BlockingWriteState>, Condvar)>,
    }

    #[derive(Debug, Default)]
    struct BlockingWriteState {
        blocked_write_started: bool,
        release_blocked_write: bool,
    }

    struct BlockingWriteHandle {
        id: RawDeviceId,
        blocked_id: RawDeviceId,
        state: Arc<(Mutex<BlockingWriteState>, Condvar)>,
    }

    impl DeviceTransport for BlockingWriteTransport {
        fn enumerate(&self) -> Result<Vec<RawHidDevice>, DeviceError> {
            Ok(self.devices.clone())
        }

        fn open(&self, id: &RawDeviceId) -> Result<Box<dyn DeviceHandle>, DeviceError> {
            if self.devices.iter().any(|device| &device.id == id) {
                Ok(Box::new(BlockingWriteHandle {
                    id: id.clone(),
                    blocked_id: self.blocked_id.clone(),
                    state: self.state.clone(),
                }))
            } else {
                Err(DeviceError::DeviceNotFound(id.clone()))
            }
        }
    }

    impl DeviceHandle for BlockingWriteHandle {
        fn read_timeout(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError> {
            Ok(None)
        }

        fn write(&mut self, report: &[u8]) -> Result<usize, DeviceError> {
            if self.id == self.blocked_id {
                let (state, condition) = &*self.state;
                let mut state = state.lock().unwrap();
                state.blocked_write_started = true;
                condition.notify_all();
                while !state.release_blocked_write {
                    state = condition.wait(state).unwrap();
                }
            }
            Ok(report.len())
        }
    }

    #[test]
    fn output_manager_does_not_block_other_devices_behind_one_device_write() {
        let blocked = RawHidDevice::mock("mock://blocked")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let other = RawHidDevice::mock("mock://other")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Usb);
        let blocked_id = blocked.id.clone();
        let other_id = other.id.clone();
        let state = Arc::new((Mutex::new(BlockingWriteState::default()), Condvar::new()));
        let transport = BlockingWriteTransport {
            devices: vec![blocked, other],
            blocked_id: blocked_id.clone(),
            state: state.clone(),
        };
        let manager = Arc::new(ControllerOutputManager::new(
            transport,
            OutputMode::DryRunHid,
        ));
        let blocked_target = ControllerOutputTarget {
            raw_device_id: blocked_id,
            transport: DeviceTransportKind::Usb,
        };
        let other_target = ControllerOutputTarget {
            raw_device_id: other_id,
            transport: DeviceTransportKind::Usb,
        };

        let blocked_manager = manager.clone();
        let blocked_thread = thread::spawn(move || {
            blocked_manager
                .write_frame(&blocked_target, &ControllerOutputFrame::default())
                .expect("blocked device write eventually succeeds");
        });

        {
            let (state_lock, condition) = &*state;
            let mut state_guard = state_lock.lock().unwrap();
            while !state_guard.blocked_write_started {
                state_guard = condition.wait(state_guard).unwrap();
            }
        }

        let (done_tx, done_rx) = mpsc::channel();
        let other_manager = manager.clone();
        let other_thread = thread::spawn(move || {
            let result =
                other_manager.write_frame(&other_target, &ControllerOutputFrame::default());
            done_tx
                .send(result)
                .expect("test receiver should stay alive");
        });

        let other_finished_before_release =
            done_rx.recv_timeout(Duration::from_millis(200)).is_ok();

        {
            let (state_lock, condition) = &*state;
            let mut state_guard = state_lock.lock().unwrap();
            state_guard.release_blocked_write = true;
            condition.notify_all();
        }

        blocked_thread.join().expect("blocked writer should join");
        other_thread.join().expect("other writer should join");
        assert!(
            other_finished_before_release,
            "other controller writes should not wait for the blocked controller session"
        );
    }
}
