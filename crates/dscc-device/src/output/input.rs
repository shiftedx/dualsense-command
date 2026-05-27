use std::time::Duration;

const INPUT_USB_COMMON_OFFSET: usize = 1;
const INPUT_BT_COMMON_OFFSET: usize = 2;
const INPUT_COMMON_LEFT_STICK_X: usize = 0;
const INPUT_COMMON_LEFT_STICK_Y: usize = 1;
const INPUT_COMMON_RIGHT_STICK_X: usize = 2;
const INPUT_COMMON_RIGHT_STICK_Y: usize = 3;
const INPUT_COMMON_L2: usize = 4;
const INPUT_COMMON_R2: usize = 5;
const INPUT_COMMON_BUTTON0: usize = 7;
const INPUT_COMMON_BUTTON1: usize = 8;
const INPUT_COMMON_BUTTON2: usize = 9;
const INPUT_READ_ATTEMPTS: usize = 16;
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerInputState {
    pub left_stick: ControllerInputStickState,
    pub right_stick: ControllerInputStickState,
    pub l2: f64,
    pub r2: f64,
    pub buttons: Vec<ControllerInputButtonState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerInputReadOptions {
    pub attempts: usize,
    pub first_timeout: Duration,
    pub subsequent_timeout: Duration,
}

impl Default for ControllerInputReadOptions {
    fn default() -> Self {
        Self {
            attempts: INPUT_READ_ATTEMPTS,
            first_timeout: Duration::from_millis(3),
            subsequent_timeout: Duration::ZERO,
        }
    }
}

impl ControllerInputReadOptions {
    pub fn bridge_poll() -> Self {
        Self {
            attempts: 1,
            first_timeout: Duration::ZERO,
            subsequent_timeout: Duration::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerInputStickState {
    pub x: f64,
    pub y: f64,
    pub magnitude: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerInputButtonState {
    pub id: &'static str,
    pub label: &'static str,
    pub pressed: bool,
    pub value: f64,
}

pub(super) fn parse_dualsense_input_state(report: &[u8]) -> Option<ControllerInputState> {
    let common_offset = match report.first().copied()? {
        0x01 if report.len() > INPUT_USB_COMMON_OFFSET + INPUT_COMMON_BUTTON2 => {
            INPUT_USB_COMMON_OFFSET
        }
        0x31 if report.len() > INPUT_BT_COMMON_OFFSET + INPUT_COMMON_BUTTON2 => {
            INPUT_BT_COMMON_OFFSET
        }
        _ => return None,
    };

    let l2 = unit_axis(report[common_offset + INPUT_COMMON_L2]);
    let r2 = unit_axis(report[common_offset + INPUT_COMMON_R2]);
    let button0 = report[common_offset + INPUT_COMMON_BUTTON0];
    let button1 = report[common_offset + INPUT_COMMON_BUTTON1];
    let button2 = report[common_offset + INPUT_COMMON_BUTTON2];

    Some(ControllerInputState {
        left_stick: stick_state(
            report[common_offset + INPUT_COMMON_LEFT_STICK_X],
            report[common_offset + INPUT_COMMON_LEFT_STICK_Y],
        ),
        right_stick: stick_state(
            report[common_offset + INPUT_COMMON_RIGHT_STICK_X],
            report[common_offset + INPUT_COMMON_RIGHT_STICK_Y],
        ),
        l2,
        r2,
        buttons: standard_button_states(button0, button1, button2, l2, r2),
    })
}

fn stick_state(x: u8, y: u8) -> ControllerInputStickState {
    let x = signed_axis(x);
    let y = signed_axis(y);
    ControllerInputStickState {
        x,
        y,
        magnitude: (x.hypot(y)).min(1.0),
    }
}

fn signed_axis(value: u8) -> f64 {
    let centered = f64::from(value) - 128.0;
    if centered >= 0.0 {
        (centered / 127.0).min(1.0)
    } else {
        (centered / 128.0).max(-1.0)
    }
}

fn unit_axis(value: u8) -> f64 {
    f64::from(value) / 255.0
}

fn standard_button_states(
    button0: u8,
    button1: u8,
    button2: u8,
    l2: f64,
    r2: f64,
) -> Vec<ControllerInputButtonState> {
    let dpad = button0 & 0x0f;
    let mut buttons = Vec::with_capacity(23);
    push_button(
        &mut buttons,
        "dpad_up",
        "D-Pad Up",
        dpad_matches(dpad, &[0, 1, 7]),
    );
    push_button(
        &mut buttons,
        "dpad_right",
        "D-Pad Right",
        dpad_matches(dpad, &[1, 2, 3]),
    );
    push_button(
        &mut buttons,
        "dpad_down",
        "D-Pad Down",
        dpad_matches(dpad, &[3, 4, 5]),
    );
    push_button(
        &mut buttons,
        "dpad_left",
        "D-Pad Left",
        dpad_matches(dpad, &[5, 6, 7]),
    );
    push_button(&mut buttons, "square", "Square", button0 & 0x10 != 0);
    push_button(&mut buttons, "cross", "Cross", button0 & 0x20 != 0);
    push_button(&mut buttons, "circle", "Circle", button0 & 0x40 != 0);
    push_button(&mut buttons, "triangle", "Triangle", button0 & 0x80 != 0);
    push_button(&mut buttons, "l1", "L1", button1 & 0x01 != 0);
    push_button(&mut buttons, "r1", "R1", button1 & 0x02 != 0);
    buttons.push(ControllerInputButtonState {
        id: "l2",
        label: "L2",
        pressed: button1 & 0x04 != 0,
        value: l2,
    });
    buttons.push(ControllerInputButtonState {
        id: "r2",
        label: "R2",
        pressed: button1 & 0x08 != 0,
        value: r2,
    });
    push_button(&mut buttons, "create", "Create", button1 & 0x10 != 0);
    push_button(&mut buttons, "options", "Options", button1 & 0x20 != 0);
    push_button(&mut buttons, "l3", "L3", button1 & 0x40 != 0);
    push_button(&mut buttons, "r3", "R3", button1 & 0x80 != 0);
    push_button(&mut buttons, "ps", "PS", button2 & 0x01 != 0);
    push_button(&mut buttons, "touchpad", "Touchpad", button2 & 0x02 != 0);
    push_button(&mut buttons, "mute", "Mute", button2 & 0x04 != 0);
    push_button(&mut buttons, "edge_fn_left", "Fn Left", button2 & 0x10 != 0);
    push_button(
        &mut buttons,
        "edge_fn_right",
        "Fn Right",
        button2 & 0x20 != 0,
    );
    push_button(
        &mut buttons,
        "edge_back_left",
        "Back Left",
        button2 & 0x40 != 0,
    );
    push_button(
        &mut buttons,
        "edge_back_right",
        "Back Right",
        button2 & 0x80 != 0,
    );
    buttons
}

fn push_button(
    buttons: &mut Vec<ControllerInputButtonState>,
    id: &'static str,
    label: &'static str,
    pressed: bool,
) {
    buttons.push(ControllerInputButtonState {
        id,
        label,
        pressed,
        value: if pressed { 1.0 } else { 0.0 },
    });
}

fn dpad_matches(dpad: u8, active: &[u8]) -> bool {
    active.contains(&dpad)
}
