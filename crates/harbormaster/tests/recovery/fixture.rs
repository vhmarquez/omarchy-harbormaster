//! Disposable public-API fixture and actual detached-file allocation inspection.
use harbormaster::recovery::ArtifactStore;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hb-public-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.0.join("harbormaster/recovery").join(name)
    }
    pub(super) fn write(&self, name: &str, bytes: &[u8]) {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.path(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    pub(super) fn open(&self) -> ArtifactStore {
        ArtifactStore::open(&self.0).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub(super) fn deleted_bytes(fixture: &Fixture) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let prefix = fixture.path("").to_string_lossy().into_owned();
    let mut identities = std::collections::BTreeSet::new();
    let mut bytes = 0;
    for entry in fs::read_dir("/proc/self/fd").unwrap() {
        let path = entry.unwrap().path();
        let Ok(target) = fs::read_link(&path) else {
            continue;
        };
        let target = target.to_string_lossy();
        if target.starts_with(&prefix) && target.ends_with(" (deleted)") {
            let metadata = fs::metadata(path).unwrap();
            if identities.insert((metadata.dev(), metadata.ino())) {
                bytes += metadata.len();
            }
        }
    }
    bytes
}
