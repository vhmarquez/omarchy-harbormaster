//! Fixed registry operations owned by the same bounded database worker.
mod writes;
use super::{StorageError, queries};
use crate::projects::{CatalogRequest, CatalogResponse, PAGE_SIZE, Page, Preset, Project, Task};
use rusqlite::{Connection, OptionalExtension, Params};
use serde::{Serialize, de::DeserializeOwned};

pub(super) const TABLES: [(&str, &str); 3] = [
    (
        "projects",
        "CREATE TABLE projects(id TEXT PRIMARY KEY,root TEXT NOT NULL UNIQUE,record BLOB NOT NULL CHECK(length(record)<=12288)) STRICT",
    ),
    (
        "presets",
        "CREATE TABLE presets(name TEXT PRIMARY KEY,record BLOB NOT NULL CHECK(length(record)<=12288)) STRICT",
    ),
    (
        "tasks",
        "CREATE TABLE tasks(id TEXT PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(id),preset TEXT NOT NULL REFERENCES presets(name),label TEXT NOT NULL,record BLOB NOT NULL CHECK(length(record)<=4096),UNIQUE(project,label)) STRICT",
    ),
];

pub(super) fn execute(
    conn: &mut Connection,
    request: &CatalogRequest,
) -> Result<CatalogResponse, StorageError> {
    match request {
        CatalogRequest::AddProject { root, label } => {
            writes::project(conn, root, label).map(CatalogResponse::Project)
        }
        CatalogRequest::AddPreset {
            name,
            executable,
            profile,
        } => writes::preset(conn, name, executable, profile).map(CatalogResponse::Preset),
        CatalogRequest::AddTask {
            project_id,
            preset,
            label,
        } => writes::task(conn, project_id, preset, label).map(CatalogResponse::Task),
        CatalogRequest::Projects { after } => page(
            conn,
            "SELECT id,record FROM projects WHERE id>?1 ORDER BY id LIMIT 101",
            [after
                .as_ref()
                .map_or("", crate::protocol::ProjectId::as_str)],
        )
        .map(CatalogResponse::Projects),
        CatalogRequest::Presets { after } => page(
            conn,
            "SELECT name,record FROM presets WHERE name>?1 ORDER BY name LIMIT 101",
            [after.as_deref().unwrap_or("")],
        )
        .map(CatalogResponse::Presets),
        CatalogRequest::Tasks { project_id, after } => page(
            conn,
            "SELECT id,record FROM tasks WHERE project=?1 AND id>?2 ORDER BY id LIMIT 101",
            [
                project_id.as_str(),
                after.as_ref().map_or("", crate::protocol::TaskId::as_str),
            ],
        )
        .map(CatalogResponse::Tasks),
        CatalogRequest::Task { id } => launch_records(conn, id),
    }
}

fn launch_records(
    conn: &Connection,
    id: &crate::protocol::TaskId,
) -> Result<CatalogResponse, StorageError> {
    let task: Task = get(conn, "SELECT record FROM tasks WHERE id=?1", [id.as_str()])?
        .ok_or(StorageError::InvalidRequest)?;
    let project: Project = get(
        conn,
        "SELECT record FROM projects WHERE id=?1",
        [task.project_id.as_str()],
    )?
    .ok_or(StorageError::CorruptDatabase)?;
    let preset: Preset = get(
        conn,
        "SELECT record FROM presets WHERE name=?1",
        [&task.preset],
    )?
    .ok_or(StorageError::CorruptDatabase)?;
    Ok(CatalogResponse::Launch {
        project,
        preset,
        task,
    })
}

pub(super) fn get<T: DeserializeOwned>(
    conn: &Connection,
    sql: &str,
    values: impl Params,
) -> Result<Option<T>, StorageError> {
    let data: Option<Vec<u8>> = conn.query_row(sql, values, |row| row.get(0)).optional()?;
    data.map(|bytes| decode(&bytes)).transpose()
}

pub(super) fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, StorageError> {
    serde_json::from_slice(data).map_err(|_| StorageError::CorruptDatabase)
}

pub(super) fn encode<T: Serialize>(record: &T) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(record).map_err(|_| StorageError::InvalidRequest)
}

fn page<T: DeserializeOwned>(
    conn: &Connection,
    sql: &str,
    values: impl Params,
) -> Result<Page<T>, StorageError> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(values)?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push((
            row.get::<_, String>(0)?,
            decode::<T>(&row.get::<_, Vec<u8>>(1)?)?,
        ));
    }
    let more = entries.len() > PAGE_SIZE;
    entries.truncate(PAGE_SIZE);
    let next_after = if more {
        entries.last().map(|(id, _)| id.clone())
    } else {
        None
    };
    Ok(Page {
        revision: queries::revision(conn)?,
        items: entries.into_iter().map(|(_, row)| row).collect(),
        next_after,
    })
}

pub(super) fn fresh<T: std::str::FromStr>() -> Result<T, StorageError> {
    crate::generation::fresh()
        .map_err(|_| StorageError::PersistenceUnavailable)?
        .as_str()
        .parse()
        .map_err(|_| StorageError::PersistenceUnavailable)
}

impl From<crate::projects::ProjectError> for StorageError {
    fn from(value: crate::projects::ProjectError) -> Self {
        match value {
            crate::projects::ProjectError::InvalidInput => Self::InvalidRequest,
            crate::projects::ProjectError::UnsafePath => Self::UnsafePath,
            crate::projects::ProjectError::IdentityChanged => Self::Conflict,
        }
    }
}
