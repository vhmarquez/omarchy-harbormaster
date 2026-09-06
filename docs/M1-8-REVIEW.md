# M1 #8 independent review

Review is scoped so no author supplies the independent verdict on their own
component. The protocol author reviewed dependency preparation and the safe
peer-credential correction; the IPC author independently reviewed protocol and
ingestion; the dependency author independently reviewed IPC and ingestion.
Actual revision/hash bindings, commands, results and limits are retained in
[the review receipts](../evidence/m1-8/review/). Final combined, dedicated native,
hosted Docker and paired evidence are separately indexed in
[the candidate evidence](../evidence/m1-8/README.md). Review PASS is not merge
approval or a substitute for current-head qualification.

## Corrections with observed failures

- Exact duplicate-key and depth-8 validation are independently sensitive to
  removing their guards. Seeded malformed-input coverage executes 12,000 cases
  and 60 structured hostile cases across all five public wire decoders.
- An accepted-pending response initially accepted an extra committed revision.
  Its typed empty payload now rejects that false durability claim.
- Queue drain retains exact replay receipts. Independently bounded record and
  byte ceilings freeze a generation rather than silently forgetting protection.
  Three real failing tests exposed A-to-B-to-A generation reuse and session
  tokens accepted by another registry/authorizer. Internally issued fresh
  generations and private instance/scope proofs close those paths.
- Real descriptor exhaustion exposed a stale socket after an additional open
  failed. Ownership is now captured before that open; the independent EMFILE
  regression passes and cleanup still refuses unrelated replacements.
- A real PID namespace fixture established that Linux can return peer PID 0.
  Source review found the pinned rustix socket getter assumes a nonzero PID;
  that getter was not invoked on the invalid representation. This is no claim
  of demonstrated memory corruption or a published advisory. The minimal nix
  getter preserves the raw signed PID, which is rejected before conversion.
  Removing only this positive-PID rejection makes the real native fixture fail.
  No authored unsafe code or Docker permission change is involved.

## Responsibility and maintainability disposition

Production direction remains ingestion to typed protocol and narrow transport
evidence; transport has no domain grants; protocol has no I/O; public dependency
preparation is separate from offline validation. One existing crate contains
these cohesive modules. Native approval, runtime execution, persistence and QML
business decisions are absent. Shared rules remain in protocol types, registry
scope checks and the canonical check inventory rather than duplicated clients.
Review found no remaining correctness, security or maintainability blocker.

The canonical analyzer's approximately 300-file/50-function nonblank-line
thresholds trigger review rather than automatic splitting:

| Unit | Observed trigger | Independent disposition |
|---|---:|---|
| `protocol/control.rs` | 303 file lines | Accepted: one typed twelve-operation schema and revision contract; functions remain small. |
| `verification/checks.py::plan` | 51 function lines | Accepted: explicit shared/portable/native inventory stays auditable in one place. |
| `tests/protocol.rs` | 361 file lines | Accepted: cohesive wire boundary cases with shared fixture constants. |
| `tests/ingestion.rs` | 302 file lines | Accepted: shared disposable peers and admission/session cases; control and limit families already separate. |
| `tests/ipc.rs` | 453 file lines | Accepted: sixteen transport cases, shared disposable fixtures and bounded subprocess FD-pressure probe; each test function below 50. |
| `tests/ipc_namespace.rs::server_process` | 55 function lines (Clippy body: 51) | Accepted: a single fixed sandbox argument list; narrow documented Clippy expectation keeps security flags together. |

The existing 429-line qualification test file retains its prior #7 disposition.
Generated locks, receipts and frozen design assets are excluded from handwritten
production thresholds. No metric-driven compression or meaningless split was
used. Fixture durations are recorded measurements, not qualification of future
product throughput, latency or RSS budgets.

## Remaining scope

Admission is volatile and emits no durable acknowledgment. Trusted restoration,
atomic persistence and lifecycle/terminal replay semantics belong to #9/#10.
The fixed same-UID limitations and private socket race boundaries are documented
in [the protocol/IPC contract](M1-8-PROTOCOL.md). There is no daemon deployment,
live adapter, credentialed harness test, read-only product diagnostic, spool or
retention implementation. Resolve retention mapping and numeric tombstone
cap/TTL before later cleanup; M2 remains unstarted. Stop for explicit owner
approval of the published PR/revision before merge or dependent implementation.
