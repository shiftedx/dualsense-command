use super::*;
use crate::input_bridge::virtual_state_from_input;
use dscc_device::{ControllerInputButtonState, ControllerInputStickState};
use std::{hint::black_box, time::Instant};

fn assert_under(label: &str, elapsed_ms: u128, budget_ms: u128) {
    assert!(
        elapsed_ms <= budget_ms,
        "{label} took {elapsed_ms}ms, over the {budget_ms}ms deterministic perf guard"
    );
}

fn perf_input() -> ControllerInputState {
    ControllerInputState {
        left_stick: ControllerInputStickState {
            x: 0.42,
            y: -0.25,
            magnitude: 0.49,
        },
        right_stick: ControllerInputStickState {
            x: -0.18,
            y: 0.75,
            magnitude: 0.77,
        },
        l2: 0.36,
        r2: 0.82,
        buttons: vec![
            ControllerInputButtonState {
                id: "cross",
                label: "Cross",
                pressed: true,
                value: 1.0,
            },
            ControllerInputButtonState {
                id: "circle",
                label: "Circle",
                pressed: false,
                value: 0.0,
            },
            ControllerInputButtonState {
                id: "edge_back_left",
                label: "Back Left",
                pressed: true,
                value: 1.0,
            },
            ControllerInputButtonState {
                id: "edge_back_right",
                label: "Back Right",
                pressed: false,
                value: 0.0,
            },
        ],
    }
}

#[test]
fn input_bridge_mapping_perf_guard() {
    let input = perf_input();
    let config = InputBridgeConfig::default();
    let start = Instant::now();
    for _ in 0..10_000 {
        black_box(virtual_state_from_input(
            black_box(&input),
            black_box(&config),
        ));
    }
    assert_under("input bridge mapping", start.elapsed().as_millis(), 500);
}

#[test]
fn steam_input_parser_perf_guard() {
    let root = FsPath::new("C:/Program Files (x86)/Steam");
    let file = root.join("userdata/123456/1551360/remote/test_controller_config.vdf");
    let source = r##""controller_mappings"
{
"title" "Perf Layout"
"controller_type" "controller_ps5_edge"
"group"
{
    "ID" "1"
    "mode" "button_diamond"
    "inputs"
    {
        "button_a" { "activators" { "Full Press" { "bindings" { "binding" "xinput_button A, , Cross" } } } }
        "button_b" { "activators" { "Full Press" { "bindings" { "binding" "xinput_button B, , Circle" } } } }
        "button_x" { "activators" { "Full Press" { "bindings" { "binding" "xinput_button X, , Square" } } } }
        "button_y" { "activators" { "Full Press" { "bindings" { "binding" "xinput_button Y, , Triangle" } } } }
    }
}
"group"
{
    "ID" "2"
    "mode" "dpad"
    "inputs"
    {
        "dpad_north" { "activators" { "Full Press" { "bindings" { "binding" "key_press UP, , Up" } } } }
        "dpad_south" { "activators" { "Full Press" { "bindings" { "binding" "key_press DOWN, , Down" } } } }
        "dpad_west" { "activators" { "Full Press" { "bindings" { "binding" "key_press LEFT, , Left" } } } }
        "dpad_east" { "activators" { "Full Press" { "bindings" { "binding" "key_press RIGHT, , Right" } } } }
    }
}
}"##;
    let start = Instant::now();
    for _ in 0..1_000 {
        black_box(parse_steam_input_layout(
            black_box(root),
            black_box(&file),
            black_box(source),
        ));
    }
    assert_under("steam input parser", start.elapsed().as_millis(), 800);
}

#[test]
fn output_frame_assembly_perf_guard() {
    let request = EffectTestRequest {
        target: Some("base_feel".to_string()),
        mode: Some("adaptive_resistance".to_string()),
        intensity: Some(100),
        start_position: Some(0.18),
        l2_position: Some(0.62),
        r2_position: Some(0.84),
        duration_ms: Some(900),
        trigger: Some(TriggerConfig::default()),
    };
    let start = Instant::now();
    for _ in 0..20_000 {
        black_box(effect_test_output_frame(black_box(&request)));
    }
    assert_under("output frame assembly", start.elapsed().as_millis(), 600);
}

#[test]
fn telemetry_materialization_perf_guard() {
    let snapshot = SignalSnapshot::from_updates([
        signal_update("vehicle.speed_kmh", 142.0),
        signal_update("vehicle.rpm_ratio", 0.86),
        signal_update("input.brake", 0.35),
        signal_update("input.throttle", 0.78),
        signal_update("surface.rumble.max", 0.42),
        signal_update("surface.rumble_strip.max", 0.18),
        signal_update("wheel.slip.max", 0.28),
        signal_update("drivetrain.shift_pulse", 1.0),
    ]);
    let config = ControllerConfig::default_for("perf-controller", "DualSense Edge").normalized();
    let start = Instant::now();
    for _ in 0..10_000 {
        black_box(forza_rumble_output(
            black_box(&config.forza),
            black_box(&snapshot),
            black_box(1.0),
            black_box(&config.trigger.vibration_mode),
        ));
        black_box(forza_lightbar_output(black_box(Some(&config))));
        black_box(forza_redline_light_output(
            black_box(Some(&config)),
            black_box(&snapshot),
            black_box(1.0),
            black_box(
                config
                    .forza
                    .rev_limiter
                    .clone()
                    .normalized()
                    .threshold_ratio,
            ),
            black_box(true),
        ));
    }
    assert_under(
        "telemetry output materialization",
        start.elapsed().as_millis(),
        900,
    );
}
