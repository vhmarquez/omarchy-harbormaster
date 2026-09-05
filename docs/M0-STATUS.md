# M0 status — accepted feasibility and design

**M0 acceptance is complete as of 2026-09-05 under the owner-approved Codex Limited visibility scope.** [The owner's explicit decisions](M0-OWNER-DECISIONS.md) resolve the license, supplemental design and harness-scope gates. M1 and deployment remain unstarted and require separate authorization. This repository is not an installable manager.

| Issue | Deliverable | Acceptance status |
|---|---|---|
| [#2](https://github.com/vhmarquez/omarchy-harbormaster/issues/2) | Product/namespaces, architecture ADR and threat model | Accepted; owner selected MIT, root [LICENSE](../LICENSE) supplied; third-party notices preserved |
| [#3](https://github.com/vhmarquez/omarchy-harbormaster/issues/3) | Frozen option 02 bundle, UX contract and supplemental review/settings/recovery study | Approved by the owner; native implementation/accessibility verification remains future work |
| [#4](https://github.com/vhmarquez/omarchy-harbormaster/issues/4) | Independent terminal/user-service runtime spike | Previously verified and closed; constrained GO for the tested standalone foot/Hyprland stand-in stack |
| [#5](https://github.com/vhmarquez/omarchy-harbormaster/issues/5) | Installed-version observational hook spike and capability matrix | Accepted with explicit scope revision: Codex Limited visibility; trusted native callbacks remain unqualified, not passed |
| [#6](https://github.com/vhmarquez/omarchy-harbormaster/issues/6) | Reference workloads, typed protocol/domain and critical failure scenarios | Previously verified and closed; required drafts/specifications supplied, not achieved performance or production-daemon regression claims |

## Owner decisions and limits

The verbatim response and its mapping are recorded in [M0-OWNER-DECISIONS.md](M0-OWNER-DECISIONS.md). The owner's “#4 - accept limited codex visibility” clause resolves actual harness issue #5; runtime issue #4 is unchanged. There are no unanswered M0 owner gates.

- MIT applies to Harbormaster-authored source and documentation. The original font's OFL and the archived design provenance remain unchanged. Actual dependency/distribution license audits remain later work.
- The supplemental design and UX contract are approved. Owner approval is not evidence that anyone executed the outstanding full keyboard/scaling/accessibility or native Qt/Quickshell matrix. Those remain M3/M6 implementation gates.
- Codex's credential-free native startup stopped at authentication before trusted `/hooks` review. No native callback ran. The owner accepts Limited visibility rather than requiring that unproven path for the first implementation scope. Further qualification is tracked by M5 #24/#25 and requires separate authorization for any credentialed/network/model test.

A protected `AGENTS.md` write previously received no approval and was blocked. It was not retried through another tool/path. That project-context file remains absent; it is not an explicit M0 acceptance criterion and can be authorized separately before ongoing development. This owner-decision reconciliation does not retry it.

## Scope limits

Source inspection, synthetic callback dispatch, actual installed-loader execution, actual CLI startup callbacks and actual native runtime tests are distinct evidence classes. Test success does not establish harness parity or a working product. Performance numbers in PERFORMANCE.md remain targets. Native QML interaction, live model-driven turns, broader terminal support and release hardening remain later gates.

Original Hermes Watcher, real harness credentials/configuration and global terminal configuration are not migration targets in M0. Probes create temporary private homes/services/windows and remove only their own resources. Logout/reboot/suspend behavior is documented, not destructively tested on the user's machine. Same-UID isolation, atomic control races and a production aggregate cleanup budget remain unproven.

## Verified technical baseline and discovery side effect

The published final baseline in [evidence/parent/final-tests.json](../evidence/parent/final-tests.json) records **41 passing tests**: 10 contract checker, 10 runtime, 6 harness and 15 design, plus archive and fixture integrity checks. These include explicit synthetic fixtures and honest gap-detection tests, not 41 production tests. An initial aggregate run used an incorrect Node path; only the corrected full run is the recorded final result. [Parent verification](M0-PARENT-VERIFICATION.md) and [independent reviews](../evidence/review/README.md) distinguish pre-fix failures from post-fix acceptance.

For this documentation/license reconciliation, [owner-acceptance-checks.json](../evidence/parent/owner-acceptance-checks.json) records **38 passing non-live tests** (10 contract, 8 runtime identity/cleanup, 5 harness helpers, 15 design) plus archive/fixture checks. The real desktop and installed harness probes were not rerun; their recorded baseline is retained. All 23 original assets still match their hashes, and the MIT text matches GitHub's standard template with the copyright placeholders filled.

Initial `mise which` discovery unexpectedly pruned old cached Claude 2.1.259 and Codex 0.153.0 installations. The deletion was observed and those paths confirmed absent. The baseline tested versions remain Claude 2.1.260 and Codex 0.153.2. Reproducible probes use installed binaries directly and never invoke mise. No silent reinstall or global configuration change was attempted. The Hermes Watcher Git tree was separately verified clean.

New-authored files passed scoped diff whitespace checks. The byte-preserved original font `OFL.txt` has one inherited trailing-space warning; its license bytes are intentionally unchanged.

## Publication history and current tracking

The original verified artifact commit is `d2b0e78dce0affb09726e9778698a389836b52f8`; the pre-approval status tip was `28f9ccab05ac518c17364446f93e51ffef6926e2`. At that earlier checkpoint, #4/#6 were closed and #2/#3/#5 were open. Exact REST issue states and GraphQL showed three open/two closed while the REST milestone aggregate still reported five open/zero closed. That historical discrepancy is retained in [remote-publication.json](../evidence/parent/remote-publication.json), not rewritten as a successful counter check.

Acceptance commit `3b9c1a3cd20b999adcbc55be844f14b855fef658` publishes MIT and the owner decisions. Exact read-back confirms **all five M0 issues #2–#6 and milestone 1 are closed**; roadmap #1 remains open with its M0 entries checked. Issue #5 explicitly revises scope rather than marking trusted native callbacks as passed. [Owner-closure publication evidence](../evidence/parent/owner-closure-publication.json) records matching issue bodies/states, milestone description/state, GitHub LICENSE bytes and GraphQL enumeration of **zero open/five closed** M0 issues. A cache-busted REST milestone aggregate still reports five open/zero closed; that stale counter is recorded, not trusted or forced by reassigning/cycling issues. All later milestone descriptions/states are unchanged, and M1 #7–#10 remain open. See roadmap [#1](https://github.com/vhmarquez/omarchy-harbormaster/issues/1) and [milestone 1](https://github.com/vhmarquez/omarchy-harbormaster/milestone/1).

## Resumption boundary

Read roadmap #1, this status, the owner-decision record and exact issue evidence before new work. GitHub publication state takes precedence over an older snapshot. M0 completion removes its dependency gate; it does not authorize M1, deployment, a protected context-file write or a credentialed Codex test. Obtain the next scoped work authorization and preserve the accepted capability limits.
