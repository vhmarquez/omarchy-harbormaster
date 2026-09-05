# M0 reference workloads and acceptance budgets

Issue: [#6](https://github.com/vhmarquez/omarchy-harbormaster/issues/6). These are design targets and a measurement protocol, **not achieved performance claims**. There is no Harbormaster production daemon in M0.

## Reference environment, observed 2026-09-05

Linux 7.1.9-arch1-2 x86_64, glibc 2.44; Intel Core i7-14700KF, 28 logical CPUs; physical memory reported by sysconf: 67,167,666,176 bytes. Linux process clock resolution: 100 ticks/second. Python 3.14.7, Node 26.7.0, Git 2.55.0, tmux 3.7c, systemd 261.2, Quickshell 0.3.1. These fields were collected without recording hostname, serial number, home-directory contents or real sessions. Harness versions and runtime terminal evidence are in their dedicated reports.

Rust/cargo were not on the parent shell PATH during M0 discovery. M1 owns reproducible pinned Rust tooling; this is not a failed application build or a reason to install global packages in M0. Machine results are a reference, not a minimum specification or a compatibility matrix.

## Workloads

| ID | Inputs | Duration and acceptance |
|---|---|---|
| quiet-20 | 20 registered sessions, three harness capability fixtures, 2,000 metadata history entries, no events | Warm up 30s; sample 180s spanning at least six 30s reconciliation intervals; supervisor + necessary bridge RSS <64 MiB; CPU <0.25% of one core |
| unchanged-active | Same registry, one live working turn whose process state is unchanged | 180s; no watch/write feedback loop, no metadata writes or snapshots solely for elapsed labels; report wakeups and health probes |
| ordinary | 20 sessions, controlled metadata events at 10/sec for 180s | p95 accepted-event to rendered selected/list state <200ms; loss/drop count zero; CPU/RSS reported |
| burst | 1,000 minimal events/sec for 10s from multiple producer identities, ordinary load resumes afterward | No silent loss; bounded queues; backpressure/drop counters and freshness visible; recover current snapshot within 5s after burst subsides |
| registry-2000 | 2,000 sessions, 20,000 bounded metadata facts, long/Unicode labels and projects | No unbounded QML model; snapshot pagination and virtualized list behavior; report CPU/RSS/p95 separately, not misapply quiet-20 budget |
| slow-consumer | Pause one bridge reader for 30s while another consumes ordinary traffic | Healthy consumer remains responsive; slow queue bounded, disconnect/resnapshot signaled; no growing memory backlog |
| disk-fault | Separate temporary local filesystem with full/read-only/locked DB scenarios | No durable acknowledgment before persistence; bounded retries, truthful degraded status and recovery; never fill the user's actual home volume |
| notification-crash | Crash before send, after send/before acknowledgment, and after durable acknowledgment | No silent unread-outcome deletion; uncertain duplicate-visible notification possibility explicitly recorded |
| native-ui | 1280x800 and 1024x768 native QML windows, 620px compact adaptation; dark/light and fractional/high scaling | Warm popup and immediate local action feedback <100ms where compositor permits; native render measurement, not DOM timing proxy |

Synthetic events are labeled fixtures and bypass no real harness approval. Actual harness integration gates use disposable homes and real versions, with explicit opt-in for credentialed/network/model calls. Mock servers must never be passed off as provider/harness success evidence.

## Resource ceilings for protocol v0

- Frame: at most 16,384 UTF-8 bytes including newline; JSON object depth at most 8, no duplicate keys or non-finite numbers. Hook labels/content are not accepted event fields.
- Producer command handoff: target 25ms ordinary callback overhead, hard 100ms bounded forward attempt before bounded metadata-only fallback/drop. Measure in the real harness callback, excluding harness generation time.
- Ingestion queue: 1,024 frames globally, max 128 per producer; slow-client unsent stream max 256 frames or 1 MiB, whichever first. Saturation results in explicit retry/resnapshot/drop accounting, never a success response.
- Snapshot: pages at most 100 records; 1,024-byte UTF-8 task labels, 4,096-byte approved local paths, 256-byte external harness IDs; text display clips independently of stored limits.
- Metadata spool: max 256 KiB per producer, 8 MiB total, age 24h, all bounded regular owned files; overflow visibly counted and reconnect reconciles liveness. Accept only allowlisted metadata with verified adapter source-field provenance and version-specific mappings. Mapping review and synthetic content/credential canary tests are required before enabling persistence, including temporaries and recovery. String schemas minimize fields; they do not prove absence of sensitive content (see PROTOCOL.md).
- Registry: soft UI warning at 2,000 sessions, explicit hard policy ceiling 10,000 metadata sessions; retained facts max 20,000 or 30 days, whichever first. Tombstones protecting replay and unread/outbox state are separate bounded tables; pending outcome eviction requires visible overflow/expiry state. M1 must pin and test the independent tombstone cap/TTL. Before age/cap eviction of a still-authoritative tombstone, atomically retire its producer generation; subsequent lifecycle ingestion requires a fresh manager-issued generation and authoritative reconciliation, never replay under a new generation. If retirement cannot commit, do not prune protection or accept unprotected work; expose resource/persistence failure. See PROTOCOL.md for restart and reconciliation rules.
- Outbox: max 1,000 pending deliveries, default expiry 7 days; oldest expiry/overflow remains auditable and does not mean human review. Logs: 4 files × 1 MiB, sanitized metadata only. DB page/WAL limits and checkpoint thresholds must be validated in M1 against selected SQLite build; no zero-loss guarantee under full disk.

These are proposed initial hard limits that M1 tests must enforce. Changes require a recorded rationale and updated fixtures, not silent increases to make a benchmark pass.

## Measurement method

Use monotonic clocks for within-process timings; carry a correlation event ID through ingress, durable acknowledgment, projection and actual render acknowledgment. Report ingress-to-visible and accepted-to-visible separately so blocked ingestion does not hide latency. Measure distributions (p50/p95/p99/max), event counts, errors and drops, not only averages. Repeat quiet/ordinary measurements three times and retain all runs, including failures.

CPU: report user+system CPU seconds divided by wall seconds ×100 for one core; do not divide by 28 CPUs. Use process-tree accounting for daemon + required bridge, identify child DB worker threads correctly, and report `SC_CLK_TCK` quantization. RSS: report high-water and sampled sum for daemon/bridge; keep Qt/UI, tmux, terminal and model/harness memory separate. Record instrumentation overhead, wakeups, disk bytes/size and IPC traffic. Do not subtract failed probes or count only an empty-home workload.

CI in M1 can validate deterministic bounds; hardware-dependent latency budgets require a supported reference run. Results must record executable/tool versions, fixture revision, commands, sampled duration and explicit pass/fail/unsupported status. None of these target budgets have been measured against an application at M0.
