//! One explicit reconciliation; a lost generation receipt requires fresh proof.
use super::operation::{Operation, Poll};
use crate::{
    domain::{
        ObservationPolicy, ObservationState, ReconciliationEvidence, TurnKey, reconcile_projection,
    },
    ingestion::{AdmissionError, ProducerRegistration, Registry},
    protocol::{EventKind, ProducerGeneration, Revision},
    storage::{
        DatabaseWorker, Reconciliation, ReconciliationBaseline, Registration, Request, Response,
        StorageError,
    },
};

/// Trusted manager evidence and grants. None of these fields are wire authority.
pub struct ReconciliationInput {
    pub registration: Registration,
    pub evidence: Option<ReconciliationEvidence>,
    pub uid: u32,
    pub allowed_signals: Vec<EventKind>,
    pub resolved_turn: Option<TurnKey>,
    pub continued_turn: Option<TurnKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationProgress {
    Pending,
    Backpressure,
    UnknownOutcome,
    Installed {
        generation: ProducerGeneration,
        revision: Revision,
    },
    Unavailable {
        generation: ProducerGeneration,
        revision: Revision,
    },
    Rejected(StorageError),
    AdmissionRejected(AdmissionError),
}

pub struct ReconciliationJob<'a> {
    worker: &'a DatabaseWorker,
    registry: &'a mut Registry,
    input: ReconciliationInput,
    operation: Operation,
    verified: bool,
    done: bool,
    progress: ReconciliationProgress,
}

impl<'a> ReconciliationJob<'a> {
    /// Validate bounded grants before any durable or volatile mutation.
    /// # Errors
    /// Rejects invalid capability grants. Missing identity proof stays unavailable.
    pub fn new(
        worker: &'a DatabaseWorker,
        registry: &'a mut Registry,
        input: ReconciliationInput,
    ) -> Result<Self, AdmissionError> {
        ObservationPolicy::new(&input.allowed_signals, ObservationState::Fresh)
            .map_err(|_| AdmissionError::InvalidHandshake)?;
        if input.resolved_turn.is_some() && input.continued_turn.is_some() {
            return Err(AdmissionError::ReconciliationRequired);
        }
        let baseline = baseline(&input);
        let verified = matches!(baseline, ReconciliationBaseline::Verified { .. });
        if !verified && (input.resolved_turn.is_some() || input.continued_turn.is_some()) {
            return Err(AdmissionError::ReconciliationRequired);
        }
        let discarded = registry.invalidate_generation(
            &input.registration.producer_id,
            input.registration.previous_generation.as_ref(),
        )?;
        let request = Request::Reconcile(Box::new(Reconciliation {
            registration: input.registration.clone(),
            baseline,
            discarded_events: u64::try_from(discarded).unwrap_or(u64::MAX),
            resolved_turn: input.resolved_turn.clone(),
            continued_turn: input.continued_turn.clone(),
        }));
        Ok(Self {
            worker,
            registry,
            input,
            operation: Operation::new(request),
            verified,
            done: false,
            progress: ReconciliationProgress::Pending,
        })
    }

    /// Poll once without blocking. Unknown completion never installs a generation.
    pub fn poll(&mut self) -> ReconciliationProgress {
        if self.done {
            return self.progress.clone();
        }
        self.progress = match self.operation.poll(self.worker) {
            Poll::Pending => ReconciliationProgress::Pending,
            Poll::Backpressure => ReconciliationProgress::Backpressure,
            Poll::Unknown => {
                self.done = true;
                ReconciliationProgress::UnknownOutcome
            }
            Poll::Result(result) => {
                self.done = true;
                self.received(result)
            }
        };
        self.progress.clone()
    }

    fn received(&mut self, result: Result<Response, StorageError>) -> ReconciliationProgress {
        let (generation, revision) = match result {
            Ok(Response::Registered {
                generation,
                revision,
            }) => (generation, revision),
            Err(error) => return ReconciliationProgress::Rejected(error),
            _ => return ReconciliationProgress::Rejected(StorageError::InvalidRequest),
        };
        if !self.verified {
            return ReconciliationProgress::Unavailable {
                generation,
                revision,
            };
        }
        let registration = &self.input.registration;
        let installed = self.registry.install_committed(ProducerRegistration {
            producer_id: registration.producer_id.clone(),
            generation: generation.clone(),
            run_id: registration.run_id.clone(),
            harness: registration.harness,
            uid: self.input.uid,
            allowed_signals: self.input.allowed_signals.clone(),
            next_sequence: registration.next_sequence,
        });
        match installed {
            Ok(_) => ReconciliationProgress::Installed {
                generation,
                revision,
            },
            Err(error) => ReconciliationProgress::AdmissionRejected(error),
        }
    }

    /// Inspect the unchanged request; this is not a commit or generation receipt.
    #[must_use]
    pub fn request(&self) -> &Request {
        self.operation.request()
    }
}

fn baseline(input: &ReconciliationInput) -> ReconciliationBaseline {
    let Some(evidence) = &input.evidence else {
        return ReconciliationBaseline::Unavailable;
    };
    let scope = &evidence.expected_scope;
    let registration = &input.registration;
    if scope.producer_id != registration.producer_id
        || scope.run_id != registration.run_id
        || scope.harness != registration.harness
        || scope.previous_generation != registration.previous_generation
    {
        return ReconciliationBaseline::Unavailable;
    }
    match reconcile_projection(evidence) {
        Ok(projection) => ReconciliationBaseline::Verified {
            current_turn: projection.turn_id.clone(),
            projection,
        },
        Err(_) => ReconciliationBaseline::Unavailable,
    }
}
