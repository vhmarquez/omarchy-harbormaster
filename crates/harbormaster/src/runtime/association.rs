//! Exact foot/compositor/tmux-client association and verified focus readback.
use super::{
    Backend, ControlState, ManagedRun, ProcessIdentity, WindowIdentity, command, control,
    desktop::{Desktop, Window},
    process::read_bounded,
};
use crate::manager::ManagerError;
use std::time::{Duration, Instant};

impl Backend {
    pub(crate) fn associate(
        &self,
        run: &ManagedRun,
        state: &ControlState,
    ) -> Result<WindowIdentity, ManagerError> {
        let desktop = Desktop::new(self)?;
        let server = self.owned_server(run, state)?;
        let (pane, _held) = self.owned_pane(run, state, &server)?;
        let intent = state
            .terminal
            .as_ref()
            .ok_or(ManagerError::ActionUnavailable)?;
        let process = intent
            .process
            .as_ref()
            .ok_or(ManagerError::OwnershipUnverified)?;
        if self.terminal_process(run, intent)? != *process {
            return Err(ManagerError::OwnershipUnverified);
        }
        let _foot = process.pin()?;
        let windows = desktop.windows(self)?;
        let found = windows
            .iter()
            .filter(|w| w.pid == process.pid && w.app_id == intent.app_id())
            .collect::<Vec<_>>();
        if found.len() != 1 || !control::address(&found[0].address) {
            return Err(ManagerError::OwnershipUnverified);
        }
        let (client, tty) = self.client(run, process, &pane.session)?;
        let association = WindowIdentity {
            address: found[0].address.clone(),
            compositor: desktop.compositor,
            client,
            tty,
        };
        if intent
            .window
            .as_ref()
            .is_some_and(|saved| *saved != association)
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        Ok(association)
    }

    pub(crate) fn wait_association(
        &self,
        run: &ManagedRun,
        state: &ControlState,
    ) -> Result<WindowIdentity, ManagerError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match self.associate(run, state) {
                Ok(window) => return Ok(window),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn client(
        &self,
        run: &ManagedRun,
        foot: &ProcessIdentity,
        session: &str,
    ) -> Result<(ProcessIdentity, String), ManagerError> {
        let mut query = self.tmux(run)?;
        query.args([
            "list-clients",
            "-F",
            "#{client_pid}:#{session_id}:#{client_tty}",
        ]);
        let mut found = Vec::new();
        for line in command::run(query)?.lines() {
            let fields = line.split(':').collect::<Vec<_>>();
            if fields.len() != 3 || fields[1] != session {
                continue;
            }
            let Ok(pid) = fields[0].parse() else {
                continue;
            };
            let Ok(client) = ProcessIdentity::read(pid) else {
                continue;
            };
            let stat = read_bounded(&format!("/proc/{pid}/stat"), 4096)?;
            let parent = stat
                .rsplit_once(')')
                .and_then(|(_, tail)| tail.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u32>().ok());
            if parent != Some(foot.pid) {
                continue;
            }
            let tty = std::fs::read_link(format!("/proc/{pid}/fd/0"))?;
            if tty.to_str() != Some(fields[2]) {
                return Err(ManagerError::OwnershipUnverified);
            }
            client.pin()?;
            found.push((client, fields[2].to_owned()));
        }
        if found.len() != 1 || !foot.current() {
            return Err(ManagerError::OwnershipUnverified);
        }
        found.pop().ok_or(ManagerError::OwnershipUnverified)
    }

    pub(crate) fn focus(
        &self,
        run: &ManagedRun,
        state: &ControlState,
    ) -> Result<WindowIdentity, ManagerError> {
        let desktop = Desktop::new(self)?;
        let expected = state
            .terminal
            .as_ref()
            .and_then(|t| t.window.as_ref())
            .ok_or(ManagerError::ActionUnavailable)?;
        let association = self.associate(run, state)?;
        if association != *expected {
            return Err(ManagerError::OwnershipUnverified);
        }
        // Address enters the fixed expression only after strict hex validation
        // and full association checks. No title, arbitrary Lua or close action.
        let mut dispatch = desktop.command(self);
        dispatch.args([
            "dispatch",
            &format!(
                "hl.dsp.focus({{window=\"address:{}\"}})",
                association.address
            ),
        ]);
        command::run(dispatch)?;
        let active = desktop.active(self)?;
        let intent = state
            .terminal
            .as_ref()
            .ok_or(ManagerError::ActionUnavailable)?;
        let process = intent
            .process
            .as_ref()
            .ok_or(ManagerError::OwnershipUnverified)?;
        if !matches_window(&active, &association, process, &intent.app_id())
            || self.associate(run, state)? != association
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        Ok(association)
    }
}

pub(super) fn matches_window(
    window: &Window,
    expected: &WindowIdentity,
    process: &ProcessIdentity,
    app_id: &str,
) -> bool {
    control::address(&window.address)
        && window.address == expected.address
        && window.pid == process.pid
        && window.app_id == app_id
}
