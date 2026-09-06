"""Public SQLite preparation is source-only, owned and atomic on failure."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from tooling.install import private_root
from tooling.sqlite_prepare import prepare_sqlite


class SqlitePreparation(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = private_root(self.root / "tools")
        self.project = self.root / "project"
        (self.project / "tools").mkdir(parents=True)
        self.pins = {"schema": 1, "version": "3.53.4", "source_id": "fixture", "files": [],
                     "url": "https://sqlite.org/2026/sqlite-amalgamation-3530400.zip"}
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w") as archive:
            for name in ("sqlite3.c", "sqlite3.h"):
                data = ("fixture " + name).encode()
                member = "sqlite-amalgamation-3530400/" + name
                archive.writestr(member, data)
                self.pins["files"].append({"path": member, "sha256": hashlib.sha256(data).hexdigest(),
                                           "sha3_256": hashlib.sha3_256(data).hexdigest()})
        self.archive = stream.getvalue()
        self.pins.update(archive_sha256=hashlib.sha256(self.archive).hexdigest(),
                         archive_sha3_256=hashlib.sha3_256(self.archive).hexdigest())
        (self.project / "tools/sqlite.lock.json").write_text(json.dumps(self.pins))

    def download(self, url, checksum, destination, **kwargs):
        self.assertEqual(checksum, hashlib.sha256(self.archive).hexdigest())
        destination.write_bytes(self.archive)

    def test_preparation_installs_only_source_and_archive(self):
        with patch("tooling.sqlite_prepare.download", side_effect=self.download):
            self.assertEqual(prepare_sqlite(self.project, self.tools)["version"], "3.53.4")
        self.assertEqual({p.name for p in (self.tools / "sqlite-source").iterdir()},
                         {"sqlite3.c", "sqlite3.h", "sqlite-amalgamation-3530400.zip"})

    def test_existing_group_is_not_overwritten(self):
        (self.tools / "sqlite-source").mkdir()
        with patch("tooling.sqlite_prepare.download") as download:
            with self.assertRaisesRegex(ValueError, "already exists"):
                prepare_sqlite(self.project, self.tools)
            download.assert_not_called()

    def test_download_failure_preserves_other_groups(self):
        (self.tools / "rust").mkdir()
        sentinel = self.tools / "rust/retained"
        sentinel.write_text("original")
        with patch("tooling.sqlite_prepare.download", side_effect=ValueError("bad hash")):
            with self.assertRaisesRegex(ValueError, "bad hash"):
                prepare_sqlite(self.project, self.tools)
        self.assertFalse((self.tools / "sqlite-source").exists())
        self.assertEqual(sentinel.read_text(), "original")


if __name__ == "__main__":
    unittest.main()
