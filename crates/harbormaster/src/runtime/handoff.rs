//! Private one-use launch metadata. Contains no prompt or credential material.
use super::{ManagedRun, private_directory};
use crate::{
    manager::ManagerError,
    projects::{PreparedLaunch, Preset, Project, Task},
    protocol::LocalPath,
};
use rustix::fs::{self, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    os::unix::process::CommandExt,
    path::Path,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    project: Project,
    preset: Preset,
    task: Task,
    home: LocalPath,
}

pub(super) fn create(
    run: &ManagedRun,
    project: Project,
    preset: Preset,
    task: Task,
    home: LocalPath,
) -> Result<(), ManagerError> {
    let directory = run.directory();
    private_directory(directory.parent().ok_or(ManagerError::UnsafePath)?, true)?;
    std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let fd = private_directory(&directory, false)?;
    let data = serde_json::to_vec(&Launch {
        project,
        preset,
        task,
        home,
    })
    .map_err(|_| ManagerError::InvalidRequest)?;
    if data.len() > 32768 {
        return Err(ManagerError::InvalidRequest);
    }
    let file = fs::openat(
        &fd,
        "launch.json",
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| ManagerError::UnsafePath)?;
    let mut file = std::fs::File::from(file);
    file.write_all(&data)?;
    file.sync_all()?;
    Ok(())
}

/// Entered only as the fixed tmux pane command; all metadata is revalidated.
pub(crate) fn worker(directory: &Path) -> Result<(), ManagerError> {
    let fd = private_directory(directory, false)?;
    let launch = load(&fd)?;
    let cwd = crate::projects::paths::open_identity(
        Path::new(launch.project.root.as_str()),
        false,
        launch.project.identity,
    )?;
    let executable = crate::projects::paths::open_identity(
        Path::new(launch.preset.executable.as_str()),
        true,
        launch.preset.identity,
    )?;
    let prepared = PreparedLaunch::new(launch.project, launch.preset, launch.task)?;
    let parent = BTreeMap::from([
        ("HOME".to_owned(), launch.home.as_str().to_owned()),
        ("TERM".to_owned(), "tmux-256color".to_owned()),
    ]);
    let configured = prepared.command(&parent)?;
    let mut command = std::process::Command::new(format!(
        "/proc/self/fd/{}",
        std::os::fd::AsRawFd::as_raw_fd(&executable)
    ));
    command.args(configured.get_args()).env_clear();
    for (key, value) in configured.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    rustix::process::fchdir(&cwd).map_err(|_| ManagerError::UnsafePath)?;
    // Scripts need the pinned descriptor across the interpreter exec. No raw FFI.
    rustix::io::fcntl_setfd(&executable, rustix::io::FdFlags::empty())
        .map_err(|_| ManagerError::UnsafePath)?;
    fs::unlinkat(&fd, "launch.json", fs::AtFlags::empty()).map_err(|_| ManagerError::UnsafePath)?;
    Err(command.exec().into())
}

use std::os::unix::fs::DirBuilderExt;

fn load(fd: &std::os::fd::OwnedFd) -> Result<Launch, ManagerError> {
    let file = fs::openat(
        fd,
        "launch.json",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| ManagerError::UnsafePath)?;
    let stat = fs::fstat(&file).map_err(|_| ManagerError::UnsafePath)?;
    if stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o7777 != 0o600
        || fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::RegularFile
        || stat.st_nlink != 1
    {
        return Err(ManagerError::UnsafePath);
    }
    let mut data = Vec::new();
    std::fs::File::from(file)
        .take(32769)
        .read_to_end(&mut data)?;
    if data.len() > 32768 {
        return Err(ManagerError::InvalidRequest);
    }
    serde_json::from_slice(&data).map_err(|_| ManagerError::InvalidRequest)
}
