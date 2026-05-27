use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Arc, Mutex, MutexGuard},
};

use dscc_core::ControllerOutputFrame;

mod encoding;
mod input;

pub use encoding::{encode_controller_output_frame, EncodedOutputReport, OutputReportKind};
pub use input::{
    ControllerInputButtonState, ControllerInputReadOptions, ControllerInputState,
    ControllerInputStickState,
};

use encoding::encode_controller_output_frame_buffer;
use input::parse_dualsense_input_state;

use crate::{
    edge_profile::{
        edge_onboard_transport_supported, edge_onboard_write_transport_supported,
        read_edge_onboard_profiles_from_handle, write_edge_onboard_profile_to_handle_for_transport,
        EdgeOnboardProfile,
    },
    error::DeviceError,
    manager::OutputMode,
    status::{DeviceTransportKind, RawDeviceId},
    transport::{DeviceHandle, DeviceTransport},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerOutputTarget {
    pub raw_device_id: RawDeviceId,
    pub transport: DeviceTransportKind,
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
        self.read_input_state_with_options(target, ControllerInputReadOptions::default())
    }

    pub fn read_input_state_with_options(
        &self,
        target: &ControllerOutputTarget,
        options: ControllerInputReadOptions,
    ) -> Result<Option<ControllerInputState>, DeviceError> {
        let session = self.session_for(target)?;
        let read_result = {
            let mut session = lock_session(&session);
            let mut buffer = [0_u8; 256];
            let mut input = None;
            let mut fault = None;
            for _ in 0..options.attempts.max(1) {
                let timeout = if input.is_some() {
                    options.subsequent_timeout
                } else {
                    options.first_timeout
                };
                match session.handle.read_timeout_into(&mut buffer, timeout) {
                    Ok(Some(size)) => {
                        if let Some(parsed) = parse_dualsense_input_state(&buffer[..size]) {
                            input = Some(parsed);
                        }
                    }
                    Ok(None) => {
                        if input.is_some() {
                            break;
                        }
                    }
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
        if !edge_onboard_write_transport_supported(target.transport) {
            return Err(DeviceError::TransportFault(
                "DualSense Edge onboard profile writes require USB or Bluetooth HID feature report access"
                    .to_string(),
            ));
        }

        let session = self.session_for(target)?;
        let write_result = {
            let mut session = lock_session(&session);
            write_edge_onboard_profile_to_handle_for_transport(
                session.handle.as_mut(),
                target.transport,
                profile,
            )
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

#[cfg(test)]
mod tests;
