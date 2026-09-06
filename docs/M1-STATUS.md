# M1 status and handoff

M1 is authorized by the owner's 2026-09-05 development goal. Work begins from
M0 closure `ed68ec9dd954b9a54bc20bc14e39a088f99290b0`. The approval/workflow and
pre-implementation responsibility record is [DEVELOPMENT.md](DEVELOPMENT.md).

| Issue | Current scope | Gate |
|---|---|---|
| #7 | Cargo/QML foundation and owner-approved mandatory portable/native verification split | Exact-head Docker + native evidence, independent review, then explicit owner approval before merge |
| #8 | Not started | Approved/merged #7 |
| #9 | Not started | Approved/merged #8 |
| #10 | Not started | Approved/merged dependencies |

The initial hosted preflight and missing-binary failures were corrected without
relaxing confinement. Run 33994133494 then demonstrated actual inner namespace
permission denial. The owner approved keeping Docker locked down and making
native bubblewrap integration separately mandatory. See the [verification
contract and historical evidence](M1-VERIFICATION.md). A portable GitHub PASS is
not complete qualification. The paired evidence checker must accept trusted
Docker/native records against the exact current head, and independent review
and explicit revision approval remain mandatory. No branch protection, automatic
merge or live desktop/harness policy is changed.
The owner explicitly permits free standard GitHub CI, not paid runners/services.

The issue-linked integration branch is `feat/m1-7-developer-foundation`.
Retrieve its PR/current head with `gh pr view feat/m1-7-developer-foundation`;
CI and owner approval must be read from that exact revision, not inferred from
a local report. The current handoff is awaiting that PR acceptance gate.
Concurrent CLI, tooling, CI and maintainability edits used separate worktrees;
only scoped artifacts are integrated. No feature work is committed to main.
A green test/reviewer verdict, this goal and M0 approval are not merge approval.
The requested deliverable is a focused #7 PR; dependent implementation stops at
its owner approval gate. M1 cannot be called complete while #7–#10 remain open.

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
