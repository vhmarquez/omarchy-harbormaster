use super::*;

#[test]
fn one_durable_generation_and_exact_duplicate_receipt_survive_unlink_delay() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let event = entry.event().clone();
    let (result, receipt) = fixture.commit(entry);
    assert_eq!(
        result,
        CommitProgress::Committed {
            revision: Revision::new(2)
        }
    );
    assert_eq!(receipt.as_ref().unwrap().0.event(), &event);
    let before = fixture.call(Request::Status);
    let entry = artifacts::read_synthetic(&fixture.store, 1000)
        .pop()
        .unwrap();
    let (result, duplicate) = fixture.commit(entry);
    assert_eq!(
        result,
        CommitProgress::Committed {
            revision: Revision::new(2)
        }
    );
    assert_eq!(fixture.call(Request::Status), before);
    let (receipt, entry) = duplicate.unwrap();
    confirm_spool(&mut fixture.store, entry, &receipt).unwrap();
    assert_eq!(fixture.store.snapshot().unwrap().spool_files, 0);
}

#[test]
fn missing_identity_proof_is_durable_unknown_and_never_active() {
    let mut fixture = Fixture::new();
    let mut input = fixture.input(None);
    input.evidence = None;
    assert!(matches!(
        fixture.reconcile(input),
        ReconciliationProgress::Unavailable { .. }
    ));
    let Response::Producer(Some(producer)) = fixture.call(Request::Producer(id(2))) else {
        panic!("producer")
    };
    assert!(!producer.active && !producer.reconciled);
}

#[test]
fn sequence_gap_retires_scope_without_fact_ack_or_spool_removal() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 2, "turn.started");
    let (progress, receipt) = fixture.commit(entry);
    assert_eq!(
        progress,
        CommitProgress::Rejected(CommitFailure::SequenceGap)
    );
    assert!(receipt.is_none());
    let Response::Producer(Some(producer)) = fixture.call(Request::Producer(id(2))) else {
        panic!("producer")
    };
    assert!(!producer.active && !producer.reconciled);
    assert_eq!(fixture.store.snapshot().unwrap().spool_files, 1);
    let Response::Status(status) = fixture.call(Request::Status) else {
        panic!("status")
    };
    assert_eq!(status.facts, 0);
}

#[test]
fn original_apply_intent_can_be_retained_and_replayed_after_receipt_is_discarded() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        policy(),
        1,
        true,
    );
    assert!(matches!(finish(&mut job), CommitProgress::Committed { .. }));
    let original = job.operation.request().clone();
    let pending = job.into_pending().unwrap();
    let mut resumed = CommitJob::resume(&fixture.worker, &mut fixture.registry, pending);
    assert_eq!(resumed.operation.request(), &original);
    assert!(
        matches!(finish(&mut resumed),CommitProgress::Committed{revision} if revision==Revision::new(2))
    );
    assert!(resumed.take_committed().is_some());
}

#[test]
fn explicit_verified_wait_continuity_resolves_original_obligation_after_new_generation_terminal() {
    let mut fixture = Fixture::new();
    let old = fixture.registered();
    let entry = fixture.stage(&old, 1, "turn.awaiting_approval");
    let event = entry.event().clone();
    assert!(fixture.commit(entry).1.is_some());
    let mut input = fixture.input(Some(old.clone()));
    let turn_id = event.event.turn_id().unwrap().clone();
    input.evidence.as_mut().unwrap().current_turn = CurrentTurnEvidence::Known {
        turn_id: turn_id.clone(),
        state: TurnState::AwaitingApproval,
    };
    input.continued_turn = Some(TurnKey {
        producer_id: id(2),
        generation: old.clone(),
        run_id: id(4),
        turn_id,
    });
    let ReconciliationProgress::Installed {
        generation: new, ..
    } = fixture.reconcile(input)
    else {
        panic!("continued baseline")
    };
    let entry = fixture.stage(&new, 1, "turn.completed");
    assert!(fixture.commit(entry).1.is_some());
    let Response::Outcome(Some(outcome)) =
        fixture.call(Request::Outcome(crate::storage::OutcomeKey {
            producer_id: id(2),
            generation: old,
            event_id: event.event_id,
        }))
    else {
        panic!("original obligation")
    };
    assert_eq!(outcome.outcome_revision, Some(Revision::new(2)));
    assert_eq!(
        outcome.resolved_reasons,
        vec![AttentionReason::NativeApproval]
    );
    assert!(outcome.attention.iter().all(|reason| !reason.reviewed));
    assert_eq!(outcome.delivery, Some(DeliveryState::Pending));
    assert_eq!(fixture.status().attention, 2);
}
