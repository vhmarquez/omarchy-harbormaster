"""New IPC test families cannot be hidden by the existing CLI/unit suites."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from verification.checks import plan, validate


class IpcInventoryTests(unittest.TestCase):
    def test_each_ipc_family_is_a_separate_required_portable_target(self):
        inventory = {name: (argv, kind) for name, argv, kind in plan(scope="portable")}
        for name, target in (("rust-protocol", "protocol"),
                             ("rust-protocol-fuzz", "protocol_fuzz"),
                             ("rust-ipc", "ipc"), ("rust-ingestion", "ingestion")):
            with self.subTest(target=target):
                self.assertIn(name, inventory)
                argv, kind = inventory[name]
                self.assertEqual(argv[argv.index("--test") + 1], target)
                self.assertEqual(kind, "rust")
                self.assertIn("--locked", argv)
                self.assertIn("--offline", argv)
                self.assertFalse(validate(kind, "test result: ok. 0 passed; 0 failed; 0 ignored;"))
        self.assertFalse(set(inventory) & {"m0-harness-sandbox"})


if __name__ == "__main__":
    unittest.main()
