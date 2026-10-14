use std::time::Duration;

use crate::{
    capabilities::infer_capabilities,
    error::DeviceError,
    status::{
        BatteryInfo, ConnectionState, ControllerId, ControllerInfo, ControllerState, DeviceFamily,
        DevicePathHint, DeviceTransportKind, RawDeviceId,
    },
    transport::{DeviceHandle, DeviceTransport},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceAccess {
    Available,
    PermissionDenied,
    Busy,
    Unavailable,
}

impl DeviceAccess {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawHidDevice {
    pub id: RawDeviceId,
    pub path_hint: DevicePathHint,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub interface_number: Option<i32>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number_present: bool,
    pub family_hint: DeviceFamily,
    pub transport_hint: DeviceTransportKind,
    pub battery: BatteryInfo,
    pub access: DeviceAccess,
}

impl RawHidDevice {
    pub fn mock(backend_path: impl AsRef<str>) -> Self {
        let backend_path = backend_path.as_ref();
        Self {
            id: RawDeviceId::from_stable_source(backend_path),
            path_hint: DevicePathHint::from_backend_path(backend_path),
            vendor_id: None,
            product_id: None,
            usage_page: None,
            usage: None,
            interface_number: None,
            manufacturer: None,
            product: None,
            serial_number_present: false,
            family_hint: DeviceFamily::Unknown,
            transport_hint: DeviceTransportKind::Unknown,
            battery: BatteryInfo::UNKNOWN,
            access: DeviceAccess::Available,
        }
    }

    pub fn with_raw_id(mut self, id: impl Into<String>) -> Self {
        self.id = RawDeviceId::new(id);
        self
    }

    pub fn with_vendor_product(mut self, vendor_id: u16, product_id: u16) -> Self {
        self.vendor_id = Some(vendor_id);
        self.product_id = Some(product_id);
        self
    }

    pub fn with_usage(mut self, usage_page: u16, usage: u16) -> Self {
        self.usage_page = Some(usage_page);
        self.usage = Some(usage);
        self
    }

    pub fn with_interface_number(mut self, interface_number: i32) -> Self {
        self.interface_number = Some(interface_number);
        self
    }

    pub fn with_manufacturer(mut self, manufacturer: impl Into<String>) -> Self {
        self.manufacturer = Some(manufacturer.into());
        self
    }

    pub fn with_product(mut self, product: impl Into<String>) -> Self {
        self.product = Some(product.into());
        self
    }

    pub fn with_serial_number_present(mut self, serial_number_present: bool) -> Self {
        self.serial_number_present = serial_number_present;
        self
    }

    pub fn with_family_hint(mut self, family: DeviceFamily) -> Self {
        self.family_hint = family;
        self
    }

    pub fn with_transport_hint(mut self, transport: DeviceTransportKind) -> Self {
        self.transport_hint = transport;
        self
    }

    pub fn with_battery(mut self, battery: BatteryInfo) -> Self {
        self.battery = battery;
        self
    }

    pub fn with_access(mut self, access: DeviceAccess) -> Self {
        self.access = access;
        self
    }

    pub fn is_supported_controller(&self) -> bool {
        self.family_hint.is_supported_controller()
    }

    pub fn sanitized(&self) -> SanitizedHidDevice {
        SanitizedHidDevice {
            id: self.id.clone(),
            path_hint: self.path_hint.clone(),
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            usage_page: self.usage_page,
            usage: self.usage,
            interface_number: self.interface_number,
            manufacturer: self.manufacturer.clone(),
            product: self.product.clone(),
            serial_number_present: self.serial_number_present,
            family_hint: self.family_hint,
            transport_hint: self.transport_hint,
            access: self.access,
        }
    }

    pub(crate) fn controller_info(&self, controller_id: ControllerId) -> ControllerInfo {
        ControllerInfo {
            id: controller_id,
            raw_device_id: self.id.clone(),
            path_hint: self.path_hint.clone(),
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            family: self.family_hint,
            transport: self.transport_hint,
            capabilities: infer_capabilities(self.family_hint),
        }
    }

    pub(crate) fn controller_state(&self, controller_id: ControllerId) -> ControllerState {
        ControllerState {
            id: controller_id,
            connection: ConnectionState::Connected,
            battery: self.battery,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHidDevice {
    pub id: RawDeviceId,
    pub path_hint: DevicePathHint,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub interface_number: Option<i32>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number_present: bool,
    pub family_hint: DeviceFamily,
    pub transport_hint: DeviceTransportKind,
    pub access: DeviceAccess,
}

pub fn list_sanitized_hid<T: DeviceTransport>(
    transport: &T,
) -> Result<Vec<SanitizedHidDevice>, DeviceError> {
    transport
        .enumerate()
        .map(|devices| devices.iter().map(RawHidDevice::sanitized).collect())
}

pub fn list_sanitized_hid_with_access_probe<T: DeviceTransport>(
    transport: &T,
) -> Result<Vec<SanitizedHidDevice>, DeviceError> {
    let mut devices = transport.enumerate()?;
    probe_access(transport, &mut devices);
    Ok(devices.iter().map(RawHidDevice::sanitized).collect())
}

pub(crate) fn probe_access<T: DeviceTransport>(transport: &T, devices: &mut [RawHidDevice]) {
    for device in devices {
        if !should_probe_access(device) || device.access != DeviceAccess::Available {
            continue;
        }

        match transport.open(&device.id) {
            Ok(mut handle) => {
                if let Some(battery) = read_dualsense_battery(&mut *handle) {
                    device.battery = battery;
                }
            }
            Err(DeviceError::PermissionDenied(_)) => {
                device.access = DeviceAccess::PermissionDenied;
            }
            Err(DeviceError::AccessBlocked { .. }) => {
                device.access = DeviceAccess::Busy;
            }
            Err(DeviceError::DeviceNotFound(_)) => {
                device.access = DeviceAccess::Unavailable;
            }
            Err(DeviceError::BackendUnavailable(_))
            | Err(DeviceError::TransportFault(_))
            | Err(DeviceError::ShutdownRequested) => {
                device.access = DeviceAccess::Unavailable;
            }
        }
    }
}

fn read_dualsense_battery(handle: &mut dyn DeviceHandle) -> Option<BatteryInfo> {
    for _ in 0..4 {
        let Ok(Some(report)) = handle.read_timeout(Duration::from_millis(8)) else {
            continue;
        };
        if let Some(battery) = parse_dualsense_battery_report(&report) {
            return Some(battery);
        }
    }
    None
}

fn parse_dualsense_battery_report(report: &[u8]) -> Option<BatteryInfo> {
    let status0 = match report.first().copied()? {
        // USB full input report: report id 0x01 followed by the common DualSense input report.
        0x01 if report.len() >= 54 => report[53],
        // Bluetooth full input report: report id 0x31, one BT header byte, then common report.
        0x31 if report.len() >= 55 => report[54],
        _ => return None,
    };
    Some(battery_from_status0(status0))
}

fn battery_from_status0(status0: u8) -> BatteryInfo {
    let capacity = status0 & 0x0f;
    let charging_status = (status0 >> 4) & 0x0f;
    let percent = (capacity.saturating_mul(10)).saturating_add(5).min(100);

    match charging_status {
        0x0 => BatteryInfo::new(Some(percent), crate::status::BatteryState::Discharging),
        0x1 => BatteryInfo::new(Some(percent), crate::status::BatteryState::Charging),
        0x2 => BatteryInfo::new(Some(100), crate::status::BatteryState::Full),
        _ => BatteryInfo::UNKNOWN,
    }
}

fn should_probe_access(device: &RawHidDevice) -> bool {
    matches!(
        device.family_hint,
        DeviceFamily::DualSense | DeviceFamily::DualSenseEdge | DeviceFamily::UnknownSony
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;

    #[test]
    fn sanitized_listing_omits_raw_backend_path_but_keeps_research_fields() {
        let raw_path = "/dev/hidraw42/private-interface";
        let device = RawHidDevice::mock(raw_path)
            .with_vendor_product(0x1234, 0xabcd)
            .with_usage(1, 5)
            .with_interface_number(2)
            .with_manufacturer("Example Manufacturer")
            .with_product("Example Controller")
            .with_serial_number_present(true)
            .with_family_hint(DeviceFamily::UnknownSony)
            .with_transport_hint(DeviceTransportKind::Usb);
        let transport = MockTransport::with_devices(vec![device]);

        let listing = list_sanitized_hid(&transport).expect("mock listing should succeed");

        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].vendor_id, Some(0x1234));
        assert_eq!(listing[0].product_id, Some(0xabcd));
        assert!(listing[0].serial_number_present);
        assert!(!listing[0].path_hint.to_string().contains(raw_path));
    }

    #[test]
    fn access_probe_marks_busy_controller_without_attaching_private_path() {
        let device = RawHidDevice::mock("mock://edge")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_access(DeviceAccess::Busy);
        let transport = MockTransport::with_devices(vec![device]);

        let listing = list_sanitized_hid_with_access_probe(&transport).unwrap();

        assert_eq!(listing[0].access, DeviceAccess::Busy);
        assert!(!listing[0].path_hint.to_string().contains("mock://edge"));
    }

    #[test]
    fn parses_dualsense_usb_battery_status_report() {
        let mut report = vec![0_u8; 64];
        report[0] = 0x01;
        report[53] = 0x17;

        let battery = parse_dualsense_battery_report(&report).expect("battery parses");

        assert_eq!(battery.percent, Some(75));
        assert_eq!(battery.state, crate::status::BatteryState::Charging);
    }

    #[test]
    fn parses_dualsense_bluetooth_full_battery_status_report() {
        let mut report = vec![0_u8; 78];
        report[0] = 0x31;
        report[54] = 0x25;

        let battery = parse_dualsense_battery_report(&report).expect("battery parses");

        assert_eq!(battery.percent, Some(100));
        assert_eq!(battery.state, crate::status::BatteryState::Full);
    }
}
