use super::{Reconciliation, ReconciliationBaseline, ReducerWrite, StorageError};
use crate::domain::{ObservationState, ProcessState, TurnKey, TurnState};
use crate::protocol::{EventEnvelope, encode_frame};

pub(super) fn apply(set: &ReducerWrite) -> Result<(), StorageError> {
    let effects = &set.effects;
    if set.accepted_at < 0
        || effects.attention.len() > 8
        || effects
            .attention
            .iter()
            .map(|reason| *reason as u8)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != effects.attention.len()
        || effects.projection.run_id != set.fact.run_id
        || effects.projection.turn_id.as_ref()
            != effects.current_turn.as_ref().map(|key| &key.turn_id)
        || (effects.current_turn.is_none() && effects.projection.turn != TurnState::Unknown)
        || effects.current_turn.as_ref().is_some_and(|key| {
            !same_scope(key, &set.fact) || effects.projection.turn_id.as_ref() != Some(&key.turn_id)
        })
        || effects
            .resolve_transient
            .as_ref()
            .is_some_and(|key| !same_scope(key, &set.fact))
        || effects.tombstone.as_ref().is_some_and(|item| {
            !same_scope(&item.key, &set.fact)
                || set.fact.event.turn_id() != Some(&item.key.turn_id)
                || item.outcome_id != set.fact.event_id
                || item.terminal.is_none_or(|state| !state.is_terminal())
                || item.terminal != terminal(&set.fact)
        })
    {
        return Err(StorageError::InvalidRequest);
    }
    encode_frame(&set.fact).map_err(|_| StorageError::InvalidRequest)?;
    if serde_json::to_vec(set)
        .map_err(|_| StorageError::InvalidRequest)?
        .len()
        > 24_576
    {
        return Err(StorageError::InvalidRequest);
    }
    Ok(())
}
pub(super) fn reconciliation(request: &Reconciliation) -> Result<(), StorageError> {
    if let ReconciliationBaseline::Verified {
        projection,
        current_turn,
    } = &request.baseline
        && (projection.run_id != request.registration.run_id
            || projection.process != ProcessState::Alive
            || projection.observation != ObservationState::Fresh
            || projection.turn_id != *current_turn
            || match current_turn {
                Some(_) => !matches!(
                    projection.turn,
                    TurnState::Working | TurnState::AwaitingInput | TurnState::AwaitingApproval
                ),
                None => projection.turn != TurnState::Unknown,
            })
    {
        return Err(StorageError::InvalidRequest);
    }
    if matches!(request.baseline, ReconciliationBaseline::Unavailable)
        && (request.resolved_turn.is_some() || request.continued_turn.is_some())
    {
        return Err(StorageError::InvalidRequest);
    }
    if let Some(key) = &request.continued_turn
        && (request.resolved_turn.is_some()
            || key.producer_id != request.registration.producer_id
            || key.run_id != request.registration.run_id
            || request.registration.previous_generation.as_ref() != Some(&key.generation)
            || !matches!(&request.baseline, ReconciliationBaseline::Verified { projection, current_turn }
                if matches!(projection.turn, TurnState::AwaitingInput | TurnState::AwaitingApproval)
                    && current_turn.as_ref() == Some(&key.turn_id)))
    {
        return Err(StorageError::InvalidRequest);
    }
    Ok(())
}
fn same_scope(key: &TurnKey, fact: &EventEnvelope) -> bool {
    key.producer_id == fact.producer_id
        && key.generation == fact.generation
        && key.run_id == fact.run_id
}
pub(super) fn terminal(fact: &EventEnvelope) -> Option<crate::domain::TurnState> {
    use crate::{domain::TurnState, protocol::EventKind};
    match fact.event.kind() {
        EventKind::TurnCompleted => Some(TurnState::Completed),
        EventKind::TurnFailed => Some(TurnState::Failed),
        EventKind::TurnInterrupted => Some(TurnState::Interrupted),
        _ => None,
    }
}
