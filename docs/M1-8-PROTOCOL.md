# M1 #8 executable protocol and private IPC

This issue implements library boundaries in the existing Rust crate. The CLI
still exposes help/version only. No daemon, installed socket, live harness,
database, reducer, notification, runtime action or QML presentation is enabled.
The tests create only private disposable Unix fixtures inside the unchanged
offline verifier. TCP/UDP listeners are absent.

## Wire contract

All entry points require exactly one newline-terminated UTF-8 JSON object,
protocol integer 0, at most 16,384 bytes including newline and depth 8. They
reject duplicate decoded keys, nonfinite values, trailing frames/garbage,
unknown fields and type coercions before a producer lookup or authorization.
The common strict decoder retains duplicate evidence before conversion to typed
values. Encoding uses a bounded writer. Errors contain fixed codes, never input
values or raw exception/command output.

`ProjectId`, `TaskId`, `RunId`, `ProducerId`, `EventId`, `RequestId` and
`ProducerGeneration` are distinct canonical lowercase UUID types. `Seq` and
`Revision` are canonical unsigned-64-bit decimal strings, including values above
JavaScript's exact integer range. Increment is checked; exhaustion requires
reconciliation rather than wrapping. Turn/harness IDs and snapshot cursors have
a 256-byte maximum; adapter versions 64 bytes; explicit control labels 1,024
bytes and absolute local paths 4,096 bytes. Opaque strings reject empty/control
characters. Path syntax is not filesystem approval or ownership proof.

The common event envelope and nine allowlisted kinds follow [PROTOCOL.md](PROTOCOL.md).
Payloads are distinct: run start has optional harness-session metadata; run exit
has an i32 exit code or known termination reason; turn signals require a TurnId;
failure adds only an enumerated reason; health has a bounded adapter version and
unique allowlisted signals. Content/environment/command/path/label fields are
absent. These schemas minimize metadata; opaque strings cannot prove their source
was free of sensitive content. Adapter mapping and persistence canary gates remain
mandatory before those later paths are enabled.

Negotiation uses `operation: "negotiate"` and the corresponding event/control
channel. An event handshake supplies producer ID, generation, run ID, harness and
`requested_signals`. A control handshake supplies `requested_operations`. These
bounded unique lists narrow an existing trusted grant; a request never creates
registration, changes a generation, proves ownership or grants native approval.

Control envelopes have `protocol`, `channel`, `request_id`, `operation`,
`arguments`, and `expected_revision` for every stateful operation. Read-only
requests reject an expected revision. Concrete argument schemas are:

| Operation | Arguments |
|---|---|
| snapshot | `limit` integer 1–100, optional `cursor` |
| subscribe | `after_revision` |
| project.register | `path`, `label` |
| launch | `project_id`, `harness` |
| focus, attach, resume, interrupt_owned, end_owned | `run_id` |
| review.mark | `run_id`, `outcome_revision` |
| attention.snooze | `run_id`, `until_unix_seconds` canonical decimal |
| policy.update | `notifications_enabled` boolean |

These schemas authorize no implementation of those actions. Snapshot record
projection, durable idempotency, target/process identity and action execution
belong to later approved work. No retention choice, raw command or approve
operation is introduced. Responses distinguish typed errors, explicit pending
acceptance, and capability/resync results. `accepted_pending` rejects committed
revision/result/error fields; it cannot be interpreted as durable success.

## Transport and authorization

`PrivateSockets` traverses an explicit runtime path descriptor by descriptor with
no-follow checks and validates the final directory as current-UID, mode 0700.
There is no environment-derived shared-temp fallback. Its own `harbormaster`
directory and fixed `events.sock`/`control.sock` entries are private. Existing
socket/file entries are refused, not blindly removed or adopted. Linux binding
through a held `/proc/self/fd` directory anchors the path across ancestor renames.
Socket nodes are restricted to 0600 before accepting peers. `SO_PEERCRED` is read
from the actual accepted stream with nix's raw `pid_t` representation; an unmapped
zero or negative PID is rejected before conversion. Listener channel and UID/GID/PID evidence cannot
be deserialized from JSON or replaced through a raw-stream escape hatch.

Cleanup owns only recorded socket identities, with a retained descriptor when
available. Rollback after file-descriptor exhaustion removes proven owned nodes.
Identity mismatches preserve replacements. Linux has no atomic
identity-conditional unlink: hostile concurrent same-UID replacement remains
outside this guarantee. Socket modes, credentials, UUIDs and worktrees do not
sandbox malicious code already running as the desktop user.

Transport accepts at most 64 simultaneous connections across both listeners,
including idle/handshake peers. Each poll performs bounded framing and at most
one read or write. Handshake, partial-frame and unsent-write deadlines are
independently bounded to 100 ms; a caller may tighten them. No timer/reactor is
silently installed: callers must poll fairly and invoke deadline checks. A slow
consumer cannot enlarge its queue beyond 256 frames or 1 MiB; overflow closes
that stream and requires reconnect/resnapshot. A requested 16 KiB send buffer
also bounds the configured kernel send queue (Linux accounts its own overhead).
One healthy stream remains usable while another is refused for saturation.

Event sessions require the event listener, registered peer UID/run/harness and
active generation. Negotiated signals are the intersection of registered and
requested signals, exposed through a read-only accessor. Control sessions use a
separate authorizer and real control listener; authorization rechecks explicit
grants, negotiated operation, current supported/fresh capability evidence and
expected revision. A later executor still owes target ownership and persistence
checks. Session proofs are private and bound to their creating authority.

## Ordering and recovery boundary

The admission queue is globally bounded at 1,024 frames and 128 per producer,
with round-robin producer dequeue and per-producer FIFO order. Capacity is
reserved before sequence advancement. Exact duplicates report only
`AlreadyAdmitted`, including after dequeue; they never acknowledge durable data.
Conflicting EventIds/sequences and gaps require reconciliation. Lower-than-
checkpoint sequences are rejected, and u64 exhaustion cannot wrap.

Exact volatile receipts are independently capped at 1,024 per producer, 8,192
globally and 8 MiB of accepted frame bytes. Saturation freezes the generation;
dequeue never forgets replay protection. Trusted reconciliation first issues a
fresh random UUID using one nonblocking safe rustix call, then discards/counts
old queued frames and invalidates previous session scope. The caller cannot
choose a retired UUID or relabel a backlog through this API. Entropy failure
leaves the old state intact. Trusted initial registration/checkpoint restoration
still requires authoritative run/replay knowledge; a newly constructed registry
has no implicitly active producer. Active registrations are capped at 10,000.

This is volatile protocol admission, not the #9/#10 durability or lifecycle
implementation. It performs no SQLite transaction, terminal-tombstone pruning,
spooling, review/attention update or outbox acknowledgment. Authoritative
run/current-turn reconciliation remains required before trusted rotation;
unknown wire generations never invoke that API. Numeric terminal tombstone
cap/TTL and retention mapping remain unresolved before later cleanup work.

## Verification and review

The canonical portable plan now has 25 required checks, including separately
named `protocol`, `protocol_fuzz`, `ipc` and `ingestion` integration targets.
The local combined plan has 27; native retains its original three required
entries and both real sandbox methods, plus the required `ipc_namespace` target.
That real native fixture connects an outer same-UID client to an inner PID
namespace server, which must safely reject its unmapped PID. Docker confinement is unchanged.
Only real portable/native paired qualification can set completion true.

Deterministic seeded malformed-input fuzzing records its actual corpus and
accepted/rejected/roundtrip counts in the protocol receipts; it is bounded
mutation testing, not an exhaustive proof or a coverage-guided fuzz campaign.
Real socket tests cover malformed/truncated streams, peer/channel evidence,
path replacement, rollback, FD exhaustion, slow peers and resource limits.
Admission tests compose actual socket framing with handshake/authorization,
duplicates/conflicts/gaps, generation rotation, byte/count limits and truthful
backpressure. No unmeasured application latency/RSS target is claimed.

Current candidate source, actual qualification and independent review are
recorded in `evidence/m1-8/` and the issue-linked PR. Historical component receipts
identify their composition limits and intermediate failures. They cannot replace
current-revision integrated Docker, native and paired evidence or owner approval.
