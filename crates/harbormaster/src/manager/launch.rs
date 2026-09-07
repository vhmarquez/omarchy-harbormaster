//! Durable launch intent and explicit shared-project acknowledgement.
use super::{
    ManagerError,
    operations::{Operations, submit},
};
use crate::{
    projects::{CatalogRequest, CatalogResponse, PreparedLaunch},
    protocol::TaskId,
    runtime::{
        ControlRequest, ControlResponse, ControlState, ManagedRun, RunnerRequest, RunnerResponse,
    },
    storage::{Request as Store, Response},
};

impl Operations {
    pub(super) fn launch(
        &self,
        task_id: &TaskId,
        allow_shared: bool,
    ) -> Result<serde_json::Value, ManagerError> {
        let existing = self.runner(RunnerRequest::ByTask {
            task_id: task_id.clone(),
        })?;
        if let RunnerResponse::Found(Some(run)) = existing {
            return self.retry(run);
        }
        let (project, _, _) = self.launch_records(task_id)?;
        if !allow_shared
            && matches!(
                self.control(ControlRequest::OtherWriters {
                    project_id: project.id,
                    task_id: task_id.clone(),
                })?,
                ControlResponse::OtherWriters(true)
            )
        {
            return Err(ManagerError::ConcurrentWriter);
        }
        self.runtime.preflight()?;
        let RunnerResponse::Reserved { run, .. } = self.runner(RunnerRequest::Reserve {
            task_id: task_id.clone(),
            runtime_root: self.runtime.root(),
        })?
        else {
            return Err(ManagerError::PersistenceUnavailable);
        };
        self.retry(run)
    }

    fn retry(&self, mut run: ManagedRun) -> Result<serde_json::Value, ManagerError> {
        let mut state = self.control_state(&run)?;
        if state.stopped || state.ending || run.server.as_ref().is_some_and(|p| !p.current()) {
            return serde_json::to_value(self.runtime.observe(run))
                .map_err(|_| ManagerError::InvalidRequest);
        }
        if run.server.is_none() {
            if self.runtime.identify(&run, &mut state).is_err() {
                let nonce = state
                    .launch_nonce
                    .as_ref()
                    .ok_or(ManagerError::UnknownOutcome)?;
                let (project, preset, task) = self.launch_records(&run.task_id)?;
                self.runtime.preflight()?;
                self.runtime
                    .start(&run, project, preset, task, nonce)
                    .map_err(|_| ManagerError::UnknownOutcome)?;
            }
            let server = self.runtime.identify(&run, &mut state)?;
            let RunnerResponse::Identified(identified) = self.runner(RunnerRequest::Identify {
                id: run.id.clone(),
                server,
            })?
            else {
                return Err(ManagerError::PersistenceUnavailable);
            };
            run = identified;
            self.save_control(&state)?;
        }
        self.runtime.ensure_pane(&run, &state)?;
        // A fast exit is a truthful observation, not permission to respawn it.
        let _ = self.runtime.reconcile(&run, &mut state);
        self.save_control(&state)?;
        serde_json::to_value(self.runtime.observe(run)).map_err(|_| ManagerError::InvalidRequest)
    }

    fn launch_records(
        &self,
        task_id: &TaskId,
    ) -> Result<
        (
            crate::projects::Project,
            crate::projects::Preset,
            crate::projects::Task,
        ),
        ManagerError,
    > {
        let response = submit(
            &self.database,
            Store::Catalog(Box::new(CatalogRequest::Task {
                id: task_id.clone(),
            })),
        )?;
        let Response::Catalog(response) = response else {
            return Err(ManagerError::PersistenceUnavailable);
        };
        let CatalogResponse::Launch {
            project,
            preset,
            task,
        } = *response
        else {
            return Err(ManagerError::PersistenceUnavailable);
        };
        PreparedLaunch::new(project.clone(), preset.clone(), task.clone())?;
        Ok((project, preset, task))
    }

    pub(super) fn control(&self, request: ControlRequest) -> Result<ControlResponse, ManagerError> {
        match submit(&self.database, Store::RuntimeControl(Box::new(request)))? {
            Response::RuntimeControl(response) => Ok(*response),
            _ => Err(ManagerError::PersistenceUnavailable),
        }
    }
    pub(super) fn control_state(&self, run: &ManagedRun) -> Result<ControlState, ManagerError> {
        match self.control(ControlRequest::Get(run.id.clone()))? {
            ControlResponse::State(state) => {
                Ok(state.map_or_else(|| ControlState::new(run.id.clone(), None), |s| *s))
            }
            _ => Err(ManagerError::PersistenceUnavailable),
        }
    }
    pub(super) fn save_control(&self, state: &ControlState) -> Result<(), ManagerError> {
        self.control(ControlRequest::Save(Box::new(state.clone())))
            .map(|_| ())
    }
}
