//! Trusted manager commands. These types are not a wire or adapter authority.
use crate::protocol::{
    EventEnvelope, EventId, HarnessKind, ProducerGeneration, ProducerId, Revision, RunId, Seq,
    TurnId,
};
use serde::Serialize;

pub const MAX_FACTS: usize = 20_000;
pub const MAX_TOMBSTONES: usize = 20_000;
pub const MAX_RUNS: usize = 10_000;
pub const MAX_ATTENTION: usize = 20_000;
pub const MAX_OUTBOX_PENDING: usize = 1_000;
pub const MAX_OUTBOX_AUDIT: usize = 20_000;
pub const PAGE_LIMIT: u16 = 100;
pub const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum HistoryRetention {
    SevenDays,
    #[default]
    ThirtyDays,
}
impl HistoryRetention {
    #[must_use]
    pub const fn days(self) -> i64 {
        match self {
            Self::SevenDays => 7,
            Self::ThirtyDays => 30,
        }
    }
    /// # Errors
    /// Rejects unsupported policy values, including the historical 90-day illustration.
    pub fn from_days(days: u16) -> Result<Self, super::StorageError> {
        match days {
            7 => Ok(Self::SevenDays),
            30 => Ok(Self::ThirtyDays),
            _ => Err(super::StorageError::InvalidRequest),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub expected_revision: Revision,
    pub producer_id: ProducerId,
    pub run_id: RunId,
    pub harness: HarnessKind,
    pub next_sequence: Seq,
    /// None only for a previously unknown producer. Reconciliation must name
    /// the stored generation even when retired; the engine returns a fresh one.
    pub previous_generation: Option<ProducerGeneration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProcessState {
    Unknown,
    Alive,
    Exited,
    Zombie,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ObservationState {
    Fresh,
    Stale,
    Disconnected,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TurnState {
    Unknown,
    Working,
    AwaitingInput,
    AwaitingApproval,
    Completed,
    Failed,
    Interrupted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AttentionReason {
    Input,
    NativeApproval,
    Failure,
    ConnectionUncertainty,
    Completion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeliveryState {
    Pending,
    AttemptedUncertain,
    Acknowledged,
    Expired,
    Overflowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunProjection {
    pub run_id: RunId,
    pub process: ProcessState,
    pub observation: ObservationState,
    pub turn: TurnState,
    pub turn_id: Option<TurnId>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionMutation {
    pub outcome_id: EventId,
    pub reason: AttentionReason,
    /// Review is an explicit manager decision. Delivery and maintenance never
    /// set this field, and opening a UI is not a storage command.
    pub reviewed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutboxIntent {
    pub outcome_id: EventId,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalTombstone {
    pub turn_id: TurnId,
    pub outcome_id: EventId,
}

/// A future authorized reducer supplies the complete write set. Constructing
/// this value asserts neither authorization nor safe adapter source provenance.
/// #9 exposes no adapter or wire dispatch that can persist an arbitrary frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriteSet {
    pub expected_revision: Revision,
    pub fact: EventEnvelope,
    pub projection: RunProjection,
    pub attention: Vec<AttentionMutation>,
    pub outbox: Option<OutboxIntent>,
    pub tombstone: Option<TerminalTombstone>,
    pub accepted_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotQuery {
    pub expected_revision: Option<Revision>,
    pub after_run: Option<RunId>,
    pub limit: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeKey {
    pub producer_id: ProducerId,
    pub generation: ProducerGeneration,
    pub event_id: EventId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewUpdate {
    pub expected_revision: Revision,
    pub outcome: OutcomeKey,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryUpdate {
    pub expected_revision: Revision,
    pub outcome: OutcomeKey,
    pub state: DeliveryState,
    pub at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Register(Registration),
    Commit(Box<WriteSet>),
    Snapshot(SnapshotQuery),
    Producer(ProducerId),
    Outcome(OutcomeKey),
    MarkReviewed(ReviewUpdate),
    Maintain {
        expected_revision: Revision,
        now: i64,
        history: HistoryRetention,
    },
    SetDelivery(DeliveryUpdate),
    Backup,
    RestoreBackup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerRecord {
    pub producer_id: ProducerId,
    pub harness: HarnessKind,
    pub generation: ProducerGeneration,
    pub run_id: RunId,
    pub next_sequence: Option<Seq>,
    pub active: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPage {
    pub revision: Revision,
    pub runs: Vec<RunProjection>,
    pub next_after: Option<RunId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeRecord {
    pub outcome: OutcomeKey,
    pub run_id: RunId,
    pub attention: Vec<AttentionMutation>,
    pub delivery: Option<DeliveryState>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaintenanceResult {
    pub facts_removed: u32,
    pub tombstones_removed: u32,
    pub generations_retired: u32,
    pub deliveries_expired: u32,
    pub more: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Registered {
        generation: ProducerGeneration,
        revision: Revision,
    },
    Committed {
        revision: Revision,
        duplicate: bool,
    },
    Snapshot(SnapshotPage),
    Producer(Option<ProducerRecord>),
    Outcome(Option<OutcomeRecord>),
    Maintained {
        revision: Revision,
        result: MaintenanceResult,
    },
    DeliveryUpdated {
        revision: Revision,
    },
    BackupComplete,
    Restored {
        revision: Revision,
    },
}
