# Issue #8 acceptance evidence

Candidate branch: `codex/m1-8-protocol-ipc`, based on owner-merged main
`70a671bf5bf93aaed7a3acbc1ad26df76fd60187`. The issue-linked PR records the
exact published revision and current-head qualification. No merge approval has
been granted; #8 stays open and #9/#10/M2 remain unstarted.

| Acceptance criterion | Implementation and executable evidence |
|---|---|
| Versioned typed requests/events, IDs, sequence/generation recovery, capabilities | `protocol/` and `ingestion/`; 14 protocol tests, strict decoder unit tests, 23 admission/control tests; all nine event kinds and twelve operations. |
| Private sockets, ownership/peer checks, bounded framing/queues/backpressure | `ipc/`; 16 disposable Unix-socket tests, independent EMFILE regression, mandatory native unmapped-PID test and causal counterfactual; no TCP/UDP listener. |
| Event/control separation, malformed-input fuzzing, same-UID limitations | Actual event/control peer composition, independent instance/generation tests; two fuzz/property tests run 12,000 seeded mutations plus 60 hostile cases; [contract](../../docs/M1-8-PROTOCOL.md). |

## Actual records

- [Post-merge main summary](post-merge-main/summary.json): hosted Docker portable
  21 required PASS, fresh local combined 22 PASS, dedicated native 3 PASS, actual
  paired PASS for 140 selected files. This completes the #7 post-merge handoff.
- `candidate/combined/` and `candidate/native/` contain the final actual local
  report, selected-source manifest and complete required logs. They must match
  the published revision's selected source bytes. A single execution always
  leaves `qualification_complete=false`.
- Current-head hosted portable Docker run/artifact and the actual paired CLI
  result are published together in the PR's qualification comment. That record
  binds all selected source bytes to the full candidate commit. Committed local
  records alone do not prove hosted Docker or paired completion.
- [Independent review receipts](review/) preserve scoped verdicts, source
  hashes, commands and real mutation outcomes. [Review rationale](../../docs/M1-8-REVIEW.md)
  explains responsibilities, corrections and size-trigger dispositions.
- Component `protocol/`, `ipc/`, `dependencies/`, `peercred-dependencies/` and
  root JSON receipts preserve observed RED/GREEN and policy probes. Initial
  missing-module compile failures establish scaffolding RED only; later real
  behavior failures and causal guard mutations establish semantic sensitivity.
  Intermediate failures remain labeled; final candidate logs supersede their
  counts without rewriting them.

The required current inventory is 25 portable, 4 native and 27 in the local
union. Original M0 network-denial and inner/outer network-namespace identity
methods remain mandatory alongside the new PID-namespace target. Three
explicit optional live/future probes skip; no required target may skip.
Verification preserves default Docker seccomp, dropped capabilities, no network,
no-new-privileges and private runtime paths. Only standard public-repository CI
is used. No live harness, model, deployment, desktop or credential calls occur.

Durable acknowledgment, database transactions, terminal tombstones, product
diagnostics and spool/recovery remain later M1 work. Fixture timings do not
establish product performance budgets. Same-UID hostile tampering and execution
attestation remain outside trusted, quiescent source/evidence consistency checks.
