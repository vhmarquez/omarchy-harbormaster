//! Pure deterministic domain vocabulary; no I/O or storage dependencies.
mod types;
pub use types::*;
mod evidence;
mod policy;
mod reducer;
pub use policy::{DomainError, ObservationPolicy};
pub use reducer::reduce;

/// One revision-consistent view supplied by the durable owner. The tombstone
/// concerns the incoming event's turn, which can differ from the current turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReductionState {
    pub projection: RunProjection,
    pub current_turn: Option<TurnKey>,
    pub tombstone: Option<TombstoneEvidence>,
}

/// Pure intent, never authorization or a durable receipt. Transient reasons are
/// ensured once per scoped turn/reason; resolution never sets human review.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Effects {
    pub projection: RunProjection,
    pub current_turn: Option<TurnKey>,
    pub attention: Vec<AttentionReason>,
    pub resolve_transient: Option<TurnKey>,
    pub tombstone: Option<TombstoneEvidence>,
    /// The atomic owner must disable normal admission until fresh reconciliation.
    pub reconciliation_required: bool,
}
