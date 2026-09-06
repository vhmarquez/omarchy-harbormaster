"""Compile the verified SQLite source offline into the fresh private check state."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tooling.install import no_symlinks, run
from tooling.sqlite_source import read_pins, verify_sqlite

# No environment-selected compiler/options, arbitrary source path, or shared output.
DEFINES = ("SQLITE_THREADSAFE=1", "SQLITE_DQS=0", "SQLITE_DEFAULT_FOREIGN_KEYS=1",
           "SQLITE_DEFAULT_FILE_PERMISSIONS=0600", "SQLITE_USE_URI=0",
           "SQLITE_TRUSTED_SCHEMA=0", "SQLITE_TEMP_STORE=3", "SQLITE_OMIT_LOAD_EXTENSION=1")
OPTIONS = ("THREADSAFE=1", "DQS=0", "DEFAULT_FOREIGN_KEYS", "USE_URI",
           "DEFAULT_FILE_PERMISSIONS=0600", "TEMP_STORE=3", "OMIT_LOAD_EXTENSION")
PROBE = r'''#include "sqlite3.h"
#include <stdio.h>
int main(void) {
  printf("version=%s\nnumber=%d\nsource=%s\n", sqlite3_libversion(), sqlite3_libversion_number(), sqlite3_sourceid());
  for (int i=0; sqlite3_compileoption_get(i); ++i) printf("option=%s\n", sqlite3_compileoption_get(i));
  sqlite3 *db = 0;
  if (sqlite3_open("file:/harbormaster-no-such-directory/probe?mode=memory", &db) != SQLITE_CANTOPEN) return 4;
  sqlite3_close(db);
  if (sqlite3_open(":memory:", &db) != SQLITE_OK) return 2;
  int trusted = -1;
  if (sqlite3_db_config(db, SQLITE_DBCONFIG_TRUSTED_SCHEMA, -1, &trusted) != SQLITE_OK || trusted != 0) return 5;
  if (sqlite3_exec(db, "CREATE TABLE smoke (v INTEGER); INSERT INTO smoke VALUES (1)", 0, 0, 0) != SQLITE_OK) return 3;
  return sqlite3_close(db) != SQLITE_OK;
}
'''


def validate_runtime(output, dynamic, pins):
    lines = output.splitlines()
    expected = [f"version={pins['version']}", f"number={pins['version_number']}", f"source={pins['source_id']}"]
    if lines[:3] != expected:
        raise ValueError("loaded SQLite version/source identity mismatch")
    actual = set(line.removeprefix("option=") for line in lines[3:])
    if not set(OPTIONS) <= actual:
        raise ValueError("loaded SQLite compile options mismatch: " + repr(sorted(set(OPTIONS) - actual)))
    needed = re.findall(r"\(NEEDED\).*\[([^]]+)\]", dynamic)
    if not needed or any("sqlite" in name.lower() for name in needed):
        raise ValueError("unexpected dynamic SQLite linkage")
    return {"version": pins["version"], "source_id": pins["source_id"],
            "compile_options": sorted(actual), "dynamic_needed": needed, "sqlite_linkage": "static"}


def build(project, tools, destination):
    report = verify_sqlite(project, tools)
    pins = read_pins(project)
    destination = Path(destination)
    no_symlinks(destination)
    # The verifier starts with a new private /state for every execution bundle.
    # Never silently reuse an old library, even if its name looks correct.
    destination.mkdir(mode=0o700)
    include, lib = destination / "include", destination / "lib"
    include.mkdir(mode=0o700)
    lib.mkdir(mode=0o700)
    shutil.copyfile(Path(tools) / "sqlite-source/sqlite3.h", include / "sqlite3.h")
    commands = [
        ["/usr/bin/cc", "-O2", "-fPIC", "-fvisibility=hidden", *["-D" + value for value in DEFINES],
         "-c", str(Path(tools) / "sqlite-source/sqlite3.c"), "-o", str(destination / "sqlite3.o")],
        ["/usr/bin/ar", "rcsD", str(lib / "libsqlite3.a"), str(destination / "sqlite3.o")],
    ]
    for command in commands:
        run(command, destination, timeout=180)
    (destination / "probe.c").write_text(PROBE)
    probe = destination / "probe"
    run(["/usr/bin/cc", "-O2", "-I", str(include), str(destination / "probe.c"),
         str(lib / "libsqlite3.a"), "-pthread", "-lm", "-o", str(probe)], destination, timeout=30)
    output = run([str(probe)], destination, timeout=10)
    dynamic = run(["/usr/bin/readelf", "-d", str(probe)], destination, timeout=10)
    report["runtime"] = validate_runtime(output, dynamic, pins)
    report["compiler"] = run(["/usr/bin/cc", "--version"], destination, timeout=10).splitlines()[0]
    report["defines"] = list(DEFINES)
    report["static_archive_sha256"] = hashlib.sha256((lib / "libsqlite3.a").read_bytes()).hexdigest()
    report["scope"] = "actual offline C build/static link/runtime smoke; Rust consumers also check loaded identity"
    return report


if __name__ == "__main__":
    try:
        print(json.dumps(build(Path.cwd(), Path("/tools"), Path("/state/sqlite")), indent=2))
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        print(f"FAIL SQLite offline build: {error}", file=sys.stderr)
        raise SystemExit(1)
