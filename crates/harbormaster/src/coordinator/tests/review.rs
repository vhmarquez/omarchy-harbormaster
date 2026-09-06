use super::*;

fn until_apply(job: &mut CommitJob<'_>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !matches!(job.purpose, super::super::commit::Purpose::Apply) {
        assert_eq!(job.poll(), CommitProgress::Pending);
        assert!(
            job.take_committed().is_none(),
            "no receipt before Apply submission"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn independent_receipt_exists_only_after_observed_atomic_commit() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let event = entry.event().clone();
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        policy(),
        7,
        true,
    );
    assert!(job.take_committed().is_none());
    until_apply(&mut job);
    let before = fixture
        .worker
        .try_submit(Request::Status)
        .unwrap()
        .wait_timeout(Duration::from_secs(3))
        .unwrap();
    let Response::Status(before) = before else {
        panic!("status");
    };
    assert_eq!(
        (before.facts, before.tombstones, before.pending_deliveries),
        (0, 0, 0)
    );
    assert!(job.take_committed().is_none());
    assert_eq!(
        finish(&mut job),
        CommitProgress::Committed {
            revision: Revision::new(2)
        }
    );
    let (receipt, entry) = job.take_committed().unwrap();
    assert_eq!(receipt.event(), &event);
    assert!(job.take_committed().is_none());
    drop(job);
    let status = fixture.status();
    assert_eq!(
        (
            status.facts,
            status.tombstones,
            status.attention,
            status.pending_deliveries
        ),
        (1, 1, 1, 1)
    );
    confirm_spool(&mut fixture.store, entry, &receipt).unwrap();
}

#[test]
fn independent_same_identity_changed_payload_gets_no_receipt() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let event = entry.event().clone();
    assert!(fixture.commit(entry).1.is_some());
    let mut value: serde_json::Value = serde_json::from_slice(&artifacts::source(1, 1)).unwrap();
    value["metadata"]["generation"] = generation.as_str().into();
    let other_root = artifacts::Fixture::new();
    let mut other_store = other_root.open();
    artifacts::stage_synthetic(&mut other_store, &serde_json::to_vec(&value).unwrap(), 2000);
    let entry = artifacts::read_synthetic(&other_store, 2000)
        .into_iter()
        .find(|entry| entry.event().event.kind() == EventKind::TurnStarted)
        .unwrap();
    assert_eq!(entry.event().event_id, event.event_id);
    assert_eq!(entry.event().seq, event.seq);
    let (progress, receipt) = fixture.commit(entry);
    assert_eq!(progress, CommitProgress::Rejected(CommitFailure::Conflict));
    assert!(receipt.is_none());
    assert_eq!(fixture.status().facts, 1);
    assert_eq!(fixture.store.snapshot().unwrap().spool_files, 1);
    assert_eq!(other_store.snapshot().unwrap().spool_files, 1);
}

#[test]
fn independent_exact_receipt_does_not_bypass_source_harness_scope() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let original = entry.event().clone();
    assert!(fixture.commit(entry).1.is_some());
    let other_root = artifacts::Fixture::new();
    let mut other_store = other_root.open();
    let mut value: serde_json::Value = serde_json::from_slice(&artifacts::source(1, 1)).unwrap();
    value["metadata"]["generation"] = generation.as_str().into();
    value["metadata"]["kind"] = "turn.completed".into();
    let entry =
        artifacts::review_codex_entry(&mut other_store, &serde_json::to_vec(&value).unwrap());
    assert_eq!(entry.event(), &original);
    assert_eq!(entry.harness(), HarnessKind::Codex);
    let (progress, receipt) = fixture.commit(entry);
    assert_eq!(
        progress,
        CommitProgress::Rejected(CommitFailure::Unreconciled)
    );
    assert!(receipt.is_none());
    assert_eq!(other_store.snapshot().unwrap().spool_files, 1);
}

#[test]
fn independent_confirmation_receipt_cannot_authorize_a_different_source_harness() {
    let mut fixture = Fixture::new();
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let original = entry.event().clone();
    let (receipt, _original_entry) = fixture.commit(entry).1.unwrap();
    let other_root = artifacts::Fixture::new();
    let mut other_store = other_root.open();
    let mut value: serde_json::Value = serde_json::from_slice(&artifacts::source(1, 1)).unwrap();
    value["metadata"]["generation"] = generation.as_str().into();
    value["metadata"]["kind"] = "turn.completed".into();
    let entry =
        artifacts::review_codex_entry(&mut other_store, &serde_json::to_vec(&value).unwrap());
    assert_eq!(entry.event(), &original);
    assert_eq!(entry.harness(), HarnessKind::Codex);
    assert!(
        confirm_spool(&mut other_store, entry, &receipt).is_err(),
        "durable Hermes receipt cannot confirm a Codex source artifact"
    );
    assert_eq!(other_store.snapshot().unwrap().spool_files, 1);
}
