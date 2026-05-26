use std::collections::BTreeMap;
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

#[derive(Clone, Debug, Default)]
pub struct MockVirtualOutputBackend {
    inner: Arc<Mutex<MockVirtualOutputBackendInner>>,
}

#[derive(Clone, Debug)]
struct MockVirtualOutputBackendInner {
    available: bool,
    message: String,
    sessions: BTreeMap<String, MockVirtualOutputSession>,
    next_session: u64,
}

#[derive(Clone, Debug)]
struct MockVirtualOutputSession {
    states: Vec<VirtualGamepadState>,
}

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
}
