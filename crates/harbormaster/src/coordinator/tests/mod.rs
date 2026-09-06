//! Private synthetic source qualification through the real durable owner.
mod crash;
mod peers;
mod pending;
mod pipeline;
mod review;
use super::*;
use crate::{
    domain::*,
    ingestion::Registry,
    protocol::*,
    recovery::{ArtifactStore, tests as artifacts},
    storage::{DatabaseWorker, Registration, Request, Response},
};
use std::time::{Duration, Instant};

struct Fixture {
    root: artifacts::Fixture,
    worker: DatabaseWorker,
    store: ArtifactStore,
    registry: Registry,
}
impl Fixture {
    fn new() -> Self {
        let root = artifacts::Fixture::new();
        Self::at(root)
    }
    fn at(root: artifacts::Fixture) -> Self {
        let worker = DatabaseWorker::open(&root.0).unwrap();
        let store = root.open();
        Self {
            root,
            worker,
            store,
            registry: Registry::new(),
        }
    }
    fn call(&self, request: Request) -> Response {
        self.worker
            .try_submit(request)
            .unwrap()
            .wait_timeout(Duration::from_secs(3))
            .unwrap()
    }
    fn status(&self) -> crate::storage::StorageStatus {
        let Response::Status(status) = self.call(Request::Status) else {
            panic!("status")
        };
        status
    }
    fn revision(&self) -> Revision {
        self.status().revision
    }

    fn input(&self, previous: Option<ProducerGeneration>) -> ReconciliationInput {
        let scope = ReconciliationScope {
            producer_id: id(2),
            previous_generation: previous.clone(),
            run_id: id(4),
            harness: HarnessKind::Hermes,
        };
        let runtime = RuntimeIdentity {
            boot_id: [1; 16],
            pid: 1,
            start_ticks: 10,
        };
        ReconciliationInput {
            registration: Registration {
                expected_revision: self.revision(),
                producer_id: id(2),
                run_id: id(4),
                harness: HarnessKind::Hermes,
                next_sequence: Seq::new(1),
                previous_generation: previous,
            },
            evidence: Some(ReconciliationEvidence {
                expected_scope: scope.clone(),
                observed_scope: scope,
                expected_runtime: runtime,
                observed_runtime: runtime,
                current_boot: [1; 16],
                process: ProcessState::Alive,
                freshness: ObservationState::Fresh,
                current_turn: CurrentTurnEvidence::KnownNone,
            }),
            uid: rustix::process::getuid().as_raw(),
            allowed_signals: EventKind::ALL.to_vec(),
            resolved_turn: None,
            continued_turn: None,
        }
    }
    fn reconcile(&mut self, input: ReconciliationInput) -> ReconciliationProgress {
        let mut job = ReconciliationJob::new(&self.worker, &mut self.registry, input).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let progress = job.poll();
            if !matches!(
                progress,
                ReconciliationProgress::Pending | ReconciliationProgress::Backpressure
            ) {
                return progress;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn registered(&mut self) -> ProducerGeneration {
        let input = self.input(None);
        let ReconciliationProgress::Installed { generation, .. } = self.reconcile(input) else {
            panic!("installed exact durable generation")
        };
        generation
    }
    fn stage(&mut self, generation: &ProducerGeneration, seq: u64, kind: &str) -> ReplayEntry {
        let mut value: serde_json::Value =
            serde_json::from_slice(&artifacts::source(seq, 1)).unwrap();
        value["metadata"]["generation"] = generation.as_str().into();
        value["metadata"]["kind"] = kind.into();
        artifacts::stage_synthetic(&mut self.store, &serde_json::to_vec(&value).unwrap(), 1000);
        artifacts::read_synthetic(&self.store, 1000)
            .into_iter()
            .find(|entry| {
                entry.event().generation == *generation && entry.event().seq == Seq::new(seq)
            })
            .unwrap()
    }
    fn commit(
        &mut self,
        entry: ReplayEntry,
    ) -> (CommitProgress, Option<(DurableReceipt, ReplayEntry)>) {
        let mut job = CommitJob::new(&self.worker, &mut self.registry, entry, policy(), 1, true);
        let progress = finish(&mut job);
        (progress, job.take_committed())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.worker.request_shutdown();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !self.worker.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
fn id<T: std::str::FromStr>(n: u64) -> T
where
    T::Err: std::fmt::Debug,
{
    format!("00000000-0000-4000-8000-{n:012x}").parse().unwrap()
}
fn policy() -> ObservationPolicy {
    ObservationPolicy::new(&EventKind::ALL, ObservationState::Fresh).unwrap()
}
fn finish(job: &mut CommitJob<'_>) -> CommitProgress {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let progress = job.poll();
        if !matches!(
            progress,
            CommitProgress::Pending | CommitProgress::Backpressure
        ) {
            return progress;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
