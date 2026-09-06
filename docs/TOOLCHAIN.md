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
even with `yanked="deny"`. Promoting warnings closes that gap. The #7 zero-dependency baseline needed no registry cache. The #8 reviewed
source/index preparation below supplies the newly locked graph; an absent cache
must fail, never trigger a verifier fetch.

## #8 public dependency preparation and offline state

`tools/dependencies.lock.json` binds the exact 23 registry packages in
`Cargo.lock` to their crates.io archive checksums and full registry entry bytes
from crates.io-index commit `9137a21173fbb8ae8536ae5b96cd1fa9da3ddfa3`
(`2026-09-06T13:20:09Z`). The index config is separately hash-pinned. Registry
files use immutable raw GitHub commit URLs; archives use public static.crates.io
URLs. These HTTPS observations are the reviewed integrity baseline, not an
independent signature or live registry audit. The existing RustSec snapshot and
seven-day policy remain independently required.

Explicitly prepare the missing group in the already-owned root, separately from
verification:

```sh
env -i PATH=/usr/bin HOME=/nonexistent /usr/bin/python3 -I \
  scripts/prepare-dependencies.py --online --tools-root "$PWD/.tools"
env -i PATH=/usr/bin HOME=/nonexistent /usr/bin/python3 -I \
  scripts/prepare-dependencies.py --check --tools-root "$PWD/.tools"
```

Preparation downloads data only and executes no crate/build script. It reuses
the existing private root, nonblocking ownership lock, atomic staging and
credential-free bounded HTTPS downloader. Source archives are capped at 25 MB
each and raw registry entries at 10 MB. Existing installed groups cannot be
overwritten; a failure removes only this operation's temporary staging tree.
Upgrades require a new explicitly prepared tools root; no in-place cache refresh
or package update is implied.

For the reviewed peer-credential correction, the fresh local root is
`/home/vhm/Work/omarchy-harbormaster/.tools-m1-8-final`. Existing public groups
`rust`, `node`, `bin` and `advisory-db` were copied by those exact names only
from the independently inspected original `.tools` root, retaining timestamps
and license files; the read-only inspection API reverified their binaries,
libraries and snapshot before use. New dependency preparation populated only
the missing `dependencies` group. The original root was neither overwritten nor
removed. Local execution passes this fresh root via `scripts/verify.py --tools`;
CI still prepares a completely fresh `.tools` using the same reviewed locks.

The installed `.tools/dependencies` contains source `.crate` archives, full raw
registry entries and deterministic Cargo sparse-cache records. It contains no
Cargo configuration overrides or expanded/executable source tree. Cargo 1.98.1's
cache version 3 / index version 2 representation is a pinned compatibility
input; real frozen Cargo and cargo-deny probes verify that both consume it.

The canonical `tool-pins` preflight verifies every archive, raw index record,
generated cache byte and index config; compares the exact registry graph with
`Cargo.lock`; rejects missing, extra, symlinked or corrupt files; and rejects
locked versions with absent/true yank status. Its independent index freshness
rule is `0 <= age < 7 days` using the pinned upstream commit timestamp. Touching
cache files cannot refresh it. Review and repin the index at least weekly and
when a relevant yank warrants it. No live yank status is claimed after the
snapshot. A reviewed new yank must fail, rather than receive an exception.

After all pin checks pass, `preflight.py --stage-cargo` copies only those verified
inputs from read-only `/tools/dependencies/cargo` into initially empty private
`/state/cargo`. Ordinary preflight invocation and the `--check` preparation mode
remain read-only. Cargo expands source only in disposable, network-disabled
state; dependency build scripts and procedural macros execute within the existing
sandbox. No host Cargo cache, user configuration, credentials or writable tool
mount is supplied. The Docker and mandatory separate native qualification
protections remain unchanged; native qualification also requires current pins.

`tests/tooling/test_dependencies.py` covers integrity, freshness, yank semantics,
lock equality, malformed records, private copies and failed atomic preparation.
Run `python3 -B tests/tooling/probe_dependencies.py --tools-root "$PWD/.tools"`
for real frozen offline build/policy probes and negative missing/corrupt source,
missing registry and yanked-version fixtures. Add `--crate nix` to target the
peer-credential dependency; default `serde` retains the original graph probes. These probes use disposable state
and do not mutate prepared inputs or start a service. Scoped observed results
are in `evidence/m1-8/dependencies/`; final integrated revision qualification is
separate evidence.

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
