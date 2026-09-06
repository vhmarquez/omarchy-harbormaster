use harbormaster::recovery::{ArtifactStore, RecoveryError};
use std::{fs, os::unix::fs::{DirBuilderExt, MetadataExt}, path::Path, process::{Command, Stdio}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

fn descriptors(path: &Path, pid: &str) -> Vec<String> {
    let expected = fs::metadata(path).unwrap();
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else { return Vec::new() };
    entries.filter_map(|entry| {
        let entry = entry.ok()?;
        let actual = fs::metadata(entry.path()).ok()?;
        ((actual.dev(), actual.ino()) == (expected.dev(), expected.ino()))
            .then(|| entry.file_name().to_string_lossy().into_owned())
    }).collect()
}

#[test]
fn real_artifact_lock_can_temporarily_outlive_local_owner_during_spawn() {
    let root = std::env::temp_dir().join("independent-spawn-ownership");
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let lock = root.join("harbormaster/recovery/recovery.lock");
    let store = ArtifactStore::open(&root).unwrap();
    let original = descriptors(&lock, "self");
    assert_eq!(original.len(), 1);
    let info = fs::read_to_string(format!("/proc/self/fdinfo/{}", original[0])).unwrap();
    let flags = info.lines().find_map(|line| line.strip_prefix("flags:\t")).unwrap();
    assert_ne!(u32::from_str_radix(flags.trim(), 8).unwrap() & rustix::fs::OFlags::CLOEXEC.bits(), 0);
    drop(store);
    let stop = Arc::new(AtomicBool::new(false));
    let threads: Vec<_> = (0..4).map(|_| {
        let stop = stop.clone();
        std::thread::spawn(move || {
            for _ in 0..5000 {
                if stop.load(Ordering::Relaxed) { break; }
                assert!(Command::new("/usr/bin/true").stdout(Stdio::null()).spawn().unwrap().wait().unwrap().success());
            }
        })
    }).collect();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut transients = 0;
    let mut child_witnesses = Vec::new();
    while Instant::now() < deadline && child_witnesses.is_empty() {
        assert!(descriptors(&lock, "self").is_empty(), "local owner leaked");
        match ArtifactStore::open(&root) {
            Ok(store) => {
                std::thread::sleep(Duration::from_micros(100));
                drop(store);
            }
            Err(RecoveryError::AlreadyOwned) => {
                transients += 1;
                assert!(descriptors(&lock, "self").is_empty(), "local owner leaked after refusal");
                for process in fs::read_dir("/proc").unwrap().flatten() {
                    let pid = process.file_name().to_string_lossy().into_owned();
                    if pid.parse::<u32>().is_ok_and(|id| id != std::process::id()) {
                        let held = descriptors(&lock, &pid);
                        if !held.is_empty() { child_witnesses.push((pid, held)); }
                    }
                }
            }
            Err(other) => panic!("unexpected store failure: {other:?}"),
        }
    }
    stop.store(true, Ordering::Relaxed);
    for thread in threads { thread.join().unwrap(); }
    assert!(descriptors(&lock, "self").is_empty());
    let reopened = ArtifactStore::open(&root).unwrap();
    eprintln!("verified_cloexec=true; transient_refusals={transients}; observed_inherited_child_descriptors={child_witnesses:?}; reopen_after_children=true");
    assert!(transients > 0, "bounded stress did not reproduce a transient");
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
