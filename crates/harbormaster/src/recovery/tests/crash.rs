use super::*;
use crate::recovery::{CleanupReason, SPOOL_TTL_MS};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn crash_child() {
    let Some(path) = std::env::var_os("HB_RECOVERY_CRASH_ROOT") else {
        return;
    };
    let mut store = ArtifactStore::open(Path::new(&path)).unwrap();
    stage_synthetic(&mut store, &source(1, 3), 10);
    panic!("child did not stop at requested checkpoint");
}

#[test]
fn actual_sigkill_at_create_stage_and_rename_preserves_bounded_metadata() {
    for phase in ["created", "staged", "renamed"] {
        let fixture = Fixture::new();
        let mut child = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "recovery::tests::crash::crash_child",
                    "--nocapture",
                ])
                .env("HB_RECOVERY_CRASH_ROOT", &fixture.0)
                .env("HB_RECOVERY_CRASH_PHASE", phase)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.0.join("checkpoint").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before checkpoint"
            );
            assert!(Instant::now() < deadline, "checkpoint deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
        child.0.kill().unwrap();
        assert_eq!(child.0.wait().unwrap().signal(), Some(9));
        let mut store = fixture.open();
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.spool_files, 1);
        assert!(snapshot.spool_bytes <= crate::recovery::MAX_RECORD_BYTES as u64);
        assert!(snapshot.unknown_gap);
        fixture.scan_canaries();
        assert_eq!(
            store.read_reviewed(10, REVIEWED_SYNTHETIC).unwrap().len(),
            usize::from(phase == "renamed")
        );
        let plan = store
            .preview_cleanup(&CleanupReason::Expired, 10 + SPOOL_TTL_MS)
            .unwrap();
        assert_eq!(plan.eligible_files(), 1);
        assert_eq!(store.apply_cleanup(plan).unwrap().removed_files, 1);
        fixture.scan_canaries();
    }
}

#[test]
fn actual_full_private_filesystem_preserves_partial_and_reports_unknown_gap() {
    use std::io::Write;
    let root = verified_fault_root();
    let path = root.join(format!("recovery-full-{}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let fixture = Fixture(path);
    let mut store = fixture.open();
    let mut filler = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(fixture.0.join("bounded-filler"))
        .unwrap();
    let mut observed_full = false;
    for _ in 0..=256 {
        match filler.write(&[0; 4096]) {
            Ok(count) => assert_eq!(count, 4096),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(28));
                observed_full = true;
                break;
            }
        }
    }
    assert!(observed_full, "bounded fill must reach actual ENOSPC");
    assert!(matches!(
        store.stage_reviewed(input(&source(1, 3)), 10, REVIEWED_SYNTHETIC),
        Err(crate::recovery::RecoveryError::Io)
    ));
    let status = store.snapshot().unwrap();
    assert_eq!(status.spool_files, 1);
    assert_eq!(status.spool_bytes, 0);
    assert_eq!(status.rejected_attempts, 1);
    assert!(status.unknown_gap);
    fixture.scan_canaries();
    drop(filler);
    fs::remove_file(fixture.0.join("bounded-filler")).unwrap();
    drop(store);
    let mut reopened = fixture.open();
    assert!(read_synthetic(&reopened, 10).is_empty());
    let plan = reopened
        .preview_cleanup(&CleanupReason::Expired, 10 + SPOOL_TTL_MS)
        .unwrap();
    assert_eq!(reopened.apply_cleanup(plan).unwrap().removed_files, 1);
    stage_synthetic(&mut reopened, &source(1, 3), 10);
    fixture.scan_canaries();
}

// Filling is authorized only after proving the fixed isolated mount and its cap.
fn verified_fault_root() -> &'static Path {
    use rustix::fs::{self as rfs, Mode, OFlags};
    let root = Path::new("/fault-fs");
    let fd = rfs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .unwrap();
    assert_eq!(
        rfs::fstatfs(&fd).unwrap().f_type,
        0x0102_1994,
        "only the fixed isolated tmpfs may be filled"
    );
    let capacity = rfs::fstatvfs(&fd).unwrap();
    assert!(
        capacity
            .f_blocks
            .checked_mul(capacity.f_frsize)
            .is_some_and(|bytes| bytes > 0 && bytes <= 1024 * 1024),
        "fault filesystem must be at most 1 MiB"
    );
    root
}
