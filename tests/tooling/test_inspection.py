"""Read-only inspection rejects altered hashes and Git metadata symlinks."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from tooling.inspection import verify_database, verify_file


class InspectionTests(unittest.TestCase):
    def test_file_hash_and_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "binary"
            file.write_bytes(b"fixture")
            with self.assertRaisesRegex(ValueError, "SHA256"):
                verify_file(file, "0" * 64)
            link = Path(directory) / "link"
            link.symlink_to(file)
            with self.assertRaisesRegex(ValueError, "symlink"):
                verify_file(link, hashlib.sha256(b"fixture").hexdigest())

    def test_git_metadata_symlink_is_not_trusted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            parent = root / "advisory-db"
            repository = parent / "db"
            repository.mkdir(parents=True)
            (repository / "advisory.md").write_bytes(b"fixture")
            (repository / ".git").symlink_to(root)
            archive = parent / "snapshot.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                member = tarfile.TarInfo("upstream/advisory.md")
                member.size = 7
                output.addfile(member, io.BytesIO(b"fixture"))
            pin = {"sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "directory": "db",
                   "upstream_commit": "fixture", "commit_date": "fixture", "maximum_age_days": 7}
            (parent / "preparation.json").write_text(json.dumps(pin))
            with self.assertRaisesRegex(ValueError, "symlink"):
                verify_database(root, pin)
            (repository / ".git").unlink()
            with self.assertRaisesRegex(ValueError, "Git"):
                verify_database(root, pin)


if __name__ == "__main__":
    unittest.main()
