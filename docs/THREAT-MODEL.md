# Harbormaster M0 threat model

Scope: a single desktop user's local manager, trusted built-in adapters, native UI, independent terminal runtime and manager-owned metadata. No remote service or deployment exists in M0.

## Assets and trust boundaries

Assets: user files/worktrees, process ownership, native approval authority, credentials retained by harnesses, private metadata, attention/outcome integrity and desktop responsiveness. Actors include the user, manager, separately running harness processes, other local users, same-user untrusted project/agent code, malformed event producers and compromised dependencies.

Boundaries: QML bridge → control socket; observational producer → event socket; manager → filesystem/Git; registry → OS process identity; daemon → SQLite worker; outbox → desktop notification service; installer → harness configuration. Producer sockets never accept control operations. User-issued UI commands are not authorized solely because valid JSON arrived.

## Risks, controls and required evidence

| Risk | Required control | Verification surface |
|---|---|---|
| Forged/malformed events | Private socket directory, peer credentials, scoped producer/run identity, strict bounded schema | Wrong UID/type, unknown producer, oversized/nested/duplicate-key frames; no domain mutation |
| Same-UID malicious agent | Explicit limitation; no claim that socket tokens or Rust sandbox agents | Documentation; any stronger boundary requires separate OS-isolation design |
| Shell/argument injection | Fixed reviewed executables and argument vectors; labels never interpolated | Spaces, quotes, newlines, option-like text; zero unintended command execution |
| Malicious repo config / Git helpers | Explicit project trust, no implicit repo-defined hooks, constrained Git environment | Dirty/untrusted repo fixtures; external diff/textconv and helper execution excluded |
| Wrong process/window controlled | Boot + process start identity, verified owned runner and window association, pidfd where supported | Stale/PID-reuse/zombie fixtures; no unrelated process touched |
| Confused launch/retry | Idempotency ledger plus runtime reconciliation before spawn | Crash before/after launch acknowledgment; one matching runtime only |
| Filesystem attacks | Descriptor-relative safe opens, ownership/type/no-follow checks, bounded owned cleanup | Symlink/replacement/path escape tests, no unrelated files removed |
| Content/credential retention | No default prompt/response/transcript/env capture; allowlisted event metadata | Canary secrets/content rejected or stripped before spool/database/logging |
| Policy change from diagnostics | Read-only status projection; explicit policy mutation endpoint | Repeated status/doctor/export cannot change collection settings |
| Crash leftovers | Safe metadata-only bounded spool and named owned temps; centralized cleanup | Abrupt exit, privacy disable, pruning and recovery; no content-bearing leftovers |
| Dropped/reordered facts | Producer sequence/generation, terminal tombstones, durable event/outbox transaction | Late starts cannot revive completed turns; gaps visibly degrade freshness |
| Misleading notification/review | Delivery, attention and human review tracked separately | Prune/send/ack crash; folder opening cannot mark reviewed; no exactly-once display claim |
| Resource exhaustion | Bounded frame/depth/queues/disk/batches, coalescing and slow-consumer resnapshot | Active self-event, bursts, large registry, disk-full and slow bridge fixtures |
| Vulnerable dependencies | Pinned toolchain/locks/actions, advisories/license audit, minimal unsafe boundary | M1 CI gates and M8 independent review/SBOM; not yet a passed dependency audit |
| Unsafe uninstall/update | Owned-resource inventory and rollback, preserve unrelated hooks/config/worktrees | Interrupted installation/removal; independent runtime jobs not killed by UI stop |

## Honest guarantees

An arbitrary-command coding agent under the same UID may read or modify that user's data and processes. tmux, Git worktrees, Rust and per-UID sockets are not protection against this actor. Harness authentication is never imported into the manager. Removing a row does not securely erase SSD pages, WAL files, backups or snapshots: minimum collection is primary protection.

A runtime may survive UI/daemon restarts only when its independent ownership was verified. Logout policy is explicit; lingering is never enabled automatically. Suspend pauses computation, and reboot ends processes. A restored metadata row is not a restored running agent.

## Release stop conditions

Block release for an unbounded private-data path, wrong-target control, permission bypass, silent event loss presented as healthy, missing ownership proof, or an unresolved required test. Missing optional hook support instead narrows advertised capability with a visible limitation. Never replace unavailable integration evidence with invented model responses.
