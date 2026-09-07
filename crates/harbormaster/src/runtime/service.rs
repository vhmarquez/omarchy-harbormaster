use super::{
    ManagedRun, ProcessIdentity, RunView, RuntimeState, command, handoff, private_directory,
};
use crate::{
    manager::ManagerError,
    projects::{Preset, Project, Task},
    protocol::LocalPath,
};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

pub(crate) struct Backend {
    root: LocalPath,
    pub(super) home: PathBuf,
    pub(super) session_runtime: Option<PathBuf>,
    executable: PathBuf,
}
impl Backend {
    pub(crate) fn new(root: &Path) -> Result<Self, ManagerError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(ManagerError::UnsafePath)?;
        private_home(&home)?;
        let root = root
            .to_str()
            .ok_or(ManagerError::UnsafePath)?
            .parse()
            .map_err(|_| ManagerError::UnsafePath)?;
        Ok(Self {
            root,
            home,
            session_runtime: std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
            executable: std::env::current_exe()?,
        })
    }

    pub(crate) fn root(&self) -> LocalPath {
        self.root.clone()
    }

    pub(crate) fn preflight(&self) -> Result<(), ManagerError> {
        let session = self
            .session_runtime
            .as_ref()
            .ok_or(ManagerError::RuntimeUnavailable)?;
        private_directory(session, false)?;
        if self.root.as_str().len() + 67 >= 104
            || !self.executable.is_absolute()
            || !self.executable.is_file()
        {
            return Err(ManagerError::UnsafePath);
        }
        for path in [
            "/usr/bin/tmux",
            "/usr/bin/systemd-run",
            "/usr/bin/systemctl",
            "/usr/bin/env",
        ] {
            let info = std::fs::metadata(path)?;
            if !info.is_file()
                || info.uid() != 0
                || info.mode() & 0o022 != 0
                || info.mode() & 0o111 == 0
            {
                return Err(ManagerError::RuntimeUnavailable);
            }
        }
        Ok(())
    }

    pub(super) fn command(&self, executable: &str) -> Command {
        let mut command = Command::new(executable);
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C");
        if let Some(root) = &self.session_runtime {
            command.env("XDG_RUNTIME_DIR", root);
        }
        command
    }

    pub(crate) fn start(
        &self,
        run: &ManagedRun,
        project: Project,
        preset: Preset,
        task: Task,
        nonce: &crate::protocol::RunId,
    ) -> Result<ProcessIdentity, ManagerError> {
        handoff::create(
            run,
            project,
            preset,
            task,
            self.home
                .to_str()
                .ok_or(ManagerError::UnsafePath)?
                .parse()
                .map_err(|_| ManagerError::UnsafePath)?,
        )?;
        let socket = run.directory().join("tmux.sock");
        // A retry can encounter the already accepted transient unit. Validate
        // its observed identity below instead of treating exit status as proof.
        let _ = command::run(self.start_command(run, nonce)?);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !socket.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.server_identity(run)
    }

    fn start_command(
        &self,
        run: &ManagedRun,
        nonce: &crate::protocol::RunId,
    ) -> Result<Command, ManagerError> {
        let mut command = self.command("/usr/bin/systemd-run");
        command
            .args([
                "--user",
                "--no-ask-password",
                "--quiet",
                "--collect",
                "--service-type=exec",
                "--expand-environment=no",
            ])
            .arg(format!("--unit={}", run.unit()))
            .arg(format!(
                "--description={}",
                super::ownership::description(run, nonce)
            ))
            .args([
                "--property=KillMode=control-group",
                "--property=Restart=no",
                "--property=TimeoutStopSec=5",
                "--property=UMask=0077",
                "--property=StandardInput=null",
                "--property=StandardOutput=null",
                "--property=StandardError=null",
                "--",
                "/usr/bin/env",
                "-i",
            ])
            .arg(format!("HOME={}", self.home.display()))
            .arg(format!(
                "XDG_RUNTIME_DIR={}",
                self.session_runtime
                    .as_ref()
                    .ok_or(ManagerError::RuntimeUnavailable)?
                    .display()
            ))
            .args([
                "PATH=/usr/bin:/bin",
                "LC_ALL=C",
                "/usr/bin/tmux",
                "-D",
                "-f",
                "/dev/null",
                "-S",
            ])
            .arg(run.directory().join("tmux.sock"));
        Ok(command)
    }

    fn server_identity(&self, run: &ManagedRun) -> Result<ProcessIdentity, ManagerError> {
        let mut query = self.command("/usr/bin/systemctl");
        query
            .args([
                "--user",
                "show",
                "--no-pager",
                "--property=MainPID",
                "--property=PartOf",
                "--property=BindsTo",
                "--property=KillMode",
            ])
            .arg(run.unit());
        let text = command::run(query)?;
        let values = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect::<std::collections::BTreeMap<_, _>>();
        if values.get("PartOf") != Some(&"")
            || values.get("BindsTo") != Some(&"")
            || values.get("KillMode") != Some(&"control-group")
        {
            return Err(ManagerError::RuntimeUnavailable);
        }
        let pid = values
            .get("MainPID")
            .and_then(|pid| pid.parse().ok())
            .ok_or(ManagerError::RuntimeUnavailable)?;
        ProcessIdentity::read(pid)
    }

    pub(crate) fn start_pane(&self, run: &ManagedRun) -> Result<(), ManagerError> {
        let mut configure = self.tmux(run)?;
        configure.args(["set-option", "-g", "remain-on-exit", "on"]);
        command::run(configure)?;
        let mut command = self.tmux(run)?;
        command
            .args(["new-session", "-d", "-s", "managed", "-c"])
            .arg(run.directory())
            .arg(&self.executable)
            .arg("runtime-worker")
            .arg(run.directory());
        command::run(command)?;
        Ok(())
    }

    pub(crate) fn ensure_pane(
        &self,
        run: &ManagedRun,
        state: &super::ControlState,
    ) -> Result<(), ManagerError> {
        let server = self.owned_server(run, state)?;
        if self.pane(run, &server).is_ok() {
            return Ok(());
        }
        if state.launch_nonce.is_none() || !handoff::pending(run)? {
            return Err(ManagerError::UnknownOutcome);
        }
        // tmux's exact session name and the atomic handoff claim independently
        // reject duplicate execution, including a delayed earlier request.
        let _ = self.start_pane(run);
        self.pane(run, &server).map(|_| ())
    }

    pub(super) fn tmux(&self, run: &ManagedRun) -> Result<Command, ManagerError> {
        private_directory(&run.directory(), false)?;
        let socket = run.directory().join("tmux.sock");
        let stat = socket.symlink_metadata()?;
        if !stat.file_type().is_socket()
            || stat.uid() != rustix::process::geteuid().as_raw()
            || stat.mode() & 0o077 != 0
        {
            return Err(ManagerError::UnsafePath);
        }
        let mut command = self.command("/usr/bin/tmux");
        command.arg("-S").arg(socket);
        Ok(command)
    }

    pub(crate) fn observe(&self, run: ManagedRun) -> RunView {
        let state = match &run.server {
            None => RuntimeState::Uncertain,
            Some(server) if !server.current() => RuntimeState::Unavailable,
            Some(_) => self.pane_state(&run).unwrap_or(RuntimeState::Uncertain),
        };
        RunView {
            unit: run.unit(),
            run,
            state,
            limited_visibility: true,
        }
    }

    fn pane_state(&self, run: &ManagedRun) -> Result<RuntimeState, ManagerError> {
        let mut query = self.tmux(run)?;
        query.args([
            "display-message",
            "-p",
            "-t",
            "=managed:",
            "#{pid}:#{pane_dead}:#{pane_pid}",
        ]);
        let text = command::run(query)?;
        let fields = text.trim().split(':').collect::<Vec<_>>();
        let server = run
            .server
            .as_ref()
            .ok_or(ManagerError::RuntimeUnavailable)?;
        if fields.len() != 3
            || fields[0].parse::<u32>().ok() != Some(server.pid)
            || !server.current()
        {
            return Err(ManagerError::RuntimeUnavailable);
        }
        match fields[1] {
            "1" => Ok(RuntimeState::Exited),
            "0" => {
                ProcessIdentity::read(
                    fields[2]
                        .parse()
                        .map_err(|_| ManagerError::RuntimeUnavailable)?,
                )?;
                Ok(RuntimeState::Running)
            }
            _ => Err(ManagerError::RuntimeUnavailable),
        }
    }
}

fn private_home(home: &Path) -> Result<(), ManagerError> {
    crate::projects::paths::directory(home, false)?;
    home.to_str()
        .ok_or(ManagerError::UnsafePath)?
        .parse::<LocalPath>()
        .map_err(|_| ManagerError::UnsafePath)?;
    Ok(())
}
