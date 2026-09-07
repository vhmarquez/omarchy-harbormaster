# Current continuation

M1 is complete: owner-approved PRs #49–#52 are merged, issues #7–#10 and
milestone 2 are closed, and actual main `9e8b2d2d45b2f6ff7d574c339c90616ef4cfc16a`
passed post-merge qualification. [Completion evidence](https://github.com/vhmarquez/omarchy-harbormaster/issues/10#issuecomment-5562630253).

The owner subsequently authorized continued implementation under the revised
[first usable Hermes plugin plan](https://github.com/vhmarquez/omarchy-harbormaster/issues/1).
PR #53 is approved and merged; #11 is complete at qualified main `59880ff`.
The owner authorized #12 on `codex/m2-12-managed-runtime`.
[Responsibilities and scope](M2-12-RUNTIME.md) are recorded before production
work. Stop for explicit approval of this PR/revision before merge or dependent
#13 work. No deployment or live profile activation is implied.

The owner's revised workflow supersedes the historical exhaustive evidence
instructions below: focused consequential tests while implementing, one practical
integration demonstration and one focused independent review, concise summaries
and raw logs kept separately. Use existing Docker CI and separately mandatory
native qualification. Repeat only for relevant changes, actual failures or
unresolved concerns; identical selected bytes can reuse actual execution records
when canonical pairing binds them to the exact candidate/merge SHA. No routine
mutation campaigns, overlapping reviews or evidence-only PR loops. Keep the
existing isolation, ownership, privacy, retention and native approval protections.

The following #7 contract is historical context, not a current merge blocker or
permission to ignore the updated owner direction.

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

## Owner-approved verification split (2026-09-05)

Owner authorization, verbatim:

> Keep Docker locked down and make the native bubblewrap integration a separate mandatory check.

This approves execution placement, not merge or dependent #8 implementation.
Docker confinement, permissions, package trust and its portable checks remain
unchanged. The real M0 sandbox test moves intact to a separately named native
check; it is never optional, mocked into passing or silently skipped.

Responsibilities recorded before this correction's implementation:

- `verification/checks.py` owns explicit `all`, `portable` and `native` plans.
  Portable retains the four hook/projection helper tests. Native selects the
  exact moved sandbox test with shared isolation/tool-integrity preflights.
- `scripts/verify.py` selects/reports scope. Default local `all` still requires
  both qualifications. Docker requires explicit `--scope portable`; it cannot
  claim native qualification. Unexecuted required qualifications are `NOT_RUN`,
  outside the three existing optional live/future SKIPs.
- `verification/qualification.py` checks paired portable/native report evidence
  against the same source manifest and an explicit full Git revision. It rejects
  missing, stale, mismatched, failed or skipped required evidence. This is a
  consistency gate for trusted execution records, not cryptographic attestation
  that an untrusted producer ran tests or authorization to merge.
- `spikes/harnesses/test_sandbox.py` retains the actual network attempt and
  credential/home checks, without exposing real profiles. The local qualified
  Linux backend runs it; no native hosted runner is assumed qualified.
- CI invokes `verify.py --scope portable` in the same locked-down Docker backend.
  The merge review also requires native execution and paired evidence for the
  exact candidate revision. GitHub portable green alone is insufficient.

One CLI remains the entry point: ordinary execution uses `--scope`; paired
review uses `--qualify PORTABLE_REPORT_DIR NATIVE_REPORT_DIR --revision SHA`.
No new dependency, daemon, deployment or security-policy exception is needed.

The native runner also remains inside an outer bubblewrap sandbox. Therefore
connection failure alone must not be attributed to the inner M0 sandbox. Add a
separate native assertion that the inner network-namespace identity differs
from the outer one; prove its sensitivity by removing only the inner
`--unshare-net` in a disposable fixture while retaining outer isolation. Keep
the original M0 test method intact. No listener or reachable-network positive
control is authorized or needed for that namespace-identity assertion.

### Review-driven result validation corrections

Before applying the independent split review's corrections, the dependency
boundary is explicit: a small `verification/strict_json.py` owns JSON structural
validation shared by report/manifests and JSON-formatted required logs.
`qualification.py` retains bounded file I/O and committed-source binding;
`checks.py` owns the explicit native method inventory and its result validator.
Neither checks nor JSON decoding imports qualification or the CLI. Metrics
coverage requires an actual positive integer. Native logs must show both selected
methods and one unambiguous successful suite result, not merely a positive test
count. CLI argument presence must not be inferred from a revision's truthiness.
These are corrections to the approved gate, not new product functionality.

## Unresolved later gates

Before #9/#10 cleanup implementation, reconcile illustrated 7/30/90-day retention
with the 20,000-fact/30-day ceiling, and fix independent tombstone cap/TTL with
atomic generation retirement/reconciliation. Do not silently raise M0 limits.
Storage, hostile IPC, crash/replay, read-only diagnostics and privacy-path tests
are owed by their corresponding issues; the M0 fixture inventory does not prove
those implementations. Native UI/accessibility and measured runtime performance
remain their own later gates. M1 does not authorize M2 or production deployment.
