use super::common::*;

#[test]
fn different_turn_progress_without_a_terminal_cannot_replace_current_attention() {
    for payload in [
        EventPayload::TurnStarted(turn("T")),
        EventPayload::TurnAwaitingInput(turn("T")),
    ] {
        let before = state(Some("U"), TurnState::AwaitingApproval);
        let result = reduce(&before, &event(payload), &policy()).unwrap();
        assert_eq!(result.current_turn, Some(key("U")));
        assert_eq!(result.projection.turn, TurnState::AwaitingApproval);
        assert_eq!(result.projection.observation, ObservationState::Stale);
        assert!(result.reconciliation_required);
        assert!(result.resolve_transient.is_none());
    }
}

#[test]
fn stale_policy_applies_to_historical_terminals_without_losing_the_outcome() {
    let stale = ObservationPolicy::new(&EventKind::ALL, ObservationState::Disconnected).unwrap();
    let before = state(Some("U"), TurnState::Working);
    let result = reduce(
        &before,
        &event(EventPayload::TurnCompleted(turn("T"))),
        &stale,
    )
    .unwrap();
    assert_eq!(result.current_turn, Some(key("U")));
    assert_eq!(
        result.projection.observation,
        ObservationState::Disconnected
    );
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert!(result.reconciliation_required);
    assert!(result.attention.contains(&AttentionReason::Completion));
    assert!(
        result
            .attention
            .contains(&AttentionReason::ConnectionUncertainty)
    );
    assert_eq!(result.tombstone.unwrap().key, key("T"));
}

#[test]
fn terminal_dominance_does_not_bypass_current_freshness_policy() {
    let stale = ObservationPolicy::new(&EventKind::ALL, ObservationState::Stale).unwrap();
    let mut before = state(Some("U"), TurnState::Working);
    terminal(&mut before, "T", Some(TurnState::Completed));
    let result = reduce(
        &before,
        &event(EventPayload::TurnStarted(turn("T"))),
        &stale,
    )
    .unwrap();
    assert_eq!(result.projection.observation, ObservationState::Stale);
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert!(result.reconciliation_required);
    assert!(result.tombstone.is_none());
}

#[test]
fn terminal_tombstones_dominate_late_start_and_wait_even_at_u64_max() {
    for payload in [
        EventPayload::TurnStarted(turn("T")),
        EventPayload::TurnAwaitingInput(turn("T")),
        EventPayload::TurnAwaitingApproval(turn("T")),
    ] {
        let mut before = state(Some("T"), TurnState::Completed);
        terminal(&mut before, "T", Some(TurnState::Completed));
        let mut incoming = event(payload);
        incoming.seq = Seq::new(u64::MAX);
        let result = reduce(&before, &incoming, &policy()).unwrap();
        assert_eq!(result.projection, before.projection);
        assert!(result.attention.is_empty());
        assert!(result.tombstone.is_none());
        assert!(!result.reconciliation_required);
    }
}

#[test]
fn late_terminal_keeps_current_newer_turn_and_its_transient_attention() {
    let before = state(Some("U"), TurnState::AwaitingApproval);
    let result = reduce(
        &before,
        &event(EventPayload::TurnCompleted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection, before.projection);
    assert_eq!(result.current_turn, Some(key("U")));
    assert_eq!(result.resolve_transient, Some(key("T")));
    assert_eq!(result.tombstone.unwrap().key, key("T"));
    assert_eq!(result.attention, vec![AttentionReason::Completion]);
}

#[test]
fn genuinely_new_turn_can_start_after_terminal_without_reviewing_old_outcome() {
    let before = state(Some("T"), TurnState::Completed);
    let result = reduce(
        &before,
        &event(EventPayload::TurnStarted(turn("U"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection.turn, TurnState::Working);
    assert_eq!(result.current_turn, Some(key("U")));
    assert!(result.attention.is_empty());
    assert!(result.tombstone.is_none());
}

#[test]
fn conflicting_terminal_preserves_first_outcome_and_requests_reconciliation() {
    let mut before = state(Some("U"), TurnState::Working);
    terminal(&mut before, "T", Some(TurnState::Completed));
    let result = reduce(
        &before,
        &event(EventPayload::TurnInterrupted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(result.projection.turn, TurnState::Working);
    assert_eq!(result.current_turn, Some(key("U")));
    assert_eq!(result.projection.observation, ObservationState::Stale);
    assert_eq!(
        result.attention,
        vec![AttentionReason::ConnectionUncertainty]
    );
    assert!(result.tombstone.is_none());
    assert!(result.reconciliation_required);
}

#[test]
fn repeated_terminal_has_no_second_outcome_even_with_new_event_identity() {
    let mut before = state(Some("T"), TurnState::Completed);
    terminal(&mut before, "T", Some(TurnState::Completed));
    let result = reduce(
        &before,
        &event(EventPayload::TurnCompleted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert!(result.attention.is_empty());
    assert!(result.tombstone.is_none());
    assert!(!result.reconciliation_required);
}

#[test]
fn pruned_legacy_terminal_still_protects_and_cannot_be_guessed() {
    let mut before = state(Some("T"), TurnState::Unknown);
    terminal(&mut before, "T", None);
    let late = reduce(
        &before,
        &event(EventPayload::TurnStarted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(late.projection.turn, TurnState::Unknown);
    assert!(late.tombstone.is_none());
    let terminal = reduce(
        &before,
        &event(EventPayload::TurnCompleted(turn("T"))),
        &policy(),
    )
    .unwrap();
    assert_eq!(terminal.projection.turn, TurnState::Unknown);
    assert!(terminal.reconciliation_required);
    assert!(terminal.tombstone.is_none());
}

#[test]
fn cross_producer_or_generation_sequence_is_not_turn_authority() {
    let before = state(Some("T"), TurnState::Working);
    let mut incoming = event(EventPayload::TurnStarted(turn("U")));
    incoming.producer_id = "00000000-0000-4000-8000-000000000099".parse().unwrap();
    assert_eq!(
        reduce(&before, &incoming, &policy()),
        Err(DomainError::InvalidScope)
    );
    incoming.producer_id = key("T").producer_id;
    incoming.generation = "00000000-0000-4000-8000-000000000088".parse().unwrap();
    assert_eq!(
        reduce(&before, &incoming, &policy()),
        Err(DomainError::InvalidScope)
    );
}
