use super::*;
use crate::{protocol::*, storage::*};
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

mod bounds;
mod catalog;
mod recovery;
mod reducer;
mod reducer_attention;
mod reducer_continuity;
mod reducer_continuity_guards;
mod reducer_migration;
mod reducer_progress;
mod reducer_reconciliation;
mod reducer_retirement;
mod retirement_projection;
mod runners;
mod writes;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(std::env::var_os("TMPDIR").expect("isolated fixture TMPDIR"))
            .join(format!(
                "storage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }
    fn open(&self) -> Engine {
        Engine::open(&self.root).unwrap()
    }
    fn database(&self) -> PathBuf {
        self.root.join("harbormaster/state.db")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
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
fn register(engine: &mut Engine, seq: u64) -> (ProducerGeneration, Revision) {
    let expected = queries::revision(engine.connection.as_ref().unwrap()).unwrap();
    match engine
        .execute(&Request::Register(Registration {
            expected_revision: expected,
            producer_id: id(1),
            run_id: id(2),
            harness: HarnessKind::Hermes,
            next_sequence: Seq::new(seq),
            previous_generation: None,
        }))
        .unwrap()
    {
        Response::Registered {
            generation,
            revision,
        } => (generation, revision),
        _ => panic!("registration response"),
    }
}
fn write(
    generation: ProducerGeneration,
    expected_revision: Revision,
    seq: u64,
    event: u32,
) -> WriteSet {
    WriteSet {
        expected_revision,
        fact: EventEnvelope {
            producer_id: id(1),
            generation,
            event_id: id(event),
            seq: Seq::new(seq),
            run_id: id(2),
            event: EventPayload::TurnCompleted(TurnPayload {
                turn_id: "fixture-turn".parse().unwrap(),
            }),
        },
        projection: RunProjection {
            run_id: id(2),
            process: ProcessState::Alive,
            observation: ObservationState::Fresh,
            turn: TurnState::Completed,
            turn_id: Some("fixture-turn".parse().unwrap()),
        },
        attention: vec![AttentionMutation {
            outcome_id: id(event),
            reason: AttentionReason::Completion,
            reviewed: false,
        }],
        outbox: Some(OutboxIntent {
            outcome_id: id(event),
        }),
        tombstone: Some(TerminalTombstone {
            turn_id: "fixture-turn".parse().unwrap(),
            outcome_id: id(event),
        }),
        accepted_at: 100,
    }
}
fn commit(engine: &mut Engine, set: &WriteSet) -> Response {
    engine
        .execute(&Request::Commit(Box::new(set.clone())))
        .unwrap()
}
fn outcome(set: &WriteSet) -> OutcomeKey {
    OutcomeKey {
        producer_id: set.fact.producer_id.clone(),
        generation: set.fact.generation.clone(),
        event_id: set.fact.event_id.clone(),
    }
}
fn count(engine: &Engine, table: &str) -> u32 {
    engine
        .connection
        .as_ref()
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}
