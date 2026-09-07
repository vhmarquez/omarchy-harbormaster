//! Registration observes owned path identities; it does not execute their contents.
use super::{FileIdentity, ProjectError};
use rustix::fs::{self, FileType, Mode, OFlags, Stat};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

fn safe(stat: &Stat) -> Result<(), ProjectError> {
    let uid = rustix::process::geteuid().as_raw();
    if (stat.st_uid != uid && stat.st_uid != 0) || stat.st_mode & 0o022 != 0 {
        Err(ProjectError::UnsafePath)
    } else {
        Ok(())
    }
}

pub(crate) fn directory(path: &Path, create: bool) -> Result<OwnedFd, ProjectError> {
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(ProjectError::UnsafePath);
    }
    let mut fd = fs::open("/", DIRECTORY, Mode::empty()).map_err(|_| ProjectError::UnsafePath)?;
    for part in path.components() {
        match part {
            Component::RootDir => (),
            Component::Normal(name) => {
                if create {
                    match fs::mkdirat(&fd, name, Mode::from_raw_mode(0o700)) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => (),
                        Err(_) => return Err(ProjectError::UnsafePath),
                    }
                }
                fd = fs::openat(&fd, name, DIRECTORY, Mode::empty())
                    .map_err(|_| ProjectError::UnsafePath)?;
            }
            _ => return Err(ProjectError::UnsafePath),
        }
        safe(&fs::fstat(&fd).map_err(|_| ProjectError::UnsafePath)?)?;
    }
    Ok(fd)
}

pub(crate) fn identity(path: &Path, executable: bool) -> Result<FileIdentity, ProjectError> {
    if path
        .components()
        .collect::<std::path::PathBuf>()
        .as_os_str()
        != path.as_os_str()
    {
        return Err(ProjectError::UnsafePath);
    }
    let fd = if executable {
        if path.file_name().and_then(|name| name.to_str()) != Some("hermes") {
            return Err(ProjectError::InvalidInput);
        }
        let parent = directory(path.parent().ok_or(ProjectError::UnsafePath)?, false)?;
        fs::openat(
            &parent,
            "hermes",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| ProjectError::UnsafePath)?
    } else {
        directory(path, false)?
    };
    let stat = fs::fstat(&fd).map_err(|_| ProjectError::UnsafePath)?;
    safe(&stat)?;
    if executable
        && (FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_mode & 0o111 == 0
            || stat.st_mode & 0o6000 != 0)
    {
        return Err(ProjectError::UnsafePath);
    }
    Ok(FileIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}
