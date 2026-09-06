//! Receipt lookup precedes reduction; only explicit revision conflicts refresh it.
use super::{
    CommitFailure, CommitJob, CommitProgress, DurableReceipt, commit::Purpose, operation::Operation,
};
use crate::{
    domain::reduce,
    storage::{
        EventReceipt, ProducerRetirement, ReducerContext, ReducerWrite, Request, Response,
        StorageError,
    },
};

impl CommitJob<'_> {
    pub(super) fn response(&mut self, result: Result<Response, StorageError>) -> CommitProgress {
        match result {
            Ok(Response::Context(context)) if matches!(self.purpose, Purpose::Context) => {
                self.context(&context)
            }
            Ok(Response::Committed { revision, .. }) if matches!(self.purpose, Purpose::Apply) => {
                self.accept(revision)
            }
            Ok(Response::Retired { .. }) => {
                if let Purpose::Retire(failure) = self.purpose {
                    self.reject(failure)
                } else {
                    self.reject(CommitFailure::InvalidEvidence)
                }
            }
            Err(StorageError::StaleRevision) if self.retries < 3 => {
                self.retries += 1;
                self.purpose = Purpose::Context;
                self.operation = Operation::new(Request::Context(Box::new(self.event().clone())));
                CommitProgress::Pending
            }
            Err(error) => {
                self.freeze();
                self.unknown_gap = true;
                self.reject(CommitFailure::Storage(error))
            }
            _ => self.reject(CommitFailure::InvalidEvidence),
        }
    }

    fn context(&mut self, context: &ReducerContext) -> CommitProgress {
        if let EventReceipt::Exact { revision } = context.receipt {
            return self.accept(revision);
        }
        let Some(producer) = &context.producer else {
            return self.reject(CommitFailure::Unreconciled);
        };
        let event = self.event();
        if producer.generation != event.generation
            || producer.run_id != event.run_id
            || producer.harness != self.entry.as_ref().expect("owned entry").harness()
            || !producer.active
            || !producer.reconciled
        {
            return self.reject(CommitFailure::Unreconciled);
        }
        let failure = match context.receipt {
            EventReceipt::Conflict => Some(CommitFailure::Conflict),
            EventReceipt::PriorSequenceUnverifiable => {
                Some(CommitFailure::PriorSequenceUnverifiable)
            }
            _ if producer.next_sequence != Some(event.seq) => Some(CommitFailure::SequenceGap),
            _ => None,
        };
        if let Some(failure) = failure {
            return self.retire(context.revision, failure);
        }
        let Ok(effects) = reduce(&context.state, event, &self.policy) else {
            return self.retire(context.revision, CommitFailure::InvalidEvidence);
        };
        if effects.reconciliation_required {
            self.freeze();
        }
        self.operation = Operation::new(Request::Apply(Box::new(ReducerWrite {
            expected_revision: context.revision,
            fact: self.event().clone(),
            effects,
            accepted_at: self.accepted_at,
            notify: self.notify,
            discarded_events: self.discarded,
        })));
        self.purpose = Purpose::Apply;
        CommitProgress::Pending
    }

    fn retire(
        &mut self,
        revision: crate::protocol::Revision,
        failure: CommitFailure,
    ) -> CommitProgress {
        self.freeze();
        self.operation = Operation::new(Request::RetireProducer(ProducerRetirement {
            expected_revision: revision,
            producer_id: self.event().producer_id.clone(),
            generation: self.event().generation.clone(),
            discarded_events: self.discarded,
        }));
        self.purpose = Purpose::Retire(failure);
        CommitProgress::Pending
    }

    fn accept(&mut self, revision: crate::protocol::Revision) -> CommitProgress {
        self.receipt = Some(DurableReceipt {
            event: self.event().clone(),
            revision,
        });
        self.done = true;
        CommitProgress::Committed { revision }
    }

    fn reject(&mut self, failure: CommitFailure) -> CommitProgress {
        self.done = true;
        CommitProgress::Rejected(failure)
    }

    fn event(&self) -> &crate::protocol::EventEnvelope {
        self.entry
            .as_ref()
            .expect("job retains its eligible event")
            .event()
    }

    pub(super) fn freeze(&mut self) {
        let Some(entry) = &self.entry else {
            return;
        };
        let event = entry.event();
        if let Ok(count) = self
            .registry
            .invalidate_generation(&event.producer_id, Some(&event.generation))
        {
            self.discarded = self
                .discarded
                .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        }
    }
}
