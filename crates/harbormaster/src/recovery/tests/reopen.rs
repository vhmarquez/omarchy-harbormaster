use super::*;
use crate::recovery::RecoveryError;
use std::{
    os::unix::fs::MetadataExt,
    time::{Duration, Instant},
};

impl Fixture {
    /// Only after all local stores/tokens have been dropped. A parallel test's
    /// fork can briefly retain a CLOEXEC lock description until its exec.
    pub(crate) fn reopen_after_release(&self) -> ArtifactStore {
        self.assert_no_local_lock_descriptor();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match ArtifactStore::open(&self.0) {
                Ok(store) => return store,
                Err(RecoveryError::AlreadyOwned) if Instant::now() < deadline => {
                    self.assert_no_local_lock_descriptor();
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("fixture reopen after local release failed: {error:?}"),
            }
        }
    }

    fn assert_no_local_lock_descriptor(&self) {
        let expected = fs::metadata(self.artifacts().join("recovery.lock")).unwrap();
        for entry in fs::read_dir("/proc/self/fd").unwrap() {
            match fs::metadata(entry.unwrap().path()) {
                Ok(actual) => assert_ne!(
                    (actual.dev(), actual.ino()),
                    (expected.dev(), expected.ino()),
                    "fixture still has a local lock descriptor; drop every owner/token first"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => panic!("cannot establish fixture lock release: {error:?}"),
            }
        }
    }
}

#[test]
#[should_panic(expected = "fixture still has a local lock descriptor")]
fn bounded_reopen_cannot_mask_a_retained_local_token() {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    let _held = stage_synthetic(&mut store, &source(1, 3), 10);
    drop(store);
    fixture.reopen_after_release();
}
