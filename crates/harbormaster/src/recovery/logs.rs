//! Four fixed bounded files. Records have no caller-supplied text fields.
use super::{
    ArtifactStore, MAX_LOG_BYTES, RecoveryError,
    files::{Entry, Identity},
    paths::private_file,
};
use rustix::fs::{self, Mode, OFlags, RenameFlags};

const NAMES: [&str; 4] = [
    "metadata.0.log",
    "metadata.1.log",
    "metadata.2.log",
    "metadata.3.log",
];
pub(super) fn is_log(name: &str) -> bool {
    NAMES.contains(&name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogReason {
    StorageUnavailable,
    UnsupportedAdapter,
    MissingHook,
    RecoveryDropped,
    CleanupCompleted,
    ReconciliationRequired,
}

/// Fixed codes and numeric manager data, never environment/paths/errors/payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LogRecord {
    pub observed_ms: u64,
    pub reason: LogReason,
    pub count: u64,
    pub unknown_gap: bool,
}

impl ArtifactStore {
    /// Append one bounded record, rotating only fixed names when needed. Four
    /// files × 1 MiB, including partially written records; no implicit age TTL.
    /// # Errors
    /// Reject unsafe/oversized files; report write, rotation and durability faults.
    pub fn append_log(&mut self, record: LogRecord) -> Result<(), RecoveryError> {
        self.write_log(record).map_err(|error| self.failure(error))
    }

    fn write_log(&mut self, record: LogRecord) -> Result<(), RecoveryError> {
        let mut bytes = serde_json::to_vec(&record).map_err(|_| RecoveryError::InvalidRecord)?;
        bytes.push(b'\n');
        if bytes.len() > 256 {
            return Err(RecoveryError::BoundExceeded);
        }
        let entries = self.directory.scan()?;
        let current = entries.iter().find(|entry| entry.name == NAMES[0]);
        let rotate =
            current.is_some_and(|entry| entry.bytes() + bytes.len() as u64 > MAX_LOG_BYTES);
        self.changing()?;
        if rotate {
            self.rotate_logs(entries)?;
        }
        let entries = self.directory.scan()?;
        let current = entries.into_iter().find(|entry| entry.name == NAMES[0]);
        let flags =
            OFlags::WRONLY | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let flags = if current.is_some() {
            flags
        } else {
            flags | OFlags::CREATE | OFlags::EXCL
        };
        let fd = fs::openat(
            &self.directory.fd,
            NAMES[0],
            flags,
            Mode::from_raw_mode(0o600),
        )?;
        let stat = fs::fstat(&fd)?;
        private_file(&stat, self.directory.uid)?;
        if current.is_some_and(|entry| Identity::from_stat(&stat).ok() != Some(entry.identity)) {
            return Err(RecoveryError::IdentityChanged);
        }
        if u64::try_from(stat.st_size).map_err(|_| RecoveryError::UnsafePath)? + bytes.len() as u64
            > MAX_LOG_BYTES
        {
            return Err(RecoveryError::BoundExceeded);
        }
        let mut file = std::fs::File::from(fd);
        self.directory.file_identity(NAMES[0], &file)?;
        super::files::write_once(&mut file, &bytes)?;
        self.directory.file_identity(NAMES[0], &file)?;
        file.sync_all()?;
        fs::fsync(&self.directory.fd)?;
        Ok(())
    }

    fn rotate_logs(&self, entries: Vec<Entry>) -> Result<(), RecoveryError> {
        let mut handles = Vec::new();
        for entry in entries.into_iter().filter(|entry| is_log(&entry.name)) {
            handles.push(self.directory.hold(entry)?);
        }
        for held in &handles {
            self.directory.validate(held)?;
        }
        if let Some(oldest) = handles.iter().find(|entry| entry.entry.name == NAMES[3]) {
            self.directory.remove(oldest)?;
        }
        for index in (0..3).rev() {
            if let Some(held) = handles
                .iter()
                .find(|entry| entry.entry.name == NAMES[index])
            {
                self.directory.validate(held)?;
                fs::renameat_with(
                    &self.directory.fd,
                    NAMES[index],
                    &self.directory.fd,
                    NAMES[index + 1],
                    RenameFlags::NOREPLACE,
                )?;
                fs::fsync(&self.directory.fd)?;
            }
        }
        Ok(())
    }
}
