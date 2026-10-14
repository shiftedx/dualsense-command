use crate::status::{ControllerId, ControllerInfo, ControllerState, DevicePathHint};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceEvent {
    Attached(ControllerInfo),
    Detached(ControllerId),
    StatusChanged(ControllerState),
    PermissionDenied(DevicePathHint),
    Faulted {
        id: Option<ControllerId>,
        message: String,
    },
}
