//! Compare independently acquired evidence; this module never probes a process.
use super::{DomainError, ObservationState, ProcessState, RunProjection, TurnState};
use crate::protocol::{HarnessKind, ProducerGeneration, ProducerId, RunId, TurnId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationScope {
    pub producer_id: ProducerId,
    pub previous_generation: Option<ProducerGeneration>,
    pub run_id: RunId,
    pub harness: HarnessKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeIdentity {
    pub boot_id: [u8; 16],
    pub pid: u32,
    pub start_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentTurnEvidence {
    KnownNone,
    Known { turn_id: TurnId, state: TurnState },
    Unavailable,
    Conflicting,
}

/// Trusted manager inputs, not deserialized wire claims. Equality checks cannot
/// establish their source provenance. M1 supplies no runtime collector or live
/// adapter eligibility; only independently acquired evidence may be supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationEvidence {
    pub expected_scope: ReconciliationScope,
    pub observed_scope: ReconciliationScope,
    pub expected_runtime: RuntimeIdentity,
    pub observed_runtime: RuntimeIdentity,
    pub current_boot: [u8; 16],
    pub process: ProcessState,
    pub freshness: ObservationState,
    pub current_turn: CurrentTurnEvidence,
}

/// Construct a baseline only from consistent fresh identity/current-turn inputs.
/// This pure result is not proof that a process was inspected or authorization.
/// # Errors
/// Rejects missing, stale, contradictory, exited/zombie or wrong-scope evidence.
pub fn reconcile_projection(
    evidence: &ReconciliationEvidence,
) -> Result<RunProjection, DomainError> {
    if evidence.expected_scope != evidence.observed_scope
        || evidence.expected_runtime != evidence.observed_runtime
        || evidence.expected_runtime.boot_id != evidence.current_boot
    {
        return Err(DomainError::InvalidScope);
    }
    if evidence.process != ProcessState::Alive
        || evidence.freshness != ObservationState::Fresh
        || evidence.expected_runtime.pid == 0
        || evidence.expected_runtime.start_ticks == 0
        || evidence.current_boot == [0; 16]
    {
        return Err(DomainError::InvalidEvidence);
    }
    let (turn, turn_id) = match &evidence.current_turn {
        CurrentTurnEvidence::KnownNone => (TurnState::Unknown, None),
        CurrentTurnEvidence::Known { turn_id, state }
            if matches!(
                state,
                TurnState::Working | TurnState::AwaitingInput | TurnState::AwaitingApproval
            ) =>
        {
            (*state, Some(turn_id.clone()))
        }
        _ => return Err(DomainError::InvalidEvidence),
    };
    Ok(RunProjection {
        run_id: evidence.expected_scope.run_id.clone(),
        process: ProcessState::Alive,
        observation: ObservationState::Fresh,
        turn,
        turn_id,
    })
}
