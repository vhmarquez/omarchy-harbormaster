//! A permit lives as long as either execution or its retained receipt.
use super::super::{Request, Response, StorageError};
use super::MAX_OUTSTANDING;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitFailure {
    Invalid(StorageError),
    Full,
    Stopped,
}

/// An unqueued request remains owned by the caller, unchanged.
#[derive(Debug)]
pub struct SubmitError {
    pub request: Request,
    pub reason: SubmitFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptError {
    /// The request remains available on this ticket; it may still commit.
    Pending,
    /// Execution/reply channel ended without a result; no rollback is implied.
    UnknownOutcome,
    /// This ticket's result was already received; it cannot be acknowledged twice.
    AlreadyReceived,
    Storage(StorageError),
}

struct Pending {
    request: Request,
    _permit: Permit,
}

struct Permit {
    count: Arc<AtomicUsize>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) struct Envelope {
    pending: Arc<Pending>,
    pub(super) reply: SyncSender<Result<Response, StorageError>>,
}

impl Envelope {
    pub(super) fn request(&self) -> &Request {
        &self.pending.request
    }
}

/// The exact pending request and one-slot reply are retained through timeouts.
/// Dropping the ticket releases its reference, not an executing operation.
pub struct Ticket {
    pending: Arc<Pending>,
    receiver: Receiver<Result<Response, StorageError>>,
    received: bool,
}

impl Ticket {
    /// Trusted input for later retry/reconciliation; never a durable success proof.
    #[must_use]
    pub fn request(&self) -> &Request {
        &self.pending.request
    }

    /// Receive the result at most once. Timeout preserves this same ticket.
    ///
    /// # Errors
    /// Distinguishes pending/unknown outcome, an already consumed result, and
    /// an explicit storage error. None claims that a timed-out write rolled back.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Result<Response, ReceiptError> {
        if self.received {
            return Err(ReceiptError::AlreadyReceived);
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => {
                self.received = true;
                result.map_err(ReceiptError::Storage)
            }
            Err(RecvTimeoutError::Timeout) => Err(ReceiptError::Pending),
            Err(RecvTimeoutError::Disconnected) => Err(ReceiptError::UnknownOutcome),
        }
    }
}

pub(super) struct Pool {
    count: Arc<AtomicUsize>,
}

impl Pool {
    pub(super) fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(super) fn outstanding(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    pub(super) fn submit(
        &self,
        sender: &SyncSender<Envelope>,
        request: Request,
    ) -> Result<Ticket, SubmitError> {
        if self
            .count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_OUTSTANDING).then_some(count + 1)
            })
            .is_err()
        {
            return Err(SubmitError {
                request,
                reason: SubmitFailure::Full,
            });
        }
        let pending = Arc::new(Pending {
            request,
            _permit: Permit {
                count: Arc::clone(&self.count),
            },
        });
        let (reply, receiver) = mpsc::sync_channel(1);
        match sender.try_send(Envelope {
            pending: Arc::clone(&pending),
            reply,
        }) {
            Ok(()) => Ok(Ticket {
                pending,
                receiver,
                received: false,
            }),
            Err(error) => {
                let (envelope, reason) = match error {
                    TrySendError::Full(envelope) => (envelope, SubmitFailure::Full),
                    TrySendError::Disconnected(envelope) => (envelope, SubmitFailure::Stopped),
                };
                drop(envelope);
                // Pending remains private and has exactly this local reference.
                let request = Arc::try_unwrap(pending)
                    .unwrap_or_else(|_| unreachable!("unqueued request has no external reference"))
                    .request;
                Err(SubmitError { request, reason })
            }
        }
    }
}
