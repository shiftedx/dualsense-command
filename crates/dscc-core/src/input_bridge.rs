use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub output_kind: InputBridgeOutputKind,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub bindings: Vec<InputBridgeBindingConfig>,
    #[serde(default = "default_input_bridge_shift_bindings")]
    pub shift_bindings: Vec<InputBridgeBindingConfig>,
}

impl Default for InputBridgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            output_kind: InputBridgeOutputKind::Xbox360,
            auto_start: false,
            bindings: default_input_bridge_bindings(),
            shift_bindings: default_input_bridge_shift_bindings(),
        }
    }
}

impl InputBridgeConfig {
    pub fn normalized(mut self) -> Self {
        self.output_kind = InputBridgeOutputKind::Xbox360;
        self.bindings =
            normalize_input_bridge_bindings(self.bindings, default_input_bridge_bindings());
        self.shift_bindings = normalize_input_bridge_bindings(
            self.shift_bindings,
            default_input_bridge_shift_bindings(),
        );
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputBridgeOutputKind {
    #[default]
    Xbox360,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InputBridgeSource {
    Button(String),
    Axis(String),
    Stick(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeBindingConfig {
    pub source: InputBridgeSource,
    pub target: InputBridgeTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InputBridgeTarget {
    PassThrough,
    Disabled,
    Button(VirtualButton),
    Axis(VirtualAxis),
    Command(DsccBridgeCommand),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum VirtualButton {
    A,
    B,
    X,
    Y,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftShoulder,
    RightShoulder,
    LeftThumb,
    RightThumb,
    Back,
    Start,
    Guide,
}

impl VirtualButton {
    pub fn id(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::X => "x",
            Self::Y => "y",
            Self::DpadUp => "dpad_up",
            Self::DpadDown => "dpad_down",
            Self::DpadLeft => "dpad_left",
            Self::DpadRight => "dpad_right",
            Self::LeftShoulder => "left_shoulder",
            Self::RightShoulder => "right_shoulder",
            Self::LeftThumb => "left_thumb",
            Self::RightThumb => "right_thumb",
            Self::Back => "back",
            Self::Start => "start",
            Self::Guide => "guide",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum VirtualAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DsccBridgeCommand {
    ShiftLayer,
    ProfileNext,
    ProfilePrevious,
}

pub fn default_input_bridge_bindings() -> Vec<InputBridgeBindingConfig> {
    use InputBridgeSource::{Axis, Button, Stick};
    use InputBridgeTarget::{Axis as TargetAxis, Button as TargetButton, Command};
    vec![
        binding(Button("cross".into()), TargetButton(VirtualButton::A)),
        binding(Button("circle".into()), TargetButton(VirtualButton::B)),
        binding(Button("square".into()), TargetButton(VirtualButton::X)),
        binding(Button("triangle".into()), TargetButton(VirtualButton::Y)),
        binding(
            Button("dpad_up".into()),
            TargetButton(VirtualButton::DpadUp),
        ),
        binding(
            Button("dpad_down".into()),
            TargetButton(VirtualButton::DpadDown),
        ),
        binding(
            Button("dpad_left".into()),
            TargetButton(VirtualButton::DpadLeft),
        ),
        binding(
            Button("dpad_right".into()),
            TargetButton(VirtualButton::DpadRight),
        ),
        binding(
            Button("l1".into()),
            TargetButton(VirtualButton::LeftShoulder),
        ),
        binding(
            Button("r1".into()),
            TargetButton(VirtualButton::RightShoulder),
        ),
        binding(Button("l3".into()), TargetButton(VirtualButton::LeftThumb)),
        binding(Button("r3".into()), TargetButton(VirtualButton::RightThumb)),
        binding(Button("create".into()), TargetButton(VirtualButton::Back)),
        binding(Button("options".into()), TargetButton(VirtualButton::Start)),
        binding(Button("ps".into()), TargetButton(VirtualButton::Guide)),
        binding(
            Button("edge_back_left".into()),
            TargetButton(VirtualButton::LeftThumb),
        ),
        binding(
            Button("edge_back_right".into()),
            TargetButton(VirtualButton::RightThumb),
        ),
        binding(
            Button("edge_fn_left".into()),
            Command(DsccBridgeCommand::ShiftLayer),
        ),
        binding(
            Button("edge_fn_right".into()),
            Command(DsccBridgeCommand::ShiftLayer),
        ),
        binding(
            Stick("left_stick".into()),
            TargetAxis(VirtualAxis::LeftStickX),
        ),
        binding(
            Stick("left_stick".into()),
            TargetAxis(VirtualAxis::LeftStickY),
        ),
        binding(
            Stick("right_stick".into()),
            TargetAxis(VirtualAxis::RightStickX),
        ),
        binding(
            Stick("right_stick".into()),
            TargetAxis(VirtualAxis::RightStickY),
        ),
        binding(Axis("l2".into()), TargetAxis(VirtualAxis::LeftTrigger)),
        binding(Axis("r2".into()), TargetAxis(VirtualAxis::RightTrigger)),
    ]
}

pub fn default_input_bridge_shift_bindings() -> Vec<InputBridgeBindingConfig> {
    use InputBridgeSource::Button;
    use InputBridgeTarget::Command;
    vec![
        binding(
            Button("dpad_left".into()),
            Command(DsccBridgeCommand::ProfilePrevious),
        ),
        binding(
            Button("dpad_right".into()),
            Command(DsccBridgeCommand::ProfileNext),
        ),
    ]
}

fn binding(source: InputBridgeSource, target: InputBridgeTarget) -> InputBridgeBindingConfig {
    InputBridgeBindingConfig { source, target }
}

fn normalize_input_bridge_bindings(
    bindings: Vec<InputBridgeBindingConfig>,
    defaults: Vec<InputBridgeBindingConfig>,
) -> Vec<InputBridgeBindingConfig> {
    let configured_sources: BTreeSet<InputBridgeSource> = bindings
        .iter()
        .map(|binding| binding.source.clone())
        .collect();
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for binding in bindings.into_iter().chain(
        defaults
            .into_iter()
            .filter(|binding| !configured_sources.contains(&binding.source)),
    ) {
        let key = (binding.source.clone(), binding.target.clone());
        if seen.insert(key) {
            normalized.push(binding);
        }
        if normalized.len() >= 96 {
            break;
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_defaults_include_standard_xinput_shape() {
        let config = InputBridgeConfig::default().normalized();
        assert!(config.bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("cross".to_string())
                && binding.target == InputBridgeTarget::Button(VirtualButton::A)
        }));
        assert!(config.bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Axis("r2".to_string())
                && binding.target == InputBridgeTarget::Axis(VirtualAxis::RightTrigger)
        }));
    }

    #[test]
    fn bridge_normalization_dedupes_and_readds_defaults() {
        let config = InputBridgeConfig {
            enabled: true,
            bindings: vec![
                binding(
                    InputBridgeSource::Button("cross".to_string()),
                    InputBridgeTarget::Button(VirtualButton::A),
                ),
                binding(
                    InputBridgeSource::Button("cross".to_string()),
                    InputBridgeTarget::Button(VirtualButton::A),
                ),
            ],
            ..InputBridgeConfig::default()
        }
        .normalized();
        let cross_to_a = config
            .bindings
            .iter()
            .filter(|binding| {
                binding.source == InputBridgeSource::Button("cross".to_string())
                    && binding.target == InputBridgeTarget::Button(VirtualButton::A)
            })
            .count();
        assert_eq!(cross_to_a, 1);
        assert!(config.bindings.len() > 10);
    }

    #[test]
    fn bridge_normalization_preserves_disabled_overrides() {
        let config = InputBridgeConfig {
            enabled: true,
            bindings: vec![binding(
                InputBridgeSource::Button("cross".to_string()),
                InputBridgeTarget::Disabled,
            )],
            ..InputBridgeConfig::default()
        }
        .normalized();

        assert!(config.bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("cross".to_string())
                && binding.target == InputBridgeTarget::Disabled
        }));
        assert!(!config.bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("cross".to_string())
                && binding.target == InputBridgeTarget::Button(VirtualButton::A)
        }));
    }

    #[test]
    fn bridge_defaults_include_shift_profile_cycle() {
        let config = InputBridgeConfig::default().normalized();
        assert!(config.shift_bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("dpad_left".to_string())
                && binding.target == InputBridgeTarget::Command(DsccBridgeCommand::ProfilePrevious)
        }));
        assert!(config.shift_bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("dpad_right".to_string())
                && binding.target == InputBridgeTarget::Command(DsccBridgeCommand::ProfileNext)
        }));
        assert_eq!(config.shift_bindings, default_input_bridge_shift_bindings());
    }

    #[test]
    fn shift_binding_override_replaces_default_for_source() {
        let config = InputBridgeConfig {
            shift_bindings: vec![binding(
                InputBridgeSource::Button("dpad_right".to_string()),
                InputBridgeTarget::Disabled,
            )],
            ..InputBridgeConfig::default()
        }
        .normalized();

        assert!(config.shift_bindings.iter().any(|binding| {
            binding.source == InputBridgeSource::Button("dpad_right".to_string())
                && binding.target == InputBridgeTarget::Disabled
        }));
        assert!(!config.shift_bindings.iter().any(|binding| {
            binding.target == InputBridgeTarget::Command(DsccBridgeCommand::ProfileNext)
        }));
        assert!(config.shift_bindings.iter().any(|binding| {
            binding.target == InputBridgeTarget::Command(DsccBridgeCommand::ProfilePrevious)
        }));
    }

    #[test]
    fn bridge_config_without_shift_bindings_deserializes_defaults() {
        let config: InputBridgeConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.shift_bindings, default_input_bridge_shift_bindings());
    }
}
