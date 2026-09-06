//! Actual public worker/SQLite tests in disposable manager-owned directories.
#[path = "storage/common.rs"]
mod common;
#[path = "storage/event.rs"]
mod event;
#[path = "storage/linkage.rs"]
mod linkage;
#[path = "storage/policy.rs"]
mod policy;
#[path = "storage/queries.rs"]
mod queries;
#[path = "storage/recovery.rs"]
mod recovery;
#[path = "storage/transactions.rs"]
mod transactions;
