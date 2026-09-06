//! Recovery privacy baseline: wire typing alone cannot establish provenance.
use harbormaster::protocol::{EventEnvelope, EventPayload, Seq, TurnPayload};

#[test]
fn source_mapping_must_precede_serialization() {
    let canary = "SYNTHETIC_PROMPT_CANARY";
    let event = EventEnvelope {
        event_id: "00000000-0000-4000-8000-000000000001".parse().unwrap(),
        producer_id: "00000000-0000-4000-8000-000000000002".parse().unwrap(),
        generation: "00000000-0000-4000-8000-000000000003".parse().unwrap(),
        run_id: "00000000-0000-4000-8000-000000000004".parse().unwrap(),
        seq: Seq::new(1),
        event: EventPayload::TurnStarted(TurnPayload { turn_id: canary.parse().unwrap() }),
    };
    let serialized = serde_json::to_string(&event).unwrap();
    assert!(!serialized.contains(canary), "baseline schema accepts content mis-mapped into an opaque ID");
}
