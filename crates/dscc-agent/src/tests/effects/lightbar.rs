use super::*;

#[test]
fn forza_redline_blink_phase_toggles_predictably() {
    assert!(forza_redline_blink_on(0));
    assert!(forza_redline_blink_on(
        FORZA_REDLINE_BLINK_HALF_PERIOD_MS - 1
    ));
    assert!(!forza_redline_blink_on(FORZA_REDLINE_BLINK_HALF_PERIOD_MS));
    assert!(forza_redline_blink_on(
        FORZA_REDLINE_BLINK_HALF_PERIOD_MS * 2
    ));
}

#[test]
fn forza_redline_blink_replaces_continuous_rpm_lightbar() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    config.lightbar.color = "#0044ff".to_string();
    config.lightbar.rpm_color = "#ffcc00".to_string();
    config.lightbar.brightness = 50;

    let idle = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.0)]);
    let just_below = SignalSnapshot::from_updates([signal_update(
        "vehicle.rpm_ratio",
        FORZA_REV_LIMIT_RATIO - 0.01,
    )]);
    let redline = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 1.0)]);

    let idle_output =
        forza_redline_light_output(Some(&config), &idle, 1.0, FORZA_REV_LIMIT_RATIO, true);
    let below_output =
        forza_redline_light_output(Some(&config), &just_below, 1.0, FORZA_REV_LIMIT_RATIO, true);
    let redline_on =
        forza_redline_light_output(Some(&config), &redline, 1.0, FORZA_REV_LIMIT_RATIO, true);
    let redline_off =
        forza_redline_light_output(Some(&config), &redline, 1.0, FORZA_REV_LIMIT_RATIO, false);
    let disabled_redline =
        forza_redline_light_output(Some(&config), &redline, 0.0, FORZA_REV_LIMIT_RATIO, true);

    assert_eq!(
        idle_output.lightbar.color,
        RgbColor {
            red: 0,
            green: 68,
            blue: 255,
        }
    );
    assert_eq!(below_output.lightbar.color, idle_output.lightbar.color);
    assert_eq!(
        below_output.player_leds,
        Some(PlayerLedsOutput { count: 0 })
    );
    assert_eq!(
        redline_on.lightbar.color,
        RgbColor {
            red: 255,
            green: 204,
            blue: 0,
        }
    );
    assert_eq!(
        redline_on.player_leds,
        Some(PlayerLedsOutput {
            count: FORZA_REDLINE_PLAYER_LED_COUNT
        })
    );
    assert_eq!(redline_off.lightbar.color, idle_output.lightbar.color);
    assert_eq!(redline_off.player_leds, Some(PlayerLedsOutput { count: 0 }));
    assert_eq!(disabled_redline.lightbar.color, idle_output.lightbar.color);
    assert_eq!(
        disabled_redline.player_leds,
        Some(PlayerLedsOutput { count: 0 })
    );
    assert_eq!(
        redline_on.lightbar.brightness,
        idle_output.lightbar.brightness
    );
}
