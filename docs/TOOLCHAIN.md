# Private toolchain preparation — M1 #7

## Scope and pins

`tools/toolchain.lock.json` is the reviewed source of truth for Linux
`x86_64-unknown-linux-gnu`. No rustup, mise, npm installation, user Git
configuration, credential helper, model, harness, daemon or listener is needed.

For #7, the canonical `tool-pins` preflight requires exact equality between
`rust-toolchain.toml`'s `toolchain.channel`, `Cargo.toml`'s
`workspace.package.rust-version`, the lock's `rust.version`, and the compiler
release verified against `rust.binary_versions.rustc` by the shared read-only
inspection API. This runs inside the offline boundary before test execution.
Standalone Cargo does not consume rustup's channel declaration, so the explicit
cross-check is required. The workspace declaration is the same pinned release,
not a separately tested lower MSRV; #7 installs or qualifies no older compiler.

- Rust **1.98.1**: standalone rustc, cargo, target std, rustfmt and Clippy
  component URLs and SHA256 values come from the official release manifest.[1]
- cargo-deny **0.20.2**: official Linux x86_64 musl executable archive; it runs
  on this GNU target without a separate musl toolchain.[2]
- Node **26.8.1**: official Linux x64 standalone distribution, only for the
  preserved M0 design regression tests; no npm packages are installed.
  Its tarball SHA256 comes from the official release checksum list.[10]
- RustSec upstream commit
  `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`, committed
  `2026-09-02T09:13:32Z`; its HTTPS archive is independently SHA256-pinned.[5]

The lock also records hashes measured from the verified downloaded executables
and Rust libraries, enabling inspection without trusting editable receipts.
Those derived pins must be regenerated from hash-verified official archives when
updating tools. System Python (3.11+ with `tarfile.data_filter`), Git, a C linker,
CA certificates and bubblewrap for the probes are host prerequisites, not
silently downloaded dependencies. Native Qt/Quickshell and their distro package
revisions are separate verifier concerns: this policy does **not** audit them.

## Explicit online preparation

From the repository root, with an explicit absolute destination:

```sh
env -i PATH=/usr/bin HOME=/nonexistent /usr/bin/python3 -I \
  scripts/prepare-tools.py --online --tools-root "$PWD/.tools"
```

The parent must already exist. A new destination is created mode 0700 with
`.harbormaster-tools.json` mode 0600. Existing destinations must have that exact
ownership marker, belong to the current UID and be private. Even an empty
unmarked directory is refused; choose a new path instead of adopting it.
Existing installations are never overwritten or silently skipped.
`--only rust|deny|rustsec|node` prepares a missing group in an already-owned root;
this also resumes after a later group fails. For upgrades prefer a fresh root,
verify it, then explicitly retire the old owned tools after callers stop using it.

Every artifact requires a credential-free HTTPS URL and exact lowercase SHA256.
Downloads disable ambient proxies/authentication, including redirects, have a
30-second socket timeout, 600-second transfer deadline and 300 MB compressed
limit. A hash failure, timeout or oversize download removes its partial file.
Tar extraction rejects traversal and uses Python's data filter, with 100,000
members / 3 GB expanded limits. Destination symlink ancestors are refused.
These checks do not claim isolation against a hostile process with the same UID.

Each group installs in an owned temporary directory under the destination,
under a nonblocking ownership-marker lock. Rust's official `install.sh` uses
an explicit final `--prefix=<root>/rust` plus staging `--destdir` and
`--disable-ldconfig`; it never touches the system toolchain. Completed groups
are renamed into place. Failure removes that group's staging area while keeping
previously completed groups. Subprocesses use a fresh allowlisted environment,
`PATH=/usr/bin`, private HOME/TMPDIR, disabled Git configuration/helpers/hooks,
no stdin, bounded runtime, and process-group termination on timeout.

Installed layout:

```text
.tools/rust/bin/{rustc,cargo,rustfmt,clippy-driver,...}
.tools/bin/cargo-deny
.tools/node/bin/node
.tools/advisory-db/snapshot.tar.gz
.tools/advisory-db/advisory-db-3157b0e258782691/{crates,.git,...}
```

Each group has `preparation.json` recording actual versions, download hashes and
provenance. Rust/Node distributions retain their license notices; cargo-deny's
release payload is retained in `bin/cargo-deny-distribution`.

## Read-only offline inspection API

```sh
env -i PATH=/usr/bin HOME=/nonexistent /usr/bin/python3 -I \
  scripts/prepare-tools.py --check --tools-root "$PWD/.tools"
```

This emits a JSON report and exits nonzero on failure. It creates no directories,
locks, temporary files or downloads, checks the marker, executable/library
hashes and actual version banners, compares every RustSec archive file with the
installed tree, rejects extra advisory files, and enforces snapshot age.
It does not hash arbitrary extra files outside those enumerated tool paths or
claim a complete OS/third-party-runtime security assessment. npm is not invoked
or part of the executable integrity gate.

For the canonical verifier, add the repository `scripts` directory to the
Python import path and call:

```python
from tooling.prepare import verify_existing
report = verify_existing(tools_root, parsed_lock, now=None)
```

`tools_root` is a `Path`, `parsed_lock` is parsed JSON, and optional `now` is an
aware UTC datetime for deterministic tests. Failures raise `ValueError` or
`OSError`; there is no skip result. This is the shared version/integrity/freshness
rule, rather than duplicating it in the verifier. Run it against read-only
`/tools` inside the verifier's network-disabled sandbox. The CLI explicitly
disables Python package bytecode writes; API callers should use `python3 -B`.

## RustSec representation, freshness and offline cargo-deny

The prepared database is an **honest local Git commit of the verified upstream
archive**, not a clone and not a claim to possess upstream Git history.
`preparation.json` distinguishes the upstream commit from the local snapshot
commit. It retains the archive and all advisory license/attribution metadata.
The local commit date and `.git/HEAD` mtime are set to the upstream commit date,
not the extraction date. No fetch occurs during verification.

Our freshness rule is **0 <= age < 7 days**, measured against the immutable
upstream `commit_date` in the reviewed lock. Future dates or seven-day-old
snapshots fail closed. Refresh the commit, archive hash and date in a reviewed
PR **at least weekly**, and immediately when a relevant advisory warrants it;
repeat the offline gate and record its snapshot identity. This is a pinned
snapshot check, **not a live vulnerability audit**. Re-extracting an old archive
or touching Git files cannot renew this independent age gate. cargo-deny's
own staleness implementation considers Git file times, so its `P7D` setting is
only defense in depth, not the freshness authority.[4]

cargo-deny 0.20.2 lowercases the database URL and applies XXH64 with seed
`0xca80de71`; for exactly `https://github.com/RustSec/advisory-db` the child is
`advisory-db-3157b0e258782691`. It also opens a writable parent `db.lock`.[3]
Copy the prepared `advisory-db` parent (preserving file timestamps) from
read-only `/tools` into writable `/state/advisory-db`. `tools/deny.toml` uses
that exact parent path. In fresh state use:

```text
PATH=/tools/rust/bin:/tools/bin:/tools/node/bin:/usr/bin
HOME=/state
CARGO_HOME=/state/cargo
CARGO_NET_OFFLINE=true
```

Inside the verifier sandbox, invoke the real pinned binary:

```sh
/tools/bin/cargo-deny --manifest-path /workspace/Cargo.toml \
  --config /workspace/tools/deny.toml --frozen --workspace \
  check --deny warnings all
```

`--frozen` means locked plus offline; do not use the removed
`--disable-fetch` option. **`--deny warnings` is essential:** real 0.20.2 probes
showed missing cached registry yank metadata produces `warning[index-failure]`
even with `yanked="deny"`. Promoting warnings closes that gap. The current
zero-dependency workspace needs no registry cache. Future dependencies require
explicit, separately reviewed offline source/index preparation; an absent cache
must fail, not trigger a verifier fetch. No dependency-preparation framework is
introduced by #7.

## License and dependency policy

`tools/deny.toml` checks all features and runtime/build/dev dependencies; private
workspace crates are not exempt. It denies unknown registries/Git sources,
wildcards and duplicate versions, known vulnerabilities, unmaintained/unsound
crates, yanks, and licenses outside the explicit allowlist. cargo-deny denies
unlicensed packages by default; its removed `unlicensed` key must not be used.[9]
The allowlist does not waive obligations: MIT/BSD/ISC/Zlib notices, applicable
Apache-2.0 notices/license/patent terms and Unicode notices still need retention
when redistributing covered code. Changed or new license expressions require
review, not a blanket exception. This is not legal advice or a transitive
license audit of the Rust/Node/Qt tool distributions themselves.

Retain the actual distributions' full license and third-party notices if tools
are redistributed: Rust component notices, cargo-deny's MIT/Apache files and
Node's comprehensive `LICENSE`. Node's copyright/permission notice must accompany
substantial copies.[11] RustSec is generally CC0, with explicitly identified
GHSA-derived content under CC-BY-4.0; retain its `LICENSES` and per-advisory
license/URL attribution rather than treating the whole database as CC0.[7]
No tools, database or vendored fixture are part of a shipped manager in #7.

## Executable regression evidence

```sh
/usr/bin/python3 -m unittest discover -s tests/tooling -v
# Explicit ONLINE preparation of one hash-pinned, known-vulnerable test crate:
/usr/bin/python3 tests/tooling/probe_policy.py --tools-root "$PWD/.tools" --prepare-fixture
# OFFLINE execution: bubblewrap unshares the network and mounts tools read-only.
/usr/bin/python3 tests/tooling/probe_policy.py --tools-root "$PWD/.tools"
```

The probe executes the real rustc/cargo/rustfmt/Clippy and cargo-deny, not mock
API responses. A dependency-free fixture must pass all four deny categories.
A real vendored `rustc-serialize=0.3.24` registry dependency must fail with
`RUSTSEC-2022-0004`; its source is read by Cargo metadata, never built/executed.
Separate negative probes require a GPL-3.0-only root license, a missing database
and missing registry yank metadata to fail. The fixture is not a production
workspace dependency.
Unit tests cover hash rejection, timeout/size cleanup, tar traversal/link escape,
symlink destinations, ownership/modes, failed atomic staging, credential clearing,
subprocess timeout and exact stale/future date boundaries. These fixtures do not
substitute for the parent verifier's actual project, native UI or M0 test gates.

## Sources

[1] https://static.rust-lang.org/dist/channel-rust-1.98.1.toml
[2] https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2
[3] https://raw.githubusercontent.com/EmbarkStudios/cargo-deny/bca0dde53651ee946720e4540b5ce2610bec8f06/src/advisories/helpers/db.rs
[4] https://raw.githubusercontent.com/EmbarkStudios/cargo-deny/bca0dde53651ee946720e4540b5ce2610bec8f06/src/git.rs
[5] https://api.github.com/repos/RustSec/advisory-db/commits/5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5
[7] https://raw.githubusercontent.com/RustSec/advisory-db/5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5/LICENSE.txt
[9] https://raw.githubusercontent.com/EmbarkStudios/cargo-deny/bca0dde53651ee946720e4540b5ce2610bec8f06/docs/src/checks/licenses/cfg.md
[10] https://nodejs.org/dist/v26.8.1/SHASUMS256.txt
[11] https://raw.githubusercontent.com/nodejs/node/v26.8.1/LICENSE
