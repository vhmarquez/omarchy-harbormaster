"""SQLite source/build and storage suites remain mandatory in explicit scopes."""
from pathlib import Path
import os
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from verification.checks import plan, validate
from verification.sandbox import environment


class StorageInventoryTests(unittest.TestCase):
    def test_sqlite_build_precedes_every_cargo_check_in_each_scope(self):
        for scope in ("portable", "native", "all"):
            checks = plan(scope=scope)
            names = [name for name, _, _ in checks]
            with self.subTest(scope=scope):
                self.assertEqual(names.count("sqlite-build"), 1)
                build = names.index("sqlite-build")
                self.assertGreater(build, names.index("tool-pins"))
                self.assertEqual(checks[build], ("sqlite-build", ["/usr/bin/python3", "-B",
                                                               "scripts/verification/sqlite.py"], "exit"))
                for index, (_, argv, _) in enumerate(checks):
                    if argv[0] == "/tools/rust/bin/cargo":
                        self.assertGreater(index, build)

    def test_storage_and_abrupt_exit_are_separate_required_portable_targets(self):
        checks = {name: (argv, kind) for name, argv, kind in plan(scope="portable")}
        for name, target in (("rust-storage", "storage"), ("rust-storage-crash", "storage_crash")):
            with self.subTest(target=target):
                self.assertIn(name, checks)
                argv, kind = checks[name]
                self.assertEqual(argv, ["/tools/rust/bin/cargo", "test", "--locked", "--offline",
                                       "--workspace", "--test", target, "--all-features"])
                self.assertEqual(kind, "rust")
                self.assertFalse(validate(kind, "test result: ok. 0 passed; 0 failed; 0 ignored;"))
        self.assertNotIn("m0-harness-sandbox", checks)
        self.assertNotIn("rust-ipc-namespace", checks)

    def test_sqlite_link_inputs_are_fixed_and_never_inherited(self):
        expected = {"SQLITE3_NO_PKG_CONFIG": "1", "SQLITE3_STATIC": "1",
                    "SQLITE3_LIB_DIR": "/state/sqlite/lib",
                    "SQLITE3_INCLUDE_DIR": "/state/sqlite/include"}
        hostile = dict.fromkeys(expected, "/host/foreign-sqlite")
        hostile.update(PKG_CONFIG_PATH="/host/foreign", SQLITE3_LIB_NAME="foreign")
        with patch.dict(os.environ, hostile):
            actual = environment()
        self.assertEqual({key: actual.get(key) for key in expected}, expected)
        self.assertNotIn("PKG_CONFIG_PATH", actual)
        self.assertNotIn("SQLITE3_LIB_NAME", actual)


if __name__ == "__main__":
    unittest.main()
