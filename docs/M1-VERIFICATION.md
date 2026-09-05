# Verification — M1 #7

## Commands and supported scope

Prepare explicit, publicly downloadable pinned tools in a NEW private directory:

    python3 scripts/prepare-tools.py --online --tools-root .tools

Run all required checks offline, with no implicit downloads:

    python3 scripts/verify.py

Default backend: existing `/usr/bin/bwrap` on Linux x86_64. The local qualified
Qt version is 6.11.2, Python 3.14.7. Existing host packages are not modified.
A missing tool or mismatched pin fails required preflight, rather than silently
skipping checks or installing globally. See [TOOLCHAIN.md](TOOLCHAIN.md) and
[DEPENDENCIES.md](DEPENDENCIES.md) for exact pins, origins and obligations.

Hosted CI builds the pinned Arch image, then invokes the SAME check inventory:

    python3 scripts/verify.py --backend docker --image sha256:<built-image-id>

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

## Maintainability

The required report checks analysis coverage, not a hard size pass/fail.
Files over 300 or functions over 50 nonblank lines trigger human review.
Python function spans use ASTs; Rust/QML metrics are labeled lexical estimates,
not a complete grammar, cyclomatic-complexity, coupling or duplication analysis.
Generated/vendored/frozen/legacy files are excluded and tests are reported
separately. Clippy warnings are errors; a justified size exception must be
narrowly annotated and explained, never globally suppressed to hide a hotspot.
The PR records hotspot dispositions plus independent architectural review.

## Evidence status

Component evidence and observed RED/GREEN/fault-injection runs are being
consolidated for #7. A wrong QML fixture expectation produced an actual native
failure, followed by a passing sentinel; this qualifies the tool gate, not
product-UI TDD. Early isolation self-probe exposed bubblewrap's generated PWD;
it is now explicitly fixed to `/work` in the common allowlist.

## Local execution evidence (after review corrections)

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

The baseline GitHub branch-protection endpoint reports main is not protected.
No protection/settings were changed; explicit owner approval and current-head
CI remain mandatory operational gates. Code/maintainability review has passed;
hosted CI is a separate current-PR-head gate, not part of these local artifacts.
Read `gh pr view feat/m1-7-developer-foundation` and `gh pr checks` for current
remote status. **#7 and M1 are not complete before approved, verified merges.**
