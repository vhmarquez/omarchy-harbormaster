use harbormaster::storage::{DatabaseWorker, Request, Response, SnapshotQuery, StorageError};
use std::fs::{self, DirBuilder, Permissions};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "hb-independent-path-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
    fn manager(&self) -> PathBuf {
        self.0.join("harbormaster")
    }
    fn file(&self, name: &str) -> PathBuf {
        self.manager().join(name)
    }
    fn prepare(&self) {
        DirBuilder::new()
            .mode(0o700)
            .create(self.manager())
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(
    worker: &DatabaseWorker,
    request: Request,
) -> Result<Response, harbormaster::storage::ReceiptError> {
    worker
        .try_submit(request)
        .unwrap()
        .wait_timeout(Duration::from_secs(3))
}
fn stop(worker: DatabaseWorker) {
    worker.request_shutdown();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !worker.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
fn query() -> Request {
    Request::Snapshot(SnapshotQuery {
        expected_revision: None,
        after_run: None,
        limit: 10,
    })
}
fn private(path: &Path) {
    fs::set_permissions(path, Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn manager_open_and_backup_uses_only_private_fixed_paths() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).expect("actual private database open");
    assert!(matches!(
        request(&worker, query()),
        Ok(Response::Snapshot(_))
    ));
    assert_eq!(
        request(&worker, Request::Backup),
        Ok(Response::BackupComplete)
    );
    for entry in fs::read_dir(f.manager()).unwrap() {
        let entry = entry.unwrap();
        let info = entry.metadata().unwrap();
        assert_eq!(
            info.permissions().mode() & 0o7777,
            0o600,
            "{}",
            entry.path().display()
        );
    }
    stop(worker);
}

#[test]
fn known_entries_reject_symlinks_and_hardlinks_without_touching_targets() {
    for name in [
        "state.db",
        "state.db-wal",
        "state.db-shm",
        "state.db-journal",
        "state.backup.db",
        "state.staging.db",
        "state.rollback.db",
        "state.staging.db-journal",
        "state.lock",
    ] {
        for link in [false, true] {
            let f = Fixture::new();
            f.prepare();
            let target = f.0.join("protected");
            fs::write(&target, b"fixture sentinel").unwrap();
            private(&target);
            if link {
                std::os::unix::fs::symlink(&target, f.file(name)).unwrap();
            } else {
                fs::hard_link(&target, f.file(name)).unwrap();
            }
            assert!(
                matches!(DatabaseWorker::open(&f.0), Err(StorageError::UnsafePath)),
                "{name}, symlink={link}"
            );
            assert_eq!(fs::read(&target).unwrap(), b"fixture sentinel");
        }
    }
}

#[test]
fn backup_sidecars_must_not_follow_symlinks_or_modify_targets() {
    for name in [
        "state.backup.db-wal",
        "state.backup.db-shm",
        "state.backup.db-journal",
    ] {
        let f = Fixture::new();
        let worker = DatabaseWorker::open(&f.0).unwrap();
        assert_eq!(
            request(&worker, Request::Backup),
            Ok(Response::BackupComplete)
        );
        let header = fs::read(f.file("state.backup.db")).unwrap();
        println!(
            "backup header journal bytes {:?}, candidate {name}",
            &header[18..20]
        );
        let target = f.0.join("protected");
        fs::write(&target, vec![0xa7; 32768]).unwrap();
        private(&target);
        let before = fs::read(&target).unwrap();
        std::os::unix::fs::symlink(&target, f.file(name)).unwrap();
        let result = request(&worker, Request::Backup);
        println!("backup with {name}: {result:?}");
        assert_eq!(
            fs::read(&target).unwrap(),
            before,
            "modified target through{name}"
        );
        assert!(result.is_err(), "accepted unsafe {name}");
        stop(worker);
    }
}

#[test]
fn fresh_database_refuses_other_owner_lock() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).unwrap();
    assert!(matches!(
        DatabaseWorker::open(&f.0),
        Err(StorageError::AlreadyOwned)
    ));
    assert!(request(&worker, query()).is_ok());
    stop(worker);
}

#[test]
fn retained_database_replacement_is_refused_and_preserved() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).unwrap();
    fs::rename(f.file("state.db"), f.0.join("original")).unwrap();
    fs::write(f.file("state.db"), b"replacement sentinel").unwrap();
    private(&f.file("state.db"));
    assert_eq!(
        request(&worker, Request::Backup),
        Err(harbormaster::storage::ReceiptError::Storage(
            StorageError::UnsafePath
        ))
    );
    assert_eq!(
        fs::read(f.file("state.db")).unwrap(),
        b"replacement sentinel"
    );
    stop(worker);
    assert_eq!(
        fs::read(f.file("state.db")).unwrap(),
        b"replacement sentinel"
    );
}

#[test]
fn foreign_future_and_corrupt_main_files_are_not_modified() {
    for case in ["foreign", "future", "corrupt"] {
        let f = Fixture::new();
        let worker = DatabaseWorker::open(&f.0).unwrap();
        stop(worker);
        let path = f.file("state.db");
        match case {
            "foreign" => {
                let c = rusqlite::Connection::open(&path).unwrap();
                c.pragma_update(None, "application_id", 123).unwrap();
            }
            "future" => {
                let c = rusqlite::Connection::open(&path).unwrap();
                c.pragma_update(None, "user_version", 99).unwrap();
            }
            _ => {
                fs::write(&path, b"invalid SQLite").unwrap();
            }
        }
        let before = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert!(DatabaseWorker::open(&f.0).is_err(), "{case}");
        assert_eq!(fs::read(&path).unwrap(), before, "{case}");
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            modified,
            "{case}"
        );
    }
}

#[test]
fn pre_migration_backup_is_usable_by_the_fixed_restore_api() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).unwrap();
    stop(worker);
    {
        let c = rusqlite::Connection::open(f.file("state.db")).unwrap();
        c.execute_batch("DROP TABLE maintenance; PRAGMA user_version=1;")
            .unwrap();
    }
    let worker = DatabaseWorker::open(&f.0).unwrap();
    let backup_before = fs::read(f.file("state.backup.db")).unwrap();
    assert!(
        matches!(
            request(&worker, Request::RestoreBackup),
            Ok(Response::Restored { .. })
        ),
        "pre-migration backup cannot be restored"
    );
    stop(worker);
    assert_eq!(fs::read(f.file("state.backup.db")).unwrap(), backup_before);
    let restored = rusqlite::Connection::open(f.file("state.db")).unwrap();
    let version: i64 = restored
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 2,
        "restored copy was not migrated to the active schema"
    );
    let rows: i64 = restored
        .query_row("SELECT count(*) FROM maintenance", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn backup_sidecars_reject_hardlinks_before_writing_external_inodes() {
    let mut violations = Vec::new();
    for name in [
        "state.backup.db-wal",
        "state.backup.db-shm",
        "state.backup.db-journal",
    ] {
        for content in [vec![], vec![0xa7; 32768]] {
            let f = Fixture::new();
            let worker = DatabaseWorker::open(&f.0).unwrap();
            assert_eq!(
                request(&worker, Request::Backup),
                Ok(Response::BackupComplete)
            );
            let target = f.0.join("protected");
            fs::write(&target, &content).unwrap();
            private(&target);
            fs::hard_link(&target, f.file(name)).unwrap();
            let result = request(&worker, Request::Backup);
            let actual = fs::read(&target).unwrap();
            println!(
                "hardlinked {name} initial{} result{result:?} after{}",
                content.len(),
                actual.len()
            );
            if actual != content {
                violations.push(format!(
                    "modified hardlinked target through{name}, before{} after{}",
                    content.len(),
                    actual.len()
                ));
            }
            if result.is_ok() {
                violations.push(format!("accepted hardlink{name} initial{}", content.len()));
            }
            stop(worker);
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("; "));
}

#[test]
fn oversized_existing_wal_is_rejected_before_sqlite_rewrites_it() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).unwrap();
    stop(worker);
    let wal = f.file("state.db-wal");
    let length = 4_u64 * 1024 * 1024 + 16_384 * (4096 + 24) + 33;
    let file = fs::File::create(&wal).unwrap();
    private(&wal);
    file.set_len(length).unwrap();
    drop(file);
    let before = fs::metadata(&wal).unwrap().modified().unwrap();
    let result = DatabaseWorker::open(&f.0);
    let actual = fs::metadata(&wal).map(|m| m.len());
    println!(
        "oversized existing WAL before{length}, result {}, after{actual:?}",
        match &result {
            Ok(_) => "Ok".to_owned(),
            Err(e) => format!("{e:?}"),
        }
    );
    let rejected = result.is_err();
    if let Ok(worker) = result {
        stop(worker);
    }
    assert!(rejected, "oversized existing WAL was accepted");
    assert_eq!(fs::metadata(&wal).unwrap().len(), length);
    assert_eq!(fs::metadata(&wal).unwrap().modified().unwrap(), before);
}

#[test]
fn symlink_ancestors_and_public_state_roots_are_rejected_before_creation() {
    let f = Fixture::new();
    let target = f.0.join("target");
    DirBuilder::new().mode(0o700).create(&target).unwrap();
    let alias = f.0.join("alias");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    assert!(matches!(
        DatabaseWorker::open(&alias),
        Err(StorageError::UnsafePath)
    ));
    assert!(!target.join("harbormaster").exists());
    fs::set_permissions(&target, Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        DatabaseWorker::open(&target),
        Err(StorageError::UnsafePath)
    ));
    assert!(!target.join("harbormaster").exists());
}

#[test]
fn incomplete_restore_entries_require_recovery_without_deleting_data() {
    for name in ["state.staging.db", "state.rollback.db"] {
        let f = Fixture::new();
        let worker = DatabaseWorker::open(&f.0).unwrap();
        stop(worker);
        let original = fs::read(f.file("state.db")).unwrap();
        fs::write(f.file(name), b"incomplete restore fixture").unwrap();
        private(&f.file(name));
        assert!(matches!(
            DatabaseWorker::open(&f.0),
            Err(StorageError::RecoveryRequired)
        ));
        assert_eq!(
            fs::read(f.file(name)).unwrap(),
            b"incomplete restore fixture"
        );
        assert_eq!(fs::read(f.file("state.db")).unwrap(), original);
    }
}

#[test]
fn failed_backup_preserves_existing_backup_and_replacement_sentinel() {
    let f = Fixture::new();
    let worker = DatabaseWorker::open(&f.0).unwrap();
    assert_eq!(
        request(&worker, Request::Backup),
        Ok(Response::BackupComplete)
    );
    let backup = fs::read(f.file("state.backup.db")).unwrap();
    fs::write(f.file("state.staging.db"), b"preexisting sentinel").unwrap();
    private(&f.file("state.staging.db"));
    assert!(request(&worker, Request::Backup).is_err());
    assert_eq!(fs::read(f.file("state.backup.db")).unwrap(), backup);
    assert_eq!(
        fs::read(f.file("state.staging.db")).unwrap(),
        b"preexisting sentinel"
    );
    stop(worker);
}
