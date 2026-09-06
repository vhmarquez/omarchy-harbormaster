"""Real pinned-tool regression probes; fixture preparation and execution are separate.

Run --prepare-fixture once (explicit HTTPS), then rerun without it (network namespace).
The vulnerable crate is read by cargo metadata, NEVER built or executed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from tooling.download import download
from tooling.install import extract, no_symlinks, private_root, run
from tooling.prepare import validate_snapshot_age

FIXTURE_URL = "https://static.crates.io/crates/rustc-serialize/rustc-serialize-0.3.24.crate"
FIXTURE_SHA = "dcf128d1287d2ea9d80910b5f1120d0b8eede3fbf1abe91c40d39ea7d51e6fda"


def sandbox(tools, scratch):
    state = scratch / "state"
    state.mkdir(mode=0o700)
    shutil.copytree(tools / "advisory-db", state / "advisory-db")
    return ["/usr/bin/bwrap", "--unshare-all", "--die-with-parent", "--new-session", "--cap-drop", "ALL",
            "--ro-bind", "/usr", "/usr", "--symlink", "usr/lib", "/lib",
            "--symlink", "usr/lib", "/lib64", "--symlink", "usr/bin", "/bin",
            "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
            "--ro-bind", str(tools), "/tools", "--bind", str(state), "/state",
            "--bind", str(scratch / "project"), "/workspace",
            "--ro-bind", str(ROOT / "tools" / "deny.toml"), "/policy.toml",
            "--chdir", "/workspace"]


def project(scratch, dependency=False):
    workspace = scratch / "project"
    workspace.mkdir()
    (workspace / "src").mkdir()
    (workspace / "src" / "lib.rs").write_text("pub fn foundation() -> bool {\n    true\n}\n")
    manifest = '[package]\nname="policy-probe"\nversion="0.1.0"\nedition="2024"\nlicense="MIT"\n'
    if dependency:
        manifest += '[dependencies]\nrustc-serialize="=0.3.24"\n'
    (workspace / "Cargo.toml").write_text(manifest)
    return workspace


def vendor(archive, workspace):
    extracted = extract(archive, workspace / "vendor")
    source = extracted / "rustc-serialize-0.3.24"
    files = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in source.rglob("*") if p.is_file()}
    (source / ".cargo-checksum.json").write_text(json.dumps({"files": files, "package": FIXTURE_SHA}))
    config = workspace / ".cargo"
    config.mkdir()
    (config / "config.toml").write_text('[source.crates-io]\nreplace-with="vendored"\n'
                                        '[source.vendored]\ndirectory="vendor"\n')


def rejection(execute, args, needle):
    try:
        execute(args)
    except RuntimeError as error:
        if needle not in str(error):
            raise AssertionError(str(error)) from error
        return str(error)
    raise AssertionError(f"expected rejection containing {needle!r}, but command passed")


def probe(tools, scratch, name, dependency=False):
    workspace = project(scratch, dependency)
    if dependency:
        vendor(tools / "policy-fixture.crate", workspace)
    command = sandbox(tools, scratch)
    env = {"HOME": "/state", "TMPDIR": "/tmp", "CARGO_HOME": "/state/cargo",
           "PATH": "/tools/rust/bin:/tools/bin:/usr/bin", "RUSTC": "/tools/rust/bin/rustc",
           "RUSTDOC": "/tools/rust/bin/rustdoc", "CARGO": "/tools/rust/bin/cargo"}
    def execute(args):
        return run(command + args, scratch / "home", extra_env=env)
    outputs = {"lockfile": execute(["/tools/rust/bin/cargo", "generate-lockfile", "--offline"])}
    deny = ["/tools/bin/cargo-deny", "--manifest-path", "/workspace/Cargo.toml", "--config",
            "/policy.toml", "--frozen", "--workspace", "check", "--show-stats", "--deny", "warnings"]
    if dependency:
        outputs["vulnerable_dependency_rejected"] = rejection(execute, deny + ["advisories"], "RUSTSEC-2022-0004")
        outputs["missing_registry_index_rejected"] = rejection(
            execute, deny + ["--allow", "vulnerability", "--allow", "unmaintained", "advisories"], "error[index-failure]")
    else:
        outputs["deny_all"] = execute(deny + ["all"])
        for args in (["test", "--frozen"], ["fmt", "--check"], ["clippy", "--frozen", "--", "-D", "warnings"]):
            outputs[args[0]] = execute(["/tools/rust/bin/cargo", *args])
        manifest = workspace / "Cargo.toml"
        original = manifest.read_text()
        manifest.write_text(original.replace('license="MIT"', 'license="GPL-3.0-only"'))
        outputs["forbidden_license_rejected"] = rejection(execute, deny + ["licenses"], "GPL-3.0-only")
        manifest.write_text(original)
        shutil.rmtree(scratch / "state" / "advisory-db")
        outputs["missing_database_rejected"] = rejection(execute, deny + ["advisories"], "failed to get HEAD timestamp")
    return {"name": name, "network": "bwrap --unshare-all", "outputs": outputs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools-root", required=True, type=Path)
    parser.add_argument("--prepare-fixture", action="store_true", help="explicit online fixture download ONLY")
    args = parser.parse_args()
    tools = private_root(args.tools_root)
    fixture = tools / "policy-fixture.crate"
    no_symlinks(fixture)
    if args.prepare_fixture:
        print(json.dumps(download(FIXTURE_URL, FIXTURE_SHA, fixture), indent=2))
        return
    with fixture.open("rb") as source:
        assert hashlib.file_digest(source, "sha256").hexdigest() == FIXTURE_SHA, "fixture SHA256 mismatch"
    lock = json.loads((ROOT / "tools" / "toolchain.lock.json").read_text())
    validate_snapshot_age(lock["rustsec"])
    results = []
    for dependency in (False, True):
        with tempfile.TemporaryDirectory(prefix=".policy-probe-", dir=tools) as directory:
            results.append(probe(tools, Path(directory), "registry-vulnerability" if dependency else "dependency-free", dependency))
    print(json.dumps({"fixture": {"url": FIXTURE_URL, "sha256": FIXTURE_SHA}, "results": results}, indent=2))


if __name__ == "__main__":
    main()
