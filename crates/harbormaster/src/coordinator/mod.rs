//! Narrow manager coordination; no wire dispatcher, process collector or service.
mod commit;
mod decision;
mod operation;
mod registration;
pub use commit::{CommitFailure, CommitJob, CommitProgress, PendingCommit};
pub use registration::{ReconciliationInput, ReconciliationJob, ReconciliationProgress};

use crate::protocol::{EventEnvelope, Revision};
use crate::recovery::{ArtifactStore, RecoveryError, ReplayEntry};

/// Constructed only after an actual exact durable receipt or atomic commit.
/// No wire deserializer, public constructor, or volatile admission conversion.
pub struct DurableReceipt {
    event: EventEnvelope,
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
    store.confirm_committed(entry, receipt.event())
}

#[cfg(test)]
mod tests;
