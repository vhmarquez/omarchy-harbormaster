"""Installed harness contract: offline startup only; never a generation request."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class Installed(unittest.TestCase):
    def test_minimal_installed_integrations(self):
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp) / "report.json"
            r = subprocess.run(["/usr/bin/python3", str(HERE / "run.py"), "--out", str(report)],
                               capture_output=True, text=True, timeout=180)
            self.assertEqual(r.returncode, 0, r.stderr)
            data = json.loads(report.read_text())
            self.assertFalse(str(Path.home()) + "/" in report.read_text(), "evidence must redact host home paths")
            self.assertEqual(set(data["harnesses"]), {"hermes", "claude", "codex"})
            for name, harness in data["harnesses"].items():
                self.assertTrue(harness["version"], name)
                self.assertTrue(harness["restored"], name)
            self.assertTrue(data["harnesses"]["hermes"]["loader_callbacks_verified"])
            self.assertTrue(data["harnesses"]["hermes"]["disabled_plugin_did_not_run"])
            # Blockers must be explicit, not a vacuous 'pass' from zero events.
            claude = data["harnesses"]["claude"]
            self.assertTrue(claude["events"] or claude["blocker"])
            self.assertEqual([e["event"] for e in claude["events"]], ["Setup", "SessionStart", "SessionEnd"])
            self.assertEqual(len({e["session_id"] for e in claude["events"]}), 1)
            codex = data["harnesses"]["codex"]
            self.assertTrue(codex["hooks_listed"])
            self.assertTrue(codex["untrusted_hooks_need_review"])
            self.assertEqual(codex["generation_requests_sent"], 0)
            self.assertEqual(codex["trust_writes_sent"], 0)
            self.assertTrue("native_startup" in codex, "native CLI startup probe missing")
            native = codex["native_startup"]
            self.assertEqual(native["semantic_input_bytes"], 0)
            self.assertTrue(native["screen_bytes_seen"] > 0)
            self.assertTrue("schema_client_methods" in codex, "control inventory missing")
            self.assertIn("turn/interrupt", codex["schema_client_methods"])
            self.assertIn("thread/resume", codex["schema_client_methods"])
            self.assertIn("item/tool/requestUserInput", codex["schema_server_methods"])
            self.assertEqual(data["privacy"]["transcripts_exported"], 0)


if __name__ == "__main__":
    unittest.main()
