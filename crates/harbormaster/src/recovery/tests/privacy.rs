use super::*;
use crate::recovery::{CleanupReason, LogReason, LogRecord, RecoveryError};

#[test]
fn excluded_content_never_reaches_spool_temp_logs_or_cleanup() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    stage_synthetic(&mut store, &source(1, 3), 10);
    store
        .append_log(LogRecord {
            observed_ms: 10,
            reason: LogReason::MissingHook,
            count: 1,
            unknown_gap: true,
        })
        .unwrap();
    fixture.scan_canaries();
    let snapshot = store.snapshot().unwrap();
    let before: Vec<_> = fs::read_dir(fixture.artifacts())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.clone(),
                fs::metadata(&path).unwrap().modified().unwrap(),
            )
        })
        .collect();
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    assert_eq!(plan.eligible_files(), 2);
    assert_eq!(store.snapshot().unwrap(), snapshot);
    for (path, time) in before {
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), time);
    }
    let result = store.apply_cleanup(plan).unwrap();
    assert_eq!(result.removed_files, 2);
    assert_eq!(result.failure, None);
    fixture.scan_canaries();
}

#[test]
fn mis_mapped_allowed_turn_and_version_strings_reject_before_temp() {
    for bad_source in [Source::Prompt, Source::Environment, Source::Unproven] {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let mut value: serde_json::Value = serde_json::from_slice(&source(1, 3)).unwrap();
        value["metadata"]["payload"]["turn_id"] = CANARIES[0].into();
        let bytes = serde_json::to_vec(&value).unwrap();
        let mapping = [Mapping {
            harness: HarnessKind::Hermes,
            version: REVIEWED_SYNTHETIC[0].version,
            turn: bad_source,
            session: Source::Metadata,
            health_version: bad_source,
        }];
        assert!(matches!(
            store.stage_reviewed(input(&bytes), 10, &mapping),
            Err(RecoveryError::UnprovenSource)
        ));
        value["metadata"]["kind"] = "producer.health".into();
        value["metadata"]["payload"] =
            serde_json::json!({"adapter_version":CANARIES[4],"signals":[]});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            store.stage_reviewed(input(&bytes), 10, &mapping),
            Err(RecoveryError::UnprovenSource)
        ));
        assert_eq!(store.snapshot().unwrap().spool_files, 0);
        fixture.scan_canaries();
    }
}

#[test]
fn unproven_optional_session_is_omitted_and_wrong_version_rejected() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let mut value: serde_json::Value = serde_json::from_slice(&source(1, 3)).unwrap();
    value["metadata"]["kind"] = "run.started".into();
    value["metadata"]["payload"] = serde_json::json!({"harness_session_id":CANARIES[4]});
    let bytes = serde_json::to_vec(&value).unwrap();
    let mapping = [Mapping {
        harness: HarnessKind::Hermes,
        version: REVIEWED_SYNTHETIC[0].version,
        turn: Source::Metadata,
        session: Source::Environment,
        health_version: Source::Metadata,
    }];
    let receipt = store.stage_reviewed(input(&bytes), 10, &mapping).unwrap();
    assert!(
        matches!(&receipt.event().event, crate::protocol::EventPayload::RunStarted(payload) if payload.harness_session_id.is_none())
    );
    fixture.scan_canaries();
    let bad_version = AdapterRecord {
        harness: HarnessKind::Hermes,
        version: CANARIES[4],
        source: &bytes,
    };
    assert!(matches!(
        store.stage_reviewed(bad_version, 10, &mapping),
        Err(RecoveryError::UnsupportedAdapter)
    ));
    assert!(matches!(
        store.read_batch(10),
        Err(RecoveryError::UnsupportedAdapter)
    ));
    assert_eq!(store.read_reviewed(10, &mapping).unwrap().len(), 1);
}
