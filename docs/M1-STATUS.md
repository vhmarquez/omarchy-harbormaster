# M1 status and handoff

Live state verified 2026-09-06: PRs #49–#51 were explicitly owner-approved
and merged; issues #7–#9 are closed. Main is
`23be30f0d8fa4fb9767241d08faa0a543a3c950b`. M1 milestone 2 and roadmap #1
remain open; #10 remains open and M2 has not begun.

| Issue | Current scope | Gate |
|---|---|---|
| #7 | Owner-merged foundation, qualified main | Completed dependency |
| #8 | Owner-merged protocol/private IPC, qualified main | Completed dependency |
| #9 | Owner-approved and merged SQLite library, qualified main | Completed dependency |
| #10 | Reducers/recovery work on `codex/m1-10-reducers-recovery` | Independent review, current-head qualification and explicit owner PR/revision approval before merge |

[#9 post-merge evidence](../evidence/m1-10/post-merge-main/summary.json) preserves
actual main hosted Docker portable 28 required PASS, fresh combined 30 PASS,
dedicated native 5 PASS, and paired PASS for all 215 selected source files.
Run 34052470724 / artifact 9995027347 was downloaded and inspected. The three
optional live/future probes skipped; no required checks skipped.

[#10 boundaries](M1-10-DEVELOPMENT.md) were committed before implementation.
The [#9 evidence index](../evidence/m1-9/README.md) remains the historical
acceptance/review record; its former approval gate is superseded by the verified
owner-approved merge. [#8 post-merge evidence](../evidence/m1-9/post-merge-main/summary.json)
and [#8 implementation evidence](../evidence/m1-8/README.md) remain unchanged.

## Current implementation and limits

The single Rust package supplies help/version/error handling, typed protocol,
private IPC/admission and the bounded SQLite worker. The #10 candidate adds
pure lifecycle/attention reducers, exact durable receipt coordination, verified
fresh-generation reconciliation, bounded recovery artifacts and metadata logs,
explicit cleanup/policy, and immutable diagnostics. Only private disposable
fixtures run these libraries. Unsupported daemon/bridge/launch modes still fail;
there is no installable manager, live adapter, observer or recovery UI.
QML remains the native Qt Quick Test sentinel, with no durable business logic.

The [storage contract](M1-9-STORAGE-ENGINE.md) covers atomic facts/projection/
attention/outbox/checkpoint/tombstones, scoped exact retries, fixed safe paths,
known migration, live backup, bounded query/row/page/WAL policies and explicit
recovery limits. A lost ticket or startup/recovery timeout is an unknown outcome.
Corrupt-page recovery needs an authoritative revision upper bound including
possibly committed unknown outcomes; without it the recovery API is unavailable.
The qualified main does not yet provide a deterministic lifecycle reducer,
replay/reconciliation controller, logging/spool or read-only diagnostic product
interface. #10 is implementing and must evidence those acceptance criteria.
See [coordination](M1-10-COORDINATION.md), [recovery](M1-10-RECOVERY.md) and
[diagnostics](M1-10-DIAGNOSTICS.md) for implemented library boundaries.
Production source eligibility is deliberately empty: synthetic private mappings
qualify privacy behavior, without claiming live adapter/version support.
Product latency/RSS/idle-CPU budgets remain unmeasured.

## Approved retention

The owner approved 7/30-day history, default 30; reject illustrated 90, with the
independent 20,000-fact ceiling. Tombstones have a separate 20,000/30-day limit;
eviction atomically retires affected generations and needs fresh reconciliation.
Active/unreviewed obligations are protected separately. Outbox keeps its separate
1,000 pending / 7-day expiry policy. No frozen design asset was changed, and
there is no real-data cleanup or secure-erasure claim.

## Preserved boundaries

Keep Docker locked down and native bubblewrap separately mandatory, preserving
both original M0 methods and the #8 unmapped-peer-PID method. Portable green
alone is incomplete. Pair actual trusted receipts against the exact candidate's
selected source bytes. Dedicated branches and separate editing worktrees remain
required; worktrees are not security sandboxes. Only standard free public-repo
GitHub CI is authorized. No runner, privilege, seccomp or system configuration changed. The verifier adds
a private 1 MiB `/fault-fs` tmpfs in each existing backend for real ENOSPC tests;
it never fills a host volume.

MIT, frozen option 02/supplemental designs, native harness approvals and Codex
Limited visibility remain unchanged. Watcher, real sessions, credentials,
harness profiles, desktop configuration and prior tool/report roots are intact.
No M2, deployment, live shell activation or subsequent-PR merge is authorized.
M1 remains incomplete until all #7–#10 gates and approved merges are verified.
