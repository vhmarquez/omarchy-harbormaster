# M1 #9 independent review and dispositions

Independent reviewers worked in separate checkouts. The SQLite build author
reviewed filesystem/backup behavior; a separate reviewer assessed dependencies,
transactional correctness, worker ownership and maintainability. The engine
author's test-quality review of the parent's crash fixture is additional input,
not independent approval of the author's own engine. No review grants merge
approval or authorizes #10/M2.

The final verdict and exact reviewed source hashes belong to
[the final review receipt](../evidence/m1-9/independent-final-review/review-final.json).
It must be PASS and cover the published source, including the shared generation
helper, recovery wrapper and public storage/crash targets. The linked component
reviews below retain their narrower source scopes and actual intermediate errors.
Current-head canonical combined/native/hosted/paired qualification is separate.

## Correctness and security findings resolved

- Outcome identity is scoped by producer, generation and event. An independent
  producer may use the same event ID. Complete write-set identity controls exact
  retry; changed attention/projection/outbox effects cannot reuse an old success.
- Human review has an explicit scoped update independent of delivery/history.
  Registration preserves run and harness scope. Opening/restoring saved state
  retires generations before new observations can be accepted.
- A live restore originally reused a revision issued after its backup. The
  corrected revision exceeds both live and backup revisions; the independent
  removal of the live floor reproduces the stale-revision acceptance risk.
- Actual SQLite rejected its all-path NOFOLLOW flag on the deliberate descriptor
  anchor. The correction preserves OS no-follow traversal, header and retained
  inode checks. It documents SQLite canonicalization and hostile same-UID limits.
- A preexisting hardlinked backup SHM file could modify an unrelated fixture
  inode during a read-only backup check. All main/backup/staging/rollback derived
  sidecars are now checked, and published backups are standalone DELETE files.
  The independent original suite passes 12 tests after both backup corrections;
  removal of the backup-SHM guard reproduces the unsafe acceptance.
- A known V1 backup was unusable after migrating the live database. Restore now
  migrates its verified staged copy and preserves the original backup. Removing
  only staged migration is detected by the independent recovery fixture.
- WAL pressure is checked before mutation, including startup retirement. A busy
  checkpoint backpressures the next write; post-COMMIT checkpoint failure cannot
  turn a committed outcome into a rollback claim.
- The worker permit survives queued/executing work as well as retained tickets.
  Removing the envelope's retained permit produces the independent 63-versus-64
  capacity failure. Timed-out/disconnected receipts retain unknown outcomes.
- Corrupt-page recovery is explicit and requires an authoritative upper bound
  including possibly committed unknown revisions. A last acknowledgement is
  insufficient. Recovery timeout may leave changed state and is documented as
  unknown, with no automatic recovery or guessed revision.

[Filesystem review and original/causal executions](../evidence/m1-9/independent-path-review/review.json),
[core review and causal executions](../evidence/m1-9/independent-core-review/review-core.json),
and [independent dependency/source review](../evidence/m1-9/independent-sqlite-review/review.json)
record exact results. Author late-SQL-failure, SQLITE_FULL, row-cap and WAL-reader
fixtures add meaningful semantic coverage; compile errors are not called
behavioral RED. The initial public fixture compilation error and formatting/lint
failures are preserved in [the evidence index](../evidence/m1-9/README.md).

The crash fixture uses a finite test-only trigger and an observed sustained
write-lock window after the child completed startup/registration. It does not
claim an exact SQL instruction at SIGKILL. The other fixture proves committed
rows are visible before ticket consumption. Final independent execution must
also reject both a projection written before BEGIN and full autocommit; those
actual causal results belong to the final receipt. Process interruption and
SQLite rollback are not physical power-loss attestation or #10 reducer recovery.

## Maintainability

Storage is partitioned by cohesive responsibilities: transport/permits, trusted
types and validation, fixed filesystem ownership, exact schema, atomic writes,
bounded queries, explicit maintenance and supported backup/recovery. There is
one worker and one connection, no public arbitrary SQL/closure API, async
framework, extra crate, reducer or UI dependency. Protocol remains pure.
The shared private generation helper removes duplicated OS entropy and UUID
formatting without coupling storage to volatile admission; callers retain their
own typed error categories. No authored unsafe code or new dependency feature
was needed for crash tests or this reuse.

The size report covers every selected Rust/QML/Python source and reports tests
separately. No new storage production file/function exceeds the 300/50 triggers.
The canonical plan's 54 nonblank lines remain one shallow declarative inventory
(three branches, nesting one), accepted as clearer than parallel plan fragments.

The new retention test is 56 lexical nonblank lines (54 under Clippy's count).
Its narrow `expect(too_many_lines)` keeps history, tombstone and obligation
lifetimes visible on one fixture; it introduces no production exception or
blanket lint suppression. Existing protocol/control and large #8 integration
fixtures, plus the paired-evidence test, retain their
[previously accepted dispositions](M1-8-REVIEW.md). Raw execution logs intentionally
retain their original blank lines; source-only whitespace checks pass.

Full Rust all-target/all-feature Clippy still denies warnings. Measurements here
are review signals and actual fixture timings, not achieved product performance
budgets, an upstream SQLite full-suite result or a complete redistribution audit.
