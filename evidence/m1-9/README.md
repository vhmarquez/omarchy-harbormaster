# M1 #9 evidence index

Candidate for issue #9; owner approval of each published PR/revision remains
required before merge or dependent #10. M1 is not complete; M2 is unstarted.
No evidence here qualifies live harnesses, adapters, UI or real user databases.

| Acceptance criterion | Implementation and executable evidence |
|---|---|
| One bounded manager SQLite worker; migration/backup/recovery | `src/storage/worker*`, `paths`, `schema`, `backup`, `engine`; 64 retained-operation permits; actual 22 engine and 6 worker unit tests, 15 public storage and 2 real process-kill tests. [Engine regressions](storage-engine/summary.json), [public integration](integration-third/results.json), [independent path review](independent-path-review/review.json). |
| Atomic lifecycle/attention/outbox; distinct expiry | Complete scoped write-set receipt, projection, attention, outbox, tombstone, sequence checkpoint and revision in one transaction. Actual late-SQL failure, SQLITE_FULL, independent row caps, exact retry/conflict and seven/30-day separation tests; [semantic mutants](storage-engine/semantic-mutations.json), [core review](independent-core-review/review-core.json). |
| Current SQLite/WAL/local-filesystem policy | Official SQLite 3.53.4 reviewed source and static build, exact runtime source ID, private files/local filesystem allowlist, bounded WAL with real blocked-reader backpressure, supported live-backup API. [Build evidence](sqlite-build/), [independent dependency review](independent-sqlite-review/review.json), [storage contract](../../docs/M1-9-STORAGE-ENGINE.md). |

The [pre-implementation boundaries and approved retention decision](../../docs/M1-9-DEVELOPMENT.md)
record scope before cleanup implementation. [Actual #8 main qualification](post-merge-main/summary.json)
completes the owner-merged dependency, not candidate #9 qualification.

Component records retain actual exits/output and selected source hashes. Author
RED/GREEN and independent review are distinguished. The initial integration
compile failure is a test fixture's attempted deserialization of trusted-only
`WriteSet`, not a product behavior regression; the fixture now reconstructs and
compares its fixed synthetic write set. The second run passed storage/crashes but
strict Clippy found one long lifecycle test, now narrowly justified. The third
passed all 39 library, 15 storage and 2 crash tests plus strict all-target Clippy.
The initial combined run had only a formatting failure in the storage reexport;
its corrected final complete execution is recorded separately.

Final combined, dedicated native, actual hosted Docker artifact and paired CLI
records are required against the exact published head. The PR identifies their
full revision, run and artifact IDs; no component pass fills an absent final gate.
The only optional skips are the preserved three live/future probes. Evidence
consistency is not execution attestation; use trusted actual records.

[Final independent review](independent-final-review/review-final.json) binds the final production/test files and actual crash negative controls. [Review dispositions](../../docs/M1-9-REVIEW.md) explain corrected findings and size exceptions.
