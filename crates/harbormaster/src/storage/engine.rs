use super::{
    Request, Response, StorageError, backup, maintenance,
    paths::{DATABASE, Paths, ROLLBACK},
    queries, schema, transaction,
};
use rusqlite::Connection;
use std::path::Path;

pub(crate) struct Engine {
    connection: Option<Connection>,
    paths: Paths,
}
impl Engine {
    pub(crate) fn open(root: &Path) -> Result<Self, StorageError> {
        let paths = Paths::open(root)?;
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
        if version < schema::VERSION {
            backup::create(&connection, &paths)?;
            schema::migrate(&mut connection)?;
        }
        schema::configure(&connection)?;
        invalidate_generations(&mut connection)?;
        checkpoint(&connection)?;
        paths.check()?;
        Ok(Self {
            connection: Some(connection),
            paths,
        })
    }

    pub(crate) fn execute(&mut self, request: &Request) -> Result<Response, StorageError> {
        request.validate()?;
        self.paths.check()?;
        if matches!(request, Request::RestoreBackup) {
            return self.restore();
        }
        let read_only = matches!(
            request,
            Request::Snapshot(_) | Request::Producer(_) | Request::Outcome(_)
        );
        let conn = self
            .connection
            .as_mut()
            .ok_or(StorageError::RecoveryRequired)?;
        if !read_only {
            pressure(conn, &self.paths)?;
        }
        let result = match request {
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
            Request::Backup => backup::create(conn, &self.paths).map(|()| Response::BackupComplete),
            Request::RestoreBackup => unreachable!(),
        };
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
        backup::stage_restore(&self.paths)?;
        checkpoint(conn)?;
        let conn = self
            .connection
            .take()
            .ok_or(StorageError::RecoveryRequired)?;
        conn.close()
            .map_err(|(_, error)| StorageError::from(error))?;
        backup::replace(&self.paths)?;
        let mut restored = schema::writable(&self.paths)?;
        if schema::inspect(&restored)
            .and_then(|_| schema::configure(&restored))
            .is_err()
        {
            drop(restored);
            self.paths.rename(ROLLBACK, DATABASE)?;
            self.connection = Some(schema::writable(&self.paths)?);
            return Err(StorageError::RecoveryRequired);
        }
        invalidate_generations(&mut restored)?;
        let revision = queries::revision(&restored)?;
        self.connection = Some(restored);
        self.paths.remove(ROLLBACK)?;
        Ok(Response::Restored { revision })
    }
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

fn invalidate_generations(conn: &mut Connection) -> Result<(), StorageError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let active: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM producers WHERE active=1)",
        [],
        |row| row.get(0),
    )?;
    if active {
        let next = queries::revision(&tx)?
            .checked_next()
            .ok_or(StorageError::ResourceExhausted)?;
        tx.execute("UPDATE producers SET active=0 WHERE active=1", [])?;
        queries::advance(&tx, next)?;
    }
    tx.commit()?;
    Ok(())
}
