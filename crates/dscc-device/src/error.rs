use std::{error::Error, fmt};

use crate::status::{DevicePathHint, RawDeviceId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceError {
    BackendUnavailable(String),
    DeviceNotFound(RawDeviceId),
    PermissionDenied(DevicePathHint),
    AccessBlocked {
        path_hint: DevicePathHint,
        reason: String,
    },
    TransportFault(String),
    ShutdownRequested,
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable(message) => write!(f, "device backend unavailable: {message}"),
            Self::DeviceNotFound(id) => write!(f, "device not found: {id}"),
            Self::PermissionDenied(path_hint) => {
                write!(f, "permission denied opening device at {path_hint}")
            }
            Self::AccessBlocked { path_hint, reason } => {
                write!(f, "device access blocked at {path_hint}: {reason}")
            }
            Self::TransportFault(message) => write!(f, "device transport fault: {message}"),
            Self::ShutdownRequested => f.write_str("device loop shutdown requested"),
        }
    }
}

impl Error for DeviceError {}
