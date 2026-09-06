use harbormaster::recovery::{ArtifactStore,CleanupReason,LogReason,LogRecord,MAX_LOG_BYTES};
use std::{collections::{BTreeMap,BTreeSet},fs,io::Write,os::unix::fs::{DirBuilderExt,MetadataExt,OpenOptionsExt}};
#[test]
fn held_cleanup_preview_does_not_hide_unlinked_log_bytes() {
    let root=std::path::PathBuf::from(std::env::var_os("TMPDIR").unwrap()).join("independent-retained-logs");
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let mut store=ArtifactStore::open(&root).unwrap();
    let artifacts=root.join("harbormaster/recovery");
    let mut identities=BTreeSet::new();
    for index in 0..4 {
        let path=artifacts.join(format!("metadata.{index}.log"));
        let mut file=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).unwrap();
        file.write_all(&vec![b'0';MAX_LOG_BYTES as usize]).unwrap();
        let stat=file.metadata().unwrap();identities.insert((stat.dev(),stat.ino()));
    }
    let held=store.preview_cleanup(&CleanupReason::PrivacyDisabled,10).unwrap();
    let result=store.append_log(LogRecord{observed_ms:10,reason:LogReason::MissingHook,count:1,unknown_gap:true});
    let named=store.snapshot().unwrap().log_bytes;
    let mut detached=BTreeMap::new();
    for entry in fs::read_dir("/proc/self/fd").unwrap().flatten() {
        if let Ok(stat)=fs::metadata(entry.path()) {
            if stat.is_file() && stat.nlink()==0 && identities.contains(&(stat.dev(),stat.ino())) {
                detached.insert((stat.dev(),stat.ino()),stat.len());
            }
        }
    }
    let retained:u64=detached.values().sum();
    eprintln!("append={result:?}; named={named}; retained_unlinked={retained}; physical_log_bytes={}",named+retained);
    assert!(named+retained<=MAX_LOG_BYTES*4,"retained preview hides unlinked bytes beyond log cap");
    drop(held);
}
