//! Runnable local control service; producer events cannot dispatch control work.
pub(crate) mod client;
mod controls;
mod launch;
mod operations;
mod server;
mod wire;

use serde::{Deserialize, Serialize};
pub use server::serve;
use std::path::PathBuf;
pub(crate) use wire::{Command, ControlCall as Request, LogoutPolicy, Reply, RunAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagerError {
    InvalidRequest,
    UnsafePath,
    Unavailable,
    Conflict,
    Busy,
    PersistenceUnavailable,
    RuntimeUnavailable,
    UnknownOutcome,
    OwnershipUnverified,
    ActionUnavailable,
    ConcurrentWriter,
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OwnershipUnverified => "runtime ownership cannot be verified; no control performed",
            Self::ActionUnavailable => "action unavailable for this runtime or supported desktop",
            Self::ConcurrentWriter => "another managed attempt may write this project root; inspect it or repeat with --allow-shared-checkout",
            Self::InvalidRequest => "invalid manager command",
            Self::UnsafePath => "unsafe manager or runtime path",
            Self::Unavailable => "manager unavailable; start manager serve",
            Self::Conflict => "state changed; list before retrying",
            Self::Busy => "manager capacity reached; retry later",
            Self::PersistenceUnavailable => "manager persistence unavailable",
            Self::RuntimeUnavailable => {
                "terminal runtime unavailable; inspect supported prerequisites"
            }
            Self::UnknownOutcome => {
                "reply unavailable; operation may have committed or started; list before retrying"
            }
        })
    }
}
impl std::error::Error for ManagerError {}
impl From<std::io::Error> for ManagerError {
    fn from(_: std::io::Error) -> Self {
        Self::RuntimeUnavailable
    }
}
impl From<crate::ipc::IpcError> for ManagerError {
    fn from(_: crate::ipc::IpcError) -> Self {
        Self::Unavailable
    }
}
impl From<crate::projects::ProjectError> for ManagerError {
    fn from(_: crate::projects::ProjectError) -> Self {
        Self::UnsafePath
    }
}
impl From<crate::storage::StorageError> for ManagerError {
    fn from(error: crate::storage::StorageError) -> Self {
        use crate::storage::StorageError;
        match error {
            StorageError::Conflict => Self::Conflict,
            StorageError::InvalidRequest => Self::InvalidRequest,
            StorageError::UnsafePath => Self::UnsafePath,
            StorageError::ResourceExhausted => Self::Busy,
            _ => Self::PersistenceUnavailable,
        }
    }
}

pub(crate) fn runtime_root() -> Result<PathBuf, ManagerError> {
    let root = std::env::var_os("HARBORMASTER_RUNTIME_DIR")
        .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
        .map(PathBuf::from)
        .ok_or(ManagerError::UnsafePath)?;
    crate::runtime::private_directory(&root, false)?;
    Ok(root)
}
