use super::*;
use std::{
    fs,
    io::Write,
    os::unix::{fs::OpenOptionsExt, process::ExitStatusExt},
    process::{Child, Command, Stdio},
};
const CHILD_ROOT: &str = "HB_COORDINATOR_CRASH_ROOT";

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn committed_before_spool_unlink_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let mut fixture = Fixture::at(artifacts::Fixture(root.into()));
    let generation = fixture.registered();
    let entry = fixture.stage(&generation, 1, "turn.completed");
    let (_, receipt) = fixture.commit(entry);
    assert!(receipt.is_some());
    let mut marker = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(fixture.root.0.join("committed"))
        .unwrap();
    marker.write_all(b"committed-before-unlink").unwrap();
    marker.sync_all().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn actual_sigkill_after_sqlite_commit_before_unlink_replays_one_original_outcome() {
    let root = artifacts::Fixture::new();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "coordinator::tests::crash::committed_before_spool_unlink_child",
                "--nocapture",
            ])
            .env(CHILD_ROOT, &root.0)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.0.join("committed").exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    child.0.kill().unwrap();
    assert_eq!(child.0.wait().unwrap().signal(), Some(9));
    let mut fixture = Fixture::at(root);
    let before = fixture.call(Request::Status);
    let entry = artifacts::read_synthetic(&fixture.store, 1000)
        .pop()
        .unwrap();
    let (progress, receipt) = fixture.commit(entry);
    assert_eq!(
        progress,
        CommitProgress::Committed {
            revision: Revision::new(2)
        }
    );
    assert_eq!(fixture.call(Request::Status), before);
    let Response::Status(status) = before else {
        panic!("status")
    };
    assert_eq!(
        (
            status.facts,
            status.tombstones,
            status.attention,
            status.pending_deliveries
        ),
        (1, 1, 1, 1)
    );
    let (receipt, entry) = receipt.unwrap();
    confirm_spool(&mut fixture.store, entry, &receipt).unwrap();
    assert_eq!(fixture.store.snapshot().unwrap().spool_files, 0);
    scan_canaries(&fixture.root.0);
}

fn scan_canaries(path: &std::path::Path) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan_canaries(&path);
            continue;
        }
        let bytes = fs::read(path).unwrap();
        for canary in [
            "SYNTHETIC_PROMPT_CANARY",
            "SYNTHETIC_RESPONSE_CANARY",
            "SYNTHETIC_TERMINAL_CANARY",
            "SYNTHETIC_ERROR_CANARY",
            "SYNTHETIC_CREDENTIAL_CANARY",
        ] {
            assert!(
                !bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes())
            );
        }
    }
}
