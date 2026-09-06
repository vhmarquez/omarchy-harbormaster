//! Real SIGKILL at the storage transaction and unconsumed-receipt boundaries.
//! The parent and child both run inside the verifier's disposable isolation.
#[path = "storage/event.rs"]
mod event;
use event::{PRODUCER, RUN, key, write_set};
use harbormaster::protocol::{HarnessKind, Revision, Seq};
use harbormaster::storage::{
    DatabaseWorker, Registration, Request, Response, SnapshotQuery, WriteSet,
};
use rusqlite::Connection;
use std::fs;
use std::os::unix::{fs::DirBuilderExt, process::ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
const CHILD_PATH: &str = "HARBORMASTER_STORAGE_CRASH_FIXTURE";
const DEADLINE: Duration = Duration::from_secs(5);

struct ProcessFixture {
    path: PathBuf,
    child: Child,
}

impl ProcessFixture {
    fn start(test: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "hb crash-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(CHILD_PATH, &path)
            .spawn()
            .unwrap();
        Self { path, child }
    }

    fn ready(&mut self) -> WriteSet {
        self.until(|path| path.join("ready.json").is_file());
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(self.path.join("ready.json")).unwrap()).unwrap();
        let generation = value["fact"]["generation"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        // Registration is the only prior write in this fixed fixture.
        let set = write_set(&generation, Revision::new(1), 1);
        assert_eq!(value, serde_json::to_value(&set).unwrap());
        set
    }

    fn until(&mut self, mut observed: impl FnMut(&Path) -> bool) {
        let deadline = Instant::now() + DEADLINE;
        while !observed(&self.path) {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "child exited early"
            );
            assert!(Instant::now() < deadline, "child boundary was not observed");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn kill(&mut self) {
        self.child.kill().unwrap();
        assert_eq!(self.child.wait().unwrap().signal(), Some(9));
    }

    fn database(&self) -> PathBuf {
        self.path.join("harbormaster/state.db")
    }
}

impl Drop for ProcessFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn call(worker: &DatabaseWorker, request: Request) -> Response {
    worker
        .try_submit(request)
        .unwrap()
        .wait_timeout(DEADLINE)
        .unwrap()
}

fn child(path: &Path) {
    let worker = DatabaseWorker::open(path).unwrap();
    let Response::Registered {
        generation,
        revision,
    } = call(
        &worker,
        Request::Register(Registration {
            expected_revision: Revision::new(0),
            producer_id: PRODUCER.parse().unwrap(),
            run_id: RUN.parse().unwrap(),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(1),
            previous_generation: None,
        }),
    )
    else {
        panic!("missing registration")
    };
    let set = write_set(&generation, revision, 1);
    fs::write(
        path.join("preparing.json"),
        serde_json::to_vec(&set).unwrap(),
    )
    .unwrap();
    fs::rename(path.join("preparing.json"), path.join("ready.json")).unwrap();
    let deadline = Instant::now() + DEADLINE;
    while !path.join("go").exists() {
        assert!(Instant::now() < deadline, "parent never released commit");
        std::thread::sleep(Duration::from_millis(1));
    }
    let ticket = worker.try_submit(Request::Commit(Box::new(set))).unwrap();
    // Intentionally do not consume the receipt. The parent proves either the
    // live write lock or committed rows, then kills this entire process.
    std::thread::sleep(DEADLINE + DEADLINE);
    drop(ticket);
    panic!("parent failed to kill fixture child");
}

fn assert_reopened(fixture: &ProcessFixture, set: &WriteSet, committed: bool) {
    let worker = DatabaseWorker::open(&fixture.path).unwrap();
    let Response::Snapshot(page) = call(
        &worker,
        Request::Snapshot(SnapshotQuery {
            expected_revision: None,
            after_run: None,
            limit: 100,
        }),
    ) else {
        panic!("missing snapshot")
    };
    assert_eq!(page.runs.len(), usize::from(committed));
    let Response::Producer(Some(producer)) =
        call(&worker, Request::Producer(PRODUCER.parse().unwrap()))
    else {
        panic!("missing producer")
    };
    assert!(!producer.active);
    assert_eq!(
        producer.next_sequence,
        Some(Seq::new(if committed { 2 } else { 1 }))
    );
    let outcome = call(&worker, Request::Outcome(key(set)));
    if committed {
        assert!(matches!(outcome, Response::Outcome(Some(_))));
        assert_eq!(
            call(&worker, Request::Commit(Box::new(set.clone()))),
            Response::Committed {
                revision: Revision::new(2),
                duplicate: true,
            }
        );
    } else {
        assert_eq!(outcome, Response::Outcome(None));
    }
    worker.request_shutdown();
    let deadline = Instant::now() + DEADLINE;
    while !worker.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(worker.is_finished());
}

#[test]
fn kill_during_atomic_write_rolls_back_every_effect() {
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        child(Path::new(&path));
        return;
    }
    let mut fixture = ProcessFixture::start("kill_during_atomic_write_rolls_back_every_effect");
    let set = fixture.ready();
    let probe = Connection::open(fixture.database()).unwrap();
    probe.busy_timeout(Duration::ZERO).unwrap();
    // Fault injection only in this private fixture, AFTER manager startup and
    // registration. This finite query holds the write transaction open. The
    // parent observes an extended write-lock window; it does not claim an exact
    // SQL instruction at SIGKILL. No product hooks or SQL API are introduced.
    probe.execute_batch("CREATE TRIGGER crash_fixture AFTER INSERT ON facts BEGIN
        SELECT (WITH RECURSIVE delay(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM delay WHERE n<100000000) SELECT sum(n) FROM delay);
        END;").unwrap();
    fs::write(fixture.path.join("go"), b"go").unwrap();
    let mut busy_since = None;
    fixture.until(
        |_| match probe.execute_batch("BEGIN IMMEDIATE; ROLLBACK;") {
            Ok(()) => {
                busy_since = None;
                false
            }
            Err(error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) => {
                busy_since.get_or_insert_with(Instant::now).elapsed() >= Duration::from_millis(100)
            }
            Err(error) => panic!("unexpected lock probe: {error}"),
        },
    );
    fixture.kill();
    probe.execute_batch("DROP TRIGGER crash_fixture;").unwrap();
    for table in ["facts", "projections", "attention", "outbox", "tombstones"] {
        let count: u32 = probe
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "partial effect in {table}");
    }
    drop(probe);
    assert_reopened(&fixture, &set, false);
}

#[test]
fn kill_after_commit_before_receipt_preserves_atomic_outcome_and_retry() {
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        child(Path::new(&path));
        return;
    }
    let mut fixture = ProcessFixture::start(
        "kill_after_commit_before_receipt_preserves_atomic_outcome_and_retry",
    );
    let set = fixture.ready();
    let probe = Connection::open(fixture.database()).unwrap();
    fs::write(fixture.path.join("go"), b"go").unwrap();
    fixture.until(|_| {
        probe
            .query_row("SELECT count(*) FROM facts", [], |row| row.get::<_, u32>(0))
            .unwrap()
            == 1
    });
    fixture.kill();
    for table in ["facts", "projections", "attention", "outbox", "tombstones"] {
        let count: u32 = probe
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1, "missing committed effect in {table}");
    }
    drop(probe);
    assert_reopened(&fixture, &set, true);
}
