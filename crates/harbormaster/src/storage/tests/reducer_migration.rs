use super::*;
use rusqlite::params;

#[test]
fn legacy_fact_for_another_turn_does_not_invent_terminal_kind() {
    let fixture = Fixture::new();
    let engine = fixture.open();
    recovery::legacy_schema(&engine, 2);
    let mut set = write(id(9), Revision::new(1), 1, 5);
    seed(engine.connection.as_ref().unwrap(), &set);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute(
            "UPDATE tombstones SET turn_id='different-turn' WHERE event=?1",
            [set.fact.event_id.as_str()],
        )
        .unwrap();
    drop(engine);
    let mut engine = fixture.open();
    set.fact.event = EventPayload::TurnCompleted(TurnPayload {
        turn_id: "different-turn".parse().unwrap(),
    });
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(set.fact)))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.state.tombstone.unwrap().terminal, None);
}

#[test]
fn both_legacy_versions_preserve_receipt_bytes_and_known_or_unprovable_outcomes() {
    for version in [1, 2] {
        let fixture = Fixture::new();
        let engine = fixture.open();
        recovery::legacy_schema(&engine, version);
        let set = write(id(9), Revision::new(1), 1, 5);
        seed(engine.connection.as_ref().unwrap(), &set);
        drop(engine);
        let mut engine = fixture.open();
        assert_eq!(
            schema::inspect(engine.connection.as_ref().unwrap()).unwrap(),
            schema::VERSION
        );
        assert_eq!(
            schema::header(&fixture.root.join("harbormaster/state.backup.db")).unwrap(),
            version
        );
        assert_eq!(
            commit(&mut engine, &set),
            Response::Committed {
                revision: Revision::new(2),
                duplicate: true
            }
        );
        assert_evidence(&mut engine, &set);
        engine.execute(&Request::RestoreBackup).unwrap();
        assert_evidence(&mut engine, &set);
        assert_eq!(
            schema::header(&fixture.root.join("harbormaster/state.backup.db")).unwrap(),
            version
        );
    }
}
fn assert_evidence(engine: &mut Engine, set: &WriteSet) {
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
    assert_eq!(
        context.state.tombstone.unwrap().terminal,
        Some(TurnState::Completed)
    );
    assert!(
        context.state.current_turn.is_none(),
        "legacy projection has no proven scoped current turn"
    );
    assert!(!context.producer.unwrap().reconciled);
    let mut unknown = set.fact.clone();
    unknown.event_id = id(6);
    unknown.event = EventPayload::TurnCompleted(TurnPayload {
        turn_id: "unproven".parse().unwrap(),
    });
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(unknown)))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.state.tombstone.unwrap().terminal, None);
    let mut key = outcome(set);
    key.event_id = id(6);
    let Response::Outcome(Some(record)) = engine.execute(&Request::Outcome(key)).unwrap() else {
        panic!("outcome")
    };
    assert_eq!(record.outcome_revision, None);
    assert!(!record.attention[0].reviewed);
    assert!(record.resolved_reasons.is_empty());
}
fn seed(conn: &Connection, set: &WriteSet) {
    conn.execute(
        "UPDATE metadata SET revision=?1",
        [2_u64.to_be_bytes().as_slice()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO producers VALUES(?1,?2,?3,0,?4,1)",
        params![
            set.fact.producer_id.as_str(),
            set.fact.generation.as_str(),
            set.fact.run_id.as_str(),
            2_u64.to_be_bytes().as_slice()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO projections VALUES(?1,1,0,4,'fixture-turn')",
        [set.fact.run_id.as_str()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO facts VALUES(?1,?2,?3,?4,?5,100,?6,?7)",
        params![
            set.fact.event_id.as_str(),
            set.fact.producer_id.as_str(),
            set.fact.generation.as_str(),
            1_u64.to_be_bytes().as_slice(),
            set.fact.run_id.as_str(),
            serde_json::to_vec(set).unwrap(),
            2_u64.to_be_bytes().as_slice()
        ],
    )
    .unwrap();
    seed_obligations(conn, set);
}

fn seed_obligations(conn: &Connection, set: &WriteSet) {
    for (event, turn) in [
        (set.fact.event_id.clone(), "fixture-turn"),
        (id(6), "unproven"),
    ] {
        conn.execute(
            "INSERT INTO attention VALUES(?1,?2,?3,?4,4,0)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                event.as_str(),
                set.fact.run_id.as_str()
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO outbox VALUES(?1,?2,?3,?4,0,100,100)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                event.as_str(),
                set.fact.run_id.as_str()
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tombstones VALUES(?1,?2,?3,?4,?5,100)",
            params![
                set.fact.producer_id.as_str(),
                set.fact.generation.as_str(),
                set.fact.run_id.as_str(),
                turn,
                event.as_str()
            ],
        )
        .unwrap();
    }
}
