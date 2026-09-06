//! Deterministic lifecycle decisions; side effects belong to the durable owner.
use super::{
    AttentionReason, DomainError, Effects, ObservationPolicy, ObservationState, ProcessState,
    ReductionState, TombstoneEvidence, TurnKey, TurnState, evidence,
};
use crate::protocol::{EventEnvelope, EventPayload, RunExitedPayload, TerminationReason};

/// Reduce a typed observation after separate admission/provenance checks.
/// # Errors
/// Rejects inconsistent domain evidence and unsupported observation kinds.
pub fn reduce(
    state: &ReductionState,
    event: &EventEnvelope,
    policy: &ObservationPolicy,
) -> Result<Effects, DomainError> {
    evidence::validate(state, event, policy)?;
    let mut effects = Effects {
        projection: state.projection.clone(),
        current_turn: state.current_turn.clone(),
        attention: Vec::new(),
        resolve_transient: None,
        tombstone: None,
        reconciliation_required: false,
    };
    if policy.freshness() != ObservationState::Fresh {
        uncertain(&mut effects, policy.freshness(), true);
    }
    if let Some(key) = evidence::turn_key(event) {
        reduce_turn(state, event, key, policy, &mut effects);
    } else {
        reduce_run(event, policy, &mut effects);
    }
    Ok(effects)
}

fn reduce_turn(
    state: &ReductionState,
    event: &EventEnvelope,
    key: TurnKey,
    policy: &ObservationPolicy,
    effects: &mut Effects,
) {
    let observed = evidence::turn_state(&event.event);
    if let Some(terminal) = &state.tombstone {
        if observed.is_terminal() && terminal.terminal != Some(observed) {
            uncertain(effects, stale_unless_explicit(policy), false);
        }
        return;
    }
    if observed.is_terminal() {
        terminal(event, key, observed, effects);
    } else if policy.freshness() != ObservationState::Fresh {
        uncertain(effects, policy.freshness(), true);
    } else if matches!(
        effects.projection.process,
        ProcessState::Exited | ProcessState::Zombie
    ) {
        uncertain(effects, ObservationState::Stale, true);
    } else {
        progress(key, observed, effects);
    }
}

fn progress(key: TurnKey, observed: TurnState, effects: &mut Effects) {
    let same = effects.current_turn.as_ref() == Some(&key);
    if !same && effects.current_turn.is_some() && !effects.projection.turn.is_terminal() {
        uncertain(effects, ObservationState::Stale, false);
        return;
    }
    if same && observed == TurnState::Working && effects.projection.turn != TurnState::Working {
        // A repeated start can have been delayed past a native wait. Transport
        // order alone cannot prove that approval/input was supplied.
        return;
    }
    if !same || effects.projection.turn != observed {
        if !same {
            effects.resolve_transient = effects.current_turn.clone();
        }
        match observed {
            TurnState::AwaitingInput => effects.attention.push(AttentionReason::Input),
            TurnState::AwaitingApproval => effects.attention.push(AttentionReason::NativeApproval),
            _ => {}
        }
    }
    if !same || effects.projection.turn != TurnState::AwaitingApproval {
        effects.projection.turn = observed;
    }
    effects.projection.turn_id = Some(key.turn_id.clone());
    effects.projection.observation = ObservationState::Fresh;
    effects.current_turn = Some(key);
}

fn terminal(event: &EventEnvelope, key: TurnKey, observed: TurnState, effects: &mut Effects) {
    effects.tombstone = Some(TombstoneEvidence {
        key: key.clone(),
        outcome_id: event.event_id.clone(),
        terminal: Some(observed),
    });
    effects.resolve_transient = Some(key.clone());
    effects.attention.push(if observed == TurnState::Completed {
        AttentionReason::Completion
    } else {
        AttentionReason::Failure
    });
    if effects
        .current_turn
        .as_ref()
        .is_none_or(|current| current == &key)
    {
        effects.projection.turn = observed;
        effects.projection.turn_id = Some(key.turn_id.clone());
        effects.current_turn = Some(key);
    }
}

fn reduce_run(event: &EventEnvelope, policy: &ObservationPolicy, effects: &mut Effects) {
    match &event.event {
        EventPayload::RunExited(exit) => {
            effects.projection.process = match exit {
                RunExitedPayload::ExitCode { .. }
                | RunExitedPayload::Terminated {
                    reason: TerminationReason::Signaled,
                } => ProcessState::Exited,
                RunExitedPayload::Terminated { .. } => ProcessState::Unknown,
            };
            uncertain(effects, stale_unless_explicit(policy), true);
        }
        EventPayload::ProducerHealth(health) => {
            let missing = crate::protocol::EventKind::ALL
                .iter()
                .any(|signal| policy.supports(*signal) && !health.signals.contains(signal));
            if missing || policy.freshness() != ObservationState::Fresh {
                let freshness = if missing {
                    ObservationState::Unsupported
                } else {
                    policy.freshness()
                };
                uncertain(effects, freshness, true);
            }
        }
        _ => {}
    }
}

fn uncertain(effects: &mut Effects, freshness: ObservationState, lose_current: bool) {
    if !effects
        .attention
        .contains(&AttentionReason::ConnectionUncertainty)
    {
        effects
            .attention
            .push(AttentionReason::ConnectionUncertainty);
    }
    effects.projection.observation = freshness;
    if lose_current && !effects.projection.turn.is_terminal() {
        effects.projection.turn = TurnState::Unknown;
    }
    effects.reconciliation_required = true;
}

fn stale_unless_explicit(policy: &ObservationPolicy) -> ObservationState {
    if policy.freshness() == ObservationState::Fresh {
        ObservationState::Stale
    } else {
        policy.freshness()
    }
}
