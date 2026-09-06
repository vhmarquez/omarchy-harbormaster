use super::*;
use crate::{
    ingestion::{Admission, AdmissionError, EventSession},
    ipc::{Channel, Connection, ConnectionBudget, PrivateSockets},
};
use std::os::unix::net::UnixStream;

struct Peer {
    connection: Connection,
    _client: UnixStream,
    _sockets: PrivateSockets,
    _root: artifacts::Fixture,
}
impl Peer {
    fn new() -> Self {
        let root = artifacts::Fixture::new();
        let sockets = PrivateSockets::bind(&root.0).unwrap();
        let client = UnixStream::connect(root.0.join("harbormaster/events.sock")).unwrap();
        let connection = sockets
            .accept(
                Channel::Event,
                &ConnectionBudget::new(2).unwrap(),
                Instant::now() + Duration::from_secs(3),
            )
            .unwrap()
            .unwrap();
        Self {
            connection,
            _client: client,
            _sockets: sockets,
            _root: root,
        }
    }
    fn session(
        &self,
        registry: &Registry,
        generation: &ProducerGeneration,
    ) -> Result<EventSession, AdmissionError> {
        registry.connect(
            &self.connection,
            &EventHandshake {
                producer_id: id(2),
                generation: generation.clone(),
                run_id: id(4),
                harness: HarnessKind::Hermes,
                requested_signals: EventKind::ALL.to_vec(),
            },
        )
    }
}

#[test]
fn real_peer_uses_committed_generation_and_reconnect_discards_old_backlog_once() {
    let mut fixture = Fixture::new();
    let peer = Peer::new();
    let old = fixture.registered();
    let session = peer.session(&fixture.registry, &old).unwrap();
    let entry = fixture.stage(&old, 1, "turn.started");
    let frame = encode_frame(entry.event()).unwrap();
    assert_eq!(
        fixture.registry.admit(&session, &frame),
        Ok(Admission::Queued)
    );
    assert_eq!(
        fixture.registry.admit(&session, &frame),
        Ok(Admission::AlreadyAdmitted)
    );
    let status = fixture.status();
    assert_eq!(
        status.facts, 0,
        "volatile duplicate is not a durable receipt"
    );
    let input = fixture.input(Some(old.clone()));
    let ReconciliationProgress::Installed {
        generation: new, ..
    } = fixture.reconcile(input)
    else {
        panic!("new generation")
    };
    assert_ne!(old, new);
    assert_eq!(fixture.registry.discarded(), 1);
    let status = fixture.status();
    assert_eq!(status.discarded_events, 1);
    assert_eq!(
        fixture.registry.admit(&session, &frame),
        Err(AdmissionError::StaleGeneration)
    );
    assert!(peer.session(&fixture.registry, &new).is_ok());
    let (result, receipt) = fixture.commit(entry);
    assert_eq!(
        result,
        CommitProgress::Rejected(CommitFailure::Unreconciled)
    );
    assert!(receipt.is_none());
    assert!(
        peer.session(&fixture.registry, &new).is_ok(),
        "old spool cannot invalidate new generation"
    );
}

#[test]
fn missing_hook_retires_and_blocks_real_peer_while_preserving_native_wait() {
    let mut fixture = Fixture::new();
    let peer = Peer::new();
    let generation = fixture.registered();
    let session = peer.session(&fixture.registry, &generation).unwrap();
    let entry = fixture.stage(&generation, 1, "turn.awaiting_approval");
    let event = entry.event().clone();
    let (_, receipt) = fixture.commit(entry);
    assert!(receipt.is_some());
    let entry = fixture.stage(&generation, 2, "turn.started");
    fixture
        .registry
        .admit(&session, &encode_frame(&event).unwrap())
        .unwrap();
    let mut job = CommitJob::new(
        &fixture.worker,
        &mut fixture.registry,
        entry,
        ObservationPolicy::new(&EventKind::ALL, ObservationState::Disconnected).unwrap(),
        1,
        true,
    );
    assert!(matches!(finish(&mut job), CommitProgress::Committed { .. }));
    assert_eq!(job.status().known_queued_discards, 1);
    drop(job);
    let status = fixture.status();
    assert_eq!(
        status.discarded_events, 1,
        "retirement and known queue loss commit together"
    );
    assert!(matches!(
        peer.session(&fixture.registry, &generation),
        Err(AdmissionError::ReconciliationRequired)
    ));
    let Response::Outcome(Some(outcome)) =
        fixture.call(Request::Outcome(crate::storage::OutcomeKey {
            producer_id: id(2),
            generation,
            event_id: event.event_id,
        }))
    else {
        panic!("outcome")
    };
    assert!(
        outcome
            .attention
            .iter()
            .any(|row| row.reason == AttentionReason::NativeApproval && !row.reviewed)
    );
    assert!(outcome.resolved_reasons.is_empty());
}

#[test]
fn independent_old_scope_invalidation_preserves_new_peer_and_queued_event() {
    let mut fixture = Fixture::new();
    let peer = Peer::new();
    let old = fixture.registered();
    let input = fixture.input(Some(old.clone()));
    let ReconciliationProgress::Installed {
        generation: new, ..
    } = fixture.reconcile(input)
    else {
        panic!("new registration");
    };
    let session = peer.session(&fixture.registry, &new).unwrap();
    let entry = fixture.stage(&new, 1, "turn.started");
    let frame = encode_frame(entry.event()).unwrap();
    assert_eq!(
        fixture.registry.admit(&session, &frame),
        Ok(Admission::Queued)
    );
    assert_eq!(fixture.registry.queued(), 1);
    assert_eq!(
        fixture.registry.invalidate_generation(&id(2), Some(&old)),
        Err(AdmissionError::StaleGeneration)
    );
    assert_eq!(fixture.registry.queued(), 1);
    assert_eq!(fixture.registry.discarded(), 0);
    assert_eq!(
        fixture.registry.admit(&session, &frame),
        Ok(Admission::AlreadyAdmitted)
    );
    assert!(peer.session(&fixture.registry, &new).is_ok());
}
