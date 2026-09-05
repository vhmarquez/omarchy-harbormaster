# M0 status — evidence before implementation

M0 is **not complete** while owner-only gates or required integration evidence remain unresolved. No M1 work or deployment is authorized by the M0 artifacts.

| Issue | Deliverable | Gate status |
|---|---|---|
| [#2](https://github.com/vhmarquez/omarchy-harbormaster/issues/2) | Product/namespaces, architecture ADR and threat model | Drafted; license owner confirmation outstanding |
| [#3](https://github.com/vhmarquez/omarchy-harbormaster/issues/3) | Frozen option 02 bundle, UX contract and supplemental review/settings/recovery study | Original option 02 approved; supplemental personal review still outstanding |
| [#4](https://github.com/vhmarquez/omarchy-harbormaster/issues/4) | Independent terminal/user-service runtime spike | Completed and issue closed; parent rerun passed, constrained GO for tested standalone foot/Hyprland stack |
| [#5](https://github.com/vhmarquez/omarchy-harbormaster/issues/5) | Installed-version observational hook spike and capability matrix | Partial: Codex auth gate prevented trusted native callback execution; remains open |
| [#6](https://github.com/vhmarquez/omarchy-harbormaster/issues/6) | Reference workloads, typed protocol/domain and critical failure scenarios | Completed and issue closed; required drafts/specifications supplied and checker rerun, not a measured performance claim |

## Decisions requiring the owner

1. Project license: MIT recommended; Apache-2.0 also compatible in principle. The repository was empty and unlicensed at preflight. The license question received no selection; silence did not authorize a grant. No project LICENSE was created.
2. Supplemental design approval: the original option 02 composition is selected. New review disposition, settings, onboarding and recovery screens are delegated design proposals, not personally user-reviewed artifacts. Review `docs/design/supplemental.html` and the design checklist before closing user-review criteria.
3. Codex integration gate: either explicitly accept limited visibility/deferred native-hook qualification for the first implementation scope, or authorize a separately scoped authenticated disposable native `/hooks` test. Credential-free startup stopped before that UI. No existing credentials were copied or login/trust gate bypassed.

A protected `AGENTS.md` write also received no approval and was blocked. It was not retried through another tool/path. That project-context file remains absent; it is not one of M0's explicit issue acceptance criteria and can be authorized separately before ongoing development.

## Scope limits

Source inspection, synthetic callback dispatch, actual installed-loader execution, actual CLI startup callbacks and actual native runtime tests are distinct evidence classes. Test success must not flatten these into harness parity or claim a product exists. Performance numbers in PERFORMANCE.md are targets. Native QML application interaction, full live model-driven turns, broader terminal support and release hardening remain later gates.

Original Hermes Watcher, real harness credentials/configuration and global terminal configuration are not migration targets in M0. Probes create temporary private homes/services/windows and remove only their own resources. Logout/reboot/suspend behavior is documented, not destructively tested on the user's machine.

## Verification and discovery side effect

Parent's final six commands passed: **41 tests** (10 contract checker, 10 runtime, 6 harness and 15 design), plus archive and fixture integrity checks. See `evidence/parent/final-tests.json`; those include explicit synthetic fixtures and gap-detection tests, not 41 production tests. An initial aggregate run used an incorrect Node test path; the corrected full run is the recorded final result.

Initial `mise which` discovery unexpectedly pruned old cached Claude 2.1.259 and Codex 0.153.0 installations. The deletion was observed in tool output and parent confirmed those paths absent. Current tested versions remain Claude 2.1.260 and Codex 0.153.2. Reproducible probes use installed binaries directly and never invoke mise. No silent reinstall or global configuration change was attempted. The Hermes Watcher Git tree was separately verified clean.

New-authored files pass diff whitespace checks. The byte-preserved original font `OFL.txt` has one inherited trailing-space warning; its license bytes are intentionally unchanged.

## Published checkpoint

The verified code/design artifact commit is `d2b0e78dce0affb09726e9778698a389836b52f8`. GitHub issue bodies, states and evidence comments were read back; #4/#6 are closed and #2/#3/#5 remain open. Exact REST issue states and GraphQL counts agree on three open/two closed. At this checkpoint the REST milestone aggregate still reported five open/zero closed even after cache-busted retry; that inconsistency is retained in `evidence/parent/remote-publication.json`, not represented as a successful counter check. No milestone reassignment or issue reopen/close cycling was used to force it.

## Resumption procedure

Read roadmap issue #1, the exact M0 issue bodies/comments and this status, then reproduce the documented checks. Final issue states and commit evidence on GitHub take precedence over an older local snapshot. Close only criteria with evidence and explicit approvals. M1 remains gated until M0 is actually complete or the owner explicitly revises scope.
