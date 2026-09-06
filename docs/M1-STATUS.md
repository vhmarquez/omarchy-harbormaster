# M1 status and handoff

M1 is authorized by the owner. Live state checked 2026-09-06: owner-merged PR #49,
closed #7, main `70a671bf5bf93aaed7a3acbc1ad26df76fd60187`, and open #8/#9/#10.
Milestone 2 remains open (one closed, three open issues); roadmap #1 remains open.

| Issue | Current scope | Gate |
|---|---|---|
| #7 | Merged by owner; post-merge main qualified | Completed foundation dependency |
| #8 | Protocol/private IPC development on `codex/m1-8-protocol-ipc` | Fresh checks, independent review, explicit owner PR/revision approval before merge |
| #9 | Not started | Approved/merged #8 |
| #10 | Not started | Approved/merged dependencies |

[Post-merge evidence](../evidence/m1-8/post-merge-main/summary.json): actual main
hosted Docker portable 21 required PASS, fresh combined 22 required PASS, dedicated
native 3 required PASS, paired PASS for 140 selected source files. Only the three
explicit optional live/future probes skipped. See [verification](M1-VERIFICATION.md)
and [pre-implementation #8 responsibilities](M1-8-DEVELOPMENT.md).

Keep Docker locked down and native bubblewrap integration separately mandatory.
A portable green run alone never completes qualification. Actual trusted receipts
must pair against each exact candidate revision. Work uses dedicated branches;
independent editing agents use separate worktrees, not security sandboxes.
Only free standard public-repository GitHub CI is authorized. No new PR has owner
merge approval, and no dependent #9/#10 implementation or M2 work is authorized yet.

## Scope now

- One dependency-free Rust package supplies real help/version/error handling
  and a minimal build/test foundation. Unsupported runtime/bridge/launch modes
  fail explicitly; this is not an installable manager or a daemon stub.
- The QML file is a real Qt Quick Test sentinel, not a production screen or
  claim of approved UI/native-accessibility implementation.
- Verification uses private disposable homes and bounded offline subprocesses;
  live runtime/harness probes remain separate explicit opt-ins, not silent passes.
- Performance budgets, hostile IPC, storage/crash/replay/retention and read-only
  product diagnostics are still owed by their corresponding M1/later issues.

## Outstanding decisions before cleanup implementation

The illustrated 90-day retention choice is not permission to exceed the
20,000-fact/30-day backend ceiling. Resolve the effective history choices and
separate active/unreviewed/outbox policies before #9/#10 cleanup. Pin numeric
terminal-tombstone cap/TTL with atomic producer-generation retirement and
reconciliation; no silent cap increase or replay-defense eviction.

## Safety and preserved decisions

MIT, original font/OFL and frozen design provenance are unchanged. Option 02
and the supplemental UX are approved; native harness approvals stay native.
Codex remains Limited visibility, without trusted-native-callback qualification.
No live Watcher changes, desktop activation, harness profiles, real sessions,
credentials, protected context files, system package updates or toolchain cleanup
are authorized. Local Docker daemon access is denied; no permission/system change
is made to work around it. CI uses an isolated container instead; actual hosted
execution must be verified before acceptance. No M2/deployment has started.
