"""Read-only integrity and freshness checks for the reviewed Cargo input graph."""
from datetime import datetime, timedelta, timezone
import hashlib
from pathlib import Path
import re
import shutil
import tomllib

from verification.strict_json import loads
from .inspection import verify_file
from .install import no_symlinks, private_root

REGISTRY = "index.crates.io-1949cf8c6b5b557f"
SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
INDEX_BASE = "https://raw.githubusercontent.com/rust-lang/crates.io-index/"


def index_path(name):
    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", name):
        raise ValueError("invalid registry crate name")
    if len(name) < 3:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def records(raw):
    if len(raw) > 10_000_000:
        raise ValueError("registry record exceeds 10 MB")
    entries = {}
    for line in raw.splitlines():
        entry = loads(line.decode("utf-8"))
        version = entry["vers"]
        if not isinstance(version, str) or "\0" in version or version in entries:
            raise ValueError("invalid/duplicate registry version")
        entries[version] = (line, entry)
    return entries


def cache_record(raw):
    """Cargo 1.98.1 sparse cache v3/index v2; preserve exact upstream row bytes."""
    result = bytearray(b'\x03\x02\x00\x00\x00etag: "pinned"\0')
    for version, (line, _) in records(raw).items():
        result.extend(version.encode() + b"\0" + line + b"\0")
    return bytes(result)


def read_pins(project, now=None):
    pins = loads((project / "tools/dependencies.lock.json").read_text())
    if pins["schema"] != 1 or pins["maximum_age_days"] != 7:
        raise ValueError("unsupported dependency pin policy")
    if not re.fullmatch(r"[0-9a-f]{40}", pins["index_commit"]):
        raise ValueError("invalid registry commit pin")
    date = datetime.fromisoformat(pins["commit_date"])
    if date.tzinfo is None:
        raise ValueError("registry commit date requires a timezone")
    age = (now or datetime.now(timezone.utc)) - date
    if not timedelta(0) <= age < timedelta(days=7):
        raise ValueError("registry snapshot is stale or future-dated; review and repin")
    selected = set()
    for package in pins["packages"]:
        index_path(package["name"])
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", package["version"]):
            raise ValueError("invalid crate version pin")
        for field in ("checksum", "index_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", package[field]):
                raise ValueError("invalid dependency SHA256 pin")
        identity = (package["name"], package["version"], package["checksum"])
        if any(existing[0] == package["name"] for existing in selected):
            raise ValueError("duplicate dependency pin")
        selected.add(identity)
    lock = tomllib.loads((project / "Cargo.lock").read_text())
    graph = set()
    for package in lock["package"]:
        if "source" not in package:
            continue
        if package["source"] != SOURCE:
            raise ValueError("unsupported Cargo.lock source")
        graph.add((package["name"], package["version"], package["checksum"]))
    if graph != selected or not graph:
        raise ValueError("dependency pins do not match Cargo.lock registry graph")
    return pins


def verify_package(group, package):
    name, version = package["name"], package["version"]
    source = Path("cargo/registry/cache") / REGISTRY / f"{name}-{version}.crate"
    raw_path = Path("indexes") / name
    cache = Path("cargo/registry/index") / REGISTRY / ".cache" / index_path(name)
    verify_file(group / source, package["checksum"])
    verify_file(group / raw_path, package["index_sha256"])
    raw = (group / raw_path).read_bytes()
    entries = records(raw)
    entry = entries.get(version, (None, {}))[1]
    if entry.get("name") != name or entry.get("cksum") != package["checksum"]:
        raise ValueError("registry version/checksum does not match dependency pin")
    if entry.get("yanked") is not False:
        raise ValueError(f"locked crate is yanked or missing yank metadata: {name} {version}")
    verify_file(group / cache, hashlib.sha256(cache_record(raw)).hexdigest())
    return {source, raw_path, cache}


def verify_dependencies(project, tools, now=None):
    project, tools = Path(project), Path(tools)
    if not tools.is_dir():
        raise ValueError("missing prepared tools root")
    private_root(tools)
    pins = read_pins(project, now)
    group = tools / "dependencies"
    config = Path("cargo/registry/index") / REGISTRY / "config.json"
    verify_file(group / config, pins["config_sha256"])
    expected = {config}
    for package in pins["packages"]:
        expected.update(verify_package(group, package))
    actual = set()
    for path in group.rglob("*"):
        no_symlinks(path)
        if path.is_file():
            actual.add(path.relative_to(group))
        elif not path.is_dir():
            raise ValueError("non-regular dependency input")
    if actual != expected:
        raise ValueError("unexpected/missing prepared dependency inputs")
    return {"packages": len(pins["packages"]), "index_commit": pins["index_commit"],
            "commit_date": pins["commit_date"], "maximum_age_days": 7,
            "scope": "hash-pinned Cargo sources and registry snapshot; NOT a live yank/advisory audit"}


def stage_cargo(project, tools, destination):
    """Copy only verified inputs into an empty disposable offline Cargo home."""
    report = verify_dependencies(project, tools)
    destination = Path(destination)
    no_symlinks(destination)
    if destination.exists() and any(destination.iterdir()):
        raise ValueError("Cargo home must be empty before dependency staging")
    shutil.copytree(Path(tools) / "dependencies/cargo", destination, dirs_exist_ok=True)
    return report
