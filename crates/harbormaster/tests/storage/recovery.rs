use super::common::*;
use harbormaster::protocol::{Revision, Seq};
use harbormaster::storage::{
    DatabaseWorker, ReceiptError, Registration, Request, Response, StorageError,
};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

fn corrupt_fact_page(database: &std::path::Path) {
    use std::io::{Seek, Write};
    let connection = rusqlite::Connection::open(database).unwrap();
    let page: u32 = connection
        .query_row(
            "SELECT rootpage FROM sqlite_schema WHERE name='facts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(connection);
    let mut file = fs::OpenOptions::new().write(true).open(database).unwrap();
    file.seek(std::io::SeekFrom::Start(u64::from(page - 1) * 4096))
        .unwrap();
    file.write_all(&[0xff; 32]).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn explicit_worker_recovery_requires_a_nonoverflowing_trusted_revision_upper_bound() {
    let mut fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let set = write_set(&generation, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(set.clone())))
        .unwrap();
    fixture.call(Request::Backup).unwrap();
    fixture.close();
    let database = fixture.path.join("harbormaster/state.db");
    corrupt_fact_page(&database);
    assert_eq!(
        DatabaseWorker::open(&fixture.path).err(),
        Some(StorageError::CorruptDatabase)
    );
    let before = fs::read(&database).unwrap();
    assert_eq!(
        DatabaseWorker::recover_backup(&fixture.path, Revision::new(u64::MAX)).err(),
        Some(StorageError::ResourceExhausted)
    );
    assert_eq!(fs::read(&database).unwrap(), before);
    fixture.recover(Revision::new(15));
    assert_eq!(fixture.revision(), Revision::new(16));
    assert!(matches!(
        fixture.call(Request::Outcome(key(&set))),
        Ok(Response::Outcome(Some(_)))
    ));
    let Response::Producer(Some(producer)) = fixture
        .call(Request::Producer(PRODUCER.parse().unwrap()))
        .unwrap()
    else {
        panic!("missing recovered producer")
    };
    assert!(!producer.active);
}

#[test]
fn reopen_preserves_committed_outcome_and_refuses_old_generation_new_work() {
    let mut fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let first = write_set(&generation, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(first.clone())))
        .unwrap();
    fixture.reopen();
    assert!(matches!(
        fixture.call(Request::Outcome(key(&first))),
        Ok(Response::Outcome(Some(_)))
    ));
    let Response::Producer(Some(saved)) = fixture
        .call(Request::Producer(PRODUCER.parse().unwrap()))
        .unwrap()
    else {
        panic!("missing saved producer")
    };
    assert!(!saved.active);
    let mut next = write_set(&generation, fixture.revision(), 2);
    next.fact.event_id = "11111111-1111-4111-8111-111111111112".parse().unwrap();
    next.attention.clear();
    next.outbox = None;
    next.tombstone = None;
    assert_eq!(
        fixture.call(Request::Commit(Box::new(next))),
        Err(ReceiptError::Storage(StorageError::StaleGeneration))
    );
    let Response::Registered {
        generation: fresh, ..
    } = fixture
        .call(Request::Register(Registration {
            expected_revision: fixture.revision(),
            producer_id: PRODUCER.parse().unwrap(),
            run_id: RUN.parse().unwrap(),
            harness: harbormaster::protocol::HarnessKind::Hermes,
            next_sequence: Seq::new(2),
            previous_generation: Some(generation.clone()),
        }))
        .unwrap()
    else {
        panic!("missing fresh registration")
    };
    assert_ne!(fresh, generation);
}

#[test]
fn live_backup_and_restore_reject_every_pre_restore_revision() {
    let fixture = Fixture::new();
    let generation = fixture.register(PRODUCER, RUN, 1);
    let first = write_set(&generation, fixture.revision(), 1);
    fixture
        .call(Request::Commit(Box::new(first.clone())))
        .unwrap();
    assert_eq!(fixture.call(Request::Backup), Ok(Response::BackupComplete));
    let mut second = write_set(&generation, fixture.revision(), 2);
    second.fact.event_id = "11111111-1111-4111-8111-111111111112".parse().unwrap();
    second.attention.clear();
    second.outbox = None;
    second.tombstone = None;
    fixture.call(Request::Commit(Box::new(second))).unwrap();
    let before_restore = fixture.revision();
    let Response::Restored { revision } = fixture.call(Request::RestoreBackup).unwrap() else {
        panic!("restore did not finish")
    };
    assert!(revision.value() > before_restore.value());
    assert!(matches!(
        fixture.call(Request::Outcome(key(&first))),
        Ok(Response::Outcome(Some(_)))
    ));
    assert_eq!(
        fixture.call(Request::Snapshot(harbormaster::storage::SnapshotQuery {
            expected_revision: Some(before_restore),
            after_run: None,
            limit: 100,
        })),
        Err(ReceiptError::Storage(StorageError::StaleRevision))
    );
}

#[test]
fn private_fixed_state_is_exclusive_and_rejects_foreign_db_without_writing_it() {
    let mut fixture = Fixture::new();
    assert_eq!(
        DatabaseWorker::open(&fixture.path).err(),
        Some(StorageError::AlreadyOwned)
    );
    let directory = fixture.path.join("harbormaster");
    let database = directory.join("state.db");
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&database).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fixture.close();
    fs::remove_file(&database).unwrap();
    let foreign = rusqlite::Connection::open(&database).unwrap();
    foreign.execute_batch("CREATE TABLE foreign_records(value TEXT); INSERT INTO foreign_records VALUES('foreign-fixture-canary');").unwrap();
    drop(foreign);
    fs::set_permissions(&database, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fs::read(&database).unwrap();
    assert_eq!(
        DatabaseWorker::open(&fixture.path).err(),
        Some(StorageError::ForeignDatabase)
    );
    assert_eq!(fs::read(&database).unwrap(), before);
    assert!(!directory.join("state.db-wal").exists());
    assert!(!directory.join("state.db-shm").exists());
}

#[test]
fn symlinked_database_is_rejected_and_unrelated_target_remains_exact() {
    let mut fixture = Fixture::new();
    fixture.close();
    let database = fixture.path.join("harbormaster/state.db");
    fs::remove_file(&database).unwrap();
    let unrelated = fixture.path.join("unrelated-fixture");
    fs::write(&unrelated, b"unrelated-canary").unwrap();
    symlink(&unrelated, database).unwrap();
    assert_eq!(
        DatabaseWorker::open(&fixture.path).err(),
        Some(StorageError::UnsafePath)
    );
    assert_eq!(fs::read(unrelated).unwrap(), b"unrelated-canary");
}
