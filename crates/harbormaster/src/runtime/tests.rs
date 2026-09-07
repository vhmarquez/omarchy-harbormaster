use super::{ProcessIdentity, association::matches_window, control, desktop::Window};
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct ChildGuard(Child);
impl ChildGuard {
    fn sleep() -> Self {
        Self(
            Command::new("/usr/bin/sleep")
                .arg("60")
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn held_identity_rejects_stale_boot_start_and_zombies_and_preserves_neighbor() {
    let target = ChildGuard::sleep();
    let mut neighbor = ChildGuard::sleep();
    let identity = ProcessIdentity::read(target.0.id()).unwrap();
    let mut wrong = identity.clone();
    wrong.start_ticks += 1;
    assert!(wrong.pin().is_err());
    wrong = identity.clone();
    wrong.boot_id = "00000000-0000-4000-8000-000000000000".into();
    assert!(wrong.pin().is_err());
    let held = identity.pin().unwrap();
    held.terminate().unwrap();
    assert!(held.exited(2).unwrap());
    // Keep the exited child unreaped: a zombie is never a controllable target.
    assert!(identity.pin().is_err());
    assert!(held.terminate().is_err());
    assert!(neighbor.0.try_wait().unwrap().is_none());
    let mut zombie = ChildGuard(Command::new("/usr/bin/true").env_clear().spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    while ProcessIdentity::read(zombie.0.id()).is_ok() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(zombie.0.wait().unwrap().success());
}

#[test]
fn focus_binding_and_persisted_metadata_reject_wrong_targets_and_expressions() {
    let process = ProcessIdentity::read(std::process::id()).unwrap();
    let expected = super::WindowIdentity {
        address: "0x1234".into(),
        compositor: "instance_1".into(),
        client: process.clone(),
        tty: "/dev/pts/1".into(),
    };
    let mut window = Window {
        address: expected.address.clone(),
        pid: process.pid,
        app_id: "harbormaster.test".into(),
    };
    assert!(matches_window(
        &window,
        &expected,
        &process,
        "harbormaster.test"
    ));
    window.pid += 1;
    assert!(!matches_window(
        &window,
        &expected,
        &process,
        "harbormaster.test"
    ));
    window.pid = process.pid;
    window.app_id = "unrelated".into();
    assert!(!matches_window(
        &window,
        &expected,
        &process,
        "harbormaster.test"
    ));
    for address in [
        "",
        "0x",
        "activewindow",
        "0x123\"}); hl.dsp.exit()",
        "0x123\n",
    ] {
        assert!(!control::address(address));
    }
    assert!(!control::component("../other-instance"));
    assert!(!control::target("$0\n", '$'));
}

#[test]
fn finished_pane_requires_saved_exact_session_pane_and_pid() {
    use super::ownership::{Pane, matching_dead_pane};
    let saved = super::PaneIdentity {
        process: ProcessIdentity::read(std::process::id()).unwrap(),
        pane: "%0".into(),
        session: "$0".into(),
    };
    let mut pane = Pane {
        identity: saved.clone(),
        dead: true,
    };
    // tmux retains the PID but cannot supply a dead worker's boot/start identity.
    pane.identity.process.start_ticks = 0;
    pane.identity.process.boot_id.clear();
    assert!(matching_dead_pane(&pane, &saved).is_ok());
    pane.dead = false;
    assert!(matching_dead_pane(&pane, &saved).is_err());
    pane.dead = true;
    let mut wrong = saved.clone();
    wrong.pane = "%1".into();
    assert!(matching_dead_pane(&pane, &wrong).is_err());
    wrong = saved.clone();
    wrong.session = "$1".into();
    assert!(matching_dead_pane(&pane, &wrong).is_err());
    wrong = saved;
    wrong.process.pid += 1;
    assert!(matching_dead_pane(&pane, &wrong).is_err());
}
