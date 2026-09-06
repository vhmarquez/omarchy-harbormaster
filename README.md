# Harbormaster

An Omarchy-native, local-first agent manager for concurrent Hermes, Claude Code and Codex work.

**Stage: M0 feasibility/design accepted; M1 #7 is merged and post-merge qualified; #8 protocol/IPC work is in progress on a feature branch. This repository does not yet contain an installable manager.** See [M1 status](docs/M1-STATUS.md), [development boundaries](docs/DEVELOPMENT.md) and [verification](docs/M1-VERIFICATION.md). The approved composition is option 02, Project manager: project navigation, session list and inspector, plus an attention-first bar popup, with the supplemental review/settings/recovery study approved. A finished agent turn is not proof the work is correct.

## Project tracking

- [Roadmap and product contract](https://github.com/vhmarquez/omarchy-harbormaster/issues/1)
- [Milestones](https://github.com/vhmarquez/omarchy-harbormaster/milestones)
- [Current M0 status and acceptance evidence](docs/M0-STATUS.md)
- [Owner decisions: MIT, design approval and Codex Limited visibility](docs/M0-OWNER-DECISIONS.md)

## M0 artifacts

| Area | Artifact |
|---|---|
| Scope, namespaces, architecture, license status | [ADR 0001](docs/adr/0001-product-and-architecture.md) |
| Security and boundaries | [Threat model](docs/THREAT-MODEL.md) |
| Original selected design and supplemental study | [Design provenance](docs/design/PROVENANCE.md), [original HTML](docs/design/original/index.html), [supplemental HTML](docs/design/supplemental.html), [UX contract](docs/design/UX-CONTRACT.md) |
| Terminal ownership, focus and restart feasibility | [Runtime spike](spikes/runtime/README.md), [ADR 0002](docs/adr/0002-runtime.md) |
| Installed harness observations and limitations | [Harness spike](spikes/harnesses/README.md), [capability matrix](docs/harness-capabilities.md) |
| Typed domain and wire draft | [Protocol v0](docs/PROTOCOL.md) |
| Reference workloads and unmeasured targets | [Performance](docs/PERFORMANCE.md) |
| Critical future regression requirements | [14 synthetic failure specifications](contracts/failure-fixtures.json) |
| Parent's verification scope and caveats | [Verification](docs/M0-PARENT-VERIFICATION.md) |

Open HTML studies locally with their adjacent assets. GitHub renders the text and images but does not run a raw HTML prototype. All design data is fictitious; static design boards are not browser screenshots.

## Reproduction

Fixture-document checks (Python standard library, no credentials/network):

    python3 -B -m unittest discover -s tests -v
    python3 -B scripts/verify-m0.py

The checker validates the failure-specification inventory. A green result is not proof the future Rust daemon passes these regressions or that all M0 acceptance gates are complete. Runtime/harness/design commands and dependencies are documented separately in their READMEs; actual desktop tests use disposable resources and briefly open their own terminal window. Review those scopes before running them. Rust/QML application setup and full CI begin in M1.

## Scope and safety

The proposed implementation is Rust + native Qt Quick/QML + manager-owned SQLite and bounded local Unix sockets. Agent jobs have a separate terminal/runtime lifecycle so UI/daemon closure need not end work. Capability support follows installed-version evidence, not UI illustrations. Native approvals remain in the native harness interface.

No application service, real-profile observer, deployment, credential transfer or Watcher migration is installed by these documents/spikes. Git worktrees provide separate checkouts, not a security sandbox. Strong isolation from an agent running arbitrary commands as the same desktop user is outside v1.

## Licensing

Harbormaster-authored source and documentation are licensed under [MIT](LICENSE), explicitly selected by the owner on 2026-09-05. Archived user-supplied design material retains its [provenance and licensing caveats](docs/design/PROVENANCE.md); the bundled JetBrains Mono font remains under its unchanged [OFL](docs/design/original/assets/OFL.txt). MIT does not supersede third-party asset/dependency rights or future Qt/Quickshell distribution obligations.
