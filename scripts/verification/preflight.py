"""Read-only tool inspection and exact workspace Rust declarations preflight."""
import json
from pathlib import Path
import sys
import tomllib

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tooling.inspection import verify_existing


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
    return report


if __name__ == "__main__":
    try:
        print(json.dumps(verify(Path.cwd(), Path("/tools")), indent=2))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"FAIL tool preflight: {error}", file=sys.stderr)
        raise SystemExit(1)
