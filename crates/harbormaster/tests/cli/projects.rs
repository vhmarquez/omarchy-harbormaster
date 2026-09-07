use harbormaster::projects::{PreparedLaunch, Preset, Project, Task};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(std::env::var_os("TMPDIR").unwrap()).join(format!(
            "catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["state", "project with spaces", "bin", "home"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        fs::set_permissions(root.join("state"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            root.join("project with spaces/harbormaster.toml"),
            b"command = 'touch injected'\n",
        )
        .unwrap();
        fs::write(root.join("bin/hermes"), b"#!/usr/bin/python3\nimport json,os,sys\nprint(json.dumps({'argv':sys.argv[1:],'environment':dict(os.environ)}))\n").unwrap();
        fs::set_permissions(root.join("bin/hermes"), fs::Permissions::from_mode(0o755)).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_str().unwrap().to_owned()
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_harbormaster"))
            .args(args)
            .env_clear()
            .env("HOME", self.root.join("home"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("HERMES_HOME", "synthetic-untrusted-home")
            .env("OPENAI_API_KEY", "synthetic-secret-canary")
            .current_dir(self.root.join("project with spaces"))
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("synthetic-secret-canary"));
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn records(&self) -> (Project, Preset, Task) {
        let project = self.json(&[
            "project",
            "add",
            &self.path("project with spaces"),
            "Example project",
        ]);
        let preset = self.json(&[
            "preset",
            "add",
            "coding",
            &self.path("bin/hermes"),
            "synthetic",
        ]);
        let task = self.json(&[
            "task",
            "add",
            project["Project"]["id"].as_str().unwrap(),
            "coding",
            "--help $(touch injected) ' metadata",
        ]);
        (
            serde_json::from_value(project["Project"].clone()).unwrap(),
            serde_json::from_value(preset["Preset"].clone()).unwrap(),
            serde_json::from_value(task["Task"].clone()).unwrap(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn registration_lists_and_launch_preview_persist_across_cli_processes() {
    let f = Fixture::new();
    let (project, preset, task) = f.records();
    let (again, _, duplicate_task) = f.records();
    assert_eq!(again, project);
    assert_eq!(duplicate_task, task);
    assert_eq!(
        f.json(&["project", "list"])["Projects"]["items"],
        json!([project])
    );
    assert_eq!(
        f.json(&["preset", "list"])["Presets"]["items"],
        json!([preset])
    );
    assert_eq!(
        f.json(&["task", "list", project.id.as_str()])["Tasks"]["items"],
        json!([task])
    );
    let plan = f.json(&["task", "plan", task.id.as_str()]);
    assert_eq!(plan["argv"], json!(["-p", "synthetic", "chat"]));
    assert_eq!(plan["cwd"], f.path("project with spaces"));
    assert!(!plan.to_string().contains("touch injected"));
    assert!(!f.root.join("project with spaces/injected").exists());
    assert_eq!(
        fs::read(f.root.join("project with spaces/harbormaster.toml")).unwrap(),
        b"command = 'touch injected'\n"
    );
    assert_eq!(
        fs::metadata(f.root.join("state"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(f.root.join("state/harbormaster"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(f.root.join("state/harbormaster/state.db"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn invalid_commands_paths_and_changed_executable_do_not_launch_or_echo_inputs() {
    let f = Fixture::new();
    for args in [
        vec!["preset", "add", "bad", "/not-used", "--unsafe"],
        vec![
            "task",
            "add",
            "private-invalid-id",
            "coding",
            "private-label",
        ],
        vec!["project", "add", "relative-private-path", "private-label"],
    ] {
        let output = f.run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            output.stderr,
            b"harbormaster: invalid command; use --help\n"
        );
    }
    assert!(!f.root.join("state/harbormaster").exists());
    symlink(f.root.join("project with spaces"), f.root.join("link")).unwrap();
    assert_eq!(
        f.run(&["project", "add", &f.path("link"), "Example"])
            .status
            .code(),
        Some(1)
    );
    let (_, _, task) = f.records();
    fs::rename(f.root.join("bin/hermes"), f.root.join("bin/old-hermes")).unwrap();
    fs::write(f.root.join("bin/hermes"), b"#!/bin/sh\ntouch injected\n").unwrap();
    fs::set_permissions(f.root.join("bin/hermes"), fs::Permissions::from_mode(0o755)).unwrap();
    let output = f.run(&["task", "plan", task.id.as_str()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!f.root.join("project with spaces/injected").exists());
    fs::set_permissions(f.root.join("state"), fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(f.run(&["project", "list"]).status.code(), Some(1));
}

#[test]
fn prepared_argv_and_environment_keep_labels_and_credentials_out_of_the_child() {
    let f = Fixture::new();
    let (project, preset, task) = f.records();
    let plan = PreparedLaunch::new(project, preset, task).unwrap();
    let parent = BTreeMap::from([
        ("HOME".into(), f.path("home")),
        ("LANG".into(), "C.UTF-8".into()),
        ("TERM".into(), "xterm-256color".into()),
        ("PATH".into(), ".:/untrusted".into()),
        ("OPENAI_API_KEY".into(), "synthetic-secret-canary".into()),
        ("HERMES_HOME".into(), "/untrusted".into()),
        ("LD_PRELOAD".into(), "/untrusted".into()),
        ("PYTHONPATH".into(), "/untrusted".into()),
    ]);
    let output = plan.command(&parent).unwrap().output().unwrap();
    assert!(output.status.success());
    let child: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(child["argv"], json!(["-p", "synthetic", "chat"]));
    assert_eq!(child["environment"]["HOME"], f.path("home"));
    assert_eq!(child["environment"]["PATH"], "/usr/local/bin:/usr/bin:/bin");
    for key in ["OPENAI_API_KEY", "HERMES_HOME", "LD_PRELOAD", "PYTHONPATH"] {
        assert!(child["environment"].get(key).is_none());
    }
    assert!(!String::from_utf8_lossy(&output.stdout).contains("synthetic-secret-canary"));
}
