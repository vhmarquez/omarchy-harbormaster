use super::{Command, ManagerError, Reply, Request};
use crate::{
    projects::{CatalogRequest, CatalogResponse, PreparedLaunch},
    protocol::Revision,
    runtime::{Backend, RunnerRequest, RunnerResponse},
    storage::{DatabaseWorker, ReceiptError, Request as Store, Response},
};
use std::{sync::mpsc, time::Duration};

pub(super) struct Work {
    pub request: Request,
    pub reply: mpsc::SyncSender<Reply>,
}

pub(super) struct Operations {
    pub database: DatabaseWorker,
    pub runtime: Backend,
}

impl Operations {
    pub(super) fn run(self, receiver: mpsc::Receiver<Work>) {
        for work in receiver {
            let reply = match self.execute(&work.request) {
                Ok(value) => self.revision().map(|revision| Reply::Ok {
                    protocol: 0,
                    request_id: work.request.request_id.clone(),
                    revision,
                    value,
                }),
                Err(error) => Err(error),
            }
            .unwrap_or_else(|error| Reply::Error {
                protocol: 0,
                request_id: work.request.request_id,
                error,
            });
            let _ = work.reply.try_send(reply);
        }
    }

    fn revision(&self) -> Result<Revision, ManagerError> {
        match submit(&self.database, Store::Status)? {
            Response::Status(status) => Ok(status.revision),
            _ => Err(ManagerError::PersistenceUnavailable),
        }
    }

    fn execute(&self, request: &Request) -> Result<serde_json::Value, ManagerError> {
        if let Some(expected) = request.expected_revision
            && self.revision()? != expected
        {
            return Err(ManagerError::Conflict);
        }
        match &request.command {
            Command::Status => Ok(serde_json::json!({"manager":"running"})),
            Command::Stop => Ok(serde_json::json!({"manager":"stopping"})),
            Command::Catalog { request } => catalog(&self.database, request.clone()),
            Command::Launch {
                task_id,
                allow_shared_checkout,
                ..
            } => self.launch(task_id, *allow_shared_checkout),
            Command::RunAction { run_id, action } => self.run_action(run_id, *action),
            Command::Runs { project_id, after } => {
                let response = submit(
                    &self.database,
                    Store::Runner(Box::new(RunnerRequest::List {
                        project_id: project_id.clone(),
                        after: after.clone(),
                    })),
                )?;
                let Response::Runner(response) = response else {
                    return Err(ManagerError::PersistenceUnavailable);
                };
                let RunnerResponse::Listed(page) = *response else {
                    return Err(ManagerError::PersistenceUnavailable);
                };
                let items = page
                    .items
                    .into_iter()
                    .map(|run| self.runtime.observe(run))
                    .collect::<Vec<_>>();
                Ok(
                    serde_json::json!({"items":items,"next_after":page.next_after,"revision":page.revision}),
                )
            }
        }
    }

    pub(super) fn runner(&self, request: RunnerRequest) -> Result<RunnerResponse, ManagerError> {
        match submit(&self.database, Store::Runner(Box::new(request)))? {
            Response::Runner(response) => Ok(*response),
            _ => Err(ManagerError::PersistenceUnavailable),
        }
    }
}

pub(crate) fn submit(database: &DatabaseWorker, request: Store) -> Result<Response, ManagerError> {
    let mut ticket = database
        .try_submit(request)
        .map_err(|_| ManagerError::Busy)?;
    ticket
        .wait_timeout(Duration::from_secs(5))
        .map_err(|error| match error {
            ReceiptError::Storage(error) => error.into(),
            _ => ManagerError::UnknownOutcome,
        })
}

fn catalog(
    database: &DatabaseWorker,
    request: CatalogRequest,
) -> Result<serde_json::Value, ManagerError> {
    let Response::Catalog(response) = submit(database, Store::Catalog(Box::new(request)))? else {
        return Err(ManagerError::PersistenceUnavailable);
    };
    match *response {
        CatalogResponse::Launch {
            project,
            preset,
            task,
        } => serde_json::to_value(PreparedLaunch::new(project, preset, task)?.preview()),
        response => serde_json::to_value(response),
    }
    .map_err(|_| ManagerError::InvalidRequest)
}
