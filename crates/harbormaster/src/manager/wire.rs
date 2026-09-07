use super::ManagerError;
use crate::{
    projects::CatalogRequest,
    protocol::{ProjectId, RequestId, Revision, RunId, TaskId, strict},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogoutPolicy {
    ExistingUserManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum Command {
    Status,
    Stop,
    Catalog {
        request: CatalogRequest,
    },
    Launch {
        task_id: TaskId,
        logout_policy: LogoutPolicy,
        #[serde(default)]
        allow_shared_checkout: bool,
    },
    RunAction {
        run_id: RunId,
        action: RunAction,
    },
    Runs {
        project_id: ProjectId,
        after: Option<RunId>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunAction {
    Actions,
    Open,
    Attach,
    Resume,
    End,
}
impl Command {
    pub(crate) fn stateful(&self) -> bool {
        match self {
            Self::Status | Self::Runs { .. } => false,
            Self::RunAction { action, .. } => !matches!(action, RunAction::Actions),
            Self::Catalog { request } => !request.is_read(),
            Self::Stop | Self::Launch { .. } => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlCall {
    pub protocol: u8,
    pub channel: String,
    pub request_id: RequestId,
    pub expected_revision: Option<Revision>,
    pub command: Command,
}
impl ControlCall {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ManagerError> {
        let value = strict::decode(bytes).map_err(|_| ManagerError::InvalidRequest)?;
        let request: Self = strict::typed(value).map_err(|_| ManagerError::InvalidRequest)?;
        if request.protocol != 0
            || request.channel != "control"
            || (request.command.stateful() && request.expected_revision.is_none())
        {
            return Err(ManagerError::InvalidRequest);
        }
        if let Command::Catalog { request: catalog } = &request.command {
            catalog
                .validate()
                .map_err(|_| ManagerError::InvalidRequest)?;
        }
        Ok(request)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum Reply {
    Ok {
        protocol: u8,
        request_id: RequestId,
        revision: Revision,
        value: serde_json::Value,
    },
    Error {
        protocol: u8,
        request_id: RequestId,
        error: ManagerError,
    },
}
