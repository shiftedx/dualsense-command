use crate::status::DeviceFamily;

/// Sony Interactive Entertainment vendor id observed on the Windows
/// DualSense Edge Bluetooth HID nodes recorded in `docs/hardware-validation.md`.
pub const SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID: u16 = 0x054c;

/// DualSense Edge product id observed on the Windows Bluetooth HID nodes
/// recorded in `docs/hardware-validation.md`.
pub const DUALSENSE_EDGE_PRODUCT_ID: u16 = 0x0df2;

pub fn infer_family(
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    manufacturer: Option<&str>,
    product: Option<&str>,
) -> DeviceFamily {
    if vendor_id == Some(SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID)
        && product_id == Some(DUALSENSE_EDGE_PRODUCT_ID)
    {
        return DeviceFamily::DualSenseEdge;
    }

    infer_family_from_strings(manufacturer, product)
        .or_else(|| {
            if vendor_id == Some(SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID) {
                Some(DeviceFamily::UnknownSony)
            } else {
                None
            }
        })
        .unwrap_or(DeviceFamily::Unknown)
}

fn infer_family_from_strings(
    manufacturer: Option<&str>,
    product: Option<&str>,
) -> Option<DeviceFamily> {
    let product = product.unwrap_or_default().to_ascii_lowercase();
    let manufacturer = manufacturer.unwrap_or_default().to_ascii_lowercase();

    if product.contains("dualsense edge") {
        Some(DeviceFamily::DualSenseEdge)
    } else if product.contains("dualsense") {
        Some(DeviceFamily::DualSense)
    } else if manufacturer.contains("sony")
        || manufacturer.contains("playstation")
        || product.contains("sony")
        || product.contains("playstation")
    {
        Some(DeviceFamily::UnknownSony)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_edge_vid_pid_identifies_dualsense_edge_without_product_string() {
        assert_eq!(
            infer_family(
                Some(SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID),
                Some(DUALSENSE_EDGE_PRODUCT_ID),
                None,
                None,
            ),
            DeviceFamily::DualSenseEdge
        );
    }

    #[test]
    fn product_string_still_identifies_standard_dualsense() {
        assert_eq!(
            infer_family(None, None, None, Some("DualSense Wireless Controller")),
            DeviceFamily::DualSense
        );
    }

    #[test]
    fn sony_vendor_without_known_product_stays_unknown_sony() {
        assert_eq!(
            infer_family(
                Some(SONY_INTERACTIVE_ENTERTAINMENT_VENDOR_ID),
                Some(0xffff),
                None,
                None,
            ),
            DeviceFamily::UnknownSony
        );
    }
}
