use super::projects::Fixture;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Manager<'a> {
    fixture: &'a Fixture,
    child: Child,
}
impl<'a> Manager<'a> {
    fn start(fixture: &'a Fixture) -> Self {
        let root = fixture.root.join("runtime");
        fs::create_dir_all(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let child = command(fixture)
            .args(["manager", "serve"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut manager = Self { fixture, child };
        let deadline = Instant::now() + Duration::from_secs(3);
        while !root.join("harbormaster/control.sock").exists() {
            assert!(
                manager.child.try_wait().unwrap().is_none(),
                "manager exited during startup"
            );
            assert!(Instant::now() < deadline, "manager startup timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
        manager
    }
    fn json(&self, args: &[&str]) -> Value {
        json_output(self.fixture, args)
    }
    fn stopped(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "manager stop timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !self
                .fixture
                .root
                .join("runtime/harbormaster/control.sock")
                .exists()
        );
    }
}
impl Drop for Manager<'_> {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn command(f: &Fixture) -> Command {
    let mut command = f.command();
    command.env("XDG_RUNTIME_DIR", f.root.join("runtime"));
    command
}
fn json_output(f: &Fixture, args: &[&str]) -> Value {
    let output = command(f).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("synthetic-secret-canary"));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn manager_routes_catalog_and_restarts_cleanly_with_bounded_control() {
    let f = Fixture::new();
    let (project, _, task) = f.records();
    let mut manager = Manager::start(&f);
    assert_eq!(
        manager.json(&["project", "list"])["Projects"]["items"][0]["id"],
        project.id.as_str()
    );
    assert_eq!(
        manager.json(&["task", "plan", task.id.as_str()])["argv"],
        json!(["-p", "synthetic", "chat"])
    );
    assert_control_boundaries(&f, &manager);
    manager.json(&["manager", "stop"]);
    manager.stopped();
    drop(manager);
    let mut restarted = Manager::start(&f);
    let response = restarted.json(&[
        "task",
        "add",
        project.id.as_str(),
        "coding",
        "Created through IPC",
    ]);
    assert_eq!(response["Task"]["label"], "Created through IPC");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(restarted.child.id()).unwrap()),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    restarted.stopped();
    drop(restarted);
    let mut again = Manager::start(&f);
    assert_eq!(
        again.json(&["task", "list", project.id.as_str()])["Tasks"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    again.json(&["manager", "stop"]);
    again.stopped();
}

#[test]
fn runtime_worker_executes_pinned_metadata_with_a_private_environment() {
    let f = Fixture::new();
    let (project, preset, task) = f.records();
    let directory = f.root.join("handoff");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let handoff = json!({"project":project,"preset":preset,"task":task,"home":f.path("home")});
    fs::write(
        directory.join("launch.json"),
        serde_json::to_vec(&handoff).unwrap(),
    )
    .unwrap();
    fs::set_permissions(
        directory.join("launch.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let output = f
        .command()
        .args(["runtime-worker", directory.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let child: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(child["argv"], json!(["-p", "synthetic", "chat"]));
    assert_eq!(child["environment"]["HOME"], f.path("home"));
    for key in ["OPENAI_API_KEY", "HERMES_HOME", "TMUX", "PYTHONPATH"] {
        assert!(child["environment"].get(key).is_none());
    }
    assert!(!directory.join("launch.json").exists());
}

fn assert_control_boundaries(f: &Fixture, manager: &Manager<'_>) {
    let request = json!({"protocol":0,"channel":"control","request_id":"12345678-0000-4000-8000-000000000000","expected_revision":"0","command":{"operation":"catalog","request":{"operation":"add_project","root":f.path("project with spaces"),"label":"unapproved replacement"}}});
    let mut stream = UnixStream::connect(f.root.join("runtime/harbormaster/control.sock")).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    writeln!(stream, "{request}").unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&reply).unwrap()["error"],
        "conflict"
    );
    let mut event = UnixStream::connect(f.root.join("runtime/harbormaster/events.sock")).unwrap();
    let _ = writeln!(event, "{request}");
    assert_eq!(
        manager.json(&["project", "list"])["Projects"]["items"][0]["label"],
        "Example project"
    );
}
