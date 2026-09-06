use harbormaster::protocol::{ProducerGeneration, Revision, parse_event};
use harbormaster::storage::{
    AttentionMutation, AttentionReason, ObservationState, OutboxIntent, OutcomeKey, ProcessState,
    RunProjection, TerminalTombstone, TurnState, WriteSet,
};
pub const PRODUCER: &str = "22222222-2222-4222-8222-222222222222";
pub const RUN: &str = "44444444-4444-4444-8444-444444444444";
pub const EVENT: &str = "11111111-1111-4111-8111-111111111111";

pub fn write_set(generation: &ProducerGeneration, revision: Revision, sequence: u64) -> WriteSet {
    let fact = parse_event(format!(r#"{{"protocol":0,"channel":"event","event_id":"{EVENT}","producer_id":"{PRODUCER}","generation":"{generation}","seq":"{sequence}","run_id":"{RUN}","kind":"turn.completed","payload":{{"turn_id":"fixture-turn"}}}}
"#).as_bytes()).unwrap();
    WriteSet {
        expected_revision: revision,
        fact,
        projection: RunProjection {
            run_id: RUN.parse().unwrap(),
            process: ProcessState::Alive,
            observation: ObservationState::Fresh,
            turn: TurnState::Completed,
            turn_id: Some("fixture-turn".parse().unwrap()),
        },
        attention: vec![AttentionMutation {
            outcome_id: EVENT.parse().unwrap(),
            reason: AttentionReason::Completion,
            reviewed: false,
        }],
        outbox: Some(OutboxIntent {
            outcome_id: EVENT.parse().unwrap(),
        }),
        tombstone: Some(TerminalTombstone {
            turn_id: "fixture-turn".parse().unwrap(),
            outcome_id: EVENT.parse().unwrap(),
        }),
        accepted_at: 100,
    }
}

pub fn key(set: &WriteSet) -> OutcomeKey {
    OutcomeKey {
        producer_id: set.fact.producer_id.clone(),
        generation: set.fact.generation.clone(),
        event_id: set.fact.event_id.clone(),
    }
}
