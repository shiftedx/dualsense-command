use crate::ControllerDiscoveryEvent;

#[cfg(target_os = "windows")]
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use crate::{ControllerDiagnostic, DiscoveredController};
#[cfg(target_os = "windows")]
use dscc_core::{
    BatteryState, ConnectionState, ControllerCapabilities, ControllerFamily, ControllerId,
    ControllerInfo, ControllerState, ControllerTransportKind,
};

#[cfg(target_os = "windows")]
const CONTROLLER_CACHE_TTL: Duration = Duration::from_secs(60);

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct ControllerCache {
    events: Vec<ControllerDiscoveryEvent>,
    refreshed_at: Option<Instant>,
}

#[cfg(target_os = "windows")]
struct SetupDiInfoSet(windows_sys::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO);

#[cfg(target_os = "windows")]
impl Drop for SetupDiInfoSet {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Devices::DeviceAndDriverInstallation::SetupDiDestroyDeviceInfoList(
                self.0,
            );
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn controller_events() -> Vec<ControllerDiscoveryEvent> {
    static CACHE: std::sync::OnceLock<Mutex<ControllerCache>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(ControllerCache::default()));
    let now = Instant::now();
    {
        let cache = match cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache
            .refreshed_at
            .is_some_and(|refreshed_at| now.duration_since(refreshed_at) < CONTROLLER_CACHE_TTL)
        {
            return cache.events.clone();
        }
    }

    let events = discover_controller_events();
    let mut cache = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.events = events.clone();
    cache.refreshed_at = Some(Instant::now());
    events
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn controller_events() -> Vec<ControllerDiscoveryEvent> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn discover_controller_events() -> Vec<ControllerDiscoveryEvent> {
    let records = setupapi_present_controller_records();
    if records.is_empty() {
        return Vec::new();
    }

    controller_events_from_text(&records.join("\n"))
}

#[cfg(target_os = "windows")]
fn setupapi_present_controller_records() -> Vec<String> {
    use windows_sys::Win32::{
        Devices::DeviceAndDriverInstallation::{
            SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
            SetupDiGetDeviceRegistryPropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO,
            SETUP_DI_REGISTRY_PROPERTY, SPDRP_DEVICEDESC, SPDRP_FRIENDLYNAME, SPDRP_HARDWAREID,
            SP_DEVINFO_DATA,
        },
        Foundation::{
            GetLastError, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE,
        },
    };

    fn registry_property_text(
        info_set: HDEVINFO,
        device_info: &SP_DEVINFO_DATA,
        property: SETUP_DI_REGISTRY_PROPERTY,
    ) -> Option<String> {
        let mut data_type = 0u32;
        let mut required_size = 0u32;
        let first_result = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                info_set,
                device_info,
                property,
                &mut data_type,
                std::ptr::null_mut(),
                0,
                &mut required_size,
            )
        };
        if first_result == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_INSUFFICIENT_BUFFER || required_size == 0 {
                return None;
            }
        }

        let mut buffer = vec![0u8; required_size as usize];
        let second_result = unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                info_set,
                device_info,
                property,
                &mut data_type,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut required_size,
            )
        };
        if second_result == 0 {
            return None;
        }

        let valid_len = (required_size as usize).min(buffer.len());
        utf16_bytes_to_search_text(&buffer[..valid_len])
    }

    fn instance_id_text(info_set: HDEVINFO, device_info: &SP_DEVINFO_DATA) -> Option<String> {
        let mut required_chars = 0u32;
        let first_result = unsafe {
            SetupDiGetDeviceInstanceIdW(
                info_set,
                device_info,
                std::ptr::null_mut(),
                0,
                &mut required_chars,
            )
        };
        if first_result == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_INSUFFICIENT_BUFFER || required_chars == 0 {
                return None;
            }
        }

        let mut buffer = vec![0u16; required_chars as usize];
        let second_result = unsafe {
            SetupDiGetDeviceInstanceIdW(
                info_set,
                device_info,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut required_chars,
            )
        };
        if second_result == 0 {
            return None;
        }

        utf16_units_to_search_text(&buffer)
    }

    let info_set = unsafe {
        SetupDiGetClassDevsW(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        )
    };
    if info_set == INVALID_HANDLE_VALUE as HDEVINFO {
        return Vec::new();
    }
    let info_set = SetupDiInfoSet(info_set);

    let mut records = Vec::new();
    let mut index = 0u32;
    loop {
        let mut device_info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        let enum_result = unsafe { SetupDiEnumDeviceInfo(info_set.0, index, &mut device_info) };
        if enum_result == 0 {
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                break;
            }
            index += 1;
            continue;
        }

        let mut fields = Vec::new();
        for property in [SPDRP_FRIENDLYNAME, SPDRP_DEVICEDESC, SPDRP_HARDWAREID] {
            if let Some(value) = registry_property_text(info_set.0, &device_info, property) {
                fields.push(value);
            }
        }
        if let Some(value) = instance_id_text(info_set.0, &device_info) {
            fields.push(value);
        }

        let record = fields.join("\t");
        if candidate_text_is_controller(&record) {
            records.push(record);
        }
        index += 1;
    }
    records
}

#[cfg(target_os = "windows")]
pub(crate) fn utf16_bytes_to_search_text(bytes: &[u8]) -> Option<String> {
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    utf16_units_to_search_text(&units)
}

#[cfg(target_os = "windows")]
fn utf16_units_to_search_text(units: &[u16]) -> Option<String> {
    let parts = units
        .split(|unit| *unit == 0)
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let text = String::from_utf16_lossy(part);
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(target_os = "windows")]
pub(crate) fn candidate_text_is_controller(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("dualsense")
        || lower.contains("wireless controller")
        || lower.contains("playstation")
        || lower.contains("vid_054c")
        || lower.contains("vid&054c")
        || lower.contains("pid_0df2")
        || lower.contains("pid&0df2")
        || lower.contains("pid_0ce6")
        || lower.contains("pid&0ce6")
}

#[cfg(target_os = "windows")]
pub(crate) fn controller_events_from_text(text: &str) -> Vec<ControllerDiscoveryEvent> {
    let mut found_edge = false;
    let mut found_explicit_standard = false;
    let mut found_generic_standard = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("dualsense edge")
            || lower.contains("pid&0df2")
            || lower.contains("pid_0df2")
        {
            found_edge = true;
            continue;
        }
        if lower.contains("pid&0ce6") || lower.contains("pid_0ce6") {
            found_explicit_standard = true;
            continue;
        }
        if lower.contains("dualsense") || lower.contains("wireless controller") {
            found_generic_standard = true;
        }
    }

    let found_standard = found_explicit_standard || (!found_edge && found_generic_standard);
    let mut events = Vec::new();
    if found_edge {
        events.push(controller_event(
            "windows-pnp-dualsense-edge",
            "DualSense Edge",
            ControllerFamily::DualSenseEdge,
            0x0df2,
        ));
    }
    if found_standard {
        events.push(controller_event(
            "windows-pnp-dualsense",
            "DualSense",
            ControllerFamily::DualSense,
            0x0ce6,
        ));
    }
    events
}

#[cfg(target_os = "windows")]
fn controller_event(
    id: &str,
    name: &str,
    family: ControllerFamily,
    product_id: u16,
) -> ControllerDiscoveryEvent {
    let info = ControllerInfo {
        id: ControllerId(id.to_string()),
        vendor_id: 0x054c,
        product_id,
        family,
        transport: ControllerTransportKind::Bluetooth,
        connection: ConnectionState::Connected,
        capabilities: ControllerCapabilities {
            adaptive_triggers: true,
            lightbar: true,
            player_leds: true,
            rumble: true,
            microphone_led: true,
            edge_buttons: family == ControllerFamily::DualSenseEdge,
        },
    };
    let state = ControllerState {
        id: info.id.clone(),
        connection: ConnectionState::Connected,
        battery_percent: None,
        battery_state: BatteryState::Unknown,
    };

    ControllerDiscoveryEvent::Attached(
        DiscoveredController::new(info, state)
            .with_name(name)
            .with_transport_label("bluetooth")
            .with_diagnostic(ControllerDiagnostic::warning(
                "windows_pnp_fallback",
                "Controller is present in Windows PnP, but hidapi did not expose an open HID handle; configuration is available and hardware output is disabled.",
            )),
    )
}

pub(crate) fn is_controller_id(id: &str) -> bool {
    id.starts_with("windows-pnp-")
}
