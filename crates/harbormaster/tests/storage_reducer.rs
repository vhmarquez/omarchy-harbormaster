//! Public worker integration on the verifier's private, bounded regular tmpfs.
use harbormaster::{
    domain::{
        AttentionReason, Effects, ObservationState, ProcessState, RunProjection, TombstoneEvidence,
        TurnKey, TurnState,
    },
    protocol::{
        EventEnvelope, EventPayload, HarnessKind, ProducerGeneration, Revision, Seq, TurnPayload,
    },
    storage::{
        DatabaseWorker, EventReceipt, OutcomeKey, ReceiptError, Reconciliation,
        ReconciliationBaseline, ReducerWrite, Registration, Request, Response, StorageError,
    },
};
use std::{
    fs,
    io::Write,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    worker: DatabaseWorker,
}
impl Fixture {
    fn new() -> Self {
        let stat = rustix::fs::statfs("/fault-fs").expect("required private fault tmpfs");
        assert_eq!(stat.f_type, 0x0102_1994);
        let capacity = stat
            .f_blocks
            .checked_mul(u64::try_from(stat.f_bsize).unwrap())
            .unwrap();
        assert!(
            capacity > 0 && capacity <= 1024 * 1024,
            "refuse an unbounded or host filesystem"
        );
        let root = PathBuf::from(format!("/fault-fs/storage-full-{}", std::process::id()));
        Self::at(root)
    }
    fn at(root: PathBuf) -> Self {
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let worker = DatabaseWorker::open(&root).unwrap();
        Self { root, worker }
    }
    fn call(&self, request: Request) -> Result<Response, ReceiptError> {
        self.worker
            .try_submit(request)
            .unwrap()
            .wait_timeout(Duration::from_secs(3))
    }
    fn reconcile(&self) -> ProducerGeneration {
        let Response::Registered { generation, .. } = self
            .call(Request::Reconcile(Box::new(Reconciliation {
                registration: Registration {
                    expected_revision: Revision::new(0),
                    producer_id: id(1),
                    run_id: id(2),
                    harness: HarnessKind::Hermes,
                    next_sequence: Seq::new(1),
                    previous_generation: None,
                },
                baseline: ReconciliationBaseline::Verified {
                    projection: RunProjection {
                        run_id: id(2),
                        process: ProcessState::Alive,
                        observation: ObservationState::Fresh,
                        turn: TurnState::Unknown,
                        turn_id: None,
                    },
                    current_turn: None,
                },
                discarded_events: 0,
                resolved_turn: None,
            })))
            .unwrap()
        else {
            panic!("registration")
        };
        generation
    }
}
#[test]
fn cleanup_preview_from_another_worker_cannot_authorize_cleanup() {
    let a =
        Fixture::at(std::env::temp_dir().join(format!("storage-preview-{}-a", std::process::id())));
    let b =
        Fixture::at(std::env::temp_dir().join(format!("storage-preview-{}-b", std::process::id())));
    let Response::CleanupPreview(preview) = a.call(Request::CleanupPreview { now: 100 }).unwrap()
    else {
        panic!("preview")
    };
    assert_eq!(
        b.call(Request::ApplyCleanup(Box::new(preview))),
        Err(ReceiptError::Storage(StorageError::Conflict))
    );
    let Response::Status(status) = b.call(Request::Status).unwrap() else {
        panic!("status")
    };
    assert_eq!(status.revision, Revision::new(0));
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.worker.request_shutdown();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !self.worker.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.worker.is_finished() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn id<T: std::str::FromStr>(value: u32) -> T
where
    T::Err: std::fmt::Debug,
{
    format!("{value:08x}-0000-4000-8000-000000000000")
        .parse()
        .unwrap()
}
fn write(generation: ProducerGeneration) -> ReducerWrite {
    let key = TurnKey {
        producer_id: id(1),
        generation: generation.clone(),
        run_id: id(2),
        turn_id: "fixture-T".parse().unwrap(),
    };
    ReducerWrite {
        expected_revision: Revision::new(1),
        fact: EventEnvelope {
            event_id: id(3),
            producer_id: id(1),
            generation,
            seq: Seq::new(1),
            run_id: id(2),
            event: EventPayload::TurnCompleted(TurnPayload {
                turn_id: key.turn_id.clone(),
            }),
        },
        effects: Effects {
            projection: RunProjection {
                run_id: id(2),
                process: ProcessState::Alive,
                observation: ObservationState::Fresh,
                turn: TurnState::Completed,
                turn_id: Some(key.turn_id.clone()),
            },
            current_turn: Some(key.clone()),
            attention: vec![AttentionReason::Completion],
            resolve_transient: None,
            tombstone: Some(TombstoneEvidence {
                key,
                outcome_id: id(3),
                terminal: Some(TurnState::Completed),
            }),
            reconciliation_required: false,
        },
        accepted_at: 100,
        notify: true,
    }
}
#[test]
fn regular_filesystem_enospc_never_acknowledges_or_partially_commits_then_same_request_retries() {
    let fixture = Fixture::new();
    let set = write(fixture.reconcile());
    let before = fixture
        .call(Request::Context(Box::new(set.fact.clone())))
        .unwrap();
    let ballast_path = fill(&fixture);
    let result = fixture.call(Request::Apply(Box::new(set.clone())));
    assert!(
        matches!(
            result,
            Err(ReceiptError::Storage(
                StorageError::ResourceExhausted | StorageError::PersistenceUnavailable
            ))
        ),
        "expected actual SQLite write failure: {result:?}"
    );
    assert_eq!(
        fixture
            .call(Request::Context(Box::new(set.fact.clone())))
            .unwrap(),
        before
    );
    assert_empty_effects(&fixture, &set);
    fs::remove_file(ballast_path).unwrap();
    assert_eq!(
        fixture.call(Request::Apply(Box::new(set.clone()))).unwrap(),
        Response::Committed {
            revision: Revision::new(2),
            duplicate: false
        }
    );
    let Response::Context(context) = fixture.call(Request::Context(Box::new(set.fact))).unwrap()
    else {
        panic!("context")
    };
    assert_eq!(
        context.receipt,
        EventReceipt::Exact {
            revision: Revision::new(2)
        }
    );
    assert!(context.state.tombstone.is_some());
}

fn fill(fixture: &Fixture) -> PathBuf {
    let ballast_path = fixture.root.join("owned-ballast");
    let mut ballast = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ballast_path)
        .unwrap();
    let mut full = false;
    for _ in 0..=128 {
        match ballast.write_all(&[0x5a; 8192]) {
            Ok(()) => (),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(28));
                full = true;
                break;
            }
        }
    }
    assert!(full, "the fixed filesystem must produce real ENOSPC");
    ballast_path
}

fn assert_empty_effects(fixture: &Fixture, set: &ReducerWrite) {
    let Response::Status(status) = fixture.call(Request::Status).unwrap() else {
        panic!("status")
    };
    assert_eq!(
        (
            status.facts,
            status.tombstones,
            status.attention,
            status.pending_deliveries
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        fixture
            .call(Request::Outcome(OutcomeKey {
                producer_id: id(1),
                generation: set.fact.generation.clone(),
                event_id: id(3)
            }))
            .unwrap(),
        Response::Outcome(None)
    );
}
