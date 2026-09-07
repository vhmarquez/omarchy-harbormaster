//! Persisted intent and observed ownership, separate from harness lifecycle facts.
use super::ProcessIdentity;
use crate::{manager::ManagerError, protocol::RunId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlState {
    pub id: RunId,
    pub launch_nonce: Option<RunId>,
    pub invocation: Option<String>,
    pub pane: Option<PaneIdentity>,
    pub terminal: Option<TerminalIntent>,
    pub stopped: bool,
    pub ending: bool,
}

impl ControlState {
    #[must_use]
    pub fn new(id: RunId, launch_nonce: Option<RunId>) -> Self {
        Self {
            id,
            launch_nonce,
            invocation: None,
            pane: None,
            terminal: None,
            stopped: false,
            ending: false,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ManagerError> {
        let bytes = serde_json::to_vec(self).map_err(|_| ManagerError::InvalidRequest)?;
        if bytes.len() > 8192 || self.invocation.as_ref().is_some_and(|s| !hex(s, 32)) {
            return Err(ManagerError::InvalidRequest);
        }
        if self.pane.is_some() && self.invocation.is_none()
            || self.terminal.is_some() && self.pane.is_none()
            || self.stopped && !self.ending
        {
            return Err(ManagerError::InvalidRequest);
        }
        if let Some(pane) = &self.pane {
            pane.process.validate()?;
            if !target(&pane.pane, '%') || !target(&pane.session, '$') {
                return Err(ManagerError::InvalidRequest);
            }
        }
        if let Some(terminal) = &self.terminal {
            terminal.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneIdentity {
    pub process: ProcessIdentity,
    pub pane: String,
    pub session: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalIntent {
    pub nonce: RunId,
    pub process: Option<ProcessIdentity>,
    pub window: Option<WindowIdentity>,
}
impl TerminalIntent {
    pub(crate) fn unit(&self) -> String {
        format!("harbormaster-terminal-{}.service", self.nonce.as_str())
    }
    pub(crate) fn app_id(&self) -> String {
        format!("harbormaster.{}", self.nonce.as_str())
    }
    fn validate(&self) -> Result<(), ManagerError> {
        if self.window.is_some() && self.process.is_none() {
            return Err(ManagerError::InvalidRequest);
        }
        if let Some(process) = &self.process {
            process.validate()?;
        }
        if let Some(window) = &self.window {
            window.client.validate()?;
            if !address(&window.address)
                || !component(&window.compositor)
                || !window.tty.strip_prefix("/dev/pts/").is_some_and(decimal)
            {
                return Err(ManagerError::InvalidRequest);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowIdentity {
    pub address: String,
    pub compositor: String,
    pub client: ProcessIdentity,
    pub tty: String,
}

pub(crate) fn target(value: &str, prefix: char) -> bool {
    value.strip_prefix(prefix).is_some_and(decimal)
}
fn decimal(value: &str) -> bool {
    !value.is_empty() && value.len() <= 10 && value.bytes().all(|b| b.is_ascii_digit())
}
pub(crate) fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn address(value: &str) -> bool {
    value
        .strip_prefix("0x")
        .is_some_and(|s| !s.is_empty() && s.len() <= 16 && hex(s, s.len()))
}
pub(crate) fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlRequest {
    Get(RunId),
    Save(Box<ControlState>),
    OtherWriters {
        project_id: crate::protocol::ProjectId,
        task_id: crate::protocol::TaskId,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlResponse {
    State(Option<Box<ControlState>>),
    Saved,
    OtherWriters(bool),
}
