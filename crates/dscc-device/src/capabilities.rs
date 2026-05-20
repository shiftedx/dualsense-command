use crate::status::{ControllerCapabilities, DeviceFamily};

pub fn infer_capabilities(family: DeviceFamily) -> ControllerCapabilities {
    match family {
        DeviceFamily::DualSense => dualsense(false),
        DeviceFamily::DualSenseEdge => dualsense(true),
        DeviceFamily::UnknownSony | DeviceFamily::Unknown => ControllerCapabilities {
            adaptive_triggers: false,
            lightbar: false,
            player_leds: false,
            rumble: false,
            microphone_led: false,
            edge_buttons: false,
        },
    }
}

fn dualsense(edge_buttons: bool) -> ControllerCapabilities {
    ControllerCapabilities {
        adaptive_triggers: true,
        lightbar: true,
        player_leds: true,
        rumble: true,
        microphone_led: true,
        edge_buttons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_capabilities_include_edge_controls() {
        let capabilities = infer_capabilities(DeviceFamily::DualSenseEdge);

        assert!(capabilities.adaptive_triggers);
        assert!(capabilities.edge_buttons);
    }

    #[test]
    fn unknown_capabilities_do_not_claim_support() {
        let capabilities = infer_capabilities(DeviceFamily::UnknownSony);

        assert!(!capabilities.adaptive_triggers);
        assert!(!capabilities.rumble);
    }
}
