use crate::{error::DeviceError, status::DevicePathHint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticCode {
    BackendUnavailable,
    PermissionDenied,
    AccessBlocked,
    TransportFault,
    DeviceNotFound,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub message: String,
    pub path_hint: Option<DevicePathHint>,
}

impl DeviceDiagnostic {
    pub fn backend_unavailable(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: DiagnosticCode::BackendUnavailable,
            message: message.into(),
            path_hint: None,
        }
    }

    pub fn permission_denied(path_hint: DevicePathHint) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: DiagnosticCode::PermissionDenied,
            message: "controller is visible, but the process cannot open it".to_string(),
            path_hint: Some(path_hint),
        }
    }

    pub fn access_blocked(path_hint: DevicePathHint, reason: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: DiagnosticCode::AccessBlocked,
            message: reason.into(),
            path_hint: Some(path_hint),
        }
    }
}

pub fn diagnostic_for_error(error: &DeviceError) -> DeviceDiagnostic {
    match error {
        DeviceError::BackendUnavailable(message) => {
            DeviceDiagnostic::backend_unavailable(message.clone())
        }
        DeviceError::PermissionDenied(path_hint) => {
            DeviceDiagnostic::permission_denied(path_hint.clone())
        }
        DeviceError::AccessBlocked { path_hint, reason } => {
            DeviceDiagnostic::access_blocked(path_hint.clone(), reason.clone())
        }
        DeviceError::DeviceNotFound(id) => DeviceDiagnostic {
            severity: DiagnosticSeverity::Info,
            code: DiagnosticCode::DeviceNotFound,
            message: format!("device {id} is no longer present"),
            path_hint: None,
        },
        DeviceError::TransportFault(message) => DeviceDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: DiagnosticCode::TransportFault,
            message: message.clone(),
            path_hint: None,
        },
        DeviceError::ShutdownRequested => DeviceDiagnostic {
            severity: DiagnosticSeverity::Info,
            code: DiagnosticCode::Shutdown,
            message: "device loop is shutting down".to_string(),
            path_hint: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_error_maps_to_sanitized_diagnostic() {
        let path_hint = DevicePathHint::from_backend_path("/dev/hidraw9/private");
        let diagnostic = diagnostic_for_error(&DeviceError::PermissionDenied(path_hint.clone()));

        assert_eq!(diagnostic.code, DiagnosticCode::PermissionDenied);
        assert_eq!(diagnostic.path_hint, Some(path_hint));
        assert!(!diagnostic.message.contains("/dev/hidraw9"));
    }
}
