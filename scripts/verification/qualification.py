"""Consistency checks for trusted reports, not execution attestation."""
import hashlib
import math
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile

from .checks import plan, validate
from .sandbox import EXCLUDED, SOURCES, snapshot
from .strict_json import loads


def read_bytes(path, limit):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise ValueError("evidence must be a bounded regular file")
        data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError("evidence size limit exceeded")
        return data


def read_json(path):
    return loads(read_bytes(path, 2 * 1024 * 1024))


def _git(root, *arguments):
    environment = {"PATH": "/usr/bin", "LANG": "C.UTF-8",
                   "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                   "GIT_NO_REPLACE_OBJECTS": "1", "GIT_OPTIONAL_LOCKS": "0",
                   "GIT_TERMINAL_PROMPT": "0"}
    result = subprocess.run(["/usr/bin/git", "-C", str(root), *arguments],
                            env=environment, stdin=subprocess.DEVNULL,
                            capture_output=True, check=True, timeout=10)
    if len(result.stdout) > 2 * 1024 * 1024:
        raise ValueError("Git source inventory exceeds limit")
    return result.stdout


def _commit_blobs(root, revision):
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("revision must be an exact lowercase SHA-1 commit identifier")
    if _git(root, "cat-file", "-t", revision).strip() != b"commit":
        raise ValueError("revision is not a commit")
    records = _git(root, "ls-tree", "-rz", "--full-tree", revision, "--", *SOURCES)
    blobs = {}
    for record in records.split(b"\0"):
        if not record:
            continue
        metadata, encoded = record.split(b"\t", 1)
        name = os.fsdecode(encoded)
        if any(part in EXCLUDED or part.startswith(".env") for part in Path(name).parts):
            continue
        mode, kind, digest = metadata.split()
        if mode not in {b"100644", b"100755"} or kind != b"blob":
            raise ValueError("commit source is not a regular file")
        blobs[name] = digest.decode("ascii")
    return blobs


def source_at_revision(root, revision):
    """Bind the existing reviewed snapshot roots to exact committed file bytes."""
    blobs = _commit_blobs(root, revision)
    hashes = {}
    with tempfile.TemporaryDirectory(prefix="harbormaster-qualification-") as tmp:
        source = Path(tmp) / "source"
        snapshot(root, source)
        for path in sorted(source.rglob("*")):
            if not path.is_file():
                continue
            name = str(path.relative_to(source))
            data = read_bytes(path, 32 * 1024 * 1024)
            git_blob = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
            if blobs.get(name) != git_blob:
                raise ValueError("checkout source differs from requested revision")
            hashes[name] = hashlib.sha256(data).hexdigest()
    if not hashes or set(hashes) != set(blobs):
        raise ValueError("revision source inventory differs from checkout")
    return hashes


def summarize(checks, scope):
    """A selected PASS never implies an unexecuted qualification passed."""
    required = [row for row in checks if row.get("required") is True]
    expected = [name for name, _, _ in plan(scope=scope)]
    valid_inventory = [row["name"] for row in required] == expected
    by_name = {row["name"]: row for row in required}
    result = {"portable": "NOT_RUN", "native": "NOT_RUN"}
    for group in result:
        if scope not in ("all", group):
            continue
        good = valid_inventory and all(by_name[name]["status"] == "PASS"
                                       for name, _, _ in plan(scope=group))
        result[group] = "PASS" if good else "FAIL"
    return result


def _inventory(rows, scope):
    expected = [name for name, _, _ in plan(scope=scope)]
    optional = {"m0-live-runtime", "m0-live-harness", "native-shell-integration"}
    seen = set()
    required = []
    if not isinstance(rows, list):
        raise ValueError("checks must be a list")
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("name"), str):
            raise ValueError("check must be a named object")
        if (not isinstance(row.get("status"), str)
                or row["status"] not in {"PASS", "FAIL", "SKIP", "NOT_RUN"}):
            raise ValueError("check status must be explicit and known")
        name = row["name"]
        if name in seen or name not in (*expected, *optional):
            raise ValueError("duplicate or unknown check")
        seen.add(name)
        if row.get("required") is not (name in expected):
            raise ValueError("check requirement differs from plan")
        if name in expected:
            required.append(name)
    if required != expected:
        raise ValueError("required check inventory differs from scoped plan")


def _check(row, name, argv, kind, directory):
    elapsed = row.get("elapsed_seconds")
    if (row.get("status") != "PASS" or type(row.get("exit_code")) is not int
            or row["exit_code"] != 0 or row.get("timed_out") is not False
            or row.get("output_limited") is not False
            or type(elapsed) not in (int, float) or elapsed < 0
            or (isinstance(elapsed, float) and not math.isfinite(elapsed))):
        raise ValueError("required check lacks successful execution metadata")
    command = row.get("command")
    if (not isinstance(command, list) or len(command) != len(argv)
            or any(not isinstance(value, str) or "\0" in value for value in command)):
        raise ValueError("check command must be the planned argv")
    if row["name"] == "isolation-probe":
        matches = command[:-1] == argv[:-1] and Path(command[-1]).is_absolute()
    else:
        matches = command == argv
    if not matches:
        raise ValueError("check command differs from scoped plan")
    if row.get("log") != name + ".txt":
        raise ValueError("check log differs from planned filename")
    text = read_bytes(directory / (name + ".txt"), 4 * 1024 * 1024).decode("utf-8")
    if not validate(kind, text):
        raise ValueError("required check log fails its result validator")


def _report(directory, scope, backend):
    report = read_json(directory / "report.json")
    expected = {group: "PASS" if group == scope else "NOT_RUN"
                for group in ("portable", "native")}
    if (not isinstance(report, dict) or type(report.get("schema")) is not int
            or report.get("schema") != 2 or report.get("scope") != scope
            or report.get("backend") != backend or report.get("passed") is not True
            or report.get("qualification_complete") is not False
            or report.get("qualifications") != expected):
        raise ValueError("expected successful schema 2 single-scope report")
    _inventory(report.get("checks"), scope)
    required = [row for row in report["checks"] if row["required"]]
    for row, (name, argv, kind) in zip(required, plan(scope=scope)):
        _check(row, name, argv, kind, directory)
    if summarize(report["checks"], scope) != report["qualifications"]:
        raise ValueError("declared qualification differs from check results")
    return report


def _directory(path):
    if not isinstance(path, (str, Path)):
        raise ValueError("qualification directory must be a path")
    path = Path(path).absolute()
    for component in (path, *path.parents):
        if not stat.S_ISDIR(component.lstat().st_mode):
            raise ValueError("qualification directory path must not contain symlinks")
    return path


def qualify(root, portable_directory, native_directory, revision):
    """Check bounded evidence consistency against selected committed source bytes.

    Requires trusted, quiescent source and evidence directories; path checks do
    not make concurrent hostile mutation safe. Neither execution attestation,
    whole-repository identity (excluded roots/.git), nor owner approval follows.
    """
    hashes = source_at_revision(_directory(root), revision)
    for directory, scope, backend in ((portable_directory, "portable", "docker"),
                                      (native_directory, "native", "bwrap")):
        directory = _directory(directory)
        report = _report(directory, scope, backend)
        if "revision" in report and report["revision"] != revision:
            raise ValueError("declared report revision differs from requested commit")
        if read_json(directory / "source-sha256.json") != hashes:
            raise ValueError("evidence source inventory differs from requested revision")
    return {"revision": revision, "qualifications": {"portable": "PASS", "native": "PASS"},
            "qualification_complete": True, "passed": True}
