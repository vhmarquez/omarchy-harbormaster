use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub struct Fixture {
    root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "harbormaster-cli-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).expect("create private CLI fixture");
        for directory in [
            "home", "config", "state", "data", "cache", "runtime", "work",
        ] {
            fs::create_dir(root.join(directory)).expect("create fixture directory");
        }
        for file in [
            "config/harbormaster/config.toml",
            "state/harbormaster/state.db",
            "home/.config/harbormaster/config.toml",
            "home/.hermes/profiles/synthetic/config.yaml",
            "home/.claude/settings.json",
            "home/.codex/config.toml",
            "work/sentinel.txt",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().expect("fixture parent"))
                .expect("create synthetic profile parent");
            fs::write(path, b"synthetic fixture; not valid configuration\n")
                .expect("write synthetic sentinel");
        }
        Self { root }
    }

    pub fn run(&self, args: &[impl AsRef<OsStr>]) -> Output {
        self.run_with_output(args, Stdio::piped(), Stdio::piped())
    }

    pub fn run_with_output(
        &self,
        args: &[impl AsRef<OsStr>],
        stdout: Stdio,
        stderr: Stdio,
    ) -> Output {
        let before = snapshot(&self.root);
        let output = Command::new(env!("CARGO_BIN_EXE_harbormaster"))
            .args(args)
            .stdout(stdout)
            .stderr(stderr)
            .env_clear()
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("TMPDIR", self.root.join("runtime"))
            .current_dir(self.root.join("work"))
            .output()
            .expect("run the compiled CLI");
        assert_eq!(snapshot(&self.root), before, "CLI changed private fixtures");
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SnapshotEntry {
    path: PathBuf,
    contents: Option<Vec<u8>>,
    permissions: fs::Permissions,
    modified: std::time::SystemTime,
}

fn snapshot(root: &Path) -> Vec<SnapshotEntry> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).expect("read private fixture") {
        let path = entry.expect("fixture entry").path();
        let metadata = fs::symlink_metadata(&path).expect("fixture metadata");
        let contents = if metadata.is_dir() {
            entries.extend(snapshot(&path));
            None
        } else {
            assert!(
                metadata.is_file(),
                "fixture contains a symlink or special file"
            );
            Some(fs::read(&path).expect("read sentinel"))
        };
        entries.push(SnapshotEntry {
            path,
            contents,
            permissions: metadata.permissions(),
            modified: metadata.modified().expect("fixture modification time"),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}
