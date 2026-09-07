#!/usr/bin/env python3
"""Build an uninstalled CLI with the existing pinned offline confinement."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tempfile

from tooling.install import no_symlinks
from verification import sandbox

ROOT = Path(__file__).resolve().parents[1]


def build(tools, output):
    no_symlinks(tools)
    no_symlinks(output)
    output.mkdir(mode=0o700)
    results = []
    with tempfile.TemporaryDirectory(prefix="harbormaster-build-") as temporary:
        base = Path(temporary)
        source, state = base / "source", base / "state"
        sandbox.snapshot(ROOT, source)
        state.mkdir(mode=0o700)
        commands = [
            ["/usr/bin/python3", "-B", "scripts/verification/preflight.py", "--stage-cargo"],
            ["/usr/bin/python3", "-B", "scripts/verification/sqlite.py"],
            ["/tools/rust/bin/cargo", "build", "--locked", "--offline", "--release",
             "--package", "harbormaster", "--bin", "harbormaster", "--all-features"],
            ["/state/target/release/harbormaster", "--help"],
        ]
        for command in commands:
            result = sandbox.run(command, source, tools, state, 240)
            results.append({"command": command, **result})
            (output / "build.json").write_text(json.dumps(results, indent=2) + "\n")
            if result["exit_code"] != 0 or result["timed_out"] or result["output_limited"]:
                raise ValueError("build failed; see build.json")
        binary = output / "harbormaster"
        shutil.copyfile(state / "target/release/harbormaster", binary)
        binary.chmod(0o700)
        hashes = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in sorted(source.rglob("*")) if p.is_file()}
        (output / "source-sha256.json").write_text(json.dumps(hashes, indent=2) + "\n")
        (output / "sha256.txt").write_text(hashlib.sha256(binary.read_bytes()).hexdigest()
                                          + "  harbormaster\n")
    return binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, default=ROOT / ".tools")
    parser.add_argument("--output", type=Path, required=True,
                        help="new output directory under an existing parent")
    options = parser.parse_args()
    try:
        binary = build(options.tools.absolute(), options.output.absolute())
    except (OSError, ValueError) as error:
        parser.exit(1, f"Build unavailable: {error}\n")
    print(f"Built {binary}; not installed. Qualification uses scripts/verify.py.")


if __name__ == "__main__":
    main()
