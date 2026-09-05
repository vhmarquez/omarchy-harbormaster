"""Opt-in real services/tmux integration; no harness or model invocation."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class RuntimeProbeTests(unittest.TestCase):
    def test_lifecycle_probe_preserves_owned_worker(self):
        script = Path(__file__).with_name("probe.py")
        self.assertTrue(script.exists(), "runtime probe not implemented")
        with tempfile.TemporaryDirectory(prefix="harbormaster-test-") as directory:
            result = Path(directory) / "result.json"
            run = subprocess.run(["python3", "-B", str(script), "--phase", "lifecycle",
                                  "--output", str(result)], capture_output=True, text=True,
                                 timeout=90)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            data = json.loads(result.read_text())
            self.assertEqual(data["verdict"], "VALIDATED")
            required = {"separate_cgroups", "default_tmux_unchanged", "ui_restart_survival",
                        "daemon_restart_survival", "daemon_sigkill_survival",
                        "daemon_recovery_no_duplicate", "runtime_stop_ends_worker"}
            self.assertTrue(required <= data["checks"].keys())
            self.assertTrue(all(data["checks"].values()), data["checks"])
            self.assertTrue(all(data["cleanup"].values()), data["cleanup"])
            if os.environ.get("HARBORMASTER_EVIDENCE"):
                Path(os.environ["HARBORMASTER_EVIDENCE"]).write_text(result.read_text())


    def test_desktop_probe_focuses_exact_disposable_terminal(self):
        script = Path(__file__).with_name("probe.py")
        with tempfile.TemporaryDirectory(prefix="harbormaster-test-") as directory:
            result = Path(directory) / "result.json"
            run = subprocess.run(["python3", "-B", str(script), "--phase", "desktop",
                                  "--allow-desktop", "--output", str(result)],
                                 capture_output=True, text=True, timeout=90)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            data = json.loads(result.read_text())
            required = {"exact_attach", "exact_focus", "attached_across_restarts",
                        "terminal_close_survival", "exact_reattach", "stale_window_rejected",
                        "wrong_identity_rejected", "missing_session_rejected"}
            self.assertTrue(required <= data["checks"].keys())
            self.assertTrue(all(data["checks"].values()), data["checks"])
            self.assertTrue(all(data["cleanup"].values()), data["cleanup"])
            self.assertTrue(data["cleanup"]["original_focus_restored"])
            self.assertTrue(data["cleanup"]["owned_windows_gone"])
            if os.environ.get("HARBORMASTER_DESKTOP_EVIDENCE"):
                Path(os.environ["HARBORMASTER_DESKTOP_EVIDENCE"]).write_text(result.read_text())


if __name__ == "__main__":
    unittest.main()
