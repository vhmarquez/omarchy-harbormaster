use super::reducer::{apply_set, reconcile};
use super::*;

fn working(
    generation: ProducerGeneration,
    revision: Revision,
    seq: u64,
    event: u32,
) -> ReducerWrite {
    let mut set = apply_set(write(generation, revision, seq, event));
    let turn: TurnId = "current-U".parse().unwrap();
    set.fact.event = EventPayload::TurnStarted(TurnPayload {
        turn_id: turn.clone(),
    });
    set.effects.projection.turn = TurnState::Working;
    set.effects.projection.turn_id = Some(turn.clone());
    set.effects.current_turn.as_mut().unwrap().turn_id = turn;
    set.effects.attention.clear();
    set.effects.tombstone = None;
    set.effects.resolve_transient = None;
    set
}
fn projection(engine: &Engine) -> (RunProjection, Option<crate::domain::TurnKey>) {
    super::super::super::context::projection(engine.connection.as_ref().unwrap(), &id(2)).unwrap()
}
fn assert_retired(engine: &Engine, before: &(RunProjection, Option<crate::domain::TurnKey>)) {
    let (actual, key) = projection(engine);
    assert_eq!(actual.process, before.0.process);
    assert_eq!(actual.observation, ObservationState::Stale);
    assert_eq!(
        actual.turn,
        if before.0.turn.is_terminal() {
            before.0.turn
        } else {
            TurnState::Unknown
        }
    );
    assert_eq!(actual.turn_id, before.0.turn_id);
    assert_eq!(key, before.1);
    assert!(
        !queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
}
#[test]
fn tombstone_cleanup_retires_current_projection_with_generation() {
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
    engine
        .execute(&Request::Apply(Box::new(working(
            generation,
            Revision::new(2),
            2,
            6,
        ))))
        .unwrap();
    let before = projection(&engine);
    engine
        .execute(&Request::Maintain {
            expected_revision: Revision::new(3),
            now: 31 * DAY,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap();
    assert_retired(&engine, &before);
    assert_eq!(count(&engine, "tombstones"), 0);
    assert_eq!(count(&engine, "attention"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
}
#[test]
fn startup_and_restore_cannot_keep_retired_current_work_fresh() {
    for terminal in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.open();
        let (generation, revision) = reconcile(&mut engine);
        let set = if terminal {
            apply_set(write(generation, revision, 1, 5))
        } else {
            working(generation, revision, 1, 5)
        };
        engine.execute(&Request::Apply(Box::new(set))).unwrap();
        let before = projection(&engine);
        engine.execute(&Request::Backup).unwrap();
        engine.execute(&Request::RestoreBackup).unwrap();
        assert_retired(&engine, &before);
        drop(engine);
        let engine = fixture.open();
        assert_retired(&engine, &before);
    }
}
#[test]
fn startup_retires_active_work_in_the_same_revision() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    engine
        .execute(&Request::Apply(Box::new(working(
            generation, revision, 1, 5,
        ))))
        .unwrap();
    let before = projection(&engine);
    drop(engine);
    let engine = fixture.open();
    assert_retired(&engine, &before);
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(3)
    );
}
#[test]
fn failed_cleanup_preserves_projection_retirement_and_every_table() {
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
    engine
        .execute(&Request::Apply(Box::new(working(
            generation,
            Revision::new(2),
            2,
            6,
        ))))
        .unwrap();
    let before = projection(&engine);
    engine.connection.as_ref().unwrap().execute_batch("CREATE TRIGGER fixture_failure BEFORE UPDATE ON producers BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    assert!(
        engine
            .execute(&Request::Maintain {
                expected_revision: Revision::new(3),
                now: 31 * DAY,
                history: HistoryRetention::ThirtyDays
            })
            .is_err()
    );
    assert_eq!(projection(&engine), before);
    assert_eq!(count(&engine, "facts"), 2);
    assert_eq!(count(&engine, "tombstones"), 1);
    assert_eq!(count(&engine, "attention"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(3)
    );
}
#[test]
fn inactive_legacy_projection_is_unknown_after_migration() {
    let fixture = Fixture::new();
    let engine = fixture.open();
    recovery::legacy_schema(&engine, 2);
    let conn = engine.connection.as_ref().unwrap();
    conn.execute(
        "INSERT INTO producers VALUES(?1,?2,?3,0,?4,0)",
        rusqlite::params![
            id::<ProducerId>(1).as_str(),
            id::<ProducerGeneration>(9).as_str(),
            id::<RunId>(2).as_str(),
            1_u64.to_be_bytes().as_slice()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO projections VALUES(?1,1,0,1,'current-U')",
        [id::<RunId>(2).as_str()],
    )
    .unwrap();
    drop(engine);
    let engine = fixture.open();
    let (projection, key) = projection(&engine);
    assert_eq!(projection.process, ProcessState::Alive);
    assert_eq!(projection.observation, ObservationState::Stale);
    assert_eq!(projection.turn, TurnState::Unknown);
    assert!(key.is_none());
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(1)
    );
}

#[test]
fn pruning_prior_generation_protection_preserves_fresh_current_projection() {
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
    let mut request = super::reducer_continuity::continuation(&engine, generation);
    request.continued_turn = None;
    let ReconciliationBaseline::Verified {
        projection: baseline,
        current_turn,
    } = &mut request.baseline
    else {
        panic!("baseline")
    };
    baseline.turn = TurnState::Working;
    baseline.turn_id = Some("current-U".parse().unwrap());
    *current_turn = baseline.turn_id.clone();
    engine
        .execute(&Request::Reconcile(Box::new(request)))
        .unwrap();
    let before = projection(&engine);
    engine
        .execute(&Request::Maintain {
            expected_revision: Revision::new(3),
            now: 31 * DAY,
            history: HistoryRetention::ThirtyDays,
        })
        .unwrap();
    assert_eq!(projection(&engine), before);
    let record = queries::producer(engine.connection.as_ref().unwrap(), &id(1))
        .unwrap()
        .unwrap();
    assert!(record.active && record.reconciled);
    assert_eq!(count(&engine, "tombstones"), 0);
}
