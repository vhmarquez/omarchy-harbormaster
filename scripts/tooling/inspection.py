"""Read-only inspection: no downloads, extraction, locks, caches or temporary files."""
from datetime import datetime, timezone, timedelta
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

from .install import clean_env, no_symlinks, private_root


def validate_snapshot_age(pin, now=None):
    now = now or datetime.now(timezone.utc)
    date = datetime.fromisoformat(pin["commit_date"])
    if date.tzinfo is None or not timedelta(0) <= now - date < timedelta(days=pin["maximum_age_days"]):
        raise ValueError("RustSec snapshot is stale or future-dated; review and repin the public database")


def check_rust_version(name, banner, pin):
    expected = pin["binary_versions"][name]
    fields = banner.split()
    if len(fields) < 2 or fields[1].removesuffix("-stable") != expected:
        raise ValueError(f"unexpected {name} version: {banner}; expected {expected}")


def verify_file(path, expected):
    no_symlinks(path)
    if not path.is_file():
        raise ValueError(f"missing prepared file: {path}")
    with path.open("rb") as source:
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    if actual != expected:
        raise ValueError(f"SHA256 mismatch: {path}; expected {expected}, got {actual}; prepare a NEW tools root")
    return actual


def verify_database(root, pin):
    parent = root / "advisory-db"
    archive = parent / "snapshot.tar.gz"
    verify_file(archive, pin["sha256"])
    repository = parent / pin["directory"]
    no_symlinks(repository)
    no_symlinks(repository / ".git")
    if not (repository / ".git" / "HEAD").is_file():
        raise ValueError("missing Git snapshot HEAD; prepare a NEW tools root")
    expected_files = set()
    with tarfile.open(archive) as source:
        for member in source:
            if not member.isfile():
                continue
            relative = Path(*Path(member.name).parts[1:])
            target = repository / relative
            with source.extractfile(member) as content:
                expected = hashlib.file_digest(content, "sha256").hexdigest()
            verify_file(target, expected)
            expected_files.add(relative)
    actual_files = set()
    for path in repository.rglob("*"):
        relative = path.relative_to(repository)
        no_symlinks(path)
        if relative.parts[0] == ".git":
            continue
        if path.is_file():
            actual_files.add(relative)
    if actual_files != expected_files:
        raise ValueError("RustSec snapshot contains unexpected/missing files")
    metadata = json.loads((parent / "preparation.json").read_text())
    for key in ("sha256", "upstream_commit", "commit_date", "directory", "maximum_age_days"):
        if metadata[key] != pin[key]:
            raise ValueError(f"RustSec snapshot metadata does not match lock: {key}")
    return metadata


def verify_existing(root, lock, now=None):
    root = Path(root).absolute()
    no_symlinks(root)
    if not root.is_dir():
        raise ValueError(f"missing tools root: {root}; run explicit online preparation")
    private_root(root)  # Existing directory only: validates marker, never creates it here.
    validate_snapshot_age(lock["rustsec"], now)
    checked = {name: verify_file(root / name, digest) for name, digest in lock["installed_files"].items()}
    snapshot = verify_database(root, lock["rustsec"])
    env = clean_env(Path("/nonexistent"))
    versions = {}
    binaries = {name: "rust/bin/" + name for name in lock["rust"]["binary_versions"]}
    binaries.update({"cargo-deny": "bin/cargo-deny", "node": "node/bin/node"})
    for name, relative in binaries.items():
        try:
            result = subprocess.run([str(root / relative), "--version"], cwd=root, env=env,
                                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, text=True, timeout=15, check=True)
        except (subprocess.SubprocessError, OSError) as error:
            raise ValueError(f"prepared {name} version check failed: {error}") from error
        banner = result.stdout.strip()
        if name in lock["rust"]["binary_versions"]:
            check_rust_version(name, banner, lock["rust"])
        elif banner != ("v" + lock["node"]["version"] if name == "node" else "cargo-deny " + lock["cargo_deny"]["version"]):
            raise ValueError(f"unexpected {name} version: {banner}")
        versions[name] = banner
    return {"tools_root": str(root), "versions": versions, "verified_files": checked, "rustsec": snapshot,
            "scope": "pinned snapshot only, NOT a live audit; executable/library pins plus full advisory archive contents"}
