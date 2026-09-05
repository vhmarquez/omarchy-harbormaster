# ADR 0001 — Local-first native agent management

Status: M0 architectural decision; implementation and deployment not authorized by this record.
Issue: [#2](https://github.com/vhmarquez/omarchy-harbormaster/issues/2)

## Decision and scope

The product name is Harbormaster; the repository is `vhmarquez/omarchy-harbormaster`. The selected user-reviewed design is option 02, Project manager. A compact bar popup prioritizes attention; an expanded native window organizes projects, sessions and review. Brand vocabulary does not replace normal task/project/session terminology.

Initial release architecture is multi-harness, Hermes-first, then Claude Code and Codex to validate the shared boundary. Capability parity is not a promise. Each adapter exposes only operations/signals supported by tested versions. An unproven optional hook or control is disabled, not guessed; an unproven required runtime assumption blocks that implementation path.

### Namespaces

| Purpose | Chosen namespace |
|---|---|
| CLI | `harbormaster` (availability/package registry reservation not claimed) |
| Daemon | `harbormaster daemon`, one executable with explicit modes |
| QML-to-daemon bridge | `harbormaster bridge`, long-lived bounded stream |
| Omarchy plugin identifier | `vhm.harbormaster` |
| Config | `$XDG_CONFIG_HOME/harbormaster/config.toml` |
| Metadata | `$XDG_STATE_HOME/harbormaster/state.db` |
| Runtime | `$XDG_RUNTIME_DIR/harbormaster/` |
| Control / event socket | `control.sock` / `events.sock` beneath private runtime directory |
| Runtime service family | `harbormaster-runtime-<validated-id>.service` |
| Manager service | `harbormaster.service`; never owns harness service cgroups |

XDG config/state fallbacks are `~/.config` and `~/.local/state`. No insecure shared `/tmp` fallback for a missing runtime directory: fail with an actionable diagnostic. State directories are owned by the desktop user, mode 0700, regular files 0600; validate types/ownership and avoid symlink races. Exact per-session runtime service structure follows ADR 0002 and is not implied to be installed.

## Stack and alternatives

- Rust daemon/domain/CLI: typed state, bounded async Tokio I/O, Serde framing, clap commands, sanitized tracing. Prefer rustix for narrow platform interactions; domain code forbids unsafe. A small reviewed FFI boundary remains part of the threat model.
- Qt Quick/QML in installed Omarchy/Quickshell: native fonts, palette, keyboard and pointer behavior. Thin presentation layer; durable decisions belong in the daemon. Long-lived stdio-to-socket bridge avoids introducing Rust-to-Qt FFI or process-per-refresh polling.
- SQLite metadata owned by the manager, one bounded blocking worker and short transactions. WAL remains conditional on dependency/version advisory review in M1; supported live backup API rather than copying a live main DB. No harness database writes or network-share database.
- Existing terminal plus separate namespaced tmux/user-service runtime: decision contingent on ADR 0002 evidence. No custom terminal/multiplexer.
- Minimal Hermes Python observer; Claude/Codex command hooks are observational and use negotiated metadata capabilities. No dynamic third-party daemon plugins in v1.

Python would speed an incremental Watcher update, but this is a new supervisor with explicit security/resource constraints. Go is credible, but Rust better fits explicit ownership and typed state design. Neither language prevents logic bugs or unauthorized process control. Electron/webview and a custom PTY stack add unnecessary implementation/security surfaces. No measured Rust speedup has been established by M0.

## Product and responsibility boundaries

Projects, user-authored task labels, harness profiles, conversations, runs and turns are distinct identities. Profile creation is not automatic per task. Managed sessions have proven launch ownership; discovered sessions are not silently adopted or terminated. Open focuses a known native terminal; Attach connects to an existing managed runtime; Resume launches a new run from saved context only after excluding a still-live original.

An observed finish is not task acceptance. Process liveness, observation freshness, turn outcome, attention, user review and notification state are independent. Unknown is not idle; stale observation never authorizes launching a duplicate process or killing a target. Only verified owned runs can be terminated, with graceful shutdown and bounded escalation. PID alone is not identity.

Task labels are metadata, not automatic prompts. Explicit prompt transport, if later added, must use a supported private channel, not shell interpolation or assumed-secret process arguments. Native harness permission policy and approval UI remain authoritative.

Initial workflows cover trusted project registration, explicit checkout/worktree launch, native focus/attach/resume, attention, search, review links, restart reconciliation and truthful capability limitations. Cross-machine development uses Git and reproducible checks, not live database synchronization.

## Non-goals

No remote management, cloud sync, credential broker, custom multiplexer, arbitrary plugin execution, automatic approvals, automatic commit/push/merge, automatic dirty-worktree deletion, embedded transcripts/chat, autonomous task scheduling, or stronger OS isolation in v1. A user choosing a worktree chooses a separate checkout, not a security sandbox.

## Licensing

The owner explicitly selected **MIT** on 2026-09-05; the project grant is in [LICENSE](../../LICENSE) and the response is preserved in the [owner-decision record](../M0-OWNER-DECISIONS.md). The third-party design font retains its upstream OFL and is not relicensed; archived user-supplied design material retains its [provenance and licensing caveats](../design/PROVENANCE.md). Dependency/license compatibility must be audited against actual pinned dependencies in M1, especially Qt/Quickshell and distribution obligations; selecting a permissive project license does not remove those obligations.

## Consequences and gates

The [M0 exit record](../M0-STATUS.md) now combines runtime feasibility, explicit per-harness limitations, owner-approved design, domain/performance contracts and the MIT choice. Codex Limited visibility is accepted; trusted native callbacks remain unqualified rather than assumed on the critical path. M1 still requires a separate work authorization and remains unstarted. Routine reversible details within an authorized milestone may be settled by the implementation owner. M0 does not install a runtime or claim production readiness. Source/version inspection and fixture tests are labeled separately from actual harness execution.
