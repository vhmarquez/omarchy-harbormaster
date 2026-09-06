use super::*;
use super::{reducer::reconcile, reducer_attention::waiting};

#[test]
fn only_explicit_verified_old_current_progress_resolves_waits_across_reconciliation() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let original = write(generation.clone(), revision, 1, 5);
    let set = waiting(original.clone());
    engine
        .execute(&Request::Apply(Box::new(set.clone())))
        .unwrap();
    let mut request = reconciliation(&set);
    request.resolved_turn.as_mut().unwrap().turn_id = "wrong-turn".parse().unwrap();
    assert_eq!(
        engine.execute(&Request::Reconcile(Box::new(request))),
        Err(StorageError::Conflict)
    );
    let Response::Registered {
        generation: fresh,
        revision,
    } = engine
        .execute(&Request::Reconcile(Box::new(reconciliation(&set))))
        .unwrap()
    else {
        panic!("registered")
    };
    assert_ne!(fresh, generation);
    assert_eq!(revision, Revision::new(3));
    let record = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&original))
        .unwrap()
        .unwrap();
    assert_eq!(record.resolved_reasons, vec![AttentionReason::Input]);
    assert!(!record.attention[0].reviewed);
    assert_eq!(record.outcome_revision, Some(Revision::new(2)));
    assert_eq!(record.delivery, Some(DeliveryState::Pending));
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(set.fact)))
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
    assert_eq!(context.state.current_turn.unwrap().generation, fresh);
}
#[test]
fn discard_overflow_refuses_retirement_without_changing_any_admission_state() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, _) = reconcile(&mut engine);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute(
            "UPDATE metadata SET discarded=?1",
            [u64::MAX.to_be_bytes().as_slice()],
        )
        .unwrap();
    let request = ProducerRetirement {
        expected_revision: Revision::new(1),
        producer_id: id(1),
        generation,
        discarded_events: 1,
    };
    assert_eq!(
        engine.execute(&Request::RetireProducer(request.clone())),
        Err(StorageError::ResourceExhausted)
    );
    let producer = queries::producer(engine.connection.as_ref().unwrap(), &id(1))
        .unwrap()
        .unwrap();
    assert!(producer.active && producer.reconciled);
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(1)
    );
    let mut request = request;
    request.discarded_events = 0;
    engine.execute(&Request::RetireProducer(request)).unwrap();
    let Response::Status(status) = engine.execute(&Request::Status).unwrap() else {
        panic!("status")
    };
    assert_eq!(status.discarded_events, u64::MAX);
}
#[test]
fn retirement_makes_nonterminal_unknown_without_resolving_native_wait() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let set = waiting(write(generation.clone(), revision, 1, 5));
    engine
        .execute(&Request::Apply(Box::new(set.clone())))
        .unwrap();
    engine
        .execute(&Request::RetireProducer(ProducerRetirement {
            expected_revision: Revision::new(2),
            producer_id: id(1),
            generation,
            discarded_events: 0,
        }))
        .unwrap();
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(set.fact.clone())))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.state.current_turn, set.effects.current_turn);
    assert_eq!(context.state.projection.turn, TurnState::Unknown);
    assert_eq!(context.state.projection.process, ProcessState::Alive);
    let record = queries::outcome(
        engine.connection.as_ref().unwrap(),
        &OutcomeKey {
            producer_id: set.fact.producer_id,
            generation: set.fact.generation,
            event_id: set.fact.event_id,
        },
    )
    .unwrap()
    .unwrap();
    assert!(record.resolved_reasons.is_empty());
    assert!(!record.attention[0].reviewed);
}
fn reconciliation(set: &ReducerWrite) -> Reconciliation {
    let mut projection = set.effects.projection.clone();
    projection.turn = TurnState::Working;
    Reconciliation {
        registration: Registration {
            expected_revision: Revision::new(2),
            producer_id: set.fact.producer_id.clone(),
            run_id: set.fact.run_id.clone(),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(9),
            previous_generation: Some(set.fact.generation.clone()),
        },
        baseline: ReconciliationBaseline::Verified {
            current_turn: projection.turn_id.clone(),
            projection,
        },
        discarded_events: 3,
        resolved_turn: set.effects.current_turn.clone(),
        continued_turn: None,
    }
}
