# M2 #12: runnable manager and independent terminal runtime

PR #53 is owner-approved, merged and qualified at main
`59880ff75804c9fcf1d69df618e571c0f97a960e`. The owner authorized proceeding to
#12. Each subsequent PR/revision still requires approval before merge or
its dependent implementation. This increment implements #12 only.

## Responsibilities recorded before implementation

- `manager`: runnable bounded control reactor over the existing private IPC,
  a bounded operation worker, revision-guarded registry/launch/list commands
  and graceful stop. Producer events have no control authority; observation
  adapters are not activated here.
- `runtime`: fixed argv-based systemd/tmux operations, private launch handoff,
  process identity and metadata-only runtime observation. One runner has one
  independent transient user service and private socket. Manager shutdown
  never terminates a runtime. No default tmux server or configuration is read.
- `storage/runners`: durable, bounded runner reservations and identity updates
  through the existing SQLite worker. Reserve before an external launch; an
  ambiguous launch is retained for reconciliation rather than blindly retried.
- `cli`: manager serve/stop and run launch/list, routing registry commands
  through the running manager. Preserve side-effect-free argument parsing.
- Focused integration: exercise the real Rust manager and disposable terminal
  worker, including manager restart. Keep the canonical Docker/native boundary
  unchanged. A separately scoped practical service demonstration uses temporary
  product-owned resources, no live harness/profile or desktop window.

Dependency direction: CLI -> manager client/server -> storage and runtime;
control wire types -> protocol IDs and project metadata. Pure reducers remain
independent of service, filesystem and UI operations. No new external package
or product deployment is planned.

## Scope and limits

Follow [ADR 0002](adr/0002-runtime.md): foreground tmux in an independent
`harbormaster-runtime-<id>.service`, explicit private socket, `/dev/null` tmux
configuration, no manager PartOf/BindsTo relationship. Preserve native harness
approvals and fixed profile arguments; task labels never become prompts.

This increment makes one managed attempt per named task launchable/listable.
Full retry/relaunch idempotency, verified graphical focus/attach and graceful
owned termination remain #13. Broader crash/disconnection reconciliation is
#15. Actual native UI is #18; CLI client lifetime cannot own the runtime.
Unsupported/missing runtime prerequisites fail explicitly. Logout follows the
existing user-manager policy; never enable lingering. Suspend pauses computing;
reboot ends processes and cannot be described as session continuity.

Use focused consequential checks and one practical demonstration, one
independent review and the existing required Docker/native qualification.
Keep raw logs outside the repository; do not add a broad test campaign or an
evidence-only PR cycle. No install, live profile, model call, default tmux
mutation, desktop focus change or unrelated service cleanup is authorized.
