# M1 #10 storage extension

The public worker retains the same 64-request bound and unknown-outcome tickets.
`Context` performs one short read transaction containing the global revision,
producer registration, scoped current turn, incoming-turn tombstone and exact
event receipt classification. Receipt comparison includes the complete canonical
event payload. A pruned receipt cannot prove an event from its checkpoint; exact
retained receipts remain useful after generation retirement. `Apply` compares
the entire original reducer write set before revision/admission checks, so only
an unchanged retry can recover its original committed revision.

`Reconcile` commits one engine-issued generation and a trusted manager baseline.
Only Fresh/Alive evidence with either no current turn (Unknown) or one concrete
Working/Input/Approval turn establishes `reconciled`. Unavailable evidence stays
inactive. Legacy registration and legacy commits cannot establish this gate.
Each run admits at most one active producer. A reconciliation request may name
one explicitly verified old current turn whose transient reasons resolve; a
generation change alone does not resolve them. Uncertainty effects atomically
retire admission before the worker can execute another queued write.
`RetireProducer` handles a scoped conflict/gap without persisting an invalid
fact or issuing another generation. It preserves the independent process
dimension, current turn identity, terminal outcomes and sequence checkpoint;
observation becomes Stale and a nonterminal turn becomes Unknown. Revision and
full-u64 known-loss overflow are checked before mutations. The exact ticket is
retained through unknown completion, so callers must not retry loss accounting
as a new operation merely because acknowledgment has not arrived.

The V3 schema preserves V1/V2 receipt bytes and migrates only recognized layouts,
with the existing verified pre-migration backup. Scoped current identity is not
invented for a legacy projection. Retained facts can establish outcome revision
and terminal kind; pruned legacy evidence keeps explicit `None`. Both old backup
versions migrate in fixed staging before restore, leaving the original backup
unchanged. The existing exclusive paths, sidecar guards, finite database/WAL
bound, backup deadline and corrupt-recovery revision-floor restrictions apply.
Corrupt recovery still requires an authoritative revision upper bound including
possibly committed unknown outcomes; the last acknowledged revision is not
sufficient when later outcomes are unknown.

Attention resolution is independent of review and delivery. A partial unique
index permits at most one unresolved scoped Input, NativeApproval and
ConnectionUncertainty row per turn, so precise resolution changes at most three
rows. A no-turn uncertainty reason coalesces only within the incoming producer,
generation and run. Legacy unknown scopes remain protected. Repeated ensures
do not create another outbox item; terminal outcomes retain their first revision
after fact pruning. A late terminal may add its own tombstone and outcome while
the current-turn projection remains another turn.

`Policy`, `Status` and cleanup preview read existing state. Policy changes and
cleanup application are explicit operations. A preview contains at most 100
fact, 100 tombstone and 100 delivery candidates and is bound to its engine
instance, revision, time and policy. Another engine, restart or restore cannot
reuse it. Tombstone cleanup retires affected generations before removing
protection; attention review is untouched. Existing retention/capacity choices
remain the owner's approved 7/30-day history, independent 20,000/30-day
tombstones and 1,000-pending/7-day outbox policy.

Component receipts in `evidence/m1-10/storage` record actual RED/GREEN and source
hashes. Coverage includes preserved legacy migration/restore, full payload and
write-set conflicts, pruned outcome evidence, exact transient resolution,
coalescing, late terminal retention, inactive admission, late transaction abort,
cross-engine cleanup rejection, and real regular-file ENOSPC on the verifier's
guarded 1 MiB tmpfs. This is synthetic library evidence; final current-head
Docker/native qualification and owner approval remain separate.
