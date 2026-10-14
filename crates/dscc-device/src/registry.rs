use std::collections::{BTreeMap, BTreeSet};

use crate::{
    enumeration::{DeviceAccess, RawHidDevice},
    events::DeviceEvent,
    status::{ControllerId, ControllerInfo, ControllerState, RawDeviceId},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryConfig {
    pub detach_grace_scans: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            detach_grace_scans: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub info: ControllerInfo,
    pub state: ControllerState,
    missing_scans: usize,
}

impl RegistryEntry {
    pub fn missing_scans(&self) -> usize {
        self.missing_scans
    }
}

#[derive(Clone, Debug)]
pub struct DeviceRegistry {
    config: RegistryConfig,
    entries: BTreeMap<RawDeviceId, RegistryEntry>,
    assigned_controller_ids: BTreeMap<RawDeviceId, ControllerId>,
    next_controller_index: u64,
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self::with_config(RegistryConfig::default())
    }

    pub fn with_config(config: RegistryConfig) -> Self {
        Self {
            config,
            entries: BTreeMap::new(),
            assigned_controller_ids: BTreeMap::new(),
            next_controller_index: 1,
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &RegistryEntry> {
        self.entries.values()
    }

    pub fn controller_infos(&self) -> Vec<ControllerInfo> {
        self.entries
            .values()
            .map(|entry| entry.info.clone())
            .collect()
    }

    pub fn controller_states(&self) -> Vec<ControllerState> {
        self.entries
            .values()
            .map(|entry| entry.state.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn reconcile(&mut self, devices: Vec<RawHidDevice>) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        let mut seen_ids = BTreeSet::new();

        for device in devices {
            seen_ids.insert(device.id.clone());

            match device.access {
                DeviceAccess::Available => {}
                DeviceAccess::PermissionDenied => {
                    events.push(DeviceEvent::PermissionDenied(device.path_hint.clone()));
                    continue;
                }
                DeviceAccess::Busy | DeviceAccess::Unavailable => {
                    let controller_id = self
                        .entries
                        .get(&device.id)
                        .map(|entry| entry.info.id.clone());
                    events.push(DeviceEvent::Faulted {
                        id: controller_id,
                        message: format!("device access is {:?}", device.access),
                    });
                    continue;
                }
            }

            if !device.is_supported_controller() {
                continue;
            }

            let controller_id = self.controller_id_for(&device.id);
            let info = device.controller_info(controller_id.clone());
            let state = device.controller_state(controller_id);

            match self.entries.get_mut(&device.id) {
                Some(entry) => {
                    entry.missing_scans = 0;
                    entry.info = info;
                    if entry.state != state {
                        entry.state = state.clone();
                        events.push(DeviceEvent::StatusChanged(state));
                    }
                }
                None => {
                    self.entries.insert(
                        device.id,
                        RegistryEntry {
                            info: info.clone(),
                            state,
                            missing_scans: 0,
                        },
                    );
                    events.push(DeviceEvent::Attached(info));
                }
            }
        }

        let missing_ids = self
            .entries
            .keys()
            .filter(|id| !seen_ids.contains(*id))
            .cloned()
            .collect::<Vec<_>>();

        for id in missing_ids {
            let should_detach = if let Some(entry) = self.entries.get_mut(&id) {
                entry.missing_scans += 1;
                entry.missing_scans > self.config.detach_grace_scans
            } else {
                false
            };

            if should_detach {
                if let Some(entry) = self.entries.remove(&id) {
                    events.push(DeviceEvent::Detached(entry.info.id));
                }
            }
        }

        events
    }

    fn controller_id_for(&mut self, raw_id: &RawDeviceId) -> ControllerId {
        if let Some(controller_id) = self.assigned_controller_ids.get(raw_id) {
            return controller_id.clone();
        }

        let controller_id =
            ControllerId::new(format!("controller-{:04}", self.next_controller_index));
        self.next_controller_index += 1;
        self.assigned_controller_ids
            .insert(raw_id.clone(), controller_id.clone());
        controller_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{BatteryInfo, BatteryState, DeviceFamily, DeviceTransportKind};

    fn supported_device(path: &str) -> RawHidDevice {
        RawHidDevice::mock(path)
            .with_family_hint(DeviceFamily::DualSense)
            .with_transport_hint(DeviceTransportKind::Usb)
    }

    #[test]
    fn reconcile_attaches_supported_devices() {
        let mut registry = DeviceRegistry::new();
        let events = registry.reconcile(vec![supported_device("mock://pad-a")]);

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DeviceEvent::Attached(_)));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn reconcile_uses_detach_grace_period() {
        let mut registry = DeviceRegistry::with_config(RegistryConfig {
            detach_grace_scans: 1,
        });
        registry.reconcile(vec![supported_device("mock://pad-a")]);

        let first_missing = registry.reconcile(Vec::new());
        assert!(first_missing.is_empty());
        assert_eq!(registry.len(), 1);

        let second_missing = registry.reconcile(Vec::new());
        assert_eq!(registry.len(), 0);
        assert!(matches!(
            second_missing.as_slice(),
            [DeviceEvent::Detached(_)]
        ));
    }

    #[test]
    fn reconnect_reuses_controller_id_for_same_raw_device() {
        let mut registry = DeviceRegistry::with_config(RegistryConfig {
            detach_grace_scans: 0,
        });
        let attach = registry.reconcile(vec![supported_device("mock://pad-a")]);
        let first_id = match &attach[0] {
            DeviceEvent::Attached(info) => info.id.clone(),
            event => panic!("expected attach event, got {event:?}"),
        };

        registry.reconcile(Vec::new());
        let reattach = registry.reconcile(vec![supported_device("mock://pad-a")]);
        let second_id = match &reattach[0] {
            DeviceEvent::Attached(info) => info.id.clone(),
            event => panic!("expected attach event, got {event:?}"),
        };

        assert_eq!(first_id, second_id);
    }

    #[test]
    fn reconcile_emits_status_changes() {
        let mut registry = DeviceRegistry::new();
        registry.reconcile(vec![supported_device("mock://pad-a")]);

        let events = registry.reconcile(vec![supported_device("mock://pad-a")
            .with_battery(BatteryInfo::new(Some(77), BatteryState::Discharging))]);

        assert!(matches!(
            events.as_slice(),
            [DeviceEvent::StatusChanged(state)] if state.battery.percent == Some(77)
        ));
    }

    #[test]
    fn permission_denied_device_does_not_attach() {
        let mut registry = DeviceRegistry::new();
        let events = registry.reconcile(vec![
            supported_device("mock://pad-a").with_access(DeviceAccess::PermissionDenied)
        ]);

        assert!(matches!(
            events.as_slice(),
            [DeviceEvent::PermissionDenied(_)]
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn unknown_devices_are_listed_elsewhere_but_not_attached() {
        let mut registry = DeviceRegistry::new();
        let events = registry.reconcile(vec![RawHidDevice::mock("mock://keyboard")]);

        assert!(events.is_empty());
        assert!(registry.is_empty());
    }
}
