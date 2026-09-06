//! Trusted registration, wire negotiation, and volatile replay-safe admission.
use super::{
    Admission, AdmissionError, MAX_PRODUCER_RECEIPTS, MAX_PRODUCERS, MAX_RECEIPT_BYTES,
    MAX_RECEIPTS, queue::Queue,
};
use crate::ipc::{Channel, Connection};
use crate::protocol::{
    EventEnvelope, EventHandshake, EventId, EventKind, HarnessKind, ProducerGeneration, ProducerId,
    RunId, Seq, parse_event,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[cfg(test)]
mod tests;

/// Trusted manager input, never deserialized from a producer frame.
/// Generations must be newly manager-issued or restored with authoritative
/// replay knowledge by the future durable owner. No process proof is inferred.
pub struct ProducerRegistration {
    pub producer_id: ProducerId,
    pub generation: ProducerGeneration,
    pub run_id: RunId,
    pub harness: HarnessKind,
    pub uid: u32,
    pub allowed_signals: Vec<EventKind>,
    pub next_sequence: Seq,
}

/// Connection-local authorization; fields cannot be supplied by wire JSON.
pub struct EventSession {
    authority: Rc<()>,
    scope: Rc<()>,
    producer: ProducerId,
    generation: ProducerGeneration,
    signals: BTreeSet<EventKind>,
}

impl EventSession {
    /// Negotiated intersection only; producer claims never widen registration.
    #[must_use]
    pub fn negotiated_signals(&self) -> impl ExactSizeIterator<Item = EventKind> + '_ {
        self.signals.iter().copied()
    }
}

struct Producer {
    scope: Rc<()>,
    registration: ProducerRegistration,
    next: Option<Seq>,
    blocked: bool,
    receipts: BTreeMap<Seq, EventEnvelope>,
    event_ids: BTreeMap<EventId, Seq>,
    bytes: usize,
}

impl Producer {
    fn new(registration: ProducerRegistration) -> Self {
        Self {
            scope: Rc::new(()),
            next: Some(registration.next_sequence),
            registration,
            blocked: false,
            receipts: BTreeMap::new(),
            event_ids: BTreeMap::new(),
            bytes: 0,
        }
    }

    fn sequence(&mut self, event: &EventEnvelope) -> Result<Admission, AdmissionError> {
        if self.blocked {
            return Err(AdmissionError::ReconciliationRequired);
        }
        if let Some(previous) = self.receipts.get(&event.seq) {
            if previous == event {
                return Ok(Admission::AlreadyAdmitted);
            }
            self.blocked = true;
            return Err(AdmissionError::Conflict);
        }
        if self.event_ids.contains_key(&event.event_id) {
            self.blocked = true;
            return Err(AdmissionError::Conflict);
        }
        let Some(next) = self.next else {
            return Err(AdmissionError::ReconciliationRequired);
        };
        if event.seq < next {
            return Err(AdmissionError::ReplayRejected);
        }
        if event.seq > next {
            self.blocked = true;
            return Err(AdmissionError::SequenceGap);
        }
        Ok(Admission::Queued)
    }
}

/// Empty after restart; callers must explicitly establish trusted registrations.
#[derive(Default)]
pub struct Registry {
    authority: Rc<()>,
    producers: BTreeMap<ProducerId, Producer>,
    queue: Queue,
    receipts: usize,
    receipt_bytes: usize,
    discarded: u64,
    rejected: u64,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register reviewed producer scope without accepting a wire claim as trust.
    /// # Errors
    /// Rejects duplicate registration, invalid scope, or a full active registry.
    pub fn register(&mut self, registration: ProducerRegistration) -> Result<(), AdmissionError> {
        if self.producers.contains_key(&registration.producer_id) {
            return Err(AdmissionError::Conflict);
        }
        if self.producers.len() >= MAX_PRODUCERS {
            return Err(AdmissionError::ResourceExhausted);
        }
        unique_signals(&registration.allowed_signals)?;
        self.producers.insert(
            registration.producer_id.clone(),
            Producer::new(registration),
        );
        Ok(())
    }

    /// Install the exact generation returned by committed durable reconciliation.
    /// The coordinator supplies that receipt; this seam does not establish it.
    /// Validate before discarding old work, and never issue or reuse a generation.
    /// Returns the bounded number of queued observations discarded, without ACK.
    /// # Errors
    /// Rejects changed identity, repeated generation, invalid grants or capacity.
    pub(crate) fn install_committed(
        &mut self,
        registration: ProducerRegistration,
    ) -> Result<usize, AdmissionError> {
        unique_signals(&registration.allowed_signals)?;
        let (receipts, bytes) =
            if let Some(previous) = self.producers.get(&registration.producer_id) {
                let old = &previous.registration;
                if old.run_id != registration.run_id
                    || old.harness != registration.harness
                    || old.uid != registration.uid
                {
                    return Err(AdmissionError::PermissionDenied);
                }
                if old.generation == registration.generation {
                    return Err(AdmissionError::StaleGeneration);
                }
                (previous.receipts.len(), previous.bytes)
            } else {
                if self.producers.len() >= MAX_PRODUCERS {
                    return Err(AdmissionError::ResourceExhausted);
                }
                (0, 0)
            };
        self.receipts -= receipts;
        self.receipt_bytes -= bytes;
        let discarded = self.discard_queued(&registration.producer_id);
        self.producers.insert(
            registration.producer_id.clone(),
            Producer::new(registration),
        );
        Ok(discarded)
    }

    /// Block all sessions and discard queued work without retagging observations.
    /// Retained receipts/checkpoint still protect replay until a committed install.
    /// # Errors
    /// Rejects an unknown producer without changing registry state.
    pub(crate) fn invalidate(&mut self, producer: &ProducerId) -> Result<usize, AdmissionError> {
        let record = self
            .producers
            .get_mut(producer)
            .ok_or(AdmissionError::UnknownProducer)?;
        record.blocked = true;
        record.scope = Rc::new(());
        Ok(self.discard_queued(producer))
    }

    fn discard_queued(&mut self, producer: &ProducerId) -> usize {
        let discarded = self.queue.discard(producer);
        self.discarded = self
            .discarded
            .saturating_add(u64::try_from(discarded).unwrap_or(u64::MAX));
        discarded
    }

    /// Bind an existing scope to a real event-channel peer and narrowed signals.
    /// # Errors
    /// Unknown generations, wrong peers/runs/harnesses, or invalid claims fail.
    pub fn connect(
        &self,
        peer: &Connection,
        handshake: &EventHandshake,
    ) -> Result<EventSession, AdmissionError> {
        if peer.channel() != Channel::Event {
            return Err(AdmissionError::PermissionDenied);
        }
        let producer = self
            .producers
            .get(&handshake.producer_id)
            .ok_or(AdmissionError::UnknownProducer)?;
        let registration = &producer.registration;
        if handshake.generation != registration.generation {
            return Err(AdmissionError::StaleGeneration);
        }
        if peer.peer().uid() != registration.uid
            || handshake.run_id != registration.run_id
            || handshake.harness != registration.harness
        {
            return Err(AdmissionError::PermissionDenied);
        }
        if producer.blocked {
            return Err(AdmissionError::ReconciliationRequired);
        }
        let requested = unique_signals(&handshake.requested_signals)?;
        let allowed = unique_signals(&registration.allowed_signals)?;
        let signals = requested.intersection(&allowed).copied().collect();
        Ok(EventSession {
            authority: self.authority.clone(),
            scope: producer.scope.clone(),
            producer: handshake.producer_id.clone(),
            generation: handshake.generation.clone(),
            signals,
        })
    }

    /// Parse and reserve queue space before advancing any accepted sequence.
    /// # Errors
    /// Returns sanitized schema/auth/recovery/backpressure errors without success.
    pub fn admit(
        &mut self,
        session: &EventSession,
        frame: &[u8],
    ) -> Result<Admission, AdmissionError> {
        if !Rc::ptr_eq(&session.authority, &self.authority) {
            return Err(AdmissionError::PermissionDenied);
        }
        let result = parse_event(frame)
            .map_err(AdmissionError::Protocol)
            .and_then(|event| self.reserve(session, event, frame.len()));
        if result.is_err() {
            self.rejected = self.rejected.saturating_add(1);
        }
        result
    }

    fn reserve(
        &mut self,
        session: &EventSession,
        event: EventEnvelope,
        bytes: usize,
    ) -> Result<Admission, AdmissionError> {
        let producer = self
            .producers
            .get_mut(&session.producer)
            .ok_or(AdmissionError::UnknownProducer)?;
        if !Rc::ptr_eq(&session.scope, &producer.scope)
            || session.generation != producer.registration.generation
            || event.generation != session.generation
        {
            return Err(AdmissionError::StaleGeneration);
        }
        if event.producer_id != session.producer
            || event.run_id != producer.registration.run_id
            || !session.signals.contains(&event.event.kind())
        {
            return Err(AdmissionError::PermissionDenied);
        }
        if producer.sequence(&event)? == Admission::AlreadyAdmitted {
            return Ok(Admission::AlreadyAdmitted);
        }
        if !self.queue.has_room(&session.producer) {
            return Err(AdmissionError::QueueFull);
        }
        if producer.receipts.len() >= MAX_PRODUCER_RECEIPTS
            || self.receipts >= MAX_RECEIPTS
            || bytes > MAX_RECEIPT_BYTES - self.receipt_bytes
        {
            producer.blocked = true;
            return Err(AdmissionError::ReconciliationRequired);
        }
        producer.next = event.seq.checked_next();
        producer.event_ids.insert(event.event_id.clone(), event.seq);
        producer.receipts.insert(event.seq, event.clone());
        producer.bytes += bytes;
        self.receipts += 1;
        self.receipt_bytes += bytes;
        self.queue.push(event);
        Ok(Admission::Queued)
    }

    /// Trusted manager-only reconciliation that issues a fresh generation.
    /// The caller must independently verify run/current-turn identity first and
    /// must never retag discarded callbacks. Callers cannot choose the UUID.
    /// This volatile operation provides no #9/#10 atomic persistence guarantee.
    /// # Errors
    /// Rejects unknown producers or unavailable OS entropy, leaving state intact.
    pub fn reconcile(
        &mut self,
        producer: &ProducerId,
        next: Seq,
    ) -> Result<ProducerGeneration, AdmissionError> {
        let record = self
            .producers
            .get_mut(producer)
            .ok_or(AdmissionError::UnknownProducer)?;
        let generation =
            crate::generation::fresh().map_err(|_| AdmissionError::EntropyUnavailable)?;
        if generation == record.registration.generation {
            return Err(AdmissionError::StaleGeneration);
        }
        self.receipts -= record.receipts.len();
        self.receipt_bytes -= record.bytes;
        record.receipts.clear();
        record.event_ids.clear();
        record.bytes = 0;
        record.registration.generation = generation.clone();
        record.scope = Rc::new(());
        record.next = Some(next);
        record.blocked = false;
        self.discard_queued(producer);
        Ok(generation)
    }

    /// Dequeue fairly for a future consumer. This is not durable acceptance.
    pub fn pop(&mut self) -> Option<EventEnvelope> {
        self.queue.pop()
    }
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.count()
    }
    #[must_use]
    pub fn discarded(&self) -> u64 {
        self.discarded
    }
    #[must_use]
    pub fn rejected(&self) -> u64 {
        self.rejected
    }
}

fn unique_signals(signals: &[EventKind]) -> Result<BTreeSet<EventKind>, AdmissionError> {
    let unique: BTreeSet<_> = signals.iter().copied().collect();
    if signals.len() > EventKind::ALL.len() || unique.len() != signals.len() {
        return Err(AdmissionError::InvalidHandshake);
    }
    Ok(unique)
}
