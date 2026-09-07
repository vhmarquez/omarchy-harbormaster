# M2 #13: retry and verified owned terminal controls

PR #54 is owner-merged at `82096a3a3c302323d7a184ea127741ec3c544d61`.
Its actual main Docker report and unchanged native report are qualified. The
owner authorized continuation after the pause. This increment implements #13;
approval of its PR/revision remains required before merge or dependent work.

## Responsibilities recorded before implementation

- `runtime/process`: held pidfds, boot/start/UID identity and bounded graceful
  signaling. Never signal a discovered numeric PID or fall back when pidfds
  are unavailable.
- `runtime/ownership`: validate the exact private tmux server, user-service
  invocation, pane/session and pane scope relationship. Recover only persisted
  launch intent; never infer ownership from a unit name or window title alone.
- `runtime/terminal`: standalone foot activation and exact Hyprland association,
  client parentage/TTY/session checks, repeat activation reconciliation and focus
  readback. Store only association metadata, never titles or terminal content.
- `storage`: persist bounded launch/control intent and observed identities on
  the existing worker, before external effects. Preserve legacy runner records
  through migration; unresolved legacy attempts remain uncertain.
- `manager`: serialize revision-guarded launch/retry, Open/Attach/End operations
  and warn before a second managed writer uses the same registered project root.
  Explicit acknowledgement permits sharing; no Git mutation or worktree creation.
- `cli`: expose these commands and explicit unavailable capability results.
  Resume stays unavailable until a native harness adapter supports it.

Dependency direction remains CLI -> manager -> runtime/storage. Runtime and
storage exchange typed intent/identity values; reducers and producer events gain
no control authority. Process signaling and compositor dispatch stay at narrow
effect boundaries. No new external package or speculative adapter framework.

## Implementation limits and verification

Use the accepted standalone foot/Hyprland path from ADR 0002. A separately owned
terminal service survives manager lifetime. Open focuses a verified association
or attaches to the existing runtime; Attach never creates another harness job.
Unknown launch/activation outcomes are reconciled conservatively, not retried
blindly. End requests graceful termination through held identities only, with
a deadline and no numeric-PID or wildcard fallback. Unsupported actions return
explicit errors. Broader manager crash/socket recovery remains #15.

Use focused checks for consequential identity/retry failures, one practical
synthetic workflow demonstration, one independent review and unchanged mandatory
Docker/native qualification. A desktop demonstration, if run, uses only new
disposable foot windows and temporary focus with restoration; it does not change
desktop configuration, touch real harnesses, install anything or collect content.
Keep raw evidence outside source. No broad test campaign or evidence-only PR.
