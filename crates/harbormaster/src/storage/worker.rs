//! One blocking owner, fixed commands, and bounded outstanding receipts.
mod pending;
#[cfg(test)]
mod tests;

use super::{Engine, Request, Response, StorageError};
use crate::protocol::Revision;
use pending::{Envelope, Pool};
pub use pending::{ReceiptError, SubmitError, SubmitFailure, Ticket};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Covers queued/executing operations and completed, unconsumed tickets together.
pub const MAX_OUTSTANDING: usize = 64;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// A single database thread. Opening the library never starts a service/adapter.
///
/// Drop requests shutdown without waiting on filesystem I/O. An operation
/// already executing may still commit; dropping a ticket does not cancel it.
pub struct DatabaseWorker {
    sender: SyncSender<Envelope>,
    pool: Pool,
    stopping: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl DatabaseWorker {
    /// Open fixed manager-owned state in the database thread.
    ///
    /// # Errors
    /// Returns the sanitized storage failure, or persistence unavailable if
    /// startup fails/times out. A timed-out open may still be finishing OS I/O;
    /// it drops its connection when that I/O returns, without accepting work.
    pub fn open(state_home: &Path) -> Result<Self, StorageError> {
        Self::start(state_home, None)
    }

    /// Explicitly recover an identifiable, corrupt manager database from its
    /// verified fixed backup. No automatic recovery or revision guessing occurs.
    ///
    /// `revision_floor` must be a trusted upper bound on EVERY revision issued
    /// or possibly committed before recovery, including unknown outcomes. The
    /// last acknowledged revision alone is insufficient. If that bound is not
    /// available, this recovery operation must not be used. The restored state
    /// receives a greater revision and requires fresh producer reconciliation.
    /// A startup timeout is an unknown recovery outcome: replacement may already
    /// have committed or may finish after timeout. It does not cancel recovery
    /// or guarantee unchanged state; reconcile before any subsequent operation.
    ///
    /// # Errors
    /// Rejects unsafe paths, foreign/future/unidentifiable databases, pending
    /// WAL data, invalid backups, revision overflow, or failed/timed-out startup.
    pub fn recover_backup(
        state_home: &Path,
        revision_floor: Revision,
    ) -> Result<Self, StorageError> {
        Self::start(state_home, Some(revision_floor))
    }

    fn start(state_home: &Path, recovery_floor: Option<Revision>) -> Result<Self, StorageError> {
        let path = state_home.to_path_buf();
        let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING);
        let (ready, initialized) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let thread = thread::Builder::new()
            .name("harbormaster-db".into())
            .spawn(move || {
                let opened = match recovery_floor {
                    Some(floor) => Engine::recover_backup(&path, floor),
                    None => Engine::open(&path),
                };
                let mut engine = match opened {
                    Ok(engine) => engine,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                if stop.load(Ordering::Acquire) || ready.send(Ok(())).is_err() {
                    return;
                }
                serve(&receiver, &stop, |request| engine.execute(request));
            })
            .map_err(|_| StorageError::PersistenceUnavailable)?;
        match initialized.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                sender,
                pool: Pool::new(),
                stopping,
                thread,
            }),
            result => {
                stopping.store(true, Ordering::Release);
                Err(result
                    .ok()
                    .and_then(Result::err)
                    .unwrap_or(StorageError::PersistenceUnavailable))
            }
        }
    }

    /// Queue a validated command without blocking. Rejection returns ownership
    /// of the exact request; no operation has been queued in that case.
    ///
    /// # Errors
    /// Rejects invalid input, exhausted outstanding capacity, or shutdown.
    pub fn try_submit(&self, request: Request) -> Result<Ticket, SubmitError> {
        if let Err(error) = request.validate() {
            return Err(SubmitError {
                request,
                reason: SubmitFailure::Invalid(error),
            });
        }
        if self.stopping.load(Ordering::Acquire) {
            return Err(SubmitError {
                request,
                reason: SubmitFailure::Stopped,
            });
        }
        self.pool.submit(&self.sender, request)
    }

    /// Request a stop. Queued commands may never execute and their tickets
    /// become unknown outcomes; an executing command may still commit.
    pub fn request_shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    /// Number of outstanding permits, including completed tickets still held.
    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.pool.outstanding()
    }
}

impl Drop for DatabaseWorker {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}

// The private dispatcher seam lets tests pause execution at a channel boundary.
// Production exposes only Request; there is no public SQL/closure interface.
fn serve(
    receiver: &Receiver<Envelope>,
    stopping: &AtomicBool,
    mut execute: impl FnMut(&Request) -> Result<Response, StorageError>,
) {
    while !stopping.load(Ordering::Acquire) {
        match receiver.recv_timeout(POLL_INTERVAL) {
            Ok(envelope) => {
                let _ = envelope.reply.send(execute(envelope.request()));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
