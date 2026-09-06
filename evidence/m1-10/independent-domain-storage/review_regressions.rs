use super::common::*;
use harbormaster::protocol::{RunExitedPayload, TerminationReason};

fn uncertain_exit(reason: TerminationReason) {
    let before = state(Some("T"), TurnState::Working);
    let incoming = event(EventPayload::RunExited(RunExitedPayload::Terminated {
        reason,
    }));
    let result = reduce(&before, &incoming, &policy()).unwrap();
    assert_ne!(
        result.projection.process,
        ProcessState::Exited,
        "lost or unknown observation does not establish process termination"
    );
    assert_eq!(result.projection.observation, ObservationState::Stale);
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert!(result.reconciliation_required);
    assert!(result.attention.contains(&AttentionReason::ConnectionUncertainty));
    assert!(result.tombstone.is_none());
    assert!(result.resolve_transient.is_none());
}

#[test]
fn lost_observation_cannot_establish_process_exit() {
    uncertain_exit(TerminationReason::Lost);
}

#[test]
fn unknown_observation_cannot_establish_process_exit() {
    uncertain_exit(TerminationReason::Unknown);
}

#[test]
fn unknown_terminal_kind_cannot_preserve_live_current_work() {
    let mut before = state(Some("T"), TurnState::Working);
    terminal(&mut before, "T", None);
    let incoming = event(EventPayload::TurnStarted(turn("T")));
    match reduce(&before, &incoming, &policy()) {
        Err(DomainError::InvalidEvidence) => (),
        Ok(result) => {
            assert_eq!(
                result.projection.turn,
                TurnState::Unknown,
                "an unknown terminal kind still excludes current live work"
            );
            assert_ne!(result.projection.observation, ObservationState::Fresh);
            assert!(result.reconciliation_required);
            assert!(result.tombstone.is_none());
            assert!(result.resolve_transient.is_none());
        }
        Err(other) => panic!("unexpected rejection: {other:?}"),
    }
}

fn contradiction_preserves_policy(freshness: ObservationState) {
    let mut before = state(Some("U"), TurnState::Working);
    terminal(&mut before, "T", Some(TurnState::Completed));
    let observed = ObservationPolicy::new(&EventKind::ALL, freshness).unwrap();
    let result = reduce(
        &before,
        &event(EventPayload::TurnInterrupted(turn("T"))),
        &observed,
    )
    .unwrap();
    assert_eq!(
        result.projection.observation,
        freshness,
        "a contradictory historical terminal cannot replace explicit observation policy"
    );
    assert_eq!(result.current_turn, Some(key("U")));
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert_eq!(result.attention, vec![AttentionReason::ConnectionUncertainty]);
    assert!(result.reconciliation_required);
    assert!(result.tombstone.is_none());
    assert!(result.resolve_transient.is_none());
}

#[test]
fn terminal_contradiction_preserves_disconnected_observation() {
    contradiction_preserves_policy(ObservationState::Disconnected);
}

#[test]
fn terminal_contradiction_preserves_unsupported_observation() {
    contradiction_preserves_policy(ObservationState::Unsupported);
}
