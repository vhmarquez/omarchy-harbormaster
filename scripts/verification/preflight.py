"""Read-only pin inspection, then verified inputs copied to private Cargo state."""
import argparse
import json
from pathlib import Path
import sys
import tomllib

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tooling.inspection import verify_existing
from tooling.dependencies import stage_cargo, verify_dependencies


def verify(root, tools):
    lock = json.loads((root / "tools/toolchain.lock.json").read_text())
    report = verify_existing(tools, lock)
    # Inspection has checked the actual rustc banner against this binary pin.
    checked_release = lock["rust"]["binary_versions"]["rustc"]
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text())
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    declarations = {
        "channel": toolchain["toolchain"]["channel"],
        "workspace_rust_version": workspace["workspace"]["package"]["rust-version"],
        "lock_release": lock["rust"]["version"],
        "checked_rustc_release": checked_release,
    }
    if any(value != checked_release for value in declarations.values()):
        raise ValueError("Rust declaration mismatch: " + json.dumps(declarations, sort_keys=True))
    report["rust_declarations"] = declarations
    report["dependencies"] = verify_dependencies(root, tools)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage-cargo", action="store_true")
    options = parser.parse_args()
    try:
        report = verify(Path.cwd(), Path("/tools"))
        if options.stage_cargo:
            stage_cargo(Path.cwd(), Path("/tools"), Path("/state/cargo"))
        print(json.dumps(report, indent=2))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"FAIL tool preflight: {error}", file=sys.stderr)
        raise SystemExit(1)
