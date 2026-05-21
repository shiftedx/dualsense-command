use dscc_device::OutputMode;

pub(crate) fn flag_enabled(name: &str) -> bool {
    flag(name).unwrap_or(false)
}

pub(crate) fn configured_output_mode() -> OutputMode {
    if flag("DSCC_DISABLE_HARDWARE_OUTPUT").unwrap_or(false) {
        OutputMode::DryRunHid
    } else if let Some(enabled) = flag("DSCC_ENABLE_HARDWARE_OUTPUT") {
        if enabled {
            OutputMode::HardwareOutput
        } else {
            OutputMode::DryRunHid
        }
    } else {
        OutputMode::HardwareOutput
    }
}

fn flag(name: &str) -> Option<bool> {
    std::env::var(name).ok().map(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
    })
}
