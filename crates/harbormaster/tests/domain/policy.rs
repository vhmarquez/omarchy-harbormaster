use super::common::*;
use harbormaster::protocol::HealthPayload;

#[test]
fn contradictory_current_evidence_cannot_revive_or_preserve_false_working() {
    let incoming = event(EventPayload::TurnStarted(turn("T")));
    let mut before = state(Some("T"), TurnState::Working);
    terminal(&mut before, "T", Some(TurnState::Completed));
    assert_eq!(
        reduce(&before, &incoming, &policy()),
        Err(DomainError::InvalidEvidence)
    );
    before = state(Some("T"), TurnState::Completed);
    assert_eq!(
        reduce(&before, &incoming, &policy()),
        Err(DomainError::InvalidEvidence)
    );
    before = state(None, TurnState::Working);
    assert_eq!(
        reduce(&before, &incoming, &policy()),
        Err(DomainError::InvalidEvidence)
    );
}

#[test]
fn missing_declared_hooks_degrade_but_health_cannot_restore_current_turn_proof() {
    let before = state(Some("T"), TurnState::Working);
    let incoming = event(EventPayload::ProducerHealth(HealthPayload {
        adapter_version: "fixture-1".parse().unwrap(),
        signals: vec![EventKind::ProducerHealth],
    }));
    let result = reduce(&before, &incoming, &policy()).unwrap();
    assert_eq!(result.projection.observation, ObservationState::Unsupported);
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert!(result.reconciliation_required);
    let before = ReductionState {
        projection: result.projection,
        current_turn: result.current_turn,
        tombstone: None,
    };
    let incoming = event(EventPayload::ProducerHealth(HealthPayload {
        adapter_version: "fixture-1".parse().unwrap(),
        signals: EventKind::ALL.to_vec(),
    }));
    let result = reduce(&before, &incoming, &policy()).unwrap();
    assert_eq!(result.projection.observation, ObservationState::Unsupported);
    assert_eq!(result.projection.turn, TurnState::Unknown);
}

#[test]
fn unsupported_signals_cannot_be_enabled_by_health_claims() {
    let restricted =
        ObservationPolicy::new(&[EventKind::ProducerHealth], ObservationState::Unsupported)
            .unwrap();
    let before = state(None, TurnState::Unknown);
    let incoming = event(EventPayload::ProducerHealth(HealthPayload {
        adapter_version: "fixture-1".parse().unwrap(),
        signals: EventKind::ALL.to_vec(),
    }));
    let result = reduce(&before, &incoming, &restricted).unwrap();
    assert_eq!(result.projection.observation, ObservationState::Unsupported);
    assert_eq!(result.projection.turn, TurnState::Unknown);
    assert!(!restricted.supports(EventKind::TurnStarted));
    assert_eq!(
        reduce(
            &before,
            &event(EventPayload::TurnStarted(turn("T"))),
            &restricted
        ),
        Err(DomainError::UnsupportedSignal)
    );
}

#[test]
fn stale_policy_does_not_claim_working_or_manufacture_success() {
    let stale = ObservationPolicy::new(&EventKind::ALL, ObservationState::Stale).unwrap();
    let before = state(None, TurnState::Unknown);
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
fn unchanged_health_is_deterministic_and_has_no_attention_or_policy_side_effect() {
    let before = state(Some("T"), TurnState::Working);
    let policy = policy();
    let incoming = event(EventPayload::ProducerHealth(HealthPayload {
        adapter_version: "fixture-1".parse().unwrap(),
        signals: EventKind::ALL.to_vec(),
    }));
    for _ in 0..100 {
        let result = reduce(&before, &incoming, &policy).unwrap();
        assert_eq!(result.projection, before.projection);
        assert!(result.attention.is_empty());
        assert!(result.resolve_transient.is_none());
        assert!(result.tombstone.is_none());
    }
    assert_eq!(policy, super::common::policy());
}

#[test]
fn inconsistent_tombstone_and_projection_evidence_rejects_before_effects() {
    let mut before = state(Some("T"), TurnState::Working);
    terminal(&mut before, "T", Some(TurnState::Working));
    assert_eq!(
        reduce(
            &before,
            &event(EventPayload::TurnStarted(turn("T"))),
            &policy()
        ),
        Err(DomainError::InvalidEvidence)
    );
    before.tombstone = None;
    before.current_turn = Some(key("U"));
    assert_eq!(
        reduce(
            &before,
            &event(EventPayload::TurnStarted(turn("T"))),
            &policy()
        ),
        Err(DomainError::InvalidEvidence)
    );
}

#[test]
fn policy_rejects_duplicate_signals() {
    assert_eq!(
        ObservationPolicy::new(&[EventKind::TurnStarted; 2], ObservationState::Fresh),
        Err(DomainError::InvalidEvidence)
    );
}
