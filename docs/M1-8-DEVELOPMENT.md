# M1 #8 — protocol and private IPC boundaries

Recorded before production implementation on `codex/m1-8-protocol-ipc`, based on
main `70a671bf5bf93aaed7a3acbc1ad26df76fd60187`. The owner merged #49 and #7 is
closed. Fresh post-merge qualification is recorded in
[the baseline receipt](../evidence/m1-8/post-merge-main/summary.json).

## Responsibility and dependency direction

Keep the existing single Rust crate and thin help/version executable. Issue #8
adds library boundaries exercised only with disposable fixtures; it does not
enable a daemon, bridge, live harness adapter, terminal control or deployment.

- `protocol/`: validated identity and sequence types, versioned event/control
  requests and responses, capability records, strict bounded JSON decoding and
  serialization. Pure data and validation; no filesystem, transport, database,
  process control, clock or presentation dependencies.
- `ipc/`: Linux descriptor-relative private runtime paths, separate event and
  control Unix sockets, socket ownership and peer credentials, RAII ownership,
  bounded incremental newline framing and nonblocking I/O. No registry or domain
  decisions. It returns peer/channel evidence that wire fields cannot confer.
- `ingestion/`: explicit trusted registration and authorization, capability
  intersection, bounded volatile admission queues and sequence/generation
  recovery. Event credentials cannot authorize a control operation. Queue
  saturation and gaps are explicit failures/reconciliation states. It imports
  protocol types and narrowly defined transport evidence, never SQLite or QML.
- `scripts/tooling/`: explicit public pinned dependency preparation, separated
  from offline validation. New crates require locked source/checksum and registry
  yank metadata, licenses and advisory validation; missing inputs fail closed.
- Integration targets separately exercise strict hostile input/fuzzing, real
  disposable Unix sockets, and admission/recovery. Each is registered in the
  canonical required portable inventory; native bubblewrap qualification remains
  separately mandatory and unchanged.

Dependency direction: executable -> existing CLI library; ingestion -> protocol
and narrow peer/channel evidence; IPC -> protocol framing limits and safe OS
wrappers. Protocol does not import IPC, ingestion, storage, adapters or QML.
Tests may compose the boundaries; production has no mock durable completion.

Use pinned Serde/serde_json for typed JSON and rustix for safe descriptor/socket
operations. Review the actual resolved graph and preparation before use. A
reactor/runtime and launch framework are unnecessary for a library qualification
with bounded nonblocking operations; do not scaffold future daemon behavior.
Workspace `unsafe_code = forbid` remains. No new unsafe platform shim is planned.

## Admission is not persistence

The trusted manager API supplies an explicitly registered producer/run/harness,
generation, peer UID, allowed signals and authoritative sequence checkpoint.
Wire handshakes may negotiate an existing registration; they cannot create or
rotate a generation or widen permissions. Unknown/stale generations fail closed.
After restart, no previous registration is implicitly active. A gap or ambiguous
sequence requires authoritative reconciliation, not inferred current activity.

Admission and queueing never acknowledge a durable fact, update lifecycle or
attention, mark review, or create outbox intent. No `ok` durable event receipt is
sent before the future atomic persistence boundary. Old sequences are rejected
even after a bounded in-memory duplicate window expires; protocol recovery does
not implement the #10 terminal-tombstone reducer. Generation retirement and
trusted fresh registration cannot retag old queued events. #9/#10 must supply
their transactional durability/replay guarantees after their approval gates.

Control parsing is distinct from operation authorization. Explicit grants and
supported capabilities are required; peer UID or an event token alone does not
grant control. Native approval operations are absent. Runtime operations remain
unavailable until their own implementation and verified identity contracts exist.

## Fixed limits and test obligations

Preserve PROTOCOL.md and PERFORMANCE.md: frame <=16,384 bytes including newline,
depth <=8, no duplicate keys/nonfinite values/unknown fields/coerced types,
canonical UUID/decimal identities; ingress <=1,024 globally and <=128 per
producer; unsent client stream <=256 frames or 1 MiB; snapshot page <=100.
Reject partial/oversized/invalid streams with bounded work and buffering. Private
runtime paths have no shared temporary fallback. Validate every ancestor and
retain descriptors across operations; refuse existing unowned/unsafe resources.
Same-UID hostile replacement is explicitly outside a socket-token isolation
guarantee; cleanup must not remove unrelated replacement files.

Use observed RED/GREEN tests for handwritten behavior, deterministic seeded
malformed-input fuzzing plus boundary/Unicode/duplicate/ordering fixtures, actual
private Unix socket and peer credential tests inside the unchanged offline
sandbox, and slow-consumer/fairness/overflow checks. Record counts and measured
fixture durations without claiming the unmeasured product latency/RSS budgets.
Independent correctness/security/maintainability review assesses responsibilities,
coupling, public API, duplication and >300-file/>50-function nonblank-line triggers.

## Approval and later scope

Publish the focused #8 PR with actual current-revision portable Docker, separate
native and paired qualification, local combined checks and independent review.
Stop for explicit owner approval of that PR/revision before merge or dependent
#9 implementation. Green checks, review and #49's merge are not approval of #8.
No M2 work. Retention mapping and numeric tombstone TTL/cap must be resolved before
later cleanup implementation; this issue introduces no retention policy or spool.
