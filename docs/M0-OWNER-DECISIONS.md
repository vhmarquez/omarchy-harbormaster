# M0 owner decisions — 2026-09-05

## Source and issue mapping

The repository owner explicitly answered the remaining license, supplemental design and Codex scope questions in the development conversation:

> #2 - MIT license #3 - approved #4 - accept limited codex visibility

The response is preserved verbatim. Its Codex clause is applied to **harness issue [#5](https://github.com/vhmarquez/omarchy-harbormaster/issues/5)**, not runtime issue #4. Runtime #4 was already completed; it is unchanged. The mapping follows the explicit words “accept limited codex visibility,” not an invented runtime decision.

This is an owner-decision record transcribed by the implementation agent, not a claim that the owner authored a GitHub comment or personally ran a test. It supersedes the unanswered-owner-gate status at commit `28f9ccab05ac518c17364446f93e51ffef6926e2`; the earlier evidence and comments remain historical records.

## #2 — MIT and product boundary

MIT is selected and the project grant is in [LICENSE](../LICENSE). Harbormaster-authored source and documentation use MIT. Existing third-party rights/notices remain in force: the unmodified JetBrains Mono font retains its [OFL](design/original/assets/OFL.txt). The archived user-supplied original design bundle remains reference material with its [provenance and licensing caveats](design/PROVENANCE.md); this record does not invent an upstream rights grant. Actual pinned dependency/distribution obligations, including Qt/Quickshell, still require the M1 and release license audits.

The established local-first security/scope contract and namespaces remain in [ADR 0001](adr/0001-product-and-architecture.md) and the [threat model](THREAT-MODEL.md). No remote management, universal approval action, transcript collection or same-UID sandbox guarantee is added.

## #3 — approved design baseline

The owner approves the option 02 Project manager baseline and the supplemental review/settings/onboarding/recovery design and [UX contract](design/UX-CONTRACT.md), including the proposed state, privacy, keyboard/focus and responsive/theme semantics. This resolves M0's design-approval criteria.

Approval is not technical certification. Existing browser and actual-inline model tests retain their recorded scope; no new owner-executed browser test is inferred. Full keyboard/accessibility/scaling and native Qt/Quickshell behavior remain implementation verification gates in M3/M6. The [review checklist](design/REVIEW-CHECKLIST.md) distinguishes approved design from tests still owed. The original assets, supplemental HTML and regression tests are unchanged by this decision reconciliation.

## #5 — accepted Limited visibility scope

The owner accepts **Limited visibility** for Codex in the first implementation scope and defers trusted native-hook qualification. This is an explicit revision of M0 issue #5's all-harness native-hook expectation, as permitted by the milestone's “narrow scope if necessary” exit gate.

Accepted M0 evidence:

- Hermes: installed loader plus reversible fixture setup, with synthetic callback dispatch; not a live generated turn.
- Claude Code: actual offline init-only Setup/SessionStart/SessionEnd callbacks and reversible disposable configuration; not full turn/control qualification.
- Codex: real credential-free native startup stopping at authentication, read-only hook discovery and App Server initialization/schema inspection. Native hooks remain untrusted; **no trusted native callback was observed**.

The [capability matrix](harness-capabilities.md) remains authoritative. Limited visibility must show uncertainty and unavailable capabilities; no inferred input/approval/turn outcome, terminal inactivity-as-idle, transcript scraping, native-TUI/App-Server equivalence or harness parity is authorized. Native approval interfaces remain authoritative.

Trusted native Codex callbacks remain technically unqualified. Further qualification belongs to [M5 issue #24](https://github.com/vhmarquez/omarchy-harbormaster/issues/24), with degraded-mode mapping in [#25](https://github.com/vhmarquez/omarchy-harbormaster/issues/25). Credentialed/network/model tests require separate scoped authorization. The owner's acceptance does not authorize logging in, copying credentials, manufacturing trust hashes or bypassing approval/trust checks.

## M0 exit and next boundary

Together with the previously verified runtime #4 and protocol/performance/failure-specification #6 evidence, these decisions resolve M0 acceptance. Performance remains unmeasured targets; runtime feasibility remains limited to the tested standalone-foot/Hyprland stand-in experiment. The repository is not an installable manager.

M1 and deployment remain unstarted and require a separate work authorization. No live Watcher migration, protected project-context-file write, real harness configuration change or desktop deployment is included. See [M0 status](M0-STATUS.md) and roadmap [#1](https://github.com/vhmarquez/omarchy-harbormaster/issues/1) for the publication state.
