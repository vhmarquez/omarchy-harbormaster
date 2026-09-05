# ADR 0002 — Independent terminal runtime, narrow verified attach/focus

- Status: **Accepted for M0 feasibility; constrained GO**, not deployed
- Issue: [#4](https://github.com/vhmarquez/omarchy-harbormaster/issues/4)
- Depends on: [ADR 0001](0001-product-and-architecture.md), issue #2
- Evidence: [runtime spike](../../spikes/runtime/README.md), [desktop](../../spikes/runtime/evidence/desktop.json), [lifecycle](../../spikes/runtime/evidence/lifecycle.json), [tests](../../spikes/runtime/evidence/tests.json)

## Context

Harbormaster must preserve a managed interactive session when its Rust daemon or native QML UI closes/restarts. It must distinguish focus of an existing associated terminal, attachment to a detached managed runtime, and resume of a saved harness conversation. A terminal/title match alone must not authorize control.

A real disposable spike tested tmux 3.7c (Arch 3.7_c-1), systemd 261, standalone foot 1.27.0, Hyprland 0.56.2 and Omarchy 4.0.2-1. A Python daemon and a sleep-based UI service deliberately stood in for the unimplemented product; the services, cgroups, terminal window, tmux client and continuously progressing worker were real. The overall compatibility verdict is PARTIAL; the tested lifecycle and terminal path is VALIDATED.

## Decision

**GO** with an independent user-service runtime and external terminal interaction. Keep the planned Rust supervisor/CLI and QML presentation stack; do not embed a terminal emulator or make the daemon own the harness's lifetime cgroup.

1. **Runtime ownership is independent of daemon/UI.** Use the product namespace from ADR 0001: `harbormaster-runtime-<validated-id>.service`, separate from `harbormaster.service`. The initial supported topology is one managed runner's tmux server/session per runtime unit with a private explicit socket under `$XDG_RUNTIME_DIR/harbormaster/`; e.g. `runners/<validated-id>/tmux.sock`. The one-runner spike validates this ownership pattern, not multi-runner scaling. Never place runtime under daemon `PartOf`/`BindsTo`, and do not “fix” lifetime by setting `KillMode=none` or `process` on the manager.
2. **Let the user manager start a foreground tmux server.** The proven command shape is `tmux -D -f /dev/null -S <private-socket>`, followed by a separate argv-based `new-session` request. `-D` does not accept a command. No default tmux socket/config is modified or adopted. Use a mode-0700 user-owned runtime directory and explicit paths; a missing/unsafe runtime directory is an actionable failure, not a shared `/tmp` fallback.
3. **Account for systemd-enabled tmux pane scopes.** The installed tmux creates `tmux-spawn-<uuid>.scope` with `PartOf=<runtime-unit>`. Harness PID cgroup equality with the server is false on this build. Reconcile the verified pane and scope association; include those scopes in resource accounting. Stopping the runtime propagated to its worker in the real probe; daemon/UI restarts did not. Do not broad-match or kill arbitrary `tmux-spawn-*` units.
4. **Narrow initial terminal support to standalone foot on the tested Hyprland API.** Spawn a new foot with a per-runner/window nonce app-id and a fixed executable/argv. Verify the resulting compositor PID/app-id and tmux client PID parentage, target session ID and terminal TTY. Keep process boot/start identity and compositor/window identity. Focus requires exact verified association; attach explicitly uses the managed private socket and exact session target. Do not infer association from title/class alone or fall back to another terminal/session.
5. **Use the tested Lua dispatcher, with readback.** On Hyprland 0.56.2, `hyprctl dispatch 'hl.dsp.focus({window="address:0x..."})'` works; old `focuswindow address:...` fails. Only validated hex addresses may enter that fixed expression. Verify the compositor active window after dispatch. The snapshot/dispatch race is not eliminated by this spike; later production code must preserve fail-closed identity/revalidation semantics and avoid destructive window operations.
6. **Daemon recovery reconnects, not duplicates.** Revalidate saved runner identity and query the exact private tmux target before associating a restarted daemon. Distinguish stale/missing runtime from current live runtime. The probe reconnected after restart/SIGKILL without changing the server/worker identity or creating another session. Durable DB reconciliation/idempotent launch is still an implementation requirement, not something this temporary JSON fixture proves.
7. **Runtime shutdown is a distinct explicit operation.** Closing UI or terminal is not “end session.” Stopping the runtime is allowed to end the owned worker, subject to the future verified ownership/graceful-termination contract. No generic keystroke-based interrupt/approval, discovered-session termination or native conversation resume is authorized by this ADR.

The probe's `harbormaster-m0-*` unit names, `$XDG_RUNTIME_DIR/hb-m0-*` directories, `/dev/null` terminal config, and 180-second `RuntimeMaxSec` are **disposable test choices**, not installed product configuration or a requested change to the user's preferred terminal.

## Observed evidence

The desktop probe recorded 18 passing checks and six passing cleanup checks. The initial four identity/integration tests passed; independent review then exposed a cleanup-timeout defect outside that happy-path coverage. A fresh-context fix added six fault tests and made cleanup/readback/export attempts independent. Parent reran the resulting ten-test runtime suite, including real lifecycle and desktop tests: all passed. Updated canonical probes and `evidence/parent/post-review-runtime-*.json` match the fixed source hashes; the earlier `evidence/tests.json` records the historical four-test run. Both canonical probes embed source hashes and executed argv/return codes.

- Separate actual daemon, UI and server service cgroups; verified pane scope tied to runtime.
- Same worker/server boot/start/PID identity and increasing heartbeat through UI restart, daemon restart, daemon SIGKILL and daemon recreation.
- Exact live foot/tmux association remained attached during those restarts.
- Explicit focus after switching away, verified through compositor readback.
- Wrong address/PID/app-id/start/boot/compositor associations rejected without a focus dispatch; wrong/missing session targets rejected.
- Closing the owned terminal did not end the worker. Stale window association was refused. A fresh owned terminal attached to the original runtime.
- Stopping the runtime ended the worker/server, not the recovered daemon.
- Final runs removed their services, associated pane scope, windows and private directory; original focus restored; default tmux/config stat metadata unchanged. No default-server session or transcript was read.

[Investigation evidence](../../spikes/runtime/evidence/investigation.json) retains actual failed intermediate assumptions: same-cgroup expectation, obsolete Hyprland dispatch syntax (including failed explicit focus restoration), and the difference between tmux exact session and pane targets. Final green results do not erase these compatibility findings.

## Alternatives and no-go boundaries

| Alternative | Decision |
|---|---|
| tmux launched directly in the manager cgroup | NO-GO for the continuity promise: default service stop kills that ownership domain. Use independent user-manager activation. |
| `KillMode=none`/`process` to leave children behind | Rejected: loses coherent ownership/cleanup instead of designing it. |
| Independent tmux runtime + external foot | GO on the measured matrix; native harness UI remains authoritative. |
| Arbitrary installed terminal / footclient shared server | NO-GO until separate identity and attach/focus probes pass; no compatibility is invented. |
| New PTY emulator/embedded terminal or deep agent protocol control | Out of first-release scope; this evidence does not justify adding it. |
| Automatic survival/resume across reboot | NO-GO as a continuous-process claim; only metadata recovery and explicitly supported native resume/new launch are legitimate. |

## Logout, suspend and reboot policy

These disruptive events were **not performed**. The host already reported `Linger=yes`; no lingering setting, user-manager policy, profile, service enablement or desktop config changed.

- UI/daemon restart continuity is demonstrated. **Logout continuity is conditional**, not implied: user-manager lifetime/logind policy and graphical-session dependency choices matter. Make logout behavior an explicit user choice; never enable lingering silently. Respect an existing policy without treating it as a fresh authorization to change it.
- Suspend does not continue computing; on wake, revalidate runtime/terminal/network state rather than synthesize progress. Sleep/wake behavior remains a later test gate.
- Reboot/power loss destroys in-memory processes. Recover metadata with new boot identities and offer resume/new-launch choices supported by each harness. A resumed conversation is not the same running process.

## Consequences and remaining gates

No critical M0 terminal-persistence assumption remains dependent on a fabricated integration **within the narrowed matrix**. Issue #4's disposable feasibility/evidence gate can close on this basis. This ADR is not an M1 implementation, release-support matrix, performance claim or deployment authorization.

Before shipping managed runtime, M2 must implement and exercise durable registry/reconciliation, idempotent launch ambiguity, pidfd/identity-aware ownership and termination, same-UID/path/race defenses, multiple-runner accounting, runtime-server crashes, actual Rust/QML restart tests, environment inheritance, and install/uninstall recovery. M8 must rerun the disposable matrix on supported versions and cover logout policy, sleep/wake and reboot recovery in an authorized disposable environment. Race-free destructive controls remain unavailable until independently proven.

Same-UID arbitrary code can spoof/access user resources; app-ids, tmux and user services are not an OS isolation boundary. The observation/control distinction and native harness approval policies remain unchanged.
