"""Real Cargo empty-suite regressions, executed inside the verifier boundary."""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification.checks import plan, validate


class IntegrationGateTests(unittest.TestCase):
    def test_library_tests_cannot_satisfy_missing_or_empty_integration_target(self):
        _, argv, kind = next(check for check in plan() if check[0] == "rust-integration")
        for fixture in ("absent", "empty"):
            with self.subTest(fixture=fixture), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
                    shutil.copyfile(ROOT / name, root / name)
                shutil.copytree(ROOT / "crates", root / "crates")
                tests = root / "crates/harbormaster/tests"
                shutil.rmtree(tests)
                if fixture == "empty":
                    tests.mkdir()
                    (tests / "cli.rs").write_text("// Deliberately no integration tests.\n")
                env = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"))
                result = subprocess.run(argv, cwd=root, env=env, capture_output=True,
                                        text=True, timeout=60, check=False)
                output = result.stdout + result.stderr
                self.assertFalse(result.returncode == 0 and validate(kind, output), output)


if __name__ == "__main__":
    unittest.main()
