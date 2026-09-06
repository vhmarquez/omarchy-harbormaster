"""Real frozen Cargo policy checks in disposable state; no online mode."""
import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from tooling.dependencies import REGISTRY, cache_record, index_path, read_pins, verify_dependencies
from verification.sandbox import run


def execute(tools, state, command):
    result = run(command, ROOT, tools, state, 240)
    return {"command": command, **result}


def probe(tools, crate="serde"):
    inspection = verify_dependencies(ROOT, tools)
    version = next(p["version"] for p in read_pins(ROOT)["packages"] if p["name"] == crate)
    cargo = ["/tools/rust/bin/cargo", "test", "--frozen", "--workspace"]
    deny = ["/tools/bin/cargo-deny", "--config", "tools/deny.toml", "--frozen",
            "--workspace", "check", "--deny", "warnings", "all"]
    results = {"inspection": inspection}
    for case in ("complete", "missing-source", "corrupt-source", "missing-index", "yanked"):
        with tempfile.TemporaryDirectory(prefix="harbormaster-dependency-probe-") as temporary:
            state = Path(temporary)
            shutil.copytree(tools / "dependencies/cargo", state / "cargo")
            shutil.copytree(tools / "advisory-db", state / "advisory-db")
            archive = state / f"cargo/registry/cache/{REGISTRY}/{crate}-{version}.crate"
            cached = state / f"cargo/registry/index/{REGISTRY}/.cache/{index_path(crate)}"
            if case == "missing-source":
                archive.unlink()
            elif case == "corrupt-source":
                archive.write_bytes(b"invalid crate archive")
            elif case == "missing-index":
                cached.unlink()
            elif case == "yanked":
                rows = []
                for line in (tools / "dependencies/indexes" / crate).read_text().splitlines():
                    entry = json.loads(line)
                    if entry["vers"] == version:
                        entry["yanked"] = True
                    rows.append(json.dumps(entry))
                cached.write_bytes(cache_record(("\n".join(rows) + "\n").encode()))
            command = deny if case == "yanked" else cargo
            result = execute(tools, state, command)
            if (result["exit_code"] == 0) != (case == "complete"):
                raise AssertionError(f"unexpected {case} result: {result}")
            if case == "yanked" and "yanked" not in result["output"]:
                raise AssertionError("yank fixture failed without a yank diagnostic")
            results[case] = result
            if case == "complete":
                result = execute(tools, state, deny)
                if result["exit_code"] != 0:
                    raise AssertionError(f"full policy failed: {result}")
                results["deny-complete"] = result
    return results


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools-root", type=Path, required=True)
    parser.add_argument("--crate", choices=("serde", "nix"), default="serde")
    args = parser.parse_args()
    print(json.dumps(probe(args.tools_root, args.crate), indent=2))
