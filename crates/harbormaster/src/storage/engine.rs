use super::{
    Request, Response, StorageError, backup, maintenance,
    paths::{DATABASE, Paths, ROLLBACK},
    queries, schema, transaction,
};
use rusqlite::Connection;
use std::path::Path;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub(crate) struct Engine {
    connection: Option<Connection>,
    paths: Paths,
    owner: std::sync::Arc<()>,
}
impl Engine {
    /// Explicit manager-only recovery. The caller supplies an authoritative
    /// revision upper bound including possibly committed unknown outcomes.
    /// A last acknowledgement is insufficient; an unknown bound must never be guessed.
    pub(crate) fn recover_backup(
        root: &Path,
        revision_floor: crate::protocol::Revision,
    ) -> Result<Self, StorageError> {
        let paths = Paths::open(root)?;
        schema::header(&paths.path(DATABASE))?;
        // A corrupt main database with pending WAL cannot be safely classified
        // or checkpointed here. Preserve it for explicit supported recovery.
        if paths.length("state.db-wal")? != 0 {
            return Err(StorageError::RecoveryRequired);
        }
        match schema::read_only(&paths.path(DATABASE)) {
            Err(StorageError::CorruptDatabase) => (),
            Err(error) => return Err(error),
            Ok(_) => return Err(StorageError::InvalidRequest),
        }
        let next = backup::stage_restore(&paths, revision_floor)?;
        backup::replace(&paths)?;
        let mut engine = Self {
            connection: None,
            paths,
            owner: std::sync::Arc::new(()),
        };
        engine.install_restore(next)?;
        Ok(engine)
    }

    pub(crate) fn open(root: &Path) -> Result<Self, StorageError> {
        let paths = Paths::open(root)?;
        if paths.length("state.db-wal")? > schema::WAL_BOUND {
            return Err(StorageError::ResourceExhausted);
        }
        let exists = paths.exists(DATABASE)?;
        let version = if exists {
            let readonly = schema::read_only(&paths.path(DATABASE))?;
            schema::inspect(&readonly)?
        } else {
            paths.create(DATABASE)?;
            schema::VERSION
        };
        let mut connection = schema::writable(&paths)?;
        if !exists {
            schema::initialize(&mut connection)?;
        }
        pressure(&connection, &paths)?;
        if version < schema::VERSION {
            backup::create(&connection, &paths)?;
            schema::migrate(&mut connection)?;
        }
        schema::configure(&connection)?;
        pressure(&connection, &paths)?;
        invalidate_generations(&mut connection, None)?;
        checkpoint(&connection)?;
        paths.check()?;
        Ok(Self {
            connection: Some(connection),
            paths,
            owner: std::sync::Arc::new(()),
        })
    }

    pub(crate) fn execute(&mut self, request: &Request) -> Result<Response, StorageError> {
        request.validate()?;
        if let Request::ApplyCleanup(preview) = request
            && !std::sync::Arc::ptr_eq(&self.owner, &preview.owner)
        {
            return Err(StorageError::Conflict);
        }
        self.paths.check()?;
        if matches!(request, Request::RestoreBackup) {
            return self.restore();
        }
        let read_only = matches!(
            request,
            Request::Snapshot(_)
                | Request::Producer(_)
                | Request::Outcome(_)
                | Request::Context(_)
                | Request::Policy
                | Request::Status
                | Request::CleanupPreview { .. }
        );
        let conn = self
            .connection
            .as_mut()
            .ok_or(StorageError::RecoveryRequired)?;
        if !read_only {
            pressure(conn, &self.paths)?;
        }
        let result = dispatch(conn, request, &self.paths, &self.owner);
        // Never turn a successful commit into a reported rollback because a
        // later checkpoint is busy. The next write must pass pressure first.
        if result.is_ok()
            && !read_only
            && self
                .paths
                .length("state.db-wal")
                .is_ok_and(|length| length >= u64::from(schema::WAL_TRIGGER))
        {
            let _ = checkpoint(conn);
        }
        result
    }

    fn restore(&mut self) -> Result<Response, StorageError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or(StorageError::RecoveryRequired)?;
        pressure(conn, &self.paths)?;
        let next = backup::stage_restore(&self.paths, queries::revision(conn)?)?;
        checkpoint(conn)?;
        let conn = self
            .connection
            .take()
            .ok_or(StorageError::RecoveryRequired)?;
        conn.close()
            .map_err(|(_, error)| StorageError::from(error))?;
        backup::replace(&self.paths)?;
        self.install_restore(next)?;
        Ok(Response::Restored { revision: next })
    }

    fn install_restore(&mut self, next: crate::protocol::Revision) -> Result<(), StorageError> {
        let result = load_restored(&self.paths, next);
        let Ok(restored) = result else {
            if self.paths.length("state.db-wal")? != 0 || self.paths.exists("state.db-shm")? {
                return Err(StorageError::RecoveryRequired);
            }
            self.paths.rename(ROLLBACK, DATABASE)?;
            return Err(StorageError::RecoveryRequired);
        };
        self.connection = Some(restored);
        self.owner = std::sync::Arc::new(());
        self.paths.remove(ROLLBACK)?;
        Ok(())
    }
}

fn load_restored(
    paths: &Paths,
    next: crate::protocol::Revision,
) -> Result<Connection, StorageError> {
    let mut restored = schema::writable(paths)?;
    schema::inspect(&restored)?;
    schema::configure(&restored)?;
    invalidate_generations(&mut restored, Some(next))?;
    checkpoint(&restored)?;
    Ok(restored)
}

fn pressure(conn: &Connection, paths: &Paths) -> Result<(), StorageError> {
    let length = paths.length("state.db-wal")?;
    if length > schema::WAL_BOUND {
        return Err(StorageError::ResourceExhausted);
    }
    if length >= u64::from(schema::WAL_TRIGGER) {
        checkpoint(conn)?;
    }
    Ok(())
}
fn checkpoint(conn: &Connection) -> Result<(), StorageError> {
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    if busy != 0 {
        return Err(StorageError::Backpressure);
    }
    Ok(())
}

fn invalidate_generations(
    conn: &mut Connection,
    forced_revision: Option<crate::protocol::Revision>,
) -> Result<(), StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let active: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM producers WHERE active=1)",
        [],
        |row| row.get(0),
    )?;
    if active || forced_revision.is_some() {
        let next = match forced_revision {
            Some(next) => next,
            None => queries::revision(&tx)?
                .checked_next()
                .ok_or(StorageError::ResourceExhausted)?,
        };
        tx.execute(
            "UPDATE producers SET active=0,reconciled=0 WHERE active=1",
            [],
        )?;
        queries::advance(&tx, next)?;
    }
    tx.commit()?;
    Ok(())
}

fn dispatch(
    conn: &mut Connection,
    request: &Request,
    paths: &Paths,
    owner: &std::sync::Arc<()>,
) -> Result<Response, StorageError> {
    match request {
        Request::Context(fact) => {
            super::context::context(conn, fact).map(|context| Response::Context(Box::new(context)))
        }
        Request::Apply(set) => super::reducer_transaction::apply(conn, set),
        Request::Reconcile(request) => super::reconciliation::reconcile(conn, request),
        Request::ReviewOutcome(update) => super::attention_state::review(conn, update),
        Request::Policy => super::policy::policy(conn).map(Response::Policy),
        Request::Status => super::policy::status(conn).map(Response::Status),
        Request::UpdatePolicy {
            expected_revision,
            history,
        } => super::policy::update(conn, *expected_revision, *history),
        Request::CleanupPreview { now } => {
            super::policy::preview(conn, *now, owner).map(Response::CleanupPreview)
        }
        Request::ApplyCleanup(preview) => super::policy::apply(conn, preview),
        Request::Register(registration) => transaction::register(conn, registration),
        Request::Commit(set) => transaction::commit(conn, set),
        Request::Snapshot(query) => queries::snapshot(conn, query).map(Response::Snapshot),
        Request::Producer(id) => queries::producer(conn, id).map(Response::Producer),
        Request::Outcome(key) => queries::outcome(conn, key).map(Response::Outcome),
        Request::MarkReviewed(update) => maintenance::mark_reviewed(conn, update),
        Request::Maintain {
            expected_revision,
            now,
            history,
        } => maintenance::maintain(conn, *expected_revision, *now, *history),
        Request::SetDelivery(update) => maintenance::set_delivery(conn, update),
        Request::Backup => backup::create(conn, paths).map(|()| Response::BackupComplete),
        Request::RestoreBackup => unreachable!(),
    }
}
