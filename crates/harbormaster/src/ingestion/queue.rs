//! Bounded round-robin FIFO queues, with one ready entry per producer.
use super::{MAX_PRODUCER_QUEUED, MAX_QUEUED};
use crate::protocol::{EventEnvelope, ProducerId};
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
pub(super) struct Queue {
    producers: BTreeMap<ProducerId, VecDeque<EventEnvelope>>,
    ready: VecDeque<ProducerId>,
    count: usize,
}

impl Queue {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn has_room(&self, producer: &ProducerId) -> bool {
        self.count < MAX_QUEUED
            && self.producers.get(producer).map_or(0, VecDeque::len) < MAX_PRODUCER_QUEUED
    }

    // Caller reserves with has_room under the same exclusive registry borrow.
    pub fn push(&mut self, event: EventEnvelope) {
        let producer = event.producer_id.clone();
        let queue = self.producers.entry(producer.clone()).or_default();
        if queue.is_empty() {
            self.ready.push_back(producer);
        }
        queue.push_back(event);
        self.count += 1;
    }

    pub fn pop(&mut self) -> Option<EventEnvelope> {
        let producer = self.ready.pop_front()?;
        let queue = self.producers.get_mut(&producer)?;
        let event = queue.pop_front()?;
        self.count -= 1;
        if queue.is_empty() {
            self.producers.remove(&producer);
        } else {
            self.ready.push_back(producer);
        }
        Some(event)
    }

    pub fn discard(&mut self, producer: &ProducerId) -> usize {
        let removed = self.producers.remove(producer).map_or(0, |queue| queue.len());
        self.count -= removed;
        self.ready.retain(|id| id != producer);
        removed
    }
}
