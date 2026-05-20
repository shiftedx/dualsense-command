use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use crate::{
    enumeration::{DeviceAccess, RawHidDevice},
    error::DeviceError,
    status::RawDeviceId,
};

pub trait DeviceTransport: Send + Sync + 'static {
    fn enumerate(&self) -> Result<Vec<RawHidDevice>, DeviceError>;
    fn open(&self, id: &RawDeviceId) -> Result<Box<dyn DeviceHandle>, DeviceError>;
}

pub trait DeviceHandle: Send {
    fn read_timeout(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError>;
    fn write(&mut self, report: &[u8]) -> Result<usize, DeviceError>;
}

#[derive(Clone, Debug, Default)]
pub struct MockTransport {
    inner: Arc<Mutex<MockTransportInner>>,
}

#[derive(Clone, Debug, Default)]
struct MockTransportInner {
    devices: Vec<RawHidDevice>,
    read_reports: BTreeMap<RawDeviceId, VecDeque<Vec<u8>>>,
    writes: BTreeMap<RawDeviceId, Vec<Vec<u8>>>,
    enumerate_error: Option<DeviceError>,
    open_errors: BTreeMap<RawDeviceId, DeviceError>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_devices(devices: Vec<RawHidDevice>) -> Self {
        let transport = Self::new();
        transport.set_devices(devices);
        transport
    }

    pub fn set_devices(&self, devices: Vec<RawHidDevice>) {
        self.lock().devices = devices;
    }

    pub fn push_read_report(&self, id: RawDeviceId, report: Vec<u8>) {
        self.lock()
            .read_reports
            .entry(id)
            .or_default()
            .push_back(report);
    }

    pub fn writes_for(&self, id: &RawDeviceId) -> Vec<Vec<u8>> {
        self.lock().writes.get(id).cloned().unwrap_or_default()
    }

    pub fn fail_enumeration(&self, error: DeviceError) {
        self.lock().enumerate_error = Some(error);
    }

    pub fn clear_enumeration_failure(&self) {
        self.lock().enumerate_error = None;
    }

    pub fn fail_open(&self, id: RawDeviceId, error: DeviceError) {
        self.lock().open_errors.insert(id, error);
    }

    fn lock(&self) -> MutexGuard<'_, MockTransportInner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl DeviceTransport for MockTransport {
    fn enumerate(&self) -> Result<Vec<RawHidDevice>, DeviceError> {
        let inner = self.lock();
        if let Some(error) = &inner.enumerate_error {
            return Err(error.clone());
        }
        Ok(inner.devices.clone())
    }

    fn open(&self, id: &RawDeviceId) -> Result<Box<dyn DeviceHandle>, DeviceError> {
        let inner = self.lock();
        if let Some(error) = inner.open_errors.get(id) {
            return Err(error.clone());
        }

        let device = inner
            .devices
            .iter()
            .find(|device| &device.id == id)
            .ok_or_else(|| DeviceError::DeviceNotFound(id.clone()))?;

        match device.access {
            DeviceAccess::Available => Ok(Box::new(MockDeviceHandle {
                id: id.clone(),
                inner: self.inner.clone(),
            })),
            DeviceAccess::PermissionDenied => {
                Err(DeviceError::PermissionDenied(device.path_hint.clone()))
            }
            DeviceAccess::Busy | DeviceAccess::Unavailable => Err(DeviceError::AccessBlocked {
                path_hint: device.path_hint.clone(),
                reason: format!("device access is {:?}", device.access),
            }),
        }
    }
}

#[derive(Clone, Debug)]
struct MockDeviceHandle {
    id: RawDeviceId,
    inner: Arc<Mutex<MockTransportInner>>,
}

impl MockDeviceHandle {
    fn lock(&self) -> MutexGuard<'_, MockTransportInner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl DeviceHandle for MockDeviceHandle {
    fn read_timeout(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError> {
        Ok(self
            .lock()
            .read_reports
            .get_mut(&self.id)
            .and_then(VecDeque::pop_front))
    }

    fn write(&mut self, report: &[u8]) -> Result<usize, DeviceError> {
        self.lock()
            .writes
            .entry(self.id.clone())
            .or_default()
            .push(report.to_vec());
        Ok(report.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enumeration::RawHidDevice;

    #[test]
    fn mock_handle_reads_queued_reports_and_records_writes() {
        let device = RawHidDevice::mock("mock://pad-a");
        let id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.push_read_report(id.clone(), vec![1, 2, 3]);

        let mut handle = transport.open(&id).expect("mock device should open");

        assert_eq!(
            handle.read_timeout(Duration::from_millis(1)).unwrap(),
            Some(vec![1, 2, 3])
        );
        assert_eq!(handle.read_timeout(Duration::from_millis(1)).unwrap(), None);
        assert_eq!(handle.write(&[4, 5]).unwrap(), 2);
        assert_eq!(transport.writes_for(&id), vec![vec![4, 5]]);
    }

    #[test]
    fn mock_open_reports_permission_denied_with_sanitized_hint() {
        let device =
            RawHidDevice::mock("mock://private-pad").with_access(DeviceAccess::PermissionDenied);
        let id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);

        let error = match transport.open(&id) {
            Ok(_) => panic!("open should be denied"),
            Err(error) => error,
        };

        assert!(matches!(error, DeviceError::PermissionDenied(_)));
        assert!(!error.to_string().contains("mock://private-pad"));
    }
}
