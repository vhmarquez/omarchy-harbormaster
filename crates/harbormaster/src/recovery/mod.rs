//! Bounded private manager artifacts. Spooling is never durable acknowledgment.
//!
//! M1 qualifies no live source mapping. Only private synthetic unit fixtures can
//! exercise eligible event persistence. Same-UID malicious code is not isolated.
mod cleanup;
mod files;
mod logs;
mod names;
mod paths;
mod provenance;
mod spool;

pub use cleanup::{CleanupPlan, CleanupReason, CleanupResult};
pub use logs::{LogReason, LogRecord};
pub use provenance::AdapterRecord;
pub use spool::{ReplayEntry, SpoolReceipt};

use paths::Directory;
use std::path::Path;

pub const MAX_PRODUCER_BYTES: u64 = 256 * 1024;
pub const MAX_SPOOL_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_SPOOL_FILES: usize = 1024;
pub const MAX_PRODUCER_FILES: usize = 64;
pub const MAX_RECORD_BYTES: usize = 16 * 1024;
pub const MAX_REPLAY_BATCH: usize = 32;
pub const MAX_CLEANUP_BATCH: usize = 128;
pub const MAX_HELD_HANDLES: usize = 128;
pub const SPOOL_TTL_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
pub const LOG_FILES: usize = 4;

/// Fixed failures contain neither adapter input nor paths or OS error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    UnsupportedAdapter,
    UnprovenSource,
    InvalidRecord,
    UnsafePath,
    AlreadyOwned,
    BoundExceeded,
    IdentityChanged,
    Io,
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedAdapter => "unsupported_adapter",
            Self::UnprovenSource => "unproven_source",
            Self::InvalidRecord => "invalid_record",
            Self::UnsafePath => "unsafe_path",
            Self::AlreadyOwned => "already_owned",
            Self::BoundExceeded => "bound_exceeded",
            Self::IdentityChanged => "identity_changed",
            Self::Io => "io_unavailable",
        })
    }
}
impl std::error::Error for RecoveryError {}
impl From<std::io::Error> for RecoveryError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<rustix::io::Errno> for RecoveryError {
    fn from(_: rustix::io::Errno) -> Self {
        Self::Io
    }
}

/// Immutable numeric observation; loss accounting is volatile, not a DB receipt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RecoveryStatus {
    pub spool_bytes: u64,
    pub spool_files: usize,
    pub protected_files: usize,
    pub log_bytes: u64,
    pub rejected_attempts: u64,
    pub discarded_artifacts: u64,
    pub unknown_gap: bool,
}

/// Explicitly opened manager-owned artifacts; opening does not read harness data.
pub struct ArtifactStore {
    directory: Directory,
    rejected_attempts: u64,
    discarded_artifacts: u64,
    unknown_gap: bool,
    revision: u64,
}
impl ArtifactStore {
    /// Create/open only the fixed private manager recovery directory and lock.
    /// # Errors
    /// Reject unsafe ancestry/entries, nonlocal filesystems and a concurrent owner.
    pub fn open(root: &Path) -> Result<Self, RecoveryError> {
        let store = Self {
            directory: Directory::open(root)?,
            rejected_attempts: 0,
            discarded_artifacts: 0,
            unknown_gap: true,
            revision: 0,
        };
        store.directory.scan()?;
        Ok(store)
    }

    /// Read already-owned filesystem metadata without rotation, deletion or policy.
    /// # Errors
    /// Reject changed identities, unsafe entries or an excessive scan.
    pub fn snapshot(&self) -> Result<RecoveryStatus, RecoveryError> {
        let entries = self.directory.scan()?;
        let mut status = RecoveryStatus {
            rejected_attempts: self.rejected_attempts,
            discarded_artifacts: self.discarded_artifacts,
            unknown_gap: self.unknown_gap,
            ..RecoveryStatus::default()
        };
        for entry in entries {
            if logs::is_log(&entry.name) {
                status.log_bytes = status.log_bytes.saturating_add(entry.bytes());
            } else if entry.name != paths::LOCK {
                status.spool_files += 1;
                status.spool_bytes = status.spool_bytes.saturating_add(entry.bytes());
                status.protected_files += usize::from(entry.spool.is_none());
            }
        }
        Ok(status)
    }

    fn failure(&mut self, error: RecoveryError) -> RecoveryError {
        self.rejected_attempts = self.rejected_attempts.saturating_add(1);
        self.unknown_gap = true;
        error
    }

    fn changing(&mut self) -> Result<(), RecoveryError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(RecoveryError::BoundExceeded)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
