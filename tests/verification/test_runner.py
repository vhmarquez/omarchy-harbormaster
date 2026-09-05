"""Orchestrator tests use deterministic subprocess-boundary doubles."""
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class OrchestrationTests(unittest.TestCase):
    def test_failure_does_not_hide_remaining_required_checks(self):
        self.assertTrue((SCRIPTS / "verification/runner.py").is_file(),
                        "verification orchestration missing")
        from verification.runner import execute_checks
        calls = []

        def executor(argv):
            calls.append(argv)
            return {"exit_code": 1 if argv == ["fail"] else 0, "output": "fixture"}

        with tempfile.TemporaryDirectory() as tmp:
            checks = [("first", ["fail"], "exit"), ("second", ["pass"], "exit")]
            results = execute_checks(checks, executor, Path(tmp))
        self.assertEqual(len(calls), 2)
        self.assertEqual([result["status"] for result in results], ["FAIL", "PASS"])
        self.assertTrue(all(result["required"] for result in results))

    def test_missing_executable_becomes_failed_check(self):
        self.assertTrue((SCRIPTS / "verification/runner.py").is_file(),
                        "verification orchestration missing")
        from verification.runner import execute_checks

        def executor(_argv):
            raise FileNotFoundError("synthetic missing executable")

        with tempfile.TemporaryDirectory() as tmp:
            results = execute_checks([("missing", ["missing"], "exit")], executor, Path(tmp))
        self.assertEqual(results[0]["status"], "FAIL")
        self.assertEqual(results[0]["error"], "FileNotFoundError")


    def test_failed_trust_preflight_blocks_execution_but_reports_inventory(self):
        from verification.runner import execute_checks
        calls = []

        def executor(argv):
            calls.append(argv)
            return {"exit_code": 1, "output": "synthetic integrity failure"}

        checks = [("tool-pins", ["pins"], "exit"), ("rust-unit", ["tests"], "exit")]
        with tempfile.TemporaryDirectory() as tmp:
            results = execute_checks(checks, executor, Path(tmp))
        self.assertEqual(calls, [["pins"]])
        self.assertEqual(results[1]["status"], "NOT_RUN")
        self.assertTrue(results[1]["required"])


if __name__ == "__main__":
    unittest.main()
