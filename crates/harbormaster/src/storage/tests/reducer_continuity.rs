use super::*;
use super::{
    reducer::{apply_set, reconcile},
    reducer_attention::waiting,
};

pub(super) fn approval(set: WriteSet) -> ReducerWrite {
    let mut set = waiting(set);
    set.fact.event = EventPayload::TurnAwaitingApproval(TurnPayload {
        turn_id: "fixture-turn".parse().unwrap(),
    });
    set.effects.projection.turn = TurnState::AwaitingApproval;
    set.effects.attention = vec![AttentionReason::NativeApproval];
    set
}
pub(super) fn continuation(engine: &Engine, generation: ProducerGeneration) -> Reconciliation {
    let conn = engine.connection.as_ref().unwrap();
    let (_, current) = super::super::super::context::projection(conn, &id(2)).unwrap();
    // Independent synthetic native evidence: the fixture remains waiting even
    // when restart correctly degrades the saved observer projection.
    let projection = RunProjection {
        run_id: id(2),
        process: ProcessState::Alive,
        observation: ObservationState::Fresh,
        turn: TurnState::AwaitingApproval,
        turn_id: Some("fixture-turn".parse().unwrap()),
    };
    Reconciliation {
        registration: Registration {
            expected_revision: queries::revision(conn).unwrap(),
            producer_id: id(1),
            run_id: id(2),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(1),
            previous_generation: Some(generation),
        },
        baseline: ReconciliationBaseline::Verified {
            current_turn: projection.turn_id.clone(),
            projection,
        },
        discarded_events: 0,
        resolved_turn: None,
        continued_turn: current,
    }
}
#[test]
fn same_wait_survives_repeated_restart_without_retagging_or_duplicate_attention() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (mut generation, revision) = reconcile(&mut engine);
    let origin = write(generation.clone(), revision, 1, 5);
    let original = approval(origin.clone());
    engine
        .execute(&Request::Apply(Box::new(original.clone())))
        .unwrap();
    for event in [6, 7] {
        drop(engine);
        engine = fixture.open();
        let request = continuation(&engine, generation);
        let Response::Registered {
            generation: fresh,
            revision,
        } = engine
            .execute(&Request::Reconcile(Box::new(request)))
            .unwrap()
        else {
            panic!("reconcile")
        };
        generation = fresh;
        engine
            .execute(&Request::Apply(Box::new(approval(write(
                generation.clone(),
                revision,
                1,
                event,
            )))))
            .unwrap();
        assert_eq!(count(&engine, "attention"), 1);
        assert_eq!(count(&engine, "outbox"), 1);
    }
    let revision = queries::revision(engine.connection.as_ref().unwrap()).unwrap();
    engine
        .execute(&Request::Apply(Box::new(apply_set(write(
            generation, revision, 2, 8,
        )))))
        .unwrap();
    let stale = approval(write(
        origin.fact.generation.clone(),
        revision.checked_next().unwrap(),
        2,
        99,
    ));
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(stale))),
        Err(StorageError::StaleGeneration)
    );
    assert_original(&mut engine, &origin, &original);
}
#[test]
fn contradictory_progress_and_continuity_are_rejected_atomically() {
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
    let mut request = continuation(&engine, generation);
    request.resolved_turn = request.continued_turn.clone();
    assert_eq!(
        engine.execute(&Request::Reconcile(Box::new(request))),
        Err(StorageError::InvalidRequest)
    );
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(2)
    );
}
#[test]
fn apply_discard_overflow_never_partially_commits_and_success_counts_once() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute(
            "UPDATE metadata SET discarded=?1",
            [u64::MAX.to_be_bytes().as_slice()],
        )
        .unwrap();
    let mut set = apply_set(write(generation, revision, 1, 5));
    set.discarded_events = 1;
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set.clone()))),
        Err(StorageError::ResourceExhausted)
    );
    assert_eq!(count(&engine, "facts"), 0);
    assert_eq!(count(&engine, "attention"), 0);
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute(
            "UPDATE metadata SET discarded=?1",
            [0_u64.to_be_bytes().as_slice()],
        )
        .unwrap();
    engine
        .execute(&Request::Apply(Box::new(set.clone())))
        .unwrap();
    engine
        .execute(&Request::Apply(Box::new(set.clone())))
        .unwrap();
    set.discarded_events = 2;
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(set))),
        Err(StorageError::Conflict)
    );
    let Response::Status(status) = engine.execute(&Request::Status).unwrap() else {
        panic!("status")
    };
    assert_eq!(status.discarded_events, 1);
}

fn assert_original(engine: &mut Engine, origin: &WriteSet, original: &ReducerWrite) {
    let record = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(origin))
        .unwrap()
        .unwrap();
    assert_eq!(
        record.resolved_reasons,
        vec![AttentionReason::NativeApproval]
    );
    assert_eq!(record.outcome_revision, Some(Revision::new(2)));
    assert!(!record.attention[0].reviewed);
    assert_eq!(record.delivery, Some(DeliveryState::Pending));
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(original.fact.clone())))
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
}
