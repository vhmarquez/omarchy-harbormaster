# M1 #10 responsibilities — before implementation

The owner explicitly approved PR #51 at
`1c901e50fac0d592d1cfa45cb96edaaa731c7966`; it was merged on 2026-09-06 as
`23be30f0d8fa4fb9767241d08faa0a543a3c950b`, closing #9. Actual main passed
fresh hosted Docker portable (28 required), local combined (30), dedicated
native (5), and paired qualification (215 selected files). Hosted run
34052470724 / artifact 9995027347 was downloaded and its actual logs inspected
by the pairing validator. No required skips; the three declared optional
live/future probes remain outside the required inventory.

Work now follows [issue #10](https://github.com/vhmarquez/omarchy-harbormaster/issues/10)
on `codex/m1-10-reducers-recovery`. This is a library implementation with
disposable synthetic fixtures, not a deployed manager, adapter, notification
service, terminal runtime or UI. M2 has not begun. Each subsequent PR/revision
still needs explicit owner approval before merge or dependent implementation.

## Responsibility and dependency direction

- `domain`: pure lifecycle/attention vocabulary, scoped turn identity,
  deterministic reduction and explicit observation policy. Move the existing
  state vocabulary here and re-export it from storage for compatibility. This
  module can depend on typed protocol identities/events, never SQLite, IPC,
  files, clocks, adapters or QML. Time and capability evidence are inputs.
- `storage`: keep the sole bounded SQLite worker and closed request vocabulary.
  Add a bounded revision-consistent reducer context and durable event receipt
  lookup; persist scoped current turns, independent reason resolution and
  outcome revisions that survive fact pruning. Known-schema migration/backup
  must preserve existing obligations. A late terminal for T can be retained
  without replacing a newer current U. Preserve original exact write-set
  retries and unknown-outcome semantics.
- `reconciliation` / commit coordination: sequence the domain decision and
  atomic storage effects; retain the original submitted intent through pending
  or unknown completion. Only a durable receipt permits acknowledgment. Install
  the one generation issued by durable reconciliation into volatile admission;
  do not independently generate a second UUID. Retire/invalidate old sessions
  and count discarded backlog without retagging it. Runtime/current-turn proof
  remains an explicit trusted manager input, not a wire assertion or M2 probe.
- `recovery`: bounded manager-owned metadata artifacts and metadata log records.
  Use descriptor-relative private directories, fixed/generated validated names,
  ownership/type/link/inode checks, bounded scans and explicit cleanup plans.
  Spool staging/partial/orphan files count toward physical limits across all
  generations of a producer. Replay preserves identity and is never an ACK.
  Logs accept closed reason codes and numeric data, not arbitrary text.
- Provenance boundary: an event's string schema is not proof of safe source
  mapping. Validate eligibility before serialization or temporary creation.
  No live adapter/version becomes eligible in M1; synthetic reviewed mappings
  exercise canaries, including deliberate mis-mapping into allowed string
  fields. Keep test fixtures private to tests, with no production test escape
  or public assertion that arbitrary wire strings are safe. Unproven optional
  fields are omitted; unproven required fields reject/degrade; no raw fallback.
- Diagnostics project an already-owned, immutable snapshot using fixed codes
  and counters. They cannot initialize storage, select policy, activate
  collection, rotate logs, clean files or read private harness content.
  Policy changes and cleanup application are explicit revision/identity-bound
  operations; cleanup has a read-only preview before confirmed application.
- `scripts/verify.py` remains the sole verification entry point. Add each new
  integration family to the required portable inventory and its regression
  tests. Preserve locked-down Docker and separately mandatory native checks,
  including the two original M0 methods and unmapped-peer-PID method.

Actual filesystem-full tests need a dedicated bounded fixture, rather than
filling the host volume or relabeling a device-write failure as a full regular
filesystem. Both existing launch backends will provide a fresh private 1 MiB
tmpfs at `/fault-fs` for each check. Tests must verify its filesystem type and
capacity before bounded filling, with no fallback directory. This adds no host
mount, network, capability, seccomp exception or optional-native waiver.

## Reducer rules and protected boundaries

Process, observation, turn, attention, human review and delivery are independent.
Terminal tombstones dominate later starts/waits for the same producer/generation/
run/turn, even at a greater sequence. A genuinely new turn may start; a late
terminal for an older turn does not replace the new current turn. Contradictory
terminal evidence preserves the first outcome and makes uncertainty visible.
Exact duplicate events create no second outcome or notification; conflicts,
sequence gaps and missing hooks never imply idle, success or healthy working.

Input/approval reasons may resolve from trusted progress without human review.
New outcomes remain unreviewed even after earlier outcomes were reviewed.
Reconnect requires independently verified run identity and authoritative
current-turn evidence. Missing/ambiguous evidence stays stale/unknown and cannot
authorize normal lifecycle admission. Old queued/spooled callbacks are discarded
with visible gap accounting, never assigned the fresh generation.

The owner's approved history policy remains 7/30 days (default 30), rejecting
90, with the 20,000-fact ceiling. Tombstones are separately 20,000/30 days;
eviction atomically retires affected generations before removing protection.
Active/unreviewed obligations and the 1,000-pending/7-day outbox remain separate.
Spool limits remain 256 KiB per producer, 8 MiB total, 24 hours, including
temporary/orphan bytes. Logs remain four 1 MiB files; no invented log age TTL.
Bound file counts, scans, memory and cleanup batches as well as payload bytes.
If full storage also prevents durable drop accounting, report known loss plus
an explicit unknown gap, not zero loss. Never fill the actual home volume.

## Acceptance evidence and review

Observe RED/GREEN before implementation; test real ordering, duplicates,
conflicts, gaps, reconnect, tombstone eviction, independent review/delivery,
unknown commits, missing hooks, read-only diagnostics and policy. Run real
child termination and isolated full/read-only/locked filesystem tests. Scan
DB/WAL/spool/temp/log/diagnostic bytes for synthetic content and credential
canaries through failures and cleanup. Exercise symlink/hardlink/inode races,
bounded orphan handling and preservation of unrelated sentinels.

Independent review covers correctness/security and responsibility boundaries,
dependency direction, duplication, public API size and maintainability triggers.
Use separate worktrees for concurrent editors. Record actual causal mutation
results and review dispositions. Do not claim unmeasured harness callback,
runtime, UI, performance or desktop-delivery qualification. Existing M0 fixture
inventories remain specifications where the corresponding later implementation
is outside M1. No new dependency is planned.
