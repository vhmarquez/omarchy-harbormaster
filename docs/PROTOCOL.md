# Protocol/domain v0 — contract and M1 wire boundary

Issue [#6](https://github.com/vhmarquez/omarchy-harbormaster/issues/6). M1 #8 implements the strict typed wire/admission library described in [M1-8-PROTOCOL.md](M1-8-PROTOCOL.md). Durable reducers, storage and live adapters remain later work. Limits refer to PERFORMANCE.md. Wire examples and failure scenarios are synthetic specifications, never actual integration logs.

## Identity types

| Type | Representation | Meaning |
|---|---|---|
| ProjectId, TaskId, RunId, ProducerId, EventId, RequestId | canonical lowercase UUID strings | Opaque manager-issued keys; never infer identity from task/title/path |
| HarnessKind | `hermes`, `claude`, `codex` | Built-in adapter kind, not lifecycle state |
| HarnessSessionId | optional bounded opaque string, max 256 UTF-8 bytes | Harness conversation identity; no universal assumed format |
| TurnId | bounded adapter-scoped opaque string | A turn inside a conversation; scoped by run and producer generation |
| ProducerGeneration | UUID | Reinstallation/reconnect generation; changed only through a trusted handshake |
| Seq | canonical nonnegative decimal string, up to unsigned 64-bit maximum | Avoid JSON/JavaScript floating-point rounding; checked parsing before ordering |
| Revision | opaque decimal string | Committed projection revision; not a wall-clock timestamp |
| ProcessIdentity | boot ID, PID, kernel start ticks, optional open pidfd | PID alone never authorizes liveness/control |
| WindowIdentity | compositor connection generation, verified window handle, owned runtime association | A title/class alone is insufficient to control a stale/reused window |

A task can own multiple conversations/runs. A conversation may have multiple sequential runs. A profile is a user configuration identity; a task never creates one automatically. Parent/child links are optional verified adapter facts; unverified relationships remain absent.

## Orthogonal state records

- Ownership: managed with immutable launch intent/runtime proof, or discovered. Changing a display label does not confer ownership.
- Process: unknown | alive | exited | zombie. A zombie is not a functioning agent and cannot be a resumable live target.
- Observation: fresh | stale | disconnected | unsupported. Last observation is evidence age, not process uptime.
- Turn: unknown | working | awaiting_input | awaiting_approval | completed | failed | interrupted. Turn outcome never directly marks task reviewed.
- Attention: reason set with event/outcome IDs, optional snooze expiry and independent seen timestamps. Reasons include user input, native approval, failure, connection uncertainty and unreviewed completion. Views may partition common reasons without losing combined status.
- Review: unreviewed | reviewed, scoped to a specific completed outcome revision. New outcomes return to unreviewed; opening project/terminal does not review anything. Further work is an explicit supported native action, not automatically submitted text.
- Delivery: pending | attempted_uncertain | acknowledged | expired | overflowed. `acknowledged` is a notification service acknowledgment, never proof a human saw it.
- Capability: per-operation supported/unsupported/unknown, source, tested version, reason and observation freshness. State controls require both capability and current target ownership/identity proof.

## Transport

Use newline-delimited UTF-8 JSON on separate local event/control sockets. Each frame is a strict object, protocol major 0, bounded before parsing, no duplicate keys, non-finite values, unknown structural fields or trailing garbage. Validate byte length, depth, exact types, enum and identifier format before lookup. Required fields never receive permissive defaults. Disconnect overlong streams before unbounded buffering. Invalid JSON cannot mutate a registry or launch a process.

Event producer handshake binds a manager-issued producer ID and generation to a run, harness, permitted event kinds and peer credentials. This limits accidental/confused producers, not malicious arbitrary code already running as the same UID. Tokens never go into process titles or evidence. Reconnection with an unknown generation requires explicit registration/reconciliation; a random generation must not reset replay defenses.

Common event envelope:

    {"protocol":0,"channel":"event","event_id":"11111111-1111-4111-8111-111111111111","producer_id":"22222222-2222-4222-8222-222222222222","generation":"33333333-3333-4333-8333-333333333333","seq":"1","run_id":"44444444-4444-4444-8444-444444444444","kind":"turn.started","payload":{"turn_id":"fixture-turn-1"}}

Allowlisted event kinds: run.started, run.exited, turn.started, turn.awaiting_input, turn.awaiting_approval, turn.completed, turn.failed, turn.interrupted and producer.health. Common envelope has no prompt, response, terminal output, environment, command line, credentials, path or user task label. Event kind payloads are distinct tagged structures: turn events need TurnId; failures have a bounded enumerated reason (not arbitrary error output); run.exited has a signed 32-bit exit code or bounded known termination reason; health declares adapter version and available supported signals, never the entire environment. Unknown events/versions are rejected with a machine-readable code and degrade only the producer, not delete its history.

Control request envelope: protocol, channel=control, request_id, operation, expected_revision where stateful, and operation-specific arguments. Allowed operations are snapshot, subscribe, project.register, launch, focus, attach, resume, interrupt_owned, end_owned, review.mark, attention.snooze and policy.update. Event credentials never grant control; no approve/allow-all operation exists.

Launch/resume require a unique RequestId and durable normalized intent. Retrying the same ID/intent returns the original result; same ID/different intent is a conflict. A separate new RequestId does not authorize duplicate resume of a still-live conversation. State-changing requests reject stale revisions/identity instead of silently choosing another session. UI double activation is coalesced; daemon remains authoritative.

Response envelope: protocol, request_id, status=ok|error|accepted_pending, committed_revision when available, typed result or error code. Allowed errors include unsupported_version, invalid_frame, unknown_producer, stale_generation, sequence_gap, capability_unavailable, stale_target, permission_denied, conflict, resource_exhausted, persistence_unavailable. Arbitrary exceptions/command output are not exposed. An accepted_pending response does not mean a runtime launched or a change was committed.

## Ordering and recovery

Deduplicate by producer/generation/EventId and sequence; the same sequence with differing content is a conflict, not a last-writer-wins update. A sequence gap marks limited freshness and requests reconciliation. Never invent lost lifecycle transitions. Terminal facts/tombstones for a turn dominate delayed start events even if the late callback has a greater transport sequence. Out-of-order observations may be retained without reviving a terminal turn. A genuinely new TurnId can start new work.

Tombstone expiry is not permission to forget replay protection while continuing the same generation. Before evicting any still-authoritative tombstone due to age or capacity, commit retirement of its producer generation atomically with eviction. Only currently registered active generations may submit lifecycle events, including after restart; retired or unknown generations remain rejected even when their detailed tombstones are gone. Keep this bounded by the active-generation registry, not an indefinitely growing retired-generation denylist. If the retirement transaction fails, retain protection and report resource/persistence failure rather than accept unprotected events.

Resuming observation requires a trusted handshake issuing a fresh manager generation, independently verified run/process identity and authoritative current-turn reconciliation before accepting new lifecycle events. Never re-register an old generation, retag queued callbacks/spooled events into the new one, or infer a new turn from a greater sequence alone. Discarded old-generation backlog is visibly accounted as a gap. If current-turn evidence is unavailable or ambiguous, remain stale/unknown and do not declare working until reconciliation proves current work or a genuinely new turn. Rotation does not clear terminal outcomes, unread review or outbox intent; those retain their separate policies. M1 must fix finite tombstone cap/TTL values and test limit-triggered retirement, restart at the transaction boundary and delayed events before and after reconciliation.

Accepted durable facts, projection changes and outbox intent commit in one transaction. Only then send durable acknowledgment. During failure, bounded metadata spool/drop and truthful stale state replace fake success. Restart reconciliation checks runner identity independently of the observer. Reboot marks previous boot's processes unavailable; only saved context can support a new Resume run.

Subscribe begins with a revision-stamped snapshot (paginated at one consistent snapshot revision), then ordered deltas. Expired cursor, sequence discontinuity or slow-client overflow forces a new snapshot with explicit resync status. UI keeps selected RunId where present, not row index; removing it clears target controls or chooses a visibly indicated neighboring selection, never silently applies an old command to the new row.

## Persistence and privacy

One SQLite writer owns durable decisions. Read-only diagnostics/status must not mutate policy, archive sessions or load transcripts. User task labels/project paths are private metadata accepted only through explicit control flows, not event producers. Retention of history, tombstones, attention and pending notifications has separate bounded policies. Exports are opt-in and sanitized; presentation masking is neither deletion nor consent to collect.

The event allowlist provides field-level minimization, not proof of secrecy: opaque identifiers and adapter-version strings can contain sensitive text if an adapter maps the wrong source field. String types, length limits and the absence of prompt/response field names cannot establish content absence. Before enabling an adapter's persistence path, review and test a version-specific allowlist of source-field provenance and mappings: identifiers/version metadata must come from documented metadata sources, never prompt, response, terminal/error output or environment fallbacks. Unproven optional mappings are omitted; unproven required mappings reject/degrade the signal. Never spool or log the raw source payload as a fallback.

Required adapter canary tests inject synthetic content/credential markers into excluded source fields and deliberately mis-map them into accepted string fields. Verify rejection or omission before persistence, then scan spool files, temporaries, database/WAL, logs and recovery/diagnostic output across crash/cleanup paths. Passing tests establish behavior for the tested mappings and versions, not that string schemas prove secrecy or that same-UID malicious code is isolated. These are M1+ implementation gates, not executed privacy evidence in M0.

Machine-readable failure scenarios live in `contracts/failure-fixtures.json`. Schema version is exactly integer `1`; all 14 currently specified scenarios are required and their individual deletion is checked. They specify setup, operation, expected outcome, invariant and future proof mechanism. They are requirements to execute in M1+, not a reducer implementation or claims that those failures are fixed today.
