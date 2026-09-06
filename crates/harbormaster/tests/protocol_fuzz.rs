//! Deterministic seeded hostile-frame/property target; no provider or harness calls.
use harbormaster::protocol::{
    MAX_FRAME_BYTES, ProtocolError, encode_frame, parse_control, parse_control_handshake,
    parse_event, parse_event_handshake, parse_response,
};

const SEED: u64 = 0x4841_5242_4f52_0008;
const ITERATIONS: usize = 12_000;

fn corpus() -> [Vec<u8>; 5] {
    [
        r#"{"protocol":0,"channel":"event","event_id":"11111111-1111-4111-8111-111111111111","producer_id":"22222222-2222-4222-8222-222222222222","generation":"33333333-3333-4333-8333-333333333333","seq":"18446744073709551615","run_id":"44444444-4444-4444-8444-444444444444","kind":"turn.started","payload":{"turn_id":"fixture-é-😀"}}"#,
        r#"{"protocol":0,"channel":"control","request_id":"55555555-5555-4555-8555-555555555555","operation":"snapshot","arguments":{"limit":100}}"#,
        r#"{"protocol":0,"channel":"event","operation":"negotiate","producer_id":"22222222-2222-4222-8222-222222222222","generation":"33333333-3333-4333-8333-333333333333","run_id":"44444444-4444-4444-8444-444444444444","harness":"codex","requested_signals":["turn.started"]}"#,
        r#"{"protocol":0,"channel":"control","operation":"negotiate","requested_operations":["snapshot"]}"#,
        r#"{"protocol":0,"request_id":"55555555-5555-4555-8555-555555555555","status":"accepted_pending"}"#,
    ].map(|json| format!("{json}\n").into_bytes())
}

fn canonical(kind: usize, frame: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    match kind {
        0 => encode_frame(&parse_event(frame)?),
        1 => encode_frame(&parse_control(frame)?),
        2 => encode_frame(&parse_event_handshake(frame)?),
        3 => encode_frame(&parse_control_handshake(frame)?),
        4 => encode_frame(&parse_response(frame)?),
        _ => unreachable!("fixed fixture index"),
    }
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn index(state: &mut u64, length: usize) -> usize {
    usize::try_from(next(state) % u64::try_from(length).unwrap()).unwrap()
}

fn mutate(frame: &mut Vec<u8>, state: &mut u64, mode: usize) {
    let offset = index(state, frame.len());
    match mode {
        0 => {} // Actual valid acceptance/roundtrip coverage in every seeded run.
        1 => {
            frame[offset] ^= 1 << index(state, 8);
        }
        2 => {
            frame.truncate(offset);
        }
        3 => {
            frame.insert(offset, u8::try_from(next(state) & 255).unwrap());
        }
        4 => {
            frame.remove(offset);
        }
        5 => {
            frame.extend_from_within(..);
        }
        6 => {
            frame.splice(offset..offset, [b'\n', b'\r', 0, 255]);
        }
        _ => {
            frame.resize(MAX_FRAME_BYTES + 1, b' ');
        }
    }
}

#[test]
fn deterministic_seeded_malformed_input_and_roundtrip_properties() {
    let fixtures = corpus();
    let mut state = SEED;
    let mut accepted = [0_usize; 5];
    let mut rejected = [0_usize; 5];
    let mut modes = [0_usize; 8];
    for _ in 0..ITERATIONS {
        let kind = index(&mut state, fixtures.len());
        let mode = index(&mut state, modes.len());
        let mut input = fixtures[kind].clone();
        mutate(&mut input, &mut state, mode);
        modes[mode] += 1;
        match canonical(kind, &input) {
            Ok(encoded) => {
                assert!(encoded.len() <= MAX_FRAME_BYTES);
                assert_eq!(canonical(kind, &encoded).unwrap(), encoded);
                accepted[kind] += 1;
            }
            Err(_) => rejected[kind] += 1,
        }
    }
    assert!(accepted.iter().all(|count| *count > 0));
    assert!(rejected.iter().all(|count| *count > 0));
    assert!(modes.iter().all(|count| *count > 0));
    assert_eq!(
        accepted.iter().sum::<usize>() + rejected.iter().sum::<usize>(),
        ITERATIONS
    );
    println!(
        "protocol_fuzz seed={SEED} iterations={ITERATIONS} accepted={accepted:?} rejected={rejected:?} mutation_modes={modes:?}"
    );
}

#[test]
fn structured_hostile_families_are_rejected_on_every_wire_entry_point() {
    let mut rejected = 0;
    for (kind, frame) in corpus().iter().enumerate() {
        let text = std::str::from_utf8(frame).unwrap();
        let variants = [
            text.replace(r#""protocol":0"#, r#""protocol":0,"protocol":0"#),
            text.replace(r#""protocol":0"#, r#""protocol":0,"protoco\u006c":0"#),
            text.replace(r#""protocol":0"#, r#""protocol":true"#),
            text.replace(r#""protocol":0"#, r#""protocol":0.0"#),
            text.replace(r#""protocol":0"#, r#""protocol":0,"unknown":1"#),
            text.replace(r#""protocol":0"#, r#""protocol":0,"unknown":1e999"#),
            text.replace(r#""protocol":0"#, r#""protocol":0,"unknown":NaN"#),
            text.replace(r#""protocol":0"#, r#""protocol":0,"unknown":Infinity"#),
            text.replace(
                r#""protocol":0"#,
                r#""protocol":0,"unknown":[[[[[[[[0]]]]]]]]"#,
            ),
            text.trim_end().to_owned(),
            format!("{text}{}\n", "{}"),
            text.replace(
                r#""protocol":0"#,
                r#""protocol":0,"unknown":{"name":1,"na\u006de":2}"#,
            ),
        ];
        for variant in variants {
            assert!(canonical(kind, variant.as_bytes()).is_err());
            rejected += 1;
        }
        let mut exact = frame[..frame.len() - 1].to_vec();
        exact.resize(MAX_FRAME_BYTES - 1, b' ');
        exact.push(b'\n');
        assert!(canonical(kind, &exact).is_ok());
        exact.insert(0, b' ');
        assert!(canonical(kind, &exact).is_err());
    }
    assert_eq!(rejected, 60);
    println!(
        "protocol_fuzz structured_families=12 structured_rejected={rejected} exact_limit_accepted=5 over_limit_rejected=5"
    );
}
