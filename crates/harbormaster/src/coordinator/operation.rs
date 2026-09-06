//! One retained database intent; polling never waits or fabricates an outcome.
use crate::storage::{
    DatabaseWorker, ReceiptError, Request, Response, StorageError, SubmitFailure, Ticket,
};
use std::time::Duration;

pub(super) struct Operation {
    request: Option<Box<Request>>,
    ticket: Option<Ticket>,
}

pub(super) enum Poll {
    Pending,
    Backpressure,
    Unknown,
    Result(Result<Response, StorageError>),
}

impl Operation {
    pub(super) fn new(request: Request) -> Self {
        Self {
            request: Some(Box::new(request)),
            ticket: None,
        }
    }

    pub(super) fn request(&self) -> &Request {
        self.ticket.as_ref().map_or_else(
            || {
                self.request
                    .as_deref()
                    .expect("an operation retains its intent")
            },
            Ticket::request,
        )
    }

    pub(super) fn poll(&mut self, worker: &DatabaseWorker) -> Poll {
        if self.ticket.is_none() {
            let request = self
                .request
                .take()
                .expect("an unqueued operation owns its request");
            match worker.try_submit(*request) {
                Ok(ticket) => self.ticket = Some(ticket),
                Err(error) => {
                    self.request = Some(Box::new(error.request));
                    return match error.reason {
                        SubmitFailure::Full => Poll::Backpressure,
                        SubmitFailure::Stopped => {
                            Poll::Result(Err(StorageError::PersistenceUnavailable))
                        }
                        SubmitFailure::Invalid(error) => Poll::Result(Err(error)),
                    };
                }
            }
        }
        match self
            .ticket
            .as_mut()
            .expect("submitted operation has a ticket")
            .wait_timeout(Duration::ZERO)
        {
            Ok(response) => Poll::Result(Ok(response)),
            Err(ReceiptError::Pending) => Poll::Pending,
            Err(ReceiptError::Storage(error)) => Poll::Result(Err(error)),
            Err(ReceiptError::UnknownOutcome | ReceiptError::AlreadyReceived) => Poll::Unknown,
        }
    }
}
