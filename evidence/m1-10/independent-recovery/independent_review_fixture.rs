use super::*;
use crate::recovery::{CleanupReason, RecoveryError};

#[test]
fn replay_revalidates_required_source_provenance() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    stage_synthetic(&mut store, &source(1, 3), 10);
    let mapping = [Mapping {
        harness: HarnessKind::Hermes,
        version: REVIEWED_SYNTHETIC[0].version,
        turn: Source::Prompt,
        session: Source::Metadata,
        health_version: Source::Metadata,
    }];
    let before = store.snapshot().unwrap();
    assert!(matches!(
        store.read_reviewed(10, &mapping),
        Err(RecoveryError::UnprovenSource)
    ));
    assert_eq!(store.snapshot().unwrap(), before);
    assert_eq!(read_synthetic(&store, 10).len(), 1);
}

#[test]
fn filename_sequence_must_match_record_sequence() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    stage_synthetic(&mut store, &source(1, 3), 10);
    let path = fs::read_dir(fixture.artifacts())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "json"))
        .unwrap();
    let mut record: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["event"]["seq"] = "2".into();
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(matches!(
        store.read_reviewed(10, REVIEWED_SYNTHETIC),
        Err(RecoveryError::InvalidRecord)
    ));
    assert!(path.exists());
}

#[test]
#[ignore = "requires isolated source-level directory-fsync failure injection"]
fn successful_unlink_is_counted_when_directory_sync_fails() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    stage_synthetic(&mut store, &source(1, 3), 10);
    let before = store.snapshot().unwrap();
    let plan = store.preview_cleanup(&CleanupReason::PrivacyDisabled, 10).unwrap();
    let result = store.apply_cleanup(plan).unwrap();
    assert_eq!(result.failure, Some(RecoveryError::Io));
    assert_eq!(result.removed_files, 1);
    assert_eq!(result.removed_bytes, before.spool_bytes);
    let after = store.snapshot().unwrap();
    assert_eq!(after.spool_files, 0);
    assert_eq!(after.discarded_artifacts, 1);
    assert!(after.unknown_gap);
}
