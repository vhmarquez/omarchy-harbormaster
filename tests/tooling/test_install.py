"""Archive extraction refuses traversal before writing any member."""
import importlib.util
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))


class ExtractionTests(unittest.TestCase):
    def test_tar_traversal_and_link_escape_leave_no_files(self):
        self.assertIsNotNone(importlib.util.find_spec("tooling.install"), "missing safe extraction")
        from tooling.install import extract
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, kind, link in [("../escape", tarfile.REGTYPE, ""),
                                     ("/escape", tarfile.REGTYPE, ""),
                                     ("safe/link", tarfile.SYMTYPE, "../../escape"),
                                     ("safe/link", tarfile.LNKTYPE, "../escape")]:
                with self.subTest(name=name, kind=kind):
                    archive = root / "input.tar"
                    with tarfile.open(archive, "w") as output:
                        item = tarfile.TarInfo(name)
                        item.type, item.linkname = kind, link
                        output.addfile(item, io.BytesIO())
                    with self.assertRaisesRegex(ValueError, "unsafe archive"):
                        extract(archive, root / "output")
                    self.assertFalse((root / "output").exists())
                    self.assertFalse((root / "escape").exists())

    def test_owned_stage_failure_cleans_only_its_files(self):
        import tooling.install as install
        self.assertTrue(hasattr(install, "owned_stage"), "missing atomic private staging")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / ".tools"
            with self.assertRaisesRegex(RuntimeError, "installer failed"):
                with install.owned_stage(root, "rust") as stage:
                    (stage / "partial").write_text("partial")
                    raise RuntimeError("installer failed")
            self.assertEqual([p.name for p in root.iterdir()], [".harbormaster-tools.json"])
            self.assertEqual(root.stat().st_mode & 0o777, 0o700)
            with install.owned_stage(root, "rust") as stage:
                (stage / "rust").mkdir()
                (stage / "rust" / "binary").write_text("fixture")
            self.assertEqual((root / "rust" / "binary").read_text(), "fixture")
            with self.assertRaisesRegex(ValueError, "already exists"):
                with install.owned_stage(root, "rust"):
                    self.fail("existing install overwritten")

    def test_unowned_or_insecure_root_is_refused(self):
        from tooling.install import owned_stage
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / ".tools"
            root.mkdir()
            for mode in (0o700, 0o777):
                root.chmod(mode)
                with self.assertRaisesRegex(ValueError, "ownership|private"):
                    with owned_stage(root, "rust"):
                        self.fail("unowned directory adopted")
            self.assertEqual(list(root.iterdir()), [])

    def test_subprocess_clears_credentials_and_timeout_kills_child(self):
        import tooling.install as install
        self.assertTrue(hasattr(install, "run"), "missing isolated subprocess")
        import os
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.dict(os.environ, {"SECRET_TOKEN": "never-copy", "PATH": "/invalid"}):
                output = install.run(["/usr/bin/env"], root)
            self.assertNotIn("SECRET_TOKEN", output)
            self.assertIn("PATH=/usr/bin", output)
            with self.assertRaisesRegex(RuntimeError, "timed out"):
                install.run(["/usr/bin/sleep", "5"], root, timeout=0.02)


if __name__ == "__main__":
    unittest.main()
