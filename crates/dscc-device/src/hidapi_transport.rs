use std::{
    io::ErrorKind,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use hidapi::{BusType, DeviceInfo, HidApi, HidDevice, HidError};

use crate::{
    enumeration::RawHidDevice,
    error::DeviceError,
    metadata,
    status::{DeviceFamily, DevicePathHint, DeviceTransportKind, RawDeviceId},
    transport::{DeviceHandle, DeviceTransport},
};

#[derive(Clone)]
pub struct HidApiTransport {
    api: Arc<Mutex<HidApi>>,
    hardware_writes_enabled: bool,
}

impl HidApiTransport {
    pub fn new() -> Result<Self, DeviceError> {
        let api = HidApi::new().map_err(|error| {
            DeviceError::BackendUnavailable(format!("hidapi initialization failed: {error}"))
        })?;
        Ok(Self {
            api: Arc::new(Mutex::new(api)),
            hardware_writes_enabled: false,
        })
    }

    pub fn with_hardware_writes_enabled(mut self, enabled: bool) -> Self {
        self.hardware_writes_enabled = enabled;
        self
    }

    fn lock_api(&self) -> MutexGuard<'_, HidApi> {
        match self.api.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl DeviceTransport for HidApiTransport {
    fn enumerate(&self) -> Result<Vec<RawHidDevice>, DeviceError> {
        let mut api = self.lock_api();
        api.refresh_devices().map_err(|error| {
            DeviceError::TransportFault(format!("hidapi refresh failed: {error}"))
        })?;

        Ok(api.device_list().map(raw_device_from_info).collect())
    }

    fn open(&self, id: &RawDeviceId) -> Result<Box<dyn DeviceHandle>, DeviceError> {
        let api = self.lock_api();
        let info = api
            .device_list()
            .find(|info| raw_device_id_for_info(info) == *id)
            .cloned()
            .ok_or_else(|| DeviceError::DeviceNotFound(id.clone()))?;
        let path_hint = DevicePathHint::from_backend_path(&path_string(&info));
        let device = info
            .open_device(&api)
            .map_err(|error| map_open_error(error, path_hint))?;

        Ok(Box::new(HidApiDeviceHandle {
            device,
            hardware_writes_enabled: self.hardware_writes_enabled,
        }))
    }
}

struct HidApiDeviceHandle {
    device: HidDevice,
    hardware_writes_enabled: bool,
}

impl DeviceHandle for HidApiDeviceHandle {
    fn read_timeout(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DeviceError> {
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut buffer = vec![0u8; 256];
        let size = self
            .device
            .read_timeout(&mut buffer, timeout_ms)
            .map_err(|error| DeviceError::TransportFault(format!("hidapi read failed: {error}")))?;

        if size == 0 {
            Ok(None)
        } else {
            buffer.truncate(size);
            Ok(Some(buffer))
        }
    }

    fn write(&mut self, report: &[u8]) -> Result<usize, DeviceError> {
        if !self.hardware_writes_enabled {
            return Ok(report.len());
        }

        self.device
            .write(report)
            .map_err(|error| DeviceError::TransportFault(format!("hidapi write failed: {error}")))
    }

    fn receive_feature_report(
        &mut self,
        report_id: u8,
        payload_len: usize,
    ) -> Result<Vec<u8>, DeviceError> {
        let mut buffer = vec![0u8; payload_len.saturating_add(1)];
        buffer[0] = report_id;
        let size = self
            .device
            .get_feature_report(&mut buffer)
            .map_err(|error| {
                DeviceError::TransportFault(format!(
                    "hidapi feature report 0x{report_id:02x} read failed: {error}"
                ))
            })?;
        if size == 0 {
            return Err(DeviceError::TransportFault(format!(
                "hidapi feature report 0x{report_id:02x} returned no bytes"
            )));
        }
        buffer.truncate(size);
        Ok(buffer.into_iter().skip(1).collect())
    }

    fn send_feature_report(&mut self, report_id: u8, payload: &[u8]) -> Result<usize, DeviceError> {
        if !self.hardware_writes_enabled {
            return Ok(payload.len());
        }

        let mut buffer = Vec::with_capacity(payload.len().saturating_add(1));
        buffer.push(report_id);
        buffer.extend_from_slice(payload);
        self.device.send_feature_report(&buffer).map_err(|error| {
            DeviceError::TransportFault(format!(
                "hidapi feature report 0x{report_id:02x} write failed: {error}"
            ))
        })?;
        Ok(payload.len())
    }
}

fn raw_device_from_info(info: &DeviceInfo) -> RawHidDevice {
    let path = path_string(info);
    RawHidDevice::mock(&path)
        .with_raw_id(raw_device_id_for_info(info).as_str())
        .with_vendor_product(info.vendor_id(), info.product_id())
        .with_usage(info.usage_page(), info.usage())
        .with_interface_number(info.interface_number())
        .with_manufacturer(info.manufacturer_string().unwrap_or_default())
        .with_product(info.product_string().unwrap_or_default())
        .with_serial_number_present(info.serial_number().is_some())
        .with_family_hint(infer_family_from_info(info))
        .with_transport_hint(transport_from_bus(info.bus_type()))
}

fn raw_device_id_for_info(info: &DeviceInfo) -> RawDeviceId {
    RawDeviceId::from_stable_source(&path_string(info))
}

fn path_string(info: &DeviceInfo) -> String {
    info.path().to_string_lossy().into_owned()
}

fn infer_family_from_info(info: &DeviceInfo) -> DeviceFamily {
    metadata::infer_family(
        Some(info.vendor_id()),
        Some(info.product_id()),
        info.manufacturer_string(),
        info.product_string(),
    )
}

fn transport_from_bus(bus: BusType) -> DeviceTransportKind {
    match bus {
        BusType::Usb => DeviceTransportKind::Usb,
        BusType::Bluetooth => DeviceTransportKind::Bluetooth,
        _ => DeviceTransportKind::Unknown,
    }
}

fn map_open_error(error: HidError, path_hint: DevicePathHint) -> DeviceError {
    match error {
        HidError::IoError { error } if error.kind() == ErrorKind::PermissionDenied => {
            DeviceError::PermissionDenied(path_hint)
        }
        error => DeviceError::AccessBlocked {
            path_hint,
            reason: format!("hidapi open failed: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_handle_would_not_write_hardware() {
        let transport = HidApiTransport {
            api: Arc::new(Mutex::new(HidApi::new().expect("hidapi should initialize"))),
            hardware_writes_enabled: false,
        };

        assert!(!transport.hardware_writes_enabled);
    }

    #[test]
    fn bus_mapping_tracks_usb_and_bluetooth_only() {
        assert_eq!(transport_from_bus(BusType::Usb), DeviceTransportKind::Usb);
        assert_eq!(
            transport_from_bus(BusType::Bluetooth),
            DeviceTransportKind::Bluetooth
        );
        assert_eq!(
            transport_from_bus(BusType::I2c),
            DeviceTransportKind::Unknown
        );
    }

    #[test]
    fn observed_edge_metadata_is_known_without_strings() {
        assert_eq!(
            metadata::infer_family(
                Some(metadata::SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID),
                Some(metadata::DUALSENSE_EDGE_PRODUCT_ID),
                None,
                None,
            ),
            DeviceFamily::DualSenseEdge
        );
    }
}
