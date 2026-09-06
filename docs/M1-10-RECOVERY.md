# M1 recovery artifact boundary

`recovery` is a library for manager metadata, not a running adapter, scheduler,
diagnostic collector or acknowledgment service. It depends on typed protocol
metadata and private filesystem operations; it does not import SQLite, ingestion,
coordinator, terminal or harness code. No live adapter/version is eligible in M1.

`AdapterRecord` borrows at most 16 KiB of source bytes and a version of at most
64 bytes. It intentionally has no `Debug` implementation. A private, empty
production mapping registry rejects every live input before parsing,
serialization or temporary creation. `EligibleMetadata` has private fields and
only the provenance gate can construct it. The private synthetic unit fixtures
exercise that same gate and the real filesystem path; they are compiled only
under `cfg(test)` and are not library-external helpers or live qualification.
Required unproven turn/version mappings reject. Unproven optional session
metadata is omitted. A typed string alone does not establish source provenance.

`stage` returns a `SpoolReceipt`, which proves only an owned spool entry. Replay
rechecks the format, source mapping, harness, observed time and complete original
identity. The generated filename includes producer, generation, a 20-digit
sequence, event ID and manager observation time. Sorting the whole bounded
inventory before selecting 32 records therefore cannot put sequence 64 ahead of
sequence 1 because of adversarial UUID ordering. `ReplayEntry::event()` and
`harness()` return validated metadata; they do not retag generations or authorize
an ACK. The crate-internal `confirm_committed` requires equality with the complete
event supplied by the coordinator's verified durable receipt and rechecks the
owned file identity. The coordinator also checks harness scope against durable
producer registration.

## Resource and filesystem rules

| Resource | Limit |
| --- | --- |
| Spool file bytes, every generation of one producer | 256 KiB |
| Total spool file bytes, including partials and unknown orphan names | 8 MiB |
| Global / per-producer spool file count | 1,024 / 64 |
| Encoded record / replay batch | 16 KiB / 32 records, at most 512 KiB |
| Outstanding identity handles / one cleanup application | 128 / 128 |
| Replay eligibility | Less than 24 hours from manager observation time |
| Metadata logs | Four fixed files, each at most 1 MiB; no age TTL |

Byte limits include actual file lengths of staging, partial and orphan artifacts;
finite file counts also bound filesystem block/entry overhead. A staging attempt
reserves the complete additional record before creating a file and makes one
bounded write attempt. Failure preserves a bounded partial for explicit cleanup.
An existing partial never becomes an ACK or an overwritten retry. Rename uses
no-replace semantics after file and directory synchronization. No physical
block-device durability or callback-latency qualification is claimed.

The only writable scope is the fixed private `harbormaster/recovery` directory
beneath a validated private state root. It owns a separate `recovery.lock` and
never operates on SQLite, IPC, harness or worktree artifacts. Traversal is
descriptor-relative, rejects symlinks and unsafe ownership/modes, and requires
regular 0600 single-link files in 0700 owned directories on a supported local
filesystem. Entry scans and payload reads have finite bounds.

Long-lived plans, receipts and replay entries contain stat identity tokens,
not regular-file descriptors. Tokens include device/inode, size, mtime and ctime;
the shared ownership lock survives store drop until outstanding tokens/plans
release it. Each operation reopens with no-follow and checks the full identity.
Transient regular descriptors close before unlink/rotation, so held previews
cannot retain deleted log/spool bytes outside the inventory. This detects ordinary
replacement and in-place changes; it does not isolate malicious same-UID code or
claim race-free deletion against such code.

`preview_cleanup` reads metadata without rotation or deletion. Only its explicit,
instance/revision-bound `apply_cleanup` removes selected generated artifacts.
Unknown names remain protected. Expired, privacy-disabled and retired-generation
decisions are distinct. Future timestamps do not expire early. Expired records
are immediately ineligible for replay; removal requires the caller to invoke
explicit maintenance/cleanup. M1 introduces no automatic scheduler. The result
reports actual successful unlinks even if a later directory fsync fails, together
with a partial failure and an unknown durability gap.

`RecoveryStatus` reports fixed numeric observations. `rejected_attempts` counts
failed staging/log attempts, not unique lost events. `discarded_artifacts` counts
explicitly removed spool artifacts, including partials. Both are volatile and
saturating. `unknown_gap` stays explicit across reopen and failure; no durable
zero-loss counter is invented. Logs take only closed `LogReason` values and
numeric/boolean fields. Read-only diagnostics consume an immutable snapshot.

## Evidence and responsibility review

Focused tests run inside the existing verifier sandbox with the pinned private
tools root and static SQLite preparation. Public target `recovery` covers the
empty eligibility registry, real read-only mount refusal, hostile paths, lock
lifetime, plan revision/identity checks and bounded log rotation. Required Rust
unit tests cover private synthetic provenance, replay, budgets, canary scans,
actual SIGKILL at create/fsynced-stage/rename, and actual regular-file ENOSPC on
the verifier's fixed 1 MiB tmpfs. The full-fs fixture proves filesystem type and
capacity before filling and never falls back to home or `/tmp`.

Observed REDs include the existing protocol schema accepting a prompt canary as
a legal turn ID, a retained preview keeping deleted log bytes beyond the cap,
and reverse UUID ordering returning sequences 64–33 while 1–32 were present.
The latter two were independent review findings, fixed before integration; their
unchanged regression tests now pass. Synthetic mapping canaries are scanned
through normal, crash, full-filesystem and cleanup paths. Root owns the combined
SQLite/WAL/coordinator diagnostics canary and durable-receipt qualification.

Modules follow concrete responsibilities: source provenance, generated names,
directory ownership, bounded file operations, spool, fixed logs and cleanup.
The fixture helper owns disposable public-test setup and detached-file inspection.
There are no new dependencies, unsafe Rust, exported synthetic constructors,
global configuration changes, deployment or M2 work. Independent final review
and canonical hosted/native qualification remain parent integration gates.
