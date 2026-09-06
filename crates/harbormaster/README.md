# Harbormaster M1 library and CLI foundation

The package contains the CLI foundation, typed protocol, private Unix IPC,
volatile admission and bounded SQLite storage. The public library is exercised
only in disposable isolated fixtures; the executable has no operational manager
mode. The exact dependency/source graph and separate static SQLite preparation
are documented in [the build review](../../docs/M1-9-SQLITE-BUILD.md).
[Storage guarantees](../../docs/M1-9-STORAGE-ENGINE.md) and
[protocol boundaries](../../docs/M1-8-PROTOCOL.md) remain separate from future
reducers, adapters and UI. ADR 0001's daemon/bridge is not scaffolded here.

## Command contract

| Arguments | Standard output | Standard error | Exit status |
| --- | --- | --- | --- |
| none, `--help`, or `-h` | Foundation help | empty | 0 |
| `--version` or `-V` | `harbormaster <Cargo package version>` | empty | 0 |
| any unsupported single argument | empty | Fixed usage error | 2 |
| more than one argument, including combined flags | empty | Fixed excess-arguments error | 2 |
| one non-Unicode argument | empty | Fixed encoding error | 2 |

No arguments mean help, never an implicit runtime start. `daemon`, `bridge`,
`help`, `version`, `--`, and arbitrary positional arguments are unsupported.
Arity is checked before encoding: excess arguments always get the same error.
Diagnostics do not quote argument values, paths, environment values, or OS I/O
errors. Output failures return status 1 with a fixed diagnostic if stderr is
available. The version comes from `env!("CARGO_PKG_VERSION")`, not a second
manually maintained value.

`interpret` is pure argument interpretation; `main` owns argument collection,
stdout/stderr, and exit status. Neither layer reads configuration or profiles,
creates state, starts subprocesses, nor performs networking.

## Tests and isolation

Run required Rust checks through `scripts/verify.py` from the repository root
with the documented prepared tools root. The verifier first validates public
inputs and builds the exact static SQLite source offline, then runs formatting,
strict all-target Clippy and each separately required test family. Native
qualification and actual hosted Docker/native pairing remain mandatory; see
[M1 verification](../../docs/M1-VERIFICATION.md).

The storage suites exercise private manager files, bounded worker receipts,
transactions, known migration/backup/recovery, supported retention and real
process interruption. Their synthetic SQL fault injection is confined to test
fixtures; no arbitrary SQL or callback interface is exposed by storage.

CLI integration tests execute the actual Cargo-built binary with a cleared
environment and synthetic private HOME/XDG/profile/work-directory fixtures.
They compare the full fixture tree before and after every invocation. Unit
tests cover the pure result and typed error contract, including malformed
arguments. These fixture assertions prove no changes in those trees, not
arbitrary-host-write detection. Running tests in a separate worktree is not
security isolation: use the verifier's filesystem, credential, desktop, and
network restrictions. No live harness, profile, credential, or model is needed.

Workspace unsafe code is forbidden. Clippy `all`/`pedantic` are enabled, with
50-line and 50-point complexity review triggers in `clippy.toml`. Any cohesive
exception requires a written review disposition, not a blanket lint allowance.
