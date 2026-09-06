"""Tests for verification isolation; use synthetic secrets only."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class EnvironmentTests(unittest.TestCase):
    def test_only_allowlisted_environment_reaches_children(self):
        self.assertTrue((SCRIPTS / "verification/sandbox.py").is_file(),
                        "isolated process environment is not implemented")
        from verification.sandbox import environment
        with patch.dict(os.environ, {"HB_SYNTHETIC_SECRET": "canary-not-real",
                                     "DISPLAY": ":99", "RUSTC_WRAPPER": "/untrusted"}):
            actual = environment()
        self.assertNotIn("HB_SYNTHETIC_SECRET", actual)
        self.assertNotIn("DISPLAY", actual)
        self.assertNotIn("RUSTC_WRAPPER", actual)
        self.assertEqual(actual["HOME"], "/state/home")
        self.assertEqual(actual["CARGO_NET_OFFLINE"], "true")


class SnapshotTests(unittest.TestCase):
    def test_snapshot_copies_only_allowed_sources_without_private_files(self):
        from verification import sandbox
        self.assertTrue(hasattr(sandbox, "snapshot"), "snapshot boundary missing")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "root"
            root.mkdir()
            (root / "Cargo.toml").write_text("[workspace]\n")
            (root / ".env").write_text("SYNTHETIC_SECRET=canary")
            (root / "docs").mkdir()
            (root / "docs/note.md").write_text("public fixture")
            (root / "docs/.env.local").write_text("SYNTHETIC_SECRET=canary")
            copied = Path(tmp) / "copy"
            sandbox.snapshot(root, copied)
            self.assertTrue((copied / "Cargo.toml").exists())
            self.assertTrue((copied / "docs/note.md").exists())
            self.assertFalse((copied / ".env").exists())
            self.assertFalse((copied / "docs/.env.local").exists())

    def test_snapshot_rejects_symlinks_in_allowed_sources(self):
        from verification import sandbox
        self.assertTrue(hasattr(sandbox, "snapshot"), "snapshot boundary missing")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "root"
            root.mkdir()
            (root / "docs").symlink_to(Path(tmp), target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                sandbox.snapshot(root, Path(tmp) / "copy")


class BoundaryTests(unittest.TestCase):
    def test_fault_filesystem_is_separately_private_and_physically_bounded(self):
        from verification import sandbox
        argv = sandbox._bubblewrap(Path("/source"), Path("/tools"), Path("/state"))
        position = argv.index("/fault-fs")
        self.assertEqual(argv[position - 5:position + 1],
                         ["--perms", "0700", "--size", "1048576", "--tmpfs", "/fault-fs"])

    # Real isolation now runs as the common required isolation-probe check.
    # Unit tests exercise process bounds without needing a nested user namespace.
    def test_bubblewrap_policy_unshares_network_and_excludes_host_homes(self):
        from verification import sandbox
        argv = sandbox._bubblewrap(Path("/source"), Path("/tools"), Path("/state"))
        self.assertIn("--unshare-all", argv)
        self.assertIn("--clearenv", argv)
        self.assertIn("--die-with-parent", argv)
        self.assertNotIn("/home", argv)
        self.assertNotIn("/run/user", argv)
        self.assertNotIn("/", [argv[index + 1] for index, value in enumerate(argv)
                                if value in ("--bind", "--ro-bind")])

    def test_timeout_is_failure_not_an_exception_or_leaked_child(self):
        from verification import sandbox
        result = sandbox._execute(["/usr/bin/python3", "-c", "import time; time.sleep(0.3)"], 0.03)
        self.assertTrue(result["timed_out"])
        self.assertNotEqual(result["exit_code"], 0)

    def test_excessive_output_is_bounded_and_fails(self):
        from verification import sandbox
        result = sandbox._execute(["/usr/bin/python3", "-c",
                                   "import sys; sys.stdout.write('x'*(6*1024*1024))"], 10)
        self.assertTrue(result["output_limited"])
        self.assertNotEqual(result["exit_code"], 0)
        self.assertLessEqual(len(result["output"]), 4 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
