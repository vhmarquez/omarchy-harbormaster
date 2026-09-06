# Dependency, platform and license gates — M1 #7

## Scope and required gates

This is a reviewed development-tool baseline, **not a complete SBOM**, legal
opinion, distro vulnerability audit or product redistribution approval.
Harbormaster's authored Rust workspace is MIT; external runtimes, tools, their
bundled components and archived assets are not automatically MIT.
**Redistribution is not approved** by this document or by a green development
check. No installable manager, Qt bundle, Quickshell plugin or container release
is produced by #7. Preserve [the owner decision](M0-OWNER-DECISIONS.md) and
[frozen design provenance](design/PROVENANCE.md).

The single required entry point is `python3 scripts/verify.py` (local bwrap), or
`python3 scripts/verify.py --backend docker --scope portable --image sha256:<local-image-ID>` in CI.
The owner-approved split additionally requires `--scope native` on isolated native
Linux and paired exact-revision evidence; Docker PASS alone is incomplete. See
[M1-VERIFICATION.md](M1-VERIFICATION.md).
Its required checks must fail when unavailable, unexecuted, failed or skipped;
optional live Omarchy probes must not masquerade as generic Qt coverage.
The workflow must not replicate the Rust/QML checks or suppress its exit code.

- Rust: the separate pinned `cargo-deny` policy covers the actual workspace,
  including private, build and development dependencies. The #7 authored MIT
  baseline is extended by the reviewed #8 graph below and the #9 SQLite graph. Its advisory, license, source and bans gates are required,
  with an explicitly prepared, pinned, freshness-checked RustSec database.
  Rust/tool/advisory pins belong to their dedicated lock/policy files, not this
  distro manifest. An absent database or new unreviewed license fails closed.
- External tools: `tools/platform-lock.json` records exact direct distro
  packages, observed package-license metadata and public integrity pins. The
  image build checks installed package versions and Qt/Python/Node releases.
  `tests/verification/test_ci_contract.py` is a required configuration drift
  gate for the workflow, image protections, explicit license baseline and frozen
  font bytes. This does not compute transitive license compatibility.
- Native Qt: QtQuick/QtTest fixtures, formatter, linter and offscreen tests remain
  required checks in the verifier. They do not exercise Omarchy or Quickshell.
- New crates, QML imports, external tools, fonts or copied source require a PR
  updating the appropriate lock, this scope table and applicable positive and
  negative gate fixtures. A license exception requires explicit review, scope,
  rationale and a tracking issue; do not broaden policy just to obtain green.

## #9 SQLite dependency scope

[The SQLite source/build review](M1-9-SQLITE-BUILD.md) records the seven added
crate pins, the separate official SQLite 3.53.4 source pin, actual static build
and linkage requirements, and unchanged license/advisory/yank policies. The full
Cargo graph now has 30 registry packages. The #8 table below is historical.

## #8 Cargo dependency scope

Direct exact pins are `rustix=1.1.4` with `fs`, `process`, `rand` plus its
default `std`; `nix=0.31.3` with default features disabled and only `socket`;
`serde=1.0.229` with `derive`; and `serde_json=1.0.151` with default `std`.
They provide safe OS wrappers, nonblocking generation entropy and typed JSON. No network listener,
async runtime, database, UUID/randomness package or future milestone scaffold
is added. The complete resolver graph has 23 registry packages; only 20 compile
on the pinned Linux target. Cargo also locks the alternate-platform `errno`,
`windows-sys` and `windows-link` sources; all 23 remain covered by the
frozen license/advisory/source/bans gate rather than excluding unbuilt targets.

`tools/dependencies.lock.json` records every exact version/checksum and immutable
registry record hash; [TOOLCHAIN.md](TOOLCHAIN.md) documents explicit public
preparation and the independent seven-day registry snapshot freshness rule.
Cargo code executes only after archive and index inspection in the existing
offline boundary. `--frozen --deny warnings`, yank denial and every existing
license policy remain unchanged.

Archive `Cargo.toml` license expressions were inspected for all 23 pins:

| Packages | Declared expression and review |
| --- | --- |
| autocfg, bitflags, cfg-if, errno, itoa, libc, proc-macro2, quote, serde, serde_core, serde_derive, serde_json, syn, windows-link, windows-sys | `MIT OR Apache-2.0`; existing allowlist applies. |
| rustix, linux-raw-sys | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`; existing MIT/Apache alternatives suffice without a new exception. |
| memchr | `Unlicense OR MIT`; existing MIT alternative suffices. |
| unicode-ident | `(MIT OR Apache-2.0) AND Unicode-3.0`; existing Unicode-3.0 permission and notices remain necessary. |
| cfg_aliases, memoffset, nix, zmij | `MIT`; existing allowlist applies. |

The `nix` addition resolves a concrete peer-credential review finding. Linux
returns PID zero when a Unix peer is outside the receiver's PID namespace; a
same-UID disposable nested-native fixture observed `(0, 1000, 1000)`. Pinned
rustix 1.1.4 reads `SO_PEERCRED` into `UCred.pid: Pid(NonZeroI32)` without validating
that field first. A post-call zero check cannot repair this representation.
The `rustix` `net` feature is therefore disabled. Nix 0.31.3 wraps `libc::ucred`
with raw integer fields, so the IPC layer can reject nonpositive PID evidence
before constructing its own peer record. This is a scoped reviewed API choice,
not a claim of demonstrated exploitation or an existing upstream advisory.
See [the Unix credential manual](https://www.man7.org/linux/man-pages/man7/unix.7.html),
[PID namespaces](https://www.man7.org/linux/man-pages/man7/pid_namespaces.7.html),
[rustix UCred](https://docs.rs/rustix/1.1.4/rustix/net/struct.UCred.html) and
[nix UnixCredentials](https://docs.rs/nix/0.31.3/nix/sys/socket/struct.UnixCredentials.html).
Exact archive/internal source hashes and the actual kernel fixture output are in
`evidence/m1-8/peercred-dependencies/peercred-finding.json`. The corrective native
Rust fixture and final integrated revision qualification are separate records.
No authored unsafe code, nightly feature or namespace relaxation is introduced.

Full source archives retain their upstream notices. A passing development gate
does not authorize redistribution or replace the required distribution-specific
notice/linking review. The full four-category actual offline cargo-deny result
and hostile missing/corrupt/yanked cache probes are recorded in
`evidence/m1-8/peercred-dependencies/nix-offline-policy.json`.

## Reproducible inputs, not a frozen host kernel

`tools/ci.Dockerfile` starts from the official Arch Linux image at the full
index digest recorded in `platform-lock.json`; the registry returned that
content digest and a Linux/amd64 manifest during public qualification.[8]
The official image is a weekly Arch base, not a complete verification image.[3]
The workflow builds for Linux/amd64, records Docker's resulting local immutable
image ID, then gives that ID—not a mutable tag—to the same verifier.

The Arch Linux Archive **2026/09/05** core and extra databases were retrieved
successfully and their complete downloaded bytes SHA-256 checked. The Dockerfile
checks those exact database hashes before resolution; all normal packages and
transitive dependencies resolve from only those databases. It does not refresh
with `-Sy`, use live mirror lists or turn off package signatures.[1][2]

Keep the snapshot's **qt6-base 6.11.2-3** and **nodejs 26.8.1-2** selection; do
not downgrade packages solely to match an existing host. Both exact packages
were downloaded, SHA-256 checked against the pinned database and their
`.PKGINFO` read. Their detached signatures verified with `gpgv` against a
temporary dearmored copy of the installed Arch public keyring.[19][20]
Qt declarative **6.11.2-1** is in the snapshot and its downloaded package hash
agrees with that repository record.[2][11]

| Tool | Local observation / preparation boundary | Pinned CI build expectation (unexecuted) |
| --- | --- | --- |
| Qt | Installed qt6-base **6.11.2-2**, declarative **6.11.2-1**; executed `qmllint --version`: **6.11.2** | qt6-base **6.11.2-3**, declarative **6.11.2-1**; same required Qt tool/runtime release **6.11.2** |
| Python | Executed interpreter **3.14.7** | Distro **3.14.7-1**, interpreter **3.14.7** |
| Node | Ambient **26.7.0** is not the new baseline; explicit private preparation must provide **26.8.1** without replacing the host | Distro **26.8.1-2**, required runtime **26.8.1** |

This historical input matrix predates hosted execution; actual run results are
tracked in PR #49 and [M1 verification](M1-VERIFICATION.md). It is not itself
an executed CI comparison.
Keep package-release differences visible; do not claim bit-identical platforms.

`platform-lock.json` records the URLs and hashes. Hashes from this public HTTPS
qualification establish an auditable integrity baseline, not independent
publisher identity verification; package signature verification happens during
the real image build against its Arch keyring. Registry digest matching is not
an assertion of cosign verification. Input pins do not promise byte-identical
image layers, a fixed hosted kernel, or that local transitive OS libraries equal
CI. The image records its full installed package/version list internally at
`/opt/harbormaster-image-packages.txt`; this is an inventory, not an SBOM audit.

Updates must review base digest, archive database bytes, direct package
hashes, package signatures, tool releases, external licenses and relevant
security changes together. Do not add arbitrary older package overrides to the
snapshot or claim a pin is currently secure merely because it is reproducible.
RustSec does not audit
Arch, Qt, Node or the container. Dependency freshness and distro security review
are distinct from the Cargo advisory result.

## External license scope table

Package metadata lists multiple license identifiers; that list is not itself
an SPDX AND/OR expression or a grant to choose commercial terms. The exact Qt
source headers below distinguish runtime libraries from developer tools.
No Qt commercial license or blanket redistribution exception is assumed.

| Component / exact baseline | Observed license evidence | #7 use and release boundary |
| --- | --- | --- |
| `qt6-base` **6.11.2-3** | `.PKGINFO`: GPL-3.0-only, LGPL-3.0-only, LicenseRef-Qt-Commercial, Qt-GPL-exception-1.0.[19] | System Qt libraries/Qt Test, development fixture only. Review exact linked modules and all bundled third-party notices before any distribution. |
| `qt6-declarative` **6.11.2-1** | Same package license identifiers.[11] | System QML/Quick/Quick Test plus developer tools; not bundled as MIT. |
| Qt Quick and Qt Quick Test **6.11.2** | Reviewed `qquickitem.h` and `quicktest.h`: LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only.[13][14] | Module-specific evidence, not proof that every shipped file has those alternatives. LGPL obligations remain applicable if that route is selected.[6] |
| `qmllint` and `qmlformat` **6.11.2** | Tool source headers: LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0.[15][16] | Invoked as development executables, not linked into Harbormaster or relicensed. Preserve tool licenses if conveying the image/tools. |
| `python` **3.14.7-1** | PSF-2.0 in pinned core metadata.[1] | Development verifier/interpreter; bundled libraries have their own notices. |
| `nodejs` **26.8.1-2** | MIT in the exact package `.PKGINFO`.[20] | Runs archived design tests; this top-level package label is not an audit of all bundled Node components. |
| `gcc` **16.2.1+r23+gd564253eb6c8-1** | GPL-3.0-or-later WITH GCC-exception-3.1; GFDL-1.3-or-later in pinned core metadata.[1] | Linker/compiler support for standalone Rust. Runtime exception scope is not a blanket waiver for distributing compiler code. |
| `bubblewrap` **0.12.0-1** | LGPL-2.1-or-later in the SHA-checked pinned extra database; exact package bytes match that record.[2] | Historical diagnostic package qualification; no longer installed in the portable CI image after the approved split. Existing native `/usr/bin/bwrap` remains mandatory for real sandbox integration. No setuid permission, capability, seccomp or host-policy relaxation is authorized. |
| `git` **2.55.0-1** | GPL-2.0-only in pinned extra metadata.[2] | Reads prepared advisory data; not a Harbormaster runtime dependency. |
| `archlinux-keyring` **20260902-1** | GPL-3.0-or-later in pinned core metadata.[1] | Public package-signature trust data; no personal signing keys are supplied. |
| Quickshell **0.3.1-1** / upstream **v0.3.1** | Exact archive `.PKGINFO`: **LGPL-3.0-only**; upstream tag LICENSE is GNU Lesser GPL version 3, not GPL-only.[12][4] | Existing local platform context only: not installed in this CI image, imported by the generic fixture, copied or distributed by #7. Any future plugin/bundle needs a scoped combined-work/distribution review. |
| Omarchy source at `f4378f0de5b44d331ee943746a97872b718a6c18` | Reviewed source LICENSE contains the MIT grant and David Heinemeier Hansson copyright.[18] | Platform context, not a claim that the whole Arch/Omarchy installation is MIT or that #7 distributes it. No deployment or shell reload. |
| Frozen `JetBrainsMono-Regular.ttf` | Preserved `design/original/assets/OFL.txt`: **OFL-1.1**, Copyright 2020 The JetBrains Mono Project Authors. | Unmodified archived font. Retain copyright and OFL text; do not relicense as MIT, sell it alone, or discard reserved-name/modified-font conditions. Tests lock the original font and license hashes. |
| User-supplied HTML/SVG/PNG/design notes | [Provenance](design/PROVENANCE.md) records **no separate license grant**. | Preserve original bytes and user provenance. Owner selection of MIT does not invent an upstream grant; obtain rights clarification before a use requiring one. |

The approved native split executes the existing installed bubblewrap **0.11.2**;
its real executable hash and kernel are recorded in
[the platform observation](../evidence/m1-7/split-native-platform.json). The
**0.12.0-1** row is historical downloaded-package qualification, not a claim that
the host was upgraded or matches the diagnostic Docker package. No global package
installation or transitive host-license audit is implied.

A later distribution review must identify the actual files and module/linking
mode, preserve copyright/license/third-party notices, determine corresponding
source and applicable installation/relinking/replacement obligations, and check
any modified-font requirements. The LGPL text explicitly discusses combined
works and relinking; merely calling the project MIT cannot satisfy those
conditions.[6] This table neither authorizes distribution nor resolves every
license combination. No SBOM-completeness or legal-approval claim is made.

## CI trust boundary and execution status

The workflow is JSON-form YAML deliberately, so the offline stdlib test can
parse it unambiguously and reject duplicate keys without installing a YAML
parser. `actions/checkout` is full-SHA pinned to reviewed v7.0.1 with
`contents: read`, no persisted credentials, no submodules/LFS and depth one.
The hosted `ubuntu-24.04` label is orchestration infrastructure, not an immutable
OS pin. Preparation runs with an empty allowlisted environment/private HOME
and explicit public-download consent:

```sh
python3 scripts/prepare-tools.py --online --tools-root .tools
python3 scripts/prepare-dependencies.py --online --tools-root .tools
python3 scripts/prepare-sqlite.py --online --tools-root .tools
```

Only public distro/tool/advisory retrieval is allowed in preparation. Image
build context is `tools/`; the Dockerfile copies only `platform-lock.json`,
never source, `.git`, `.tools`, profiles, credentials or sockets. The normal
Docker build requires no privileged option or runner package installation.
The verifier—not a second workflow test implementation—owns the read-only
allowlisted `/work`, read-only `/tools`, private writable `/state`, empty
credential-free environment, network-none, dropped capabilities,
no-new-privileges, nonroot UID, process/memory/CPU bounds and exact-owned-container
cleanup. Required verification failure must remain a failed job.

There are no cache, secret/model-call, deployment or OIDC steps, no
self-hosted runners, and no privileged fork trigger. Checkout itself uses
GitHub's short-lived read token outside the sandbox; this is not credential-free
orchestration. A PR able to rewrite its workflow or outer Python launcher is
outside this sandbox's threat boundary. Review/branch protection remains
necessary; do not execute rewritten untrusted orchestration in a trusted context.
No protection was changed here. The portable job is named `verify (portable)`;
its success does not satisfy the separate mandatory native qualification.

The verifier must write only sanitized reports and bounded test logs under
`.verify/`, never source snapshots, tool caches, credentials or escaping
symlinks. The full-SHA pinned `actions/upload-artifact` v7.0.1 step runs with
`if: always()` outside the sandbox, uploads **only `.verify/**`**, and retains
the artifact for seven days. `include-hidden-files: true` is explicit because
the report directory itself is hidden; it does not broaden the path to `.tools`
or source caches. `if-no-files-found: warn` leaves a bootstrap failure's original
failed status intact when no report exists. No ignored verification failures or
broad artifact upload are enabled. Artifact service credentials stay outside
the test container. Actual historical hosted failures and current-head artifact
results are tracked in [M1-VERIFICATION.md](M1-VERIFICATION.md) and PR #49.
**Configuration tests and public downloads are not a hosted CI PASS.** The local
Docker daemon is permission-denied; this work does not change permissions,
use sudo or reconfigure the daemon. Both mandatory lanes must pass on the real
PR head before acceptance and separately owner-approved merge.

## Sources

[1] https://archive.archlinux.org/repos/2026/09/05/core/os/x86_64/core.db
[2] https://archive.archlinux.org/repos/2026/09/05/extra/os/x86_64/extra.db
[3] https://raw.githubusercontent.com/archlinux/archlinux-docker/master/README.md
[4] https://raw.githubusercontent.com/quickshell-mirror/quickshell/v0.3.1/LICENSE
[6] https://raw.githubusercontent.com/qt/qtdeclarative/v6.11.2/LICENSES/LGPL-3.0-only.txt
[8] https://registry-1.docker.io/v2/library/archlinux/manifests/base
[11] https://archive.archlinux.org/packages/q/qt6-declarative/qt6-declarative-6.11.2-1-x86_64.pkg.tar.zst
[12] https://archive.archlinux.org/packages/q/quickshell/quickshell-0.3.1-1-x86_64.pkg.tar.zst
[13] https://raw.githubusercontent.com/qt/qtdeclarative/v6.11.2/src/quick/items/qquickitem.h
[14] https://raw.githubusercontent.com/qt/qtdeclarative/v6.11.2/src/qmltest/quicktest.h
[15] https://raw.githubusercontent.com/qt/qtdeclarative/v6.11.2/tools/qmllint/main.cpp
[16] https://raw.githubusercontent.com/qt/qtdeclarative/v6.11.2/tools/qmlformat/qmlformat.cpp
[18] https://raw.githubusercontent.com/basecamp/omarchy/f4378f0de5b44d331ee943746a97872b718a6c18/LICENSE
[19] https://archive.archlinux.org/repos/2026/09/05/extra/os/x86_64/qt6-base-6.11.2-3-x86_64.pkg.tar.zst
[20] https://archive.archlinux.org/repos/2026/09/05/extra/os/x86_64/nodejs-26.8.1-2-x86_64.pkg.tar.zst
