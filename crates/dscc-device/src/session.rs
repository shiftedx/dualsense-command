use std::time::Duration;

use crate::{
    error::DeviceError,
    events::DeviceEvent,
    status::{ControllerId, RawDeviceId},
    transport::DeviceHandle,
};

pub struct DeviceSession {
    controller_id: ControllerId,
    raw_device_id: RawDeviceId,
    handle: Box<dyn DeviceHandle>,
}

impl DeviceSession {
    pub fn new(
        controller_id: ControllerId,
        raw_device_id: RawDeviceId,
        handle: Box<dyn DeviceHandle>,
    ) -> Self {
        Self {
            controller_id,
            raw_device_id,
            handle,
        }
    }

    pub fn controller_id(&self) -> &ControllerId {
        &self.controller_id
    }

    pub fn raw_device_id(&self) -> &RawDeviceId {
        &self.raw_device_id
    }

    pub fn poll_once(&mut self, timeout: Duration) -> Result<Option<DeviceEvent>, DeviceError> {
        let _report = self.handle.read_timeout(timeout)?;
        Ok(None)
    }
}
