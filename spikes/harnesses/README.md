# Installed harness observation spike — M0 / issue #5

This is a disposable feasibility probe, **not a production adapter, installer, daemon, or release compatibility suite**. It does not start a model turn. Decisions and the per-version matrix are in [docs/harness-capabilities.md](../../docs/harness-capabilities.md).

## Reproduce

Run from the repository root on the reference Linux machine:

```sh
python3 -m unittest discover -s spikes/harnesses -p 'test_probe.py' -v
python3 spikes/harnesses/test_live.py -v
python3 spikes/harnesses/run.py --out spikes/harnesses/evidence/installed.json
```

Or run both test files together:

```sh
python3 -m unittest discover -s spikes/harnesses -v
```

No packages are installed. Required checks do not skip: missing binaries, an unusable bubblewrap namespace, loader errors, malformed protocol results, or missing observed Claude startup events cause a nonzero test result. **A green test verifies the explicitly limited experiment, not trusted Codex callback execution or three-harness parity.** The report includes the Codex blocker.

Defaults resolve these installed artifacts directly, without `mise`, launch shims, or auto-updaters:

- `$HOME/.hermes/hermes-agent` and its existing `venv/bin/python` (the tested venv links into a uv-managed Python distribution).
- `$HOME/.local/share/mise/installs/claude/latest/claude`.
- `$HOME/.local/share/mise/installs/codex/latest/bin/codex`.

Override with `--hermes-source PATH --claude PATH --codex PATH`. Review replacements first. The Hermes venv layout is intentionally reference-install-specific; another layout is a prerequisite adaptation, not a reason to fall back to the real home. A source-root `.env` causes refusal rather than mounting possible credentials. `--out` overwrites only the explicitly chosen evidence file. Binary/source paths in reports use literal `$HOME`; versions and SHA-256 values remain exact.

## What actually runs

| Probe | Actual execution | Not claimed |
|---|---|---|
| Hermes | Installed `--version`; reviewed fixture plugin discovered by actual `PluginManager`; disabled → supported `plugins enable` → loader callback dispatch → supported `plugins disable`; unrelated sentinel remains enabled | A genuine agent turn, native input/approval prompt, child spawn, or resume. Callback arguments are **synthetic fixtures**. |
| Claude | Installed `--version`; `--init-only --setting-sources user --strict-mcp-config` with reviewed disposable user hooks. Observed `Setup`, `SessionStart`, `SessionEnd`, correlated by one session ID; no stdout from hooks | Turn completion, approval/input-needed events, child lifecycle, resume, or control |
| Codex native | Installed CLI starts on a disposable PTY, no prompt/login/trust response sent. Only terminal cursor-position protocol reply is allowed. Five-second bounded observation reports the authentication gate | Reaching `/hooks`, trusting a definition, or any native hook callback |
| Codex App Server | Installed schema generator; offline `stdio://` server receives only `initialize`, `initialized`, and `hooks/list`. Native `hooks.json` definitions are listed with `trustStatus: untrusted` | A thread, a turn, a fake provider/API, an approval answer, trust mutation, or attaching to an existing terminal session |

User authorization covers reviewing and accepting our exact minimal Codex hook in a disposable environment. It does **not** remove the observed auth prerequisite: the native default-provider startup stops at login before `/hooks` is accessible. We did not configure a dummy provider or invent trusted hashes to get around it.

## Isolation and privacy

`probe.sandbox` constructs a fresh `bwrap` command with private network/PID namespaces, private `/dev`, `/proc`, `/tmp`, and empty home/runtime roots. Only `/usr`, selected installed code/runtime artifacts, and this spike are read-only mounted; `/probe` is a private `TemporaryDirectory`. Real harness profiles are not mounted. Child environment is built from an allowlist, with explicit disposable `HOME`, `HERMES_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and XDG paths. No inherited provider credentials, SSH agent, D-Bus, or display sockets are supplied. The sandbox test attempts a connection and verifies failure; it does not merely assert that a flag exists.

`observe.py` accepts at most 16 KiB of command-hook input with a one-second read deadline. It projects only known event names, bounded identifier fields, and enumerated source/surface/notification classifications. The disposable sink is at most 64 KiB, uses mode 0600, refuses final-component symlinks/nonregular files, and takes a nonblocking lock. Unknown/invalid/oversized events and sink failures return success with **no stdout or stderr**: no additional context, permission decision, tool replacement, or prompt injection. In-process Hermes callbacks return `None` and never stringify unknown values. No network is used by the observer.

Prompts, responses, tool input/output, goals, descriptions, transcript paths, and raw environment are discarded. Transcript files are never opened or exported. Codex terminal output is matched locally against fixed gate phrases and then discarded; only match names and byte counts are retained. Installed harnesses may create their own startup state/logs inside `/probe`; these disappear with the temporary directory. This is not a claim that the harness itself never writes a transcript internally.

This sink is intentionally **not** a production recovery spool: drops are silent, no delivery guarantees are offered, and the fixture config helper does not protect against hostile same-UID races in ancestor directories. M1 IPC/durability work is out of scope.

## Reversible setup checks

- JSON fixture overlays append our entries while preserving unrelated hooks and permissions. Removal restores the exact original bytes (or removes a newly created file). An intervening edit prevents rollback instead of overwriting new settings; symlink targets are rejected.
- Hermes uses its supported enable/disable CLI with default-denied tool override consent. Its fixture sentinel stays enabled and `display.compact` stays true. The owned observer directory is removed; exact original fixture config bytes are restored.
- Codex preserves its unrelated TOML config and unrelated existing `Stop` command. No hook trust state is written. Trust readback is not represented as trust acceptance.
- Before/after config/hook SHA-256 values are recorded. All test processes are bounded and reaped; Codex native teardown uses SIGTERM after the no-input observation. App Server did not exit on stdin EOF during the three-second grace period and is killed in its private namespace; this is **probe cleanup**, not a proven application control operation.

## Evidence files

- `evidence/installed.json`: reproducible version commands, executable hashes, sanitized events, loader results, restoration hashes, native gate result, App Server readback, and installed schema method inventory.
- `evidence/source-references.json`: hashed installed-source excerpts and version-pinned upstream Codex excerpts. **Source-derived, not runtime evidence.** One initial guessed subagent source URL returned 404; the successful replacement is `events/session_start.rs` (`StartHookTarget::SubagentStart`).
- `evidence/verification.json`: captured final test output and programmatic artifact checks.
- `evidence/development-notes.json`: observed RED→GREEN slices and discovery caveats; not fabricated test logs.

## Pitfalls discovered

1. A preliminary bubblewrap layout exposing a read-only host `/dev` made Claude's Bun runtime abort even on `--version`. Private `--dev /dev` fixed the reproducer; private `/proc` alone did not. The final sandbox mounts no host device tree.
2. A uv venv may point through an **unversioned** Python distribution symlink; mount its target at the path the venv actually references. Mounting only the fully resolved versioned path was insufficient.
3. Hermes uses `--version`, not a `version` subcommand. Codex's hook trust field is `trustStatus`, not a guessed `needsReview` boolean; tests use the actual returned schema/values.
4. `mise which` unexpectedly performed housekeeping and pruned old Claude 2.1.259 and Codex 0.153.0 copies during initial discovery. This external side effect is disclosed, not attributed to the sandbox. The reproducible scripts never invoke mise and did not modify real harness profiles/config/auth. No global package installation, commit, push, or GitHub write was performed.
