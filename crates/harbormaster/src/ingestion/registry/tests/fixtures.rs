use super::super::*;
use crate::ipc::{ConnectionBudget, PrivateSockets};
use crate::protocol::{EventPayload, TurnPayload, encode_frame};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(super) struct Fixture {
    pub peer: Connection,
    _client: UnixStream,
    _sockets: PrivateSockets,
    root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let root = PathBuf::from(std::env::var_os("TMPDIR").expect("isolated fixture TMPDIR"))
            .join(format!(
                "ingestion-install-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let sockets = PrivateSockets::bind(&root).unwrap();
        let client = UnixStream::connect(root.join("harbormaster/events.sock")).unwrap();
        let budget = ConnectionBudget::new(2).unwrap();
        let peer = sockets
            .accept(
                Channel::Event,
                &budget,
                Instant::now() + Duration::from_secs(5),
            )
            .unwrap()
            .unwrap();
        Self {
            peer,
            _client: client,
            _sockets: sockets,
            root,
        }
    }

    pub fn registration(&self, producer: u32, generation: u32, next: u64) -> ProducerRegistration {
        ProducerRegistration {
            producer_id: id(producer),
            generation: id(generation),
            run_id: id(100 + producer),
            harness: HarnessKind::Hermes,
            uid: self.peer.peer().uid(),
            allowed_signals: vec![EventKind::TurnStarted],
            next_sequence: Seq::new(next),
        }
    }

    pub fn session(
        &self,
        registry: &Registry,
        producer: u32,
    ) -> Result<EventSession, AdmissionError> {
        let registration = &registry
            .producers
            .get(&id::<ProducerId>(producer))
            .unwrap()
            .registration;
        registry.connect(
            &self.peer,
            &EventHandshake {
                producer_id: registration.producer_id.clone(),
                generation: registration.generation.clone(),
                run_id: registration.run_id.clone(),
                harness: registration.harness,
                requested_signals: EventKind::ALL.to_vec(),
            },
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // This tree was created solely inside the verifier's disposable TMPDIR.
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

pub(super) fn id<T: std::str::FromStr>(value: u32) -> T
where
    T::Err: std::fmt::Debug,
{
    format!("{value:08x}-0000-4000-8000-000000000000")
        .parse()
        .unwrap()
}

pub(super) fn frame(producer: u32, generation: u32, seq: u64) -> Vec<u8> {
    encode_frame(&EventEnvelope {
        producer_id: id(producer),
        generation: id(generation),
        event_id: id(u32::try_from(seq).unwrap()),
        seq: Seq::new(seq),
        run_id: id(100 + producer),
        event: EventPayload::TurnStarted(TurnPayload {
            turn_id: "fixture-turn".parse().unwrap(),
        }),
    })
    .unwrap()
}
