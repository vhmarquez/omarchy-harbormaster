//! Mandatory native-only fixture; Docker must never execute nested namespaces.
use harbormaster::ipc::{Channel, ConnectionBudget, IpcError, PrivateSockets};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const FIXTURE_ENV: &str = "HB_NATIVE_PEER_FIXTURE";
const TEST_NAME: &str = "peer_without_visible_pid_is_rejected";

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn child_server(path: &Path) {
    let sockets = PrivateSockets::bind(path).unwrap();
    let budget = ConnectionBudget::new(1).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match sockets.accept(Channel::Event, &budget, deadline) {
            Err(IpcError::PermissionDenied) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(2)),
            Ok(Some(_)) => panic!("accepted a peer whose PID is invisible in this namespace"),
            _ => panic!("native peer fixture did not reach the credential rejection"),
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the one fixed sandbox argv visible beside environment and spawn bounds."
)]
fn server_process(path: &Path) -> Child {
    let executable = std::env::current_exe().unwrap();
    // These paths already belong to the outer verifier sandbox. Only this
    // fixture directory stays writable; no host home/runtime/network is mounted.
    Command::new("/usr/bin/bwrap")
        .args([
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib",
            "/lib64",
            "--symlink",
            "usr/bin",
            "/bin",
            "--ro-bind",
            "/state/target",
            "/state/target",
            "--bind",
        ])
        .arg(path)
        .arg(path)
        .args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--clearenv",
            "--setenv",
            "PATH",
            "/usr/bin",
            "--setenv",
            FIXTURE_ENV,
        ])
        .arg(path)
        .arg("--")
        .arg(executable)
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn peer_without_visible_pid_is_rejected() {
    if let Some(path) = std::env::var_os(FIXTURE_ENV) {
        child_server(Path::new(&path));
        return;
    }
    let path = PathBuf::from(std::env::var_os("TMPDIR").unwrap())
        .join(format!("hb-peer-namespace-{}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let fixture = Fixture(path);
    let mut child = ChildGuard(Some(server_process(&fixture.0)));
    let socket = fixture.0.join("harbormaster/events.sock");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket.exists() {
        assert!(
            Instant::now() < deadline,
            "native namespace readiness deadline"
        );
        assert!(
            child.0.as_mut().unwrap().try_wait().unwrap().is_none(),
            "native namespace child exited before listen"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let _client = UnixStream::connect(socket).unwrap();
    while child.0.as_mut().unwrap().try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "native namespace completion deadline"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let output = child.0.take().unwrap().wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed; 0 ignored;"));
    println!(
        "native peer credential fixture: outer same-UID client rejected by inner PID namespace server"
    );
}
