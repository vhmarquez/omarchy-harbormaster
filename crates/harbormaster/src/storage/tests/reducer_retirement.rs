use super::reducer::{apply_set, reconcile};
use super::*;

#[test]
fn inconsistent_apply_projection_is_rejected_before_any_effect() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let mut set = apply_set(write(generation, revision, 1, 5));
    set.effects.current_turn = None;
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set.clone()))),
        Err(StorageError::InvalidRequest)
    );
    set.effects.projection.turn_id = None;
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set))),
        Err(StorageError::InvalidRequest)
    );
    assert_eq!(count(&engine, "facts"), 0);
    assert_eq!(count(&engine, "tombstones"), 0);
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(1)
    );
}
#[test]
fn explicit_retirement_preserves_terminal_process_checkpoint_and_outcome() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let original = write(generation.clone(), revision, 1, 5);
    engine
        .execute(&Request::Apply(Box::new(apply_set(original.clone()))))
        .unwrap();
    let request = Request::RetireProducer(ProducerRetirement {
        expected_revision: Revision::new(2),
        producer_id: id(1),
        generation: generation.clone(),
        discarded_events: 7,
    });
    assert_eq!(
        engine.execute(&request),
        Ok(Response::Retired {
            revision: Revision::new(3)
        })
    );
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(original.fact.clone())))
        .unwrap()
    else {
        panic!("context")
    };
    let record = context.producer.unwrap();
    assert!(!record.active && !record.reconciled);
    assert_eq!(record.generation, generation);
    assert_eq!(record.next_sequence, Some(Seq::new(2)));
    assert_eq!(context.state.projection.turn, TurnState::Completed);
    assert_eq!(context.state.projection.process, ProcessState::Alive);
    assert_eq!(
        context.state.projection.observation,
        ObservationState::Stale
    );
    assert_eq!(
        context.receipt,
        EventReceipt::Exact {
            revision: Revision::new(2)
        }
    );
    assert_eq!(count(&engine, "facts"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
    let Response::Status(status) = engine.execute(&Request::Status).unwrap() else {
        panic!("status")
    };
    assert_eq!(status.discarded_events, 7);
    assert_eq!(engine.execute(&request), Err(StorageError::StaleRevision));
}
