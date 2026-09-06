#!/usr/bin/env python3
"""Prepare exact public Cargo inputs, separately from offline execution."""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tooling.dependencies import verify_dependencies
from tooling.dependency_prepare import prepare_dependencies


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--online", action="store_true", help="explicit public pinned HTTPS downloads only")
    mode.add_argument("--check", action="store_true", help="read-only offline integrity/freshness check")
    parser.add_argument("--tools-root", required=True, type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    try:
        operation = prepare_dependencies if args.online else verify_dependencies
        print(json.dumps(operation(project, args.tools_root.absolute()), indent=2))
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        print(f"dependency preparation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
