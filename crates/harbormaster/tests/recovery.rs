//! Public unsupported-provenance, bounded logs and identity-safe cleanup checks.
use harbormaster::protocol::HarnessKind;
use harbormaster::recovery::{
    AdapterRecord, ArtifactStore, CleanupReason, LogReason, LogRecord, MAX_LOG_BYTES,
    RecoveryError, SPOOL_TTL_MS,
};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, symlink};
#[path = "recovery/fixture.rs"]
mod fixture;
use fixture::{Fixture, deleted_bytes};

const PARTIAL: &str = "spool-00000000-0000-4000-8000-000000000002_00000000-0000-4000-8000-000000000003_00000000000000000001_00000000-0000-4000-8000-000000000001_00000000000000000010.part";
fn log() -> LogRecord {
    LogRecord {
        observed_ms: 10,
        reason: LogReason::MissingHook,
        count: 1,
        unknown_gap: true,
    }
}

#[test]
fn source_mapping_must_precede_serialization() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let canary = b"SYNTHETIC_PROMPT_CANARY";
    let before = fs::metadata(fixture.path("recovery.lock"))
        .unwrap()
        .modified()
        .unwrap();
    for harness in [HarnessKind::Hermes, HarnessKind::Claude, HarnessKind::Codex] {
        for version in ["v0.21.0", "2.1.260", "0.153.2", "synthetic-m1-only-v1"] {
            let input = AdapterRecord {
                harness,
                version,
                source: canary,
            };
            assert!(matches!(
                store.stage(input, 10),
                Err(RecoveryError::UnsupportedAdapter)
            ));
        }
    }
    assert_eq!(store.snapshot().unwrap().spool_files, 0);
    assert_eq!(store.snapshot().unwrap().rejected_attempts, 12);
    assert!(store.snapshot().unwrap().unknown_gap);
    assert_eq!(
        fs::metadata(fixture.path("recovery.lock"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    assert_eq!(fs::read_dir(fixture.path("")).unwrap().count(), 1);
    let oversized = vec![0; 16_385];
    let input = AdapterRecord {
        harness: HarnessKind::Hermes,
        version: "x",
        source: &oversized,
    };
    assert!(matches!(
        store.stage(input, 10),
        Err(RecoveryError::BoundExceeded)
    ));
}

#[test]
fn logs_rotate_within_four_mebibytes_and_have_no_age_ttl() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    for index in 0..4 {
        fixture.write(
            &format!("metadata.{index}.log"),
            &vec![b'0' + index; usize::try_from(MAX_LOG_BYTES).unwrap()],
        );
    }
    store.append_log(log()).unwrap();
    let snapshot = store.snapshot().unwrap();
    assert!(snapshot.log_bytes <= MAX_LOG_BYTES * 4);
    assert_eq!(fs::read_dir(fixture.path("")).unwrap().count(), 5);
    assert_eq!(fs::read(fixture.path("metadata.1.log")).unwrap()[0], b'0');
    assert_eq!(fs::read(fixture.path("metadata.3.log")).unwrap()[0], b'2');
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.path("metadata.0.log")).unwrap()).unwrap();
    assert_eq!(current["reason"], "missing_hook");
    let plan = store
        .preview_cleanup(&CleanupReason::Expired, u64::MAX)
        .unwrap();
    assert_eq!(plan.eligible_files(), 0);
    assert_eq!(plan.protected_files(), 4);
}

#[test]
fn cleanup_preview_is_read_only_and_rejects_replaced_inode() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    fixture.write(PARTIAL, b"metadata-only");
    fixture.write("unrelated-sentinel", b"preserve");
    let before = fs::metadata(fixture.path(PARTIAL))
        .unwrap()
        .modified()
        .unwrap();
    let plan = store
        .preview_cleanup(&CleanupReason::Expired, 10 + SPOOL_TTL_MS)
        .unwrap();
    assert_eq!(plan.eligible_files(), 1);
    assert_eq!(plan.protected_files(), 1);
    assert_eq!(
        fs::metadata(fixture.path(PARTIAL))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    fs::rename(fixture.path(PARTIAL), fixture.path("retained-inode")).unwrap();
    fixture.write(PARTIAL, b"replacement-preserved");
    assert!(matches!(
        store.apply_cleanup(plan),
        Err(RecoveryError::IdentityChanged)
    ));
    assert_eq!(
        fs::read(fixture.path(PARTIAL)).unwrap(),
        b"replacement-preserved"
    );
    assert_eq!(
        fs::read(fixture.path("unrelated-sentinel")).unwrap(),
        b"preserve"
    );
}

#[test]
fn changed_revision_and_cross_instance_plans_are_rejected() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    fixture.write(PARTIAL, b"metadata-only");
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    store.append_log(log()).unwrap();
    assert!(matches!(
        store.apply_cleanup(plan),
        Err(RecoveryError::IdentityChanged)
    ));
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    drop(store);
    assert!(matches!(
        ArtifactStore::open(&fixture.0),
        Err(RecoveryError::AlreadyOwned)
    ));
    let other = Fixture::new();
    let mut different = other.open();
    assert!(matches!(
        different.apply_cleanup(plan),
        Err(RecoveryError::IdentityChanged)
    ));
    let _reopened = fixture.open();
    assert!(fixture.path(PARTIAL).exists());
}

#[test]
fn cleanup_preflight_preserves_every_entry_when_one_inode_changes_in_place() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    fixture.write(PARTIAL, b"metadata-only");
    let second = PARTIAL.replace("00000000000000000010.part", "00000000000000000011.part");
    fixture.write(&second, b"metadata-only");
    let plan = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    fs::write(fixture.path(&second), b"changed-same-size").unwrap();
    assert!(matches!(
        store.apply_cleanup(plan),
        Err(RecoveryError::IdentityChanged)
    ));
    assert_eq!(fs::read(fixture.path(PARTIAL)).unwrap(), b"metadata-only");
    assert_eq!(
        fs::read(fixture.path(&second)).unwrap(),
        b"changed-same-size"
    );
}

#[test]
fn hardlink_symlink_and_nonprivate_files_are_refused_before_writes() {
    use std::os::unix::fs::PermissionsExt;
    for hostile in ["hardlink", "symlink", "permissions"] {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let external = fixture.0.join("external-sentinel");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&external)
            .unwrap()
            .write_all(b"preserve")
            .unwrap();
        match hostile {
            "hardlink" => fs::hard_link(&external, fixture.path("metadata.0.log")).unwrap(),
            "symlink" => symlink(&external, fixture.path("metadata.0.log")).unwrap(),
            _ => {
                fixture.write("metadata.0.log", b"preserve");
                fs::set_permissions(
                    fixture.path("metadata.0.log"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap();
            }
        }
        assert_eq!(store.append_log(log()), Err(RecoveryError::UnsafePath));
        drop(store);
        assert!(matches!(
            ArtifactStore::open(&fixture.0),
            Err(RecoveryError::UnsafePath)
        ));
        assert_eq!(fs::read(external).unwrap(), b"preserve");
    }
}

#[test]
fn concurrent_owner_directory_replacement_and_readonly_mount_refuse() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    assert!(matches!(
        ArtifactStore::open(&fixture.0),
        Err(RecoveryError::AlreadyOwned)
    ));
    fs::rename(fixture.path(""), fixture.0.join("retained-directory")).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(fixture.path(""))
        .unwrap();
    assert_eq!(store.append_log(log()), Err(RecoveryError::IdentityChanged));
    assert_eq!(fs::read_dir(fixture.path("")).unwrap().count(), 0);
    assert!(
        std::path::Path::new("/work/Cargo.toml").is_file(),
        "required isolated verifier mount"
    );
    assert!(matches!(
        ArtifactStore::open(std::path::Path::new("/work")),
        Err(RecoveryError::Io)
    ));
}

#[test]
fn retained_cleanup_preview_cannot_pin_unlinked_log_bytes_past_budget() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    for index in 0..4 {
        fixture.write(
            &format!("metadata.{index}.log"),
            &vec![b'0'; usize::try_from(MAX_LOG_BYTES).unwrap()],
        );
    }
    let held = store
        .preview_cleanup(&CleanupReason::PrivacyDisabled, 10)
        .unwrap();
    store.append_log(log()).unwrap();
    let deleted = deleted_bytes(&fixture);
    assert!(
        store.snapshot().unwrap().log_bytes + deleted <= 4 * MAX_LOG_BYTES,
        "retained preview must not keep deleted log bytes allocated outside inventory"
    );
    drop(held);
}
