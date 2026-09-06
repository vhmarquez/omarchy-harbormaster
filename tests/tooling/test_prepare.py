"""CLI consent and pinned snapshot freshness tests."""
from datetime import datetime, timezone, timedelta
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


class PreparationTests(unittest.TestCase):
    def test_readonly_check_rejects_missing_tools_without_creating_them(self):
        import tooling.prepare as prepare
        self.assertTrue(hasattr(prepare, "verify_existing"), "missing read-only inspection API")
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing"
            with self.assertRaisesRegex(ValueError, "missing"):
                prepare.verify_existing(missing, {})
            self.assertFalse(missing.exists())

    def test_node_is_pinned_for_preserved_design_regressions(self):
        import json
        lock = json.loads((ROOT / "tools" / "toolchain.lock.json").read_text())
        self.assertIn("node", lock, "missing private standalone Node pin")
        self.assertEqual(lock["node"]["version"], "26.8.1")
        self.assertTrue(lock["node"]["url"].startswith("https://nodejs.org/dist/v26.8.1/"))
        self.assertRegex(lock["node"]["sha256"], r"^[0-9a-f]{64}$")

    def test_lock_requires_weekly_snapshot_refresh(self):
        import json
        lock = json.loads((ROOT / "tools" / "toolchain.lock.json").read_text())
        self.assertEqual(lock["rustsec"]["maximum_age_days"], 7)

    def test_network_requires_explicit_online_and_root(self):
        script = ROOT / "scripts" / "prepare-tools.py"
        self.assertTrue(script.exists(), "missing explicit-online preparation CLI")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / ".tools"
            result = subprocess.run([sys.executable, str(script), "--tools-root", str(root)],
                                    capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--online", result.stderr)
            self.assertFalse(root.exists())

    def test_component_versions_are_not_all_rust_release_numbers(self):
        import tooling.prepare as prepare
        self.assertTrue(hasattr(prepare, "check_rust_version"), "missing component-specific version check")
        prepare.check_rust_version("clippy-driver", "clippy 0.1.98 (48a229ceae 2026-09-01)",
                                   {"binary_versions": {"clippy-driver": "0.1.98"}})
        with self.assertRaisesRegex(ValueError, "unexpected"):
            prepare.check_rust_version("rustc", "rustc 1.97.0 (old)",
                                       {"binary_versions": {"rustc": "1.98.1"}})

    def test_stale_and_future_snapshot_dates_fail_closed(self):
        import tooling.prepare as prepare
        self.assertTrue(hasattr(prepare, "validate_snapshot_age"), "missing immutable-date freshness gate")
        now = datetime(2026, 9, 5, tzinfo=timezone.utc)
        for days in (-1, 7, 8):
            pin = {"commit_date": (now - timedelta(days=days)).isoformat(), "maximum_age_days": 7}
            with self.subTest(days=days), self.assertRaisesRegex(ValueError, "snapshot"):
                prepare.validate_snapshot_age(pin, now)
        prepare.validate_snapshot_age({"commit_date": "2026-09-02T09:13:32Z", "maximum_age_days": 7}, now)


if __name__ == "__main__":
    unittest.main()
