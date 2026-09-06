//! Descriptor-relative private directory and lock; never follows a source symlink.
use super::{RecoveryError, files::Identity};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, Stat};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

pub(super) const LOCK: &str = "recovery.lock";
const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(super) struct Authority {
    lock: OwnedFd,
    pub handles: std::sync::atomic::AtomicUsize,
}

pub(super) struct Directory {
    pub fd: OwnedFd,
    pub uid: u32,
    pub authority: std::sync::Arc<Authority>,
    root: OwnedFd,
    manager: OwnedFd,
    identity: Stat,
    manager_identity: Stat,
    lock_identity: Identity,
}
impl Directory {
    pub fn open(root: &Path) -> Result<Self, RecoveryError> {
        let uid = rustix::process::geteuid().as_raw();
        let root = open_root(root, uid)?;
        let manager = child(&root, "harbormaster", uid)?;
        let fd = child(&manager, "recovery", uid)?;
        if ![0xef53, 0x5846_5342, 0x9123_683e, 0x0102_1994, 0x794c_7630]
            .contains(&fs::fstatfs(&fd)?.f_type)
        {
            return Err(RecoveryError::UnsafePath);
        }
        // Validate all entries before even creating a lock, including FIFO/hardlink.
        super::files::scan_entries(&fd, uid)?;
        let lock = fs::openat(
            &fd,
            LOCK,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        private_file(&fs::fstat(&lock)?, uid)?;
        fs::flock(&lock, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| RecoveryError::AlreadyOwned)?;
        let value = Self {
            identity: fs::fstat(&fd)?,
            manager_identity: fs::fstat(&manager)?,
            lock_identity: Identity::from_stat(&fs::fstat(&lock)?)?,
            fd,
            uid,
            root,
            manager,
            authority: std::sync::Arc::new(Authority {
                lock,
                handles: std::sync::atomic::AtomicUsize::new(0),
            }),
        };
        value.check()?;
        fs::fsync(&value.fd)?;
        Ok(value)
    }

    pub fn check(&self) -> Result<(), RecoveryError> {
        for (parent, name, identity) in [
            (&self.root, "harbormaster", &self.manager_identity),
            (&self.manager, "recovery", &self.identity),
        ] {
            let current = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| RecoveryError::IdentityChanged)?;
            directory_safe(&current, self.uid, true)?;
            if current.st_dev != identity.st_dev || current.st_ino != identity.st_ino {
                return Err(RecoveryError::IdentityChanged);
            }
        }
        let lock = fs::statat(&self.fd, LOCK, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| RecoveryError::IdentityChanged)?;
        private_file(&lock, self.uid)?;
        if Identity::from_stat(&lock)? != self.lock_identity
            || Identity::from_stat(&fs::fstat(&self.authority.lock)?)? != self.lock_identity
        {
            return Err(RecoveryError::IdentityChanged);
        }
        Ok(())
    }
}

fn child(parent: &OwnedFd, name: &str, uid: u32) -> Result<OwnedFd, RecoveryError> {
    match fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) => fs::fsync(parent)?,
        Err(rustix::io::Errno::EXIST) => (),
        Err(error) => return Err(error.into()),
    }
    let fd = fs::openat(parent, name, DIR, Mode::empty()).map_err(|_| RecoveryError::UnsafePath)?;
    directory_safe(&fs::fstat(&fd)?, uid, true)?;
    Ok(fd)
}
fn open_root(path: &Path, uid: u32) -> Result<OwnedFd, RecoveryError> {
    if !path.is_absolute() || path.as_os_str().as_bytes().len() > 4096 {
        return Err(RecoveryError::UnsafePath);
    }
    let mut fd = fs::open("/", DIR, Mode::empty())?;
    for component in path.components() {
        match component {
            Component::RootDir => (),
            Component::Normal(name) => {
                fd = fs::openat(&fd, name, DIR, Mode::empty())
                    .map_err(|_| RecoveryError::UnsafePath)?;
            }
            _ => return Err(RecoveryError::UnsafePath),
        }
        directory_safe(&fs::fstat(&fd)?, uid, false)?;
    }
    directory_safe(&fs::fstat(&fd)?, uid, true)?;
    Ok(fd)
}
fn directory_safe(stat: &Stat, uid: u32, private: bool) -> Result<(), RecoveryError> {
    let mode = stat.st_mode & 0o7777;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || if private {
            stat.st_uid != uid || mode != 0o700
        } else {
            (stat.st_uid != uid && stat.st_uid != 0) || mode & 0o022 != 0
        }
    {
        return Err(RecoveryError::UnsafePath);
    }
    Ok(())
}
pub(super) fn private_file(stat: &Stat, uid: u32) -> Result<(), RecoveryError> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != uid
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
    {
        return Err(RecoveryError::UnsafePath);
    }
    Ok(())
}
