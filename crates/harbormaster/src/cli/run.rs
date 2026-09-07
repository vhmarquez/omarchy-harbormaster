use crate::{
    projects::{CatalogRequest, CatalogResponse, PreparedLaunch, ProjectError, paths},
    storage::{DatabaseWorker, ReceiptError, Request, Response, StorageError},
};
use std::{path::PathBuf, time::Duration};

#[derive(Debug)]
pub enum OperationError {
    StateHome,
    Storage(StorageError),
    Project(ProjectError),
    Unavailable,
    UnknownOutcome,
    Manager(crate::manager::ManagerError),
}
impl std::fmt::Display for OperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manager(error) => error.fmt(f),
            Self::StateHome => f.write_str(
                "state location must be absolute, owned and not writable by other users",
            ),
            Self::Storage(error) => error.fmt(f),
            Self::Project(error) => error.fmt(f),
            Self::Unavailable => f.write_str("database command unavailable"),
            Self::UnknownOutcome => f.write_str(
                "database reply unavailable; operation may have committed; list before retrying",
            ),
        }
    }
}
impl std::error::Error for OperationError {}

/// Execute one explicit registry operation, without starting any harness.
/// # Errors
/// Sanitized filesystem/storage/validation failures or an unknown worker outcome.
pub fn execute(request: CatalogRequest) -> Result<Vec<u8>, OperationError> {
    if let Some(output) =
        crate::manager::client::catalog(request.clone()).map_err(OperationError::Manager)?
    {
        return Ok(output);
    }
    let root = state_home()?;
    paths::directory(&root, true).map_err(|_| OperationError::StateHome)?;
    let worker = DatabaseWorker::open(&root).map_err(OperationError::Storage)?;
    let mut ticket = worker
        .try_submit(Request::Catalog(Box::new(request)))
        .map_err(|_| OperationError::Unavailable)?;
    let reply = ticket
        .wait_timeout(Duration::from_secs(5))
        .map_err(|error| match error {
            ReceiptError::Storage(error) => OperationError::Storage(error),
            _ => OperationError::UnknownOutcome,
        })?;
    let Response::Catalog(reply) = reply else {
        return Err(OperationError::Unavailable);
    };
    let encoded = match *reply {
        CatalogResponse::Launch {
            project,
            preset,
            task,
        } => {
            let plan =
                PreparedLaunch::new(project, preset, task).map_err(OperationError::Project)?;
            serde_json::to_vec_pretty(&plan.preview())
        }
        reply => serde_json::to_vec_pretty(&reply),
    };
    let mut output = encoded.map_err(|_| OperationError::Unavailable)?;
    output.push(b'\n');
    Ok(output)
}

pub(crate) fn state_home() -> Result<PathBuf, OperationError> {
    let path = if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or(OperationError::StateHome)?)
            .join(".local/state")
    };
    if !path.is_absolute() {
        return Err(OperationError::StateHome);
    }
    Ok(path)
}
