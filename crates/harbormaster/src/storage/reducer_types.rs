//! Fixed manager operations for deterministic reduction and explicit policy.
use super::{HistoryRetention, MaintenanceResult, OutcomeKey, ProducerRecord, Registration};
use crate::{
    domain::{Effects, ReductionState, RunProjection},
    protocol::{EventEnvelope, Revision, TurnId},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventReceipt {
    Missing,
    Exact {
        revision: Revision,
    },
    Conflict,
    /// A checkpoint is not proof of the pruned event's identity or payload.
    PriorSequenceUnverifiable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducerContext {
    pub revision: Revision,
    pub producer: Option<ProducerRecord>,
    pub state: ReductionState,
    pub receipt: EventReceipt,
}
/// Trusted internal effects; this type has no wire deserializer or adapter entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReducerWrite {
    pub expected_revision: Revision,
    pub fact: EventEnvelope,
    pub effects: Effects,
    pub accepted_at: i64,
    pub notify: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationBaseline {
    Unavailable,
    /// The manager must establish this baseline independently of producer claims.
    Verified {
        projection: RunProjection,
        current_turn: Option<TurnId>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    pub registration: Registration,
    pub baseline: ReconciliationBaseline,
    pub discarded_events: u64,
    /// Independently established progress for the stored old current turn.
    pub resolved_turn: Option<crate::domain::TurnKey>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeReview {
    pub expected_revision: Revision,
    pub outcome: OutcomeKey,
    pub outcome_revision: Revision,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePolicy {
    pub revision: Revision,
    pub history: HistoryRetention,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageStatus {
    pub revision: Revision,
    pub facts: u32,
    pub tombstones: u32,
    pub attention: u32,
    pub pending_deliveries: u32,
    pub discarded_events: u64,
    pub cleanup: MaintenanceResult,
}
/// An immutable bounded plan. Only the engine can construct or alter its identity.
#[derive(Debug, Clone)]
pub struct CleanupPreview {
    pub(crate) owner: std::sync::Arc<()>,
    pub(crate) revision: Revision,
    pub(crate) now: i64,
    pub(crate) history: HistoryRetention,
    pub(crate) facts: Vec<i64>,
    pub(crate) tombstones: Vec<i64>,
    pub(crate) deliveries: Vec<i64>,
    pub(crate) result: MaintenanceResult,
}
impl CleanupPreview {
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    #[must_use]
    pub const fn result(&self) -> MaintenanceResult {
        self.result
    }
}

impl PartialEq for CleanupPreview {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.owner, &other.owner)
            && self.revision == other.revision
            && self.now == other.now
            && self.history == other.history
            && self.facts == other.facts
            && self.tombstones == other.tombstones
            && self.deliveries == other.deliveries
            && self.result == other.result
    }
}
impl Eq for CleanupPreview {}
