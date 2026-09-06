"""Install pinned public tool groups into an explicitly owned root."""
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil

from .download import download
from .install import extract, owned_stage, run
from .inspection import check_rust_version, validate_snapshot_age, verify_existing


def artifact(pin, stage, name):
    archive = stage / (name + ".tar")
    receipt = download(pin["url"], pin["sha256"], archive)
    unpacked = extract(archive, stage / name)
    entries = list(unpacked.iterdir())
    if len(entries) != 1 or not entries[0].is_dir() or entries[0].is_symlink():
        raise ValueError(f"expected one archive root directory: {pin['url']}")
    return entries[0], receipt, archive


def write_receipt(target, receipt):
    (target / "preparation.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt), flush=True)


def install_rust(root, pin):
    with owned_stage(root, "rust") as stage:
        receipt = {"kind": "rust", "version": pin["version"], "artifacts": [], "versions": {}, "binaries": {}}
        destination = stage / str(root / "rust").lstrip("/")
        for component in pin["components"]:
            source, downloaded, _ = artifact(component, stage, component["name"] + "-source")
            run(["/usr/bin/sh", str(source / "install.sh"), f"--prefix={root / 'rust'}",
                 f"--destdir={stage}", "--disable-ldconfig"], stage / "home", cwd=source)
            receipt["artifacts"].append(downloaded)
            shutil.rmtree(source.parent)
        destination.rename(stage / "rust")
        for name in ("rustc", "cargo", "rustfmt", "clippy-driver"):
            binary = stage / "rust" / "bin" / name
            version = run([str(binary), "--version"], stage / "home")
            check_rust_version(name, version, pin)
            receipt["versions"][name] = version
            with binary.open("rb") as source:
                receipt["binaries"][name] = hashlib.file_digest(source, "sha256").hexdigest()
        write_receipt(stage / "rust", receipt)
    return receipt


def install_deny(root, pin):
    with owned_stage(root, "bin") as stage:
        source, receipt, _ = artifact(pin, stage, "deny-source")
        target = stage / "bin"
        target.mkdir(mode=0o700)
        shutil.copytree(source, target / "cargo-deny-distribution")
        shutil.copy2(source / "cargo-deny", target / "cargo-deny")
        version = run([str(target / "cargo-deny"), "--version"], stage / "home")
        if version != "cargo-deny " + pin["version"]:
            raise ValueError(f"unexpected cargo-deny version: {version}")
        receipt["version"] = version
        with (target / "cargo-deny").open("rb") as binary:
            receipt["binary_sha256"] = hashlib.file_digest(binary, "sha256").hexdigest()
        write_receipt(target, receipt)
    return receipt


def install_rustsec(root, pin):
    validate_snapshot_age(pin)
    with owned_stage(root, "advisory-db") as stage:
        source, receipt, archive = artifact(pin, stage, "rustsec-source")
        target = stage / "advisory-db"
        target.mkdir(mode=0o700)
        repository = target / pin["directory"]
        source.rename(repository)
        shutil.copy2(archive, target / "snapshot.tar.gz")
        git = ["/usr/bin/git", "-C", str(repository)]
        run(git + ["init", "--initial-branch=snapshot"], stage / "home")
        run(git + ["add", "--all"], stage / "home")
        identity = {"GIT_AUTHOR_NAME": "Harbormaster snapshot", "GIT_COMMITTER_NAME": "Harbormaster snapshot",
                    "GIT_AUTHOR_EMAIL": "snapshot@invalid", "GIT_COMMITTER_EMAIL": "snapshot@invalid",
                    "GIT_AUTHOR_DATE": pin["commit_date"], "GIT_COMMITTER_DATE": pin["commit_date"]}
        run(git + ["-c", "commit.gpgsign=false", "commit", "-m",
                   f"Local archive snapshot of upstream {pin['upstream_commit']}\n\nSHA256: {pin['sha256']}"],
            stage / "home", extra_env=identity)
        timestamp = datetime.fromisoformat(pin["commit_date"]).timestamp()
        os.utime(repository / ".git" / "HEAD", (timestamp, timestamp))
        receipt.update({"upstream_commit": pin["upstream_commit"], "commit_date": pin["commit_date"],
                        "local_snapshot_commit": run(git + ["rev-parse", "HEAD"], stage / "home"),
                        "representation": "local Git commit from hash-verified upstream archive, NOT upstream Git history",
                        "directory": pin["directory"], "maximum_age_days": pin["maximum_age_days"]})
        write_receipt(target, receipt)
    return receipt


def install_node(root, pin):
    with owned_stage(root, "node") as stage:
        source, receipt, _ = artifact(pin, stage, "node-source")
        target = stage / "node"
        source.rename(target)
        version = run([str(target / "bin" / "node"), "--version"], stage / "home")
        if version != "v" + pin["version"]:
            raise ValueError(f"unexpected Node version: {version}")
        receipt["version"] = version
        with (target / "bin" / "node").open("rb") as binary:
            receipt["binary_sha256"] = hashlib.file_digest(binary, "sha256").hexdigest()
        write_receipt(target, receipt)
    return receipt


def prepare(root, lock, only="all"):
    root = Path(root).absolute()
    if platform.system() != "Linux" or platform.machine() != "x86_64" or lock["target"] != "x86_64-unknown-linux-gnu":
        raise ValueError("this lock supports only x86_64-unknown-linux-gnu")
    result: dict = {"tools_root": str(root)}
    for name, key, installer in (("rust", "rust", install_rust), ("deny", "cargo_deny", install_deny),
                                 ("rustsec", "rustsec", install_rustsec), ("node", "node", install_node)):
        if only in (name, "all"):
            result[name] = installer(root, lock[key])
    return result
