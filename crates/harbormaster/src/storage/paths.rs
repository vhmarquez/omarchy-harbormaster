//! Descriptor anchored fixed manager files; same-UID replacement is not isolated.
use super::StorageError;
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, Stat};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::{cell::RefCell, collections::BTreeMap};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
pub(super) const DATABASE: &str = "state.db";
pub(super) const BACKUP: &str = "state.backup.db";
pub(super) const STAGING: &str = "state.staging.db";
pub(super) const ROLLBACK: &str = "state.rollback.db";
const KNOWN: [&str; 9] = [
    DATABASE,
    "state.db-wal",
    "state.db-shm",
    "state.db-journal",
    BACKUP,
    STAGING,
    ROLLBACK,
    "state.staging.db-journal",
    "state.lock",
];

pub(super) struct Paths {
    directory: OwnedFd,
    identity: Stat,
    parent: OwnedFd,
    _lock: OwnedFd,
    uid: u32,
    owned: RefCell<BTreeMap<&'static str, (u64, u64)>>,
}
impl Paths {
    pub(super) fn open(root: &Path) -> Result<Self, StorageError> {
        let uid = rustix::process::geteuid().as_raw();
        let parent = open_root(root, uid)?;
        match fs::mkdirat(&parent, "harbormaster", Mode::from_raw_mode(0o700)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => (),
            Err(error) => return Err(error.into()),
        }
        let directory = fs::openat(&parent, "harbormaster", DIRECTORY, Mode::empty())
            .map_err(|_| StorageError::UnsafePath)?;
        let identity = fs::fstat(&directory)?;
        directory_safe(&identity, uid, true)?;
        local_filesystem(&directory)?;
        for name in KNOWN {
            validate_entry(&directory, name, uid)?;
        }
        let lock = fs::openat(
            &directory,
            "state.lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        private_file(&fs::fstat(&lock)?, uid)?;
        fs::flock(&lock, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| StorageError::AlreadyOwned)?;
        let mut owned = BTreeMap::new();
        for name in [DATABASE, BACKUP, STAGING, ROLLBACK, "state.lock"] {
            if let Ok(stat) = fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
                owned.insert(name, (stat.st_dev, stat.st_ino));
            }
        }
        let paths = Self {
            directory,
            identity,
            parent,
            _lock: lock,
            uid,
            owned: RefCell::new(owned),
        };
        if paths.exists(STAGING)? || paths.exists(ROLLBACK)? {
            return Err(StorageError::RecoveryRequired);
        }
        Ok(paths)
    }

    pub(super) fn path(&self, name: &'static str) -> PathBuf {
        PathBuf::from(format!(
            "/proc/self/fd/{}/{name}",
            self.directory.as_raw_fd()
        ))
    }

    pub(super) fn check(&self) -> Result<(), StorageError> {
        let current = fs::statat(&self.parent, "harbormaster", AtFlags::SYMLINK_NOFOLLOW)?;
        if current.st_dev != self.identity.st_dev || current.st_ino != self.identity.st_ino {
            return Err(StorageError::UnsafePath);
        }
        directory_safe(&current, self.uid, true)?;
        for name in KNOWN {
            validate_entry(&self.directory, name, self.uid)?;
        }
        for (name, (device, inode)) in self.owned.borrow().iter() {
            let stat = fs::statat(&self.directory, *name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| StorageError::UnsafePath)?;
            if (stat.st_dev, stat.st_ino) != (*device, *inode) {
                return Err(StorageError::UnsafePath);
            }
        }
        Ok(())
    }

    pub(super) fn exists(&self, name: &'static str) -> Result<bool, StorageError> {
        match fs::statat(&self.directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => {
                private_file(&stat, self.uid)?;
                Ok(true)
            }
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn create(&self, name: &'static str) -> Result<(), StorageError> {
        let fd = fs::openat(
            &self.directory,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let stat = fs::fstat(&fd)?;
        private_file(&stat, self.uid)?;
        self.owned
            .borrow_mut()
            .insert(name, (stat.st_dev, stat.st_ino));
        fs::fsync(&self.directory)?;
        Ok(())
    }

    pub(super) fn rename(&self, from: &'static str, to: &'static str) -> Result<(), StorageError> {
        self.check()?;
        let identity = self
            .owned
            .borrow()
            .get(from)
            .copied()
            .ok_or(StorageError::UnsafePath)?;
        if self.exists(to)? && !self.owned.borrow().contains_key(to) {
            return Err(StorageError::UnsafePath);
        }
        fs::renameat(&self.directory, from, &self.directory, to)?;
        self.owned.borrow_mut().remove(from);
        self.owned.borrow_mut().insert(to, identity);
        fs::fsync(&self.directory)?;
        Ok(())
    }

    pub(super) fn remove(&self, name: &'static str) -> Result<(), StorageError> {
        self.check()?;
        if !self.owned.borrow().contains_key(name) {
            return Err(StorageError::UnsafePath);
        }
        if self.exists(name)? {
            fs::unlinkat(&self.directory, name, AtFlags::empty())?;
        }
        self.owned.borrow_mut().remove(name);
        fs::fsync(&self.directory)?;
        Ok(())
    }

    pub(super) fn length(&self, name: &'static str) -> Result<u64, StorageError> {
        if !self.exists(name)? {
            return Ok(0);
        }
        let stat = fs::statat(&self.directory, name, AtFlags::SYMLINK_NOFOLLOW)?;
        u64::try_from(stat.st_size).map_err(|_| StorageError::UnsafePath)
    }

    pub(super) fn sync(&self, name: &'static str) -> Result<(), StorageError> {
        let fd = fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        private_file(&fs::fstat(&fd)?, self.uid)?;
        fs::fsync(&fd)?;
        fs::fsync(&self.directory)?;
        Ok(())
    }
}

fn open_root(path: &Path, uid: u32) -> Result<OwnedFd, StorageError> {
    if !path.is_absolute() || path.as_os_str().as_bytes().len() > 4096 {
        return Err(StorageError::UnsafePath);
    }
    let mut fd = fs::open("/", DIRECTORY, Mode::empty())?;
    for component in path.components() {
        match component {
            Component::RootDir => (),
            Component::Normal(name) => {
                fd = fs::openat(&fd, name, DIRECTORY, Mode::empty())
                    .map_err(|_| StorageError::UnsafePath)?;
            }
            _ => return Err(StorageError::UnsafePath),
        }
        directory_safe(&fs::fstat(&fd)?, uid, false)?;
    }
    directory_safe(&fs::fstat(&fd)?, uid, true)?;
    Ok(fd)
}
fn directory_safe(stat: &Stat, uid: u32, private: bool) -> Result<(), StorageError> {
    let mode = stat.st_mode & 0o7777;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || if private {
            stat.st_uid != uid || mode != 0o700
        } else {
            (stat.st_uid != uid && stat.st_uid != 0) || mode & 0o022 != 0
        }
    {
        return Err(StorageError::UnsafePath);
    }
    Ok(())
}
fn private_file(stat: &Stat, uid: u32) -> Result<(), StorageError> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != uid
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
    {
        return Err(StorageError::UnsafePath);
    }
    Ok(())
}
fn validate_entry(fd: &OwnedFd, name: &str, uid: u32) -> Result<(), StorageError> {
    match fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => private_file(&stat, uid),
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn local_filesystem(fd: &OwnedFd) -> Result<(), StorageError> {
    let kind = fs::fstatfs(fd)?.f_type;
    // Linux ext4, XFS, Btrfs, tmpfs and overlayfs (the locked Docker fixture).
    // This does not prove physical durability of the backing block device.
    if ![0xef53, 0x5846_5342, 0x9123_683e, 0x0102_1994, 0x794c_7630].contains(&kind) {
        return Err(StorageError::UnsupportedFilesystem);
    }
    Ok(())
}
