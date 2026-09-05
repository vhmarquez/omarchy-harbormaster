"""The common isolation probe also runs in CI without nested namespaces."""
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class ProbeTests(unittest.TestCase):
    def test_visible_outside_canary_rejects_boundary(self):
        self.assertTrue((SCRIPTS / "verification/probe.py").is_file(), "isolation probe missing")
        from verification.probe import inspect
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "synthetic-canary"
            path.write_text("not-a-real-secret")
            with self.assertRaises(ValueError):
                inspect(path)

    def test_probe_is_required_in_common_plan(self):
        from verification.checks import plan
        self.assertIn("isolation-probe", [item[0] for item in plan()])


if __name__ == "__main__":
    unittest.main()
