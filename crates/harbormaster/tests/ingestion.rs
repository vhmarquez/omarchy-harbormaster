//! Disposable Unix peers and synthetic metadata; no durable/runtime side effects.
use harbormaster::ingestion::{Admission, AdmissionError, ProducerRegistration, Registry};
use harbormaster::ipc::{Channel, Connection, ConnectionBudget, PrivateSockets};
use harbormaster::protocol::{EventHandshake, HarnessKind, Seq, parse_event_handshake};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
#[path = "ingestion/control.rs"]
mod control;
#[path = "ingestion/limits.rs"]
mod limits;
const GENERATION: &str = "33333333-3333-4333-8333-333333333333";
const PRODUCER: &str = "22222222-2222-4222-8222-222222222222";
const RUN: &str = "44444444-4444-4444-8444-444444444444";

struct Fixture {
    client: UnixStream,
    peer: Connection,
    _sockets: PrivateSockets,
    path: PathBuf,
}

impl Fixture {
    fn new(channel: Channel) -> Self {
        let path = std::env::temp_dir().join(format!(
            "hb-ingestion-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let sockets = PrivateSockets::bind(&path).unwrap();
        let filename = if channel == Channel::Event {
            "events.sock"
        } else {
            "control.sock"
        };
        let client = UnixStream::connect(path.join("harbormaster").join(filename)).unwrap();
        let budget = ConnectionBudget::new(64).unwrap();
        let peer = sockets
            .accept(channel, &budget, Instant::now() + Duration::from_secs(5))
            .unwrap()
            .unwrap();
        Self {
            client: client,
            peer,
            _sockets: sockets,
            path,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this isolated test fixture is recursively removed.
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn handshake() -> EventHandshake {
    parse_event_handshake(format!(r#"{{"protocol":0,"channel":"event","operation":"negotiate","producer_id":"{PRODUCER}","generation":"{GENERATION}","run_id":"{RUN}","harness":"hermes","requested_signals":["turn.started"]}}
"#).as_bytes()).unwrap()
}

fn registered(peer: &Connection) -> Registry {
    let h = handshake();
    let mut registry = Registry::new();
    registry
        .register(ProducerRegistration {
            producer_id: h.producer_id,
            generation: h.generation,
            run_id: h.run_id,
            harness: HarnessKind::Hermes,
            uid: peer.peer().uid(),
            allowed_signals: h.requested_signals,
            next_sequence: Seq::new(1),
        })
        .unwrap();
    registry
}

fn event(sequence: u64, id: u64, generation: &str) -> Vec<u8> {
    format!(r#"{{"protocol":0,"channel":"event","event_id":"11111111-1111-4111-8111-{id:012x}","producer_id":"{PRODUCER}","generation":"{generation}","seq":"{sequence}","run_id":"{RUN}","kind":"turn.started","payload":{{"turn_id":"fixture-turn"}}}}
"#).into_bytes()
}

#[test]
fn admission_is_bounded_and_duplicates_never_count_as_durable_acknowledgment() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    let first = event(1, 1, GENERATION);
    assert_eq!(registry.admit(&session, &first), Ok(Admission::Queued));
    assert_eq!(
        registry.admit(&session, &first),
        Ok(Admission::AlreadyAdmitted)
    );
    assert_eq!(registry.queued(), 1);
    assert!(registry.pop().is_some());
    assert_eq!(
        registry.admit(&session, &first),
        Ok(Admission::AlreadyAdmitted)
    );
    assert_eq!(registry.queued(), 0);
}

#[test]
fn wrong_channel_generation_producer_run_and_signal_cannot_queue() {
    let f = Fixture::new(Channel::Control);
    let registry = registered(&f.peer);
    assert!(matches!(
        registry.connect(&f.peer, &handshake()),
        Err(AdmissionError::PermissionDenied)
    ));
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    for (before, after) in [
        (GENERATION, "55555555-5555-4555-8555-555555555555"),
        (PRODUCER, "55555555-5555-4555-8555-555555555555"),
        (RUN, "55555555-5555-4555-8555-555555555555"),
        ("turn.started", "turn.completed"),
    ] {
        let input = String::from_utf8(event(1, 1, GENERATION))
            .unwrap()
            .replace(before, after);
        assert!(registry.admit(&session, input.as_bytes()).is_err());
        assert_eq!(registry.queued(), 0);
    }
}

#[test]
fn unknown_generation_cannot_reconnect_or_reset_sequence() {
    let f = Fixture::new(Channel::Event);
    let registry = registered(&f.peer);
    let mut h = handshake();
    h.generation = "55555555-5555-4555-8555-555555555555".parse().unwrap();
    assert!(matches!(
        registry.connect(&f.peer, &h),
        Err(AdmissionError::StaleGeneration)
    ));
    let empty = Registry::new();
    assert!(matches!(
        empty.connect(&f.peer, &handshake()),
        Err(AdmissionError::UnknownProducer)
    ));
}

#[test]
fn sequence_gap_and_conflict_require_trusted_reconciliation() {
    for bad in [
        event(3, 3, GENERATION),
        event(1, 2, GENERATION),
        event(2, 1, GENERATION),
    ] {
        let f = Fixture::new(Channel::Event);
        let mut registry = registered(&f.peer);
        let session = registry.connect(&f.peer, &handshake()).unwrap();
        registry.admit(&session, &event(1, 1, GENERATION)).unwrap();
        assert!(registry.admit(&session, &bad).is_err());
        assert_eq!(
            registry.admit(&session, &event(2, 2, GENERATION)),
            Err(AdmissionError::ReconciliationRequired)
        );
        assert_eq!(registry.queued(), 1);
    }
}

#[test]
fn per_producer_backpressure_does_not_advance_sequence() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    for i in 1..=128 {
        registry.admit(&session, &event(i, i, GENERATION)).unwrap();
    }
    assert_eq!(
        registry.admit(&session, &event(129, 129, GENERATION)),
        Err(AdmissionError::QueueFull)
    );
    registry.pop().unwrap();
    assert_eq!(
        registry.admit(&session, &event(129, 129, GENERATION)),
        Ok(Admission::Queued)
    );
    assert_eq!(registry.queued(), 128);
}

#[test]
fn fresh_generation_discards_backlog_and_invalidates_old_session() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    registry.admit(&session, &event(1, 1, GENERATION)).unwrap();
    let generation = registry
        .reconcile(&handshake().producer_id, Seq::new(1))
        .unwrap();
    assert_eq!(registry.queued(), 0);
    assert_eq!(registry.discarded(), 1);
    assert_eq!(
        registry.admit(&session, &event(2, 2, GENERATION)),
        Err(AdmissionError::StaleGeneration)
    );
    let mut h = handshake();
    h.generation = generation.clone();
    let session = registry.connect(&f.peer, &h).unwrap();
    assert_eq!(
        registry.admit(&session, &event(1, 3, &generation.to_string())),
        Ok(Admission::Queued)
    );
}

#[test]
fn malformed_content_is_not_retained_or_admitted() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    let malformed = String::from_utf8(event(1, 1, GENERATION)).unwrap().replace(
        "\"turn_id\":",
        "\"prompt\":\"SYNTHETIC-PRIVATE-CONTENT\",\"turn_id\":",
    );
    let error = registry.admit(&session, malformed.as_bytes()).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("SYNTHETIC"));
    assert_eq!(registry.queued(), 0);
    assert_eq!(
        registry.admit(&session, &event(1, 1, GENERATION)),
        Ok(Admission::Queued)
    );
}

#[test]
fn sessions_cannot_cross_registry_instances() {
    let f = Fixture::new(Channel::Event);
    let first = registered(&f.peer);
    let session = first.connect(&f.peer, &handshake()).unwrap();
    let mut second = registered(&f.peer);
    assert_eq!(
        second.admit(&session, &event(1, 1, GENERATION)),
        Err(AdmissionError::PermissionDenied)
    );
}

#[test]
fn a_retired_generation_cannot_be_reactivated() {
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let session = registry.connect(&f.peer, &handshake()).unwrap();
    registry.admit(&session, &event(1, 1, GENERATION)).unwrap();
    let second = registry
        .reconcile(&handshake().producer_id, Seq::new(1))
        .unwrap();
    let third = registry
        .reconcile(&handshake().producer_id, Seq::new(1))
        .unwrap();
    assert_ne!(second, handshake().generation);
    assert_ne!(third, handshake().generation);
    assert_ne!(second, third);
    assert_eq!(
        registry.admit(&session, &event(1, 1, GENERATION)),
        Err(AdmissionError::StaleGeneration)
    );
}

#[test]
fn real_socket_frames_negotiate_then_enter_only_the_event_queue() {
    use harbormaster::protocol::encode_frame;
    use std::io::Write;
    let mut f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    f.client
        .write_all(&encode_frame(&handshake()).unwrap())
        .unwrap();
    let frame = f.peer.read_frame(Instant::now()).unwrap().unwrap();
    let claim = parse_event_handshake(&frame).unwrap();
    let session = registry.connect(&f.peer, &claim).unwrap();
    f.peer.complete_handshake(Instant::now()).unwrap();
    f.client.write_all(&event(1, 1, GENERATION)).unwrap();
    let frame = f.peer.read_frame(Instant::now()).unwrap().unwrap();
    assert_eq!(registry.admit(&session, &frame), Ok(Admission::Queued));
    f.client
        .write_all(b"{\"protocol\":0,\"channel\":\"control\",\"operation\":\"end_owned\"}\n")
        .unwrap();
    let frame = f.peer.read_frame(Instant::now()).unwrap().unwrap();
    assert!(registry.admit(&session, &frame).is_err());
    assert_eq!(registry.queued(), 1);
}

#[test]
fn negotiation_narrows_signals_and_duplicate_registration_cannot_overwrite_scope() {
    use harbormaster::protocol::EventKind;
    let f = Fixture::new(Channel::Event);
    let mut registry = registered(&f.peer);
    let mut h = handshake();
    h.requested_signals.push(EventKind::TurnCompleted);
    let session = registry.connect(&f.peer, &h).unwrap();
    assert_eq!(
        session.negotiated_signals().collect::<Vec<_>>(),
        vec![EventKind::TurnStarted]
    );
    assert_eq!(
        registry.register(ProducerRegistration {
            producer_id: h.producer_id,
            generation: h.generation,
            run_id: h.run_id,
            harness: h.harness,
            uid: f.peer.peer().uid(),
            allowed_signals: h.requested_signals,
            next_sequence: Seq::new(99)
        }),
        Err(AdmissionError::Conflict)
    );
    assert_eq!(
        registry.admit(&session, &event(1, 1, GENERATION)),
        Ok(Admission::Queued)
    );
}
