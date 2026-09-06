use super::*;

#[test]
fn protected_attention_audit_and_projection_caps_do_not_silently_evict() {
    let cases = [
        (
            "attention",
            "WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000) INSERT INTO attention(producer,generation,event,run,reason,reviewed) SELECT 'fixture','fixture',printf('%d',x),'fixture',0,0 FROM n",
            20_000,
        ),
        (
            "outbox",
            "WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000) INSERT INTO outbox(producer,generation,event,run,state,created,changed) SELECT 'fixture','fixture',printf('%d',x),'fixture',2,100,100 FROM n",
            20_000,
        ),
        (
            "projections",
            "WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO projections(run,process,observation,turn,turn_id) SELECT printf('%d',x),0,0,0,NULL FROM n",
            10_000,
        ),
    ];
    for (table, sql, maximum) in cases {
        let fixture = Fixture::new();
        let mut engine = fixture.open();
        let (generation, revision) = register(&mut engine, 1);
        engine
            .connection
            .as_ref()
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        let set = write(generation, revision, 1, 3);
        assert_eq!(
            engine.execute(&Request::Commit(Box::new(set))),
            Err(StorageError::ResourceExhausted)
        );
        assert_eq!(count(&engine, table), maximum);
        assert_eq!(count(&engine, "facts"), 0);
        assert_eq!(
            queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
            revision
        );
    }
}

#[test]
fn producer_registry_cap_rejects_registration_without_new_revision() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    engine.connection.as_ref().unwrap().execute_batch("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO producers(producer,generation,run,harness,next_seq,active) SELECT printf('%d',x),'fixture','fixture',0,X'0000000000000000',0 FROM n").unwrap();
    assert_eq!(
        engine.execute(&Request::Register(Registration {
            expected_revision: Revision::new(0),
            producer_id: id(1),
            run_id: id(2),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(1),
            previous_generation: None
        })),
        Err(StorageError::ResourceExhausted)
    );
    assert_eq!(count(&engine, "producers"), 10_000);
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(0)
    );
}

#[test]
fn fact_capacity_can_be_released_by_bounded_maintenance_only() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    engine.connection.as_ref().unwrap().execute_batch("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000) INSERT INTO facts SELECT printf('fixture-%d',x),'fixture','fixture',CAST(printf('%08d',x) AS BLOB),'fixture',100,X'7b7d',X'0000000000000000' FROM n").unwrap();
    let mut set = write(generation, revision, 1, 3);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set.clone()))),
        Err(StorageError::ResourceExhausted)
    );
    let Response::Maintained { revision, result } = engine
        .execute(&Request::Maintain {
            expected_revision: revision,
            now: 100,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(result.facts_removed, 100);
    assert_eq!(count(&engine, "facts"), 19_900);
    set.expected_revision = revision;
    commit(&mut engine, &set);
    assert_eq!(count(&engine, "facts"), 19_901);
    assert_eq!(count(&engine, "attention"), 1);
}

#[test]
fn tombstone_cap_requires_atomic_retirement_and_failed_retirement_preserves_rows() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    engine.connection.as_ref().unwrap().execute("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<20000) INSERT INTO tombstones(producer,generation,run,turn_id,event,created) SELECT ?1,?2,?3,printf('fixture-%d',x),'fixture',100 FROM n", rusqlite::params![id::<ProducerId>(1).as_str(), generation.as_str(), id::<RunId>(2).as_str()]).unwrap();
    let set = write(generation, revision, 1, 3);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set.clone()))),
        Err(StorageError::ResourceExhausted)
    );
    engine.connection.as_ref().unwrap().execute_batch("CREATE TRIGGER fixture_fault BEFORE DELETE ON tombstones BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let maintain = Request::Maintain {
        expected_revision: revision,
        now: 100,
        history: HistoryRetention::ThirtyDays,
    };
    assert!(engine.execute(&maintain).is_err());
    assert_eq!(count(&engine, "tombstones"), 20_000);
    assert!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch("DROP TRIGGER fixture_fault")
        .unwrap();
    let Response::Maintained { revision, result } = engine.execute(&maintain).unwrap() else {
        panic!()
    };
    assert_eq!(result.tombstones_removed, 100);
    assert_eq!(result.generations_retired, 1);
    let mut set = set;
    set.expected_revision = revision;
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set))),
        Err(StorageError::StaleGeneration)
    );
    assert_eq!(count(&engine, "tombstones"), 19_900);
}

#[test]
fn database_page_exhaustion_is_a_real_rollback() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, mut revision) = register(&mut engine, 1);
    let conn = engine.connection.as_ref().unwrap();
    let pages: u32 = conn
        .pragma_query_value(None, "page_count", |row| row.get(0))
        .unwrap();
    conn.pragma_update(None, "max_page_count", pages).unwrap();
    let mut exhausted = false;
    for seq in 1..=100 {
        let mut set = write(
            generation.clone(),
            revision,
            seq,
            u32::try_from(seq + 10).unwrap(),
        );
        set.tombstone = None;
        let before = count(&engine, "facts");
        match engine.execute(&Request::Commit(Box::new(set))) {
            Ok(Response::Committed { revision: next, .. }) => revision = next,
            Err(StorageError::ResourceExhausted) => {
                exhausted = true;
                assert_eq!(count(&engine, "facts"), before);
                assert_eq!(count(&engine, "attention"), before);
                assert_eq!(count(&engine, "outbox"), before);
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(
        exhausted,
        "SQLite max_page_count must cause real SQLITE_FULL"
    );
}

#[test]
fn history_and_outbox_expiry_preserve_unreviewed_obligations() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    commit(&mut engine, &set);
    assert_eq!(
        HistoryRetention::from_days(90),
        Err(StorageError::InvalidRequest)
    );
    let Response::Maintained { revision, result } = engine
        .execute(&Request::Maintain {
            expected_revision: Revision::new(2),
            now: 100 + 8 * DAY,
            history: HistoryRetention::SevenDays,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(result.facts_removed, 1);
    assert_eq!(result.deliveries_expired, 1);
    assert_eq!(result.tombstones_removed, 0);
    assert_eq!(count(&engine, "projections"), 1);
    let row = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&set))
        .unwrap()
        .unwrap();
    assert_eq!(row.delivery, Some(DeliveryState::Expired));
    assert!(!row.attention[0].reviewed);
    engine
        .execute(&Request::Maintain {
            expected_revision: revision,
            now: 100 + 31 * DAY,
            history: HistoryRetention::SevenDays,
        })
        .unwrap();
    assert_eq!(count(&engine, "tombstones"), 0);
    assert!(
        !queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
    assert_eq!(count(&engine, "attention"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
}

#[test]
fn full_pending_outbox_rolls_back_other_effects() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    engine.connection.as_ref().unwrap().execute_batch("WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) INSERT INTO outbox(producer,generation,event,run,state,created,changed) SELECT 'fixture','fixture',printf('%d',x),'fixture',0,100,100 FROM n").unwrap();
    let set = write(generation, revision, 1, 3);
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(set))),
        Err(StorageError::ResourceExhausted)
    );
    assert_eq!(count(&engine, "outbox"), 1000);
    assert_eq!(count(&engine, "facts"), 0);
    assert_eq!(count(&engine, "attention"), 0);
}

#[test]
fn blocked_real_wal_reader_backpressures_next_write() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, mut revision) = register(&mut engine, 1);
    let reader = Connection::open(fixture.database()).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM metadata;")
        .unwrap();
    let mut blocked = false;
    for sequence in 1..=1000 {
        let mut set = write(
            generation.clone(),
            revision,
            sequence,
            u32::try_from(sequence + 10).unwrap(),
        );
        set.tombstone = None;
        set.outbox = None;
        set.attention.clear();
        match engine.execute(&Request::Commit(Box::new(set))) {
            Ok(Response::Committed { revision: next, .. }) => revision = next,
            Err(StorageError::Backpressure) => {
                blocked = true;
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(
        blocked,
        "a real reader must eventually prevent checkpoint and admission"
    );
    assert!(engine.paths.length("state.db-wal").unwrap() <= schema::WAL_BOUND);
    let before = count(&engine, "facts");
    let failed = engine.execute(&Request::Maintain {
        expected_revision: revision,
        now: 100,
        history: HistoryRetention::ThirtyDays,
    });
    assert_eq!(failed, Err(StorageError::Backpressure));
    assert_eq!(count(&engine, "facts"), before);
    reader.execute_batch("ROLLBACK").unwrap();
    engine
        .execute(&Request::Maintain {
            expected_revision: revision,
            now: 100,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap();
}
