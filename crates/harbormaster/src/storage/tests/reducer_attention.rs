use super::reducer::{apply_set, reconcile};
use super::*;

pub(super) fn waiting(set: WriteSet) -> ReducerWrite {
    let mut set = apply_set(set);
    set.fact.event = EventPayload::TurnAwaitingInput(TurnPayload {
        turn_id: "fixture-turn".parse().unwrap(),
    });
    set.effects.projection.turn = TurnState::AwaitingInput;
    set.effects.attention = vec![AttentionReason::Input];
    set.effects.tombstone = None;
    set.effects.resolve_transient = None;
    set
}
#[test]
fn repeated_wait_coalesces_and_resolution_does_not_review_or_revise_the_outcome() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let original = write(generation.clone(), revision, 1, 5);
    let first = waiting(original.clone());
    engine.execute(&Request::Apply(Box::new(first))).unwrap();
    let second = waiting(write(generation.clone(), Revision::new(2), 2, 6));
    engine.execute(&Request::Apply(Box::new(second))).unwrap();
    assert_eq!(count(&engine, "attention"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
    let terminal = apply_set(write(generation, Revision::new(3), 3, 7));
    engine.execute(&Request::Apply(Box::new(terminal))).unwrap();
    let Response::Outcome(Some(record)) = engine
        .execute(&Request::Outcome(outcome(&original)))
        .unwrap()
    else {
        panic!("outcome")
    };
    assert_eq!(record.resolved_reasons, vec![AttentionReason::Input]);
    assert!(!record.attention[0].reviewed);
    assert_eq!(record.outcome_revision, Some(Revision::new(2)));
    assert_eq!(
        engine.execute(&Request::ReviewOutcome(OutcomeReview {
            expected_revision: Revision::new(4),
            outcome: outcome(&original),
            outcome_revision: Revision::new(3)
        })),
        Err(StorageError::Conflict)
    );
    engine
        .execute(&Request::ReviewOutcome(OutcomeReview {
            expected_revision: Revision::new(4),
            outcome: outcome(&original),
            outcome_revision: Revision::new(2),
        }))
        .unwrap();
    let Response::Outcome(Some(record)) = engine
        .execute(&Request::Outcome(outcome(&original)))
        .unwrap()
    else {
        panic!("outcome")
    };
    assert!(record.attention[0].reviewed);
}
#[test]
fn late_terminal_records_its_own_key_without_replacing_current_turn() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let mut terminal = apply_set(write(generation, revision, 1, 5));
    let mut current = terminal.effects.current_turn.clone().unwrap();
    current.turn_id = "current-U".parse().unwrap();
    terminal.effects.current_turn = Some(current.clone());
    terminal.effects.projection.turn_id = Some(current.turn_id.clone());
    terminal.effects.projection.turn = TurnState::Working;
    engine
        .execute(&Request::Apply(Box::new(terminal.clone())))
        .unwrap();
    let Response::Context(context) = engine
        .execute(&Request::Context(Box::new(terminal.fact)))
        .unwrap()
    else {
        panic!("context")
    };
    assert_eq!(context.state.current_turn, Some(current));
    assert_eq!(context.state.projection.turn, TurnState::Working);
    assert_eq!(context.state.tombstone, terminal.effects.tombstone);
}
#[test]
fn uncertainty_and_terminal_commit_together_then_reject_queued_admission() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let mut terminal = apply_set(write(generation.clone(), revision, 1, 5));
    terminal
        .effects
        .attention
        .push(AttentionReason::ConnectionUncertainty);
    terminal.effects.reconciliation_required = true;
    engine
        .execute(&Request::Apply(Box::new(terminal.clone())))
        .unwrap();
    assert_eq!(count(&engine, "attention"), 2);
    let record = queries::producer(engine.connection.as_ref().unwrap(), &id(1))
        .unwrap()
        .unwrap();
    assert!(!record.active && !record.reconciled);
    let next = waiting(write(generation, Revision::new(2), 2, 6));
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(next))),
        Err(StorageError::StaleGeneration)
    );
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(terminal.clone()))),
        Ok(Response::Committed {
            revision: Revision::new(2),
            duplicate: true
        })
    );
    terminal.notify = false;
    assert_eq!(
        engine.execute(&Request::Apply(Box::new(terminal))),
        Err(StorageError::Conflict)
    );
}
#[test]
fn late_failure_rolls_back_resolution_projection_reasons_and_checkpoint() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = reconcile(&mut engine);
    let first = write(generation.clone(), revision, 1, 5);
    engine
        .execute(&Request::Apply(Box::new(waiting(first.clone()))))
        .unwrap();
    engine.connection.as_ref().unwrap().execute_batch("CREATE TRIGGER fixture_failure BEFORE INSERT ON facts BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    let terminal = apply_set(write(generation, Revision::new(2), 2, 6));
    assert!(engine.execute(&Request::Apply(Box::new(terminal))).is_err());
    assert_eq!(count(&engine, "facts"), 1);
    assert_eq!(count(&engine, "outbox"), 1);
    assert_eq!(count(&engine, "tombstones"), 0);
    let record = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&first))
        .unwrap()
        .unwrap();
    assert!(record.resolved_reasons.is_empty());
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(2)
    );
    assert_eq!(
        queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .next_sequence,
        Some(Seq::new(2))
    );
}
