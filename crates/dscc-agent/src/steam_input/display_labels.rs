pub(super) fn friendly_steam_input(input: &str, source: Option<&str>) -> String {
    match input {
        "button_a" => "Cross".to_string(),
        "button_b" => "Circle".to_string(),
        "button_x" => "Square".to_string(),
        "button_y" => "Triangle".to_string(),
        "dpad_north" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Up".to_string()
        }
        "dpad_south" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Down".to_string()
        }
        "dpad_east" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Right".to_string()
        }
        "dpad_west" if source.is_some_and(|source| source.contains("trackpad")) => {
            "Swipe Left".to_string()
        }
        "dpad_north" => "D-Pad Up".to_string(),
        "dpad_south" => "D-Pad Down".to_string(),
        "dpad_east" => "D-Pad Right".to_string(),
        "dpad_west" => "D-Pad Left".to_string(),
        "button_escape" => "Options".to_string(),
        "button_menu" => "Create".to_string(),
        "button_back_left" => "Back Left".to_string(),
        "button_back_right" => "Back Right".to_string(),
        "button_back_left_upper" => "Fn Left".to_string(),
        "button_back_right_upper" => "Fn Right".to_string(),
        "click" => match source {
            Some("left_trigger") => "L2 Full Pull".to_string(),
            Some("right_trigger") => "R2 Full Pull".to_string(),
            Some("joystick") => "Left Stick Click".to_string(),
            Some("right_joystick") => "Right Stick Click".to_string(),
            Some("left_trackpad") => "Left Touchpad Press".to_string(),
            Some("right_trackpad") => "Right Touchpad Press".to_string(),
            Some("gyro") => "Gyro".to_string(),
            _ => "Click".to_string(),
        },
        "edge" => match source {
            Some("left_trigger") => "L2 Soft Pull".to_string(),
            Some("right_trigger") => "R2 Soft Pull".to_string(),
            _ => "Soft Pull".to_string(),
        },
        "dpad_up" => "Swipe Up".to_string(),
        "dpad_down" => "Swipe Down".to_string(),
        "dpad_left" => "Swipe Left".to_string(),
        "dpad_right" => "Swipe Right".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn friendly_steam_binding(binding: &str) -> String {
    let binding = binding.trim();
    let Some((kind, rest)) = binding.split_once(' ') else {
        return title_case_words(&binding.replace('_', " "));
    };
    let target = rest.split(',').next().unwrap_or(rest).trim();
    match kind {
        "xinput_button" => match target.to_ascii_lowercase().as_str() {
            "a" => "A Button".to_string(),
            "b" => "B Button".to_string(),
            "x" => "X Button".to_string(),
            "y" => "Y Button".to_string(),
            "dpad_up" | "dpad_north" => "DPad Up".to_string(),
            "dpad_down" | "dpad_south" => "DPad Down".to_string(),
            "dpad_left" | "dpad_west" => "DPad Left".to_string(),
            "dpad_right" | "dpad_east" => "DPad Right".to_string(),
            "start" => "Start".to_string(),
            "select" | "back" => "Select".to_string(),
            "shoulder_left" => "Left Bumper".to_string(),
            "shoulder_right" => "Right Bumper".to_string(),
            "trigger_left" => "Left Trigger".to_string(),
            "trigger_right" => "Right Trigger".to_string(),
            "joystick_left" => "Left Stick Click".to_string(),
            "joystick_right" => "Right Stick Click".to_string(),
            other => title_case_words(&other.replace('_', " ")),
        },
        "key_press" => format!("{} Key", friendly_key_name(target)),
        "mouse_button" => format!("{} Mouse", title_case_words(&target.replace('_', " "))),
        "mouse_wheel" => format!("Wheel {}", title_case_words(&target.replace('_', " "))),
        "mode_shift" => "Mode Shift".to_string(),
        other => title_case_words(&format!("{} {}", other.replace('_', " "), target)),
    }
}

pub(super) fn steam_binding_kind(binding: &str) -> String {
    match binding.split_whitespace().next().unwrap_or("binding") {
        "xinput_button" => "XInput".to_string(),
        "key_press" => "Key".to_string(),
        "mouse_button" | "mouse_wheel" => "Mouse".to_string(),
        "mode_shift" => "Mode Shift".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn friendly_steam_source(source: &str) -> String {
    match source {
        "left_trackpad" => "Left Trackpad".to_string(),
        "right_trackpad" => "Right Trackpad".to_string(),
        "center_trackpad" => "Center Trackpad".to_string(),
        "joystick" => "Left Joystick".to_string(),
        "right_joystick" => "Right Joystick".to_string(),
        "dpad" => "Directional Pad".to_string(),
        "button_diamond" | "abxy" => "Face Buttons".to_string(),
        "left_trigger" => "Left Trigger".to_string(),
        "right_trigger" => "Right Trigger".to_string(),
        "gyro" => "Gyro".to_string(),
        "switch" => "Switches".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn friendly_steam_source_mode(mode: &str) -> String {
    match mode {
        "four_buttons" => "Four Buttons".to_string(),
        "dpad" => "Directional Pad".to_string(),
        "joystick_move" => "Joystick".to_string(),
        "joystick_camera" => "Joystick Camera".to_string(),
        "absolute_mouse" => "Mouse Region".to_string(),
        "relative_mouse" => "Mouse".to_string(),
        "mouse_joystick" => "Mouse Joystick".to_string(),
        "scrollwheel" => "Scroll Wheel".to_string(),
        "2dscroll" => "Directional Swipe".to_string(),
        "single_button" => "Single Button".to_string(),
        "trigger" => "Analog Trigger".to_string(),
        "switches" => "Switches".to_string(),
        "gyro" => "Gyro".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn friendly_steam_activator(activator: &str) -> String {
    match activator {
        "Full_Press" => "Full Press".to_string(),
        "Soft_Press" => "Soft Pull".to_string(),
        "Long_Press" => "Long Press".to_string(),
        "Double_Press" => "Double Press".to_string(),
        "Start_Press" => "Start Press".to_string(),
        "Release_Press" => "Release".to_string(),
        "Chord_Press" => "Chord".to_string(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn friendly_steam_controller_type(controller_type: &str) -> String {
    match controller_type {
        "controller_ps5_edge" => "DualSense Edge".to_string(),
        "controller_ps5" => "DualSense".to_string(),
        "controller_ps4" => "DualShock 4".to_string(),
        "controller_neptune" => "Steam Deck".to_string(),
        "controller_steamcontroller_gordon" => "Steam Controller".to_string(),
        "controller_xboxone" => "Xbox One".to_string(),
        "controller_xbox360" => "Xbox 360".to_string(),
        "controller_xboxelite" => "Xbox Elite".to_string(),
        "controller_generic" => "Generic Gamepad".to_string(),
        other => title_case_words(&other.replace("controller_", "").replace('_', " ")),
    }
}

fn friendly_key_name(key: &str) -> String {
    match key {
        "DASH" => "-".to_string(),
        "EQUALS" => "=".to_string(),
        "SPACE" => "Space".to_string(),
        "ENTER" => "Enter".to_string(),
        "ESCAPE" => "Esc".to_string(),
        other if other.len() == 1 => other.to_ascii_uppercase(),
        other => title_case_words(&other.replace('_', " ")),
    }
}

pub(super) fn clean_steam_layout_title(title: &str) -> String {
    if title.trim().is_empty() || title.starts_with('#') {
        "Steam Input Layout".to_string()
    } else {
        title.trim().chars().take(80).collect()
    }
}

pub(crate) fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
