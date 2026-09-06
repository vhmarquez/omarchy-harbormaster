"""Real frozen Cargo policy checks in disposable state; no online mode."""
import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from tooling.dependencies import REGISTRY, cache_record, verify_dependencies
from verification.sandbox import run


def execute(tools, state, command):
    result = run(command, ROOT, tools, state, 240)
    return {"command": command, **result}


def probe(tools):
    inspection = verify_dependencies(ROOT, tools)
    cargo = ["/tools/rust/bin/cargo", "test", "--frozen", "--workspace"]
    deny = ["/tools/bin/cargo-deny", "--config", "tools/deny.toml", "--frozen",
            "--workspace", "check", "--deny", "warnings", "all"]
    results = {"inspection": inspection}
    for case in ("complete", "missing-source", "corrupt-source", "missing-index", "yanked"):
        with tempfile.TemporaryDirectory(prefix="harbormaster-dependency-probe-") as temporary:
            state = Path(temporary)
            shutil.copytree(tools / "dependencies/cargo", state / "cargo")
            shutil.copytree(tools / "advisory-db", state / "advisory-db")
            archive = state / f"cargo/registry/cache/{REGISTRY}/serde-1.0.229.crate"
            cached = state / f"cargo/registry/index/{REGISTRY}/.cache/se/rd/serde"
            if case == "missing-source":
                archive.unlink()
            elif case == "corrupt-source":
                archive.write_bytes(b"invalid crate archive")
            elif case == "missing-index":
                cached.unlink()
            elif case == "yanked":
                rows = []
                for line in (tools / "dependencies/indexes/serde").read_text().splitlines():
                    entry = json.loads(line)
                    if entry["vers"] == "1.0.229":
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
    args = parser.parse_args()
    print(json.dumps(probe(args.tools_root), indent=2))
