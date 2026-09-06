//! Public read-only diagnostics contract; root owns private coordinator pipeline tests.
use harbormaster::{
    diagnostics::{DiagnosticError, MAX_DIAGNOSTIC_BYTES, project},
    protocol::Revision,
    recovery::RecoveryStatus,
    storage::{HistoryRetention, MaintenanceResult, StoragePolicy, StorageStatus},
};
#[path = "coordination/fixture.rs"]
mod fixture;

fn snapshots() -> (StorageStatus, StoragePolicy, RecoveryStatus) {
    (
        StorageStatus {
            revision: Revision::new(4),
            facts: 1,
            tombstones: 2,
            attention: 3,
            pending_deliveries: 4,
            discarded_events: 5,
            cleanup: MaintenanceResult {
                facts_removed: 1,
                tombstones_removed: 2,
                generations_retired: 3,
                deliveries_expired: 4,
                more: true,
            },
        },
        StoragePolicy {
            revision: Revision::new(4),
            history: HistoryRetention::ThirtyDays,
        },
        RecoveryStatus {
            rejected_attempts: 7,
            discarded_artifacts: 8,
            unknown_gap: true,
            ..RecoveryStatus::default()
        },
    )
}

#[test]
fn mismatched_status_and_policy_revisions_reject_before_projection() {
    let (status, mut policy, recovery) = snapshots();
    policy.revision = Revision::new(5);
    assert_eq!(
        project(&status, &policy, &recovery, None),
        Err(DiagnosticError::MismatchedRevision)
    );
}

#[test]
fn projection_preserves_policy_counter_units_and_unknown_gap() {
    let (status, policy, recovery) = snapshots();
    let before = (status.clone(), policy.clone(), recovery);
    let expected =
        serde_json::to_vec(&project(&status, &policy, &recovery, None).unwrap()).unwrap();
    for _ in 0..100 {
        assert_eq!(
            serde_json::to_vec(&project(&status, &policy, &recovery, None).unwrap()).unwrap(),
            expected
        );
    }
    assert_eq!((status, policy, recovery), before);
    let value: serde_json::Value = serde_json::from_slice(&expected).unwrap();
    assert_eq!(value["scope"], "manager_metadata");
    assert!(
        value["commit"].is_null(),
        "no job snapshot is not zero loss"
    );
    assert_eq!(value["history_days"], 30);
    assert_eq!(value["recovery"]["rejected_attempts"], 7);
    assert_eq!(value["recovery"]["discarded_artifacts"], 8);
    assert_eq!(value["recovery"]["unknown_gap"], true);
    assert!(expected.len() < MAX_DIAGNOSTIC_BYTES);
    let keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "cleanup",
            "commit",
            "history_days",
            "recovery",
            "revision",
            "scope",
            "storage"
        ]
    );
}

#[test]
fn maximum_numeric_projection_has_fixed_bounded_output() {
    let maximum = MaintenanceResult {
        facts_removed: u32::MAX,
        tombstones_removed: u32::MAX,
        generations_retired: u32::MAX,
        deliveries_expired: u32::MAX,
        more: true,
    };
    let status = StorageStatus {
        revision: Revision::new(u64::MAX),
        facts: u32::MAX,
        tombstones: u32::MAX,
        attention: u32::MAX,
        pending_deliveries: u32::MAX,
        discarded_events: u64::MAX,
        cleanup: maximum,
    };
    let policy = StoragePolicy {
        revision: status.revision,
        history: HistoryRetention::SevenDays,
    };
    let recovery = RecoveryStatus {
        spool_bytes: u64::MAX,
        spool_files: usize::MAX,
        protected_files: usize::MAX,
        log_bytes: u64::MAX,
        rejected_attempts: u64::MAX,
        discarded_artifacts: u64::MAX,
        unknown_gap: true,
    };
    let commit = harbormaster::coordinator::CommitStatus {
        known_queued_discards: u64::MAX,
        unknown_gap: true,
    };
    let bytes =
        serde_json::to_vec(&project(&status, &policy, &recovery, Some(&commit)).unwrap()).unwrap();
    assert!(bytes.len() < MAX_DIAGNOSTIC_BYTES);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["history_days"], 7);
    assert_eq!(value["revision"], u64::MAX.to_string());
}

#[test]
fn actual_snapshots_format_repeatedly_without_files_policy_or_canary_collection() {
    let fixture = fixture::Fixture::new();
    let (status, policy, recovery) = fixture.snapshots();
    let before_files = fixture.file_state();
    let baseline =
        serde_json::to_vec(&project(&status, &policy, &recovery, None).unwrap()).unwrap();
    for _ in 0..100 {
        let bytes =
            serde_json::to_vec(&project(&status, &policy, &recovery, None).unwrap()).unwrap();
        assert_eq!(bytes, baseline);
        for canary in fixture::CANARIES {
            assert!(
                !bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes())
            );
        }
    }
    let mut mismatched = policy.clone();
    mismatched.revision = mismatched.revision.checked_next().unwrap();
    assert_eq!(
        project(&status, &mismatched, &recovery, None),
        Err(DiagnosticError::MismatchedRevision)
    );
    assert_eq!(fixture.file_state(), before_files);
    assert_eq!(fixture.snapshots(), (status, policy, recovery));
}

#[test]
fn current_job_loss_is_separate_from_durable_and_artifact_counters() {
    let (status, policy, recovery) = snapshots();
    let commit = harbormaster::coordinator::CommitStatus {
        known_queued_discards: 9,
        unknown_gap: true,
    };
    let value =
        serde_json::to_value(project(&status, &policy, &recovery, Some(&commit)).unwrap()).unwrap();
    assert_eq!(value["commit"]["known_queued_discards"], 9);
    assert_eq!(value["commit"]["unknown_gap"], true);
    assert_eq!(value["storage"]["discarded_events"], 5);
    assert_eq!(value["recovery"]["discarded_artifacts"], 8);
}
