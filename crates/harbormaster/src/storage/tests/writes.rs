use super::*;

#[test]
fn all_effects_roll_back_after_late_sql_failure() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    engine.connection.as_ref().unwrap().execute_batch("CREATE TRIGGER fixture_fault BEFORE INSERT ON outbox BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(
        engine
            .execute(&Request::Commit(Box::new(set.clone())))
            .is_err()
    );
    for table in ["facts", "projections", "attention", "outbox", "tombstones"] {
        assert_eq!(count(&engine, table), 0);
    }
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        revision
    );
    assert_eq!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .next_sequence,
        Some(Seq::new(1))
    );
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch("DROP TRIGGER fixture_fault")
        .unwrap();
    assert_eq!(
        commit(&mut engine, &set),
        Response::Committed {
            revision: Revision::new(2),
            duplicate: false
        }
    );
    for table in ["facts", "projections", "attention", "outbox", "tombstones"] {
        assert_eq!(count(&engine, table), 1);
    }
}

#[test]
fn duplicates_require_identical_complete_write_set() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    commit(&mut engine, &set);
    assert_eq!(
        commit(&mut engine, &set),
        Response::Committed {
            revision: Revision::new(2),
            duplicate: true
        }
    );
    let mut altered = set.clone();
    altered.projection.process = ProcessState::Exited;
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(altered))),
        Err(StorageError::Conflict)
    );
    let mut altered = set;
    altered.attention[0].reviewed = true;
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(altered))),
        Err(StorageError::Conflict)
    );
    assert_eq!(count(&engine, "facts"), 1);
}

#[test]
fn scope_gap_and_stale_revision_never_change_checkpoint() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 10);
    let mut set = write(generation, revision, 11, 3);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set.clone()))),
        Err(StorageError::SequenceGap)
    );
    set.fact.seq = Seq::new(10);
    set.fact.generation = id(99);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set.clone()))),
        Err(StorageError::StaleGeneration)
    );
    set.expected_revision = Revision::new(0);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set))),
        Err(StorageError::StaleRevision)
    );
    assert_eq!(count(&engine, "facts"), 0);
}

#[test]
fn full_u64_sequence_and_revision_are_lossless_and_checked() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute(
            "UPDATE metadata SET revision=?1",
            [(u64::MAX - 2).to_be_bytes().as_slice()],
        )
        .unwrap();
    let (generation, revision) = register(&mut engine, u64::MAX);
    assert_eq!(revision.value(), u64::MAX - 1);
    let set = write(generation, revision, u64::MAX, 3);
    assert_eq!(
        commit(&mut engine, &set),
        Response::Committed {
            revision: Revision::new(u64::MAX),
            duplicate: false
        }
    );
    assert_eq!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .next_sequence,
        None
    );
    assert_eq!(
        engine.execute(&Request::MarkReviewed(ReviewUpdate {
            expected_revision: Revision::new(u64::MAX),
            outcome: outcome(&set)
        })),
        Err(StorageError::ResourceExhausted)
    );
    assert!(
        !queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&set))
            .unwrap()
            .unwrap()
            .attention[0]
            .reviewed
    );
}

#[test]
fn event_identity_is_scoped_to_producer_generation() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let first = write(generation, revision, 1, 3);
    commit(&mut engine, &first);
    let Response::Registered {
        generation,
        revision,
    } = engine
        .execute(&Request::Register(Registration {
            expected_revision: Revision::new(2),
            producer_id: id(4),
            run_id: id(5),
            harness: HarnessKind::Claude,
            next_sequence: Seq::new(1),
            previous_generation: None,
        }))
        .unwrap()
    else {
        panic!()
    };
    let mut second = write(generation, revision, 1, 3);
    second.fact.producer_id = id(4);
    second.fact.run_id = id(5);
    second.projection.run_id = id(5);
    commit(&mut engine, &second);
    assert_eq!(count(&engine, "facts"), 2);
    engine
        .execute(&Request::MarkReviewed(ReviewUpdate {
            expected_revision: Revision::new(4),
            outcome: outcome(&first),
        }))
        .unwrap();
    assert!(
        queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&first))
            .unwrap()
            .unwrap()
            .attention[0]
            .reviewed
    );
    assert!(
        !queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&second))
            .unwrap()
            .unwrap()
            .attention[0]
            .reviewed
    );
}
