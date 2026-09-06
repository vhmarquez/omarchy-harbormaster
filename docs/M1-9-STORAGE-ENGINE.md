# M1 #9 SQLite storage engine

This library is an internal manager primitive, exercised with disposable files.
It enables no adapter, reducer, daemon, real database or recovery UI. The public
bounded worker owns the only reachable engine; there is no SQL, connection,
callback, filename, URI or pragma escape hatch. A `WriteSet` is trusted manager
input. Its type does not establish authorization or safe adapter source mapping.
Future adapter persistence still requires the documented version-specific source
provenance review and content/credential canary tests.

## Atomic records and bounded reads

An accepted write atomically commits the exact complete write-set receipt,
explicit run projection, attention entries, optional outbox intent, optional
terminal tombstone, producer checkpoint and global revision. The engine validates
shape, scope, sequence and expected revision; it does not infer lifecycle policy.
The event key is `(producer, generation, EventId)`. The same EventId in another
producer or generation is independent. An exact retry returns its original
committed revision even after the current revision advanced. Any changed effect
in the complete write set conflicts, including review or notification intent.

Sequence and revision are fixed eight-byte big-endian BLOBs with SQL type/length
checks and checked Rust decoding. `u64::MAX` is representable. Sequence exhaustion
stores no next sequence; revision exhaustion refuses before changing records.
Snapshot pages contain at most 100 runs. A subsequent cursor must name the same
global revision or fail stale; no read transaction or WAL reader survives a
request. Outcome queries return one scoped outcome and at most eight attention
entries. The current reason vocabulary allows five distinct entries.

Only trusted registration/reconciliation issues a fresh random generation.
Reconciliation compares the stored generation, run and harness. Opening a saved
database retires active generations transactionally, preserving outcomes and
checkpoint evidence. No saved generation can authorize new observations without
fresh trusted reconciliation. This does not implement #10's authoritative
current-turn/runtime reconciliation.

## Independent retention and pressure

The owner-approved history choices are 7 and 30 days, default 30; 90 is rejected.
History is capped at 20,000 facts. Explicit maintenance removes at most 100 oldest
expired facts, or 100 oldest facts when capacity is reached. Tombstones have a
separate 20,000 / 30-day bound. Every maintenance deletion of a current generation's
tombstone first retires that generation in the same transaction. A failed
retirement preserves both the tombstone and the active registration.

Protected attention has its own 20,000-row ceiling. Registry/projections each
have 10,000-row ceilings. No history prune, outbox expiry or generation rotation
removes an attention obligation. Review uses an explicit scoped manager command.
Pending/attempted-uncertain outbox rows are capped at 1,000; full capacity rejects
the complete new write set. Delivery expiry at seven days changes the auditable
delivery state without changing review. Retained delivery audit has a separate
20,000-row fail-closed ceiling; it is not silently evicted to admit new work.
The caller may explicitly mark an existing delivery overflowed. No notification
service is contacted and no exactly-once display guarantee is made.

The selected build uses 4,096-byte pages, maximum 16,384 pages (64 MiB), a 1 MiB
ordinary cache target, disabled cache spill, 50 ms busy timeout, FULL synchronous
WAL and no automatic checkpoint. All mutation paths pass explicit WAL pressure
checks. At 4 MiB, a real TRUNCATE checkpoint must finish before another mutation.
A busy reader causes backpressure. `journal_size_limit` is only an additional
target. [SQLite's single-write-per-changed-page transaction behavior](https://sqlite.org/wal.html#avoiding_excessively_large_wal_files) supports the
conservative physical WAL bound of 71,696,416 bytes: trigger plus one database's
pages including frame/header overhead. Existing WAL beyond that bound is refused.
The finite database ceiling can cause SQLite full errors before row ceilings.

Transactions use fixed SQL and bounded batches. The focused fixture holds a real
SQLite reader until actual checkpoint backpressure occurs and verifies no next
write is admitted. Another lowers a disposable database's actual page ceiling
until SQLite returns FULL and checks that no partial obligation/fact survives.
These are correctness/resource fixtures, not achieved product latency or RSS
budgets. Backup/staging/rollback files consume additional bounded disk space.

## Paths, migration and explicit recovery

Only `harbormaster/state.db` beneath an existing private state root is selected.
Ancestors are descriptor-traversed without symlinks. The manager directory is
owned 0700; fixed regular files are owned 0600 with one hard link. Every main,
backup, staging and rollback `-wal`, `-shm` and `-journal` name is checked before
SQLite opens, including read-only probes. The manager keeps an exclusive lock
and retained directory/main/lock/backup/staging/rollback inode identities. Owned
rename/removal checks them again; unexpected fixed recovery leftovers fail
closed. No recursive cleanup or arbitrary file recovery exists.

Reviewed filesystem types are Linux ext4, XFS, Btrfs, tmpfs and overlayfs (needed
by the locked Docker fixture). Unknown/network filesystem types are rejected.
This allowlist does not prove backing-device durability or provide isolation
against hostile concurrent code under the same UID.

Header application identity, supported version, page size and size bound are
checked using a no-follow descriptor before SQLite touches a foreign file.
SQLite's `OPEN_NOFOLLOW` flag rejects the deliberate `/proc/self/fd` anchor too;
the actual initial-open regression exposed that. The implementation retains its
own no-follow traversal, header open and inode checks and permits SQLite to
resolve that descriptor anchor. The selected Unix VFS also adds final-open
`O_NOFOLLOW`. SQLite canonicalization and same-UID replacement are not claimed
to be a race-free filesystem isolation boundary.

Only the exact known schema is accepted; bounded schema inspection applies
connection-local limits first. Foreign/future schemas are refused. The known
v1-to-v2 migration uses a validated live-backup API snapshot before its atomic
change. The [supported backup API](https://sqlite.org/backup.html) performs 32-page steps with a two-second deadline and bounded
page validation, never a copy of a live main file. Published backups are verified
standalone DELETE-journal files; their WAL/SHM are never implicitly adopted.
A v1 backup is migrated in its fixed staging copy before activation, preserving
the original backup.

Restore validates and synchronizes staging before replacement, retains the
exclusive lock, and fsyncs each directory rename. Its committed revision exceeds
both the actual current revision and backup revision, and saved generations are
retired. A crash between replacement phases leaves recognized staging/rollback
state and fails closed on normal startup. If a failure leaves ambiguous WAL,
the original rollback file is retained rather than being paired with that WAL.

Explicit `recover_backup` handles a corrupt page only when the main file still
has recognizable manager identity and supported header. Foreign/future files,
unattributable header corruption and pending WAL are refused. Recovery requires
a trusted authoritative revision upper bound that includes possibly committed
unknown outcomes, not just the last acknowledgement. The new revision exceeds
that bound and the backup. Without that evidence recovery is unavailable; this
primitive does not invent a revision or expose an automatic salvage UI.

Deleting rows or fixed owned staging files makes no secure-erasure promise for
SQLite pages, WAL, backups, snapshots or underlying media.

## Worker receipts and process interruption

A single database thread accepts fixed typed requests. Its 64 outstanding
permits cover queued work, executing work and retained completed tickets together.
A dropped ticket does not release its queued/executing operation's permit;
completed tickets retain capacity until dropped. Submission never blocks and
returns the original request on invalid input, full capacity or shutdown.
The ticket exposes its original request by borrow for reconciliation.
Each response channel has one slot; timeout keeps the ticket available, and a
closed channel reports an unknown outcome rather than claiming rollback.
Worker drop requests stop without waiting for OS I/O. Already executing work may
commit; shutdown is not transaction cancellation.

Startup has a five-second response timeout. A late open finishes its OS I/O and
closes without accepting work. A recovery startup timeout has an unknown recovery
outcome: replacement may already be committed or may finish after timeout.
Reconcile before another operation; timeout never promises unchanged storage.

The required `storage` integration target checks the actual public worker,
static SQLite identity, bounded pagination, stale snapshots, supported retention,
reopen, backup and explicit corrupt-page recovery. `storage_crash` uses real
SIGKILL in two private subprocess fixtures: an observed extended write-lock
window held with a finite test-only SQL trigger, and committed rows visible to
a separate connection before the child consumes its ticket response. The first
window depends on scheduling and is not an exact SQL instruction crash hook.
It asserts no partial records after rollback; the second asserts all records,
checkpoint and scoped exact retry survive. Causal mutations must detect
projection writes outside the transaction and full autocommit. These tests are
storage process-interruption evidence, not power-loss attestation, native-harness
qualification or the future #10 reducer/replay/spool controller.
