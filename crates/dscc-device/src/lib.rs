//! Hardware boundary for DualSense Command Center.
//!
//! This crate owns sanitized HID discovery, optional `hidapi` transport,
//! controller lifecycle events, registry reconciliation, diagnostics, and the
//! guarded output encoding path for controller effects.

#![forbid(unsafe_code)]

pub mod capabilities;
pub mod diagnostics;
pub mod edge_profile;
pub mod enumeration;
pub mod error;
pub mod events;
#[cfg(feature = "hidapi-backend")]
pub mod hidapi_transport;
pub mod manager;
pub mod metadata;
pub mod output;
pub mod registry;
pub mod session;
pub mod status;
pub mod transport;

pub use capabilities::infer_capabilities;
pub use diagnostics::{diagnostic_for_error, DeviceDiagnostic, DiagnosticCode, DiagnosticSeverity};
pub use edge_profile::{
    decode_edge_onboard_profile, default_button_mappings, edge_onboard_transport_supported,
    encode_edge_onboard_profile, read_edge_onboard_profiles, write_edge_onboard_profile,
    EdgeButton, EdgeButtonMapping, EdgeOnboardProfile, EdgeOnboardSlotId, EdgeProfileIntensity,
    EdgeStickPreset, EdgeStickProfile, EdgeTriggerDeadzone,
};
pub use enumeration::{
    list_sanitized_hid, list_sanitized_hid_with_access_probe, DeviceAccess, RawHidDevice,
    SanitizedHidDevice,
};
pub use error::DeviceError;
pub use events::DeviceEvent;
#[cfg(feature = "hidapi-backend")]
pub use hidapi_transport::HidApiTransport;
pub use manager::{DeviceConfig, DeviceManager, OutputMode};
pub use metadata::{
    infer_family, DUALSENSE_EDGE_PRODUCT_ID, SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID,
};
pub use output::{
    encode_controller_output_frame, ControllerInputState, ControllerOutputManager,
    ControllerOutputTarget, ControllerOutputWrite, EncodedOutputReport, OutputReportKind,
};
pub use registry::{DeviceRegistry, RegistryConfig, RegistryEntry};
pub use session::DeviceSession;
pub use status::{
    BatteryInfo, BatteryState, ConnectionState, ControllerCapabilities, ControllerId,
    ControllerInfo, ControllerState, DeviceFamily, DevicePathHint, DeviceTransportKind,
    RawDeviceId,
};
pub use transport::{DeviceHandle, DeviceTransport, MockTransport};
