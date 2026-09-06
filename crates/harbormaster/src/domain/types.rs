//! Pure lifecycle vocabulary; identities and states confer no runtime authority.
use crate::protocol::{EventId, ProducerGeneration, ProducerId, RunId, TurnId};
use serde::Serialize;

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

/// A turn is never identified by its text alone or ordered across producers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TurnKey {
    pub producer_id: ProducerId,
    pub generation: ProducerGeneration,
    pub run_id: RunId,
    pub turn_id: TurnId,
}

/// Existing terminal protection. A legacy pruned fact can leave its kind unknown;
/// the protection still dominates later starts and waits for this exact key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TombstoneEvidence {
    pub key: TurnKey,
    pub outcome_id: EventId,
    pub terminal: Option<TurnState>,
}
