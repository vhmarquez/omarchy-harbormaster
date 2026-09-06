pub use harbormaster::domain::*;
pub use harbormaster::protocol::{EventEnvelope, EventKind, EventPayload, Seq, TurnPayload};

pub fn event(payload: EventPayload) -> EventEnvelope {
    EventEnvelope {
        event_id: "00000000-0000-4000-8000-000000000001".parse().unwrap(),
        producer_id: "00000000-0000-4000-8000-000000000002".parse().unwrap(),
        generation: "00000000-0000-4000-8000-000000000003".parse().unwrap(),
        seq: Seq::new(1),
        run_id: "00000000-0000-4000-8000-000000000004".parse().unwrap(),
        event: payload,
    }
}

pub fn turn(id: &str) -> TurnPayload {
    TurnPayload {
        turn_id: id.parse().unwrap(),
    }
}

pub fn key(id: &str) -> TurnKey {
    let event = event(EventPayload::TurnStarted(turn(id)));
    TurnKey {
        producer_id: event.producer_id,
        generation: event.generation,
        run_id: event.run_id,
        turn_id: id.parse().unwrap(),
    }
}

pub fn state(id: Option<&str>, turn: TurnState) -> ReductionState {
    ReductionState {
        projection: RunProjection {
            run_id: key("T").run_id,
            process: ProcessState::Alive,
            observation: ObservationState::Fresh,
            turn,
            turn_id: id.map(|id| id.parse().unwrap()),
        },
        current_turn: id.map(key),
        tombstone: None,
    }
}

pub fn policy() -> ObservationPolicy {
    ObservationPolicy::new(&EventKind::ALL, ObservationState::Fresh).unwrap()
}

pub fn terminal(state: &mut ReductionState, id: &str, kind: Option<TurnState>) {
    state.tombstone = Some(TombstoneEvidence {
        key: key(id),
        outcome_id: "00000000-0000-4000-8000-000000000009".parse().unwrap(),
        terminal: kind,
    });
}
