# M1 status and handoff

Live state verified 2026-09-06: the owner merged PR #49 and PR #50; issues #7
and #8 are closed. Main is `47d726972991c88da897fe20ef4ce413142227d5`.
Milestone 2 remains open with two closed and two open issues; roadmap #1 is open.

| Issue | Current scope | Gate |
|---|---|---|
| #7 | Owner-merged foundation, qualified main | Completed dependency |
| #8 | Owner-merged protocol/private IPC, qualified main | Completed dependency |
| #9 | SQLite library candidate on `codex/m1-9-sqlite-storage` | Current-head qualification and explicit owner PR/revision approval before merge |
| #10 | Not started | Approved/merged dependencies; no dependent implementation before #9 approval |

[#8 post-merge evidence](../evidence/m1-9/post-merge-main/summary.json) preserves
actual main hosted Docker portable 25 required PASS, fresh combined 27 PASS,
dedicated native 4 PASS, and paired PASS for all 174 selected source files.
Run 34039894234 / artifact 9991394591 was downloaded and inspected. The three
optional live/future probes skipped; no required checks skipped.

The [#9 evidence index](../evidence/m1-9/README.md) maps its three acceptance
criteria to executed fixtures, review and final qualification records. Read the
issue-linked PR for its exact published head, hosted run/artifact and actual
paired report. Component evidence cannot substitute for those final gates.
[#8 evidence](../evidence/m1-8/README.md) and its former approval text remain
historical; the verified owner merge supersedes that former blocker.

## Current implementation and limits

The single Rust package supplies help/version/error handling, typed protocol,
private IPC/admission and the bounded SQLite worker. Only private disposable
fixtures run these libraries. Unsupported daemon/bridge/launch modes still fail;
there is no installable manager, live adapter, observer or recovery UI.
QML remains the native Qt Quick Test sentinel, with no durable business logic.

The [storage contract](M1-9-STORAGE-ENGINE.md) covers atomic facts/projection/
attention/outbox/checkpoint/tombstones, scoped exact retries, fixed safe paths,
known migration, live backup, bounded query/row/page/WAL policies and explicit
recovery limits. A lost ticket or startup/recovery timeout is an unknown outcome.
Corrupt-page recovery needs an authoritative revision upper bound including
possibly committed unknown outcomes; without it the recovery API is unavailable.
No deterministic lifecycle reducer, replay/reconciliation controller, logging,
spool or read-only diagnostic product interface is claimed; #10 still owes them.
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
GitHub CI is authorized. No runner, permission or system configuration changed.

MIT, frozen option 02/supplemental designs, native harness approvals and Codex
Limited visibility remain unchanged. Watcher, real sessions, credentials,
harness profiles, desktop configuration and prior tool/report roots are intact.
No M2, deployment, live shell activation or subsequent-PR merge is authorized.
M1 remains incomplete until all #7–#10 gates and approved merges are verified.
