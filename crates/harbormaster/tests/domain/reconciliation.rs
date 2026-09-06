use super::common::*;
use harbormaster::protocol::HarnessKind;

fn proof() -> ReconciliationEvidence {
    let scope = ReconciliationScope {
        producer_id: key("T").producer_id,
        previous_generation: Some(key("T").generation),
        run_id: key("T").run_id,
        harness: HarnessKind::Hermes,
    };
    let runtime = RuntimeIdentity {
        boot_id: [1; 16],
        pid: 19,
        start_ticks: 81,
    };
    ReconciliationEvidence {
        expected_scope: scope.clone(),
        observed_scope: scope,
        expected_runtime: runtime,
        observed_runtime: runtime,
        current_boot: [1; 16],
        process: ProcessState::Alive,
        freshness: ObservationState::Fresh,
        current_turn: CurrentTurnEvidence::Known {
            turn_id: turn("T").turn_id,
            state: TurnState::Working,
        },
    }
}

#[test]
fn independently_matched_current_turn_produces_exact_baseline() {
    for turn_state in [
        TurnState::Working,
        TurnState::AwaitingInput,
        TurnState::AwaitingApproval,
    ] {
        let mut evidence = proof();
        evidence.current_turn = CurrentTurnEvidence::Known {
            turn_id: turn("T").turn_id,
            state: turn_state,
        };
        let result = reconcile_projection(&evidence).unwrap();
        assert_eq!(result.turn, turn_state);
        assert_eq!(result.turn_id, Some(turn("T").turn_id));
        assert_eq!(result.run_id, evidence.expected_scope.run_id);
    }
}

#[test]
fn verified_no_current_turn_is_unknown_instead_of_invented_working_or_completion() {
    let mut evidence = proof();
    evidence.current_turn = CurrentTurnEvidence::KnownNone;
    let result = reconcile_projection(&evidence).unwrap();
    assert_eq!(result.turn, TurnState::Unknown);
    assert_eq!(result.turn_id, None);
}

#[test]
fn same_pid_wrong_boot_start_or_scope_never_activates() {
    let mut variants = Vec::new();
    let mut evidence = proof();
    evidence.observed_runtime.start_ticks += 1;
    variants.push(evidence);
    let mut evidence = proof();
    evidence.observed_runtime.boot_id = [2; 16];
    variants.push(evidence);
    let mut evidence = proof();
    evidence.current_boot = [2; 16];
    variants.push(evidence);
    let mut evidence = proof();
    evidence.observed_scope.harness = HarnessKind::Codex;
    variants.push(evidence);
    let mut evidence = proof();
    evidence.observed_scope.previous_generation = None;
    variants.push(evidence);
    let mut evidence = proof();
    evidence.observed_scope.run_id = "00000000-0000-4000-8000-000000000099".parse().unwrap();
    variants.push(evidence);
    for evidence in variants {
        assert!(reconcile_projection(&evidence).is_err());
    }
}

#[test]
fn missing_or_ambiguous_turn_and_liveness_cannot_activate() {
    for turn in [
        CurrentTurnEvidence::Unavailable,
        CurrentTurnEvidence::Conflicting,
        CurrentTurnEvidence::Known {
            turn_id: turn("T").turn_id,
            state: TurnState::Unknown,
        },
        CurrentTurnEvidence::Known {
            turn_id: turn("T").turn_id,
            state: TurnState::Completed,
        },
    ] {
        let mut evidence = proof();
        evidence.current_turn = turn;
        assert!(reconcile_projection(&evidence).is_err());
    }
    for process in [
        ProcessState::Unknown,
        ProcessState::Exited,
        ProcessState::Zombie,
    ] {
        let mut evidence = proof();
        evidence.process = process;
        assert!(reconcile_projection(&evidence).is_err());
    }
    for freshness in [
        ObservationState::Stale,
        ObservationState::Disconnected,
        ObservationState::Unsupported,
    ] {
        let mut evidence = proof();
        evidence.freshness = freshness;
        assert!(reconcile_projection(&evidence).is_err());
    }
}

#[test]
fn zero_process_identity_components_are_not_evidence() {
    let mut evidence = proof();
    for runtime in [
        RuntimeIdentity {
            pid: 0,
            ..evidence.expected_runtime
        },
        RuntimeIdentity {
            start_ticks: 0,
            ..evidence.expected_runtime
        },
        RuntimeIdentity {
            boot_id: [0; 16],
            ..evidence.expected_runtime
        },
    ] {
        evidence.expected_runtime = runtime;
        evidence.observed_runtime = runtime;
        evidence.current_boot = runtime.boot_id;
        assert!(reconcile_projection(&evidence).is_err());
    }
}
