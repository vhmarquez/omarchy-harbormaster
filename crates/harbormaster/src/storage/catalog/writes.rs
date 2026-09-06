use super::{encode, fresh, get};
use crate::projects::{MAX_PRESETS, MAX_PROJECTS, MAX_TASKS, Preset, Project, Task, paths};
use crate::protocol::{LocalPath, ProjectId, TaskLabel};
use crate::storage::{StorageError, queries, transaction};
use rusqlite::{Connection, params};
use std::path::Path;

pub(super) fn project(
    conn: &mut Connection,
    root: &LocalPath,
    label: &TaskLabel,
) -> Result<Project, StorageError> {
    let identity = paths::identity(Path::new(root.as_str()), false)?;
    let tx = conn.transaction()?;
    if let Some(existing) = get::<Project>(
        &tx,
        "SELECT record FROM projects WHERE root=?1",
        [root.as_str()],
    )? {
        return if existing.label == *label && existing.identity == identity {
            Ok(existing)
        } else {
            Err(StorageError::Conflict)
        };
    }
    transaction::capacity(&tx, "SELECT count(*) FROM projects", MAX_PROJECTS, 1)?;
    let record = Project {
        id: fresh()?,
        root: root.clone(),
        label: label.clone(),
        identity,
    };
    let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
    tx.execute(
        "INSERT INTO projects(id,root,record) VALUES(?1,?2,?3)",
        params![record.id.as_str(), root.as_str(), encode(&record)?],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(record)
}

pub(super) fn preset(
    conn: &mut Connection,
    name: &str,
    executable: &LocalPath,
    profile: &str,
) -> Result<Preset, StorageError> {
    let identity = paths::identity(Path::new(executable.as_str()), true)?;
    let record = Preset {
        name: name.to_owned(),
        executable: executable.clone(),
        profile: profile.to_owned(),
        identity,
    };
    let tx = conn.transaction()?;
    if let Some(existing) = get::<Preset>(&tx, "SELECT record FROM presets WHERE name=?1", [name])?
    {
        return if existing == record {
            Ok(existing)
        } else {
            Err(StorageError::Conflict)
        };
    }
    transaction::capacity(&tx, "SELECT count(*) FROM presets", MAX_PRESETS, 1)?;
    let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
    tx.execute(
        "INSERT INTO presets(name,record) VALUES(?1,?2)",
        params![name, encode(&record)?],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(record)
}

pub(super) fn task(
    conn: &mut Connection,
    project_id: &ProjectId,
    preset: &str,
    label: &TaskLabel,
) -> Result<Task, StorageError> {
    let tx = conn.transaction()?;
    if get::<Project>(
        &tx,
        "SELECT record FROM projects WHERE id=?1",
        [project_id.as_str()],
    )?
    .is_none()
        || get::<Preset>(&tx, "SELECT record FROM presets WHERE name=?1", [preset])?.is_none()
    {
        return Err(StorageError::InvalidRequest);
    }
    if let Some(existing) = get::<Task>(
        &tx,
        "SELECT record FROM tasks WHERE project=?1 AND label=?2",
        [project_id.as_str(), label.as_str()],
    )? {
        return if existing.preset == preset {
            Ok(existing)
        } else {
            Err(StorageError::Conflict)
        };
    }
    transaction::capacity(&tx, "SELECT count(*) FROM tasks", MAX_TASKS, 1)?;
    let record = Task {
        id: fresh()?,
        project_id: project_id.clone(),
        preset: preset.to_owned(),
        label: label.clone(),
    };
    let next = queries::require_revision(&tx, queries::revision(&tx)?)?;
    tx.execute(
        "INSERT INTO tasks(id,project,preset,label,record) VALUES(?1,?2,?3,?4,?5)",
        params![
            record.id.as_str(),
            project_id.as_str(),
            preset,
            label.as_str(),
            encode(&record)?
        ],
    )?;
    queries::advance(&tx, next)?;
    tx.commit()?;
    Ok(record)
}
