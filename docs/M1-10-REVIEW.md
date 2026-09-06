# M1 #10 review dispositions

Independent review covers correctness, security and maintainability with separate
editing/review worktrees. No author verdict substitutes for independent review
of that author's production modules. Component verdicts bind recorded source
hashes; the final canonical Docker/native pair remains a separate gate.

## Correctness and security corrections

- Pure reduction now preserves native waits against delayed starts, keeps a late
  terminal T separate from current U, preserves first terminal outcomes and
  explicit disconnected/unsupported observation evidence, and treats lost/unknown
  process observations as uncertainty. Unknown legacy terminal evidence cannot
  preserve live working state. Five additional independent assertions actually
  failed before correction and passed on the corrected 33-case domain suite.
- Storage compares complete submitted intent on retry. Facts, projection,
  independent reason resolution, review/delivery, outbox, checkpoints, terminal
  protection and counters commit together. The independent late-failure test
  compares complete context/outcome state; moving projection before BEGIN makes
  that test fail even where the earlier author rollback assertion stayed green.
- Verified continuation moves a bounded resolution link for the same native wait
  while preserving original event/generation/revision/review/delivery. Missing
  continuity protects historical uncertainty. Tombstone cleanup, startup, restore
  and recognized legacy migration retire the matching scope and degrade live
  projections atomically; a prior generation cannot degrade a fresh replacement.
- Recovery no longer retains regular-file descriptors in tokens/plans. The
  unchanged independent regression reproduces deleted-but-open log bytes exceeding
  the four-file byte limit before the fix and zero retained bytes afterward.
  Ownership locks survive tokens, sequence sorting precedes replay batch selection,
  and cleanup reports completed unlinks even if subsequent synchronization fails.
- Exact durable replay checks immutable source harness/run scope before returning
  a receipt. The receipt retains its private harness binding through final spool
  confirmation. Independent synthetic cross-harness tests reproduced both bypasses
  before correction. These fixtures never enable a production adapter mapping.
- Diagnostics retain a separate optional immutable job loss snapshot, including
  queue discards occurring after a write was submitted. Retried intent remains
  unchanged. Durable, volatile queue, rejected-attempt and artifact counters are
  not summed into a distinct-event total; unavailable job data is null.

Actual RED/GREEN logs, deliberate compiled mutations and restored executions live
in the [evidence index](../evidence/m1-10/README.md). Historical failed executions
remain labeled by actual results, even where an initially chosen directory name
contains “green”. Fresh isolated builds are required when restoring mutated source;
old source mtimes can otherwise leave stale Cargo artifacts, as recorded.

The concurrent crash tests also exposed transient fixture lock ownership across
fork. Independent probes verified CLOEXEC, zero matching lock descriptors in the
parent, an inherited descriptor in the child, refusal before exec and successful
reopen afterward. The fixture correction applies only after explicit local
owner/token release: it first rejects any local descriptor for the exact lock
inode, then permits at most two seconds for an inherited child description to
close. Direct ownership-failure assertions while tokens remain are unchanged.
Removing the local-descriptor guard makes its regression fail. Production lock
semantics, test parallelism and required execution remain unchanged; twenty fresh
full parallel executions passed after the correction.

## Responsibility and maintainability

Pure domain rules depend only on protocol types. The closed storage worker owns
SQLite operations and exact durable intents; no arbitrary SQL or callback API is
added. The coordinator owns explicit sequencing across narrow interfaces, and
borrows the registry exclusively while a job is pending. Recovery separates source
provenance, generated names, owned directories/files, spool, logs and cleanup.
Diagnostics receive copied scalar snapshots, with no I/O/mutation handles. The
source eligibility registry stays empty in production. There are no new crates,
dependencies, unsafe Rust, services, UI business logic or M2 scaffolds.

The independent reviewers accepted these measurement triggers because keeping the
cohesive unit together makes the invariants clearer:

| Unit | Measured trigger | Disposition |
|---|---:|---|
| Ingestion registry | 355 nonblank lines | One authority/queue/receipt owner; committed installation and generation-qualified invalidation share its invariants. |
| Protocol event vocabulary | 306 nonblank lines | Closed event variants and their pure 14-line turn-ID projection stay together. |
| Verification plan | 58-line function | Four additional explicit required rows preserve auditable order, shared preflights and unchanged native partition. |
| Recovery staging | 52-line function | One checked descriptor/write/sync/no-replace-rename durability lifecycle; accepted by the independent recovery reviewer. |
| Coordinator crash and missing-hook scenarios | 52/51 nonblank lines | Cohesive setup, actual external event and resulting durable/native-obligation assertions. |
| Independent commit-receipt scenario | 51 nonblank lines | Parent accepts the reviewer-authored test: ordered context, zero durable rows, no early receipt, atomic rows and one-shot receipt are one causal scenario. |
| Storage receipt/pruning, repeated wait continuation, reconciliation matrix | 52/52/51 nonblank lines | Cohesive transactional scenarios and bounded invalid-input matrix; independently reviewed. |

The 303-line protocol control module and earlier large IPC/protocol/qualification
fixtures retain their prior approved dispositions; this change does not rewrite
them to affect metrics. Measurements are review triggers, not permission to split
files arbitrarily or suppress required lint checks. Strict whole-workspace Clippy,
formatting, the required maintainability coverage report and complete execution
inventories must pass on the final candidate. Product callback latency, idle CPU,
RSS, live adapters, terminal runtime, UI and desktop delivery remain unqualified.
