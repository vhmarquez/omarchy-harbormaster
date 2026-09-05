"""M0 helper tests. All filesystem fixtures are disposable, never profiles."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class Helpers(unittest.TestCase):
    def test_sandbox_denies_network_and_inherited_credentials(self):
        spec = importlib.util.spec_from_file_location("probe", HERE / "probe.py")
        self.assertTrue((HERE / "probe.py").exists(), "sandbox helper not implemented")
        assert spec is not None and spec.loader is not None
        probe = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probe)
        with tempfile.TemporaryDirectory() as tmp:
            cmd = probe.sandbox(Path(tmp), {})
            code = 'import os,socket,json; s=socket.socket(); s.settimeout(.1); print(json.dumps({"env":dict(os.environ),"network":s.connect_ex(("1.1.1.1",443)),"real_home_visible":os.path.exists("/home/vhm/.claude")}))'
            r = subprocess.run(cmd + ["/usr/bin/python3", "-c", code], capture_output=True, text=True,
                               env={"PATH": "/usr/bin:/bin", "ANTHROPIC_API_KEY": "synthetic-secret"}, timeout=5)
            self.assertEqual(r.returncode, 0, r.stderr)
            result = json.loads(r.stdout)
            self.assertNotIn("ANTHROPIC_API_KEY", result["env"])
            self.assertNotEqual(result["network"], 0)
            self.assertFalse(result["real_home_visible"])
            self.assertEqual(result["env"]["HERMES_HOME"], "/probe/home/.hermes")


class Install(unittest.TestCase):
    def test_overlay_refuses_symlink_and_concurrent_edits(self):
        import probe
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "settings.json"
            target.write_text('{}')
            link = Path(tmp) / "link.json"
            link.symlink_to(target)
            with self.assertRaises(ValueError):
                with probe.hook_overlay(link, {}):
                    pass
            with self.assertRaises(RuntimeError):
                with probe.hook_overlay(target, {}):
                    target.write_text('{"unrelated_new_setting":true}')
            self.assertEqual(json.loads(target.read_text()), {"unrelated_new_setting": True})

    def test_reversible_overlay_preserves_unrelated_hooks_and_exact_original_bytes(self):
        import probe
        self.assertTrue(hasattr(probe, "hook_overlay"), "reversible installer not implemented")
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "settings.json"
            original = b'{"permissions":{"defaultMode":"default"}, "hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"true"}]}]}}\n'
            path.write_bytes(original)
            own = {"SessionStart": [{"hooks": [{"type": "command", "command": "observer"}]}]}
            with probe.hook_overlay(path, own):
                installed = json.loads(path.read_text())
                self.assertEqual(installed["permissions"], {"defaultMode": "default"})
                self.assertEqual([x["hooks"][0]["command"] for x in installed["hooks"]["SessionStart"]], ["true", "observer"])
            self.assertEqual(path.read_bytes(), original)
            path.unlink()
            with probe.hook_overlay(path, own):
                self.assertTrue(path.exists())
            self.assertFalse(path.exists())


class EventProjection(unittest.TestCase):
    def test_invalid_oversized_and_unwritable_payloads_fail_open_silently(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "events.jsonl"
            cases = ["not json", "[1]", "{" * 20000,
                     json.dumps({"hook_event_name": "SessionStart", "prompt": "x" * 20000}),
                     json.dumps({"hook_event_name": "unknown"}),
                     json.dumps({"hook_event_name": ["SessionStart"]})]
            for payload in cases:
                r = subprocess.run(["/usr/bin/python3", str(HERE / "observe.py"), "claude", str(output)],
                                   input=payload, text=True, capture_output=True, timeout=2)
                self.assertEqual((r.returncode, r.stdout, r.stderr), (0, "", ""))
            self.assertFalse(output.exists())
            sentinel = Path(tmp) / "sentinel"
            sentinel.write_text("unrelated")
            output.symlink_to(sentinel)
            r = subprocess.run(["/usr/bin/python3", str(HERE / "observe.py"), "claude", str(output)],
                               input='{"hook_event_name":"Stop"}', text=True, capture_output=True, timeout=2)
            self.assertEqual((r.returncode, r.stdout, r.stderr), (0, "", ""))
            self.assertEqual(sentinel.read_text(), "unrelated")

    def test_command_hook_emits_only_allowlisted_metadata_without_stdout(self):
        script = HERE / "observe.py"
        self.assertTrue(script.exists(), "observer not implemented")
        payload = {"hook_event_name": "SessionStart", "session_id": "fixture-session",
                   "source": "startup", "prompt": "SYNTHETIC_SECRET", "transcript_path": "/must/not/read",
                   "tool_input": {"command": "SYNTHETIC_SECRET"}, "last_assistant_message": "SYNTHETIC_SECRET"}
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "events.jsonl"
            r = subprocess.run(["/usr/bin/python3", str(script), "claude", str(output)],
                               input=json.dumps(payload), text=True, capture_output=True, timeout=2)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(r.stdout, "")
            self.assertEqual(r.stderr, "")
            self.assertEqual(json.loads(output.read_text()), {"harness": "claude", "event": "SessionStart",
                             "session_id": "fixture-session", "source": "startup"})
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)


if __name__ == "__main__":
    unittest.main()
