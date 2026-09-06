"""Real read-only pin preflight, exercised within the canonical offline boundary."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification.checks import plan
from verification.runner import execute_checks


class RustPinTests(unittest.TestCase):
    def fixture(self, root):
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copyfile(ROOT / name, root / name)
        for name in ("scripts", "tools"):
            shutil.copytree(ROOT / name, root / name)
        return json.loads((root / "tools/toolchain.lock.json").read_text())

    def preflight(self, root):
        pins = next(check for check in plan() if check[0] == "tool-pins")
        after = ("after-pins", ["/usr/bin/python3", "-B", "-c", "print('executed')"], "exit")
        output = root / "logs"
        output.mkdir()

        def executor(argv):
            # These nested regression probes inspect pins read-only; the outer
            # canonical preflight already prepared the shared Cargo home.
            argv = [argument for argument in argv if argument != "--stage-cargo"]
            result = subprocess.run(argv, cwd=root, capture_output=True,
                                    text=True, timeout=60, check=False)
            return {"exit_code": result.returncode, "output": result.stdout + result.stderr}

        return execute_checks([pins, after], executor, output)

    def test_each_rust_declaration_must_equal_the_checked_compiler_release(self):
        for case in ("channel", "workspace", "lock-release", "all-declarations"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                lock = self.fixture(root)
                release = lock["rust"]["version"]
                if case in ("channel", "all-declarations"):
                    path = root / "rust-toolchain.toml"
                    path.write_text(path.read_text().replace(f'channel = "{release}"', 'channel = "1.97.0"'))
                if case in ("workspace", "all-declarations"):
                    path = root / "Cargo.toml"
                    path.write_text(path.read_text().replace(f'rust-version = "{release}"', 'rust-version = "1.97.0"'))
                if case in ("lock-release", "all-declarations"):
                    lock["rust"]["version"] = "1.97.0"
                    (root / "tools/toolchain.lock.json").write_text(json.dumps(lock))
                results = self.preflight(root)
                self.assertEqual([r["status"] for r in results], ["FAIL", "NOT_RUN"])
                self.assertIn("Rust declaration mismatch", (root / "logs/tool-pins.txt").read_text())

    def test_matching_declarations_and_checked_compiler_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            self.assertEqual([r["status"] for r in self.preflight(root)], ["PASS", "PASS"])

    def test_dependency_pin_failures_block_following_execution(self):
        for case in ("missing-lock", "stale-index", "corrupt-index-pin"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                self.fixture(root)
                path = root / "tools/dependencies.lock.json"
                pins = json.loads(path.read_text())
                if case == "missing-lock":
                    path.unlink()
                else:
                    if case == "stale-index":
                        pins["commit_date"] = "2000-01-01T00:00:00Z"
                    else:
                        pins["packages"][0]["index_sha256"] = "f" * 64
                    path.write_text(json.dumps(pins))
                results = self.preflight(root)
                self.assertEqual([r["status"] for r in results], ["FAIL", "NOT_RUN"])
                log = (root / "logs/tool-pins.txt").read_text()
                self.assertIn({"missing-lock": "No such file", "stale-index": "stale",
                               "corrupt-index-pin": "SHA256 mismatch"}[case], log)


if __name__ == "__main__":
    unittest.main()
