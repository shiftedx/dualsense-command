use dscc_core::{ControllerOutputFrame, LightbarOutput, PlayerLedsOutput, RgbColor, TriggerOutput};
use std::{
    sync::{mpsc, Condvar},
    thread,
    time::Duration,
};

use super::encoding::*;
use super::*;
use crate::{
    edge_profile::{encode_edge_onboard_profile, EdgeOnboardSlotId},
    enumeration::RawHidDevice,
    status::DeviceFamily,
    transport::{DeviceHandle, DeviceTransport, MockTransport, WriteOutcome},
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
    assert_eq!(USB_REPORT_LEN, 48);
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

    let report = encode_controller_output_frame(&frame, DeviceTransportKind::Bluetooth, 7).unwrap();
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
fn dualsense_input_parser_reads_usb_and_bluetooth_axes_and_buttons() {
    let mut usb = vec![0; 54];
    usb[0] = 0x01;
    usb[1] = 0;
    usb[2] = 255;
    usb[3] = 128;
    usb[4] = 192;
    usb[5] = 128;
    usb[6] = 255;
    usb[8] = 0x21;
    usb[9] = 0x4c;
    usb[10] = 0xc4;
    let usb_input = parse_dualsense_input_state(&usb).expect("usb input parses");
    assert_eq!(usb_input.left_stick.x, -1.0);
    assert_eq!(usb_input.left_stick.y, 1.0);
    assert_eq!(usb_input.right_stick.x, 0.0);
    assert!((usb_input.right_stick.y - 64.0 / 127.0).abs() < f64::EPSILON);
    assert_eq!(usb_input.left_stick.magnitude, 1.0);
    assert!((usb_input.l2 - 128.0 / 255.0).abs() < f64::EPSILON);
    assert_eq!(usb_input.r2, 1.0);
    assert_button(&usb_input, "dpad_right", true, 1.0);
    assert_button(&usb_input, "cross", true, 1.0);
    assert_button(&usb_input, "l2", true, 128.0 / 255.0);
    assert_button(&usb_input, "create", false, 0.0);
    assert_button(&usb_input, "edge_back_left", true, 1.0);

    let mut bluetooth = vec![0; 78];
    bluetooth[0] = 0x31;
    bluetooth[2] = 255;
    bluetooth[3] = 0;
    bluetooth[4] = 64;
    bluetooth[5] = 128;
    bluetooth[6] = 64;
    bluetooth[7] = 192;
    bluetooth[9] = 0x97;
    bluetooth[10] = 0x83;
    bluetooth[11] = 0x22;
    let bluetooth_input = parse_dualsense_input_state(&bluetooth).expect("bluetooth input parses");
    assert_eq!(bluetooth_input.left_stick.x, 1.0);
    assert_eq!(bluetooth_input.left_stick.y, -1.0);
    assert!((bluetooth_input.right_stick.x + 64.0 / 128.0).abs() < f64::EPSILON);
    assert_eq!(bluetooth_input.right_stick.y, 0.0);
    assert!((bluetooth_input.l2 - 64.0 / 255.0).abs() < f64::EPSILON);
    assert!((bluetooth_input.r2 - 192.0 / 255.0).abs() < f64::EPSILON);
    assert_button(&bluetooth_input, "dpad_up", true, 1.0);
    assert_button(&bluetooth_input, "dpad_left", true, 1.0);
    assert_button(&bluetooth_input, "square", true, 1.0);
    assert_button(&bluetooth_input, "r2", false, 192.0 / 255.0);
    assert_button(&bluetooth_input, "options", false, 0.0);
    assert_button(&bluetooth_input, "touchpad", true, 1.0);
    assert_button(&bluetooth_input, "edge_fn_right", true, 1.0);
}

#[test]
fn dualsense_input_parser_rejects_short_reports() {
    assert_eq!(parse_dualsense_input_state(&[0x01, 0, 0, 0, 0, 0, 0]), None);
    assert_eq!(parse_dualsense_input_state(&[0x31, 0, 0, 0, 0, 0, 0]), None);
    assert_eq!(parse_dualsense_input_state(&[0x02; 64]), None);
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

fn assert_button(input: &ControllerInputState, id: &str, pressed: bool, value: f64) {
    let button = input
        .buttons
        .iter()
        .find(|button| button.id == id)
        .expect("button should be present");
    assert_eq!(button.pressed, pressed);
    assert!((button.value - value).abs() < f64::EPSILON);
}

#[test]
fn output_manager_records_dry_run_write_on_mock_transport() {
    let device = RawHidDevice::mock("mock://edge")
        .with_family_hint(DeviceFamily::DualSenseEdge)
        .with_transport_hint(DeviceTransportKind::Usb);
    let raw_id = device.id.clone();
    let transport = MockTransport::with_devices(vec![device]);
    // Model a dry-run handle so the suppressed outcome — not the manager mode —
    // drives the reported hardware_output flag.
    transport.set_suppress_writes(true);
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
fn write_frame_reports_hardware_output_false_when_handle_suppresses() {
    let device = RawHidDevice::mock("mock://suppressed")
        .with_family_hint(DeviceFamily::DualSenseEdge)
        .with_transport_hint(DeviceTransportKind::Usb);
    let raw_id = device.id.clone();
    let transport = MockTransport::with_devices(vec![device]);
    transport.set_suppress_writes(true);
    // Manager is in HardwareOutput mode, but the handle suppresses the write:
    // hardware_output must follow the handle outcome, not the manager mode.
    let manager = ControllerOutputManager::new(transport.clone(), OutputMode::HardwareOutput);
    let target = ControllerOutputTarget {
        raw_device_id: raw_id.clone(),
        transport: DeviceTransportKind::Usb,
    };

    let write = manager
        .write_frame(&target, &ControllerOutputFrame::default())
        .unwrap();

    assert_eq!(write.bytes, USB_REPORT_LEN);
    assert!(!write.hardware_output);
    // The report still flows through the validated encode path and is recorded.
    assert_eq!(transport.writes_for(&raw_id).len(), 1);
}

#[test]
fn write_frame_reports_hardware_output_true_when_handle_executes() {
    let device = RawHidDevice::mock("mock://executed")
        .with_family_hint(DeviceFamily::DualSenseEdge)
        .with_transport_hint(DeviceTransportKind::Usb);
    let raw_id = device.id.clone();
    let transport = MockTransport::with_devices(vec![device]);
    // Manager is in DryRunHid mode, but the handle executes the write:
    // hardware_output must follow the handle outcome, not the manager mode.
    let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
    let target = ControllerOutputTarget {
        raw_device_id: raw_id,
        transport: DeviceTransportKind::Usb,
    };

    let write = manager
        .write_frame(&target, &ControllerOutputFrame::default())
        .unwrap();

    assert_eq!(write.bytes, USB_REPORT_LEN);
    assert!(write.hardware_output);
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
        .any(|profile| profile.slot == EdgeOnboardSlotId::Square && profile.name == "square Tune"));

    let mut write_profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Road Tune");
    write_profile.trigger_deadzone.right = [3, 97];
    transport.push_feature_report(raw_id.clone(), 0x63, vec![0x63]);
    queue_edge_profile_readback(&transport, &raw_id, &write_profile, transport_kind);
    manager
        .write_edge_onboard_profile(&target, &write_profile)
        .unwrap();

    if transport_kind == DeviceTransportKind::Bluetooth {
        let writes = transport.feature_writes_for(&raw_id, 0x63);
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].len(), 63);
        assert!(transport.feature_writes_for(&raw_id, 0x60).is_empty());
    } else {
        let writes = transport.feature_writes_for(&raw_id, 0x60);
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].len(), 64);
        assert_eq!(writes[0][0], 0x60);
    }
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
fn output_manager_rejects_edge_onboard_write_readback_mismatch() {
    let device = RawHidDevice::mock("mock://edge-profile-mismatch")
        .with_family_hint(DeviceFamily::DualSenseEdge)
        .with_transport_hint(DeviceTransportKind::Bluetooth);
    let raw_id = device.id.clone();
    let transport = MockTransport::with_devices(vec![device]);
    let manager = ControllerOutputManager::new(transport.clone(), OutputMode::DryRunHid);
    let target = ControllerOutputTarget {
        raw_device_id: raw_id.clone(),
        transport: DeviceTransportKind::Bluetooth,
    };
    let profile = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Road Tune");
    let mismatch = EdgeOnboardProfile::new(EdgeOnboardSlotId::Square, "Different Tune");
    transport.push_feature_report(raw_id.clone(), 0x63, vec![0x63]);
    queue_edge_profile_readback(
        &transport,
        &raw_id,
        &mismatch,
        DeviceTransportKind::Bluetooth,
    );

    let error = manager
        .write_edge_onboard_profile(&target, &profile)
        .expect_err("readback mismatch should fail the sync");

    assert!(error.to_string().contains("readback"));
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
    assert!(error.to_string().contains("expected 48 bytes"));
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

fn queue_edge_profile_readback(
    transport: &MockTransport,
    raw_id: &RawDeviceId,
    profile: &EdgeOnboardProfile,
    transport_kind: DeviceTransportKind,
) {
    let read_report_ids = profile.slot.read_report_ids();
    let mut reports = encode_edge_onboard_profile(profile).unwrap();
    for (report, read_report_id) in reports.iter_mut().zip(read_report_ids) {
        report[0] = read_report_id;
    }
    for (report_id, report) in read_report_ids.into_iter().zip(reports) {
        if transport_kind == DeviceTransportKind::Bluetooth {
            transport.push_feature_report(raw_id.clone(), report_id, report[1..].to_vec());
        } else {
            transport.push_feature_report(raw_id.clone(), report_id, report.to_vec());
        }
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

    fn write(&mut self, report: &[u8]) -> Result<WriteOutcome, DeviceError> {
        if self.id == self.blocked_id {
            let (state, condition) = &*self.state;
            let mut state = state.lock().unwrap();
            state.blocked_write_started = true;
            condition.notify_all();
            while !state.release_blocked_write {
                state = condition.wait(state).unwrap();
            }
        }
        Ok(WriteOutcome::Executed {
            bytes: report.len(),
        })
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
        let result = other_manager.write_frame(&other_target, &ControllerOutputFrame::default());
        done_tx
            .send(result)
            .expect("test receiver should stay alive");
    });

    let other_finished_before_release = done_rx.recv_timeout(Duration::from_millis(200)).is_ok();

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
