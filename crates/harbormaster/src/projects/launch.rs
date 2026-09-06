use super::{Preset, Project, ProjectError, Task, name_valid, paths};
use crate::protocol::{LocalPath, TaskId};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, process::Command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchPreview {
    pub task_id: TaskId,
    pub executable: LocalPath,
    pub cwd: LocalPath,
    pub argv: Vec<String>,
}

/// A preparation result, not process ownership or a race-free spawn authority.
/// The runtime must hold/revalidate identities at its actual spawn boundary.
pub struct PreparedLaunch {
    project: Project,
    preset: Preset,
    task: Task,
}

impl PreparedLaunch {
    /// # Errors
    /// Rejects unrelated records, invalid names and changed or unsafe paths.
    pub fn new(project: Project, preset: Preset, task: Task) -> Result<Self, ProjectError> {
        if project.id != task.project_id || preset.name != task.preset {
            return Err(ProjectError::InvalidInput);
        }
        name_valid(&preset.name)?;
        name_valid(&preset.profile)?;
        let prepared = Self {
            project,
            preset,
            task,
        };
        prepared.revalidate()?;
        Ok(prepared)
    }

    fn revalidate(&self) -> Result<(), ProjectError> {
        if paths::identity(Path::new(self.project.root.as_str()), false)? != self.project.identity
            || paths::identity(Path::new(self.preset.executable.as_str()), true)?
                != self.preset.identity
        {
            return Err(ProjectError::IdentityChanged);
        }
        Ok(())
    }

    #[must_use]
    pub fn preview(&self) -> LaunchPreview {
        LaunchPreview {
            task_id: self.task.id.clone(),
            executable: self.preset.executable.clone(),
            cwd: self.project.root.clone(),
            argv: vec!["-p".into(), self.preset.profile.clone(), "chat".into()],
        }
    }

    /// Prepare an unstarted process with an explicit minimal interactive environment.
    /// Native Hermes retains its own profile/config authentication; manager state
    /// never stores environment values. Runtime/desktop additions belong to #12.
    /// # Errors
    /// Rejects changed identities or absent/invalid explicit HOME.
    pub fn command(&self, parent: &BTreeMap<String, String>) -> Result<Command, ProjectError> {
        self.revalidate()?;
        let home = parent
            .get("HOME")
            .filter(|v| v.starts_with('/') && v.len() <= 4096 && !v.chars().any(char::is_control))
            .ok_or(ProjectError::InvalidInput)?;
        let mut command = Command::new(self.preset.executable.as_str());
        command
            .args(self.preview().argv)
            .current_dir(self.project.root.as_str())
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/local/bin:/usr/bin:/bin");
        for key in ["LANG", "LC_ALL", "TERM", "COLORTERM"] {
            if let Some(value) = parent.get(key).filter(|v| {
                v.len() <= 128
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.@".contains(&b))
            }) {
                command.env(key, value);
            }
        }
        Ok(command)
    }
}
