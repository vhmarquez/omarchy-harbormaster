use super::*;
use crate::recovery::{CleanupReason, RecoveryError, SPOOL_TTL_MS};

#[test]
fn replay_keeps_identity_is_repeatable_and_commit_requires_exact_event() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let receipt = stage_synthetic(&mut store, &source(1, 3), 10);
    let original = receipt.event().clone();
    assert_eq!(
        stage_synthetic(&mut store, &source(1, 3), 99).event(),
        &original
    );
    assert_eq!(store.snapshot().unwrap().spool_files, 1);
    drop(receipt);
    drop(store);
    let mut store = fixture.reopen_after_release();
    let first = read_synthetic(&store, 10).remove(0);
    let second = store
        .read_reviewed(10, REVIEWED_SYNTHETIC)
        .unwrap()
        .remove(0);
    assert_eq!(first.event(), &original);
    assert_eq!(first.harness(), HarnessKind::Hermes);
    assert_eq!(second.event(), &original);
    let mut mismatched = original.clone();
    mismatched.seq = crate::protocol::Seq::new(999);
    assert_eq!(
        store.confirm_committed(first, &mismatched),
        Err(RecoveryError::InvalidRecord)
    );
    assert_eq!(store.snapshot().unwrap().spool_files, 1);
    store.confirm_committed(second, &original).unwrap();
    assert_eq!(store.snapshot().unwrap().spool_files, 0);
}

#[test]
fn retirement_and_ttl_are_explicit_and_never_retag_generations() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let old = stage_synthetic(&mut store, &source(1, 3), 100)
        .event()
        .clone();
    let current = stage_synthetic(&mut store, &source(2, 4), 100)
        .event()
        .clone();
    for now in [0, 100, 100 + SPOOL_TTL_MS - 1] {
        assert_eq!(
            store
                .preview_cleanup(&CleanupReason::Expired, now)
                .unwrap()
                .eligible_files(),
            0
        );
    }
    assert_eq!(
        store
            .preview_cleanup(&CleanupReason::Expired, 100 + SPOOL_TTL_MS)
            .unwrap()
            .eligible_files(),
        2
    );
    let plan = store
        .preview_cleanup(
            &CleanupReason::RetiredGeneration {
                producer: old.producer_id,
                generation: old.generation,
            },
            100,
        )
        .unwrap();
    assert_eq!(plan.eligible_files(), 1);
    store.apply_cleanup(plan).unwrap();
    let batch = store.read_reviewed(100, REVIEWED_SYNTHETIC).unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].event(), &current);
    assert_eq!(store.snapshot().unwrap().discarded_artifacts, 1);
    assert!(store.snapshot().unwrap().unknown_gap);
}

#[test]
fn modified_record_and_renamed_event_identity_fail_closed() {
    for tamper in ["format", "generation", "mapping"] {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        stage_synthetic(&mut store, &source(1, 3), 10);
        let path = fs::read_dir(fixture.artifacts())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .unwrap();
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match tamper {
            "format" => record["format"] = 90.into(),
            "generation" => {
                record["event"]["generation"] = "00000000-0000-4000-8000-000000000999".into();
            }
            _ => record["mapping_version"] = "unreviewed-version".into(),
        }
        fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(store.read_reviewed(10, REVIEWED_SYNTHETIC).is_err());
        assert!(store.read_batch(10).is_err());
        assert!(path.exists());
    }
}

#[test]
fn bounded_replay_selects_predecessor_before_adversarial_uuid_order() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    for sequence in 1..=64 {
        let mut value: serde_json::Value = serde_json::from_slice(&source(sequence, 3)).unwrap();
        value["metadata"]["event_id"] =
            format!("00000000-0000-4000-8000-{:012x}", 1000 - sequence).into();
        stage_synthetic(&mut store, &serde_json::to_vec(&value).unwrap(), 10);
    }
    let batch = read_synthetic(&store, 10);
    let sequences: Vec<_> = batch
        .iter()
        .map(|entry| entry.event().seq.value())
        .collect();
    assert_eq!(sequences, (1..=32).collect::<Vec<_>>());
}

#[test]
fn held_receipts_do_not_pin_deleted_spool_or_allow_new_owner() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let held = stage_synthetic(&mut store, &source(1, 3), 10);
    let replay = read_synthetic(&store, 10).remove(0);
    let event = replay.event().clone();
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    assert_eq!(store.apply_cleanup(plan).unwrap().removed_files, 1);
    let directory = fixture.artifacts().to_string_lossy().into_owned();
    for entry in fs::read_dir("/proc/self/fd").unwrap() {
        let Ok(path) = fs::read_link(entry.unwrap().path()) else {
            continue;
        };
        let path = path.to_string_lossy();
        assert!(!(path.starts_with(&directory) && path.ends_with(" (deleted)")));
    }
    stage_synthetic(&mut store, &source(1, 3), 10);
    assert_eq!(
        store.confirm_committed(replay, &event),
        Err(RecoveryError::IdentityChanged)
    );
    assert_eq!(store.snapshot().unwrap().spool_files, 1);
    drop(store);
    assert!(matches!(
        ArtifactStore::open(&fixture.0),
        Err(RecoveryError::AlreadyOwned)
    ));
    drop(held);
    let _new_owner = fixture.reopen_after_release();
}
