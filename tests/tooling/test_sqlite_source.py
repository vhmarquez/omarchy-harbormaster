"""SQLite's prepared source boundary rejects substitution before C executes."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from tooling.install import private_root
from tooling.sqlite_source import verify_sqlite


class SqliteInputs(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = private_root(self.root / "tools")
        self.project = self.root / "project"
        (self.project / "tools").mkdir(parents=True)
        self.group = self.tools / "sqlite-source"
        self.group.mkdir()
        self.archive = self.group / "sqlite-amalgamation-3530400.zip"
        self.archive.write_bytes(b"fixture source archive")
        self.pins = {"schema": 1, "version": "3.53.4", "version_number": 3053004,
                     "source_id": "fixture source", "url": "https://sqlite.org/2026/" + self.archive.name,
                     "archive_sha256": hashlib.sha256(self.archive.read_bytes()).hexdigest(),
                     "archive_sha3_256": hashlib.sha3_256(self.archive.read_bytes()).hexdigest(), "files": []}
        for name in ("sqlite3.c", "sqlite3.h"):
            data = ("fixture source " + name).encode()
            (self.group / name).write_bytes(data)
            self.pins["files"].append({"path": "sqlite-amalgamation-3530400/" + name,
                "sha256": hashlib.sha256(data).hexdigest(), "sha3_256": hashlib.sha3_256(data).hexdigest()})
        self.save_pins()

    def save_pins(self):
        (self.project / "tools/sqlite.lock.json").write_text(json.dumps(self.pins))

    def verify(self):
        return verify_sqlite(self.project, self.tools)

    def test_complete_inputs_are_read_only(self):
        before = {p: p.stat().st_mtime_ns for p in self.tools.rglob("*")}
        self.assertEqual(self.verify()["version"], "3.53.4")
        self.assertEqual(before, {p: p.stat().st_mtime_ns for p in self.tools.rglob("*")})

    def test_missing_corrupt_and_symlink_inputs_fail(self):
        for path in sorted(self.group.iterdir()):
            original = path.read_bytes()
            for mutation in ("missing", "corrupt", "symlink"):
                with self.subTest(path=path.name, mutation=mutation):
                    path.unlink()
                    if mutation == "corrupt":
                        path.write_bytes(original + b"corruption")
                    elif mutation == "symlink":
                        target = self.root / "substitute"
                        target.write_bytes(original)
                        path.symlink_to(target)
                    try:
                        with self.assertRaises((ValueError, OSError)):
                            self.verify()
                    finally:
                        if path.exists() or path.is_symlink():
                            path.unlink()
                        path.write_bytes(original)

    def test_unexpected_file_and_directory_fail(self):
        extra = self.group / "unexpected"
        for directory in (False, True):
            with self.subTest(directory=directory):
                if directory:
                    extra.mkdir()
                else:
                    extra.write_text("unreviewed source")
                try:
                    with self.assertRaisesRegex(ValueError, "unexpected"):
                        self.verify()
                finally:
                    extra.rmdir() if directory else extra.unlink()

    def test_wrong_published_source_digest_fails(self):
        self.pins["files"][0]["sha3_256"] = "f" * 64
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "SHA3"):
            self.verify()


if __name__ == "__main__":
    unittest.main()
