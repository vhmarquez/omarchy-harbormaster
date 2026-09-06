use super::*;

#[test]
fn all_backup_stage_and_rollback_sidecars_reject_external_hardlinks() {
    for basename in ["state.backup.db", "state.staging.db", "state.rollback.db"] {
        for suffix in ["-wal", "-shm", "-journal"] {
            let fixture = Fixture::new();
            let mut engine = fixture.open();
            engine.execute(&Request::Backup).unwrap();
            let sentinel = fixture.root.join("unrelated-sentinel");
            let bytes = vec![0x5a; 32768];
            std::fs::write(&sentinel, &bytes).unwrap();
            std::fs::set_permissions(&sentinel, std::fs::Permissions::from_mode(0o600)).unwrap();
            std::fs::hard_link(
                &sentinel,
                fixture
                    .root
                    .join("harbormaster")
                    .join(format!("{basename}{suffix}")),
            )
            .unwrap();
            assert_eq!(
                engine.execute(&Request::Backup),
                Err(StorageError::UnsafePath)
            );
            assert_eq!(
                engine.execute(&Request::RestoreBackup),
                Err(StorageError::UnsafePath)
            );
            assert_eq!(std::fs::read(&sentinel).unwrap(), bytes);
        }
    }
}

#[test]
fn explicit_corrupt_page_recovery_uses_verified_backup_and_revision_floor() {
    use std::io::{Seek, Write};
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    commit(&mut engine, &set);
    engine.execute(&Request::Backup).unwrap();
    let page: u32 = engine
        .connection
        .as_ref()
        .unwrap()
        .query_row(
            "SELECT rootpage FROM sqlite_schema WHERE name='facts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(engine);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(fixture.database())
        .unwrap();
    file.seek(std::io::SeekFrom::Start(u64::from(page - 1) * 4096))
        .unwrap();
    file.write_all(&[0xff; 32]).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert!(matches!(
        Engine::open(&fixture.root),
        Err(StorageError::CorruptDatabase)
    ));
    let engine = Engine::recover_backup(&fixture.root, Revision::new(15)).unwrap();
    assert_eq!(
        queries::revision(engine.connection.as_ref().unwrap()).unwrap(),
        Revision::new(16)
    );
    assert_eq!(count(&engine, "facts"), 1);
    assert!(
        !queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
}

#[test]
fn restore_never_reuses_a_pre_restore_revision() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    commit(&mut engine, &set);
    engine.execute(&Request::Backup).unwrap();
    engine
        .execute(&Request::MarkReviewed(ReviewUpdate {
            expected_revision: Revision::new(2),
            outcome: outcome(&set),
        }))
        .unwrap();
    let Response::Restored { revision } = engine.execute(&Request::RestoreBackup).unwrap() else {
        panic!()
    };
    assert!(
        revision > Revision::new(3),
        "restore must invalidate all prior revision-bound commands"
    );
    assert_eq!(
        engine.execute(&Request::MarkReviewed(ReviewUpdate {
            expected_revision: Revision::new(3),
            outcome: outcome(&set)
        })),
        Err(StorageError::StaleRevision)
    );
}

#[test]
fn live_backup_and_restore_preserve_outcome_and_require_reconciliation() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation, revision, 1, 3);
    commit(&mut engine, &set);
    engine.execute(&Request::Backup).unwrap();
    let backup_bytes = std::fs::read(fixture.root.join("harbormaster/state.backup.db")).unwrap();
    assert_eq!(
        &backup_bytes[18..20],
        &[1, 1],
        "published backup must need no WAL/SHM"
    );
    engine
        .execute(&Request::MarkReviewed(ReviewUpdate {
            expected_revision: Revision::new(2),
            outcome: outcome(&set),
        }))
        .unwrap();
    engine.execute(&Request::RestoreBackup).unwrap();
    let row = queries::outcome(engine.connection.as_ref().unwrap(), &outcome(&set))
        .unwrap()
        .unwrap();
    assert!(!row.attention[0].reviewed);
    assert_eq!(row.delivery, Some(DeliveryState::Pending));
    assert!(
        !queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
    assert!(fixture.root.join("harbormaster/state.backup.db").is_file());
    assert!(!fixture.root.join("harbormaster/state.rollback.db").exists());
}

#[test]
fn reopening_retires_saved_generations_without_clearing_outcomes() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    let (generation, revision) = register(&mut engine, 1);
    let set = write(generation.clone(), revision, 1, 3);
    commit(&mut engine, &set);
    drop(engine);
    let mut engine = fixture.open();
    assert!(
        !queries::producer(engine.connection.as_ref().unwrap(), &id(1))
            .unwrap()
            .unwrap()
            .active
    );
    let mut next = write(generation.clone(), Revision::new(3), 2, 4);
    next.tombstone = None;
    assert_eq!(
        engine.execute(&Request::Commit(Box::new(next))),
        Err(StorageError::StaleGeneration)
    );
    let Response::Registered {
        generation: fresh, ..
    } = engine
        .execute(&Request::Register(Registration {
            expected_revision: Revision::new(3),
            producer_id: id(1),
            run_id: id(2),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(2),
            previous_generation: Some(generation.clone()),
        }))
        .unwrap()
    else {
        panic!()
    };
    assert_ne!(fresh, generation);
    assert_eq!(count(&engine, "attention"), 1);
}

#[test]
fn migration_uses_verified_backup_and_future_schema_is_untouched() {
    let fixture = Fixture::new();
    let engine = fixture.open();
    legacy_schema(&engine, 1);
    drop(engine);
    let engine = fixture.open();
    assert_eq!(
        schema::inspect(engine.connection.as_ref().unwrap()).unwrap(),
        schema::VERSION
    );
    assert_eq!(
        schema::header(&fixture.root.join("harbormaster/state.backup.db")).unwrap(),
        1
    );
    let mut engine = engine;
    engine.execute(&Request::RestoreBackup).unwrap();
    assert_eq!(
        schema::inspect(engine.connection.as_ref().unwrap()).unwrap(),
        schema::VERSION
    );
    assert_eq!(
        schema::header(&fixture.root.join("harbormaster/state.backup.db")).unwrap(),
        1
    );
    engine
        .connection
        .as_ref()
        .unwrap()
        .execute_batch("PRAGMA user_version=4")
        .unwrap();
    drop(engine);
    let before = std::fs::read(fixture.database()).unwrap();
    assert!(matches!(
        Engine::open(&fixture.root),
        Err(StorageError::FutureSchema)
    ));
    assert_eq!(std::fs::read(fixture.database()).unwrap(), before);
}

#[test]
fn unsafe_symlink_and_foreign_wal_header_have_no_database_side_effects() {
    let fixture = Fixture::new();
    let engine = fixture.open();
    drop(engine);
    let path = fixture.database();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[18] = 2;
    bytes[19] = 2;
    bytes[68..72].fill(0);
    std::fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        Engine::open(&fixture.root),
        Err(StorageError::ForeignDatabase)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(!fixture.root.join("harbormaster/state.db-wal").exists());
    assert!(!fixture.root.join("harbormaster/state.db-shm").exists());
    std::fs::rename(&path, fixture.root.join("fixture-original")).unwrap();
    std::os::unix::fs::symlink(fixture.root.join("fixture-original"), &path).unwrap();
    assert!(matches!(
        Engine::open(&fixture.root),
        Err(StorageError::UnsafePath)
    ));
}

#[test]
fn ownership_is_exclusive_and_completed_inode_replacement_is_detected() {
    let fixture = Fixture::new();
    let mut engine = fixture.open();
    assert!(matches!(
        Engine::open(&fixture.root),
        Err(StorageError::AlreadyOwned)
    ));
    std::fs::rename(fixture.database(), fixture.root.join("fixture-original")).unwrap();
    std::fs::write(fixture.database(), b"unrelated fixture").unwrap();
    std::fs::set_permissions(fixture.database(), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        engine.execute(&Request::Backup),
        Err(StorageError::UnsafePath)
    );
    assert_eq!(
        std::fs::read(fixture.database()).unwrap(),
        b"unrelated fixture"
    );
}

pub(super) fn legacy_schema(engine: &Engine, version: i64) {
    let conn = engine.connection.as_ref().unwrap();
    for (name, _) in super::super::super::schema_layout::LEGACY {
        conn.execute_batch(&format!("DROP TABLE {name}")).unwrap();
    }
    for (name, sql) in super::super::super::schema_layout::LEGACY {
        if version == 1 && name == "maintenance" {
            continue;
        }
        conn.execute_batch(sql).unwrap();
    }
    conn.execute(
        "INSERT INTO metadata VALUES(1,?1,30)",
        [0_u64.to_be_bytes().as_slice()],
    )
    .unwrap();
    if version == 2 {
        conn.execute_batch("INSERT INTO maintenance VALUES(1,0,0,0,0)")
            .unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
}
