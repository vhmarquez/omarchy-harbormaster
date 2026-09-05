"""Scoped CLI orchestration; mocked execution is NOT native or hosted evidence."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import verify
from verification.checks import plan


class ScopeReporting(unittest.TestCase):
    def test_empty_revision_never_reaches_output_or_execution_boundary(self):
        argv = ["verify.py", "--revision", ""]
        with patch.object(sys, "argv", argv), patch.object(verify, "output_directory") as output:
            with patch.object(verify, "verification", return_value=[]) as execution:
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as raised:
                    verify.main()
                self.assertEqual(raised.exception.code, 2)
                output.assert_not_called()
                execution.assert_not_called()

    def test_native_scope_reaches_the_inventory_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "output"
            output.mkdir()
            tools = Path(tmp) / "tools"
            tools.mkdir()
            options = SimpleNamespace(scope="native", tools=tools, backend="bwrap", image=None)
            with patch.object(verify, "snapshot"), patch.object(verify, "execute_checks", return_value=[]) as execute:
                verify.verification(options, output)
            actual = [n for n, _, _ in execute.call_args.args[0]]
            self.assertEqual(actual, [n for n, _, _ in plan(scope="native")])

    def test_scoped_reports_distinguish_selected_pass_from_completion(self):
        for scope, backend in (("portable", "docker"), ("native", "bwrap"), ("all", "bwrap")):
            with self.subTest(scope=scope), tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "output"
                rows = [{"name": n, "required": True, "status": "PASS"} for n, _, _ in plan(scope=scope)]
                argv = ["verify.py", "--output", str(output), "--backend", backend, "--scope", scope]
                with patch.object(sys, "argv", argv), patch.object(verify, "verification", return_value=rows):
                    with contextlib.redirect_stdout(io.StringIO()):
                        result = verify.main()
                report = json.loads((output / "report.json").read_text())
                self.assertEqual(result, 0)
                self.assertEqual(report["schema"], 2)
                self.assertEqual(report["scope"], scope)
                self.assertFalse(report["qualification_complete"],
                                 "one backend cannot complete paired Docker/native qualification")
                expected = {name: "PASS" if scope in ("all", name) else "NOT_RUN"
                            for name in ("portable", "native")}
                self.assertEqual(report["qualifications"], expected)


    def test_paired_mode_calls_only_the_evidence_boundary_and_fails_closed(self):
        revision = "1" * 40
        portable, native = Path("/synthetic/portable"), Path("/synthetic/native")
        for failure in (False, True):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "output"
                argv = ["verify.py", "--output", str(output), "--qualify", str(portable), str(native),
                        "--revision", revision]
                result = {"passed": True, "qualification_complete": True, "revision": revision,
                          "qualifications": {"portable": "PASS", "native": "PASS"}}
                with patch.object(sys, "argv", argv), patch.object(verify, "verification", return_value=[]) as execution:
                    with patch.object(verify.qualification, "qualify", create=True, return_value=result,
                                      side_effect=ValueError("synthetic missing native evidence") if failure else None) as qualify:
                        with contextlib.redirect_stdout(io.StringIO()):
                            code = verify.main()
                        qualify.assert_called_once_with(verify.ROOT, portable, native, revision)
                execution.assert_not_called()
                report = json.loads((output / "report.json").read_text())
                self.assertEqual(report["scope"], "paired")
                self.assertEqual(code, 1 if failure else 0)
                self.assertEqual(report["passed"], not failure)
                self.assertEqual(report["qualification_complete"], not failure)


if __name__ == "__main__":
    unittest.main()
