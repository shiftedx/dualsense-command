use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use dscc_core::input_bridge::{
    InputBridgeConfig, InputBridgeSource, InputBridgeTarget, VirtualAxis,
};
use dscc_device::ControllerInputState;
#[cfg(any(test, debug_assertions))]
use dscc_virtual_output::MockVirtualOutputBackend;
use dscc_virtual_output::{
    HidMaestroBrokerBackend, VirtualButtonState, VirtualGamepadState, VirtualOutputBackend,
    VirtualOutputBackendState, VirtualOutputError, VirtualOutputKind, VirtualOutputTarget,
};
use serde::{Deserialize, Serialize};

const KNOWN_INPUT_BUTTON_COUNT: usize = 23;

#[derive(Clone)]
pub(crate) struct InputBridgeService {
    backend: Arc<dyn VirtualOutputBackend>,
    provider: String,
    sessions: Arc<Mutex<BTreeMap<String, InputBridgeSessionRecord>>>,
}

#[derive(Clone, Debug)]
struct InputBridgeSessionRecord {
    controller_id: String,
    state: InputBridgeSessionState,
    target: Option<VirtualOutputTarget>,
    message: String,
    updated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputBridgeSessionState {
    Disabled,
    Starting,
    Ready,
    Active,
    Stale,
    Stopping,
    Faulted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeStatusResponse {
    pub available: bool,
    pub backend_id: String,
    pub provider: String,
    pub state: String,
    pub message: String,
    pub supported_kinds: Vec<String>,
    pub sessions: Vec<InputBridgeSessionSummary>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputBridgeSessionSummary {
    pub controller_id: String,
    pub state: InputBridgeSessionState,
    pub session_id: Option<String>,
    pub output_kind: Option<String>,
    pub message: String,
    pub updated_at_ms: u64,
}

impl InputBridgeService {
    pub(crate) fn production() -> Self {
        Self {
            backend: Arc::new(HidMaestroBrokerBackend::from_env_or_default()),
            provider: "hidmaestro".to_string(),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    #[cfg(any(test, debug_assertions))]
    pub(crate) fn mock() -> Self {
        Self {
            backend: Arc::new(MockVirtualOutputBackend::new()),
            provider: "mock".to_string(),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(crate) fn status_response(&self) -> InputBridgeStatusResponse {
        let backend = self.backend.status();
        let sessions = self.session_summaries();
        InputBridgeStatusResponse {
            available: backend.state == VirtualOutputBackendState::Available,
            backend_id: public_backend_id(&backend.backend_id),
            provider: self.provider.clone(),
            state: backend_state_label(backend.state).to_string(),
            message: public_backend_status_message(&backend.message, backend.state),
            supported_kinds: backend
                .supported_kinds
                .into_iter()
                .map(output_kind_label)
                .collect(),
            sessions,
            warnings: input_bridge_warnings(&self.provider, backend.state),
        }
    }

    pub(crate) fn session_summary(&self, controller_id: &str) -> InputBridgeSessionSummary {
        self.lock()
            .get(controller_id)
            .map(session_record_summary)
            .unwrap_or_else(|| InputBridgeSessionSummary {
                controller_id: controller_id.to_string(),
                state: InputBridgeSessionState::Disabled,
                session_id: None,
                output_kind: None,
                message: "DSCC Input Bridge is not running for this controller.".to_string(),
                updated_at_ms: 0,
            })
    }

    pub(crate) fn start_session(
        &self,
        controller_id: &str,
        output_kind: VirtualOutputKind,
        updated_at_ms: u64,
    ) -> Result<InputBridgeSessionSummary, String> {
        let old_target = {
            let mut sessions = self.lock();
            if let Some(record) = sessions.get(controller_id) {
                if matches!(
                    record.state,
                    InputBridgeSessionState::Active | InputBridgeSessionState::Starting
                ) {
                    return Ok(session_record_summary(record));
                }
            }
            let old_target = sessions
                .get_mut(controller_id)
                .and_then(|record| record.target.take());
            let record = InputBridgeSessionRecord {
                controller_id: controller_id.to_string(),
                state: InputBridgeSessionState::Starting,
                target: None,
                message: "DSCC Input Bridge is starting virtual output.".to_string(),
                updated_at_ms,
            };
            sessions.insert(controller_id.to_string(), record);
            old_target
        };
        if let Some(target) = old_target.as_ref() {
            let _ = self.backend.reset(target);
            let _ = self.backend.drop_session(target);
        }

        let target = match self.backend.create_session(controller_id, output_kind) {
            Ok(target) => target,
            Err(error) => {
                let message = public_virtual_output_error(&error);
                let mut sessions = self.lock();
                let record = InputBridgeSessionRecord {
                    controller_id: controller_id.to_string(),
                    state: InputBridgeSessionState::Faulted,
                    target: None,
                    message: message.clone(),
                    updated_at_ms,
                };
                sessions.insert(controller_id.to_string(), record);
                return Err(message);
            }
        };
        let mut sessions = self.lock();
        let record = InputBridgeSessionRecord {
            controller_id: controller_id.to_string(),
            state: InputBridgeSessionState::Active,
            target: Some(target),
            message: "DSCC Input Bridge session is active.".to_string(),
            updated_at_ms,
        };
        let summary = session_record_summary(&record);
        sessions.insert(controller_id.to_string(), record);
        Ok(summary)
    }

    pub(crate) fn is_active(&self, controller_id: &str) -> bool {
        self.lock()
            .get(controller_id)
            .is_some_and(|record| record.state == InputBridgeSessionState::Active)
    }

    pub(crate) fn submit_controller_input(
        &self,
        controller_id: &str,
        input: &ControllerInputState,
        config: &InputBridgeConfig,
        updated_at_ms: u64,
    ) -> Result<InputBridgeSessionSummary, String> {
        let target = self
            .lock()
            .get(controller_id)
            .and_then(|record| record.target.clone())
            .ok_or_else(|| "DSCC Input Bridge session is not active".to_string())?;
        let state = virtual_state_from_input(input, config);
        self.backend
            .submit_state(&target, &state)
            .map_err(|error| public_virtual_output_error(&error))?;
        let mut sessions = self.lock();
        let record = sessions
            .get_mut(controller_id)
            .ok_or_else(|| "DSCC Input Bridge session disappeared".to_string())?;
        record.state = InputBridgeSessionState::Active;
        record.message = "DSCC Input Bridge is forwarding typed controller input.".to_string();
        record.updated_at_ms = updated_at_ms;
        Ok(session_record_summary(record))
    }

    pub(crate) fn neutralize_session(
        &self,
        controller_id: &str,
        state: InputBridgeSessionState,
        message: impl Into<String>,
        updated_at_ms: u64,
    ) -> InputBridgeSessionSummary {
        let target = {
            let mut sessions = self.lock();
            sessions
                .get_mut(controller_id)
                .and_then(|record| record.target.take())
        };
        if let Some(target) = target.as_ref() {
            let _ = self.backend.reset(target);
            let _ = self.backend.drop_session(target);
        }
        let mut sessions = self.lock();
        if let Some(record) = sessions.get_mut(controller_id) {
            record.state = state;
            record.target = None;
            record.message = message.into();
            record.updated_at_ms = updated_at_ms;
            return session_record_summary(record);
        }
        InputBridgeSessionSummary {
            controller_id: controller_id.to_string(),
            state: InputBridgeSessionState::Disabled,
            session_id: None,
            output_kind: None,
            message: "DSCC Input Bridge is not running for this controller.".to_string(),
            updated_at_ms,
        }
    }

    pub(crate) fn stop_session(
        &self,
        controller_id: &str,
        updated_at_ms: u64,
    ) -> InputBridgeSessionSummary {
        let record = {
            let mut sessions = self.lock();
            sessions.remove(controller_id)
        };
        let Some(record) = record else {
            return InputBridgeSessionSummary {
                controller_id: controller_id.to_string(),
                state: InputBridgeSessionState::Disabled,
                session_id: None,
                output_kind: None,
                message: "DSCC Input Bridge was already stopped for this controller.".to_string(),
                updated_at_ms,
            };
        };
        if let Some(target) = record.target.as_ref() {
            let _ = self.backend.reset(target);
            let _ = self.backend.drop_session(target);
        }
        InputBridgeSessionSummary {
            controller_id: controller_id.to_string(),
            state: InputBridgeSessionState::Disabled,
            session_id: None,
            output_kind: None,
            message: "DSCC Input Bridge stopped and neutralized virtual output.".to_string(),
            updated_at_ms,
        }
    }

    fn session_summaries(&self) -> Vec<InputBridgeSessionSummary> {
        self.lock().values().map(session_record_summary).collect()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, InputBridgeSessionRecord>> {
        match self.sessions.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn session_record_summary(record: &InputBridgeSessionRecord) -> InputBridgeSessionSummary {
    InputBridgeSessionSummary {
        controller_id: record.controller_id.clone(),
        state: record.state,
        session_id: record
            .target
            .as_ref()
            .map(|target| public_session_id(&target.session_id)),
        output_kind: record
            .target
            .as_ref()
            .map(|target| output_kind_label(target.kind)),
        message: record.message.clone(),
        updated_at_ms: record.updated_at_ms,
    }
}

fn backend_state_label(state: VirtualOutputBackendState) -> &'static str {
    match state {
        VirtualOutputBackendState::Available => "available",
        VirtualOutputBackendState::Unavailable => "unavailable",
        VirtualOutputBackendState::Faulted => "faulted",
    }
}

fn public_backend_status_message(message: &str, state: VirtualOutputBackendState) -> String {
    let trimmed = message.trim();
    let safe = !trimmed.is_empty()
        && trimmed.len() <= 180
        && !trimmed.contains(":\\")
        && !trimmed.contains("\\\\")
        && !trimmed.contains('/');
    if safe {
        return trimmed.to_string();
    }
    match state {
        VirtualOutputBackendState::Available => "Virtual output backend is available.".to_string(),
        VirtualOutputBackendState::Unavailable => {
            "Virtual output backend is unavailable.".to_string()
        }
        VirtualOutputBackendState::Faulted => "Virtual output backend fault.".to_string(),
    }
}

fn input_bridge_warnings(provider: &str, state: VirtualOutputBackendState) -> Vec<String> {
    #[cfg(any(test, debug_assertions))]
    if provider == "mock" {
        return vec!["Input Bridge is using the in-memory test backend.".to_string()];
    }

    match (provider, state) {
        ("hidmaestro", VirtualOutputBackendState::Available) => vec![
            "HIDMaestro creates the virtual controller; HidHide remains optional for duplicate-input control.".to_string(),
        ],
        ("hidmaestro", _) => vec![
            "Install or configure the DSCC HIDMaestro broker before starting bridge sessions.".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn public_backend_id(raw: &str) -> String {
    public_identifier(raw, "virtual-output", 32).to_ascii_lowercase()
}

fn public_session_id(raw: &str) -> String {
    public_identifier(raw, "virtual-session", 64)
}

fn public_identifier(raw: &str, fallback: &str, max_len: usize) -> String {
    let trimmed = raw.trim();
    let is_safe = !trimmed.is_empty()
        && trimmed.len() <= max_len
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if is_safe {
        trimmed.to_string()
    } else {
        fallback.to_string()
    }
}

fn output_kind_label(kind: VirtualOutputKind) -> String {
    match kind {
        VirtualOutputKind::Xbox360 => "xbox360".to_string(),
    }
}

fn public_virtual_output_error(error: &VirtualOutputError) -> String {
    match error {
        VirtualOutputError::Unavailable(_) => "Virtual output backend is unavailable.".to_string(),
        VirtualOutputError::SessionNotFound(_) => {
            "Virtual output session was not found.".to_string()
        }
        VirtualOutputError::BackendFault(_) => "Virtual output backend fault.".to_string(),
    }
}

fn virtual_state_from_input(
    input: &ControllerInputState,
    config: &InputBridgeConfig,
) -> VirtualGamepadState {
    let mut state = VirtualGamepadState::neutral();
    let input_view = BridgeInputView::new(input);
    let mut buttons = BTreeMap::new();
    for binding in &config.bindings {
        match binding.target {
            InputBridgeTarget::Disabled | InputBridgeTarget::Command(_) => {}
            InputBridgeTarget::PassThrough => {}
            InputBridgeTarget::Button(button) => {
                let pressed = source_pressed(&input_view, &binding.source);
                *buttons.entry(button.id().to_string()).or_insert(false) |= pressed;
            }
            InputBridgeTarget::Axis(axis) => {
                let value = source_axis_value(&input_view, &binding.source, axis);
                apply_axis(&mut state, axis, value);
            }
        }
    }
    state.buttons = VirtualButtonState { buttons };
    state
}

#[derive(Clone, Copy, Debug, Default)]
struct BridgeButtonSample {
    pressed: bool,
    value: f64,
}

struct BridgeInputView<'a> {
    input: &'a ControllerInputState,
    buttons: [Option<BridgeButtonSample>; KNOWN_INPUT_BUTTON_COUNT],
}

impl<'a> BridgeInputView<'a> {
    fn new(input: &'a ControllerInputState) -> Self {
        let mut buttons = [None; KNOWN_INPUT_BUTTON_COUNT];
        for button in &input.buttons {
            if let Some(index) = known_button_index(button.id) {
                buttons[index] = Some(BridgeButtonSample {
                    pressed: button.pressed,
                    value: button.value,
                });
            }
        }
        Self { input, buttons }
    }

    fn button(&self, id: &str) -> Option<BridgeButtonSample> {
        known_button_index(id)
            .and_then(|index| self.buttons[index])
            .or_else(|| {
                self.input
                    .buttons
                    .iter()
                    .find(|button| button.id == id)
                    .map(|button| BridgeButtonSample {
                        pressed: button.pressed,
                        value: button.value,
                    })
            })
    }
}

fn known_button_index(id: &str) -> Option<usize> {
    match id {
        "dpad_up" => Some(0),
        "dpad_right" => Some(1),
        "dpad_down" => Some(2),
        "dpad_left" => Some(3),
        "square" => Some(4),
        "cross" => Some(5),
        "circle" => Some(6),
        "triangle" => Some(7),
        "l1" => Some(8),
        "r1" => Some(9),
        "l2" => Some(10),
        "r2" => Some(11),
        "create" => Some(12),
        "options" => Some(13),
        "l3" => Some(14),
        "r3" => Some(15),
        "ps" => Some(16),
        "touchpad" => Some(17),
        "mute" => Some(18),
        "edge_fn_left" => Some(19),
        "edge_fn_right" => Some(20),
        "edge_back_left" => Some(21),
        "edge_back_right" => Some(22),
        _ => None,
    }
}

fn source_pressed(input: &BridgeInputView<'_>, source: &InputBridgeSource) -> bool {
    match source {
        InputBridgeSource::Button(id) => input.button(id).is_some_and(|button| button.pressed),
        InputBridgeSource::Axis(id) => source_axis_unit(input, id) > 0.5,
        InputBridgeSource::Stick(id) => match id.as_str() {
            "left_stick" => input.input.left_stick.magnitude > 0.5,
            "right_stick" => input.input.right_stick.magnitude > 0.5,
            _ => false,
        },
    }
}

fn source_axis_value(
    input: &BridgeInputView<'_>,
    source: &InputBridgeSource,
    target_axis: VirtualAxis,
) -> f64 {
    match source {
        InputBridgeSource::Button(id) => input
            .button(id)
            .map(|button| if button.pressed { 1.0 } else { 0.0 })
            .unwrap_or(0.0),
        InputBridgeSource::Axis(id) => source_axis_unit(input, id),
        InputBridgeSource::Stick(id) => match (id.as_str(), target_axis) {
            ("left_stick", VirtualAxis::LeftStickX) => input.input.left_stick.x,
            ("left_stick", VirtualAxis::LeftStickY) => input.input.left_stick.y,
            ("right_stick", VirtualAxis::RightStickX) => input.input.right_stick.x,
            ("right_stick", VirtualAxis::RightStickY) => input.input.right_stick.y,
            _ => 0.0,
        },
    }
}

fn source_axis_unit(input: &BridgeInputView<'_>, id: &str) -> f64 {
    match id {
        "l2" => input.input.l2,
        "r2" => input.input.r2,
        _ => input.button(id).map(|button| button.value).unwrap_or(0.0),
    }
    .clamp(0.0, 1.0)
}

fn apply_axis(state: &mut VirtualGamepadState, axis: VirtualAxis, value: f64) {
    match axis {
        VirtualAxis::LeftStickX => state.left_stick.x = value.clamp(-1.0, 1.0),
        VirtualAxis::LeftStickY => state.left_stick.y = value.clamp(-1.0, 1.0),
        VirtualAxis::RightStickX => state.right_stick.x = value.clamp(-1.0, 1.0),
        VirtualAxis::RightStickY => state.right_stick.y = value.clamp(-1.0, 1.0),
        VirtualAxis::LeftTrigger => state.triggers.l2 = value.clamp(0.0, 1.0),
        VirtualAxis::RightTrigger => state.triggers.r2 = value.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dscc_core::input_bridge::{InputBridgeBindingConfig, VirtualButton};
    use dscc_device::{ControllerInputButtonState, ControllerInputStickState};
    use dscc_virtual_output::VirtualOutputBackendStatus;

    fn test_input(buttons: Vec<ControllerInputButtonState>) -> ControllerInputState {
        ControllerInputState {
            left_stick: ControllerInputStickState {
                x: 0.0,
                y: 0.0,
                magnitude: 0.0,
            },
            right_stick: ControllerInputStickState {
                x: 0.0,
                y: 0.0,
                magnitude: 0.0,
            },
            l2: 0.0,
            r2: 0.0,
            buttons,
        }
    }

    #[test]
    fn duplicate_virtual_button_targets_or_pressed_sources() {
        let input = test_input(vec![
            ControllerInputButtonState {
                id: "l3",
                label: "L3",
                pressed: true,
                value: 1.0,
            },
            ControllerInputButtonState {
                id: "edge_back_left",
                label: "Back Left",
                pressed: false,
                value: 0.0,
            },
        ]);
        let config = InputBridgeConfig {
            enabled: true,
            bindings: vec![
                InputBridgeBindingConfig {
                    source: InputBridgeSource::Button("l3".to_string()),
                    target: InputBridgeTarget::Button(VirtualButton::LeftThumb),
                },
                InputBridgeBindingConfig {
                    source: InputBridgeSource::Button("edge_back_left".to_string()),
                    target: InputBridgeTarget::Button(VirtualButton::LeftThumb),
                },
            ],
            ..InputBridgeConfig::default()
        };

        let state = virtual_state_from_input(&input, &config);

        assert_eq!(
            state.buttons.buttons.get("left_thumb"),
            Some(&true),
            "idle duplicate source must not cancel a pressed source"
        );
    }

    #[test]
    fn fallback_button_lookup_supports_future_typed_sources() {
        let input = test_input(vec![ControllerInputButtonState {
            id: "custom_pressure",
            label: "Custom Pressure",
            pressed: false,
            value: 0.75,
        }]);
        let config = InputBridgeConfig {
            enabled: true,
            bindings: vec![InputBridgeBindingConfig {
                source: InputBridgeSource::Axis("custom_pressure".to_string()),
                target: InputBridgeTarget::Axis(VirtualAxis::LeftTrigger),
            }],
            ..InputBridgeConfig::default()
        };

        let state = virtual_state_from_input(&input, &config);

        assert_eq!(state.triggers.l2, 0.75);
    }

    #[test]
    fn neutralize_session_drops_target_before_restart() {
        let service = InputBridgeService::mock();
        let first = service
            .start_session("controller-1", VirtualOutputKind::Xbox360, 1)
            .unwrap();
        assert_eq!(first.state, InputBridgeSessionState::Active);
        let first_session_id = first.session_id.clone();

        let stale =
            service.neutralize_session("controller-1", InputBridgeSessionState::Stale, "stale", 2);
        assert_eq!(stale.state, InputBridgeSessionState::Stale);
        assert_eq!(stale.session_id, None);

        let second = service
            .start_session("controller-1", VirtualOutputKind::Xbox360, 3)
            .unwrap();
        assert_eq!(second.state, InputBridgeSessionState::Active);
        assert_ne!(second.session_id, first_session_id);
    }

    #[test]
    fn duplicate_start_reuses_active_session() {
        let service = InputBridgeService::mock();
        let first = service
            .start_session("controller-1", VirtualOutputKind::Xbox360, 1)
            .unwrap();
        let second = service
            .start_session("controller-1", VirtualOutputKind::Xbox360, 2)
            .unwrap();

        assert_eq!(second.state, InputBridgeSessionState::Active);
        assert_eq!(second.session_id, first.session_id);
        assert_eq!(second.updated_at_ms, first.updated_at_ms);
    }

    #[test]
    fn public_status_redacts_private_backend_and_session_ids() {
        let service = InputBridgeService {
            backend: Arc::new(PrivateIdBackend),
            provider: "hidmaestro".to_string(),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        };

        let status = service.status_response();
        let summary = service
            .start_session("controller-1", VirtualOutputKind::Xbox360, 1)
            .unwrap();

        assert_eq!(status.backend_id, "virtual-output");
        assert_eq!(summary.session_id.as_deref(), Some("virtual-session"));
        assert_eq!(summary.message, "DSCC Input Bridge session is active.");
    }

    struct PrivateIdBackend;

    impl VirtualOutputBackend for PrivateIdBackend {
        fn status(&self) -> VirtualOutputBackendStatus {
            VirtualOutputBackendStatus {
                backend_id: r"\\.\pipe\dscc-provider\private-token".to_string(),
                state: VirtualOutputBackendState::Available,
                message: "available".to_string(),
                supported_kinds: vec![VirtualOutputKind::Xbox360],
            }
        }

        fn create_session(
            &self,
            _controller_id: &str,
            kind: VirtualOutputKind,
        ) -> Result<VirtualOutputTarget, VirtualOutputError> {
            Ok(VirtualOutputTarget {
                session_id: r"\\.\pipe\session\private-token".to_string(),
                kind,
            })
        }

        fn submit_state(
            &self,
            _target: &VirtualOutputTarget,
            _state: &VirtualGamepadState,
        ) -> Result<(), VirtualOutputError> {
            Ok(())
        }

        fn drop_session(&self, _target: &VirtualOutputTarget) -> Result<(), VirtualOutputError> {
            Ok(())
        }
    }
}
