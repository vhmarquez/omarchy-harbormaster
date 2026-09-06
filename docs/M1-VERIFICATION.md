# Current M1 #10 candidate

Main after the approved #9 merge is qualified at
`23be30f0d8fa4fb9767241d08faa0a543a3c950b`; its actual hosted/native paired
receipts are in [the post-merge summary](../evidence/m1-10/post-merge-main/summary.json).
The #10 branch extends the required inventory to 32 portable and 34 combined
checks. The dedicated native inventory remains 5 required checks, including the
preserved original two sandbox methods and the unmapped-peer-PID method.
There are still only three declared optional live/future probes. A single run
never establishes complete qualification; the actual hosted Docker and native
records must pair against identical selected bytes of the exact final commit.

Use the same canonical commands below with the prepared private tools root
(e.g. `--tools /home/vhm/Work/omarchy-harbormaster/.tools-m1-9` on this machine).
The four required new Rust targets are `domain`, `storage_reducer`, `recovery`
and `coordination`; positive sealed-source pipeline tests also run in the
required library suite. Real filesystem-full tests require the verifier's fresh
owned 1 MiB `/fault-fs` tmpfs, validate its capacity before filling, and have no
fallback to a host filesystem. Docker retains its locked-down policy; native
execution remains separately mandatory. See [#10 evidence](../evidence/m1-10/README.md)
for actual test/review receipts, failures, corrections and current qualification.

The records below are historical #7/#8/#9 evidence with their original counts
and revision-specific gates; they do not claim qualification of the #10 source.

# M1 #10 verification continuation

The merged storage dependency introduced required `sqlite-build`, `rust-storage` and
`rust-storage-crash` checks. Portable has 28 required entries, local combined
has 30, and separately executed native has five: three shared preflights plus
the #8 unmapped-peer-PID target and preserved M0 sandbox check (both exact methods).
Docker confinement and the mandatory pairing contract remain unchanged.

[#9 responsibilities](M1-9-DEVELOPMENT.md), [storage guarantees and limits](M1-9-STORAGE-ENGINE.md),
[SQLite source/build review](M1-9-SQLITE-BUILD.md), [review dispositions](M1-9-REVIEW.md), and [the evidence index](../evidence/m1-9/README.md)
separate semantic RED/GREEN, independent review and final qualification.
The issue-linked PR records the exact candidate, hosted run/artifact and actual
paired CLI result. An earlier pass never qualifies changed selected source.

Public preparation explicitly verifies tools, the 30-package registry graph and
the separate official SQLite source. No implicit network access occurs during
verification. Use a NEW private tools root; this host uses `.tools-m1-9` and
preserves `.tools` and `.tools-m1-8-final`. See [preparation](M1-9-SQLITE-BUILD.md).

    env -i PATH=/usr/bin python3 -B scripts/verify.py --tools /home/vhm/Work/omarchy-harbormaster/.tools-m1-9
    env -i PATH=/usr/bin python3 -B scripts/verify.py --tools /home/vhm/Work/omarchy-harbormaster/.tools-m1-9 --scope native
    env -i PATH=/usr/bin python3 -B scripts/verify.py --qualify PORTABLE_REPORT_DIRECTORY NATIVE_REPORT_DIRECTORY --revision FULL_COMMIT_SHA

Pair actual directories containing reports/manifests/logs, not JSON paths.
Paired mode takes no tools option. The current-head hosted artifact must be
portable Docker evidence. Local combined execution alone remains incomplete.
Selected sources include docs; receipts under `evidence/` are excluded to avoid
recursive hashes, which never permits altered code/docs to reuse old execution.
Failure of SQLite source preflight or offline compilation blocks later code
execution while enumerating every remaining required check as NOT_RUN.

## Current main post-merge qualification

Owner-approved PR #51 / closed #9 main
`23be30f0d8fa4fb9767241d08faa0a543a3c950b` passed fresh qualification on
2026-09-06: hosted locked-down Docker portable 28 required PASS, local combined
30 PASS, dedicated native 5 PASS, actual paired PASS for 215 selected files.
[Actual reports and logs](../evidence/m1-10/post-merge-main/summary.json) include
run 34052470724 / artifact 9995027347 and fresh local receipts. This completes
the #9 dependency gate. #10 requires its own newly registered test families,
independent review, exact-current-head qualification and owner PR approval.

Owner-merged PR #50 / closed #8 main
`47d726972991c88da897fe20ef4ce413142227d5` passed fresh qualification on
2026-09-06: hosted locked-down Docker portable 25 required PASS, local combined
27 PASS, dedicated native 4 PASS, actual paired PASS for 174 selected files.
[Actual reports and logs](../evidence/m1-9/post-merge-main/summary.json) include
run 34039894234 / artifact 9991394591 and fresh local receipts. This completes
the #8 dependency check; #9 still requires its own qualification and approval.

PR #49 / closed #7 main `70a671bf5bf93aaed7a3acbc1ad26df76fd60187` had
[its own post-merge qualification](../evidence/m1-8/post-merge-main/summary.json):
portable 21, combined 22, native 3 required PASS, paired 140 selected files.
Only the same three optional live/future probes skipped in either baseline.
Historical records below retain their original results and former blockers;
verified owner merges supersede the old approval text.

# Historical verification — M1 #7

## Commands and supported scope

Prepare explicit, publicly downloadable pinned tools in a NEW private directory:

    python3 scripts/prepare-tools.py --online --tools-root .tools

Run all required checks offline, with no implicit downloads:

    python3 scripts/verify.py

Default backend: existing `/usr/bin/bwrap` on Linux x86_64. The local qualified
Qt version is 6.11.2, Python 3.14.7. The separate native lane uses the observed
installed bubblewrap 0.11.2, not the historical 0.12.0-1 Docker diagnostic
package. [Native platform observations](../evidence/m1-7/split-native-platform.json)
record the real version, executable hash and kernel; this is not a transitive
platform audit. Existing host packages are not modified.
A missing tool or mismatched pin fails required preflight, rather than silently
skipping checks or installing globally. See [TOOLCHAIN.md](TOOLCHAIN.md) and
[DEPENDENCIES.md](DEPENDENCIES.md) for exact pins, origins and obligations.

The owner approved a mandatory split on 2026-09-05. Hosted CI builds the pinned
Arch image and runs the portable inventory (including native Qt tests):

    python3 scripts/verify.py --backend docker --scope portable --image sha256:<built-image-id>

The native Linux lane uses the existing outer bubblewrap isolation:

    env -i PATH=/usr/bin python3 -B scripts/verify.py --scope native

`all` remains the local default: the unique union of 21 portable checks and the
separate `m0-harness-sandbox` check, 22 required checks in total. The native scope
has three required entries: isolation probe, tool integrity, and real sandbox
integration. The two preflights are shared, not different test implementations.
Docker rejects `all`/`native` rather than silently skipping or weakening anything.
The four non-sandbox M0 harness helpers remain portable; the original sandbox
method is relocated intact and accompanied by an inner/outer network-namespace
identity assertion. Removing only the inner network isolation must fail that
assertion even when the outer sandbox would also deny an outbound connection.

Before merge, pair the downloaded Docker report directory and a trusted native
report directory for the exact full PR-head commit:

    env -i PATH=/usr/bin python3 -B scripts/verify.py --qualify <portable-report-dir> <native-report-dir> --revision <full-PR-head>

Both qualified results, passing current-head hosted checks, independent review
and separate owner approval are mandatory. This split authorization is not merge
approval. The native lane is currently a separately executed operational gate,
not a new GitHub-hosted job or a branch-protection setting.

Only a built immutable image ID is accepted. Local Docker access is denied on
the development host, so hosted CI—not an invented local Docker result—is the
required evidence for that backend. Do not grant Docker access, change daemon
configuration, relax namespace restrictions or run a privileged container.

## Isolation and reporting

The trusted verifier copies allowlisted source roots, rejecting symlinks/special
files and excluding `.git`, `.env*`, tools, target, caches and local reports.
Sources/tools are read-only mounts; test targets, HOME/XDG and temp directories
are private and disposable. Environment comes from a fixed allowlist, not the
user's credentials/overrides. No desktop/session sockets or host homes are bound.
Bubblewrap unshares namespaces/network; Docker has no network, dropped
capabilities, no-new-privileges, a read-only root and a non-root user. A common
required self-probe checks an outside synthetic canary, environment, read-only
input mounts and absence of external network interfaces/routes.

The source checkout/tool root are trusted, quiescent inputs, not an adversarial
concurrent-tree scanner. Worktrees are not sandboxes. This workflow is NOT a
general hostile-agent execution service or a new same-UID product guarantee.

Each check has a 240-second deadline; the inventory has an 1800-second deadline.
Output is drained with a 4 MiB per-check cap, and timeout/overflow fail with child
cleanup. Local child limits are per-process: 4 GiB address space, 240 CPU seconds,
512 MiB per regular file, no core dumps. They are NOT aggregate cgroup limits or
a hard bound on total temporary disk across a malicious process tree. Docker
also caps 256 PIDs, 4 GiB memory and two CPUs. Product budgets remain unmeasured.

Reports and bounded test logs are written to a new private `.verify/run-*`
directory. They are disposable build evidence, not production session logs;
source copies and compiled targets are removed after each run. Persistent report
directories are not automatically deleted; users manage their own local build
evidence. CI artifact retention is explicitly limited. The report identifies
PASS/FAIL/NOT_RUN/SKIP, required status, executed argv, exit/timeout/overflow and
actual elapsed time. Isolation/integrity/version preflight failures prevent
remaining code execution but still enumerate required checks as NOT_RUN.

Required suites must have actual tests and no skips/ignores. Optional live M0
runtime/harness and future native-shell integration checks are explicit SKIPs
with reasons, never part of a claimed required pass. Unit boundary doubles and
synthetic native/CLI fixtures are labeled as such; no model/provider calls or
invented application results. The explicit Rust CLI integration target is checked separately from the library
suite; an absent or empty target cannot be satisfied by unit-test passes.

Schema-2 reports include `scope`, both `qualifications`, and
`qualification_complete`. Only successful paired Docker/native review sets
completion true; even the local `all` convenience run lacks hosted Docker
evidence and remains incomplete. A successful portable run reports native `NOT_RUN`
and incomplete qualification; this is **not** an optional native skip. The
three historical optional live/future checks retain their separate names.

Paired review rejects missing/old-schema/wrong-scope results, omitted or skipped
required checks, duplicate names/keys, wrong commands, nonzero/timeout/overflow
results, invalid suite logs and source-manifest differences. It binds actual
allowlisted source bytes to the requested committed Git blobs; no dirty or
untracked selected source can be labeled as that revision. CI's synthetic merge
checkout is acceptable only when its selected source bytes equal that head.
A base/source difference requires reconciliation and fresh qualification, not a
waiver or a relabeled hash. The source snapshot boundary—not excluded evidence,
Git metadata or private tool caches—is what is byte-bound to the commit.

JSON inputs are limited to 2 MiB, logs to 4 MiB, and final-component symlinks or
nonregular input files fail. These are consistency checks on **trusted execution
evidence**, not cryptographic attestation: fabricated reports cannot prove a
process ran. Read the real GitHub run/head and trusted native execution before
acceptance. A passing paired report neither grants approval nor performs a merge.

## Maintainability

The required report checks analysis coverage, not a hard size pass/fail.
Files over 300 or functions over 50 nonblank lines trigger human review.
Python function spans use ASTs; Rust/QML metrics are labeled lexical estimates,
not a complete grammar, cyclomatic-complexity, coupling or duplication analysis.
Generated/vendored/frozen/legacy files are excluded and tests are reported
separately. Clippy warnings are errors; a justified size exception must be
narrowly annotated and explained, never globally suppressed to hide a hotspot.
The PR records hotspot dispositions plus independent architectural review.

### Split maintainability disposition

The paired evidence module has 175 nonblank lines and its largest function has
23. It owns one concern: consistency of scoped evidence with reviewed source.
The CLI retains execution/serialization boundaries rather than adding a second
verification framework. No production size exception is needed.

The 429-nonblank-line `test_qualification.py` is an explicit **test** size review
trigger. It keeps the end-to-end paired-evidence contract and its mutation matrix
together: scope/mandatory status, each actual planned check, commands, logs,
source identity and malformed filesystem/JSON inputs. The shared real disposable
Git and clearly labeled synthetic-record setup lives in one cohesive fixture
helper. Keeping these paired cases together makes each accepted/rejected input
visible next to the same contract; no production rule is copied into a test-only
implementation. Independent review must assess this rationale, not treat a
threshold as permission to omit cases or split files just to lower the count.

### Independent result-gate review corrections

The first independent result-gate review found three defects before publication:
a generic positive-count native log could omit the namespace method, permissive
metrics JSON accepted invalid coverage, and an explicit empty revision could be
silently ignored. The native/CI boundary review passed separately; it did not
substitute for fixing these result-gate defects.

The native validator now requires both exact verbose method results, their exact
executed count and one unambiguous final success summary, while preserving the
real inline namespace diagnostic. A shared strict JSON decoder applies the same
duplicate-key, finite-number, nesting and Unicode policy to envelopes and metrics;
coverage counts must be actual positive integers. An explicit empty `--revision`
without pairing fails before output creation or execution. Focused runner and
actual paired-CLI regressions use disposable committed source and clearly labeled
synthetic receipts, not fabricated execution proof.

[Correction evidence](../evidence/m1-7/split-review-corrections.json) retains the
observed RED/GREEN results, including the stale validator-kind test caught by the
full suite and its explicit update. The refreshed execution bundle below must
match the final source; earlier passing local runs do not waive re-verification.

## Approved split execution record

[The prepublication local record](../evidence/m1-7/split-verification.json)
contains actual required/optional results, source hashes and measured durations.
The [dedicated native report](../evidence/m1-7/split-native/report.json) preserves
its bounded logs and complete source manifest alongside it. These are real local
runs, not a Docker result or an execution-attestation service. The native record
can be paired with the downloaded current-head Docker directory using the command
above once its selected source bytes are bound to the committed head.

[RED/GREEN and mutation evidence](../evidence/m1-7/split-red-green.json)
distinguishes real native namespace runs from synthetic malformed/absent/skipped
suite and report fixtures. The original sandbox method remains intact; the
inner-only network mutation passes that old method but fails the added namespace
assertion. The complete current-head hosted run, paired review and independent
review verdict are linked from PR #49; no old local pass substitutes for them.

## Evidence status

Component evidence and observed RED/GREEN/fault-injection runs are being
consolidated for #7. A wrong QML fixture expectation produced an actual native
failure, followed by a passing sentinel; this qualifies the tool gate, not
product-UI TDD. Early isolation self-probe exposed bubblewrap's generated PWD;
it is now explicitly fixed to `/work` in the common allowlist.

## Historical local execution evidence (before the approved split)

The common entry point passed all **21 required checks** in the captured local
bubblewrap run. [Machine-readable summary](../evidence/m1-7/local-verification.json)
records exits, measured per-check elapsed time and every explicit optional skip.
Required suites: 53 verification tests, 16 tooling tests; 8 Rust library and
9 distinct CLI integration tests; Qt reports 3 passes (one sentinel plus native
init/cleanup); retained M0 suites report 10 contract, 2 identity, 6 cleanup,
5 harness-helper and 15 Node design tests. No required skips/ignores occurred.
The CLI target is checked independently; future feature PRs must register any
new integration target/test family in the required inventory.

The offline policy fault probes were also rerun by the parent: forbidden GPL
metadata, an absent advisory database, a real known-vulnerable vendored crate's
metadata, and absent registry index data were rejected. The vulnerable crate
was never compiled and is not a project dependency. The dependency-free tool
smoke contains zero application tests and is not counted as a passing suite.
These additional policy qualification probes are local opt-in evidence, not a
silently implied hosted CI check.

Initial whole-inventory execution exposed two integration errors: outdated
cargo-deny flags and a legacy test allocating scratch above the checkout.
The corrected command uses `--frozen ... check --deny warnings all`; the M0
cleanup test now honors private TMPDIR, without changing its finalizer or mocks.
A QML formatting mismatch was corrected using the pinned native formatter.
The common entry point was rerun afterward, including all six cleanup regressions.

Independent review reproduced three false-pass conditions: library tests hiding
an absent integration target, QML lifecycle-only suites satisfying aggregate
counts, and Rust declaration drift while standalone tools stayed unchanged.
A separate fix worktree added observed RED/GREEN regressions and narrow fixes.
[Correction evidence](../evidence/m1-7/review-corrections.json) records the fault
cases and outcomes. The parent reran the full isolated inventory afterward.
Final independent re-review passed after real fault injection confirmed all three
corrections. [Review dispositions](../evidence/m1-7/independent-review.json) include
source hashes, responsibility assessments and remaining qualification limits.

Maintainability coverage is complete for 39 analyzed Rust/QML/Python files.
Neither production nor test thresholds were triggered; the largest handwritten
production file is the cohesive AST/report scanner at 266 nonblank lines.
Independent reviewers accepted responsibility boundaries, public interfaces,
duplication/coupling and necessity. They specifically accepted the scanner
traversal complexity and bounded-execution helper without metric-driven splitting.

The hosted #7 correction (see below) separately records 54 verifier tests;
this paragraph retains the original post-CORE-correction baseline.

## Hosted isolation correction

The first [PR run](https://github.com/vhmarquez/omarchy-harbormaster/actions/runs/33986331099)
and its redundant push run failed `isolation-probe` before the rest of the
required inventory. Those failures are not a hosted pass. The exact error was
`host credentials or desktop paths are exposed`, the probe's shared rejection
for a visible `/work/.git` or `/run/user`.

A disposable namespace reproduction isolates the runtime-directory condition:
adding only an empty `/run/user` reproduces the exact failure, masking image
runtime paths with a fresh read-only `/run` passes, and exposing `.git` still
fails. [The executable reproduction](../evidence/m1-7/reproduce-runtime-layout.py)
and [actual cases](../evidence/m1-7/runtime-layout-cases.json) are deliberately
labeled synthetic bubblewrap evidence, not a Docker image execution. Run it with:

    python3 -B evidence/m1-7/reproduce-runtime-layout.py

The container launch policy now mounts a fresh 16 MiB, read-only `/run` tmpfs
with `nosuid,nodev,noexec`; HOME/XDG runtime state stays in private `/state`.
No host runtime path is bound, no probe check is removed, and no Docker
permission or daemon setting changes. The policy test failed before the change
and passed afterward. Workflow trigger tests also went RED/GREEN when removing
duplicate feature-push runs: PR checks and main-push verification remain, on
standard `ubuntu-24.04` runners. Free GitHub CI is owner-authorized; paid runners,
paid services and increasing paid usage are not. Artifact scope/retention and
all required check gates are unchanged.

The policy assertion also checks the complete tmpfs list: independent review
found that a duplicate `/run` option escaped the earlier membership-only test.
The test-only tightening has [observed mutation RED/GREEN evidence](../evidence/m1-7/duplicate-runtime-policy.json);
the production command remains unchanged. Synthetic reproduction remains separate
from Docker wiring qualification, as explicitly noted by the reviewer.

[RED/GREEN evidence](../evidence/m1-7/hosted-correction-red-green.json) and
[the corrected local inventory](../evidence/m1-7/hosted-correction-local.json)
record actual results. That local inventory is not the mandatory hosted result:
read PR #49's exact current-head checks and linked run artifacts before approval.
This correction still requires independent review before publication and owner
approval before merge; the PR's review record identifies the reviewed revision.

## Missing hosted bubblewrap dependency

After the runtime-mount correction, [run 33992905962](https://github.com/vhmarquez/omarchy-harbormaster/actions/runs/33992905962)
executed every required check: 20 passed and `m0-harness-helpers` failed because
`/usr/bin/bwrap` was absent. The same three optional checks remained explicit
SKIPs. This was a missing-executable error, not an observed namespace denial.

The dependency-only correction adds `bubblewrap 0.12.0-1` from the same signed,
SHA-checked archive snapshot, with package/license/version and executable-presence
gates. [Qualification and RED/GREEN evidence](../evidence/m1-7/bubblewrap-dependency.json)
record actual public package bytes and local execution. The M0 helper integration
is unchanged and remains required; Docker confinement and the check inventory
are unchanged. No native test is replaced by a static assertion or a skip.

Installing the binary did not qualify nested namespaces. Actual
[run 33994133494](https://github.com/vhmarquez/omarchy-harbormaster/actions/runs/33994133494)
at `f28ea9cd97497b80ea4ffffcac1e84a6b4515f78` reached 20 required PASS, one FAIL
and three optional SKIPs; the exact error was `bwrap: No permissions to create a
new namespace`. That observation supersedes the earlier missing-binary diagnosis.
It does not identify a particular kernel/seccomp/AppArmor setting as the sole cause.

The owner then explicitly approved the mandatory split documented above. Docker
confinement and the strict probe are unchanged. The diagnostic-only bubblewrap
package is no longer installed in the portable image; real `/usr/bin/bwrap` is
still required on the native host. This is not removal or mocking of the native
integration. Current-head results/review are linked from PR #49; historical
pre-split local results are not evidence that the new paired gate has run.

The baseline GitHub branch-protection endpoint reports main is not protected.
No protection/settings were changed; explicit owner approval and current-head
CI remain mandatory operational gates. Code/maintainability review has passed;
hosted CI is a separate current-PR-head gate, not part of these local artifacts.
Read `gh pr view feat/m1-7-developer-foundation` and `gh pr checks` for current
remote status. **#7 and M1 are not complete before approved, verified merges.**
