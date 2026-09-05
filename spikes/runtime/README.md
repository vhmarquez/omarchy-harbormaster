# M0 runtime feasibility — issue #4

## Verdict: PARTIAL — GO for the tested, narrowed runtime

The **real tmux/systemd/foot/Hyprland path is VALIDATED on the versions below**. A detached worker survived UI-service restart, daemon-service restart, daemon SIGKILL/recreation, and closing/reopening its terminal. Exact attach, exact focus, negative identity checks and cleanup passed. This is a disposable architecture experiment, **not an implemented Rust daemon, QML UI, harness integration or production security boundary**.

The M0 go/no-go decision is [ADR 0002](../../docs/adr/0002-runtime.md). Compatibility beyond standalone foot on the tested Hyprland version remains unvalidated; there is no generic “any terminal” promise.

## Reproduce

From the repository root, in the existing desktop user's session:

```sh
# Pure identity tests; no desktop or service side effects.
python3 -B -m unittest discover -s spikes/runtime -p test_identity.py -v

# Actual-finalizer fault injection; no services, desktop or default tmux access.
python3 -B -m unittest discover -s spikes/runtime -p test_cleanup.py -v

# Real transient user services and isolated tmux; no window creation.
./spikes/runtime/probe.py --phase lifecycle \
  --output spikes/runtime/evidence/lifecycle-rerun.json

# EXPLICIT DESKTOP OPT-IN: opens two sequential disposable foot windows,
# temporarily changes focus, closes only those windows, restores original focus.
./spikes/runtime/probe.py --phase desktop --allow-desktop \
  --output spikes/runtime/evidence/desktop-rerun.json

# Full opt-in integration suite (includes the desktop operations above).
python3 -B -m unittest discover -s spikes/runtime -p 'test_*.py' -v
```

No package install, sudo, model call, credentials or network is required. Requires Python 3 stdlib, tmux, systemd-run/systemctl/loginctl and an accessible user manager; desktop mode additionally requires standalone `foot`, `hyprctl`, a live Hyprland session and a stable initially active application window. Do not interact with/close the starting window during the brief desktop probe. Missing required prerequisites fail; they are not silently skipped. `--phase desktop` without `--allow-desktop` fails before creating resources.

For a full suite that writes both canonical evidence files:

```sh
HARBORMASTER_EVIDENCE=spikes/runtime/evidence/lifecycle.json \
HARBORMASTER_DESKTOP_EVIDENCE=spikes/runtime/evidence/desktop.json \
python3 -B -m unittest discover -s spikes/runtime -p 'test_*.py' -v
```

`--output` writes the sanitized observation JSON; process exit is nonzero unless the selected probe completes and every recorded check/cleanup assertion passes. Each canonical result records code SHA-256s, version discovery, executed argv/return codes, initial unit properties, heartbeat observations, and cleanup readbacks. The command batches executed *inside* the daemon are recorded separately.

## Tested environment

| Component | Observed version / support |
|---|---|
| Omarchy | 4.0.2-1 |
| Hyprland | 0.56.2, commit `efb50993780079460b0cbed1363e2166a2de1d9f` |
| tmux | 3.7c; Arch package `3.7_c-1`, systemd-enabled |
| systemd | 261 (261.2-1-arch) |
| foot | 1.27.0, standalone executable `/usr/bin/foot` |
| Python | 3.14.7 |
| Linux | 7.1.9-arch1-2 |
| kitty / alacritty / ghostty | Not installed; NOT tested or claimed supported |
| foot server / footclient | Not tested; shared terminal PID needs another association contract |

These are observations, **not minimum version ranges**. See `evidence/desktop.json` and `evidence/lifecycle.json` for recorded UTC timestamps. `evidence/tests.json` records a successful four-test suite. The desktop result contains 18 passing checks and six passing cleanup checks.

## Given / When / Then results

| Given | When | Then (real observation) | Verdict |
|---|---|---|---|
| A private socket and transient runtime launched by a disposable daemon | Inspect process cgroups and unit links | tmux server, daemon and UI have different cgroups; pane scope is `PartOf` the runtime, not daemon/UI | VALIDATED |
| A heartbeat worker in a detached tmux session | Restart the UI stand-in and daemon stand-in | Original worker and tmux server keep the same PID/start-ticks/boot identity; heartbeat advances | VALIDATED |
| The same runtime with an attached terminal | SIGKILL daemon, recreate its collected transient unit, reconcile existing identity | Worker/server survive, attachment stays valid, exactly one existing session remains | VALIDATED |
| A newly launched standalone foot with a unique app-id | Verify window PID/app-id and tmux client parent PID/session ID/TTY; switch away and focus exact window | Exact attachment and compositor active-window readback match | VALIDATED |
| An owned terminal association | Mutate address, PID, app-id, start-ticks, boot ID or compositor identity | All rejected before dispatch; dispatch count and focused address unchanged | VALIDATED |
| A valid tmux client and exact target | Check a wrong expected session ID and an absent exact session | Neither resolves as a valid association; no alternate/default-session fallback | VALIDATED |
| A valid disposable terminal | Close it, attempt stale focus, then launch a fresh foot attached to the same session | Stale focus sends no dispatch; worker keeps running; new client attaches to the same session | VALIDATED |
| A verified runtime unit | Stop that runtime explicitly | Worker/server end while the recovered daemon remains alive | VALIDATED |
| Completed probe or caught failure | Close owned children, stop exact units, remove own temporary directory | Units unload, live owned process identities disappear, windows close, original focus restores, default tmux/config metadata unchanged | VALIDATED in final runs |

The UI is `/usr/bin/sleep infinity` in a real transient service. The daemon is a small Python fixture that invokes `systemd-run` for the runtime, then waits; its restart verifies/reuses a private saved server identity. The worker only atomically updates a local heartbeat file. No fabricated Rust/QML integration, database reconciliation, agent reasoning, native approvals or task completion is claimed.

## What actually runs

Each run uses a fresh mode-0700 `$XDG_RUNTIME_DIR/hb-m0-*` directory, a 12-hex nonce, and three temporary services:

- `harbormaster-m0-runtime-<nonce>.service`: foreground tmux server;
- `harbormaster-m0-daemon-<nonce>.service`: disposable launcher/reconnector;
- `harbormaster-m0-ui-<nonce>.service`: UI lifecycle stand-in.

All have explicit `Type=exec`, `KillMode=control-group`, `Restart=no`, `UMask=0077`, null stdout/stderr, `TimeoutStopSec=3`, `RuntimeMaxSec=180`, and `--collect`. The 180-second cap is a **probe fail-safe, not a product lifetime policy**. No persistent unit file is created or enabled. Runtime startup is issued *from inside the daemon cgroup* to demonstrate that the user manager, rather than daemon process ancestry, owns the runtime lifecycle.

Representative argv below uses the script's generated paths/units, not user input; run the script rather than pasting undefined placeholders:

```text
systemd-run --user ... --unit=harbormaster-m0-runtime-<nonce>.service \
  -- /usr/bin/tmux -D -f /dev/null -S <private-dir>/tmux.sock

tmux -S <private-dir>/tmux.sock new-session -d -s managed -c <private-dir> \
  /usr/bin/python3 -B <spike-dir>/probe.py fixture-worker <private-dir>

foot --config=/dev/null --log-level=none --log-no-syslog \
  --app-id=org.harbormaster.probe.<nonce>.first \
  --title='Harbormaster disposable M0 probe' \
  tmux -S <private-dir>/tmux.sock attach-session -t =managed

tmux -S <private-dir>/tmux.sock display-message -p -t =managed: '#{session_id}'
tmux -S <private-dir>/tmux.sock list-clients -F '#{client_pid}\t#{session_id}\t#{client_tty}'
hyprctl -j clients
hyprctl dispatch 'hl.dsp.focus({window="address:0x<verified-hex-address>"})'
hyprctl -j activewindow
```

Subprocesses receive argv arrays, never shell-interpolated task text. The only Lua expression is fixed syntax containing a validated hexadecimal compositor address. `-f /dev/null` applies only to the private tmux server; `--config=/dev/null` applies only to the new foot instances. `TMUX`/`TMUX_PANE` are removed from fixture subprocess inheritance. The general production environment allowlist is **not** settled by this experiment.

## Important findings and failed hypotheses

`evidence/investigation.json` preserves selected fields from actual pre-fix failures, including failed cleanup focus; it is not the current acceptance result.

1. **Pane cgroup ≠ server cgroup on this tmux build.** Arch's systemd-enabled tmux creates a `tmux-spawn-<uuid>.scope` for the pane. Its `PartOf` points to the independent runtime service. The original equality assertion failed; the corrected ownership assertion verifies that actual dependency. Stopping the runtime really ended the pane. Production accounting/termination must include associated pane scopes, not assume all harness PIDs live under the server's cgroup.
2. **Hyprland's old dispatch spelling does not work here.** `hyprctl dispatch focuswindow address:...` returned 7 and a Lua parse error. Consequently early attempts could not explicitly restore focus. The supported 0.56.2 form is `hyprctl dispatch 'hl.dsp.focus({window="address:..."})'`; final runs verify restore succeeds. No persistent Hyprland settings were changed to make it work.
3. **tmux target types matter.** `display-message -t =managed` returned an empty session ID in the detached setup although `has-session -t =managed` succeeded. The pane-target form `=managed:` resolved `$0`; exact *session* attach continues to use `=managed`. The script now rejects an empty/malformed session ID immediately.
4. **`tmux -D` takes no command.** Start the foreground server first, then create the session through its explicit socket. It sets `exit-empty` off; the independent unit owns final server shutdown.

Identity tests were written/run red before the helper was implemented, then extended red for stale/ambiguous identities and malformed selector strings. Lifecycle and desktop integration tests were also run failing before their respective paths were implemented. These are disposable probe tests, not a claim that production runtime/security tests exist.

## Cleanup and privacy

Cleanup uses only retained Popen children, exact generated service names, and the private directory returned by this invocation's `mkdtemp`. It does **not** glob-kill tmux, close arbitrary windows, use `send-keys`, type into live agents, inspect terminal output, restart the user's terminal service, or touch harness profiles/configuration. The pane scope is accepted for cleanup only after its `PartOf` identifies this run's runtime. Unit `LoadState` is read back after stop; process liveness, absence of owned windows, and original focus are checked. The default tmux socket is never connected to: only socket/config stat metadata is compared before/after. That stat check is evidence of no metadata change, not transcript/session inspection.

`hyprctl` JSON necessarily includes more compositor fields; the script immediately selects only address/PID/class and never persists window titles. The original window's address is replaced with `$ORIGINAL_WINDOW`; home/probe/source paths are redacted in JSON. Boot IDs and original-window identity are not exported. Heartbeats contain no harness content. No screenshots or terminal transcripts are captured.

`evidence/cleanup-audit.json` is an additional read-only audit of exact resource names from the recorded runs. A failed run must be inspected, not relabeled successful. Catchable cleanup exceptions (including command timeouts) are recorded in `cleanup_errors` with stage, exact target when applicable, exception type and message. Each owned child/unit cleanup, readback, scratch removal and evidence export is attempted independently; a failed readback cannot skip later targets. Any such exception keeps the verdict `INVALIDATED` and exit status nonzero even if later readbacks succeed. The original probe error is retained. If JSON export itself fails, the sanitized stdout summary reports that failure; no successful evidence-file write is claimed. The fault tests execute the actual finalizer with injected external boundaries, not destructive live timeout scenarios.

Cleanup attempts do not guarantee resource removal: a hard kill/power loss cannot guarantee immediate cleanup/focus restoration either. Probe services have the bounded lifetime cap, but a hard-killed probe can leave its private scratch directory until the user runtime directory is removed. Do not perform wildcard cleanup: verify ownership/identity of the exact recorded resource before any manual intervention.

## Limits: logout, suspend, reboot, production

- **Logout:** not performed. `loginctl show-user <uid> -p Linger --value` observed `yes` already; it was not changed. User-manager lifetime, logind policy, and graphical-session dependencies govern survival. Detached tmux is not itself a logout guarantee. Product policy must be an explicit user choice; never silently enable lingering.
- **Suspend:** not performed. Suspended processes do not keep computing. Wake requires liveness/connection revalidation; network sessions can break. Nothing here proves a sleep/wake adapter contract.
- **Reboot/power loss:** not performed. In-memory tmux/harness processes do not survive. Recover metadata and offer supported native resume/new launch choices with new boot/process identities; do not label that continuous execution.
- **Other lifecycle failures:** killing the runtime intentionally ends its worker. Runtime-server crashes, user-manager crashes, session races, multiple simultaneous runs, compositor restart, and production crash/disk recovery require later implementation tests.
- **Security/control:** PID/start/boot + app-id + compositor/address association is a safety check, not same-UID isolation or an atomic compositor transaction. There remains a snapshot-to-dispatch race. No generic interrupt, approval automation, arbitrary discovered-session adoption, or race-free destructive control is validated.
- **Performance:** heartbeat ticks prove progress, not a throughput, latency, RSS or 20-session benchmark. Python/sleep stand-ins do not measure the planned Rust/QML stack.

## Sources

- Installed `man tmux`, `foot --help`, `hyprctl --help`, `systemd-run --help`, and `man loginctl` (queried during the run).
- [Hyprland 0.56 dispatcher API](https://wiki.hypr.land/0.56.0/Configuring/Basics/Dispatchers/).
- [tmux 3.7c systemd implementation](https://github.com/tmux/tmux/blob/3.7c/compat/systemd.c), including pane scopes, `PartOf` and `SendSIGHUP`.
- [Arch tmux PKGBUILD](https://gitlab.archlinux.org/archlinux/packaging/packages/tmux/-/blob/main/PKGBUILD) (`--enable-systemd`; installed package independently queried).
- [systemd.kill](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html) and [loginctl](https://www.freedesktop.org/software/systemd/man/latest/loginctl.html). Online `latest` pages returned older header versions in extraction; installed man pages plus observed behavior are authoritative for this host.

## Post-review evidence index

Parent reran all **10 runtime tests** after the cleanup fix; see `../../evidence/parent/final-tests.json`, `../../evidence/review/final-runtime-review.json` and `../../evidence/parent/final-cleanup.json`. The canonical lifecycle/desktop JSON now fingerprints the fixed source. Earlier `evidence/tests.json` and `evidence/cleanup-audit.json` are historical, not the latest acceptance run. Sequential cleanup can exceed an outer runner deadline if many underlying operations hang; per-call bounds and RuntimeMaxSec are not an aggregate cleanup guarantee. Design and test an aggregate budget before production use.
