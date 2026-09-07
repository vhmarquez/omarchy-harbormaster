//! Independent, explicitly owned terminal runtimes. No harness content capture.
mod association;
mod command;
pub(crate) mod control;
mod desktop;
mod handoff;
mod ownership;
mod process;
mod service;
mod terminal;
#[cfg(test)]
mod tests;

use crate::protocol::{LocalPath, ProjectId, RunId, TaskId};
pub use control::{
    ControlRequest, ControlResponse, ControlState, PaneIdentity, TerminalIntent, WindowIdentity,
};
pub(crate) use handoff::worker;
pub use process::ProcessIdentity;
use serde::{Deserialize, Serialize};
pub(crate) use service::Backend;

pub const MAX_MANAGED_RUNS: usize = 1_000;
pub const RUN_PAGE_SIZE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRun {
    pub id: RunId,
    pub task_id: TaskId,
    pub project_id: ProjectId,
    pub runtime_root: LocalPath,
    pub server: Option<ProcessIdentity>,
}

impl ManagedRun {
    #[must_use]
    pub fn unit(&self) -> String {
        format!("harbormaster-runtime-{}.service", self.id.as_str())
    }

    #[must_use]
    pub fn directory(&self) -> std::path::PathBuf {
        std::path::Path::new(self.runtime_root.as_str())
            .join("harbormaster/runners")
            .join(self.id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Uncertain,
    Running,
    Exited,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunView {
    pub run: ManagedRun,
    pub unit: String,
    pub state: RuntimeState,
    /// No Hermes observer exists until M3; runtime liveness is not task progress.
    pub limited_visibility: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerRequest {
    Get {
        id: RunId,
    },
    ByTask {
        task_id: TaskId,
    },
    Reserve {
        task_id: TaskId,
        runtime_root: LocalPath,
    },
    Identify {
        id: RunId,
        server: ProcessIdentity,
    },
    List {
        project_id: ProjectId,
        after: Option<RunId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerResponse {
    Found(Option<ManagedRun>),
    Reserved { run: ManagedRun, created: bool },
    Identified(ManagedRun),
    Listed(crate::projects::Page<ManagedRun>),
}

pub(crate) fn private_directory(
    path: &std::path::Path,
    create: bool,
) -> Result<std::os::fd::OwnedFd, crate::manager::ManagerError> {
    if path
        .components()
        .collect::<std::path::PathBuf>()
        .as_os_str()
        != path.as_os_str()
        || path.to_str().is_none()
    {
        return Err(crate::manager::ManagerError::UnsafePath);
    }
    let fd = crate::projects::paths::directory(path, create)?;
    let stat = rustix::fs::fstat(&fd).map_err(|_| crate::manager::ManagerError::UnsafePath)?;
    if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o7777 != 0o700 {
        return Err(crate::manager::ManagerError::UnsafePath);
    }
    Ok(fd)
}
