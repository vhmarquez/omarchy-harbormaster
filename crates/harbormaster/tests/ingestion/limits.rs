use super::{Fixture, GENERATION, PRODUCER, event, handshake, registered};
use harbormaster::ingestion::{
    Admission, AdmissionError, MAX_PRODUCER_RECEIPTS, MAX_PRODUCERS, MAX_RECEIPTS,
    ProducerRegistration, Registry,
};
use harbormaster::ipc::Channel;
use harbormaster::protocol::{EventHandshake, HarnessKind, Seq};

fn add_producer(registry: &mut Registry, uid: u32, id: u64) -> EventHandshake {
    let mut h = handshake();
    h.producer_id = format!("22222222-2222-4222-8222-{id:012x}")
        .parse()
        .unwrap();
    registry
        .register(ProducerRegistration {
            producer_id: h.producer_id.clone(),
            generation: h.generation.clone(),
            run_id: h.run_id.clone(),
            harness: HarnessKind::Hermes,
            uid,
            allowed_signals: h.requested_signals.clone(),
            next_sequence: Seq::new(1),
        })
        .unwrap();
    h
}

fn scoped_event(h: &EventHandshake, seq: u64) -> Vec<u8> {
    String::from_utf8(event(seq, seq, GENERATION))
        .unwrap()
        .replace(PRODUCER, &h.producer_id.to_string())
        .into_bytes()
}

#[test]
fn global_queue_is_bounded_and_healthy_producers_get_fair_dequeue() {
    let f = Fixture::new(Channel::Event);
    let mut registry = Registry::new();
    let mut sessions = Vec::new();
    for id in 1..=9 {
        let h = add_producer(&mut registry, f.peer.peer().uid(), id);
        sessions.push((registry.connect(&f.peer, &h).unwrap(), h));
    }
    for (session, h) in &sessions[..8] {
        for seq in 1..=128 {
            registry.admit(session, &scoped_event(h, seq)).unwrap();
        }
    }
    assert_eq!(registry.queued(), 1024);
    assert_eq!(
        registry.admit(&sessions[8].0, &scoped_event(&sessions[8].1, 1)),
        Err(AdmissionError::QueueFull)
    );
    for (_, h) in &sessions[..8] {
        assert_eq!(registry.pop().unwrap().producer_id, h.producer_id);
    }
    assert_eq!(
        registry.admit(&sessions[8].0, &scoped_event(&sessions[8].1, 1)),
        Ok(Admission::Queued)
    );
}

#[test]
fn bounded_receipts_freeze_generation_instead_of_forgetting_replay_defenses() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    for seq in 1..=u64::try_from(MAX_PRODUCER_RECEIPTS).unwrap() {
        registry
            .admit(&session, &event(seq, seq, GENERATION))
            .unwrap();
        registry.pop().unwrap();
    }
    assert_eq!(
        registry.admit(&session, &event(1, 1, GENERATION)),
        Ok(Admission::AlreadyAdmitted)
    );
    let next = u64::try_from(MAX_PRODUCER_RECEIPTS).unwrap() + 1;
    assert_eq!(
        registry.admit(&session, &event(next, next, GENERATION)),
        Err(AdmissionError::ReconciliationRequired)
    );
    assert_eq!(
        registry.admit(&session, &event(next, 1, GENERATION)),
        Err(AdmissionError::ReconciliationRequired)
    );
    let fresh = registry
        .reconcile(&handshake().producer_id, Seq::new(1))
        .unwrap();
    assert_ne!(fresh, handshake().generation);
}

#[test]
fn global_replay_records_have_an_independent_hard_ceiling() {
    let f = Fixture::new(Channel::Event);
    let mut registry = Registry::new();
    for id in 1..=u64::try_from(MAX_RECEIPTS / MAX_PRODUCER_RECEIPTS).unwrap() {
        let h = add_producer(&mut registry, f.peer.peer().uid(), id);
        let session = registry.connect(&f.peer, &h).unwrap();
        for seq in 1..=u64::try_from(MAX_PRODUCER_RECEIPTS).unwrap() {
            registry.admit(&session, &scoped_event(&h, seq)).unwrap();
            registry.pop().unwrap();
        }
    }
    let h = add_producer(&mut registry, f.peer.peer().uid(), 100);
    let session = registry.connect(&f.peer, &h).unwrap();
    assert_eq!(
        registry.admit(&session, &scoped_event(&h, 1)),
        Err(AdmissionError::ReconciliationRequired)
    );
}

#[test]
fn active_registry_has_a_hard_ceiling_and_duplicate_registration_does_not_replace() {
    let f = Fixture::new(Channel::Event);
    let mut registry = Registry::new();
    for id in 1..=u64::try_from(MAX_PRODUCERS).unwrap() {
        add_producer(&mut registry, f.peer.peer().uid(), id);
    }
    let h = handshake();
    assert_eq!(
        registry.register(ProducerRegistration {
            producer_id: h.producer_id,
            generation: h.generation,
            run_id: h.run_id,
            harness: HarnessKind::Hermes,
            uid: f.peer.peer().uid(),
            allowed_signals: h.requested_signals,
            next_sequence: Seq::new(1)
        }),
        Err(AdmissionError::ResourceExhausted)
    );
}

#[test]
fn maximum_sequence_does_not_wrap_and_checkpoint_replay_is_refused() {
    let f = Fixture::new(Channel::Event);
    let mut registry = Registry::new();
    let h = handshake();
    registry
        .register(ProducerRegistration {
            producer_id: h.producer_id.clone(),
            generation: h.generation.clone(),
            run_id: h.run_id.clone(),
            harness: h.harness,
            uid: f.peer.peer().uid(),
            allowed_signals: h.requested_signals.clone(),
            next_sequence: Seq::new(u64::MAX),
        })
        .unwrap();
    let session = registry.connect(&f.peer, &h).unwrap();
    assert_eq!(
        registry.admit(&session, &event(0, 1, GENERATION)),
        Err(AdmissionError::ReplayRejected)
    );
    registry
        .admit(&session, &event(u64::MAX, 2, GENERATION))
        .unwrap();
    assert_eq!(
        registry.admit(&session, &event(0, 3, GENERATION)),
        Err(AdmissionError::ReconciliationRequired)
    );
    assert_eq!(registry.queued(), 1);
}

#[test]
fn replay_frame_bytes_have_an_independent_hard_ceiling() {
    use harbormaster::ingestion::MAX_RECEIPT_BYTES;
    use harbormaster::protocol::MAX_FRAME_BYTES;
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    let count = u64::try_from(MAX_RECEIPT_BYTES / MAX_FRAME_BYTES).unwrap();
    for seq in 1..=count {
        let mut frame = event(seq, seq, GENERATION);
        frame.pop();
        frame.resize(MAX_FRAME_BYTES - 1, b' ');
        frame.push(b'\n');
        registry.admit(&session, &frame).unwrap();
        registry.pop().unwrap();
    }
    assert_eq!(
        registry.admit(&session, &event(count + 1, count + 1, GENERATION)),
        Err(AdmissionError::ReconciliationRequired)
    );
}
