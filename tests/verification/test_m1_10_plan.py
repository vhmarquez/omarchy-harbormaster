"""Reducer/recovery targets cannot hide behind existing library or storage runs."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from verification.checks import plan, validate


class RecoveryInventoryTests(unittest.TestCase):
    def test_each_new_family_is_explicit_required_portable_and_combined(self):
        for scope in ("portable", "all"):
            inventory = {name: (argv, kind) for name, argv, kind in plan(scope=scope)}
            for name, target in (("rust-domain", "domain"),
                                 ("rust-storage-reducer", "storage_reducer"),
                                 ("rust-recovery", "recovery"),
                                 ("rust-coordination", "coordination")):
                with self.subTest(scope=scope, target=target):
                    self.assertIn(name, inventory)
                    argv, kind = inventory[name]
                    self.assertEqual(argv, ["/tools/rust/bin/cargo", "test", "--locked", "--offline",
                                            "--workspace", "--test", target, "--all-features"])
                    self.assertEqual(kind, "rust")
                    self.assertFalse(validate(kind, "test result: ok. 0 passed; 0 failed; 0 ignored;"))
        native = {name for name, _, _ in plan(scope="native")}
        self.assertEqual(native, {"isolation-probe", "tool-pins", "sqlite-build",
                                  "rust-ipc-namespace", "m0-harness-sandbox"})


if __name__ == "__main__":
    unittest.main()
