use std::time::Duration;

use crate::game_modules::FORZA_HORIZON_IMMERSIVE_PROFILE_ID;

pub(crate) const GLOBAL_PROFILE_ID: &str = "global";
pub(crate) const DEFAULT_PROFILE_ID: &str = GLOBAL_PROFILE_ID;
pub(crate) const IMMERSIVE_PROFILE_ID: &str = FORZA_HORIZON_IMMERSIVE_PROFILE_ID;

pub(crate) fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub(crate) fn current_timestamp_millis() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

pub(crate) const HARDWARE_OUTPUT_INTERVAL: Duration = Duration::from_millis(33);
pub(crate) const INPUT_BRIDGE_PROCESS_INTERVAL: Duration = Duration::from_millis(8);
pub(crate) const INPUT_BRIDGE_CONFIG_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const INPUT_BRIDGE_STALE_AFTER: Duration = Duration::from_millis(250);
pub(crate) const CONTROLLER_INPUT_UI_CACHE_TTL: Duration = Duration::from_millis(75);
pub(crate) const HARDWARE_OUTPUT_KEEPALIVE_INTERVAL: Duration = Duration::from_millis(750);
pub(crate) const MANUAL_OUTPUT_REFRESH_INTERVAL: Duration = Duration::from_millis(250);
pub(crate) const BASE_FEEL_OUTPUT_REFRESH_INTERVAL: Duration = Duration::from_millis(33);
pub(crate) const HARDWARE_GAME_DETECTION_INTERVAL: Duration = Duration::from_millis(500);
pub(crate) const DEFAULT_EFFECT_TEST_DURATION_MS: u64 = 650;
pub(crate) const MAX_EFFECT_TEST_DURATION_MS: u64 = 1_500;
pub(crate) const DEFAULT_BASE_FEEL_TEST_DURATION_MS: u64 = 30_000;
pub(crate) const MAX_BASE_FEEL_TEST_DURATION_MS: u64 = 60_000;
pub(crate) const UDP_TELEMETRY_PROCESS_INTERVAL: Duration = Duration::from_millis(33);
#[cfg(target_os = "windows")]
pub(crate) const SHARED_MEMORY_TELEMETRY_PROCESS_INTERVAL: Duration = Duration::from_millis(33);
pub(crate) const FORZA_SHIFT_EVENT_HOLD: Duration = Duration::from_millis(190);
pub(crate) const FORZA_SUSPENSION_IMPACT_HOLD: Duration = Duration::from_millis(170);
pub(crate) const GAME_DETECTION_CACHE_TTL: Duration = Duration::from_secs(5);
pub(crate) const STEAM_INPUT_CACHE_TTL: Duration = Duration::from_secs(30);
pub(crate) const STEAM_GAME_CATALOG_CACHE_TTL: Duration = Duration::from_secs(300);
pub(crate) const UPDATE_CHECK_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
pub(crate) const TELEMETRY_WS_INVALIDATION_INTERVAL: Duration = Duration::from_millis(500);
pub(crate) const FORZA_BRAKE_FULL_FORCE_AT: f64 = 246.0 / 255.0;
pub(crate) const FORZA_THROTTLE_FULL_FORCE_AT: f64 = 252.0 / 255.0;
pub(crate) const FORZA_BRAKE_BASELINE_FORCE: f64 = 42.0 / 255.0;
pub(crate) const FORZA_BRAKE_NORMAL_FORCE: f64 = 164.0 / 255.0;
pub(crate) const FORZA_BRAKE_ENDSTOP_FORCE: f64 = 238.0 / 255.0;
pub(crate) const FORZA_THROTTLE_BASELINE_FORCE: f64 = 3.0 / 255.0;
pub(crate) const FORZA_THROTTLE_NORMAL_FORCE: f64 = 28.0 / 255.0;
pub(crate) const FORZA_THROTTLE_ENDSTOP_FORCE: f64 = 106.0 / 255.0;
pub(crate) const FORZA_HANDBRAKE_FORCE: f64 = 25.0 / 255.0;
pub(crate) const FORZA_ABS_RANGE_START_RATIO: f64 = 0.30;
pub(crate) const FORZA_ABS_MIN_SPEED_KMH: f64 = 15.0;
pub(crate) const FORZA_ABS_SLIP_THRESHOLD: f64 = 1.0;
pub(crate) const FORZA_ABS_PULSE_AMPLITUDE: f64 = 20.0 / 63.0;
pub(crate) const FORZA_ABS_PULSE_FREQUENCY_HZ: f64 = 10.0;
pub(crate) const FORZA_BRAKE_CURVE: f64 = 1.35;
pub(crate) const FORZA_THROTTLE_CURVE: f64 = 2.25;
pub(crate) const FORZA_ENDSTOP_WALL_OFFSET: f64 = 0.03;
pub(crate) const FORZA_BRAKE_OVERTRAVEL_WARNING_OFFSET: f64 = 0.28;
pub(crate) const FORZA_BRAKE_OVERTRAVEL_WARNING_MIN_POSITION: f64 = 0.70;
pub(crate) const FORZA_BRAKE_OVERTRAVEL_RAMP_WIDTH: f64 = 0.16;
pub(crate) const FORZA_BRAKE_OVERTRAVEL_RAMP_CURVE: f64 = 2.0;
pub(crate) const FORZA_THROTTLE_OVERTRAVEL_WALL_POSITION: f64 = 0.80;
pub(crate) const FORZA_THROTTLE_OVERTRAVEL_MIN_POSITION: f64 = 0.80;
pub(crate) const FORZA_BRAKE_ENDSTOP_FORCE_BOOST: f64 = 1.25;
pub(crate) const FORZA_THROTTLE_ENDSTOP_FORCE_BOOST: f64 = 3.0;
pub(crate) const FORZA_THROTTLE_OVERTRAVEL_RAMP_WIDTH: f64 = 0.20;
pub(crate) const FORZA_THROTTLE_OVERTRAVEL_RAMP_CURVE: f64 = 2.4;
pub(crate) const FORZA_SHIFT_THUMP_DEFAULT_INTENSITY: u8 = 255;
pub(crate) const TRIGGER_CURVE_SCALE: f64 = 100.0;
pub(crate) const TRIGGER_CURVE_MIN: u16 = 50;
pub(crate) const TRIGGER_CURVE_MAX: u16 = 350;
pub(crate) const TRIGGER_CURVE_POINT_MIN: usize = 4;
pub(crate) const TRIGGER_CURVE_POINT_MAX: usize = 8;
pub(crate) const FORZA_REV_LIMIT_RATIO: f64 = 0.93;
pub(crate) const FORZA_REV_LIMITER_PULSE_AMPLITUDE: f64 = 18.0 / 63.0;
pub(crate) const FORZA_REV_LIMITER_FREQUENCY_HZ: f64 = 42.0;
pub(crate) const FORZA_REV_LIMITER_WALL_FORM_THROTTLE_AT: f64 = 0.60;
pub(crate) const FORZA_REV_LIMITER_WALL_ZONES: f64 = 4.0;
pub(crate) const FORZA_SHIFT_WALL_FORM_AT: f64 = 0.15;
pub(crate) const FORZA_SHIFT_FREQUENCY_HZ: f64 = 34.0;
pub(crate) const FORZA_SHIFT_WALL_ZONES: f64 = 4.0;
pub(crate) const FORZA_SUSPENSION_IMPACT_TRIGGER_AT: f64 = 0.42;
pub(crate) const FORZA_SUSPENSION_IMPACT_RESET_AT: f64 = 0.22;
