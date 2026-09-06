pub use super::event::*;
use harbormaster::protocol::{ProducerGeneration, Revision, Seq};
use harbormaster::storage::{
    DatabaseWorker, ReceiptError, Registration, Request, Response, SnapshotQuery,
};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Fixture {
    pub path: PathBuf,
    worker: Option<DatabaseWorker>,
}

impl Fixture {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hb storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let worker = Some(DatabaseWorker::open(&path).unwrap());
        Self { path, worker }
    }

    pub fn worker(&self) -> &DatabaseWorker {
        self.worker.as_ref().unwrap()
    }

    pub fn call(&self, request: Request) -> Result<Response, ReceiptError> {
        let mut ticket = self.worker().try_submit(request).unwrap();
        ticket.wait_timeout(Duration::from_secs(3))
    }

    pub fn revision(&self) -> Revision {
        match self
            .call(Request::Snapshot(SnapshotQuery {
                expected_revision: None,
                after_run: None,
                limit: 100,
            }))
            .unwrap()
        {
            Response::Snapshot(page) => page.revision,
            other => panic!("unexpected reply {other:?}"),
        }
    }

    pub fn close(&mut self) {
        let worker = self.worker.take().unwrap();
        worker.request_shutdown();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !worker.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(worker.is_finished(), "fixture worker did not stop");
    }

    pub fn reopen(&mut self) {
        self.close();
        self.worker = Some(DatabaseWorker::open(&self.path).unwrap());
    }

    pub fn recover(&mut self, floor: Revision) {
        assert!(self.worker.is_none());
        self.worker = Some(DatabaseWorker::recover_backup(&self.path, floor).unwrap());
    }

    pub fn register(&self, producer: &str, run: &str, next: u64) -> ProducerGeneration {
        match self
            .call(Request::Register(Registration {
                expected_revision: self.revision(),
                producer_id: producer.parse().unwrap(),
                run_id: run.parse().unwrap(),
                harness: harbormaster::protocol::HarnessKind::Hermes,
                next_sequence: Seq::new(next),
                previous_generation: None,
            }))
            .unwrap()
        {
            Response::Registered { generation, .. } => generation,
            other => panic!("unexpected reply {other:?}"),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.request_shutdown();
            let deadline = Instant::now() + Duration::from_secs(3);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if !worker.is_finished() {
                return;
            }
        }
        // Only this fixture's freshly created private directory is removed.
        let _ = fs::remove_dir_all(&self.path);
    }
}
