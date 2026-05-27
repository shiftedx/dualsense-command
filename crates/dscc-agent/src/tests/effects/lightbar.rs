use super::*;

#[test]
fn forza_player_leds_follow_current_gear() {
    let third_gear = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 3.0)]);
    assert_eq!(forza_gear_player_led_count(&third_gear), 3);

    let sixth_gear = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 6.0)]);
    assert_eq!(forza_gear_player_led_count(&sixth_gear), 5);

    let neutral = SignalSnapshot::from_updates([signal_update("drivetrain.gear", 0.0)]);
    assert_eq!(forza_gear_player_led_count(&neutral), 0);
}

#[test]
fn forza_lightbar_blends_profile_color_toward_redline_with_rpm() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    config.lightbar.color = "#0044ff".to_string();
    config.lightbar.rpm_color = "#ffcc00".to_string();
    config.lightbar.brightness = 50;

    let idle = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.0)]);
    let mid = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.5)]);
    let redline = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 1.0)]);

    let idle_lightbar = forza_lightbar_output(Some(&config), &idle, 1.0);
    let mid_lightbar = forza_lightbar_output(Some(&config), &mid, 1.0);
    let redline_lightbar = forza_lightbar_output(Some(&config), &redline, 1.0);
    let disabled_rpm_leds = forza_lightbar_output(Some(&config), &redline, 0.0);

    assert_eq!(
        idle_lightbar.color,
        RgbColor {
            red: 0,
            green: 68,
            blue: 255,
        }
    );
    assert!(
        mid_lightbar.color.red > idle_lightbar.color.red,
        "mid-rpm lightbar should move toward red"
    );
    assert!(
        mid_lightbar.color.blue < idle_lightbar.color.blue,
        "mid-rpm lightbar should reduce blue while moving toward red"
    );
    assert_eq!(
        redline_lightbar.color,
        RgbColor {
            red: 255,
            green: 204,
            blue: 0,
        }
    );
    assert!(redline_lightbar.brightness > idle_lightbar.brightness);
    assert_eq!(disabled_rpm_leds.color, idle_lightbar.color);
}
