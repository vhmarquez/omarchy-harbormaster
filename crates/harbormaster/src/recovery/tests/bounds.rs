use super::*;
use crate::recovery::{
    CleanupReason, MAX_CLEANUP_BATCH, MAX_PRODUCER_BYTES, MAX_PRODUCER_FILES, MAX_REPLAY_BATCH,
    MAX_SPOOL_BYTES, MAX_SPOOL_FILES, RecoveryError, names::SpoolName,
};

#[test]
fn exact_producer_byte_limit_counts_partial_old_generation() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let original = stage_synthetic(&mut store, &source(1, 3), 10)
        .event()
        .clone();
    let bytes = store.snapshot().unwrap().spool_bytes;
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    store.apply_cleanup(plan).unwrap();
    let partial = SpoolName::new(&original, 9, true).filename();
    fixture.private_file(
        &partial,
        &vec![b' '; usize::try_from(MAX_PRODUCER_BYTES - bytes).unwrap()],
    );
    stage_synthetic(&mut store, &source(2, 4), 10);
    assert_eq!(store.snapshot().unwrap().spool_bytes, MAX_PRODUCER_BYTES);
    assert!(matches!(
        store.stage_reviewed(input(&source(3, 5)), 10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
    assert_eq!(store.snapshot().unwrap().spool_files, 2);
}

#[test]
fn global_limit_includes_unrecognized_orphan_without_deleting_it() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    fixture.private_file(
        "unrecognized-orphan",
        &vec![b' '; usize::try_from(MAX_SPOOL_BYTES).unwrap()],
    );
    assert!(matches!(
        store.stage_reviewed(input(&source(1, 3)), 10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, u64::MAX)
        .unwrap();
    assert_eq!(plan.eligible_files(), 0);
    assert_eq!(plan.protected_files(), 1);
    store.apply_cleanup(plan).unwrap();
    assert_eq!(
        fs::metadata(fixture.artifacts().join("unrecognized-orphan"))
            .unwrap()
            .len(),
        MAX_SPOOL_BYTES
    );
}

#[test]
fn producer_file_count_and_replay_batch_are_bounded() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    for sequence in 1..=u64::try_from(MAX_PRODUCER_FILES).unwrap() {
        stage_synthetic(&mut store, &source(sequence, sequence), 10);
    }
    assert_eq!(
        store.read_reviewed(10, REVIEWED_SYNTHETIC).unwrap().len(),
        MAX_REPLAY_BATCH
    );
    assert!(matches!(
        store.stage_reviewed(input(&source(100, 999)), 10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
    assert_eq!(store.snapshot().unwrap().spool_files, MAX_PRODUCER_FILES);
}

#[test]
fn global_file_count_scan_and_cleanup_batch_are_bounded() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let event = stage_synthetic(&mut store, &source(1, 3), 10)
        .event()
        .clone();
    store
        .apply_cleanup(
            store
                .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
                .unwrap(),
        )
        .unwrap();
    for index in 0..MAX_SPOOL_FILES {
        let name = SpoolName::new(&event, index as u64, true).filename();
        fixture.private_file(&name, b"");
    }
    assert!(matches!(
        store.stage_reviewed(input(&source(2, 4)), 10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, u64::MAX)
        .unwrap();
    assert_eq!(plan.eligible_files(), MAX_CLEANUP_BATCH);
    assert_eq!(plan.deferred_files(), MAX_SPOOL_FILES - MAX_CLEANUP_BATCH);
    fixture.private_file("overflow", b"");
    assert!(matches!(
        store.snapshot(),
        Err(RecoveryError::BoundExceeded)
    ));
    assert!(matches!(
        store.apply_cleanup(plan),
        Err(RecoveryError::BoundExceeded)
    ));
    fs::remove_file(fixture.artifacts().join("overflow")).unwrap();
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, u64::MAX)
        .unwrap();
    assert_eq!(
        store.apply_cleanup(plan).unwrap().removed_files,
        MAX_CLEANUP_BATCH
    );
}

#[test]
fn oversized_record_is_rejected_before_read_allocation() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let metadata =
        super::super::provenance::eligible(input(&source(1, 3)), REVIEWED_SYNTHETIC).unwrap();
    let name = SpoolName::new(&metadata.into_parts().0, 10, false).filename();
    fixture.private_file(&name, &vec![b' '; crate::recovery::MAX_RECORD_BYTES + 1]);
    assert!(matches!(
        store.read_reviewed(10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
}

#[test]
fn retained_replay_handles_are_bounded_and_released_on_drop() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    stage_synthetic(&mut store, &source(1, 3), 10);
    let mut held = Vec::new();
    for _ in 0..crate::recovery::MAX_HELD_HANDLES {
        held.push(read_synthetic(&store, 10));
    }
    assert!(matches!(
        store.read_reviewed(10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::BoundExceeded)
    ));
    held.pop();
    assert_eq!(read_synthetic(&store, 10).len(), 1);
}
