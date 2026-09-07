//! Explicit user metadata and launch preparation; no repository configuration.
mod launch;
pub(crate) mod paths;

use crate::protocol::{LocalPath, ProjectId, Revision, TaskId, TaskLabel};
pub use launch::{LaunchPreview, PreparedLaunch};
use serde::{Deserialize, Serialize};

pub const MAX_PROJECTS: usize = 1_000;
pub const MAX_PRESETS: usize = 100;
pub const MAX_TASKS: usize = 10_000;
pub const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectError {
    InvalidInput,
    UnsafePath,
    IdentityChanged,
}
impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid project or preset input",
            Self::UnsafePath => "path must be absolute, owned, accessible and free of symlinks",
            Self::IdentityChanged => "saved path identity changed; registration needs review",
        })
    }
}
impl std::error::Error for ProjectError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: ProjectId,
    pub label: TaskLabel,
    pub root: LocalPath,
    pub identity: FileIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    pub executable: LocalPath,
    pub identity: FileIdentity,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub preset: String,
    pub label: TaskLabel,
}

/// Each page reports its revision; restart pagination if that revision changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Page<T> {
    pub revision: Revision,
    pub items: Vec<T>,
    pub next_after: Option<String>,
}

/// Fixed trusted-manager operations, never interpreted from a producer event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields, rename_all = "snake_case")]
pub enum CatalogRequest {
    AddProject {
        root: LocalPath,
        label: TaskLabel,
    },
    AddPreset {
        name: String,
        executable: LocalPath,
        profile: String,
    },
    AddTask {
        project_id: ProjectId,
        preset: String,
        label: TaskLabel,
    },
    Projects {
        after: Option<ProjectId>,
    },
    Presets {
        after: Option<String>,
    },
    Tasks {
        project_id: ProjectId,
        after: Option<TaskId>,
    },
    Task {
        id: TaskId,
    },
}

impl CatalogRequest {
    pub(crate) fn validate(&self) -> Result<(), ProjectError> {
        match self {
            Self::AddPreset { name, profile, .. } => {
                name_valid(name)?;
                name_valid(profile)
            }
            Self::AddTask { preset, .. } => name_valid(preset),
            Self::Presets { after: Some(name) } => name_valid(name),
            _ => Ok(()),
        }
    }

    pub(crate) const fn is_read(&self) -> bool {
        matches!(
            self,
            Self::Projects { .. } | Self::Presets { .. } | Self::Tasks { .. } | Self::Task { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CatalogResponse {
    Project(Project),
    Preset(Preset),
    Task(Task),
    Projects(Page<Project>),
    Presets(Page<Preset>),
    Tasks(Page<Task>),
    Launch {
        project: Project,
        preset: Preset,
        task: Task,
    },
}

pub(crate) fn name_valid(value: &str) -> Result<(), ProjectError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || value.starts_with('-')
    {
        Err(ProjectError::InvalidInput)
    } else {
        Ok(())
    }
}
