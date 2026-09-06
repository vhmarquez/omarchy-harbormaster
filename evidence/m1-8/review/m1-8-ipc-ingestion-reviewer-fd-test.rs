//! Independent review reproduction, only disposable offline fixtures.
use harbormaster::ipc::PrivateSockets;
use std::fs::{self, File};
use std::os::unix::fs::DirBuilderExt;

#[test]
fn no_created_socket_is_left_after_fd_exhaustion() {
    assert_eq!(std::env::var("XDG_RUNTIME_DIR").unwrap(), "/state/runtime");
    let root=std::path::Path::new("/state/runtime/review-emfile");
    fs::DirBuilder::new().mode(0o700).create(root).unwrap();
    let old=rustix::process::getrlimit(rustix::process::Resource::Nofile);
    rustix::process::setrlimit(rustix::process::Resource::Nofile, rustix::process::Rlimit{current:Some(32), maximum:old.maximum}).unwrap();
    let mut leftovers=Vec::new();
    for free in 1..8 {
        let fixture=root.join(free.to_string());
        fs::DirBuilder::new().mode(0o700).create(&fixture).unwrap();
        let mut held=Vec::new();
        while let Ok(file)=File::open("/dev/null") {held.push(file);}
        for _ in 0..free {held.pop();}
        let result=PrivateSockets::bind(&fixture);
        let failed=result.is_err();
        drop(result); drop(held);
        for name in ["events.sock", "control.sock"] {
            if failed && fixture.join("harbormaster").join(name).exists() {
                leftovers.push((free,name.to_owned()));
            }
        }
    }
    rustix::process::setrlimit(rustix::process::Resource::Nofile,old).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert!(leftovers.is_empty(),"owned stale socket after failed bind: {leftovers:?}");
}
