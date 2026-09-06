"""Actual static Cargo link and hostile source/linkage probes in private state."""
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from tooling.install import run
from tooling.sqlite_source import read_pins
from verification.sqlite import validate_runtime

SQLITE_ENV = {"SQLITE3_NO_PKG_CONFIG": "1", "SQLITE3_STATIC": "1",
              "SQLITE3_LIB_DIR": "/state/sqlite/lib", "SQLITE3_INCLUDE_DIR": "/state/sqlite/include",
              "CARGO_HOME": "/state/cargo", "CARGO_TARGET_DIR": "/state/target",
              "PATH": "/tools/rust/bin:/tools/bin:/usr/bin"}


def probe():
    project = Path("/state/sqlite-rust-probe")
    shutil.copytree(ROOT, project)
    manifest = project / "crates/harbormaster/Cargo.toml"
    with manifest.open("a") as target:
        target.write('\n[[bin]]\nname="sqlite-probe"\npath="../../tests/tooling/sqlite_probe.rs"\n')
    cargo = ["/tools/rust/bin/cargo", "build", "--manifest-path", str(project / "Cargo.toml"),
             "--locked", "--offline", "--bin", "sqlite-probe"]
    build_output = run(cargo, Path("/state/home"), timeout=180, extra_env=SQLITE_ENV, cwd=project)
    binary = Path("/state/target/debug/sqlite-probe")
    output = run([str(binary)], Path("/state/home"), timeout=10)
    dynamic = run(["/usr/bin/readelf", "-d", str(binary)], Path("/state/home"), timeout=10)
    actual = validate_runtime(output, dynamic, read_pins(ROOT))
    # Execute an actual host dynamic SQLite probe. Rejection is causal to its
    # dynamic linkage even when a future host happens to have the exact version.
    host = Path("/state/sqlite/host-probe")
    run(["/usr/bin/cc", "-O2", "-I", "/state/sqlite/include", "/state/sqlite/probe.c",
         "-lsqlite3", "-o", str(host)], Path("/state/home"), timeout=30)
    host_dynamic = run(["/usr/bin/readelf", "-d", str(host)], Path("/state/home"), timeout=10)
    try:
        validate_runtime(output, host_dynamic, read_pins(ROOT))
    except ValueError as error:
        if "dynamic SQLite" not in str(error):
            raise
        host_rejection = str(error)
    else:
        raise AssertionError("host dynamic SQLite accepted")
    return {"cargo_command": cargo, "build_output": build_output, "runtime": actual,
            "host_dynamic_rejection": host_rejection, "host_dynamic_section": host_dynamic}


if __name__ == "__main__":
    print(json.dumps(probe(), indent=2))
