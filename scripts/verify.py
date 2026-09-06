#!/usr/bin/env python3
"""Single offline verification entry point. Preparation is an explicit command."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

from verification import qualification
from verification.checks import passed, plan
from verification.runner import execute_checks
from verification.sandbox import run, snapshot

ROOT = Path(__file__).resolve().parents[1]


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, default=ROOT / ".tools")
    parser.add_argument("--output", type=Path, help="New private report directory (must not exist)")
    parser.add_argument("--backend", choices=("bwrap", "docker"), default="bwrap")
    parser.add_argument("--image", help="Built immutable sha256 image ID for Docker")
    parser.add_argument("--scope", choices=("all", "portable", "native"), default="all",
                        help="Required qualification to execute; Docker is portable only")
    parser.add_argument("--qualify", nargs=2, type=Path, metavar=("PORTABLE", "NATIVE"),
                        help="Check paired report directories against --revision")
    parser.add_argument("--revision", help="Exact full commit for paired evidence review")
    options = parser.parse_args()
    if options.qualify:
        if not options.revision:
            parser.error("paired qualification requires --revision")
        if (options.scope != "all" or options.backend != "bwrap" or options.image is not None
                or options.tools != ROOT / ".tools"):
            parser.error("paired qualification cannot select execution options")
    elif options.revision is not None:
        parser.error("--revision requires paired --qualify reports")
    elif options.backend == "docker" and options.scope != "portable":
        parser.error("Docker requires --scope portable; native qualification is separate")
    return options


def output_directory(requested):
    if requested:
        requested.mkdir(mode=0o700)
        return requested.resolve()
    parent = ROOT / ".verify"
    if parent.is_symlink():
        raise ValueError("refusing symlinked verification directory")
    parent.mkdir(mode=0o700, exist_ok=True)
    info = parent.stat()
    if info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError("verification directory must be owned and private")
    return Path(tempfile.mkdtemp(prefix="run-", dir=parent))


def verification(options, output):
    if options.tools.is_symlink() or not options.tools.is_dir():
        raise ValueError("pinned private tools missing; run scripts/prepare-tools.py first")
    tools = options.tools.resolve()
    with tempfile.TemporaryDirectory(prefix="harbormaster-verify-") as temporary:
        base = Path(temporary)
        source, state = base / "source", base / "state"
        canary = base / "outside-canary"
        canary.write_text("synthetic-isolation-canary-not-a-secret")
        snapshot(ROOT, source)
        state.mkdir(mode=0o700)
        if (tools / "advisory-db").is_dir():
            shutil.copytree(tools / "advisory-db", state / "advisory-db", symlinks=False)
        hashes = {str(path.relative_to(source)): hashlib.sha256(path.read_bytes()).hexdigest()
                  for path in sorted(source.rglob("*")) if path.is_file()}
        (output / "source-sha256.json").write_text(json.dumps(hashes, indent=2) + "\n")
        deadline = time.monotonic() + 1800

        def executor(argv):
            timeout = min(240, max(0.01, deadline - time.monotonic()))
            return run(argv, source, tools, state, timeout, options.backend, options.image)

        return execute_checks(plan(str(canary), scope=options.scope), executor, output)


def finish_execution_report(report, scope):
    report["checks"] += [
        {"name": "m0-live-runtime", "required": False, "status": "SKIP",
         "reason": "Separate opt-in desktop/service probe; no deployment authorized"},
        {"name": "m0-live-harness", "required": False, "status": "SKIP",
         "reason": "Installed real harness probes are outside offline #7 verification"},
        {"name": "native-shell-integration", "required": False, "status": "SKIP",
         "reason": "Production Omarchy/QML UI is not implemented in #7"},
    ]
    report["qualifications"] = qualification.summarize(report["checks"], scope)
    # Even the local union lacks hosted Docker evidence; only pairing completes it.
    report["qualification_complete"] = False
    report["passed"] = passed(report["checks"]) and "FAIL" not in report["qualifications"].values()


def build_report(options, output):
    mode = "paired" if options.qualify else options.scope
    report = {"schema": 2, "started_at": datetime.now(timezone.utc).isoformat(),
              "backend": None if options.qualify else options.backend, "scope": mode,
              "checks": [], "passed": False, "qualification_complete": False,
              "qualifications": {"portable": "NOT_RUN", "native": "NOT_RUN"}}
    try:
        if options.qualify:
            report.update(qualification.qualify(ROOT, *options.qualify, options.revision))
        else:
            report["checks"] = verification(options, output)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report["checks"] = [{"name": "qualification" if options.qualify else "preparation",
                             "required": True, "status": "FAIL", "error": type(error).__name__}]
        print(f"FAIL {mode}: {error}")
    if not options.qualify:
        finish_execution_report(report, options.scope)
    return report


def main():
    options = arguments()
    try:
        output = output_directory(options.output)
    except (OSError, ValueError) as error:
        print(f"FAIL report directory: {error}")
        return 1
    report = build_report(options, output)
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"{'PASS' if report['passed'] else 'FAIL'} {report['scope']} verification; "
          f"qualifications={report['qualifications']}; complete={report['qualification_complete']}; "
          f"report: {output / 'report.json'}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
