//! Explicit user control, kept separate from producer events and launch policy.
use super::{ManagerError, RunAction, operations::Operations};
use crate::{
    protocol::RunId,
    runtime::{ControlState, ManagedRun, RunnerRequest, RunnerResponse, TerminalIntent},
};

impl Operations {
    pub(super) fn run_action(
        &self,
        id: &RunId,
        action: RunAction,
    ) -> Result<serde_json::Value, ManagerError> {
        let RunnerResponse::Found(Some(run)) =
            self.runner(RunnerRequest::Get { id: id.clone() })?
        else {
            return Err(ManagerError::InvalidRequest);
        };
        let mut state = self.control_state(&run)?;
        match action {
            RunAction::Resume => Err(ManagerError::ActionUnavailable),
            RunAction::Actions => Ok(self.actions(&run, &state)),
            RunAction::Open => {
                if state.stopped || state.ending {
                    return Err(ManagerError::ActionUnavailable);
                }
                let window = self.runtime.focus(&run, &state)?;
                Ok(serde_json::json!({"action":"focused","address":window.address}))
            }
            RunAction::Attach => self.attach(&run, &mut state),
            RunAction::End => {
                if !state.stopped {
                    if state.pane.is_none() {
                        self.runtime.reconcile(&run, &mut state)?;
                    }
                    state.ending = true;
                    self.save_control(&state)?;
                    self.runtime.end(&run, &state)?;
                    state.stopped = true;
                    self.save_control(&state)?;
                }
                Ok(serde_json::json!({"runtime":"stopped"}))
            }
        }
    }

    fn actions(&self, run: &ManagedRun, state: &ControlState) -> serde_json::Value {
        let mut observed = state.clone();
        let owned = !state.stopped && self.runtime.reconcile(run, &mut observed).is_ok();
        let associated = owned && self.runtime.associate(run, state).is_ok();
        let end = !state.stopped && self.runtime.can_end(run, &observed);
        serde_json::json!({"open":associated && !state.ending,
            "attach":owned && !state.ending && self.runtime.desktop_available(),
            "end":end, "resume":false, "limited_visibility":true,
            "terminal_unit":state.terminal.as_ref().map(TerminalIntent::unit)})
    }

    fn attach(
        &self,
        run: &ManagedRun,
        state: &mut ControlState,
    ) -> Result<serde_json::Value, ManagerError> {
        if state.stopped || state.ending || !self.runtime.desktop_available() {
            return Err(ManagerError::ActionUnavailable);
        }
        self.runtime.reconcile(run, state)?;
        if let Some(process) = state.terminal.as_ref().and_then(|t| t.process.as_ref()) {
            if process.gone() {
                state.terminal = None;
            } else if !process.current() {
                return Err(ManagerError::OwnershipUnverified);
            }
        }
        let already_attached = state.terminal.as_ref().is_some_and(|t| t.window.is_some());
        if state.terminal.is_none() {
            let nonce = crate::generation::fresh().map_err(|_| ManagerError::Unavailable)?;
            state.terminal = Some(TerminalIntent {
                nonce: nonce
                    .as_str()
                    .parse()
                    .map_err(|_| ManagerError::InvalidRequest)?,
                process: None,
                window: None,
            });
        }
        self.save_control(state)?;
        let process = self.runtime.start_terminal(run, state)?;
        state
            .terminal
            .as_mut()
            .ok_or(ManagerError::InvalidRequest)?
            .process = Some(process);
        self.save_control(state)?;
        let window = self.runtime.wait_association(run, state)?;
        state
            .terminal
            .as_mut()
            .ok_or(ManagerError::InvalidRequest)?
            .window = Some(window);
        self.save_control(state)?;
        let terminal = state
            .terminal
            .as_ref()
            .ok_or(ManagerError::InvalidRequest)?;
        Ok(
            serde_json::json!({"action":if already_attached {"already_attached"} else {"attached"},
            "terminal_unit":terminal.unit(), "terminal":terminal}),
        )
    }
}
