use super::reducer::{apply_set, reconcile};
use super::*;

#[test]
fn legacy_registration_is_not_a_reconciled_baseline() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = apply_set(write(generation, revision, 1, 5));
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set))),
        Err(StorageError::StaleGeneration)
    );
    assert_eq!(count(&engine, "facts"), 0);
}
#[test]
fn reconciliation_requires_fresh_concrete_evidence_and_one_active_producer() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, _) = reconcile(&mut engine);
    let registration = Registration {
        expected_revision: Revision::new(1),
        producer_id: id(1),
        run_id: id(2),
        harness: HarnessKind::Hermes,
        next_sequence: Seq::new(10),
        previous_generation: Some(generation.clone()),
    };
    for (process, observation, turn, current) in invalid_baselines() {
        let current_turn = current.map(|value| value.parse().unwrap());
        let request = Request::Reconcile(Box::new(Reconciliation {
            registration: registration.clone(),
            baseline: ReconciliationBaseline::Verified {
                projection: RunProjection {
                    run_id: id(2),
                    process,
                    observation,
                    turn,
                    turn_id: current_turn.clone(),
                },
                current_turn,
            },
            discarded_events: 0,
            resolved_turn: None,
        }));
        assert_eq!(engine.execute(&request), Err(StorageError::InvalidRequest));
    }
    let mut another = registration.clone();
    another.producer_id = id(99);
    another.previous_generation = None;
    assert_eq!(
        engine.execute(&Request::Register(another)),
        Err(StorageError::Conflict)
    );
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(1)
    );
    assert_eq!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .generation,
        generation
    );
}
#[test]
fn cleanup_preview_rejects_revision_change_and_policy_reads_do_not_mutate() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let Response::CleanupPreview(preview) = engine
        .execute(&Request::CleanupPreview { now: DAY })
        .unwrap()
    else {
        panic!("preview")
    };
    engine.execute(&Request::Status).unwrap();
    engine.execute(&Request::Policy).unwrap();
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(0)
    );
    engine
        .execute(&Request::UpdatePolicy {
            expected_revision: Revision::new(0),
            history: HistoryRetention::SevenDays,
        })
        .unwrap();
    assert_eq!(
        engine.execute(&Request::ApplyCleanup(Box::new(preview))),
        Err(StorageError::StaleRevision)
    );
    assert_eq!(
        engine.execute(&Request::Policy),
        Ok(Response::Policy(StoragePolicy {
            revision: Revision::new(1),
            history: HistoryRetention::SevenDays
        }))
    );
}

#[test]
fn cleanup_preview_cannot_cross_an_engine_instance() {
    let first = Fixture::new();
    let second = Fixture::new();
    let mut a = first.open();
    let mut b = second.open();
    let Response::CleanupPreview(preview) =
        a.execute(&Request::CleanupPreview { now: DAY }).unwrap()
    else {
        panic!("preview")
    };
    assert_eq!(
        b.execute(&Request::ApplyCleanup(Box::new(preview.clone()))),
        Err(StorageError::Conflict)
    );
    assert_eq!(
        queries::revision(b.connection.as_ref().unwrap()).unwrap(),
        Revision::new(0)
    );
    drop(a);
    let mut reopened = first.open();
    assert_eq!(
        reopened.execute(&Request::ApplyCleanup(Box::new(preview))),
        Err(StorageError::Conflict)
    );
}

fn invalid_baselines() -> [(
    ProcessState,
    ObservationState,
    TurnState,
    Option<&'static str>,
); 4] {
    [
        (
            ProcessState::Unknown,
            ObservationState::Fresh,
            TurnState::Unknown,
            None,
        ),
        (
            ProcessState::Alive,
            ObservationState::Stale,
            TurnState::Working,
            Some("T"),
        ),
        (
            ProcessState::Alive,
            ObservationState::Fresh,
            TurnState::Completed,
            Some("T"),
        ),
        (
            ProcessState::Alive,
            ObservationState::Fresh,
            TurnState::Working,
            None,
        ),
    ]
}
