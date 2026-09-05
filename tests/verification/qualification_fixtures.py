"""SYNTHETIC unit fixtures: no Docker/native execution or attestation.

Only source identity is real: a disposable Git repository with committed bytes.
Successful log strings and execution metadata below are deliberately fabricated
unit inputs, never evidence that a qualification lane actually ran.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from verification.checks import NATIVE_METHODS, plan


class GitFixture:
    def __init__(self, case):
        temporary = tempfile.TemporaryDirectory(prefix="qualification-fixture-")
        case.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.env = {"PATH": "/usr/bin", "HOME": str(self.root),
                    "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_AUTHOR_NAME": "Fixture", "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
                    "GIT_COMMITTER_NAME": "Fixture", "GIT_COMMITTER_EMAIL": "fixture@example.invalid"}
        self.git("init", "-q", "-b", "main")
        (self.root / "README.md").write_text("fixture source\n")
        self.git("add", "README.md")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
        self.revision = self.git("rev-parse", "HEAD").strip()
        self.hashes = {"README.md": hashlib.sha256(b"fixture source\n").hexdigest()}

    def git(self, *args):
        return subprocess.run(["/usr/bin/git", "-C", str(self.root), *args],
                              env=self.env, stdin=subprocess.DEVNULL, text=True,
                              capture_output=True, check=True, timeout=5).stdout


# Minimal success strings exercise real validators, not real execution.
SYNTHETIC_NATIVE_LOG = "".join(
    f"{target.split('.')[-1]} (__main__.{target}) ... ok\n"
    for target in NATIVE_METHODS
) + "\n----------------------------------------------------------------------\nRan 2 tests in 0.001s\n\nOK\n"

SYNTHETIC_LOGS = {
    "native": SYNTHETIC_NATIVE_LOG,
    "exit": "synthetic successful unit input\n",
    "python": "Ran 1 test in 0.001s\n\nOK\n",
    "rust": "test result: ok. 1 passed; 0 failed; 0 ignored;\n",
    "qml": "PASS   : Fixture::test_synthetic()\nTotals: 3 passed, 0 failed, 0 skipped, 0 blacklisted\n",
    "node": "# tests 1\n# fail 0\n# cancelled 0\n# skipped 0\n",
    "metrics": '{"coverage":{"complete":true,"files_analyzed":1}}',
}


class SyntheticPair(GitFixture):
    def __init__(self, case):
        super().__init__(case)
        self.directories = {}
        self.reports = {}
        for scope, backend in (("portable", "docker"), ("native", "bwrap")):
            directory = self.root / "evidence" / scope
            directory.mkdir(parents=True)
            rows = []
            for name, argv, kind in plan(str(self.root / "unused-canary"), scope=scope):
                rows.append({"name": name, "required": True, "command": argv,
                             "status": "PASS", "exit_code": 0, "timed_out": False,
                             "output_limited": False, "elapsed_seconds": 0.001,
                             "log": name + ".txt"})
                (directory / (name + ".txt")).write_text(SYNTHETIC_LOGS[kind])
            rows += [{"name": name, "required": False, "status": "SKIP",
                      "reason": "synthetic optional unit input"}
                     for name in ("m0-live-runtime", "m0-live-harness", "native-shell-integration")]
            self.directories[scope] = directory
            self.reports[scope] = {
                "schema": 2, "scope": scope, "backend": backend, "checks": rows,
                "passed": True, "qualification_complete": False,
                "qualifications": {group: "PASS" if group == scope else "NOT_RUN"
                                   for group in ("portable", "native")}}
            (directory / "source-sha256.json").write_text(json.dumps(self.hashes))
        self.write_reports()

    def write_reports(self):
        for scope, report in self.reports.items():
            (self.directories[scope] / "report.json").write_text(json.dumps(report))

    def qualify(self):
        from verification.qualification import qualify
        return qualify(self.root, self.directories["portable"], self.directories["native"],
                       self.revision)


class CliSyntheticPair(SyntheticPair):
    """Actual paired CLI over committed verifier bytes; receipts remain synthetic."""
    def __init__(self, case):
        super().__init__(case)
        scripts = Path(__file__).resolve().parents[2] / "scripts"
        target = self.root / "scripts"
        target.mkdir()
        shutil.copyfile(scripts / "verify.py", target / "verify.py")
        shutil.copytree(scripts / "verification", target / "verification",
                        ignore=shutil.ignore_patterns("__pycache__"))
        self.git("add", "scripts")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "verifier fixture")
        self.revision = self.git("rev-parse", "HEAD").strip()
        from verification.qualification import source_at_revision
        self.hashes = source_at_revision(self.root, self.revision)
        for directory in self.directories.values():
            (directory / "source-sha256.json").write_text(json.dumps(self.hashes))
        self.invocations = 0

    def cli(self):
        self.invocations += 1
        output = self.root / "evidence" / f"paired-{self.invocations}"
        result = subprocess.run(
            [sys.executable, "-B", "scripts/verify.py", "--qualify",
             str(self.directories["portable"]), str(self.directories["native"]),
             "--revision", self.revision, "--output", str(output)],
            cwd=self.root, env={"PATH": "/usr/bin"}, stdin=subprocess.DEVNULL,
            capture_output=True, text=True, timeout=10)
        report = json.loads((output / "report.json").read_text())
        return result, report
