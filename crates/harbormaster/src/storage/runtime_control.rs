//! Manager-only runtime control intent; no producer can submit these operations.
use super::{StorageError, catalog, queries};
use crate::{
    projects::Project,
    runtime::{ControlRequest, ControlResponse, ControlState},
};
use rusqlite::{Connection, params};

pub(super) const TABLE: (&str, &str) = (
    "runtime_controls",
    "CREATE TABLE runtime_controls(id TEXT PRIMARY KEY REFERENCES managed_runs(id),record BLOB NOT NULL CHECK(length(record)<=8192)) STRICT",
);

pub(super) fn execute(
    conn: &mut Connection,
    request: &ControlRequest,
) -> Result<ControlResponse, StorageError> {
    match request {
        ControlRequest::Get(id) => {
            get(conn, id.as_str()).map(|s| ControlResponse::State(s.map(Box::new)))
        }
        ControlRequest::Save(state) => save(conn, state),
        ControlRequest::OtherWriters {
            project_id,
            task_id,
        } => {
            let project: Project = catalog::get(
                conn,
                "SELECT record FROM projects WHERE id=?1",
                [project_id.as_str()],
            )?
            .ok_or(StorageError::InvalidRequest)?;
            let mut query = conn.prepare("SELECT p.record,c.record FROM managed_runs r JOIN projects p ON p.id=r.project LEFT JOIN runtime_controls c ON c.id=r.id WHERE r.task<>?1 LIMIT 1001")?;
            let mut rows = query.query([task_id.as_str()])?;
            let mut count = 0;
            while let Some(row) = rows.next()? {
                count += 1;
                if count > crate::runtime::MAX_MANAGED_RUNS {
                    return Err(StorageError::ResourceExhausted);
                }
                let other: Project = catalog::decode(&row.get::<_, Vec<u8>>(0)?)?;
                let state = row
                    .get::<_, Option<Vec<u8>>>(1)?
                    .map(|b| catalog::decode::<ControlState>(&b))
                    .transpose()?;
                if other.identity == project.identity && !state.is_some_and(|s| s.stopped) {
                    return Ok(ControlResponse::OtherWriters(true));
                }
            }
            Ok(ControlResponse::OtherWriters(false))
        }
    }
}

fn get(conn: &Connection, id: &str) -> Result<Option<ControlState>, StorageError> {
    catalog::get(
        conn,
        "SELECT record FROM runtime_controls WHERE id=?1",
        [id],
    )
}

fn save(conn: &mut Connection, state: &ControlState) -> Result<ControlResponse, StorageError> {
    state.validate().map_err(|_| StorageError::InvalidRequest)?;
    let tx = conn.transaction()?;
    if let Some(old) = get(&tx, state.id.as_str())? {
        if old == *state {
            return Ok(ControlResponse::Saved);
        }
        if old.launch_nonce != state.launch_nonce
            || old.invocation.is_some() && old.invocation != state.invocation
            || old.pane.is_some() && old.pane != state.pane
            || old.stopped && !state.stopped
            || old.ending && !state.ending
            || old.terminal.as_ref().is_some_and(|t| {
                t.process.is_none()
                    && state
                        .terminal
                        .as_ref()
                        .is_none_or(|next| next.nonce != t.nonce)
            })
        {
            return Err(StorageError::Conflict);
        }
    }
    let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
    tx.execute("INSERT INTO runtime_controls VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",
        params![state.id.as_str(), catalog::encode(state)?])?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(ControlResponse::Saved)
}
