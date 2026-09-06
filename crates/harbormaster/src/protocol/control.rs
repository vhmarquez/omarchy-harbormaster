//! Control data validation. None of these records authorize or execute operations.

use serde::{Deserialize, Serialize};

use super::{
    HarnessKind, LocalPath, ProjectId, ProtocolError, RequestId, Revision, RunId, Seq,
    SnapshotCursor, TaskLabel, Version, strict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Operation {
    #[serde(rename = "snapshot")]
    Snapshot,
    #[serde(rename = "subscribe")]
    Subscribe,
    #[serde(rename = "project.register")]
    ProjectRegister,
    #[serde(rename = "launch")]
    Launch,
    #[serde(rename = "focus")]
    Focus,
    #[serde(rename = "attach")]
    Attach,
    #[serde(rename = "resume")]
    Resume,
    #[serde(rename = "interrupt_owned")]
    InterruptOwned,
    #[serde(rename = "end_owned")]
    EndOwned,
    #[serde(rename = "review.mark")]
    ReviewMark,
    #[serde(rename = "attention.snooze")]
    AttentionSnooze,
    #[serde(rename = "policy.update")]
    PolicyUpdate,
}

impl Operation {
    pub const ALL: [Self; 12] = [
        Self::Snapshot,
        Self::Subscribe,
        Self::ProjectRegister,
        Self::Launch,
        Self::Focus,
        Self::Attach,
        Self::Resume,
        Self::InterruptOwned,
        Self::EndOwned,
        Self::ReviewMark,
        Self::AttentionSnooze,
        Self::PolicyUpdate,
    ];
    #[must_use]
    pub const fn is_stateful(self) -> bool {
        !matches!(self, Self::Snapshot | Self::Subscribe)
    }
}

/// Snapshot page size restricted to 1–100 records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct PageSize(u16);
impl TryFrom<u16> for PageSize {
    type Error = ProtocolError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ProtocolError::InvalidFrame)
        }
    }
}
impl From<PageSize> for u16 {
    fn from(value: PageSize) -> Self {
        value.0
    }
}
impl PageSize {
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotArguments {
    pub limit: PageSize,
    pub cursor: Option<SnapshotCursor>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeArguments {
    pub after_revision: Revision,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRegisterArguments {
    pub path: LocalPath,
    pub label: TaskLabel,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchArguments {
    pub project_id: ProjectId,
    pub harness: HarnessKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetArguments {
    pub run_id: RunId,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewMarkArguments {
    pub run_id: RunId,
    pub outcome_revision: Revision,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttentionSnoozeArguments {
    pub run_id: RunId,
    pub until_unix_seconds: Seq,
}
/// Only notification enablement is represented. Retention policy remains out of scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyUpdateArguments {
    pub notifications_enabled: bool,
}

/// Stateful variants always carry a revision guard; no native approval exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub enum ControlOperation {
    #[serde(rename = "snapshot")]
    Snapshot { arguments: SnapshotArguments },
    #[serde(rename = "subscribe")]
    Subscribe { arguments: SubscribeArguments },
    #[serde(rename = "project.register")]
    ProjectRegister {
        expected_revision: Revision,
        arguments: ProjectRegisterArguments,
    },
    #[serde(rename = "launch")]
    Launch {
        expected_revision: Revision,
        arguments: LaunchArguments,
    },
    #[serde(rename = "focus")]
    Focus {
        expected_revision: Revision,
        arguments: TargetArguments,
    },
    #[serde(rename = "attach")]
    Attach {
        expected_revision: Revision,
        arguments: TargetArguments,
    },
    #[serde(rename = "resume")]
    Resume {
        expected_revision: Revision,
        arguments: TargetArguments,
    },
    #[serde(rename = "interrupt_owned")]
    InterruptOwned {
        expected_revision: Revision,
        arguments: TargetArguments,
    },
    #[serde(rename = "end_owned")]
    EndOwned {
        expected_revision: Revision,
        arguments: TargetArguments,
    },
    #[serde(rename = "review.mark")]
    ReviewMark {
        expected_revision: Revision,
        arguments: ReviewMarkArguments,
    },
    #[serde(rename = "attention.snooze")]
    AttentionSnooze {
        expected_revision: Revision,
        arguments: AttentionSnoozeArguments,
    },
    #[serde(rename = "policy.update")]
    PolicyUpdate {
        expected_revision: Revision,
        arguments: PolicyUpdateArguments,
    },
}

impl ControlOperation {
    #[must_use]
    pub const fn operation(&self) -> Operation {
        match self {
            Self::Snapshot { .. } => Operation::Snapshot,
            Self::Subscribe { .. } => Operation::Subscribe,
            Self::ProjectRegister { .. } => Operation::ProjectRegister,
            Self::Launch { .. } => Operation::Launch,
            Self::Focus { .. } => Operation::Focus,
            Self::Attach { .. } => Operation::Attach,
            Self::Resume { .. } => Operation::Resume,
            Self::InterruptOwned { .. } => Operation::InterruptOwned,
            Self::EndOwned { .. } => Operation::EndOwned,
            Self::ReviewMark { .. } => Operation::ReviewMark,
            Self::AttentionSnooze { .. } => Operation::AttentionSnooze,
            Self::PolicyUpdate { .. } => Operation::PolicyUpdate,
        }
    }
    #[must_use]
    pub const fn expected_revision(&self) -> Option<Revision> {
        match self {
            Self::Snapshot { .. } | Self::Subscribe { .. } => None,
            Self::ProjectRegister {
                expected_revision, ..
            }
            | Self::Launch {
                expected_revision, ..
            }
            | Self::Focus {
                expected_revision, ..
            }
            | Self::Attach {
                expected_revision, ..
            }
            | Self::Resume {
                expected_revision, ..
            }
            | Self::InterruptOwned {
                expected_revision, ..
            }
            | Self::EndOwned {
                expected_revision, ..
            }
            | Self::ReviewMark {
                expected_revision, ..
            }
            | Self::AttentionSnooze {
                expected_revision, ..
            }
            | Self::PolicyUpdate {
                expected_revision, ..
            } => Some(*expected_revision),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(super) enum ControlChannel {
    #[serde(rename = "control")]
    Control,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRequest {
    pub request_id: RequestId,
    pub request: ControlOperation,
}
impl ControlRequest {
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.request.operation()
    }
    #[must_use]
    pub const fn expected_revision(&self) -> Option<Revision> {
        self.request.expected_revision()
    }
}

impl Serialize for ControlRequest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            protocol: Version,
            channel: ControlChannel,
            request_id: &'a RequestId,
            #[serde(flatten)]
            request: &'a ControlOperation,
        }
        Wire {
            protocol: Version,
            channel: ControlChannel::Control,
            request_id: &self.request_id,
            request: &self.request,
        }
        .serialize(serializer)
    }
}

/// Parse a control request, without granting execution or ownership privileges.
/// # Errors
/// Rejects malformed fields, missing stateful guards and unsupported operations.
pub fn parse_control(frame: &[u8]) -> Result<ControlRequest, ProtocolError> {
    let mut value = strict::decode(frame)?;
    let object = value.as_object_mut().ok_or(ProtocolError::InvalidFrame)?;
    let _: Version = strict::typed(
        object
            .remove("protocol")
            .ok_or(ProtocolError::InvalidFrame)?,
    )?;
    let _: ControlChannel = strict::typed(
        object
            .remove("channel")
            .ok_or(ProtocolError::InvalidFrame)?,
    )?;
    let request_id = strict::typed(
        object
            .remove("request_id")
            .ok_or(ProtocolError::InvalidFrame)?,
    )?;
    Ok(ControlRequest {
        request_id,
        request: strict::typed(value)?,
    })
}
