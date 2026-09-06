//! Validate causal identity before computing any lifecycle effect.
use super::{DomainError, ObservationPolicy, ReductionState, TurnKey, TurnState};
use crate::protocol::{EventEnvelope, EventPayload};

pub(super) fn validate(
    state: &ReductionState,
    event: &EventEnvelope,
    policy: &ObservationPolicy,
) -> Result<(), DomainError> {
    if state.projection.run_id != event.run_id
        || state
            .current_turn
            .as_ref()
            .is_some_and(|current| !same_scope(current, event))
    {
        return Err(DomainError::InvalidScope);
    }
    if state.projection.turn_id.as_ref()
        != state.current_turn.as_ref().map(|current| &current.turn_id)
        || (state.current_turn.is_none() && state.projection.turn != TurnState::Unknown)
        || state.tombstone.as_ref().is_some_and(|terminal| {
            Some(&terminal.key) != turn_key(event).as_ref()
                || terminal.terminal.is_some_and(|kind| !kind.is_terminal())
        })
    {
        return Err(DomainError::InvalidEvidence);
    }
    current_terminal(state, event)?;
    if !policy.supports(event.event.kind()) {
        return Err(DomainError::UnsupportedSignal);
    }
    Ok(())
}

fn current_terminal(state: &ReductionState, event: &EventEnvelope) -> Result<(), DomainError> {
    if state.current_turn.as_ref() != turn_key(event).as_ref() {
        return Ok(());
    }
    if let Some(terminal) = &state.tombstone {
        if terminal
            .terminal
            .is_some_and(|kind| kind != state.projection.turn)
            || (terminal.terminal.is_none()
                && !state.projection.turn.is_terminal()
                && state.projection.turn != TurnState::Unknown)
        {
            return Err(DomainError::InvalidEvidence);
        }
    } else if state.projection.turn.is_terminal() {
        return Err(DomainError::InvalidEvidence);
    }
    Ok(())
}

fn same_scope(key: &TurnKey, event: &EventEnvelope) -> bool {
    key.run_id == event.run_id
        && key.producer_id == event.producer_id
        && key.generation == event.generation
}

pub(super) fn turn_key(event: &EventEnvelope) -> Option<TurnKey> {
    event.event.turn_id().map(|turn| TurnKey {
        producer_id: event.producer_id.clone(),
        generation: event.generation.clone(),
        run_id: event.run_id.clone(),
        turn_id: turn.clone(),
    })
}

pub(super) const fn turn_state(event: &EventPayload) -> TurnState {
    match event {
        EventPayload::TurnStarted(_) => TurnState::Working,
        EventPayload::TurnAwaitingInput(_) => TurnState::AwaitingInput,
        EventPayload::TurnAwaitingApproval(_) => TurnState::AwaitingApproval,
        EventPayload::TurnCompleted(_) => TurnState::Completed,
        EventPayload::TurnFailed(_) => TurnState::Failed,
        EventPayload::TurnInterrupted(_) => TurnState::Interrupted,
        _ => TurnState::Unknown,
    }
}
