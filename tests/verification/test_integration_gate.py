"""Real Cargo absent/empty-suite regressions inside the verifier boundary."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification.checks import plan, validate


class IntegrationGateTests(unittest.TestCase):
    def test_library_tests_cannot_satisfy_missing_or_empty_integration_target(self):
        targets = [check for check in plan(scope="portable") if check[2] == "rust" and "--test" in check[1]]
        self.assertEqual(len(targets), 11)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            (root / "src/lib.rs").write_text("#[test] fn unrelated_unit() {}\n")
            (root / "Cargo.toml").write_text(
                '[package]\nname="integration-gate-fixture"\nversion="0.1.0"\nedition="2024"\n')
            (root / "Cargo.lock").write_text(
                'version = 4\n[[package]]\nname = "integration-gate-fixture"\nversion = "0.1.0"\n')
            tests = root / "tests"
            tests.mkdir()
            env = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"))
            for name, argv, kind in targets:
                target = argv[argv.index("--test") + 1]
                for fixture in ("absent", "empty"):
                    with self.subTest(target=name, fixture=fixture):
                        if fixture == "empty":
                            (tests / (target + ".rs")).write_text("// Deliberately no tests.\n")
                        result = subprocess.run(argv, cwd=root, env=env, capture_output=True,
                                                text=True, timeout=60, check=False)
                        output = result.stdout + result.stderr
                        self.assertFalse(result.returncode == 0 and validate(kind, output), output)
                        if fixture == "absent":
                            self.assertNotEqual(result.returncode, 0, output)
                            self.assertIn("no test target named", output)
                        else:
                            self.assertEqual(result.returncode, 0, output)
                            self.assertIn("0 passed; 0 failed; 0 ignored;", output)
                            (tests / (target + ".rs")).unlink()


if __name__ == "__main__":
    unittest.main()
