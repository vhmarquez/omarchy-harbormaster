//! Revalidate a recorded runtime before any attachment or destructive operation.
use super::{
    Backend, ControlState, ManagedRun, PaneIdentity, ProcessIdentity, command, control,
    process::{HeldProcess, read_bounded},
};
use crate::manager::ManagerError;
use std::{collections::BTreeMap, os::unix::fs::MetadataExt};

pub(super) struct Server {
    pub identity: ProcessIdentity,
    pub invocation: String,
    pub held: HeldProcess,
}
pub(super) struct Pane {
    pub identity: PaneIdentity,
    pub dead: bool,
}

impl Backend {
    pub(super) fn properties(&self, unit: &str) -> Result<BTreeMap<String, String>, ManagerError> {
        let mut query = self.command("/usr/bin/systemctl");
        query.args(["--user", "show", "--no-pager", "--property=MainPID,InvocationID,ControlGroup,PartOf,BindsTo,KillMode,Description,ActiveState"])
            .arg(unit);
        let text = command::run(query)?;
        let mut values = BTreeMap::new();
        for line in text.lines() {
            let (key, value) = line
                .split_once('=')
                .ok_or(ManagerError::OwnershipUnverified)?;
            if values.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err(ManagerError::OwnershipUnverified);
            }
        }
        Ok(values)
    }

    pub(super) fn owned_server(
        &self,
        run: &ManagedRun,
        state: &ControlState,
    ) -> Result<Server, ManagerError> {
        let values = self.properties(&run.unit())?;
        if !property(&values, "PartOf")?.is_empty()
            || !property(&values, "BindsTo")?.is_empty()
            || property(&values, "KillMode")? != "control-group"
            || property(&values, "ActiveState")? != "active"
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        let invocation = property(&values, "InvocationID")?.to_owned();
        if !control::hex(&invocation, 32)
            || state.invocation.as_ref().is_some_and(|v| *v != invocation)
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        if let Some(nonce) = &state.launch_nonce {
            if property(&values, "Description")? != description(run, nonce) {
                return Err(ManagerError::OwnershipUnverified);
            }
        } else if run.server.is_none() {
            return Err(ManagerError::OwnershipUnverified);
        }
        let identity = ProcessIdentity::read(
            property(&values, "MainPID")?
                .parse()
                .map_err(|_| ManagerError::OwnershipUnverified)?,
        )?;
        if run.server.as_ref().is_some_and(|saved| *saved != identity)
            || cgroup(identity.pid)? != property(&values, "ControlGroup")?
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        executable(identity.pid, "/usr/bin/tmux")?;
        let held = identity.pin()?;
        Ok(Server {
            identity,
            invocation,
            held,
        })
    }

    pub(super) fn pane(&self, run: &ManagedRun, server: &Server) -> Result<Pane, ManagerError> {
        let mut query = self.tmux(run)?;
        query.args([
            "list-panes",
            "-a",
            "-F",
            "#{pid}:#{session_name}:#{session_id}:#{pane_id}:#{pane_dead}:#{pane_pid}",
        ]);
        let text = command::run(query)?;
        let rows = text.lines().collect::<Vec<_>>();
        if rows.len() != 1 {
            return Err(ManagerError::OwnershipUnverified);
        }
        let fields = rows[0].split(':').collect::<Vec<_>>();
        if fields.len() != 6
            || fields[0].parse::<u32>().ok() != Some(server.identity.pid)
            || fields[1] != "managed"
            || !control::target(fields[2], '$')
            || !control::target(fields[3], '%')
            || !matches!(fields[4], "0" | "1")
            || !server.identity.current()
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        let pid = fields[5]
            .parse()
            .map_err(|_| ManagerError::OwnershipUnverified)?;
        // An exited pane has no current process identity. Its saved identity is
        // checked separately before completing an already requested shutdown.
        let process = if fields[4] == "0" {
            ProcessIdentity::read(pid)?
        } else {
            ProcessIdentity {
                pid,
                start_ticks: 0,
                boot_id: String::new(),
            }
        };
        Ok(Pane {
            identity: PaneIdentity {
                process,
                pane: fields[3].to_owned(),
                session: fields[2].to_owned(),
            },
            dead: fields[4] == "1",
        })
    }

    pub(super) fn owned_pane(
        &self,
        run: &ManagedRun,
        state: &ControlState,
        server: &Server,
    ) -> Result<(PaneIdentity, HeldProcess), ManagerError> {
        let pane = self.pane(run, server)?;
        if pane.dead
            || state
                .pane
                .as_ref()
                .is_some_and(|saved| *saved != pane.identity)
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        let group = cgroup(pane.identity.process.pid)?;
        let scope = group
            .rsplit('/')
            .next()
            .ok_or(ManagerError::OwnershipUnverified)?;
        let nonce = scope
            .strip_prefix("tmux-spawn-")
            .and_then(|s| s.strip_suffix(".scope"))
            .ok_or(ManagerError::OwnershipUnverified)?;
        nonce
            .parse::<crate::protocol::RunId>()
            .map_err(|_| ManagerError::OwnershipUnverified)?;
        let values = self.properties(scope)?;
        if property(&values, "PartOf")? != run.unit() || property(&values, "ControlGroup")? != group
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        let held = pane.identity.process.pin()?;
        Ok((pane.identity, held))
    }

    pub(crate) fn reconcile(
        &self,
        run: &ManagedRun,
        state: &mut ControlState,
    ) -> Result<ProcessIdentity, ManagerError> {
        let server = self.owned_server(run, state)?;
        let (pane, _held) = self.owned_pane(run, state, &server)?;
        state.invocation = Some(server.invocation);
        state.pane = Some(pane);
        Ok(server.identity)
    }

    pub(crate) fn identify(
        &self,
        run: &ManagedRun,
        state: &mut ControlState,
    ) -> Result<ProcessIdentity, ManagerError> {
        let server = self.owned_server(run, state)?;
        state.invocation = Some(server.invocation);
        Ok(server.identity)
    }

    pub(crate) fn end(&self, run: &ManagedRun, state: &ControlState) -> Result<(), ManagerError> {
        if state.ending
            && run.server.as_ref().is_some_and(|p| !p.current())
            && state.pane.as_ref().is_some_and(|p| !p.process.current())
            && self.properties(&run.unit()).is_ok_and(|p| {
                p.get("ActiveState")
                    .is_some_and(|s| s == "inactive" || s == "failed")
            })
        {
            return Ok(());
        }
        let server = self.owned_server(run, state)?;
        let saved = state
            .pane
            .as_ref()
            .ok_or(ManagerError::OwnershipUnverified)?;
        let current = self.pane(run, &server)?;
        if !current.dead {
            let (_, pane) = self.owned_pane(run, state, &server)?;
            pane.terminate()?;
            if !pane.exited(3)? {
                return Err(ManagerError::UnknownOutcome);
            }
        }
        let after = self.pane(run, &server)?;
        if !after.dead
            || after.identity.pane != saved.pane
            || after.identity.session != saved.session
            || after.identity.process.pid != saved.process.pid
        {
            return Err(ManagerError::OwnershipUnverified);
        }
        self.owned_server(run, state)?;
        server.held.terminate()?;
        if !server.held.exited(3)? {
            return Err(ManagerError::UnknownOutcome);
        }
        Ok(())
    }
}

pub(super) fn description(run: &ManagedRun, nonce: &crate::protocol::RunId) -> String {
    format!(
        "Harbormaster runtime {} {}",
        run.id.as_str(),
        nonce.as_str()
    )
}
pub(super) fn property<'a>(
    values: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, ManagerError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or(ManagerError::OwnershipUnverified)
}
pub(super) fn cgroup(pid: u32) -> Result<String, ManagerError> {
    let value = read_bounded(&format!("/proc/{pid}/cgroup"), 2048)?;
    let group = value
        .trim()
        .strip_prefix("0::/")
        .ok_or(ManagerError::OwnershipUnverified)?;
    if group.contains(['\n', '\r']) {
        return Err(ManagerError::OwnershipUnverified);
    }
    Ok(format!("/{group}"))
}
pub(super) fn executable(pid: u32, path: &str) -> Result<(), ManagerError> {
    let actual = std::fs::metadata(format!("/proc/{pid}/exe"))?;
    let expected = std::fs::metadata(path)?;
    if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
        return Err(ManagerError::OwnershipUnverified);
    }
    Ok(())
}
