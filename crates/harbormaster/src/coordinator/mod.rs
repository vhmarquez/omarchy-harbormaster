//! Narrow manager coordination; no wire dispatcher, process collector or service.
mod commit;
mod decision;
mod operation;
mod registration;
pub use commit::{CommitFailure, CommitJob, CommitProgress, PendingCommit};
pub use registration::{ReconciliationInput, ReconciliationJob, ReconciliationProgress};

use crate::protocol::{EventEnvelope, HarnessKind, Revision};
use crate::recovery::{ArtifactStore, RecoveryError, ReplayEntry};

/// One job's volatile observations, independent of the durable loss counter.
/// Counts may overlap durable accounting; they must never be summed as unique loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CommitStatus {
    pub known_queued_discards: u64,
    pub unknown_gap: bool,
}

/// Constructed only after an actual exact durable receipt or atomic commit.
/// No wire deserializer, public constructor, or volatile admission conversion.
pub struct DurableReceipt {
    event: EventEnvelope,
    harness: HarnessKind,
    revision: Revision,
}

impl DurableReceipt {
    #[must_use]
    pub const fn event(&self) -> &EventEnvelope {
        &self.event
    }
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }
}

/// Dispose of the exact owned spool entry only after proven durable acceptance.
/// Failed cleanup does not undo that acceptance; replay can recheck the receipt.
/// # Errors
/// Rejects mismatched event contents/entry identity or failed filesystem cleanup.
pub fn confirm_spool(
    store: &mut ArtifactStore,
    entry: ReplayEntry,
    receipt: &DurableReceipt,
) -> Result<(), RecoveryError> {
    if entry.harness() != receipt.harness {
        return Err(RecoveryError::InvalidRecord);
    }
    store.confirm_committed(entry, receipt.event())
}

#[cfg(test)]
mod tests;
