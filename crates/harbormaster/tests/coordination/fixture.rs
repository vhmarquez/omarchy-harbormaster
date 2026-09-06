//! Disposable caller-owned state: diagnostic projection never receives these handles.
use harbormaster::{
    recovery::{ArtifactStore, LogReason, LogRecord, RecoveryStatus},
    storage::{DatabaseWorker, Request, Response, StoragePolicy, StorageStatus},
};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

pub(super) const CANARIES: [&str; 3] = [
    "SYNTHETIC_DIAGNOSTIC_PATH_CANARY",
    "SYNTHETIC_PROMPT_CANARY",
    "SYNTHETIC_CREDENTIAL_CANARY",
];
pub(super) struct Fixture {
    root: PathBuf,
    worker: DatabaseWorker,
    recovery: ArtifactStore,
}
impl Fixture {
    pub fn new() -> Self {
        use std::io::Write;
        let root = std::env::temp_dir().join(format!("{}-{}", CANARIES[0], std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let mut private = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("private-unrelated-fixture"))
            .unwrap();
        private.write_all(CANARIES.join("\n").as_bytes()).unwrap();
        let worker = DatabaseWorker::open(&root).unwrap();
        let mut recovery = ArtifactStore::open(&root).unwrap();
        recovery
            .append_log(LogRecord {
                observed_ms: 10,
                reason: LogReason::MissingHook,
                count: 1,
                unknown_gap: true,
            })
            .unwrap();
        Self {
            root,
            worker,
            recovery,
        }
    }

    pub fn snapshots(&self) -> (StorageStatus, StoragePolicy, RecoveryStatus) {
        let Response::Status(status) = self.call(Request::Status) else {
            panic!("missing status");
        };
        let Response::Policy(policy) = self.call(Request::Policy) else {
            panic!("missing policy");
        };
        (status, policy, self.recovery.snapshot().unwrap())
    }

    pub fn file_state(&self) -> BTreeMap<PathBuf, (Vec<u8>, u32, SystemTime)> {
        let mut files = BTreeMap::new();
        collect(&self.root, &mut files);
        files
    }

    fn call(&self, request: Request) -> Response {
        self.worker
            .try_submit(request)
            .unwrap()
            .wait_timeout(Duration::from_secs(5))
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.worker.request_shutdown();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.worker.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn collect(path: &Path, files: &mut BTreeMap<PathBuf, (Vec<u8>, u32, SystemTime)>) {
    let directory = fs::metadata(path).unwrap();
    files.insert(
        path.to_owned(),
        (
            Vec::new(),
            directory.permissions().mode(),
            directory.modified().unwrap(),
        ),
    );
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        if metadata.is_dir() {
            collect(&entry.path(), files);
        } else {
            files.insert(
                entry.path(),
                (
                    fs::read(entry.path()).unwrap(),
                    metadata.permissions().mode(),
                    metadata.modified().unwrap(),
                ),
            );
        }
    }
}
