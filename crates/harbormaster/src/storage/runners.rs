//! Reserve a durable launch attempt before touching external runtime resources.
use super::{StorageError, catalog, queries, transaction};
use crate::{
    projects::{Page, Task},
    runtime::{MAX_MANAGED_RUNS, ManagedRun, RUN_PAGE_SIZE, RunnerRequest, RunnerResponse},
};
use rusqlite::{Connection, params};

pub(super) const TABLE: (&str, &str) = (
    "managed_runs",
    "CREATE TABLE managed_runs(id TEXT PRIMARY KEY,task TEXT NOT NULL UNIQUE REFERENCES tasks(id),project TEXT NOT NULL REFERENCES projects(id),record BLOB NOT NULL CHECK(length(record)<=8192)) STRICT",
);

pub(super) fn execute(
    conn: &mut Connection,
    request: &RunnerRequest,
) -> Result<RunnerResponse, StorageError> {
    match request {
        RunnerRequest::Reserve {
            task_id,
            runtime_root,
        } => reserve(conn, task_id, runtime_root),
        RunnerRequest::Identify { id, server } => {
            let tx = conn.transaction()?;
            let mut run: ManagedRun = catalog::get(
                &tx,
                "SELECT record FROM managed_runs WHERE id=?1",
                [id.as_str()],
            )?
            .ok_or(StorageError::InvalidRequest)?;
            if run.server.as_ref().is_some_and(|saved| saved != server) {
                return Err(StorageError::Conflict);
            }
            run.server = Some(server.clone());
            let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
            tx.execute(
                "UPDATE managed_runs SET record=?1 WHERE id=?2",
                params![catalog::encode(&run)?, id.as_str()],
            )?;
            queries::advance(&tx, next)?;
            tx.commit()?;
            Ok(RunnerResponse::Identified(run))
        }
        RunnerRequest::List { project_id, after } => list(conn, project_id, after.as_ref()),
    }
}

fn reserve(
    conn: &mut Connection,
    task_id: &crate::protocol::TaskId,
    runtime_root: &crate::protocol::LocalPath,
) -> Result<RunnerResponse, StorageError> {
    let tx = conn.transaction()?;
    if let Some(run) = catalog::get(
        &tx,
        "SELECT record FROM managed_runs WHERE task=?1",
        [task_id.as_str()],
    )? {
        return Ok(RunnerResponse::Reserved {
            run,
            created: false,
        });
    }
    let task: Task = catalog::get(
        &tx,
        "SELECT record FROM tasks WHERE id=?1",
        [task_id.as_str()],
    )?
    .ok_or(StorageError::InvalidRequest)?;
    transaction::capacity(
        &tx,
        "SELECT count(*) FROM managed_runs",
        MAX_MANAGED_RUNS,
        1,
    )?;
    let run = ManagedRun {
        id: catalog::fresh()?,
        task_id: task_id.clone(),
        project_id: task.project_id,
        runtime_root: runtime_root.clone(),
        server: None,
    };
    let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
    tx.execute(
        "INSERT INTO managed_runs(id,task,project,record) VALUES(?1,?2,?3,?4)",
        params![
            run.id.as_str(),
            run.task_id.as_str(),
            run.project_id.as_str(),
            catalog::encode(&run)?
        ],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(RunnerResponse::Reserved { run, created: true })
}

fn list(
    conn: &Connection,
    project_id: &crate::protocol::ProjectId,
    after: Option<&crate::protocol::RunId>,
) -> Result<RunnerResponse, StorageError> {
    let mut query = conn.prepare(
        "SELECT record FROM managed_runs WHERE project=?1 AND id>?2 ORDER BY id LIMIT 11",
    )?;
    let mut rows = query.query([
        project_id.as_str(),
        after.map_or("", crate::protocol::RunId::as_str),
    ])?;
    let mut items = Vec::<ManagedRun>::new();
    while let Some(row) = rows.next()? {
        items.push(catalog::decode(&row.get::<_, Vec<u8>>(0)?)?);
    }
    let more = items.len() > RUN_PAGE_SIZE;
    items.truncate(RUN_PAGE_SIZE);
    let next_after = more
        .then(|| items.last().map(|run| run.id.as_str().to_owned()))
        .flatten();
    Ok(RunnerResponse::Listed(Page {
        revision: queries::revision(conn)?,
        items,
        next_after,
    }))
}
