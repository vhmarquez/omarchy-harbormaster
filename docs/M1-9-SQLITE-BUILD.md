# M1 #9 SQLite source and static build

The new storage dependency is `rusqlite=0.40.2`, with default features disabled
and only `backup`, `limits`, and `modern_sqlite`. The actual Cargo.lock graph has
30 registry packages, 27 compiled for pinned Linux. The seven additions are:

| Crate | Exact version | Archive license expression |
| --- | --- | --- |
| rusqlite | 0.40.2 | MIT |
| libsqlite3-sys | 0.38.2 | MIT |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 |
| smallvec | 1.16.0 | MIT OR Apache-2.0 |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| vcpkg | 0.2.15 | MIT/Apache-2.0 |

The previous 23 packages and license policy remain unchanged. The binding's
build dependencies remain in the complete audit graph even though the static
build explicitly disables pkg-config discovery and does not use vcpkg on Linux.
No Cargo cache/default wasm backend, SQLite source-bundling compiler crate,
bindgen, extension loader, async runtime or patched registry archive is enabled.
`modern_sqlite` selects the upstream checked-in binding declarations; SQLite's
stable C ABI permits the newer reviewed patch release below. Authored Rust still
forbids unsafe code. This is a development-source review, not a redistribution
approval or a full platform vulnerability audit.

The registry package's bundled amalgamation is SQLite 3.53.2. The project instead
pins the exact official [SQLite 3.53.4 release](https://sqlite.org/releaselog/3_53_4.html)
(2026-07-24), including its published source ID and sqlite3.c SHA3-256. Upstream
[3.53 branch changes after 3.53.2](https://sqlite.org/src/timeline?from=version-3.53.2&to=version-3.53.4&to2=branch-3.53&y=ci)
include backup/recovery fixes relevant to this task. The older bundled code is
neither modified nor compiled. [Upstream's public-domain dedication](https://sqlite.org/copyright.html)
applies to the downloaded SQLite amalgamation independently of the binding's
MIT license. Preserve the original source notices and review redistribution
requirements when producing a distributable product.

`tools/sqlite.lock.json` records the official HTTPS archive hash, both extracted
source hashes, upstream C-source SHA3 and exact runtime identity. Explicit public
preparation downloads the archive and copies only sqlite3.c and sqlite3.h bytes;
it never compiles or executes them. It refuses an existing source group and
atomically installs only after validation. Every offline preflight checks all
three files, both digest algorithms, the closed inventory and no-symlink paths.
A missing/corrupt source, archive, header, extra file/directory or source link
fails before compilation. Registry and RustSec freshness policies remain the
independent existing seven-day limits; source hashes do not claim a live audit.

```sh
python3 -B scripts/prepare-tools.py --online --tools-root .tools-m1-9
python3 -B scripts/prepare-dependencies.py --online --tools-root .tools-m1-9
python3 -B scripts/prepare-sqlite.py --online --tools-root .tools-m1-9
python3 -B scripts/prepare-sqlite.py --check --tools-root .tools-m1-9
python3 -B scripts/verify.py --tools .tools-m1-9
```

For this local continuation, only already verified public rust/node/bin/advisory
groups were copied into the new owned 0700 `.tools-m1-9`; the new Cargo/source
groups were prepared explicitly. Old `.tools` and `.tools-m1-8-final` remain
untouched. Hosted preparation uses a fresh `.tools` and the same three commands.

The canonical `sqlite-build` check runs the actual C compiler offline under the
unchanged verifier. Source and tools are read-only; `/state/sqlite` must be fresh.
Fixed `/usr/bin/cc` and `/usr/bin/ar` argv build an object and deterministic static
archive. No inherited CC/CFLAGS/pkg-config executable override is forwarded.
Compiler version and actual archive hash are recorded; host/container toolchains
need not produce byte-identical archives. C compilation uses `-O2`, `-fPIC`,
`-fvisibility=hidden` and these reviewed fixed definitions:

- `SQLITE_THREADSAFE=1`: preserve safe bindings' required mutex support.
- `SQLITE_DQS=0`, `SQLITE_DEFAULT_FOREIGN_KEYS=1`: unambiguous SQL and FK default.
- `SQLITE_DEFAULT_FILE_PERMISSIONS=0600`: private files by default.
- `SQLITE_USE_URI=0`, `SQLITE_TRUSTED_SCHEMA=0`: conservative open/schema defaults.
- `SQLITE_TEMP_STORE=3`: temporary SQL structures remain in memory.
- `SQLITE_OMIT_LOAD_EXTENSION=1`: the loader is absent from the compiled library.

[Upstream compile-option documentation](https://sqlite.org/compile.html) warns
that nondefault combinations need testing. Actual C and Rust smoke checks plus
the storage regressions exercise this combination; no upstream full SQLite
regression-suite execution is claimed. Runtime storage configuration must still
set its own lower limits and connection flags. `USE_URI` is reported as a bare
compile-option presence even when its value is zero; the C smoke additionally
executes a URI interpretation rejection and reads the trusted-schema default.

All Cargo checks receive exactly `SQLITE3_NO_PKG_CONFIG=1`, `SQLITE3_STATIC=1`,
`SQLITE3_LIB_DIR=/state/sqlite/lib`, and
`SQLITE3_INCLUDE_DIR=/state/sqlite/include`. The binding's documented linked-build
path consumes this private archive, with no system SQLite fallback accepted by
qualification. The C smoke validates version/source ID/options and ELF NEEDED
entries; `tests/tooling/probe_sqlite.py` repeats identity/linkage against a real
Rust consumer compiled `--locked --offline` in disposable state and proves that
an actual host-dynamic SQLite linkage is rejected. This probe adds a temporary
binary target only inside its source copy; Harbormaster's CLI remains unchanged.

Native qualification includes the build before its real Cargo namespace fixture.
Docker still executes only portable checks with network-none, read-only root,
dropped capabilities, no-new-privileges, a nonroot UID and existing resource
limits. No Docker privilege, seccomp or runner change is needed. Passing this
source/build subtask is not final #9 storage or paired-revision qualification.
