"""Verification entrypoint contract: every required family is exercised."""
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class PlanTests(unittest.TestCase):
    def test_plan_contains_explicit_required_tool_and_test_gates(self):
        from verification import checks
        self.assertTrue(hasattr(checks, "plan"), "verification plan missing")
        result = checks.plan()
        names = [item[0] for item in result]
        self.assertEqual(len(names), len(set(names)))
        for required in ("tool-pins", "rust-format", "rust-clippy", "rust-unit",
                         "rust-integration", "qml-lint", "qml-format", "qml-native",
                         "advisory-license", "maintainability", "python-verification",
                         "m0-harness-sandbox"):
            self.assertIn(required, names)
        for name, argv, kind in result:
            self.assertTrue(argv, name)
            self.assertIn(kind, ("exit", "python", "native", "rust", "qml", "node", "metrics"))


if __name__ == "__main__":
    unittest.main()
