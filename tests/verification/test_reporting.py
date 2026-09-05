"""Fail-closed check reporting, with no synthetic success from empty suites."""
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class ReportingTests(unittest.TestCase):
    def test_required_skips_and_missing_results_cannot_pass(self):
        self.assertTrue((SCRIPTS / "verification/checks.py").is_file(),
                        "required-check reporting missing")
        from verification.checks import passed
        self.assertFalse(passed([]))
        self.assertFalse(passed([{"required": True, "status": "SKIP"}]))
        self.assertFalse(passed([{"required": True, "status": "FAIL"}]))
        self.assertTrue(passed([{"required": True, "status": "PASS"},
                                {"required": False, "status": "SKIP"}]))

    def test_empty_or_skipped_suite_fails_even_when_exit_is_zero(self):
        self.assertTrue((SCRIPTS / "verification/checks.py").is_file(),
                        "suite validation missing")
        from verification.checks import validate
        for output in ("", "Ran 0 tests\nOK", "Ran 1 test\nOK (skipped=1)"):
            self.assertFalse(validate("python", output))
        self.assertTrue(validate("python", "Ran 3 tests\nOK"))
        self.assertFalse(validate("rust", "test result: ok. 0 passed; 0 failed; 0 ignored;"))
        self.assertFalse(validate("rust", "test result: ok. 2 passed; 0 failed; 1 ignored;"))
        self.assertTrue(validate("rust", "test result: ok. 2 passed; 0 failed; 0 ignored;"))


    def test_node_skips_and_empty_suites_fail(self):
        from verification.checks import validate
        self.assertTrue(validate("node", "# tests 2\n# fail 0\n# cancelled 0\n# skipped 0\n"))
        self.assertFalse(validate("node", "# tests 0\n# fail 0\n# cancelled 0\n# skipped 0\n"))
        self.assertFalse(validate("node", "# tests 2\n# fail 0\n# cancelled 0\n# skipped 1\n"))


    def test_incomplete_metrics_report_fails_but_size_hotspots_do_not(self):
        from verification.checks import validate
        self.assertFalse(validate("metrics", "{}"))
        self.assertFalse(validate("metrics", '{"coverage":{"complete":false,"files_analyzed":1}}'))
        self.assertTrue(validate("metrics", '{"coverage":{"complete":true,"files_analyzed":1},"production":{"hotspots":["review"]}}'))


if __name__ == "__main__":
    unittest.main()
