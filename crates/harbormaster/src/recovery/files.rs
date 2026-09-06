//! Bounded inventory and identity tokens; regular descriptors never escape a call.
use super::{
    LOG_FILES, MAX_LOG_BYTES, MAX_SPOOL_FILES, RecoveryError, logs,
    names::SpoolName,
    paths::{Directory, LOCK, private_file},
};
use rustix::fs::{self, AtFlags, Mode, OFlags, Stat};
use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    device: u64,
    inode: u64,
    bytes: u64,
    modified: (i64, u64),
    changed: (i64, u64),
}
impl Identity {
    pub fn from_stat(stat: &Stat) -> Result<Self, RecoveryError> {
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            bytes: u64::try_from(stat.st_size).map_err(|_| RecoveryError::UnsafePath)?,
            modified: (stat.st_mtime, stat.st_mtime_nsec),
            changed: (stat.st_ctime, stat.st_ctime_nsec),
        })
    }
}
pub(super) struct Entry {
    pub name: String,
    pub identity: Identity,
    pub spool: Option<SpoolName>,
}
impl Entry {
    pub fn bytes(&self) -> u64 {
        self.identity.bytes
    }
}

pub(super) struct EntryHandle {
    pub entry: Entry,
    lease: HandleLease,
}

struct HandleLease(std::sync::Arc<super::paths::Authority>);
impl Drop for HandleLease {
    fn drop(&mut self) {
        self.0
            .handles
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(super) fn write_once(file: &mut std::fs::File, bytes: &[u8]) -> Result<(), RecoveryError> {
    use std::io::Write;
    if file.write(bytes)? != bytes.len() {
        return Err(RecoveryError::Io);
    }
    Ok(())
}

pub(super) fn scan_entries(fd: &OwnedFd, uid: u32) -> Result<Vec<Entry>, RecoveryError> {
    let path = format!("/proc/self/fd/{}", fd.as_raw_fd());
    let mut entries = Vec::new();
    for child in std::fs::read_dir(path)? {
        if entries.len() > MAX_SPOOL_FILES + LOG_FILES {
            return Err(RecoveryError::BoundExceeded);
        }
        let name = child?
            .file_name()
            .into_string()
            .map_err(|_| RecoveryError::UnsafePath)?;
        let stat = fs::statat(fd, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)?;
        private_file(&stat, uid)?;
        let identity = Identity::from_stat(&stat)?;
        if (logs::is_log(&name) && identity.bytes > MAX_LOG_BYTES)
            || (name == LOCK && identity.bytes != 0)
        {
            return Err(RecoveryError::BoundExceeded);
        }
        entries.push(Entry {
            spool: SpoolName::parse(&name),
            name,
            identity,
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    if entries
        .iter()
        .filter(|entry| entry.name != LOCK && !logs::is_log(&entry.name))
        .count()
        > MAX_SPOOL_FILES
    {
        return Err(RecoveryError::BoundExceeded);
    }
    Ok(entries)
}

impl Directory {
    pub fn file_identity(
        &self,
        name: &str,
        file: &std::fs::File,
    ) -> Result<Identity, RecoveryError> {
        self.check()?;
        let named = fs::statat(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| RecoveryError::IdentityChanged)?;
        let opened = fs::fstat(file)?;
        private_file(&named, self.uid)?;
        private_file(&opened, self.uid)?;
        let identity = Identity::from_stat(&opened)?;
        if Identity::from_stat(&named)? != identity {
            return Err(RecoveryError::IdentityChanged);
        }
        Ok(identity)
    }

    pub fn scan(&self) -> Result<Vec<Entry>, RecoveryError> {
        self.check()?;
        scan_entries(&self.fd, self.uid)
    }

    pub fn hold(&self, entry: Entry) -> Result<EntryHandle, RecoveryError> {
        self.check()?;
        self.authority
            .handles
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |count| (count < super::MAX_HELD_HANDLES).then_some(count + 1),
            )
            .map_err(|_| RecoveryError::BoundExceeded)?;
        let held = EntryHandle {
            entry,
            lease: HandleLease(self.authority.clone()),
        };
        // The descriptor is transient: a long-lived token must never pin deleted bytes.
        drop(self.open_handle(&held)?);
        Ok(held)
    }

    pub fn validate(&self, held: &EntryHandle) -> Result<(), RecoveryError> {
        self.check()?;
        if !std::sync::Arc::ptr_eq(&held.lease.0, &self.authority) {
            return Err(RecoveryError::IdentityChanged);
        }
        let named = fs::statat(
            &self.fd,
            held.entry.name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| RecoveryError::IdentityChanged)?;
        private_file(&named, self.uid)?;
        if Identity::from_stat(&named)? != held.entry.identity {
            return Err(RecoveryError::IdentityChanged);
        }
        Ok(())
    }

    fn open_handle(&self, held: &EntryHandle) -> Result<std::fs::File, RecoveryError> {
        self.validate(held)?;
        let fd = fs::openat(
            &self.fd,
            held.entry.name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let file = std::fs::File::from(fd);
        if self.file_identity(&held.entry.name, &file)? != held.entry.identity {
            return Err(RecoveryError::IdentityChanged);
        }
        Ok(file)
    }

    pub fn read(&self, held: &EntryHandle, maximum: usize) -> Result<Vec<u8>, RecoveryError> {
        if held.entry.bytes() > maximum as u64 {
            return Err(RecoveryError::BoundExceeded);
        }
        let file = self.open_handle(held)?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(held.entry.bytes()).map_err(|_| RecoveryError::BoundExceeded)?,
        );
        (&file).take(maximum as u64 + 1).read_to_end(&mut bytes)?;
        if self.file_identity(&held.entry.name, &file)? != held.entry.identity
            || bytes.len() > maximum
            || bytes.len() as u64 != held.entry.bytes()
        {
            return Err(RecoveryError::IdentityChanged);
        }
        Ok(bytes)
    }

    pub fn unlink(&self, held: &EntryHandle) -> Result<(), RecoveryError> {
        drop(self.open_handle(held)?);
        // No retained regular FD survives unlink. Same-UID malicious races remain
        // outside this boundary; recheck full metadata after the descriptor closes.
        self.validate(held)?;
        fs::unlinkat(&self.fd, held.entry.name.as_str(), AtFlags::empty())?;
        Ok(())
    }

    pub fn remove(&self, held: &EntryHandle) -> Result<(), RecoveryError> {
        self.unlink(held)?;
        fs::fsync(&self.fd)?;
        Ok(())
    }
}
