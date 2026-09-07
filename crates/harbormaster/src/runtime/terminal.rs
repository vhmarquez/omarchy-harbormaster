//! Independent standalone foot activation; fixed argv and private tmux endpoint.
use super::{
    Backend, ControlState, ManagedRun, ProcessIdentity, TerminalIntent, command,
    desktop::Desktop,
    ownership::{self, property},
};
use crate::manager::ManagerError;

impl Backend {
    pub(crate) fn desktop_available(&self) -> bool {
        Desktop::new(self).is_ok()
    }

    pub(crate) fn terminal_process(
        &self,
        run: &ManagedRun,
        intent: &TerminalIntent,
    ) -> Result<ProcessIdentity, ManagerError> {
        let values = self.properties(&intent.unit())?;
        if property(&values, "Description")? != terminal_description(run, intent)
            || !property(&values, "PartOf")?.is_empty()
            || !property(&values, "BindsTo")?.is_empty()
            || property(&values, "KillMode")? != "control-group"
            || property(&values, "ActiveState")? != "active"
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        let process = ProcessIdentity::read(
            property(&values, "MainPID")?
                .parse()
                .map_err(|_| ManagerError::OwnershipUnverified)?,
        )?;
        if intent.process.as_ref().is_some_and(|p| *p != process)
            || ownership::cgroup(process.pid)? != property(&values, "ControlGroup")?
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        ownership::executable(process.pid, "/usr/bin/foot")?;
        process.pin()?;
        Ok(process)
    }

    pub(crate) fn start_terminal(
        &self,
        run: &ManagedRun,
        state: &ControlState,
    ) -> Result<ProcessIdentity, ManagerError> {
        let desktop = Desktop::new(self)?;
        let server = self.owned_server(run, state)?;
        let (_pane, _held) = self.owned_pane(run, state, &server)?;
        let intent = state
            .terminal
            .as_ref()
            .ok_or(ManagerError::InvalidRequest)?;
        if let Ok(process) = self.terminal_process(run, intent) {
            return Ok(process);
        }
        if intent.process.is_some() {
            return Err(ManagerError::OwnershipUnverified);
        }
        let _ = command::run(self.terminal_command(run, intent, &desktop)?);
        self.terminal_process(run, intent)
    }
    fn terminal_command(
        &self,
        run: &ManagedRun,
        intent: &TerminalIntent,
        desktop: &Desktop,
    ) -> Result<std::process::Command, ManagerError> {
        let mut command = self.command("/usr/bin/systemd-run");
        command
            .args([
                "--user",
                "--quiet",
                "--collect",
                "--no-ask-password",
                "--service-type=exec",
                "--expand-environment=no",
            ])
            .arg(format!("--unit={}", intent.unit()))
            .arg(format!(
                "--description={}",
                terminal_description(run, intent)
            ))
            .args([
                "--property=KillMode=control-group",
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
                    .ok_or(ManagerError::ActionUnavailable)?
                    .display()
            ))
            .arg(format!("WAYLAND_DISPLAY={}", desktop.display))
            .args([
                "PATH=/usr/bin:/bin",
                "LC_ALL=C",
                "/usr/bin/foot",
                "--config=/dev/null",
                "--log-level=none",
                "--log-no-syslog",
            ])
            .arg(format!("--app-id={}", intent.app_id()))
            .arg("--title=Harbormaster")
            .args(["/usr/bin/tmux", "-S"])
            .arg(run.directory().join("tmux.sock"))
            .args(["attach-session", "-t", "=managed"]);
        Ok(command)
    }
}

fn terminal_description(run: &ManagedRun, intent: &TerminalIntent) -> String {
    format!(
        "Harbormaster terminal {} {}",
        run.id.as_str(),
        intent.nonce.as_str()
    )
}
