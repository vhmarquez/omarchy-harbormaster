"""Offline dependency inputs must fail closed before Cargo executes any source."""
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from tooling.dependencies import cache_record, stage_cargo, verify_dependencies
from tooling.dependency_prepare import prepare_dependencies
from tooling.install import private_root


class DependencyInputs(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = private_root(self.root / "tools")
        self.project = self.root / "project"
        (self.project / "tools").mkdir(parents=True)
        self.now = datetime.now(timezone.utc)
        self.archive = b"fixture crate archive: never executed"
        self.checksum = hashlib.sha256(self.archive).hexdigest()
        self.entry = {"name": "fixture", "vers": "1.0.0", "cksum": self.checksum, "yanked": False}
        self.record = json.dumps(self.entry).encode() + b"\n"
        self.config = b'{"dl":"https://static.crates.io/crates","api":"https://crates.io"}\n'
        self.pins = {"schema": 1, "index_commit": "a" * 40, "commit_date": self.now.isoformat(),
                     "maximum_age_days": 7, "config_sha256": hashlib.sha256(self.config).hexdigest(),
                     "packages": [{"name": "fixture", "version": "1.0.0", "checksum": self.checksum,
                                   "index_sha256": hashlib.sha256(self.record).hexdigest()}]}
        self.group = self.tools / "dependencies"
        self.registry = self.group / "cargo/registry"
        self.index = self.registry / "index/index.crates.io-1949cf8c6b5b557f"
        self.source = self.registry / "cache/index.crates.io-1949cf8c6b5b557f/fixture-1.0.0.crate"
        self.raw = self.group / "indexes/fixture"
        self.cache = self.index / ".cache/fi/xt/fixture"
        for path, content in ((self.source, self.archive), (self.raw, self.record),
                              (self.index / "config.json", self.config),
                              (self.cache, cache_record(self.record))):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        self.save_pins()
        (self.project / "Cargo.lock").write_text('version = 4\n[[package]]\nname="fixture"\n'
            'version="1.0.0"\nsource="registry+https://github.com/rust-lang/crates.io-index"\n'
            f'checksum="{self.checksum}"\n')

    def save_pins(self):
        (self.project / "tools/dependencies.lock.json").write_text(json.dumps(self.pins))

    def verify(self, now=None):
        return verify_dependencies(self.project, self.tools, now=now or self.now)

    def test_complete_inputs_pass_without_writes(self):
        before = {p: p.stat().st_mtime_ns for p in self.tools.rglob("*")}
        self.assertEqual(self.verify()["packages"], 1)
        self.assertEqual(before, {p: p.stat().st_mtime_ns for p in self.tools.rglob("*")})

    def test_missing_source_and_registry_inputs_fail(self):
        for path in (self.source, self.raw, self.cache, self.index / "config.json"):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.unlink()
                with self.assertRaises((ValueError, OSError)):
                    self.verify()
                path.write_bytes(original)

    def test_corrupt_source_and_registry_inputs_fail(self):
        for path in (self.source, self.raw, self.cache, self.index / "config.json"):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.write_bytes(original + b"corruption")
                with self.assertRaises(ValueError):
                    self.verify()
                path.write_bytes(original)

    def test_yanked_locked_release_rejected_even_with_matching_pins(self):
        self.entry["yanked"] = True
        record = json.dumps(self.entry).encode() + b"\n"
        self.raw.write_bytes(record)
        self.cache.write_bytes(cache_record(record))
        self.pins["packages"][0]["index_sha256"] = hashlib.sha256(record).hexdigest()
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "yanked"):
            self.verify()

    def test_stale_and_future_index_rejected(self):
        for delta in (timedelta(days=7), timedelta(microseconds=-1)):
            with self.subTest(delta=delta), self.assertRaisesRegex(ValueError, "stale|future"):
                self.verify(self.now + delta)
        self.verify(self.now + timedelta(days=7) - timedelta(microseconds=1))

    def test_unexpected_and_symlink_files_rejected(self):
        extra = self.group / "cargo/config.toml"
        extra.write_text('[net]\noffline=false\n')
        with self.assertRaisesRegex(ValueError, "unexpected"):
            self.verify()
        extra.unlink()
        extra.symlink_to(self.raw)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.verify()

    def test_cargo_lock_and_registry_checksum_must_match(self):
        lock = self.project / "Cargo.lock"
        lock.write_text(lock.read_text().replace(self.checksum, "f" * 64))
        with self.assertRaisesRegex(ValueError, "Cargo.lock"):
            self.verify()

    def test_duplicate_or_malformed_records_rejected(self):
        for record in (self.record + self.record, b'{"vers":"1","vers":"2"}\n', b'{"vers":NaN}\n'):
            with self.subTest(record=record), self.assertRaises(ValueError):
                cache_record(record)

    def test_staging_uses_fresh_private_home_and_copies_inputs(self):
        destination = self.root / "cargo"
        stage_cargo(self.project, self.tools, destination)
        staged = destination / self.source.relative_to(self.group / "cargo")
        self.assertEqual(staged.read_bytes(), self.archive)
        staged.write_bytes(b"private mutation")
        self.assertEqual(self.source.read_bytes(), self.archive)
        with self.assertRaisesRegex(ValueError, "empty"):
            stage_cargo(self.project, self.tools, destination)

    def test_failed_integrity_never_stages_inputs(self):
        destination = self.root / "cargo"
        self.source.write_bytes(b"corrupt")
        with self.assertRaises(ValueError):
            stage_cargo(self.project, self.tools, destination)
        self.assertFalse(destination.exists())

    def test_online_preparation_never_overwrites_existing_group(self):
        with patch("tooling.dependency_prepare.download") as download:
            with self.assertRaisesRegex(ValueError, "already exists"):
                prepare_dependencies(self.project, self.tools)
            download.assert_not_called()
        self.assertEqual(self.source.read_bytes(), self.archive)

    def test_failed_online_group_preserves_other_installed_groups(self):
        tools = private_root(self.root / "fresh-tools")
        (tools / "rust").mkdir()
        sentinel = tools / "rust/existing-owned-file"
        sentinel.write_text("unchanged")
        with patch("tooling.dependency_prepare.download", side_effect=ValueError("bad hash")):
            with self.assertRaisesRegex(ValueError, "bad hash"):
                prepare_dependencies(self.project, tools)
        self.assertFalse((tools / "dependencies").exists())
        self.assertEqual(sentinel.read_text(), "unchanged")
        self.assertEqual(set(p.name for p in tools.iterdir()), {"rust", ".harbormaster-tools.json"})

    def test_duplicate_dependency_name_and_unsafe_version_rejected(self):
        self.pins["packages"] *= 2
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.verify()
        self.pins["packages"] = self.pins["packages"][:1]
        self.pins["packages"][0]["version"] = "../../1.0.0"
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "version"):
            self.verify()


if __name__ == "__main__":
    unittest.main()
