use super::*;
use crate::domain::{Effects, TombstoneEvidence, TurnKey};

pub(super) fn apply_set(set: WriteSet) -> ReducerWrite {
    let key = TurnKey {
        producer_id: set.fact.producer_id.clone(),
        generation: set.fact.generation.clone(),
        run_id: set.fact.run_id.clone(),
        turn_id: set.projection.turn_id.clone().unwrap(),
    };
    ReducerWrite {
        expected_revision: set.expected_revision,
        fact: set.fact.clone(),
        accepted_at: set.accepted_at,
        notify: true,
        discarded_events: 0,
        effects: Effects {
            projection: set.projection,
            current_turn: Some(key.clone()),
            attention: vec![AttentionReason::Completion],
            resolve_transient: Some(key.clone()),
            tombstone: Some(TombstoneEvidence {
                key,
                outcome_id: set.fact.event_id,
                terminal: Some(TurnState::Completed),
            }),
            reconciliation_required: false,
        },
    }
}

#[test]
fn context_proves_exact_retained_payload_and_retains_terminal_after_history_cleanup() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let set = apply_set(write(generation, revision, 1, 5));
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set.clone()))),
        Ok(Response::Committed {
            revision: Revision::new(2),
            duplicate: false
        })
    );
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(set.fact.clone())))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(
        context.receipt,
        EventReceipt::Exact {
            revision: Revision::new(2)
        }
    );
    assert_conflicting_payload(&mut engine, &set);
    engine
        .execute(&Request::Maintain {
            expected_revision: Revision::new(2),
            now: 8 * DAY,
            history: HistoryRetention::SevenDays,
        })
        .unwrap();
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(set.fact.clone())))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.receipt, EventReceipt::PriorSequenceUnverifiable);
    assert_eq!(context.state.tombstone, set.effects.tombstone);
    let Response::Outcome(Some(outcome)) = engine
        .execute(&Request::Outcome(OutcomeKey {
            producer_id: set.fact.producer_id,
            generation: set.fact.generation,
            event_id: set.fact.event_id,
        }))
        .unwrap()
    else {
        panic!("outcome")
    };
    assert_eq!(outcome.outcome_revision, Some(Revision::new(2)));
}

#[test]
fn policy_preview_is_read_only_and_unavailable_reconciliation_is_inactive() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    assert_eq!(
        engine.execute(&Request::Policy),
        Ok(Response::Policy(StoragePolicy {
            revision: Revision::new(0),
            history: HistoryRetention::ThirtyDays
        }))
    );
    let Response::CleanupPreview(preview) = engine
        .execute(&Request::CleanupPreview { now: DAY })
        .unwrap()
    else {
        panic!("preview")
    };
    assert_eq!(preview.revision(), Revision::new(0));
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(0)
    );
    engine
        .execute(&Request::Reconcile(Box::new(Reconciliation {
            registration: Registration {
                expected_revision: Revision::new(0),
                producer_id: id(1),
                run_id: id(2),
                harness: HarnessKind::Hermes,
                next_sequence: Seq::new(1),
                previous_generation: None,
            },
            baseline: ReconciliationBaseline::Unavailable,
            discarded_events: 3,
            resolved_turn: None,
            continued_turn: None,
        })))
        .unwrap();
    let Response::Producer(Some(producer)) = engine.execute(&Request::Producer(id(1))).unwrap()
    else {
        panic!("producer")
    };
    assert!(!producer.active);
}

pub(super) fn reconcile(engine: &mut Engine) -> (ProducerGeneration, Revision) {
    let response = engine
        .execute(&Request::Reconcile(Box::new(Reconciliation {
            registration: Registration {
                expected_revision: Revision::new(0),
                producer_id: id(1),
                run_id: id(2),
                harness: HarnessKind::Hermes,
                next_sequence: Seq::new(1),
                previous_generation: None,
            },
            baseline: ReconciliationBaseline::Verified {
                projection: RunProjection {
                    run_id: id(2),
                    process: ProcessState::Alive,
                    observation: ObservationState::Fresh,
                    turn: TurnState::Unknown,
                    turn_id: None,
                },
                current_turn: None,
            },
            discarded_events: 0,
            resolved_turn: None,
            continued_turn: None,
        })))
        .unwrap();
    let Response::Registered {
        generation,
        revision,
    } = response
    else {
        panic!("reconciled")
    };
    (generation, revision)
}

fn assert_conflicting_payload(engine: &mut Engine, set: &ReducerWrite) {
    let mut conflict = set.fact.clone();
    conflict.event = EventPayload::TurnInterrupted(TurnPayload {
        turn_id: "fixture-turn".parse().unwrap(),
    });
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(conflict)))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.receipt, EventReceipt::Conflict);
}
