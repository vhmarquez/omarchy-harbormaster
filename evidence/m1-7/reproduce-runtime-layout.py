"""Reproduce image runtime-directory behavior in disposable bwrap namespaces.

Run from any directory with: python3 -B evidence/m1-7/reproduce-runtime-layout.py
This uses synthetic paths and host Python, NOT Docker or the pinned CI image.
"""
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification import sandbox


def reproduce():
    results = []
    with tempfile.TemporaryDirectory(prefix="hb-ci-layout-") as temporary:
        base = Path(temporary)
        source, state, tools = base / "source", base / "state", base / "tools"
        sandbox.snapshot(ROOT, source)
        tools.mkdir(mode=0o700)
        state.mkdir(mode=0o700)
        canary = base / "outside-canary"
        canary.write_text("synthetic-not-a-secret")
        for name in ("home", "cargo", "target", "config", "cache", "data", "state", "runtime", "tmp"):
            (state / name).mkdir(mode=0o700)
        for label, image_runtime, mask_runtime, git_visible, expected in (
            ("empty-runtime-baseline", False, False, False, 0),
            ("image-runtime-directory", True, False, False, 1),
            ("private-runtime-overlay", True, True, False, 0),
            ("git-leak-still-rejected", True, True, True, 1),
        ):
            if git_visible:
                (source / ".git").mkdir()
            argv = sandbox._bubblewrap(source, tools, state)
            extra = ["--dir", "/run/user"] if image_runtime else []
            if mask_runtime:
                extra += ["--tmpfs", "/run", "--remount-ro", "/run"]
            argv[argv.index("--remount-ro"):argv.index("--remount-ro")] = extra
            argv += ["/usr/bin/python3", "-B", "scripts/verification/probe.py", str(canary)]
            result = sandbox._execute(argv, 15)
            result.update(name=label, expected_exit=expected)
            results.append(result)
            if result["exit_code"] != expected or result["timed_out"] or result["output_limited"]:
                raise RuntimeError(result)
            if expected and "host credentials or desktop paths are exposed" not in result["output"]:
                raise RuntimeError(result)
    return {"scope": "Synthetic namespace reproduction; NOT Docker execution", "cases": results}


if __name__ == "__main__":
    print(json.dumps(reproduce(), indent=2))
