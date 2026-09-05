"""Revision binding uses real, disposable Git repositories and source bytes."""
import hashlib
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))


class RevisionBindingTests(unittest.TestCase):
    def setUp(self):
        from verification import qualification
        self.assertTrue(hasattr(qualification, "source_at_revision"),
                        "source revision binding missing")
        self.bind = qualification.source_at_revision
        from qualification_fixtures import GitFixture
        fixture = GitFixture(self)
        self.root = fixture.root
        self.git = fixture.git
        self.revision = fixture.revision

    def test_exact_commit_bytes_bind(self):
        self.assertEqual(self.bind(self.root, self.revision),
                         {"README.md": hashlib.sha256(b"fixture source\n").hexdigest()})

    def test_changed_tracked_and_extra_untracked_source_fail(self):
        (self.root / "README.md").write_text("changed\n")
        with self.assertRaises(ValueError):
            self.bind(self.root, self.revision)
        (self.root / "README.md").write_text("fixture source\n")
        (self.root / "tests").mkdir()
        (self.root / "tests/extra.py").write_text("pass\n")
        with self.assertRaises(ValueError):
            self.bind(self.root, self.revision)

    def test_non_source_evidence_does_not_change_identity(self):
        (self.root / "evidence").mkdir()
        (self.root / "evidence/result.json").write_text("{}")
        self.assertIn("README.md", self.bind(self.root, self.revision))

    def test_source_symlink_is_rejected(self):
        (self.root / "README.md").unlink()
        (self.root / "fixture").write_text("fixture source\n")
        (self.root / "README.md").symlink_to("fixture")
        with self.assertRaises(ValueError):
            self.bind(self.root, self.revision)

    def test_literal_revision_format_and_commit_type_are_required(self):
        for value in ("main", self.revision[:8], self.revision.upper(), "-" * 40):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    self.bind(self.root, value)
        tree = self.git("rev-parse", "HEAD^{tree}").strip()
        with self.assertRaises(ValueError):
            self.bind(self.root, tree)


if __name__ == "__main__":
    unittest.main()
