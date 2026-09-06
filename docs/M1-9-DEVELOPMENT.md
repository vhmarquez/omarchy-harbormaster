# M1 #9 — storage responsibility boundaries

Recorded before production implementation on `codex/m1-9-sqlite-storage` from
owner-approved and owner-merged PR #50 main
`47d726972991c88da897fe20ef4ce413142227d5`. Issue #8 is closed. Fresh actual
post-merge hosted Docker portable 25, local combined 27 and dedicated native 4
required checks passed; the actual paired CLI passed for all 174 selected files.
Receipts are preserved under `evidence/m1-9/post-merge-main/`.

## Scope and direction

The existing Rust package gains a manager-owned SQLite storage library and one
bounded blocking database worker, exercised only with disposable fixtures.
No daemon, bridge, adapter, real user database, harness database, QML screen or
runtime mode is activated. No #10 reducer/spool implementation or M2 work begins.

- `storage` typed requests/results describe trusted manager write sets, bounded
  queries and fixed maintenance operations. They expose no arbitrary SQL,
  callback, database filename, SQLite URI or caller-selected pragma interface.
- `storage` database modules own schema identity/migrations, one SQLite
  connection, validated atomic write sets, private fixed paths, backup/recovery,
  and configured database/WAL bounds. Split by cohesive transaction, filesystem,
  schema and backup responsibilities; do not put all SQL in the worker loop.
- `storage/worker` owns the bounded command/reply handoff and worker lifetime.
  It dispatches typed operations; it does not derive lifecycle/attention policy.
  A full queue is explicit backpressure. Lost/timed-out reply is an unknown
  commit outcome, never a rollback or success claim; retry/reopen must reconcile.
- The future #10 reducer supplies a typed event/projection/attention/outbox write
  set. #9 validates scope, expected revision and transactional consistency, then
  commits all related rows together. It does not infer terminal ordering,
  current process identity, human review or notification delivery from a frame.
- `tooling` owns explicit hash-pinned public preparation; verification owns the
  bounded offline SQLite build and exact version/source/config/linkage checks.
  Rust protocol/ingestion cannot depend on tool preparation or QML.

Dependency direction: worker -> database and typed storage model -> existing
protocol types. The protocol stays pure; IPC retains its separate event/control
authorization. Storage does not treat a raw typed frame or
`Admission::AlreadyAdmitted` as proof of durable success, authorization or adapter
source provenance. Initial manager registration and supplied write sets are
trusted internal APIs, never new wire operations. A failed database command must
leave its bounded pending item available for explicit retry/reconciliation;
volatile admission receipts are not durable acknowledgments.

## Owner-approved retention policy, 2026-09-06

The owner explicitly approved the following policy in this continuation:

- History supports 7 or 30 days, default 30, always with the independent 20,000
  retained-fact ceiling. The illustrated 90-day setting is unsupported and must
  be rejected, not silently clamped or used to raise the ceiling.
- Terminal tombstones have a separate 20,000-record / 30-day limit. Before
  age/cap eviction of an authoritative tombstone, atomically retire its producer
  generation. Subsequent ingestion needs a fresh manager-issued generation and
  authoritative reconciliation. A failed retirement preserves protection.
- Active sessions and unresolved/unreviewed obligations remain separately
  protected. Existing outbox policy remains 1,000 pending items / 7-day default
  expiry. Delivery expiry/overflow does not mean review or erase an obligation.

This resolves the prior numeric/mapping decision, not the implementation gate.
The frozen design assets remain unchanged. #9 can test storage-level history,
outbox and retirement transactions; #10 still owes deterministic lifecycle,
late-event, restart and reconciliation behavior across those boundaries.
No cleanup of real data or unconfirmed user-facing cleanup flow is authorized.
Deletion does not promise erasure from DB pages, WAL, backups or storage media.

## Required database invariants

Only fixed manager filenames beneath an owned 0700 state directory are opened.
Files are regular and private, with no-follow ancestor validation and retained
directory identity. Reject foreign SQLite application/schema identity and unsafe
existing entries before write-capable configuration. Never open a harness path,
enable URI filenames/extensions/ATTACH or load arbitrary SQL. Acquire exclusive
manager ownership; same-UID hostile replacement remains an explicit limitation.
Allow only reviewed local filesystem types; network/unknown types fail closed.

All SQLite work is serialized. Fixed SQL, bounded request sizes, bounded query
pages and row counts, a finite busy timeout, page/cache limits and explicit WAL
checkpoint policy keep individual work finite. Checkpoint blockage must cause
visible backpressure before unconstrained WAL growth; a journal size target is
not a hard WAL limit. Keep accepted lifecycle facts, projection revision,
unresolved attention, outbox status, producer checkpoint and replay protection
transactionally consistent. Only a successful COMMIT returns a committed result.
Use a lossless representation for full-u64 sequence/revision values; SQLite's
signed INTEGER cannot represent their complete protocol range.

Use reviewed current SQLite source, not an assumed-safe older bundled copy.
Current research identifies the upstream 3.53.4 release and a minimal rusqlite
binding with backup/limits support. Pin and review exact sources, resolved graph,
features, licenses and advisory applicability before building. A separate source
preparation group and offline static build are preferable to silently modifying
a registry archive. Verify the actual loaded SQLite version/source identity and
that no host dynamic SQLite was selected. No authored unsafe code is planned.

Migrations recognize only known schema versions and run transactionally, with a
verified live-backup API snapshot before destructive/change migration. Preserve
the original on failure or newer/foreign schema. Backup uses bounded page steps,
deadline handling and integrity/version validation; never copy just a live main
DB. Restore is a fixed manager-backup operation under exclusive ownership, with
validation before replacement and rollback-safe staging. It does not salvage
arbitrary/harness files, prune unrelated artifacts or implement #10 orphan cleanup.

## Evidence and approval

Register each new integration family and necessary SQLite build check in the
canonical verifier. Preserve locked-down Docker and separate mandatory native
qualification, including all three actual namespace methods from #7/#8.
Observe meaningful RED/GREEN for transaction rollback, stale/conflicting scope,
u64 limits, queue/row/disk bounds, expiry-versus-review/history, migrations,
supported backup and recovery, and actual cross-process abrupt exits around
commit. Avoid compile-only RED as a substitute for semantic regressions.

Independent authors use separate worktrees. Independent correctness/security/
maintainability review must cover safe database ownership, SQLite source and
linkage, transaction/worker APIs, privacy scope and approximately 300-file /
50-function size triggers. Metrics are review triggers, not file-splitting quotas.
Actual fixture measurements do not claim future product latency/RSS budgets.

Publish an evidence-backed #9 PR only after local combined, dedicated native,
hosted portable Docker and actual paired qualification at the exact candidate,
plus independent review. Stop for explicit owner approval of that PR/revision
before merge or dependent #10 implementation. #50's approval is not #9 approval.
