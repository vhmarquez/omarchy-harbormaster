//! Read-only preview, then explicit identity/revision-bound bounded application.
use super::{
    ArtifactStore, MAX_CLEANUP_BATCH, RecoveryError, files::EntryHandle, logs, paths::LOCK,
};
use crate::protocol::{ProducerGeneration, ProducerId};

/// A trusted manager policy decision, not a wire instruction or automatic timer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupReason {
    Expired,
    PrivacyDisabled,
    RetiredGeneration {
        producer: ProducerId,
        generation: ProducerGeneration,
    },
}

/// Opaque, instance-bound plan retaining each selected stat identity and ownership authority.
pub struct CleanupPlan {
    selected: Vec<EntryHandle>,
    protected: usize,
    deferred: usize,
    revision: u64,
    owner: std::sync::Arc<super::paths::Authority>,
}
impl CleanupPlan {
    #[must_use]
    pub fn eligible_files(&self) -> usize {
        self.selected.len()
    }
    #[must_use]
    pub fn eligible_bytes(&self) -> u64 {
        self.selected.iter().map(|entry| entry.entry.bytes()).sum()
    }
    #[must_use]
    pub const fn protected_files(&self) -> usize {
        self.protected
    }
    #[must_use]
    pub const fn deferred_files(&self) -> usize {
        self.deferred
    }
}

/// Actual progress, including a partial failure; there is no all-or-nothing claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupResult {
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub failure: Option<RecoveryError>,
}

impl ArtifactStore {
    /// Inspect metadata only. Unknown names, SQLite artifacts and unrelated files
    /// are protected; manager timestamps in the future cannot expire early.
    /// # Errors
    /// Reject unsafe/excessive inventories and identities changed during preview.
    pub fn preview_cleanup(
        &self,
        reason: &CleanupReason,
        now_ms: u64,
    ) -> Result<CleanupPlan, RecoveryError> {
        let mut plan = CleanupPlan {
            selected: Vec::new(),
            protected: 0,
            deferred: 0,
            revision: self.revision,
            owner: self.directory.authority.clone(),
        };
        for entry in self.directory.scan()? {
            let eligible = if let Some(name) = &entry.spool {
                match reason {
                    CleanupReason::Expired => name.expired(now_ms),
                    CleanupReason::PrivacyDisabled => true,
                    CleanupReason::RetiredGeneration {
                        producer,
                        generation,
                    } => name.producer == *producer && name.generation == *generation,
                }
            } else {
                *reason == CleanupReason::PrivacyDisabled && logs::is_log(&entry.name)
            };
            if eligible && plan.selected.len() < MAX_CLEANUP_BATCH {
                plan.selected.push(self.directory.hold(entry)?);
            } else if eligible {
                plan.deferred += 1;
            } else if entry.name != LOCK {
                plan.protected += 1;
            }
        }
        Ok(plan)
    }

    /// Apply only a reviewed plan from this unchanged open instance. No recursion.
    /// # Errors
    /// Reject stale/cross-instance plans and changed entries before the first write.
    pub fn apply_cleanup(&mut self, plan: CleanupPlan) -> Result<CleanupResult, RecoveryError> {
        if plan.revision != self.revision
            || !std::sync::Arc::ptr_eq(&plan.owner, &self.directory.authority)
        {
            return Err(RecoveryError::IdentityChanged);
        }
        self.directory.scan()?;
        for held in &plan.selected {
            self.directory.validate(held)?;
        }
        self.changing()?;
        let mut result = CleanupResult {
            removed_files: 0,
            removed_bytes: 0,
            failure: None,
        };
        for held in plan.selected {
            if let Err(error) = self.directory.unlink(&held) {
                result.failure = Some(error);
                self.unknown_gap = true;
                break;
            }
            result.removed_files += 1;
            result.removed_bytes += held.entry.bytes();
            if held.entry.spool.is_some() {
                self.discarded_artifacts = self.discarded_artifacts.saturating_add(1);
                self.unknown_gap = true;
            }
            if let Err(error) = rustix::fs::fsync(&self.directory.fd) {
                result.failure = Some(error.into());
                self.unknown_gap = true;
                break;
            }
        }
        Ok(result)
    }
}
