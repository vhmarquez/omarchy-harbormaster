"""QML gate must run native test functions, not just lifecycle hooks."""
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))
from verification.checks import validate

# Captured from real Qt 6.11.2 running two empty TestCases offscreen.
EMPTY_SUITES = """********* Start testing of qmltestrunner *********
Config: Using QtTest library 6.11.2, Qt 6.11.2 (x86_64-little_endian-lp64 shared (dynamic) release build; by GCC 16.2.1 20260810), arch unknown
PASS   : qmltestrunner::EmptyTwo::initTestCase()
PASS   : qmltestrunner::EmptyTwo::cleanupTestCase()
PASS   : qmltestrunner::EmptyOne::initTestCase()
PASS   : qmltestrunner::EmptyOne::cleanupTestCase()
Totals: 4 passed, 0 failed, 0 skipped, 0 blacklisted, 2ms
********* Finished testing of qmltestrunner *********
"""
TEST_PASS = "PASS   : qmltestrunner::Native::test_toolchain()\n"


class QmlReportTests(unittest.TestCase):
    def test_multiple_empty_testcases_do_not_count_as_test_execution(self):
        self.assertFalse(validate("qml", EMPTY_SUITES))

    def test_aggregate_totals_without_executed_function_cannot_pass(self):
        self.assertFalse(validate("qml", "Totals: 3 passed, 0 failed, 0 skipped, 0 blacklisted"))

    def test_empty_failed_skipped_and_blacklisted_qml_suites_fail(self):
        self.assertFalse(validate("qml", ""))
        for totals in ("3 passed, 0 failed, 1 skipped, 0 blacklisted",
                       "2 passed, 1 failed, 0 skipped, 0 blacklisted",
                       "3 passed, 0 failed, 0 skipped, 1 blacklisted",
                       "0 passed, 0 failed, 0 skipped, 0 blacklisted"):
            with self.subTest(totals=totals):
                self.assertFalse(validate("qml", TEST_PASS + "Totals: " + totals))
        self.assertFalse(validate("qml", TEST_PASS))

    def test_executed_test_function_including_data_rows_passes(self):
        for function in ("test_toolchain()", "test_toolchain(row one)"):
            with self.subTest(function=function):
                output = TEST_PASS.replace("test_toolchain()", function)
                output += "Totals: 3 passed, 0 failed, 0 skipped, 0 blacklisted"
                self.assertTrue(validate("qml", output))

    def test_lifecycle_names_or_logged_pass_text_cannot_supply_execution(self):
        for line in ("PASS   : test_NamedSuite::initTestCase()",
                     "QDEBUG : Native::initTestCase() PASS   : Native::test_fake()"):
            with self.subTest(line=line):
                output = line + "\nTotals: 4 passed, 0 failed, 0 skipped, 0 blacklisted"
                self.assertFalse(validate("qml", output))


if __name__ == "__main__":
    unittest.main()
