//! Fixed supported live backup; incomplete restore stages fail closed on reopen.
use super::{
    StorageError,
    paths::{BACKUP, DATABASE, Paths, ROLLBACK, STAGING},
    schema,
};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use std::time::{Duration, Instant};

pub(super) fn create(source: &Connection, paths: &Paths) -> Result<(), StorageError> {
    if paths.exists(STAGING)? {
        return Err(StorageError::RecoveryRequired);
    }
    if paths.exists(BACKUP)? {
        schema::read_only(&paths.path(BACKUP))?;
    }
    paths.create(STAGING)?;
    let result = copy_to_stage(source, paths);
    if result.is_err() {
        let _ = paths.remove(STAGING);
    }
    result?;
    paths.sync(STAGING)?;
    paths.rename(STAGING, BACKUP)?;
    Ok(())
}

fn copy_to_stage(source: &Connection, paths: &Paths) -> Result<(), StorageError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut destination = Connection::open_with_flags(
        paths.path(STAGING),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    destination.busy_timeout(Duration::from_millis(50))?;
    destination.pragma_update(None, "max_page_count", schema::MAX_PAGES)?;
    {
        let backup = Backup::new(source, &mut destination)?;
        loop {
            if Instant::now() >= deadline {
                return Err(StorageError::Backpressure);
            }
            match backup.step(32)? {
                StepResult::Done => break,
                StepResult::More => (),
                StepResult::Busy | StepResult::Locked => return Err(StorageError::Backpressure),
                _ => return Err(StorageError::PersistenceUnavailable),
            }
        }
    }
    // Validation visits at most MAX_PAGES. Deadline is checked after validation
    // too; a late result is refused rather than mislabeled timely completion.
    schema::inspect(&destination)?;
    if Instant::now() >= deadline {
        return Err(StorageError::Backpressure);
    }
    destination
        .close()
        .map_err(|(_, error)| StorageError::from(error))?;
    Ok(())
}

pub(super) fn stage_restore(
    paths: &Paths,
    live_revision: crate::protocol::Revision,
) -> Result<crate::protocol::Revision, StorageError> {
    if !paths.exists(BACKUP)? || paths.exists(STAGING)? || paths.exists(ROLLBACK)? {
        return Err(StorageError::RecoveryRequired);
    }
    let backup = schema::read_only(&paths.path(BACKUP))?;
    if schema::inspect(&backup)? != schema::VERSION {
        return Err(StorageError::RecoveryRequired);
    }
    let backup_revision = super::queries::revision(&backup)?;
    let next = live_revision
        .max(backup_revision)
        .checked_next()
        .ok_or(StorageError::ResourceExhausted)?;
    paths.create(STAGING)?;
    let result = copy_to_stage(&backup, paths);
    if result.is_err() {
        let _ = paths.remove(STAGING);
    }
    result?;
    paths.sync(STAGING)?;
    Ok(next)
}

pub(super) fn replace(paths: &Paths) -> Result<(), StorageError> {
    // Each durable rename is followed by directory fsync. A crash between phases
    // leaves a recognized rollback/staging entry; startup refuses automatic
    // cleanup or choosing an ambiguous revision. No unrelated file is salvaged.
    paths.rename(DATABASE, ROLLBACK)?;
    if let Err(error) = paths.rename(STAGING, DATABASE) {
        let _ = paths.rename(ROLLBACK, DATABASE);
        return Err(error);
    }
    Ok(())
}
