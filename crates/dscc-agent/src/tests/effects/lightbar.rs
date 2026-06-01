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
fn forza_redline_lightbar_ramps_then_blinks_at_rev_limiter_threshold() {
    let mut config = ControllerConfig::default_for("edge-forza", "DualSense Edge");
    config.lightbar.color = "#0044ff".to_string();
    config.lightbar.rpm_color = "#ff0000".to_string();
    config.lightbar.brightness = 50;

    let idle = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.0)]);
    let ramping = SignalSnapshot::from_updates([signal_update(
        "vehicle.rpm_ratio",
        FORZA_REV_LIMIT_RATIO - (FORZA_REDLINE_FADE_WIDTH / 2.0),
    )]);
    let at_threshold =
        SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", FORZA_REV_LIMIT_RATIO)]);
    let custom_threshold = SignalSnapshot::from_updates([signal_update("vehicle.rpm_ratio", 0.86)]);

    let idle_output =
        forza_redline_light_output(Some(&config), &idle, 1.0, FORZA_REV_LIMIT_RATIO, true);
    let ramping_output =
        forza_redline_light_output(Some(&config), &ramping, 1.0, FORZA_REV_LIMIT_RATIO, true);
    let redline_on = forza_redline_light_output(
        Some(&config),
        &at_threshold,
        1.0,
        FORZA_REV_LIMIT_RATIO,
        true,
    );
    let redline_off = forza_redline_light_output(
        Some(&config),
        &at_threshold,
        1.0,
        FORZA_REV_LIMIT_RATIO,
        false,
    );
    let disabled_redline = forza_redline_light_output(
        Some(&config),
        &at_threshold,
        0.0,
        FORZA_REV_LIMIT_RATIO,
        true,
    );
    let custom_threshold_on =
        forza_redline_light_output(Some(&config), &custom_threshold, 1.0, 0.86, true);

    assert_eq!(
        idle_output.lightbar.color,
        RgbColor {
            red: 0,
            green: 68,
            blue: 255,
        }
    );
    assert!(
        ramping_output.lightbar.color.red > idle_output.lightbar.color.red,
        "pre-redline ramp should move toward red"
    );
    assert!(
        ramping_output.lightbar.color.blue < idle_output.lightbar.color.blue,
        "pre-redline ramp should move away from the base blue"
    );
    assert_eq!(
        ramping_output.player_leds,
        Some(PlayerLedsOutput { count: 0 })
    );
    assert_eq!(
        redline_on.lightbar.color,
        RgbColor {
            red: 255,
            green: 0,
            blue: 0,
        }
    );
    assert_eq!(
        redline_on.player_leds,
        Some(PlayerLedsOutput {
            count: FORZA_REDLINE_PLAYER_LED_COUNT
        })
    );
    assert_ne!(redline_off.lightbar.color, idle_output.lightbar.color);
    assert!(redline_off.lightbar.color.red > idle_output.lightbar.color.red);
    assert_eq!(redline_off.player_leds, Some(PlayerLedsOutput { count: 0 }));
    assert_eq!(disabled_redline.lightbar.color, idle_output.lightbar.color);
    assert_eq!(
        disabled_redline.player_leds,
        Some(PlayerLedsOutput { count: 0 })
    );
    assert_eq!(
        custom_threshold_on.player_leds,
        Some(PlayerLedsOutput {
            count: FORZA_REDLINE_PLAYER_LED_COUNT
        }),
        "LED blink should trigger at the same custom threshold as rev limiter buzz"
    );
    assert_eq!(
        redline_on.lightbar.brightness,
        idle_output.lightbar.brightness
    );
}
