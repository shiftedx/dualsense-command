use std::time::Duration;
#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
use crate::enumeration::DeviceAccess;
use crate::{enumeration::RawHidDevice, error::DeviceError, status::RawDeviceId};

pub trait DeviceTransport: Send + Sync + 'static {
    fn enumerate(&self) -> Result<Vec<RawHidDevice>, DeviceError>;
    fn open(&self, id: &RawDeviceId) -> Result<Box<dyn DeviceHandle>, DeviceError>;
}

/// Result of submitting one output report to a [`DeviceHandle`].
///
/// `Executed` means the report was forwarded to the underlying HID backend;
/// `Suppressed` means the handle intentionally did not forward it (dry-run /
/// hardware writes disabled). Both carry a byte count so callers keep accurate
/// accounting, but only `Executed` means the controller was actually driven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// Report reached the HID backend; `bytes` is the count the backend accepted.
    /// A short count (< report length) is still surfaced so the caller can treat
    /// it as a fault.
    Executed { bytes: usize },
    /// Report was deliberately not forwarded to hardware; `bytes` is the offered
    /// report length. No HID I/O occurred and no controller state changed.
    Suppressed { bytes: usize },
}

impl WriteOutcome {
    /// Bytes accounted for: backend count on `Executed`, offered length on `Suppressed`.
    pub fn bytes(self) -> usize {
        match self {
            Self::Executed { bytes } | Self::Suppressed { bytes } => bytes,
        }
    }

    /// True only when the report actually reached the HID backend.
    pub fn reached_hardware(self) -> bool {
        matches!(self, Self::Executed { .. })
    }
}

pub trait DeviceHandle: Send {
    fn read_timeout(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError>;
    fn read_timeout_into(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<Option<usize>, DeviceError> {
        let Some(report) = self.read_timeout(timeout)? else {
            return Ok(None);
        };
        if report.len() > buffer.len() {
            return Err(DeviceError::TransportFault(format!(
                "input report exceeded caller buffer: {} bytes > {} bytes",
                report.len(),
                buffer.len()
            )));
        }
        buffer[..report.len()].copy_from_slice(&report);
        Ok(Some(report.len()))
    }
    fn write(&mut self, report: &[u8]) -> Result<WriteOutcome, DeviceError>;
    fn receive_feature_report(
        &mut self,
        _report_id: u8,
        _payload_len: usize,
    ) -> Result<Vec<u8>, DeviceError> {
        Err(DeviceError::TransportFault(
            "feature reports are not supported by this transport".to_string(),
        ))
    }
    fn send_feature_report(
        &mut self,
        _report_id: u8,
        _payload: &[u8],
    ) -> Result<usize, DeviceError> {
        Err(DeviceError::TransportFault(
            "feature reports are not supported by this transport".to_string(),
        ))
    }
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
#[derive(Clone, Debug, Default)]
pub struct MockTransport {
    inner: Arc<Mutex<MockTransportInner>>,
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
#[derive(Clone, Debug, Default)]
struct MockTransportInner {
    devices: Vec<RawHidDevice>,
    read_reports: BTreeMap<RawDeviceId, VecDeque<Vec<u8>>>,
    writes: BTreeMap<RawDeviceId, Vec<Vec<u8>>>,
    write_results: BTreeMap<RawDeviceId, VecDeque<Result<usize, DeviceError>>>,
    suppress_writes: bool,
    feature_reports: BTreeMap<(RawDeviceId, u8), VecDeque<Vec<u8>>>,
    feature_writes: BTreeMap<(RawDeviceId, u8), Vec<Vec<u8>>>,
    feature_write_results: BTreeMap<(RawDeviceId, u8), VecDeque<Result<usize, DeviceError>>>,
    enumerate_error: Option<DeviceError>,
    open_errors: BTreeMap<RawDeviceId, DeviceError>,
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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

    pub fn push_feature_report(&self, id: RawDeviceId, report_id: u8, report: Vec<u8>) {
        self.lock()
            .feature_reports
            .entry((id, report_id))
            .or_default()
            .push_back(report);
    }

    pub fn feature_writes_for(&self, id: &RawDeviceId, report_id: u8) -> Vec<Vec<u8>> {
        self.lock()
            .feature_writes
            .get(&(id.clone(), report_id))
            .cloned()
            .unwrap_or_default()
    }

    pub fn push_write_result(&self, id: RawDeviceId, result: Result<usize, DeviceError>) {
        self.lock()
            .write_results
            .entry(id)
            .or_default()
            .push_back(result);
    }

    /// Model a dry-run handle: when enabled, `write` still records the report for
    /// inspection but returns [`WriteOutcome::Suppressed`] instead of forwarding
    /// it, mirroring a transport opened with hardware writes disabled.
    pub fn set_suppress_writes(&self, suppress: bool) {
        self.lock().suppress_writes = suppress;
    }

    pub fn push_feature_write_result(
        &self,
        id: RawDeviceId,
        report_id: u8,
        result: Result<usize, DeviceError>,
    ) {
        self.lock()
            .feature_write_results
            .entry((id, report_id))
            .or_default()
            .push_back(result);
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

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
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

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
#[derive(Clone, Debug)]
struct MockDeviceHandle {
    id: RawDeviceId,
    inner: Arc<Mutex<MockTransportInner>>,
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
impl MockDeviceHandle {
    fn lock(&self) -> MutexGuard<'_, MockTransportInner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
impl DeviceHandle for MockDeviceHandle {
    fn read_timeout(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError> {
        Ok(self
            .lock()
            .read_reports
            .get_mut(&self.id)
            .and_then(VecDeque::pop_front))
    }

    fn read_timeout_into(
        &mut self,
        buffer: &mut [u8],
        _timeout: Duration,
    ) -> Result<Option<usize>, DeviceError> {
        let Some(report) = self
            .lock()
            .read_reports
            .get_mut(&self.id)
            .and_then(VecDeque::pop_front)
        else {
            return Ok(None);
        };
        if report.len() > buffer.len() {
            return Err(DeviceError::TransportFault(format!(
                "mock input report exceeded caller buffer: {} bytes > {} bytes",
                report.len(),
                buffer.len()
            )));
        }
        buffer[..report.len()].copy_from_slice(&report);
        Ok(Some(report.len()))
    }

    fn write(&mut self, report: &[u8]) -> Result<WriteOutcome, DeviceError> {
        let mut inner = self.lock();
        inner
            .writes
            .entry(self.id.clone())
            .or_default()
            .push(report.to_vec());
        if inner.suppress_writes {
            return Ok(WriteOutcome::Suppressed {
                bytes: report.len(),
            });
        }
        inner
            .write_results
            .get_mut(&self.id)
            .and_then(VecDeque::pop_front)
            .unwrap_or(Ok(report.len()))
            .map(|bytes| WriteOutcome::Executed { bytes })
    }

    fn receive_feature_report(
        &mut self,
        report_id: u8,
        _payload_len: usize,
    ) -> Result<Vec<u8>, DeviceError> {
        self.lock()
            .feature_reports
            .get_mut(&(self.id.clone(), report_id))
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                DeviceError::TransportFault(format!(
                    "no mock feature report queued for report id 0x{report_id:02x}"
                ))
            })
    }

    fn send_feature_report(&mut self, report_id: u8, payload: &[u8]) -> Result<usize, DeviceError> {
        let mut inner = self.lock();
        inner
            .feature_writes
            .entry((self.id.clone(), report_id))
            .or_default()
            .push(payload.to_vec());
        inner
            .feature_write_results
            .get_mut(&(self.id.clone(), report_id))
            .and_then(VecDeque::pop_front)
            .unwrap_or(Ok(payload.len()))
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
        assert_eq!(
            handle.write(&[4, 5]).unwrap(),
            WriteOutcome::Executed { bytes: 2 }
        );
        assert_eq!(transport.writes_for(&id), vec![vec![4, 5]]);
    }

    #[test]
    fn mock_handle_suppresses_writes_when_configured() {
        let device = RawHidDevice::mock("mock://dry-run");
        let id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.set_suppress_writes(true);

        let mut handle = transport.open(&id).expect("mock device should open");

        assert_eq!(
            handle.write(&[1, 2, 3]).unwrap(),
            WriteOutcome::Suppressed { bytes: 3 }
        );
        // The report is still recorded for inspection even though it was suppressed.
        assert_eq!(transport.writes_for(&id), vec![vec![1, 2, 3]]);
    }

    #[test]
    fn mock_handle_reads_and_records_feature_reports() {
        let device = RawHidDevice::mock("mock://pad-feature");
        let id = device.id.clone();
        let transport = MockTransport::with_devices(vec![device]);
        transport.push_feature_report(id.clone(), 0x70, vec![9, 8, 7]);

        let mut handle = transport.open(&id).expect("mock device should open");

        assert_eq!(
            handle.receive_feature_report(0x70, 64).unwrap(),
            vec![9, 8, 7]
        );
        assert_eq!(handle.send_feature_report(0x60, &[1, 2, 3]).unwrap(), 3);
        assert_eq!(transport.feature_writes_for(&id, 0x60), vec![vec![1, 2, 3]]);
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
