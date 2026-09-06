"""Verification CLI must fail honestly when preparation is absent."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import json

ROOT = Path(__file__).resolve().parents[2]


class VerifierCliTests(unittest.TestCase):
    def test_missing_tools_is_failed_report_not_success_or_optional_skip(self):
        self.assertTrue((ROOT / "scripts/verify.py").is_file(), "entry point missing")
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            result = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/verify.py"),
                                     "--tools", str(base / "absent"), "--output", str(base / "report")],
                                    capture_output=True, text=True, timeout=10, check=False)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            report = json.loads((base / "report/report.json").read_text())
        self.assertFalse(report["passed"])
        self.assertEqual(report["checks"][0]["name"], "preparation")
        self.assertEqual(report["checks"][0]["status"], "FAIL")
        self.assertTrue(report["checks"][0]["required"])


if __name__ == "__main__":
    unittest.main()
