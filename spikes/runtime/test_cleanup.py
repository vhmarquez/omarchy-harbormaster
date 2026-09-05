"""Actual-finalizer fault tests; no services, desktop or default tmux access.

The AST selects the real finalizer and exit status without running probe setup.
Only external command/process/window boundaries are injected: forcing a real
stop timeout could strand services or disturb the desktop. Scratch/output I/O
is real and confined to a disposable directory.
"""
import ast
from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


HERE = Path(__file__).resolve().parent


class CleanupTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("cleanup_probe", HERE / "probe.py")
        assert spec is not None and spec.loader is not None
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        tree = ast.parse((HERE / "probe.py").read_text())
        probe = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                     and node.name == "probe")
        finalizer = next(node for node in probe.body if isinstance(node, ast.Try))
        wrapper = ast.parse("def finalize(): pass")
        assert isinstance(wrapper.body[0], ast.FunctionDef)
        wrapper.body[0].body = finalizer.finalbody + [probe.body[-1]]
        exec(compile(ast.fix_missing_locations(wrapper), str(HERE / "probe.py"), "exec"),
             self.module.__dict__)
        # Honor the verifier's private TMPDIR; the checkout/parents can be read-only.
        temporary = tempfile.TemporaryDirectory(prefix="hb-cleanup-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.directory = self.root / "owned-scratch"
        self.directory.mkdir()
        self.output = self.root / "evidence" / "result.json"
        self.units = [f"harbormaster-m0-{role}-0123456789ab.service"
                      for role in ("ui", "daemon", "runtime")]
        self.processes = [{"pid": 101}, {"pid": 102}]
        self.data = {"completed": True, "checks": {"fixture": True}, "cleanup": {},
                     "verdict": "INVALIDATED"}
        self.module.__dict__.update(
            args=SimpleNamespace(phase="lifecycle", output=self.output), data=self.data,
            units=self.units, processes=self.processes, terminals=[], original=None,
            app_ids=[], directory=self.directory, baseline={}, ROOT=self.directory)
        self.command_calls = []
        self.faults = {}
        self.returncodes = {}
        self.stdout = io.StringIO()
        self.addCleanup(patch.stopall)
        patch.object(self.module.subprocess, "run", side_effect=self.command).start()
        self.process_readback = patch.object(self.module, "same_process", return_value=False).start()
        self.metadata_readback = patch.object(self.module, "default_metadata", return_value={}).start()

    def command(self, argv, **kwargs):
        self.command_calls.append(argv)
        key = (argv[2], argv[3])
        if key in self.faults:
            raise self.faults[key]
        return subprocess.CompletedProcess(argv, self.returncodes.get(key, 0),
                                           stdout="LoadState=not-found\n", stderr="")

    def finalize(self):
        try:
            with redirect_stdout(self.stdout):
                return self.module.finalize()
        except Exception as exc:
            self.fail(f"cleanup escaped instead of exporting failed evidence: {type(exc).__name__}: {exc}")

    def assert_remaining_cleanup(self):
        expected = [["systemctl", "--user", action, unit]
                    for unit in self.units for action in ("stop", "reset-failed")]
        self.assertEqual([argv for argv in self.command_calls if argv[2] != "show"], expected)
        self.assertEqual([argv[3] for argv in self.command_calls if argv[2] == "show"], self.units)
        self.assertEqual([call.args[0] for call in self.process_readback.call_args_list], self.processes)
        self.metadata_readback.assert_called_once_with()
        self.assertFalse(self.directory.exists())
        self.assertTrue(self.output.is_file())

    def test_first_stop_timeout_does_not_skip_remaining_cleanup_or_export(self):
        argv = ["systemctl", "--user", "stop", self.units[0]]
        self.faults[("stop", self.units[0])] = subprocess.TimeoutExpired(argv, 15)
        self.assertEqual(self.finalize(), 1)
        self.assert_remaining_cleanup()
        result = json.loads(self.output.read_text())
        self.assertEqual(result["verdict"], "INVALIDATED")
        error = result["cleanup_errors"][0]
        self.assertEqual(error["stage"], "stop")
        self.assertEqual(error["target"], self.units[0])
        self.assertEqual(error["type"], "TimeoutExpired")
        self.assertIn("15", error["message"])
        self.assertEqual(json.loads(self.stdout.getvalue())["verdict"], "INVALIDATED")

    def test_multiple_cleanup_errors_are_reported_without_skipping_later_attempts(self):
        children = [SimpleNamespace(pid=201), SimpleNamespace(pid=202)]
        self.module.__dict__.update(terminals=children, original={"address": "0xabc", "pid": 301},
                                    app_ids=["org.harbormaster.probe.fixture"])
        self.module.args.phase = "desktop"
        close = patch.object(self.module, "close_terminal",
                             side_effect=[OSError("child close failed"), None]).start()
        patch.object(self.module, "windows", side_effect=OSError("window readback failed")).start()
        focus = patch.object(self.module, "focus", return_value=True).start()
        self.faults[("reset-failed", self.units[0])] = OSError("reset failed")
        self.faults[("show", self.units[0])] = subprocess.TimeoutExpired(["systemctl", "show"], 15)
        self.process_readback.side_effect = [OSError("process readback failed"), False]
        (self.directory / "daemon-commands.jsonl").write_text("invalid json\n")
        remove = patch.object(self.module.shutil, "rmtree", side_effect=OSError("remove failed")).start()
        self.metadata_readback.side_effect = OSError("metadata failed")
        read_bytes = Path.read_bytes
        hashed = []

        def source_bytes(path):
            hashed.append(path.name)
            if path.name == "probe.py":
                raise OSError("source read failed")
            return read_bytes(path)

        patch.object(Path, "read_bytes", source_bytes).start()
        self.data["error"] = {"stage": "fixture failure", "type": "RuntimeError", "message": "original error"}
        self.assertEqual(self.finalize(), 1)
        self.assertEqual([call.args[0] for call in close.call_args_list], children)
        focus.assert_called_once_with(self.module.original)
        self.assertEqual([argv[3] for argv in self.command_calls if argv[2] == "stop"], self.units)
        self.assertEqual([argv[3] for argv in self.command_calls if argv[2] == "reset-failed"], self.units)
        self.assertEqual([argv[3] for argv in self.command_calls if argv[2] == "show"], self.units)
        self.assertEqual([call.args[0] for call in self.process_readback.call_args_list], self.processes)
        remove.assert_called_once_with(self.directory)
        self.metadata_readback.assert_called_once_with()
        self.assertIn("test_runtime.py", hashed)
        result = json.loads(self.output.read_text())
        self.assertEqual(result["error"], self.data["error"])
        self.assertEqual(result["verdict"], "INVALIDATED")
        errors = result["cleanup_errors"]
        self.assertEqual([error["stage"] for error in errors], [
            "close_terminal", "owned_windows_gone", "reset-failed", "unit_readback",
            "process_readback", "daemon_command_batches", "remove_directory",
            "default_tmux_unchanged", "source_sha256"])
        self.assertTrue(all(error["type"] and error["message"] for error in errors))
        for key in ("units_unloaded", "owned_processes_gone", "owned_windows_gone",
                    "private_directory_removed", "default_tmux_unchanged"):
            self.assertIs(result["cleanup"][key], False, key)
        self.assertIs(result["cleanup"]["original_focus_restored"], True)
        self.assertIsNone(result["source_sha256"]["probe.py"])

    def test_output_failure_returns_failed_structured_sanitized_summary(self):
        self.output.mkdir(parents=True)  # Real write failure, no permission assumptions.
        self.assertEqual(self.finalize(), 1)
        self.assertFalse(self.directory.exists())
        summary = json.loads(self.stdout.getvalue())
        self.assertEqual(summary["verdict"], "INVALIDATED")
        error = summary["cleanup_errors"][0]
        self.assertEqual(error["stage"], "output_export")
        self.assertEqual(error["type"], "IsADirectoryError")
        self.assertTrue(error["target"].endswith("/evidence/result.json"))
        self.assertNotIn(str(Path.home()), self.stdout.getvalue())

    def test_clean_finalizer_still_validates_and_fingerprints_cleanup_tests(self):
        # Already collected units can make stop/reset-failed nonzero; their
        # final LoadState, not those expected return codes, proves cleanup.
        self.returncodes[("stop", self.units[-1])] = 5
        for unit in self.units:
            self.returncodes[("reset-failed", unit)] = 1
        self.assertEqual(self.finalize(), 0)
        self.assert_remaining_cleanup()
        result = json.loads(self.output.read_text())
        self.assertEqual(result["verdict"], "VALIDATED")
        self.assertEqual(result["cleanup_errors"], [])
        self.assertTrue(all(result["cleanup"].values()))
        self.assertEqual(set(result["source_sha256"]),
                         {"probe.py", "identity.py", "test_identity.py", "test_runtime.py", "test_cleanup.py"})

    def test_false_readbacks_do_not_short_circuit_later_owned_targets(self):
        readback = patch.object(self.module, "unit_info", side_effect=[
            {"LoadState": "loaded"}, {"LoadState": "not-found"}, {"LoadState": "not-found"}]).start()
        self.process_readback.side_effect = [True, False]
        self.assertEqual(self.finalize(), 1)
        self.assertEqual([call.args[0] for call in readback.call_args_list], self.units)
        self.assertEqual([call.args[0] for call in self.process_readback.call_args_list], self.processes)
        result = json.loads(self.output.read_text())
        self.assertEqual(result["verdict"], "INVALIDATED")
        self.assertIs(result["cleanup"]["units_unloaded"], False)
        self.assertIs(result["cleanup"]["owned_processes_gone"], False)
        self.assertEqual(result["cleanup_errors"], [])
        self.assertFalse(self.directory.exists())

    def test_early_probe_failure_stays_failed_without_owned_resources(self):
        self.module.__dict__.update(units=[], processes=[], directory=None, ROOT=None)
        self.data["completed"] = False
        self.data["error"] = {"stage": "prerequisites", "type": "RuntimeError", "message": "fixture failure"}
        self.assertEqual(self.finalize(), 1)
        self.assertEqual(self.command_calls, [])
        self.process_readback.assert_not_called()
        result = json.loads(self.output.read_text())
        self.assertEqual(result["error"], self.data["error"])
        self.assertEqual(result["verdict"], "INVALIDATED")


if __name__ == "__main__":
    unittest.main()
