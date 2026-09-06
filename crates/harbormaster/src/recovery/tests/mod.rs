//! Private synthetic qualification. No live mapping or exported test constructor.
mod bounds;
mod crash;
mod privacy;
mod replay;

use super::{
    AdapterRecord, ArtifactStore, SpoolReceipt,
    provenance::{Mapping, Source},
};
use crate::protocol::HarnessKind;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const REVIEWED_SYNTHETIC: &[Mapping] = &[Mapping {
    harness: HarnessKind::Hermes,
    version: "synthetic-m1-only-v1",
    turn: Source::Metadata,
    session: Source::Metadata,
    health_version: Source::Metadata,
}];
const CANARIES: [&str; 5] = [
    "SYNTHETIC_PROMPT_CANARY",
    "SYNTHETIC_RESPONSE_CANARY",
    "SYNTHETIC_TERMINAL_CANARY",
    "SYNTHETIC_ERROR_CANARY",
    "SYNTHETIC_CREDENTIAL_CANARY",
];
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture(pub PathBuf);
impl Fixture {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hb-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    pub(crate) fn artifacts(&self) -> PathBuf {
        self.0.join("harbormaster/recovery")
    }
    pub(crate) fn open(&self) -> ArtifactStore {
        ArtifactStore::open(&self.0).unwrap()
    }
    fn private_file(&self, name: &str, bytes: &[u8]) {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.artifacts().join(name))
            .unwrap();
        file.write_all(bytes).unwrap();
    }
    fn scan_canaries(&self) {
        scan_canaries(&self.artifacts());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scan_canaries(path: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        for canary in CANARIES {
            assert!(
                !bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes()),
                "persisted synthetic content marker"
            );
        }
    }
}

pub(crate) fn source(sequence: u64, generation: u64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"metadata":{
        "event_id":format!("00000000-0000-4000-8000-{sequence:012x}"),
        "producer_id":"00000000-0000-4000-8000-000000000002",
        "generation":format!("00000000-0000-4000-8000-{generation:012x}"),
        "run_id":"00000000-0000-4000-8000-000000000004", "seq":sequence.to_string(),
        "kind":"turn.started", "payload":{"turn_id":"metadata-turn-1"}},
        "prompt":CANARIES[0], "response":CANARIES[1], "terminal":CANARIES[2], "error":CANARIES[3], "environment":CANARIES[4]
    })).unwrap()
}

fn input(source: &[u8]) -> AdapterRecord<'_> {
    AdapterRecord {
        harness: HarnessKind::Hermes,
        version: REVIEWED_SYNTHETIC[0].version,
        source,
    }
}

pub(crate) fn stage_synthetic(
    store: &mut ArtifactStore,
    source: &[u8],
    observed_ms: u64,
) -> SpoolReceipt {
    store
        .stage_reviewed(input(source), observed_ms, REVIEWED_SYNTHETIC)
        .unwrap()
}

pub(crate) fn read_synthetic(store: &ArtifactStore, now_ms: u64) -> Vec<super::ReplayEntry> {
    store.read_reviewed(now_ms, REVIEWED_SYNTHETIC).unwrap()
}

pub(super) fn checkpoint(phase: &str) {
    if std::env::var("HB_RECOVERY_CRASH_PHASE").ok().as_deref() == Some(phase) {
        let path = std::env::var_os("HB_RECOVERY_CRASH_ROOT").unwrap();
        let mut marker = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(Path::new(&path).join("checkpoint"))
            .unwrap();
        std::io::Write::write_all(&mut marker, phase.as_bytes()).unwrap();
        marker.sync_all().unwrap();
        loop {
            std::thread::park();
        }
    }
}
