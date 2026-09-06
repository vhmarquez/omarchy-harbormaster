"""Hash-pinned SQLite source inputs; preparation never compiles or executes them."""
import hashlib
from pathlib import Path
import re

from verification.strict_json import loads
from .inspection import verify_file


def read_pins(project):
    pins = loads((Path(project) / "tools/sqlite.lock.json").read_text())
    if pins["schema"] != 1 or not re.fullmatch(r"3\.[0-9]+\.[0-9]+", pins["version"]):
        raise ValueError("unsupported SQLite source pins")
    if not re.fullmatch(r"https://sqlite.org/20[0-9]{2}/sqlite-amalgamation-[0-9]{7}\.zip", pins["url"]):
        raise ValueError("unrecognized official SQLite archive URL")
    for digest in [pins["archive_sha256"], pins["archive_sha3_256"],
                   *(item[key] for item in pins["files"] for key in ("sha256", "sha3_256"))]:
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError("invalid SQLite source digest")
    prefix = pins["url"].rsplit("/", 1)[1][:-4]
    if [item["path"] for item in pins["files"]] != [f"{prefix}/sqlite3.c", f"{prefix}/sqlite3.h"]:
        raise ValueError("unexpected SQLite source members")
    return pins


def _sha3(path, expected):
    with path.open("rb") as source:
        actual = hashlib.file_digest(source, "sha3_256").hexdigest()
    if actual != expected:
        raise ValueError("SQLite source SHA3-256 mismatch")


def verify_sqlite(project, tools):
    """Check every source byte and the closed inventory without writing input."""
    from .install import no_symlinks, private_root
    tools = Path(tools)
    if not tools.is_dir():
        raise ValueError("missing prepared tools root")
    private_root(tools)
    pins = read_pins(project)
    group = tools / "sqlite-source"
    archive = group / pins["url"].rsplit("/", 1)[1]
    verify_file(archive, pins["archive_sha256"])
    _sha3(archive, pins["archive_sha3_256"])
    expected = {archive.name}
    for item in pins["files"]:
        name = Path(item["path"]).name
        expected.add(name)
        verify_file(group / name, item["sha256"])
        _sha3(group / name, item["sha3_256"])
    for path in group.iterdir():
        no_symlinks(path)
        if not path.is_file() or path.name not in expected:
            raise ValueError("unexpected SQLite prepared input")
    if {path.name for path in group.iterdir()} != expected:
        raise ValueError("unexpected/missing SQLite prepared input")
    return {"version": pins["version"], "source_id": pins["source_id"],
            "archive_sha256": pins["archive_sha256"], "source_sha3_256": pins["files"][0]["sha3_256"],
            "scope": "exact official source inputs only; no compilation or execution"}
