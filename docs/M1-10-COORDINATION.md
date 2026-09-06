# M1 #10 durable coordination

The library's coordinator joins the already bounded artifact, domain, ingestion
and SQLite boundaries. It opens no listener, collects no process or harness
content, and enables no live adapter mapping. `ReplayEntry` is sealed by the
private source eligibility gate. Arbitrary typed wire input cannot construct it.

A commit job borrows one database worker and exclusively borrows the volatile
registry through its nonblocking state transitions. It first reads a durable
context. An exact event receipt is sufficient to recover the original commit,
even after startup retired that generation; a checkpoint alone is insufficient.
Missing events require matching active/reconciled producer scope, harness and
next sequence before pure reduction. Invalid evidence, conflicts and gaps retire
the exact current generation without creating or acknowledging a fact. Old
spooled generations cannot invalidate a newer volatile registration.

The original Apply/Retire request survives backpressure, pending and unknown
completion. Only an explicit stale-revision error permits a bounded context
refresh (at most three). Unknown completion blocks volatile admission and
retains the original intent; it never asserts rollback. `PendingCommit` can
resume that same request, including its original event, effects and time.
Dropping a job does not cancel an executing database operation or remove spool
bytes. Polling performs one nonblocking worker poll; callers schedule subsequent
polls. Neither volatile admission nor `AlreadyAdmitted` creates a durable receipt.

Only exact durable lookup or actual atomic commit constructs `DurableReceipt`.
The receipt and owned spool entry can be taken once. Explicit spool confirmation
checks event equality and owned artifact identity before unlink; unlink failure
does not undo a committed outcome. Replaying after a lost receipt or crash at
that boundary recovers the original revision without another outcome/outbox row.

Reconciliation compares explicit trusted identity/current-turn evidence and
binds it to the requested producer, prior generation, run and harness. The pure
comparison does not collect or attest that evidence; live collection is outside
M1. Missing, stale or contradictory proof persists an inactive stale/unknown
baseline. A verified database response installs its exact newly issued generation
in admission. No second UUID is generated there. A lost reconciliation receipt
never installs a guessed generation and is not automatically retried: the manager
must independently establish fresh evidence and reconcile current durable state.

Explicit `continued_turn` links a verified ongoing native wait to its new
observation generation while preserving original event/outcome provenance,
review and delivery. Explicit `resolved_turn` instead asserts independently
verified progress; the two assertions are mutually exclusive. Repeated start
callbacks are not proof of supplied approval or input. Current attention comes
from the verified current projection; historical unresolved obligations remain
protected without being relabeled current native approval.

Known volatile discards and possible unknown gaps remain separately observable.
When a reducer decision requires retirement, volatile invalidation precedes
submission and its newly discarded count is part of that exact atomic write.
Explicit retirement/reconciliation use the same checked durable accounting.
Recovery counters describe rejected attempts and discarded artifacts separately;
they are not counts of distinct lost events or proof of a cross-filesystem
transaction. Diagnostics format already-owned snapshots without collecting data,
opening a database, changing policy, deleting files or rotating logs.

Private synthetic integration cases use real SQLite and Unix peers, a bounded
worker backlog, deliberate loss of an actual worker reply, and a child killed
with SIGKILL after commit but before spool unlink. Synthetic excluded-field
canaries are scanned across database/WAL, spool, logs and diagnostics. These are
M1 library tests, not qualification of live adapters, daemon operation, native
approval submission, UI, desktop delivery, or callback latency/RSS budgets.
