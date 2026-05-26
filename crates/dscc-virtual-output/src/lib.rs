use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VirtualOutputKind {
    #[default]
    Xbox360,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualStickState {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTriggerState {
    pub l2: f64,
    pub r2: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualButtonState {
    pub buttons: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualGamepadState {
    pub left_stick: VirtualStickState,
    pub right_stick: VirtualStickState,
    pub triggers: VirtualTriggerState,
    pub buttons: VirtualButtonState,
}

impl VirtualGamepadState {
    pub fn neutral() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualOutputTarget {
    pub session_id: String,
    pub kind: VirtualOutputKind,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VirtualOutputBackendState {
    Available,
    Unavailable,
    Faulted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualOutputBackendStatus {
    pub backend_id: String,
    pub state: VirtualOutputBackendState,
    pub message: String,
    pub supported_kinds: Vec<VirtualOutputKind>,
}

#[derive(Debug, Error)]
pub enum VirtualOutputError {
    #[error("virtual output backend is unavailable: {0}")]
    Unavailable(String),
    #[error("virtual output session was not found: {0}")]
    SessionNotFound(String),
    #[error("virtual output backend fault: {0}")]
    BackendFault(String),
}

pub trait VirtualOutputBackend: Send + Sync + 'static {
    fn status(&self) -> VirtualOutputBackendStatus;
    fn create_session(
        &self,
        controller_id: &str,
        kind: VirtualOutputKind,
    ) -> Result<VirtualOutputTarget, VirtualOutputError>;
    fn submit_state(
        &self,
        target: &VirtualOutputTarget,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError>;
    fn reset(&self, target: &VirtualOutputTarget) -> Result<(), VirtualOutputError> {
        self.submit_state(target, &VirtualGamepadState::neutral())
    }
    fn drop_session(&self, target: &VirtualOutputTarget) -> Result<(), VirtualOutputError>;
}

const HIDMAESTRO_BROKER_ENV: &str = "DSCC_HIDMAESTRO_BROKER";
const BROKER_PROTOCOL: &str = "dev.dscc.hidmaestro-broker.v1";

#[derive(Clone, Debug)]
pub struct HidMaestroBrokerBackend {
    command: Option<BrokerCommand>,
    inner: Arc<Mutex<Option<BrokerProcess>>>,
}

#[derive(Clone, Debug)]
struct BrokerCommand {
    program: PathBuf,
    args: Vec<String>,
}

#[derive(Debug)]
struct BrokerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrokerRequest<'a> {
    protocol: &'static str,
    id: u64,
    command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrokerUpdateFrame<'a> {
    protocol: &'static str,
    id: u64,
    command: &'static str,
    session_id: &'a str,
    kind: &'static str,
    lx: i16,
    ly: i16,
    rx: i16,
    ry: i16,
    lt: u16,
    rt: u16,
    buttons: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrokerResponse {
    id: u64,
    ok: bool,
    #[serde(default)]
    available: Option<bool>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    supported_kinds: Vec<String>,
}

impl HidMaestroBrokerBackend {
    pub fn from_env_or_default() -> Self {
        Self {
            command: discover_hidmaestro_broker(),
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            command: None,
            inner: Arc::new(Mutex::new(None)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<BrokerProcess>> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn broker_status(&self) -> Result<BrokerResponse, VirtualOutputError> {
        let Some(command) = &self.command else {
            return Err(VirtualOutputError::Unavailable(
                "DSCC HIDMaestro broker is not installed or configured.".to_string(),
            ));
        };
        let mut guard = self.lock();
        let process = ensure_broker_process(&mut guard, command)?;
        process.request("provider_status", None, None, None)
    }

    fn broker_request(
        &self,
        command_name: &str,
        controller_id: Option<&str>,
        session_id: Option<&str>,
        kind: Option<VirtualOutputKind>,
    ) -> Result<BrokerResponse, VirtualOutputError> {
        let Some(command) = &self.command else {
            return Err(VirtualOutputError::Unavailable(
                "DSCC HIDMaestro broker is not installed or configured.".to_string(),
            ));
        };
        let mut guard = self.lock();
        let process = ensure_broker_process(&mut guard, command)?;
        process.request(command_name, controller_id, session_id, kind)
    }

    fn broker_update(
        &self,
        target: &VirtualOutputTarget,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError> {
        let Some(command) = &self.command else {
            return Err(VirtualOutputError::Unavailable(
                "DSCC HIDMaestro broker is not installed or configured.".to_string(),
            ));
        };
        let mut guard = self.lock();
        let process = ensure_broker_process(&mut guard, command)?;
        process.send_update(&target.session_id, target.kind, state)
    }
}

impl VirtualOutputBackend for HidMaestroBrokerBackend {
    fn status(&self) -> VirtualOutputBackendStatus {
        match self.broker_status() {
            Ok(response) => {
                let available = response.ok && response.available.unwrap_or(true);
                VirtualOutputBackendStatus {
                    backend_id: "hidmaestro".to_string(),
                    state: if available {
                        VirtualOutputBackendState::Available
                    } else {
                        VirtualOutputBackendState::Unavailable
                    },
                    message: response.message.unwrap_or_else(|| {
                        if available {
                            "HIDMaestro broker is available.".to_string()
                        } else {
                            "HIDMaestro broker is unavailable.".to_string()
                        }
                    }),
                    supported_kinds: response
                        .supported_kinds
                        .iter()
                        .filter_map(|kind| parse_output_kind(kind))
                        .collect(),
                }
            }
            Err(VirtualOutputError::Unavailable(message)) => VirtualOutputBackendStatus {
                backend_id: "hidmaestro".to_string(),
                state: VirtualOutputBackendState::Unavailable,
                message,
                supported_kinds: Vec::new(),
            },
            Err(_) => VirtualOutputBackendStatus {
                backend_id: "hidmaestro".to_string(),
                state: VirtualOutputBackendState::Faulted,
                message: "HIDMaestro broker failed its health check.".to_string(),
                supported_kinds: Vec::new(),
            },
        }
    }

    fn create_session(
        &self,
        controller_id: &str,
        kind: VirtualOutputKind,
    ) -> Result<VirtualOutputTarget, VirtualOutputError> {
        let response = self.broker_request("create", Some(controller_id), None, Some(kind))?;
        if !response.ok {
            return Err(VirtualOutputError::BackendFault(
                response
                    .message
                    .unwrap_or_else(|| "HIDMaestro broker refused session creation.".to_string()),
            ));
        }
        let session_id = response.session_id.ok_or_else(|| {
            VirtualOutputError::BackendFault(
                "HIDMaestro broker did not return a session id.".to_string(),
            )
        })?;
        Ok(VirtualOutputTarget { session_id, kind })
    }

    fn submit_state(
        &self,
        target: &VirtualOutputTarget,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError> {
        self.broker_update(target, state)
    }

    fn drop_session(&self, target: &VirtualOutputTarget) -> Result<(), VirtualOutputError> {
        let response =
            self.broker_request("destroy", None, Some(&target.session_id), Some(target.kind))?;
        if response.ok {
            Ok(())
        } else {
            Err(VirtualOutputError::BackendFault(
                response.message.unwrap_or_else(|| {
                    "HIDMaestro broker refused session destruction.".to_string()
                }),
            ))
        }
    }
}

impl BrokerProcess {
    fn request(
        &mut self,
        command: &str,
        controller_id: Option<&str>,
        session_id: Option<&str>,
        kind: Option<VirtualOutputKind>,
    ) -> Result<BrokerResponse, VirtualOutputError> {
        let id = self.next_request_id();
        self.write_request(id, command, controller_id, session_id, kind)?;
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|_| broker_fault("HIDMaestro broker response read failed"))?;
        if line.trim().is_empty() {
            return Err(broker_fault("HIDMaestro broker closed its response stream"));
        }
        let response: BrokerResponse = serde_json::from_str(&line)
            .map_err(|_| broker_fault("HIDMaestro broker returned invalid JSON"))?;
        if response.id != id {
            return Err(broker_fault(
                "HIDMaestro broker returned an out-of-order response",
            ));
        }
        Ok(response)
    }

    fn send_update(
        &mut self,
        session_id: &str,
        kind: VirtualOutputKind,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError> {
        let id = self.next_request_id();
        self.write_update_frame(id, session_id, kind, state)
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        id
    }

    fn write_request(
        &mut self,
        id: u64,
        command: &str,
        controller_id: Option<&str>,
        session_id: Option<&str>,
        kind: Option<VirtualOutputKind>,
    ) -> Result<(), VirtualOutputError> {
        let request = BrokerRequest {
            protocol: BROKER_PROTOCOL,
            id,
            command,
            controller_id,
            session_id,
            kind: kind.map(output_kind_wire),
        };
        serde_json::to_writer(&mut self.stdin, &request)
            .map_err(|_| broker_fault("HIDMaestro broker request serialization failed"))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|_| broker_fault("HIDMaestro broker request write failed"))
    }

    fn write_update_frame(
        &mut self,
        id: u64,
        session_id: &str,
        kind: VirtualOutputKind,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError> {
        let frame = BrokerUpdateFrame::from_state(id, session_id, kind, state);
        serde_json::to_writer(&mut self.stdin, &frame)
            .map_err(|_| broker_fault("HIDMaestro broker update serialization failed"))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|_| broker_fault("HIDMaestro broker update write failed"))
    }
}

impl Drop for BrokerProcess {
    fn drop(&mut self) {
        let _ = self.request("cleanup", None, None, None);
        let _ = self.request("shutdown", None, None, None);
        let _ = self.child.kill();
    }
}

fn ensure_broker_process<'a>(
    slot: &'a mut Option<BrokerProcess>,
    command: &BrokerCommand,
) -> Result<&'a mut BrokerProcess, VirtualOutputError> {
    let restart = slot
        .as_mut()
        .and_then(|process| process.child.try_wait().ok().flatten())
        .is_some();
    if restart {
        *slot = None;
    }
    if slot.is_none() {
        *slot = Some(spawn_broker_process(command)?);
    }
    slot.as_mut()
        .ok_or_else(|| broker_fault("HIDMaestro broker process is unavailable"))
}

fn spawn_broker_process(command: &BrokerCommand) -> Result<BrokerProcess, VirtualOutputError> {
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| broker_fault("HIDMaestro broker could not be started"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| broker_fault("HIDMaestro broker stdin was unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| broker_fault("HIDMaestro broker stdout was unavailable"))?;
    let mut process = BrokerProcess {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        next_id: 1,
    };
    let response = process.request("hello", None, None, None)?;
    if response.ok {
        Ok(process)
    } else {
        Err(broker_fault("HIDMaestro broker handshake failed"))
    }
}

fn discover_hidmaestro_broker() -> Option<BrokerCommand> {
    if let Some(value) = std::env::var_os(HIDMAESTRO_BROKER_ENV) {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(BrokerCommand {
                program: path,
                args: Vec::new(),
            });
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))?;
    for relative in [
        "dscc-hidmaestro-broker.exe",
        "hidmaestro/dscc-hidmaestro-broker.exe",
        "providers/hidmaestro/dscc-hidmaestro-broker.exe",
    ] {
        let path = exe_dir.join(relative);
        if path.is_file() {
            return Some(BrokerCommand {
                program: path,
                args: Vec::new(),
            });
        }
    }
    None
}

fn output_kind_wire(kind: VirtualOutputKind) -> String {
    output_kind_wire_static(kind).to_string()
}

fn output_kind_wire_static(kind: VirtualOutputKind) -> &'static str {
    match kind {
        VirtualOutputKind::Xbox360 => "xbox360",
    }
}

fn parse_output_kind(kind: &str) -> Option<VirtualOutputKind> {
    match kind {
        "xbox360" => Some(VirtualOutputKind::Xbox360),
        _ => None,
    }
}

fn broker_fault(message: &str) -> VirtualOutputError {
    VirtualOutputError::BackendFault(message.to_string())
}

impl<'a> BrokerUpdateFrame<'a> {
    fn from_state(
        id: u64,
        session_id: &'a str,
        kind: VirtualOutputKind,
        state: &VirtualGamepadState,
    ) -> Self {
        Self {
            protocol: BROKER_PROTOCOL,
            id,
            command: "update",
            session_id,
            kind: output_kind_wire_static(kind),
            lx: signed_axis_wire(state.left_stick.x),
            ly: signed_axis_wire(state.left_stick.y),
            rx: signed_axis_wire(state.right_stick.x),
            ry: signed_axis_wire(state.right_stick.y),
            lt: trigger_wire(state.triggers.l2),
            rt: trigger_wire(state.triggers.r2),
            buttons: button_mask(&state.buttons.buttons),
        }
    }
}

const SIGNED_AXIS_WIRE_MAX: f64 = 32767.0;
const TRIGGER_WIRE_MAX: f64 = 65535.0;

const BUTTON_A: u32 = 1 << 0;
const BUTTON_B: u32 = 1 << 1;
const BUTTON_X: u32 = 1 << 2;
const BUTTON_Y: u32 = 1 << 3;
const BUTTON_DPAD_UP: u32 = 1 << 4;
const BUTTON_DPAD_RIGHT: u32 = 1 << 5;
const BUTTON_DPAD_DOWN: u32 = 1 << 6;
const BUTTON_DPAD_LEFT: u32 = 1 << 7;
const BUTTON_LEFT_SHOULDER: u32 = 1 << 8;
const BUTTON_RIGHT_SHOULDER: u32 = 1 << 9;
const BUTTON_LEFT_THUMB: u32 = 1 << 10;
const BUTTON_RIGHT_THUMB: u32 = 1 << 11;
const BUTTON_BACK: u32 = 1 << 12;
const BUTTON_START: u32 = 1 << 13;
const BUTTON_GUIDE: u32 = 1 << 14;
const BUTTON_TOUCHPAD: u32 = 1 << 15;
const BUTTON_SHARE: u32 = 1 << 16;

fn signed_axis_wire(value: f64) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(-1.0, 1.0) * SIGNED_AXIS_WIRE_MAX).round() as i16
}

fn trigger_wire(value: f64) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * TRIGGER_WIRE_MAX).round() as u16
}

fn button_mask(buttons: &BTreeMap<String, bool>) -> u32 {
    buttons.iter().fold(0, |mask, (button, pressed)| {
        if *pressed {
            mask | button_bit(button)
        } else {
            mask
        }
    })
}

fn button_bit(button: &str) -> u32 {
    match button {
        "a" => BUTTON_A,
        "b" => BUTTON_B,
        "x" => BUTTON_X,
        "y" => BUTTON_Y,
        "dpad_up" => BUTTON_DPAD_UP,
        "dpad_right" => BUTTON_DPAD_RIGHT,
        "dpad_down" => BUTTON_DPAD_DOWN,
        "dpad_left" => BUTTON_DPAD_LEFT,
        "left_shoulder" => BUTTON_LEFT_SHOULDER,
        "right_shoulder" => BUTTON_RIGHT_SHOULDER,
        "left_thumb" => BUTTON_LEFT_THUMB,
        "right_thumb" => BUTTON_RIGHT_THUMB,
        "back" => BUTTON_BACK,
        "start" => BUTTON_START,
        "guide" => BUTTON_GUIDE,
        "touchpad" => BUTTON_TOUCHPAD,
        "share" => BUTTON_SHARE,
        _ => 0,
    }
}

#[cfg(any(test, debug_assertions))]
#[derive(Clone, Debug, Default)]
pub struct MockVirtualOutputBackend {
    inner: Arc<Mutex<MockVirtualOutputBackendInner>>,
}

#[cfg(any(test, debug_assertions))]
#[derive(Clone, Debug)]
struct MockVirtualOutputBackendInner {
    available: bool,
    message: String,
    sessions: BTreeMap<String, MockVirtualOutputSession>,
    next_session: u64,
}

#[cfg(any(test, debug_assertions))]
#[derive(Clone, Debug)]
struct MockVirtualOutputSession {
    states: Vec<VirtualGamepadState>,
}

#[cfg(any(test, debug_assertions))]
impl Default for MockVirtualOutputBackendInner {
    fn default() -> Self {
        Self {
            available: true,
            message: "Mock virtual output backend is available".to_string(),
            sessions: BTreeMap::new(),
            next_session: 1,
        }
    }
}

#[cfg(any(test, debug_assertions))]
impl MockVirtualOutputBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        let backend = Self::new();
        {
            let mut inner = backend.lock();
            inner.available = false;
            inner.message = message.into();
        }
        backend
    }

    pub fn submitted_states(&self, session_id: &str) -> Vec<VirtualGamepadState> {
        self.lock()
            .sessions
            .get(session_id)
            .map(|session| session.states.clone())
            .unwrap_or_default()
    }

    fn lock(&self) -> MutexGuard<'_, MockVirtualOutputBackendInner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(any(test, debug_assertions))]
impl VirtualOutputBackend for MockVirtualOutputBackend {
    fn status(&self) -> VirtualOutputBackendStatus {
        let inner = self.lock();
        VirtualOutputBackendStatus {
            backend_id: "mock".to_string(),
            state: if inner.available {
                VirtualOutputBackendState::Available
            } else {
                VirtualOutputBackendState::Unavailable
            },
            message: inner.message.clone(),
            supported_kinds: if inner.available {
                vec![VirtualOutputKind::Xbox360]
            } else {
                Vec::new()
            },
        }
    }

    fn create_session(
        &self,
        controller_id: &str,
        kind: VirtualOutputKind,
    ) -> Result<VirtualOutputTarget, VirtualOutputError> {
        let mut inner = self.lock();
        if !inner.available {
            return Err(VirtualOutputError::Unavailable(inner.message.clone()));
        }
        let session_id = format!("{controller_id}-virtual-{}", inner.next_session);
        inner.next_session = inner.next_session.saturating_add(1);
        let target = VirtualOutputTarget { session_id, kind };
        inner.sessions.insert(
            target.session_id.clone(),
            MockVirtualOutputSession {
                states: vec![VirtualGamepadState::neutral()],
            },
        );
        Ok(target)
    }

    fn submit_state(
        &self,
        target: &VirtualOutputTarget,
        state: &VirtualGamepadState,
    ) -> Result<(), VirtualOutputError> {
        let mut inner = self.lock();
        let Some(session) = inner.sessions.get_mut(&target.session_id) else {
            return Err(VirtualOutputError::SessionNotFound(
                target.session_id.clone(),
            ));
        };
        session.states.push(state.clone());
        Ok(())
    }

    fn drop_session(&self, target: &VirtualOutputTarget) -> Result<(), VirtualOutputError> {
        let mut inner = self.lock();
        let Some(mut session) = inner.sessions.remove(&target.session_id) else {
            return Err(VirtualOutputError::SessionNotFound(
                target.session_id.clone(),
            ));
        };
        session.states.push(VirtualGamepadState::neutral());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_backend_records_state_and_neutralizes() {
        let backend = MockVirtualOutputBackend::new();
        let target = backend
            .create_session("controller-1", VirtualOutputKind::Xbox360)
            .unwrap();
        let mut state = VirtualGamepadState::neutral();
        state.triggers.r2 = 1.0;
        backend.submit_state(&target, &state).unwrap();
        assert_eq!(backend.submitted_states(&target.session_id).len(), 2);
        backend.reset(&target).unwrap();
        assert_eq!(
            backend
                .submitted_states(&target.session_id)
                .last()
                .unwrap()
                .triggers
                .r2,
            0.0
        );
        backend.drop_session(&target).unwrap();
    }

    #[test]
    fn unavailable_backend_refuses_sessions() {
        let backend = MockVirtualOutputBackend::unavailable("provider missing");
        assert!(matches!(
            backend.create_session("controller-1", VirtualOutputKind::Xbox360),
            Err(VirtualOutputError::Unavailable(_))
        ));
    }

    #[test]
    fn hidmaestro_backend_fails_closed_without_broker() {
        let backend = HidMaestroBrokerBackend::unavailable();
        let status = backend.status();

        assert_eq!(status.backend_id, "hidmaestro");
        assert_eq!(status.state, VirtualOutputBackendState::Unavailable);
        assert!(status.supported_kinds.is_empty());
        assert!(matches!(
            backend.create_session("controller-1", VirtualOutputKind::Xbox360),
            Err(VirtualOutputError::Unavailable(_))
        ));
    }

    #[test]
    fn hidmaestro_update_frame_is_compact_and_clamped() {
        let mut state = VirtualGamepadState::neutral();
        state.left_stick.x = 2.0;
        state.left_stick.y = -2.0;
        state.right_stick.x = 0.5;
        state.right_stick.y = f64::NAN;
        state.triggers.l2 = -1.0;
        state.triggers.r2 = 1.5;
        state.buttons.buttons.insert("a".to_string(), true);
        state.buttons.buttons.insert("b".to_string(), false);
        state.buttons.buttons.insert("dpad_right".to_string(), true);
        state.buttons.buttons.insert("unknown".to_string(), true);

        let frame =
            BrokerUpdateFrame::from_state(42, "session-1", VirtualOutputKind::Xbox360, &state);

        assert_eq!(frame.lx, 32767);
        assert_eq!(frame.ly, -32767);
        assert_eq!(frame.rx, 16384);
        assert_eq!(frame.ry, 0);
        assert_eq!(frame.lt, 0);
        assert_eq!(frame.rt, 65535);
        assert_eq!(frame.buttons, BUTTON_A | BUTTON_DPAD_RIGHT);

        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"command\":\"update\""));
        assert!(json.contains("\"sessionId\":\"session-1\""));
        assert!(json.contains("\"buttons\":33"));
        assert!(!json.contains("state"));
        assert!(!json.contains("leftStick"));
    }
}
