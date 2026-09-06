use super::*;
use super::{reducer::{apply_set, reconcile}, reducer_attention::waiting};

#[test]
fn late_sql_failure_preserves_entire_context_and_original_outcome() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let original = write(generation.clone(), revision, 1, 5);
    engine.execute(&Request::Apply(Box::new(waiting(original.clone())))).unwrap();
    let before = context(&mut engine, &original.fact);
    let outcome_before = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&original)).unwrap();
    engine.connection.as_ref().unwrap().execute_batch(
        "CREATE TRIGGER independent_late_failure BEFORE INSERT ON facts BEGIN SELECT RAISE(ABORT,'synthetic'); END"
    ).unwrap();
    let terminal = apply_set(write(generation, Revision::new(2), 2, 6));
    assert!(engine.execute(&Request::Apply(Box::new(terminal.clone()))).is_err());
    assert_eq!(
        context(&mut engine, &original.fact),
        before,
        "all projection, current scope, producer checkpoint, revision and receipt fields must roll back"
    );
    assert_eq!(queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&original)).unwrap(), outcome_before);
    assert_eq!(count(&engine, "facts"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
    assert_eq!(count(&engine, "attention"), 1);
    assert_eq!(count(&engine, "tombstones"), 0);
    assert!(queries::outcome(engine.connection.as_ref().unwrap(), &OutcomeKey {
        producer_id: terminal.fact.producer_id,
        generation: terminal.fact.generation,
        event_id: terminal.fact.event_id,
    }).unwrap().is_none());
}

fn context(engine: &mut Engine, fact: &EventEnvelope) -> ReducerContext {
    let Response::Context(value) = engine.execute(&Request::Context(Box::new(fact.clone()))).unwrap() else {
        panic!("context response")
    };
    *value
}
