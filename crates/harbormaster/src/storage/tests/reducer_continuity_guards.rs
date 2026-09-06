use super::*;
use super::{
    reducer::{apply_set, reconcile},
    reducer_continuity::{approval, continuation},
};

#[test]
fn a_verified_wait_without_an_event_is_a_current_projection_not_a_fabricated_outcome() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, _) = reconcile(&mut engine);
    let mut request = continuation(&engine, generation);
    let ReconciliationBaseline::Verified {
        projection,
        current_turn,
    } = &mut request.baseline
    else {
        panic!("baseline")
    };
    projection.turn = TurnState::AwaitingApproval;
    projection.turn_id = Some("fixture-turn".parse().unwrap());
    *current_turn = projection.turn_id.clone();
    let Response::Registered {
        generation,
        revision,
    } = engine
        .execute(&Request::Reconcile(Box::new(request)))
        .unwrap()
    else {
        panic!("registered")
    };
    let fact = approval(write(generation, revision, 1, 5)).fact;
    let Response::Context(context) = engine.execute(&Request::Context(Box::new(fact))).unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.state.projection.turn, TurnState::AwaitingApproval);
    assert_eq!(
        context.state.projection.observation,
        ObservationState::Fresh
    );
    assert_eq!(context.receipt, EventReceipt::Missing);
    assert_eq!(count(&engine, "facts"), 0);
    assert_eq!(count(&engine, "attention"), 0);
    assert_eq!(count(&engine, "outbox"), 0);
}

#[test]
fn missing_continuity_keeps_prior_wait_protected_and_old_callbacks_retired() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let origin = write(generation.clone(), revision, 1, 5);
    engine
        .execute(&Request::Apply(Box::new(approval(origin.clone()))))
        .unwrap();
    let mut request = continuation(&engine, generation.clone());
    request.continued_turn = None;
    let Response::Registered {
        generation: fresh,
        revision,
    } = engine
        .execute(&Request::Reconcile(Box::new(request)))
        .unwrap()
    else {
        panic!("registered")
    };
    engine
        .execute(&Request::Apply(Box::new(apply_set(write(
            fresh, revision, 1, 6,
        )))))
        .unwrap();
    let record = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&origin))
        .unwrap()
        .unwrap();
    assert!(record.resolved_reasons.is_empty());
    assert!(!record.attention[0].reviewed);
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(approval(write(
            generation,
            Revision::new(4),
            2,
            7
        ))))),
        Err(StorageError::StaleGeneration)
    );
}
#[test]
fn wrong_continuity_scope_or_working_baseline_cannot_rotate_a_generation() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    engine
        .execute(&Request::Apply(Box::new(approval(write(
            generation.clone(),
            revision,
            1,
            5,
        )))))
        .unwrap();
    let valid = continuation(&engine, generation);
    for change in 0..6 {
        let mut request = valid.clone();
        let key = request.continued_turn.as_mut().unwrap();
        match change {
            0 => key.producer_id = id(99),
            1 => key.run_id = id(99),
            2 => key.generation = id(99),
            3 => key.turn_id = "another".parse().unwrap(),
            4 => request.baseline = ReconciliationBaseline::Unavailable,
            _ => {
                let ReconciliationBaseline::Verified { projection, .. } = &mut request.baseline
                else {
                    panic!("baseline")
                };
                projection.turn = TurnState::Working;
            }
        }
        assert_eq!(
            engine.execute(&Request::Reconcile(Box::new(request))),
            Err(StorageError::InvalidRequest)
        );
    }
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(2)
    );
}
#[test]
fn a_terminal_old_current_cannot_be_continued_as_a_native_wait() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    engine
        .execute(&Request::Apply(Box::new(apply_set(write(
            generation.clone(),
            revision,
            1,
            5,
        )))))
        .unwrap();
    let mut request = continuation(&engine, generation);
    let ReconciliationBaseline::Verified { projection, .. } = &mut request.baseline else {
        panic!("baseline")
    };
    projection.turn = TurnState::AwaitingApproval;
    assert_eq!(
        engine.execute(&Request::Reconcile(Box::new(request))),
        Err(StorageError::Conflict)
    );
    assert_eq!(count(&engine, "tombstones"), 1);
}
#[test]
fn late_reconciliation_failure_rolls_back_generation_and_obligation_link() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let origin = write(generation.clone(), revision, 1, 5);
    engine
        .execute(&Request::Apply(Box::new(approval(origin.clone()))))
        .unwrap();
    let request = continuation(&engine, generation.clone());
    engine.connection.as_ref().unwrap().execute_batch("CREATE TRIGGER fixture_failure BEFORE UPDATE ON metadata BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    assert!(
        engine
            .execute(&Request::Reconcile(Box::new(request)))
            .is_err()
    );
    assert_eq!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .generation,
        generation
    );
    let link: String = engine
        .connection
        .as_ref()
        .unwrap()
        .query_row("SELECT resolution_generation FROM attention", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(link, generation.as_str());
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(2)
    );
}
