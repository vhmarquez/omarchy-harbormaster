# Development contract — M1 #7

## Scope and approval boundary

M1 was authorized by the owner's development goal on 2026-09-05. This feature
branch implements issue #7 only. The historical M0 status describes its own
closure; [M0 owner decisions](M0-OWNER-DECISIONS.md), MIT, the frozen option 02
and supplemental designs, and Codex Limited visibility remain unchanged.
There is no installable manager, deployed plugin or harness adapter in #7.

Each coherent change starts on an issue-linked branch from current main.
Concurrent editing agents use separate worktrees. Worktrees are separate
checkouts, not security sandboxes. Publish focused PRs with executable evidence
and independent correctness, security and maintainability review. Only the
owner's explicit approval of the current PR/revision, plus passing required
checks, authorizes merge. No direct-main feature work, auto-merge, self-approval
or protection bypass. Material changes invalidate earlier approval. Stop before
dependent #8 implementation until #7 has been approved and merged.

## Working module boundaries (recorded before implementation)

- `crates/harbormaster`: one small Rust package; pure argument interpretation
  in its library, a thin executable for output/exit. This foundation accepts
  only help/version requests and rejects unsupported operations; no daemon,
  bridge, IPC, database or launch stubs that pretend to work.
- `qml/tests`: native Qt Quick Test import/toolchain fixtures only. No mock
  manager is presented as an application and no durable decisions live in QML.
- `scripts/verify.py`: the single verification entry point; orchestration only.
- `scripts/verification/`: cohesive boundaries for isolated execution, check
  definitions/reporting and maintainability analysis, only as complexity needs.
  Tool preparation is separate from offline code execution; no silent downloads
  or use of ambient credentials inside tests.
- `tests/verification`: executable verification-runner/security/reporting tests.
- `tools/`: explicit tool pins and dependency/advisory/license policy.
- `.github/workflows`: pinned CI setup that invokes the same verifier, not a
  parallel implementation of its checks.
- `docs/`: version-controlled handoff, decisions and verification evidence.

Dependency direction now: executable -> pure library; verification runner ->
check definitions and isolated subprocess boundary. The hosted #7 correction
keeps runtime-path isolation in the container launch policy: a private, bounded,
read-only `/run` mount hides image-provided runtime directories. The common probe
continues to reject visible `/run/user` and `.git`; it does not interpret image
contents as proof of host exposure or waive the rejection. Configuration tests
cover launch policy; disposable namespace reproductions and actual hosted runs
provide distinct execution evidence. No product module or dependency is added.
Production does not import
test tooling. Future pure domain/reducer logic must not depend on IPC, storage,
adapters or UI; those boundaries are constraints, not authorization to scaffold
future milestones. The #7 workspace deliberately has no speculative crates.

## Maintainability and verification

Apply DRY, KISS, YAGNI and pragmatic SOLID. Prefer narrow explicit interfaces,
composition, typed errors, RAII, minimal dependencies and clear ownership.
Forbid unsafe Rust unless a later scoped change documents necessity and receives
a focused safety review. Do not trade genuine shared semantics for superficial
code reuse, catch-all utilities or one-module-per-trivial-helper fragmentation.

Approximately 300 nonblank lines per handwritten production file or 50 per
function triggers review, not an automatic rejection or a target to fill.
Report size/complexity hotspots and reviewer disposition; cohesive exceptions
need a rationale. Generated/vendored code and frozen design assets are excluded;
large tests/fixtures need their own rationale. No compressed formatting or
meaningless splits to satisfy numbers. Review coupling, duplicated rules,
public API size, dependency direction, nesting, dead code and unnecessary
indirection alongside correctness/security. Refactor only within this scope.

Use observed test-first RED/GREEN evidence, then refactor with tests green.
Required checks fail closed on unavailable tools, skips or unexecuted suites.
Optional, explicitly scoped live M0 probes are never silently counted as passing.
No model calls, inherited live credentials, desktop sockets or real profiles.
No deployment, shell reload, network listener or disruptive toolchain cleanup.
No protection bypass: the previously blocked AGENTS.md is not retried.

## Unresolved later gates

Before #9/#10 cleanup implementation, reconcile illustrated 7/30/90-day retention
with the 20,000-fact/30-day ceiling, and fix independent tombstone cap/TTL with
atomic generation retirement/reconciliation. Do not silently raise M0 limits.
Storage, hostile IPC, crash/replay, read-only diagnostics and privacy-path tests
are owed by their corresponding issues; the M0 fixture inventory does not prove
those implementations. Native UI/accessibility and measured runtime performance
remain their own later gates. M1 does not authorize M2 or production deployment.
