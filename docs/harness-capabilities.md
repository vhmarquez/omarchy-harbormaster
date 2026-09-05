# Harness capabilities — M0 issue #5

**Decision: conditional GO for a deliberately limited observational design; NO-GO for uniform harness control or claiming three fully verified native-hook integrations.** No M1 implementation or deployment is authorized by this spike.

The tested surface differs by harness. Claude has genuine startup/end callbacks. Hermes' actual installed plugin loader is proven with explicitly synthetic callback inputs. Codex's installed native hook discovery is proven, but its native CLI stops at authentication before a supported `/hooks` trust review can be performed in the credential-free environment. Keep that gap visible; don't convert a successful schema query into a live callback claim.

## Evidence and reproducibility

From the repository root:

```sh
python3 -m unittest discover -s spikes/harnesses -v
python3 spikes/harnesses/run.py --out spikes/harnesses/evidence/installed.json
```

See the [spike README](../spikes/harnesses/README.md) for the full isolation model, exact commands, file inventory and limitations. Reviewable evidence:

- [Installed results](../spikes/harnesses/evidence/installed.json): commands, return codes, event metadata, executable/source hashes, exact fixture restoration, native startup gate and App Server readback.
- [Source references](../spikes/harnesses/evidence/source-references.json): line-numbered, hashed installed-source and pinned upstream excerpts, separately labeled source-derived.
- [Verification](../spikes/harnesses/evidence/verification.json) and [development notes](../spikes/harnesses/evidence/development-notes.json).

A passing test asserts the limited contract and explicitly recorded blocker, not successful Codex trust acceptance, generation, permission prompting, saved-conversation resume, or runtime control.

## Tested versions — not a version range

| Harness | Exact tested build | Identity/provenance |
|---|---|---|
| Hermes | **v0.21.0 (2026.8.31)**; Python **3.11.16**; OpenAI SDK **2.24.0** | Installed Git commit `63279301bcbdc185c1b07b98a9312eb0c862f26d`; version reports upstream `79445a49`, local `63279301`, one carried commit. Local `hermes_cli/plugins.py` hash is in evidence. Do not generalize to pristine upstream or all v0.21 installations. |
| Claude Code | **2.1.260** | Native executable SHA-256 `7a2fdc74b6836ea3d183f665b869f0ee3baebc9713cbebffe5838da4ea7bd82e` |
| Codex CLI / App Server | **0.153.2** (`codex-cli 0.153.2`) | Executable SHA-256 `f8786262ebc0fa1337448a2977332beadec66c8d0cda0ce973c7849766d7943c`; generated local schema cross-checked with upstream tag `rust-v0.153.2` |

Binary paths in published evidence are normalized to literal `$HOME`, not the user's actual home path. Hashes identify the binaries, not a promise of upstream build reproducibility. Unknown versions require rerunning this spike and reviewing the contracts; capabilities must degrade rather than default to parity.

## Capability matrix

**L** = live installed behavior observed. **P** = actual installed plugin loader/dispatch with synthetic payloads, not live agent events. **S** = source/docs/help/generated-schema evidence only. **B** = blocked in this test. **U** = unproven/outside this harness test. These labels are evidence levels, not production feature flags.

Source-only Hermes cells use its event/opt-in contracts, cross-checked against hashed local source.[1][2]

Claude source-only cells use its hook reference; background controls come from the separately recorded installed help.[3]

Codex native event contracts come from documentation and the exact-version event declarations, with subagent identity in `StartHookTarget`.[4][6][9]

App Server source-only descriptions use its documented protocol, cross-checked against the installed generated method inventory.[5]

| Capability | Hermes v0.21.0 local build | Claude Code 2.1.260 | Codex 0.153.2 native terminal | Codex 0.153.2 App Server |
|---|---|---|---|---|
| Minimal startup / launch | **L** version, plugin enable/disable CLI and real loader; no agent construction | **L** `--init-only` exits 0 without model input | **L/B** native PTY reaches auth onboarding; no login/prompt/trust response | **L** offline stdio `initialize`; no managed conversation created |
| Observation transport | **P** in-process `ctx.register_hook` → `PluginManager.invoke_hook`; all callbacks return `None` | **L** command hook JSON on stdin; no stdout/decision returned | **L/B** native `hooks.json` definitions recognized and reported untrusted via read-only `hooks/list`; no command callback observed | **L** JSONL initialization/readback. This is not the native command-hook transport |
| Session lifecycle | **P/S** `on_session_start`, `on_session_finalize`, `on_session_reset`; source ties start to first/new-session prompt construction | **L** `Setup`, `SessionStart(source=startup)`, `SessionEnd`, one correlated session ID | **S/B** `SessionStart`, `SessionEnd` exist; auth gate prevented trusted execution | **S** thread lifecycle is in protocol; no `thread/start`, resume or event stream exercised |
| Turn lifecycle/outcome | **P/S** `on_session_end` is canonical **turn finalization**, not proof that the session/process ended; reduced legacy exit shapes also exist | **S** `UserPromptSubmit`, `Stop`, `StopFailure`; none invoked by this startup-only experiment | **S** `UserPromptSubmit`, `Stop`, `Interrupt`; native enum/schema verified, events not invoked | **S** turn start/interrupt and streamed lifecycle documented; no turn requested |
| User-input needed | **U** no general input-wait event proved. A turn end is not an input-required fact | **S** `Notification` (`idle_prompt`) and MCP `Elicitation`; no input-needed event observed | **U** no generic input-needed callback proved. `UserPromptSubmit` is input received, not a pending question | **S** generated `item/tool/requestUserInput` and `mcpServer/elicitation/request`; no request/answer exercised |
| Approval needed / resolved | **P/S** `pre_approval_request`, `post_approval_response`; installed source propagates session ID when available. Hooks observe, never decide | **S** `PermissionRequest` and `Notification(permission_prompt)`; actual pending permission flow untested | **S/B** native `PermissionRequest` recognized; trusted execution untested | **S** generated `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`, `item/permissions/requestApproval`; handling deliberately deferred |
| Child lineage | **P/S** explicit parent session/turn and child session/subagent IDs in `subagent_start`; `subagent_stop` available. No real child spawned | **S** `SubagentStart`/`SubagentStop`, `session_id` and `agent_id`; no full child tree or OS process mapping validated | **S** `StartHookTarget::SubagentStart` carries turn/agent identity; `SubagentStop` exists. No real child lineage validated | **S/U** deeper thread/item model exists; no live parent-child or process association validated |
| Exact-window focus | **U** requires verified runtime/window identity, not a hook name | **U** same | **U** same | **U** a protocol thread is not an existing terminal-window focus handle |
| Attach running terminal | **U**, runtime test #4 owns tmux/terminal continuity | **S/U** installed help advertises `attach <id>` for Claude's own background sessions; not tested or generalized to arbitrary terminals | **U** no arbitrary existing CLI attach proved | **U** starting an App Server is not attaching to any existing native session |
| Resume saved conversation | **S/U** CLI `--resume`/`--continue`; no saved conversation loaded | **S/U** installed `--resume`, `--continue`; no actual resume/fork performed | **S/U** source `SessionStartSource::Resume` exists; no resume performed | **S/U** generated `thread/resume`/`thread/fork`, not exercised |
| Interrupt active turn | **U** native UI/core paths not exercised; no keystroke injection adapter | **U** native Ctrl+C not tested during a turn | **S/U** `Interrupt` is a native observation event, **not an interrupt control API** | **S/U** generated `turn/interrupt`; no active turn, so no control proof |
| End/terminate owned target | **U** no harness control path qualified | **S/U** help advertises `stop\|kill <id>` for its background sessions; not tested | **U** probe SIGTERM only cleans up its own unauthenticated process | **U** probe EOF/timeout cleanup is not an application session-control test |
| Context/transcripts | Deliberately not collected | Deliberately not collected | Deliberately not collected | No history/content requests; method inventory is not permission to collect |

### Lifecycle distinctions that affect the design

1. Hermes `on_session_start` is not a generic per-turn start or process-spawn notification. Installed source `agent/conversation_loop.py:1180–1196` emits it on new-system-prompt construction; a recovery rebuild is another reason not to use it as an infallible process-identity fact.
2. Hermes `agent/turn_finalizer.py` emits `on_session_end` with turn outcomes. Closing the registry's session on every such event would be incorrect. Use surface-specific finalize/reset facts plus independent liveness reconciliation.
3. `Stop`/`SessionEnd` are not task-correctness or human-review acknowledgments. Abrupt exits can omit hook events. Missing events mean limited/stale visibility, not healthy idle.
4. Input received, a question awaiting an answer, and a permission decision awaiting a human are separate facts. No generic cross-harness input-needed capability is qualified by this spike.

## Trusted, reversible setup and data minimization

**Hermes:** the minimal plugin is authored in a disposable home, initially disabled. The real loader does not execute it before enabling. `hermes plugins enable harbormaster-m0-observer` is the supported opt-in; the optional privileged tool-override grant remains denied. Enabled callbacks emit eight projected records under synthetic dispatch. Disabling stops further observer records; unrelated sentinel callbacks remain. The original fixture configuration is restored byte-for-byte and the owned plugin directory removed. This proves the loader/bridge seam, not native turn timing.

**Claude:** reviewed hooks are installed in disposable **user** settings, not imported from an untrusted repository. `--init-only` is the documented setup-only path and emits the three observed lifecycle records with no model input. Existing `Setup` hooks and `permissions.defaultMode` are retained. No dangerous permission flags or hook decisions are used; exact settings bytes are restored afterward. This does not prove workspace-trust behavior for arbitrary project hooks.

**Codex:** reviewed observer definitions are merged alongside an unrelated existing hook; unrelated TOML remains unchanged. The real hook registry returns nine entries: eight observer event registrations plus the existing `Stop` hook, all `isManaged=false`, `trustStatus=untrusted`. The native PTY startup matches an authentication gate before `/hooks`. The user authorized scoped acceptance of the exact observer through the supported UI, but that UI cannot be reached under the tested default-provider, no-credential conditions. No credentials were copied; no fake provider, API response, trust hash, policy/managed source, or bypass was introduced. Reversibility is proven for setup/removal; **trusted native callback execution is not proven**.

The observer reads only bounded JSON input, keeps a small allowlist of metadata, and emits neither stdout nor permission/context directives. No prompts, response text, terminal transcript, tool arguments/results, credentials, or transcript files enter evidence. Fixture transcript paths are intentionally never opened. Default no-content collection remains a design constraint, not an optional later cleanup.

## Native Codex hooks versus App Server control

The installed generated hook enum contains twelve events, including **Interrupt**. A missing event in a moving documentation snapshot must not be interpreted as absence in this installed build. `hooks/list` reads **native hook configuration metadata**; it does not make those definitions trusted or execute them.

App Server is a different integration mode. Its installed schema exposes `thread/start`, `thread/resume`, `turn/start`, `turn/steer`, `turn/interrupt`, and server-to-client approval/input requests. The only live RPCs sent here are initialization and `hooks/list`. A separate reviewed user-input/approval UI would be required before Harbormaster starts an App-Server-controlled conversation. No App Server capability is evidence of authority over a discovered arbitrary native terminal session. No network listener is started; the test uses stdio.

## Go/no-go and follow-up gates

| Decision | M0 disposition |
|---|---|
| Minimal observational bridge architecture | **Conditional GO:** Claude startup path and Hermes loader seam are feasible on the exact builds. Keep event confidence/version/provenance explicit. |
| Trusted native Codex adapter on the critical path | **NO-GO until proved.** Default to **Limited visibility** with an auth/trust repair explanation. A future separately authorized authenticated disposable test must review the exact command in `/hooks`, then prove actual startup/end callbacks and reversible trust/config cleanup. |
| Uniform input/approval/lineage support | **NO-GO as a promise.** Source candidates are not live-qualified capabilities. Approvals stay in each harness's native UI. |
| Focus/attach/resume equivalence | **NO-GO.** Focus existing window, attach detached runtime and resume saved conversation remain different operations; #4 owns runtime qualification. |
| Managed control / approve / inject / terminate discovered sessions | **NO-GO.** No such adapter is implemented or validated. Event producers must not gain a control channel. |
| M1 or deployment | **Not authorized by this artifact.** |

Issue #5's minimum tests and matrix have reviewable results. The stronger statement “trusted native hooks executed on all three harnesses” remains **unmet**. Resolve M0 only by accepting this narrowed scope or by completing the blocked test; do not silently check that stronger gate as passed.

## Reference notes and caveats

- [Hermes Event Hooks](https://hermes-agent.nousresearch.com/docs/user-guide/features/hooks) and [plugin opt-in](https://hermes-agent.nousresearch.com/docs/user-guide/features/plugins); installed source excerpts are authoritative for the tested local build and have file hashes in evidence.
- [Claude hook reference](https://code.claude.com/docs/en/hooks), especially `Setup`, common fields, session, permission and subagent sections. Actual installed CLI help also advertises background attach/stop controls, but they remain untested here.
- [Codex hooks](https://developers.openai.com/codex/hooks), [App Server](https://developers.openai.com/codex/app-server), and pinned [hook event names](https://github.com/openai/codex/blob/rust-v0.153.2/codex-rs/hooks/src/lib.rs#L23-L36).
- Pinned [native auth gate](https://github.com/openai/codex/blob/rust-v0.153.2/codex-rs/tui/src/lib.rs#L1924-L1944) and [login choices](https://github.com/openai/codex/blob/rust-v0.153.2/codex-rs/tui/src/onboarding/auth.rs#L491-L519) explain why the no-auth `/hooks` path stopped. This is not a claim that every supported provider requires OpenAI auth.
- The native authentication prerequisite is supported by the pinned onboarding/login implementation, not just an inference from absent callbacks.[7][8]

**Discovery side effect:** initial `mise which` unexpectedly pruned old Claude 2.1.259 and Codex 0.153.0 installations. The scripts avoid mise entirely. No real Hermes/Claude/Codex profile/config/auth changes, global package installs, commits, pushes, or GitHub writes were made. Preliminary isolation mistakes and corrected assumptions are documented in the spike README rather than hidden behind final passing results.

## Sources

[1] https://hermes-agent.nousresearch.com/docs/user-guide/features/hooks
[2] https://hermes-agent.nousresearch.com/docs/user-guide/features/plugins
[3] https://code.claude.com/docs/en/hooks
[4] https://developers.openai.com/codex/hooks
[5] https://developers.openai.com/codex/app-server
[6] https://raw.githubusercontent.com/openai/codex/rust-v0.153.2/codex-rs/hooks/src/lib.rs
[7] https://raw.githubusercontent.com/openai/codex/rust-v0.153.2/codex-rs/tui/src/lib.rs
[8] https://raw.githubusercontent.com/openai/codex/rust-v0.153.2/codex-rs/tui/src/onboarding/auth.rs
[9] https://raw.githubusercontent.com/openai/codex/rust-v0.153.2/codex-rs/hooks/src/events/session_start.rs
