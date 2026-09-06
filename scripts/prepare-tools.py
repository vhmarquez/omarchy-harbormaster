#!/usr/bin/env python3
"""Explicit online preparation; never called implicitly by verification."""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tooling.prepare import prepare, verify_existing


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--online", action="store_true", help="consent to public hash-pinned HTTPS downloads")
    mode.add_argument("--check", action="store_true", help="read-only offline version/integrity/freshness check")
    parser.add_argument("--tools-root", required=True, type=Path,
                        help="explicit NEW private install root (normally repository/.tools)")
    parser.add_argument("--only", choices=("rust", "deny", "rustsec", "node", "all"), default="all")
    args = parser.parse_args()
    lock = Path(__file__).resolve().parents[1] / "tools" / "toolchain.lock.json"
    try:
        pins = json.loads(lock.read_text())
        result = verify_existing(args.tools_root, pins) if args.check else prepare(args.tools_root, pins, args.only)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"tool preparation failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
