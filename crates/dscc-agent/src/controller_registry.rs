use std::collections::BTreeMap;
#[cfg(target_os = "windows")]
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use dscc_core::{
    BatteryState, ConnectionState, ControllerCapabilities, ControllerFamily, ControllerId,
    ControllerInfo, ControllerState, ControllerTransportKind,
};
use dscc_device::{
    BatteryInfo as DeviceBatteryInfo, BatteryState as DeviceBatteryState,
    ConnectionState as DeviceConnectionState,
    ControllerCapabilities as DeviceControllerCapabilities, ControllerId as DeviceControllerId,
    ControllerInfo as DeviceControllerInfo, ControllerOutputTarget,
    ControllerState as DeviceControllerState, DeviceEvent, DeviceFamily, DeviceManager,
    DevicePathHint, DeviceTransport, DeviceTransportKind, RawDeviceId,
};
#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
use dscc_device::{MockTransport, RawHidDevice};

use crate::{
    ControllerDetail, ControllerDiagnostic, ControllerDiagnosticState, ControllerDiscoveryEvent,
    ControllerPermissionState, ControllerSummary, DevicePermissionProblem, DiagnosticSeverity,
    DiscoveredController, HealthCheck, RealtimeMessage,
};

#[cfg(target_os = "windows")]
const WINDOWS_PNP_CONTROLLER_CACHE_TTL: Duration = Duration::from_secs(60);

impl DiscoveredController {
    pub fn new(info: ControllerInfo, state: ControllerState) -> Self {
        Self {
            info,
            state,
            raw_device_id: None,
            name: None,
            transport_label: None,
            permission: ControllerPermissionState::Granted,
            diagnostics: Vec::new(),
        }
    }

    pub fn with_raw_device_id(mut self, raw_device_id: RawDeviceId) -> Self {
        self.raw_device_id = Some(raw_device_id);
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_transport_label(mut self, transport_label: impl Into<String>) -> Self {
        self.transport_label = Some(transport_label.into());
        self
    }

    pub fn with_permission(mut self, permission: ControllerPermissionState) -> Self {
        self.permission = permission;
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: ControllerDiagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }
}

impl ControllerDiagnostic {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Info,
            message: message.into(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
        }
    }
}

impl DevicePermissionProblem {
    pub fn for_controller(
        id: ControllerId,
        transport: ControllerTransportKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Some(id),
            transport: Some(transport),
            message: message.into(),
        }
    }

    pub fn global(message: impl Into<String>) -> Self {
        Self {
            id: None,
            transport: None,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ControllerRegistry {
    controllers: BTreeMap<String, ControllerRecord>,
    global_diagnostics: Vec<ControllerDiagnostic>,
}

impl ControllerRegistry {
    pub(crate) fn apply(&mut self, event: ControllerDiscoveryEvent) {
        match event {
            ControllerDiscoveryEvent::Attached(controller) => {
                if self.should_skip_attach(&controller) {
                    return;
                }
                self.remove_redundant_windows_pnp(&controller);
                let id = controller.info.id.0.clone();
                self.remove_disconnected_duplicate(&id, &controller.info);
                self.controllers
                    .insert(id, ControllerRecord::from_discovered(controller));
            }
            ControllerDiscoveryEvent::Detached(id) => {
                if let Some(record) = self.controllers.get_mut(&id.0) {
                    record.mark_disconnected();
                } else {
                    self.global_diagnostics.push(ControllerDiagnostic::warning(
                        "controller_detached_unknown",
                        format!("Received detach event for unknown controller {}", id.0),
                    ));
                }
            }
            ControllerDiscoveryEvent::StatusChanged(state) => {
                if let Some(record) = self.controllers.get_mut(&state.id.0) {
                    record.update_state(state);
                } else {
                    self.global_diagnostics.push(ControllerDiagnostic::warning(
                        "controller_status_unknown",
                        format!(
                            "Received status update for unknown controller {}",
                            state.id.0
                        ),
                    ));
                }
            }
            ControllerDiscoveryEvent::PermissionDenied(problem) => {
                let diagnostic =
                    ControllerDiagnostic::error("controller_permission_denied", problem.message);
                if let Some(id) = problem.id {
                    if let Some(record) = self.controllers.get_mut(&id.0) {
                        record.mark_permission_denied(diagnostic);
                    } else {
                        self.global_diagnostics.push(ControllerDiagnostic::error(
                            "controller_permission_denied",
                            format!("Permission denied for unknown controller {}", id.0),
                        ));
                    }
                } else {
                    self.global_diagnostics.push(diagnostic);
                }
            }
            ControllerDiscoveryEvent::Faulted { id, message } => {
                let diagnostic = ControllerDiagnostic::error("controller_faulted", message);
                if let Some(id) = id {
                    if let Some(record) = self.controllers.get_mut(&id.0) {
                        record.mark_faulted(diagnostic);
                    } else {
                        self.global_diagnostics.push(ControllerDiagnostic::error(
                            "controller_faulted",
                            format!("Fault event for unknown controller {}", id.0),
                        ));
                    }
                } else {
                    self.global_diagnostics.push(diagnostic);
                }
            }
        }
    }

    pub(crate) fn is_redundant_attach(&self, event: &ControllerDiscoveryEvent) -> bool {
        let ControllerDiscoveryEvent::Attached(controller) = event else {
            return false;
        };
        self.controllers
            .get(&controller.info.id.0)
            .is_some_and(|record| {
                record.state.connection == ConnectionState::Connected
                    && record.matches_identity(&controller.info)
            })
    }

    fn should_skip_attach(&self, controller: &DiscoveredController) -> bool {
        is_windows_pnp_controller_id(&controller.info.id.0)
            && self.controllers.values().any(|record| {
                !is_windows_pnp_controller_id(&record.info.id.0)
                    && record.state.connection == ConnectionState::Connected
            })
    }

    fn remove_redundant_windows_pnp(&mut self, controller: &DiscoveredController) {
        if is_windows_pnp_controller_id(&controller.info.id.0) {
            return;
        }
        self.controllers.retain(|id, record| {
            !(is_windows_pnp_controller_id(id)
                && record.state.connection == ConnectionState::Connected)
        });
    }

    fn remove_disconnected_duplicate(&mut self, attached_id: &str, attached_info: &ControllerInfo) {
        let duplicate_id = self
            .controllers
            .iter()
            .find(|(id, record)| {
                id.as_str() != attached_id
                    && record.state.connection != ConnectionState::Connected
                    && record.matches_identity(attached_info)
            })
            .map(|(id, _)| id.clone());

        if let Some(id) = duplicate_id {
            self.controllers.remove(&id);
        }
    }

    pub(crate) fn detail(&self, id: &str) -> Option<ControllerDetail> {
        self.controllers.get(id).map(ControllerRecord::detail)
    }

    pub(crate) fn output_target(&self, id: &str) -> Option<ControllerOutputTarget> {
        self.controllers
            .get(id)
            .and_then(ControllerRecord::output_target)
    }

    pub(crate) fn summaries(&self) -> Vec<ControllerSummary> {
        self.controllers
            .values()
            .map(ControllerRecord::summary)
            .collect()
    }

    fn summary_for(&self, id: &ControllerId) -> Option<ControllerSummary> {
        self.controllers.get(&id.0).map(ControllerRecord::summary)
    }

    pub(crate) fn realtime_message_for(&self, event: &ControllerDiscoveryEvent) -> RealtimeMessage {
        match event {
            ControllerDiscoveryEvent::Attached(controller) => RealtimeMessage {
                kind: "controller_attached".to_string(),
                controller: self.summary_for(&controller.info.id),
                message: None,
            },
            ControllerDiscoveryEvent::Detached(id) => RealtimeMessage {
                kind: "controller_detached".to_string(),
                controller: self.summary_for(id),
                message: None,
            },
            ControllerDiscoveryEvent::StatusChanged(state) => RealtimeMessage {
                kind: "controller_status".to_string(),
                controller: self.summary_for(&state.id),
                message: None,
            },
            ControllerDiscoveryEvent::PermissionDenied(problem) => RealtimeMessage {
                kind: "controller_permission_denied".to_string(),
                controller: problem.id.as_ref().and_then(|id| self.summary_for(id)),
                message: Some(problem.message.clone()),
            },
            ControllerDiscoveryEvent::Faulted { id, message } => RealtimeMessage {
                kind: "controller_faulted".to_string(),
                controller: id.as_ref().and_then(|id| self.summary_for(id)),
                message: Some(message.clone()),
            },
        }
    }

    pub(crate) fn health_checks(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();
        if self.controllers.is_empty() {
            checks.push(HealthCheck {
                name: "controller-discovery".to_string(),
                status: "warning".to_string(),
                detail: "No supported controllers are known to the agent".to_string(),
            });
        }

        for record in self.controllers.values() {
            checks.push(record.health_check());
        }

        checks.extend(
            self.global_diagnostics
                .iter()
                .map(|diagnostic| HealthCheck {
                    name: diagnostic.code.clone(),
                    status: severity_status(diagnostic.severity).to_string(),
                    detail: diagnostic.message.clone(),
                }),
        );

        checks
    }
}

#[derive(Debug, Clone)]
struct ControllerRecord {
    info: ControllerInfo,
    state: ControllerState,
    raw_device_id: Option<RawDeviceId>,
    name: String,
    transport: String,
    permission: ControllerPermissionState,
    diagnostic_state: ControllerDiagnosticState,
    diagnostics: Vec<ControllerDiagnostic>,
}

impl ControllerRecord {
    fn from_discovered(controller: DiscoveredController) -> Self {
        let diagnostic_state =
            diagnostic_state_for(controller.permission, controller.state.connection);
        let name = controller
            .name
            .unwrap_or_else(|| family_label(controller.info.family).to_string());
        let transport = controller
            .transport_label
            .unwrap_or_else(|| transport_label(controller.info.transport).to_string());

        Self {
            info: controller.info,
            state: controller.state,
            raw_device_id: controller.raw_device_id,
            name,
            transport,
            permission: controller.permission,
            diagnostic_state,
            diagnostics: controller.diagnostics,
        }
    }

    fn update_state(&mut self, state: ControllerState) {
        self.state = state;
        if self.permission != ControllerPermissionState::Denied {
            self.diagnostic_state = diagnostic_state_for(self.permission, self.state.connection);
        }
    }

    fn mark_disconnected(&mut self) {
        self.state.connection = ConnectionState::Disconnected;
        self.diagnostic_state = ControllerDiagnosticState::Disconnected;
        self.diagnostics.push(ControllerDiagnostic::warning(
            "controller_disconnected",
            "Controller was detached from the device backend",
        ));
    }

    fn mark_permission_denied(&mut self, diagnostic: ControllerDiagnostic) {
        self.permission = ControllerPermissionState::Denied;
        self.diagnostic_state = ControllerDiagnosticState::PermissionDenied;
        self.diagnostics.push(diagnostic);
    }

    fn mark_faulted(&mut self, diagnostic: ControllerDiagnostic) {
        self.diagnostic_state = ControllerDiagnosticState::Faulted;
        self.diagnostics.push(diagnostic);
    }

    fn summary(&self) -> ControllerSummary {
        ControllerSummary {
            id: self.info.id.0.clone(),
            name: self.name.clone(),
            model: family_label(self.info.family).to_string(),
            transport: self.transport.clone(),
            connected: self.state.connection == ConnectionState::Connected,
            connection_state: self.state.connection,
            battery_percent: battery_percent_for(&self.state),
            battery_state: self.state.battery_state,
            permission: self.permission,
            diagnostic_state: self.diagnostic_state,
        }
    }

    fn detail(&self) -> ControllerDetail {
        let summary = self.summary();
        ControllerDetail {
            id: summary.id,
            name: summary.name,
            model: summary.model,
            transport: summary.transport,
            connected: summary.connected,
            connection_state: summary.connection_state,
            battery_percent: summary.battery_percent,
            battery_state: summary.battery_state,
            permission: summary.permission,
            diagnostic_state: summary.diagnostic_state,
            vendor_id: self.info.vendor_id,
            product_id: self.info.product_id,
            capabilities: self.info.capabilities.clone(),
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn output_target(&self) -> Option<ControllerOutputTarget> {
        if self.state.connection != ConnectionState::Connected
            || self.permission != ControllerPermissionState::Granted
            || !self.info.capabilities.adaptive_triggers
        {
            return None;
        }

        Some(ControllerOutputTarget {
            raw_device_id: self.raw_device_id.clone()?,
            transport: device_transport_from_core(self.info.transport),
        })
    }

    fn health_check(&self) -> HealthCheck {
        let status = match self.diagnostic_state {
            ControllerDiagnosticState::Ok => "ok",
            ControllerDiagnosticState::Disconnected => "warning",
            ControllerDiagnosticState::PermissionDenied => "blocked",
            ControllerDiagnosticState::CannotOpen => "error",
            ControllerDiagnosticState::Unsupported => "warning",
            ControllerDiagnosticState::Faulted => "error",
            ControllerDiagnosticState::Unknown => "warning",
        };

        HealthCheck {
            name: format!("controller:{}", self.info.id.0),
            status: status.to_string(),
            detail: match self.diagnostic_state {
                ControllerDiagnosticState::Ok => {
                    format!("{} connected over {}", self.name, self.transport)
                }
                ControllerDiagnosticState::Disconnected => {
                    format!("{} is known but currently disconnected", self.name)
                }
                ControllerDiagnosticState::PermissionDenied => self
                    .diagnostics
                    .last()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| format!("Permission denied for {}", self.name)),
                ControllerDiagnosticState::CannotOpen => {
                    format!("{} is present but cannot be opened", self.name)
                }
                ControllerDiagnosticState::Unsupported => {
                    format!("{} is not a supported controller variant", self.name)
                }
                ControllerDiagnosticState::Faulted => self
                    .diagnostics
                    .last()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| format!("{} reported a transport fault", self.name)),
                ControllerDiagnosticState::Unknown => {
                    format!("{} has unknown controller status", self.name)
                }
            },
        }
    }

    fn matches_identity(&self, info: &ControllerInfo) -> bool {
        self.info.family == info.family
            && self.info.transport == info.transport
            && self.info.vendor_id == info.vendor_id
            && self.info.product_id == info.product_id
    }
}

fn battery_percent_for(state: &ControllerState) -> Option<u8> {
    state
        .battery_percent
        .map(|percent| percent.min(100))
        .or(match state.battery_state {
            BatteryState::Full => Some(100),
            _ => None,
        })
}

#[cfg(any(test, debug_assertions, feature = "test-mocks"))]
pub(crate) fn mock_device_manager() -> DeviceManager<MockTransport> {
    let transport = MockTransport::with_devices(vec![
        RawHidDevice::mock("mock://dualsense-primary")
            .with_family_hint(DeviceFamily::DualSense)
            .with_transport_hint(DeviceTransportKind::Usb)
            .with_battery(DeviceBatteryInfo::new(
                Some(88),
                DeviceBatteryState::Discharging,
            ))
            .with_product("Mock DualSense"),
        RawHidDevice::mock("mock://dualsense-edge-secondary")
            .with_family_hint(DeviceFamily::DualSenseEdge)
            .with_transport_hint(DeviceTransportKind::Bluetooth)
            .with_battery(DeviceBatteryInfo::new(
                Some(62),
                DeviceBatteryState::Discharging,
            ))
            .with_product("Mock DualSense Edge"),
    ]);

    DeviceManager::with_default_config(transport)
}

pub(crate) fn controller_events_from_device_manager<T>(
    manager: &mut DeviceManager<T>,
) -> Result<Vec<ControllerDiscoveryEvent>, dscc_device::DeviceError>
where
    T: DeviceTransport,
{
    let device_events = manager.poll_once()?;
    let states = manager
        .registry()
        .entries()
        .map(|entry| (entry.info.id.as_str().to_string(), entry.state.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut events = device_events
        .into_iter()
        .map(|event| controller_event_from_device_event(event, &states))
        .collect::<Vec<_>>();

    if states.is_empty() {
        events.extend(windows_pnp_controller_events());
    }

    Ok(events)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct WindowsPnpControllerCache {
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
fn windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    static CACHE: std::sync::OnceLock<Mutex<WindowsPnpControllerCache>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(WindowsPnpControllerCache::default()));
    let now = Instant::now();
    {
        let cache = match cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.refreshed_at.is_some_and(|refreshed_at| {
            now.duration_since(refreshed_at) < WINDOWS_PNP_CONTROLLER_CACHE_TTL
        }) {
            return cache.events.clone();
        }
    }

    let events = discover_windows_pnp_controller_events();
    let mut cache = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.events = events.clone();
    cache.refreshed_at = Some(Instant::now());
    events
}

#[cfg(target_os = "windows")]
fn discover_windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    let records = windows_setupapi_present_controller_records();
    if records.is_empty() {
        return Vec::new();
    }

    windows_pnp_controller_events_from_text(&records.join("\n"))
}

#[cfg(target_os = "windows")]
fn windows_setupapi_present_controller_records() -> Vec<String> {
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
        windows_utf16_bytes_to_search_text(&buffer[..valid_len])
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

        windows_utf16_units_to_search_text(&buffer)
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
        if windows_pnp_candidate_text_is_controller(&record) {
            records.push(record);
        }
        index += 1;
    }
    records
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_utf16_bytes_to_search_text(bytes: &[u8]) -> Option<String> {
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    windows_utf16_units_to_search_text(&units)
}

#[cfg(target_os = "windows")]
fn windows_utf16_units_to_search_text(units: &[u16]) -> Option<String> {
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
pub(crate) fn windows_pnp_candidate_text_is_controller(text: &str) -> bool {
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

#[cfg(not(target_os = "windows"))]
fn windows_pnp_controller_events() -> Vec<ControllerDiscoveryEvent> {
    Vec::new()
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_pnp_controller_events_from_text(text: &str) -> Vec<ControllerDiscoveryEvent> {
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
        events.push(windows_pnp_controller_event(
            "windows-pnp-dualsense-edge",
            "DualSense Edge",
            ControllerFamily::DualSenseEdge,
            0x0df2,
        ));
    }
    if found_standard {
        events.push(windows_pnp_controller_event(
            "windows-pnp-dualsense",
            "DualSense",
            ControllerFamily::DualSense,
            0x0ce6,
        ));
    }
    events
}

#[cfg(target_os = "windows")]
fn windows_pnp_controller_event(
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

pub(crate) fn is_windows_pnp_controller_id(id: &str) -> bool {
    id.starts_with("windows-pnp-")
}

fn controller_event_from_device_event(
    event: DeviceEvent,
    states: &BTreeMap<String, DeviceControllerState>,
) -> ControllerDiscoveryEvent {
    match event {
        DeviceEvent::Attached(info) => {
            let raw_device_id = info.raw_device_id.clone();
            let state = states
                .get(info.id.as_str())
                .cloned()
                .unwrap_or_else(|| connected_state_for(&info));
            let controller_info = core_controller_info(&info, state.connection);
            let controller_state = core_controller_state(&state);
            ControllerDiscoveryEvent::Attached(
                DiscoveredController::new(controller_info, controller_state)
                    .with_raw_device_id(raw_device_id)
                    .with_transport_label(device_transport_label(info.transport))
                    .with_diagnostic(ControllerDiagnostic::info(
                        "device_backend",
                        "Controller discovered through dscc-device",
                    )),
            )
        }
        DeviceEvent::Detached(id) => ControllerDiscoveryEvent::Detached(core_controller_id(id)),
        DeviceEvent::StatusChanged(state) => {
            ControllerDiscoveryEvent::StatusChanged(core_controller_state(&state))
        }
        DeviceEvent::PermissionDenied(path_hint) => ControllerDiscoveryEvent::PermissionDenied(
            DevicePermissionProblem::global(permission_denied_message(&path_hint)),
        ),
        DeviceEvent::Faulted { id, message } => ControllerDiscoveryEvent::Faulted {
            id: id.map(core_controller_id),
            message,
        },
    }
}

fn connected_state_for(info: &DeviceControllerInfo) -> DeviceControllerState {
    DeviceControllerState {
        id: info.id.clone(),
        connection: DeviceConnectionState::Connected,
        battery: DeviceBatteryInfo::UNKNOWN,
    }
}

fn core_controller_id(id: DeviceControllerId) -> ControllerId {
    ControllerId(id.as_str().to_string())
}

fn core_controller_info(
    info: &DeviceControllerInfo,
    connection: DeviceConnectionState,
) -> ControllerInfo {
    ControllerInfo {
        id: ControllerId(info.id.as_str().to_string()),
        vendor_id: info.vendor_id.unwrap_or(0),
        product_id: info.product_id.unwrap_or(0),
        family: core_family(info.family),
        transport: core_transport(info.transport),
        connection: core_connection(connection),
        capabilities: core_capabilities(&info.capabilities),
    }
}

fn core_controller_state(state: &DeviceControllerState) -> ControllerState {
    ControllerState {
        id: ControllerId(state.id.as_str().to_string()),
        connection: core_connection(state.connection),
        battery_percent: state.battery.percent,
        battery_state: core_battery_state(state.battery.state),
    }
}

fn core_capabilities(capabilities: &DeviceControllerCapabilities) -> ControllerCapabilities {
    ControllerCapabilities {
        adaptive_triggers: capabilities.adaptive_triggers,
        lightbar: capabilities.lightbar,
        player_leds: capabilities.player_leds,
        rumble: capabilities.rumble,
        microphone_led: capabilities.microphone_led,
        edge_buttons: capabilities.edge_buttons,
    }
}

fn core_family(family: DeviceFamily) -> ControllerFamily {
    match family {
        DeviceFamily::DualSense => ControllerFamily::DualSense,
        DeviceFamily::DualSenseEdge => ControllerFamily::DualSenseEdge,
        DeviceFamily::UnknownSony | DeviceFamily::Unknown => ControllerFamily::UnknownSony,
    }
}

fn core_transport(transport: DeviceTransportKind) -> ControllerTransportKind {
    match transport {
        DeviceTransportKind::Usb => ControllerTransportKind::Usb,
        DeviceTransportKind::Bluetooth => ControllerTransportKind::Bluetooth,
        DeviceTransportKind::Unknown => ControllerTransportKind::Unknown,
    }
}

fn device_transport_from_core(transport: ControllerTransportKind) -> DeviceTransportKind {
    match transport {
        ControllerTransportKind::Usb => DeviceTransportKind::Usb,
        ControllerTransportKind::Bluetooth => DeviceTransportKind::Bluetooth,
        ControllerTransportKind::Unknown => DeviceTransportKind::Unknown,
    }
}

fn core_connection(connection: DeviceConnectionState) -> ConnectionState {
    match connection {
        DeviceConnectionState::Connected => ConnectionState::Connected,
        DeviceConnectionState::Disconnected => ConnectionState::Disconnected,
        DeviceConnectionState::Unknown => ConnectionState::Unknown,
    }
}

fn core_battery_state(state: DeviceBatteryState) -> BatteryState {
    match state {
        DeviceBatteryState::Unknown => BatteryState::Unknown,
        DeviceBatteryState::Discharging => BatteryState::Discharging,
        DeviceBatteryState::Charging => BatteryState::Charging,
        DeviceBatteryState::Full => BatteryState::Full,
    }
}

fn device_transport_label(transport: DeviceTransportKind) -> &'static str {
    match transport {
        DeviceTransportKind::Usb => "usb",
        DeviceTransportKind::Bluetooth => "bluetooth",
        DeviceTransportKind::Unknown => "unknown",
    }
}

fn permission_denied_message(path_hint: &DevicePathHint) -> String {
    format!("Permission denied while opening controller candidate at {path_hint}")
}

fn family_label(family: ControllerFamily) -> &'static str {
    match family {
        ControllerFamily::DualSense => "DualSense",
        ControllerFamily::DualSenseEdge => "DualSense Edge",
        ControllerFamily::UnknownSony => "Unknown Sony Controller",
    }
}

fn transport_label(transport: ControllerTransportKind) -> &'static str {
    match transport {
        ControllerTransportKind::Usb => "usb",
        ControllerTransportKind::Bluetooth => "bluetooth",
        ControllerTransportKind::Unknown => "unknown",
    }
}

fn diagnostic_state_for(
    permission: ControllerPermissionState,
    connection: ConnectionState,
) -> ControllerDiagnosticState {
    if permission == ControllerPermissionState::Denied {
        return ControllerDiagnosticState::PermissionDenied;
    }

    match connection {
        ConnectionState::Connected => ControllerDiagnosticState::Ok,
        ConnectionState::Disconnected => ControllerDiagnosticState::Disconnected,
        ConnectionState::Unknown => ControllerDiagnosticState::Unknown,
    }
}

fn severity_status(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    }
}
