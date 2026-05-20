use crate::{
    enumeration::{list_sanitized_hid, probe_access, SanitizedHidDevice},
    error::DeviceError,
    events::DeviceEvent,
    registry::{DeviceRegistry, RegistryConfig},
    transport::DeviceTransport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    Mock,
    DryRunHid,
    HardwareOutput,
}

impl OutputMode {
    pub fn hardware_writes_enabled(self) -> bool {
        matches!(self, Self::HardwareOutput)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceConfig {
    pub detach_grace_scans: usize,
    pub output_mode: OutputMode,
    pub open_sessions: bool,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            detach_grace_scans: RegistryConfig::default().detach_grace_scans,
            output_mode: OutputMode::Mock,
            open_sessions: false,
        }
    }
}

pub struct DeviceManager<T: DeviceTransport> {
    transport: T,
    registry: DeviceRegistry,
    config: DeviceConfig,
}

impl<T: DeviceTransport> DeviceManager<T> {
    pub fn new(transport: T, config: DeviceConfig) -> Self {
        let registry = DeviceRegistry::with_config(RegistryConfig {
            detach_grace_scans: config.detach_grace_scans,
        });
        Self {
            transport,
            registry,
            config,
        }
    }

    pub fn with_default_config(transport: T) -> Self {
        Self::new(transport, DeviceConfig::default())
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.registry
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn poll_once(&mut self) -> Result<Vec<DeviceEvent>, DeviceError> {
        let mut devices = self.transport.enumerate()?;
        if self.config.open_sessions {
            probe_access(&self.transport, &mut devices);
        }
        Ok(self.registry.reconcile(devices))
    }

    pub fn sanitized_hid_listing(&self) -> Result<Vec<SanitizedHidDevice>, DeviceError> {
        list_sanitized_hid(&self.transport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        enumeration::RawHidDevice,
        status::{DeviceFamily, DeviceTransportKind},
        transport::MockTransport,
    };

    #[test]
    fn manager_polls_mock_transport_into_registry_events() {
        let transport = MockTransport::with_devices(vec![RawHidDevice::mock("mock://pad-a")
            .with_family_hint(DeviceFamily::DualSense)
            .with_transport_hint(DeviceTransportKind::Usb)]);
        let mut manager = DeviceManager::with_default_config(transport);

        let events = manager.poll_once().expect("mock poll should succeed");

        assert!(matches!(events.as_slice(), [DeviceEvent::Attached(_)]));
        assert_eq!(manager.registry().len(), 1);
    }

    #[test]
    fn default_output_mode_keeps_hardware_writes_disabled() {
        let config = DeviceConfig::default();

        assert_eq!(config.output_mode, OutputMode::Mock);
        assert!(!config.output_mode.hardware_writes_enabled());
    }
}
