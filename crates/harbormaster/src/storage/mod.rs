//! Manager-owned SQLite storage, reachable only through the bounded worker.
mod backup;
mod engine;
mod maintenance;
mod paths;
mod queries;
mod schema;
mod transaction;
mod types;
mod validation;
mod worker;
pub(crate) use engine::Engine;
pub use types::*;
pub use worker::{
    DatabaseWorker, MAX_OUTSTANDING, ReceiptError, SubmitError, SubmitFailure, Ticket,
};

/// Sanitized failure categories contain no paths, SQL, metadata or OS messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    InvalidRequest,
    UnsafePath,
    UnsupportedFilesystem,
    AlreadyOwned,
    ForeignDatabase,
    FutureSchema,
    CorruptDatabase,
    RecoveryRequired,
    StaleRevision,
    StaleGeneration,
    UnknownProducer,
    SequenceGap,
    Conflict,
    ResourceExhausted,
    Backpressure,
    PersistenceUnavailable,
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "storage operation failed: {self:?}")
    }
}
impl std::error::Error for StorageError {}
impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Backpressure
            }
            Some(rusqlite::ErrorCode::DiskFull | rusqlite::ErrorCode::TooBig) => {
                Self::ResourceExhausted
            }
            Some(rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase) => {
                Self::CorruptDatabase
            }
            _ => Self::PersistenceUnavailable,
        }
    }
}
impl From<rustix::io::Errno> for StorageError {
    fn from(_: rustix::io::Errno) -> Self {
        Self::PersistenceUnavailable
    }
}
impl From<std::io::Error> for StorageError {
    fn from(_: std::io::Error) -> Self {
        Self::PersistenceUnavailable
    }
}
