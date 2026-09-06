use harbormaster::protocol::{
    EventKind, EventPayload, ProtocolError, Seq, encode_frame, parse_event,
};

const EVENT: &str = concat!(
    r#"{"protocol":0,"channel":"event","event_id":"11111111-1111-4111-8111-111111111111","producer_id":"22222222-2222-4222-8222-222222222222","generation":"33333333-3333-4333-8333-333333333333","seq":"1","run_id":"44444444-4444-4444-8444-444444444444","kind":"turn.started","payload":{"turn_id":"fixture-turn-1"}}"#,
    "\n"
);

#[test]
fn accepted_m0_example_is_typed_and_roundtrips() {
    let event = parse_event(EVENT.as_bytes()).expect("M0 example");
    assert_eq!(event.event.kind(), EventKind::TurnStarted);
    assert!(matches!(event.event, EventPayload::TurnStarted(_)));
    assert_eq!(event.seq, Seq::new(1));
    assert_eq!(parse_event(&encode_frame(&event).unwrap()).unwrap(), event);
}

#[test]
fn duplicate_decoded_keys_fail_before_typed_decoding() {
    for duplicate in [
        r#""protocol":0,"protocol":0"#,
        r#""protocol":0,"protoco\u006c":0"#,
    ] {
        let frame = EVENT.replacen(r#""protocol":0"#, duplicate, 1);
        assert_eq!(
            parse_event(frame.as_bytes()),
            Err(ProtocolError::InvalidFrame)
        );
    }
}

#[test]
fn malformed_frame_never_returns_a_typed_event() {
    for frame in [
        EVENT.trim_end().to_owned(),
        format!("{EVENT}{EVENT}"),
        EVENT.replace(r#""seq":"1""#, r#""seq":1"#),
        EVENT.replace(r#""seq":"1""#, r#""seq":"01""#),
        EVENT.replace(r#""seq":"1""#, r#""seq":"18446744073709551616""#),
        EVENT.replace(r#""protocol":0"#, r#""protocol":0.0"#),
        EVENT.replace(
            r#""turn_id":"fixture-turn-1""#,
            r#""turn_id":"fixture-turn-1","prompt":"private""#,
        ),
        EVENT.replace(r#""turn_id":"fixture-turn-1""#, r#""turn_id":NaN"#),
    ] {
        assert_eq!(
            parse_event(frame.as_bytes()),
            Err(ProtocolError::InvalidFrame)
        );
    }
}

#[test]
fn framing_boundaries_include_lf_and_reject_embedded_lines() {
    use harbormaster::protocol::MAX_FRAME_BYTES;
    let exact = format!(
        "{}{}\n",
        EVENT.trim_end(),
        " ".repeat(MAX_FRAME_BYTES - EVENT.len())
    );
    assert_eq!(exact.len(), MAX_FRAME_BYTES);
    assert!(parse_event(exact.as_bytes()).is_ok());
    assert_eq!(
        parse_event(format!(" {exact}").as_bytes()),
        Err(ProtocolError::InvalidFrame)
    );
    for invalid in ["", "\n", "null\n", "[]\n", "{}\n", "{}true\n", "{}\r\n"] {
        assert!(parse_event(invalid.as_bytes()).is_err());
    }
    assert!(parse_event(EVENT.replace(',', ",\n").as_bytes()).is_err());
    assert!(parse_event(EVENT.replace(',', ",\r").as_bytes()).is_err());
    let mut invalid_utf8 = EVENT.as_bytes().to_vec();
    invalid_utf8[1] = 0xff;
    assert!(parse_event(&invalid_utf8).is_err());
}

#[test]
fn uuid_and_decimal_newtypes_are_strict_at_their_boundaries() {
    use harbormaster::protocol::{EventId, ProducerGeneration, Revision, RunId};
    for invalid in [
        "",
        "1",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "{11111111-1111-4111-8111-111111111111}",
        "11111111-1111-4111-8111-11111111111z",
    ] {
        assert!(invalid.parse::<EventId>().is_err());
        assert!(invalid.parse::<ProducerGeneration>().is_err());
        assert!(invalid.parse::<RunId>().is_err());
    }
    for invalid in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e1",
        " 1",
        "1 ",
        "١",
        "18446744073709551616",
    ] {
        assert!(invalid.parse::<Seq>().is_err());
        assert!(invalid.parse::<Revision>().is_err());
    }
    for value in [0, 1, u64::MAX] {
        assert_eq!(value.to_string().parse::<Seq>().unwrap().value(), value);
    }
    assert_eq!(Seq::new(u64::MAX).checked_next(), None);
    assert_eq!(Seq::new(0).checked_next(), Some(Seq::new(1)));
}

#[test]
fn unknown_versions_and_errors_do_not_echo_private_input() {
    let bad = EVENT.replace(r#""protocol":0"#, r#""protocol":1"#);
    assert_eq!(
        parse_event(bad.as_bytes()),
        Err(ProtocolError::UnsupportedVersion)
    );
    let bad = EVENT.replace("fixture-turn-1", &"private".repeat(100));
    assert_eq!(
        parse_event(bad.as_bytes()).unwrap_err().to_string(),
        "invalid_frame"
    );
}

fn event_kind(kind: &str, payload: &str) -> String {
    EVENT
        .replace(r#""kind":"turn.started""#, &format!(r#""kind":"{kind}""#))
        .replace(r#"{"turn_id":"fixture-turn-1"}"#, payload)
}

#[test]
fn every_event_kind_has_a_distinct_allowlisted_payload() {
    let cases = [
        ("run.started", "{}"),
        ("run.started", r#"{"harness_session_id":"conversation-1"}"#),
        ("run.exited", r#"{"exit_code":-2147483648}"#),
        ("run.exited", r#"{"exit_code":2147483647}"#),
        ("run.exited", r#"{"reason":"signaled"}"#),
        ("turn.started", r#"{"turn_id":"one"}"#),
        ("turn.awaiting_input", r#"{"turn_id":"one"}"#),
        ("turn.awaiting_approval", r#"{"turn_id":"one"}"#),
        ("turn.completed", r#"{"turn_id":"one"}"#),
        (
            "turn.failed",
            r#"{"turn_id":"one","reason":"harness_error"}"#,
        ),
        ("turn.interrupted", r#"{"turn_id":"one"}"#),
        (
            "producer.health",
            r#"{"adapter_version":"fixture-1","signals":["turn.started"]}"#,
        ),
    ];
    for (kind, payload) in cases {
        let event = parse_event(event_kind(kind, payload).as_bytes()).unwrap();
        assert_eq!(parse_event(&encode_frame(&event).unwrap()).unwrap(), event);
    }
}

#[test]
fn mismatched_event_payloads_and_unknown_metadata_are_rejected() {
    for (kind, payload) in [
        ("turn.failed", r#"{"turn_id":"one"}"#),
        (
            "turn.failed",
            r#"{"turn_id":"one","reason":"arbitrary private failure"}"#,
        ),
        ("turn.completed", r#"{"turn_id":"one","reason":"unknown"}"#),
        ("run.exited", r#"{"exit_code":2147483648}"#),
        ("run.exited", r#"{"exit_code":-2147483649}"#),
        ("run.exited", r#"{"exit_code":0.0}"#),
        ("run.exited", r#"{"exit_code":"0"}"#),
        ("run.exited", r#"{"exit_code":0,"reason":"signaled"}"#),
        ("run.started", r#"{"command":"secret"}"#),
        (
            "producer.health",
            r#"{"adapter_version":"fixture","signals":["turn.started","turn.started"]}"#,
        ),
        (
            "producer.health",
            r#"{"adapter_version":"fixture","signals":["prompt"]}"#,
        ),
        ("turn.started", r#"{"turn_id":"one","turn_\u0069d":"two"}"#),
    ] {
        assert!(
            parse_event(event_kind(kind, payload).as_bytes()).is_err(),
            "{kind} {payload}"
        );
    }
}

#[test]
fn metadata_limits_are_utf8_bytes_and_reject_control_characters() {
    use harbormaster::protocol::{AdapterVersion, HarnessSessionId, LocalPath, TaskLabel, TurnId};
    assert!("é".repeat(128).parse::<TurnId>().is_ok());
    assert!("é".repeat(129).parse::<TurnId>().is_err());
    assert!("x".repeat(256).parse::<HarnessSessionId>().is_ok());
    assert!("x".repeat(257).parse::<HarnessSessionId>().is_err());
    assert!("x".repeat(64).parse::<AdapterVersion>().is_ok());
    assert!("x".repeat(65).parse::<AdapterVersion>().is_err());
    assert!("x".repeat(1024).parse::<TaskLabel>().is_ok());
    assert!("x".repeat(1025).parse::<TaskLabel>().is_err());
    assert!(
        format!("/{}", "x".repeat(4095))
            .parse::<LocalPath>()
            .is_ok()
    );
    assert!(
        format!("/{}", "x".repeat(4096))
            .parse::<LocalPath>()
            .is_err()
    );
    assert!("relative/path".parse::<LocalPath>().is_err());
    for text in ["", "turn\0id", "turn\nid", "turn\u{1b}id"] {
        assert!(text.parse::<TurnId>().is_err());
    }
}

fn control(operation: &str, guard: &str, args: &str) -> String {
    format!(
        r#"{{"protocol":0,"channel":"control","request_id":"55555555-5555-4555-8555-555555555555","operation":"{operation}"{guard},"arguments":{args}}}"#
    ) + "\n"
}

#[test]
fn stateful_requests_require_exact_guards_and_typed_arguments() {
    use harbormaster::protocol::{Revision, parse_control};
    let guard = r#", "expected_revision":"4""#;
    let run = r#"{"run_id":"44444444-4444-4444-8444-444444444444"}"#;
    for operation in ["focus", "attach", "resume", "interrupt_owned", "end_owned"] {
        let frame = control(operation, guard, run);
        let request = parse_control(frame.as_bytes()).unwrap();
        assert_eq!(request.expected_revision(), Some(Revision::new(4)));
        assert_eq!(
            parse_control(&encode_frame(&request).unwrap()).unwrap(),
            request
        );
        for bad_guard in [
            "",
            r#", "expected_revision":null"#,
            r#", "expected_revision":4"#,
            r#", "expected_revision":"04""#,
        ] {
            assert!(parse_control(control(operation, bad_guard, run).as_bytes()).is_err());
        }
        assert!(parse_control(control(operation, guard, "{}").as_bytes()).is_err());
    }
    for (operation, args) in [
        (
            "project.register",
            r#"{"path":"/fixture/project","label":"fixture"}"#,
        ),
        (
            "launch",
            r#"{"project_id":"11111111-1111-4111-8111-111111111111","harness":"codex"}"#,
        ),
        (
            "review.mark",
            r#"{"run_id":"44444444-4444-4444-8444-444444444444","outcome_revision":"3"}"#,
        ),
        (
            "attention.snooze",
            r#"{"run_id":"44444444-4444-4444-8444-444444444444","until_unix_seconds":"400"}"#,
        ),
        ("policy.update", r#"{"notifications_enabled":false}"#),
    ] {
        assert!(parse_control(control(operation, guard, args).as_bytes()).is_ok());
        assert!(parse_control(control(operation, "", args).as_bytes()).is_err());
    }
}

#[test]
fn read_only_operations_reject_guards_and_enforce_snapshot_limits() {
    use harbormaster::protocol::{Operation, parse_control};
    let guard = r#", "expected_revision":"4""#;
    let request = parse_control(control("snapshot", "", r#"{"limit":100}"#).as_bytes()).unwrap();
    assert_eq!(request.operation(), Operation::Snapshot);
    assert_eq!(request.expected_revision(), None);
    for limit in ["0", "101", "true", "1.0", "\"1\"", "null"] {
        assert!(
            parse_control(control("snapshot", "", &format!(r#"{{"limit":{limit}}}"#)).as_bytes())
                .is_err()
        );
    }
    assert!(parse_control(control("snapshot", guard, r#"{"limit":1}"#).as_bytes()).is_err());
    assert!(
        parse_control(control("subscribe", "", r#"{"after_revision":"0"}"#).as_bytes()).is_ok()
    );
}

#[test]
fn event_and_control_channels_never_cross_and_native_approval_is_absent() {
    use harbormaster::protocol::parse_control;
    assert!(parse_control(EVENT.as_bytes()).is_err());
    assert!(parse_event(control("snapshot", "", r#"{"limit":1}"#).as_bytes()).is_err());
    for operation in ["approve", "allow_all", "approve_owned", "shell", "delete"] {
        assert!(
            parse_control(control(operation, r#", "expected_revision":"0""#, "{}").as_bytes())
                .is_err()
        );
    }
    let unknown = control(
        "policy.update",
        r#", "expected_revision":"0""#,
        r#"{"notifications_enabled":true,"retention_days":90}"#,
    );
    assert!(parse_control(unknown.as_bytes()).is_err());
}

#[test]
fn capability_handshakes_are_bounded_claims_and_preserve_identity() {
    use harbormaster::protocol::{parse_control_handshake, parse_event_handshake};
    let frame = concat!(
        r#"{"protocol":0,"channel":"event","operation":"negotiate","producer_id":"22222222-2222-4222-8222-222222222222","generation":"33333333-3333-4333-8333-333333333333","run_id":"44444444-4444-4444-8444-444444444444","harness":"codex","requested_signals":["turn.started","turn.completed"]}"#,
        "\n"
    );
    let claim = parse_event_handshake(frame.as_bytes()).unwrap();
    assert_eq!(
        claim.requested_signals,
        vec![EventKind::TurnStarted, EventKind::TurnCompleted]
    );
    assert_eq!(
        parse_event_handshake(&encode_frame(&claim).unwrap()).unwrap(),
        claim
    );
    assert!(
        parse_event_handshake(frame.replace("turn.completed", "turn.started").as_bytes()).is_err()
    );
    assert!(
        parse_event_handshake(frame.replace("codex", "unregistered_harness").as_bytes()).is_err()
    );
    assert!(parse_event(frame.as_bytes()).is_err());
    let control = concat!(
        r#"{"protocol":0,"channel":"control","operation":"negotiate","requested_operations":["snapshot","subscribe"]}"#,
        "\n"
    );
    let claim = parse_control_handshake(control.as_bytes()).unwrap();
    assert_eq!(
        parse_control_handshake(&encode_frame(&claim).unwrap()).unwrap(),
        claim
    );
    assert!(parse_control_handshake(control.replace("subscribe", "snapshot").as_bytes()).is_err());
    assert!(parse_control_handshake(control.replace("subscribe", "approve").as_bytes()).is_err());
}

#[test]
fn responses_have_explicit_typed_status_without_arbitrary_exception_fields() {
    use harbormaster::protocol::parse_response;
    let prefix = r#"{"protocol":0,"request_id":"55555555-5555-4555-8555-555555555555","status":"#;
    for suffix in [
        r#""error","error":"permission_denied"}"#,
        r#""accepted_pending"}"#,
        r#""ok","committed_revision":"1","result":{"kind":"resync_required","snapshot_revision":"1"}}"#,
        r#""ok","committed_revision":null,"result":{"kind":"capabilities","capabilities":[{"operation":"snapshot","status":"supported","source":"manager","tested_version":"fixture","reason":"available","freshness":"fresh"}]}}"#,
    ] {
        let response = parse_response(format!("{prefix}{suffix}\n").as_bytes()).unwrap();
        assert_eq!(
            parse_response(&encode_frame(&response).unwrap()).unwrap(),
            response
        );
    }
    for suffix in [
        r#""error","error":"arbitrary exception output"}"#,
        r#""error","error":"conflict","exception":"private"}"#,
        r#""accepted_pending","committed_revision":"1"}"#,
        r#""accepted_pending","result":{}}"#,
        r#""ok","result":{"kind":"runtime_launched"}}"#,
    ] {
        assert!(
            parse_response(format!("{prefix}{suffix}\n").as_bytes()).is_err(),
            "{suffix}"
        );
    }
}
