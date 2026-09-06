#!/usr/bin/env python3
"""Prepare pinned public SQLite source separately from offline compilation."""
import argparse
import json
from pathlib import Path
import sys
import zipfile

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tooling.sqlite_prepare import prepare_sqlite
from tooling.sqlite_source import verify_sqlite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--online", action="store_true", help="explicit pinned public HTTPS download only")
    mode.add_argument("--check", action="store_true", help="read-only offline integrity check")
    parser.add_argument("--tools-root", type=Path, required=True)
    args = parser.parse_args()
    try:
        operation = prepare_sqlite if args.online else verify_sqlite
        print(json.dumps(operation(Path(__file__).resolve().parents[1], args.tools_root.absolute()), indent=2))
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, zipfile.BadZipFile) as error:
        print(f"SQLite source preparation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
