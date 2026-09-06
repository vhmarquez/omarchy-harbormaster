use super::common::*;
use harbormaster::protocol::{
    FailureReason, RunExitedPayload, RunStartedPayload, TurnFailedPayload,
};

#[test]
fn new_turn_sets_working_without_changing_process_authority() {
    let mut before = state(None, TurnState::Unknown);
    before.projection.process = ProcessState::Unknown;
    let result = reduce(
        &before,
        &event(EventPayload::TurnStarted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection.turn, TurnState::Working);
    assert_eq!(result.current_turn, Some(key("T")));
    assert_eq!(result.projection.process, ProcessState::Unknown);
    assert!(result.attention.is_empty());
}

#[test]
fn delayed_same_turn_start_cannot_resolve_native_approval() {
    let before = state(Some("T"), TurnState::AwaitingApproval);
    let result = reduce(
        &before,
        &event(EventPayload::TurnStarted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection.turn, TurnState::AwaitingApproval);
    assert!(result.resolve_transient.is_none());
    assert!(result.attention.is_empty());
}

#[test]
fn waits_request_native_attention_once_per_scoped_reason() {
    for (payload, expected, reason) in [
        (
            EventPayload::TurnAwaitingInput(turn("T")),
            TurnState::AwaitingInput,
            AttentionReason::Input,
        ),
        (
            EventPayload::TurnAwaitingApproval(turn("T")),
            TurnState::AwaitingApproval,
            AttentionReason::NativeApproval,
        ),
    ] {
        let before = state(Some("T"), TurnState::Working);
        let first = reduce(&before, &event(payload.clone()), &policy()).unwrap();
        assert_eq!(first.projection.turn, expected);
        assert_eq!(first.attention, vec![reason]);
        let after = ReductionState {
            projection: first.projection,
            current_turn: first.current_turn,
            tombstone: None,
        };
        let again = reduce(&after, &event(payload), &policy()).unwrap();
        assert!(again.attention.is_empty());
        assert!(again.resolve_transient.is_none());
    }
}

#[test]
fn terminal_facts_preserve_distinct_outcomes_and_resolve_only_their_turn() {
    for (payload, expected, reason) in [
        (
            EventPayload::TurnCompleted(turn("T")),
            TurnState::Completed,
            AttentionReason::Completion,
        ),
        (
            EventPayload::TurnFailed(TurnFailedPayload {
                turn_id: turn("T").turn_id,
                reason: FailureReason::HarnessError,
            }),
            TurnState::Failed,
            AttentionReason::Failure,
        ),
        (
            EventPayload::TurnInterrupted(turn("T")),
            TurnState::Interrupted,
            AttentionReason::Failure,
        ),
    ] {
        let before = state(Some("T"), TurnState::Working);
        let incoming = event(payload);
        let result = reduce(&before, &incoming, &policy()).unwrap();
        assert_eq!(result.projection.turn, expected);
        assert_eq!(result.resolve_transient, Some(key("T")));
        assert_eq!(result.attention, vec![reason]);
        let tombstone = result.tombstone.unwrap();
        assert_eq!(tombstone.key, key("T"));
        assert_eq!(tombstone.terminal, Some(expected));
        assert_eq!(tombstone.outcome_id, incoming.event_id);
    }
}

#[test]
fn run_exit_never_invents_a_turn_completion() {
    let before = state(Some("T"), TurnState::Working);
    let result = reduce(
        &before,
        &event(EventPayload::RunExited(RunExitedPayload::ExitCode {
            exit_code: 0,
        })),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection.process, ProcessState::Exited);
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert_eq!(result.projection.observation, ObservationState::Stale);
    assert!(result.reconciliation_required);
    assert_eq!(
        result.attention,
        vec![AttentionReason::ConnectionUncertainty]
    );
    assert!(result.tombstone.is_none());
}

#[test]
fn run_started_is_observation_not_runtime_liveness_proof() {
    for process in [
        ProcessState::Unknown,
        ProcessState::Exited,
        ProcessState::Zombie,
    ] {
        let mut before = state(None, TurnState::Unknown);
        before.projection.process = process;
        let result = reduce(
            &before,
            &event(EventPayload::RunStarted(RunStartedPayload {
                harness_session_id: None,
            })),
            &policy(),
        )
        .unwrap();
        assert_eq!(result.projection.process, process);
        assert_eq!(result.projection.turn, TurnState::Unknown);
    }
}
