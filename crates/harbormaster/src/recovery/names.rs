//! Only a generated spool name is eligible for automatic age classification.
use crate::protocol::{EventEnvelope, EventId, ProducerGeneration, ProducerId, Seq};

#[derive(Clone)]
pub(super) struct SpoolName {
    pub producer: ProducerId,
    pub generation: ProducerGeneration,
    pub event: EventId,
    pub sequence: Seq,
    pub observed_ms: u64,
    pub partial: bool,
}
impl SpoolName {
    pub fn new(event: &EventEnvelope, observed_ms: u64, partial: bool) -> Self {
        Self {
            producer: event.producer_id.clone(),
            generation: event.generation.clone(),
            event: event.event_id.clone(),
            sequence: event.seq,
            observed_ms,
            partial,
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        let body = name.strip_prefix("spool-")?;
        let (body, partial) = if let Some(body) = body.strip_suffix(".part") {
            (body, true)
        } else {
            (body.strip_suffix(".json")?, false)
        };
        let pieces: Vec<_> = body.split('_').collect();
        if pieces.len() != 5
            || [pieces[2], pieces[4]]
                .iter()
                .any(|field| field.len() != 20 || !field.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return None;
        }
        Some(Self {
            producer: pieces[0].parse().ok()?,
            generation: pieces[1].parse().ok()?,
            sequence: Seq::new(pieces[2].parse().ok()?),
            event: pieces[3].parse().ok()?,
            observed_ms: pieces[4].parse().ok()?,
            partial,
        })
    }
    pub fn filename(&self) -> String {
        format!(
            "spool-{}_{}_{:020}_{}_{:020}.{}",
            self.producer,
            self.generation,
            self.sequence.value(),
            self.event,
            self.observed_ms,
            if self.partial { "part" } else { "json" }
        )
    }
    pub fn matches(&self, event: &EventEnvelope) -> bool {
        self.same_event_identity(event) && self.sequence == event.seq
    }
    pub fn same_event_identity(&self, event: &EventEnvelope) -> bool {
        self.producer == event.producer_id
            && self.generation == event.generation
            && self.event == event.event_id
    }
    pub fn expired(&self, now_ms: u64) -> bool {
        now_ms
            .checked_sub(self.observed_ms)
            .is_some_and(|age| age >= super::SPOOL_TTL_MS)
    }
}
