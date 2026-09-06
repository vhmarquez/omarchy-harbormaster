//! Serial event-to-transaction coordination, bounded by an exclusive registry borrow.
use super::{
    DurableReceipt,
    operation::{Operation, Poll},
};
use crate::{
    domain::ObservationPolicy,
    ingestion::Registry,
    protocol::Revision,
    recovery::ReplayEntry,
    storage::{DatabaseWorker, Request, StorageError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitFailure {
    Unreconciled,
    Conflict,
    SequenceGap,
    PriorSequenceUnverifiable,
    InvalidEvidence,
    Storage(StorageError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitProgress {
    Pending,
    Backpressure,
    UnknownOutcome,
    Committed { revision: Revision },
    Rejected(CommitFailure),
}

#[derive(Clone, Copy)]
pub(super) enum Purpose {
    Context,
    Apply,
    Retire(CommitFailure),
}

/// Retains the exact original intent and eligible entry through lost receipts.
/// Neither dropping this value nor creating it deletes spool data or grants ACK.
pub struct PendingCommit {
    entry: ReplayEntry,
    request: Request,
    purpose: Purpose,
    policy: ObservationPolicy,
    accepted_at: i64,
    notify: bool,
    discarded: u64,
    unknown_gap: bool,
}

pub struct CommitJob<'a> {
    pub(super) worker: &'a DatabaseWorker,
    pub(super) registry: &'a mut Registry,
    pub(super) entry: Option<ReplayEntry>,
    pub(super) operation: Operation,
    pub(super) purpose: Purpose,
    pub(super) policy: ObservationPolicy,
    pub(super) accepted_at: i64,
    pub(super) notify: bool,
    pub(super) discarded: u64,
    pub(super) unknown_gap: bool,
    pub(super) retries: u8,
    pub(super) progress: CommitProgress,
    pub(super) done: bool,
    pub(super) receipt: Option<DurableReceipt>,
}

impl<'a> CommitJob<'a> {
    /// The sealed entry has passed the one source eligibility gate. This borrow
    /// prevents concurrent admission/reconciliation through the same registry.
    #[must_use]
    pub fn new(
        worker: &'a DatabaseWorker,
        registry: &'a mut Registry,
        entry: ReplayEntry,
        policy: ObservationPolicy,
        accepted_at: i64,
        notify: bool,
    ) -> Self {
        let request = Request::Context(Box::new(entry.event().clone()));
        Self {
            worker,
            registry,
            entry: Some(entry),
            operation: Operation::new(request),
            purpose: Purpose::Context,
            policy,
            accepted_at,
            notify,
            discarded: 0,
            unknown_gap: false,
            retries: 0,
            progress: CommitProgress::Pending,
            done: false,
            receipt: None,
        }
    }

    /// Resume the unchanged retained request, never invent another event/time.
    #[must_use]
    pub fn resume(
        worker: &'a DatabaseWorker,
        registry: &'a mut Registry,
        pending: PendingCommit,
    ) -> Self {
        Self {
            worker,
            registry,
            entry: Some(pending.entry),
            operation: Operation::new(pending.request),
            purpose: pending.purpose,
            policy: pending.policy,
            accepted_at: pending.accepted_at,
            notify: pending.notify,
            discarded: pending.discarded,
            unknown_gap: pending.unknown_gap,
            retries: 0,
            progress: CommitProgress::Pending,
            done: false,
            receipt: None,
        }
    }

    /// One nonblocking worker poll, with no sleep or internal retry loop.
    pub fn poll(&mut self) -> CommitProgress {
        if self.done {
            return self.progress;
        }
        self.progress = match self.operation.poll(self.worker) {
            Poll::Pending => CommitProgress::Pending,
            Poll::Backpressure => CommitProgress::Backpressure,
            Poll::Unknown => {
                self.freeze();
                self.unknown_gap = true;
                self.done = true;
                CommitProgress::UnknownOutcome
            }
            Poll::Result(result) => self.response(result),
        };
        self.progress
    }

    /// Volatile lower bound, separate from whether a storage counter committed.
    #[must_use]
    pub const fn known_discarded(&self) -> u64 {
        self.discarded
    }

    #[must_use]
    pub const fn unknown_gap(&self) -> bool {
        self.unknown_gap
    }

    /// Move the receipt and owned entry once; no spool deletion happens here.
    pub fn take_committed(&mut self) -> Option<(DurableReceipt, ReplayEntry)> {
        let receipt = self.receipt.take()?;
        self.entry.take().map(|entry| (receipt, entry))
    }

    /// Preserve a submitted intent through pending/unknown/error recovery. This
    /// does not cancel any operation; a database worker may still finish it.
    #[must_use]
    pub fn into_pending(mut self) -> Option<PendingCommit> {
        self.freeze();
        self.entry.map(|entry| PendingCommit {
            entry,
            request: self.operation.request().clone(),
            purpose: self.purpose,
            policy: self.policy,
            accepted_at: self.accepted_at,
            notify: self.notify,
            discarded: self.discarded,
            unknown_gap: self.unknown_gap,
        })
    }
}
