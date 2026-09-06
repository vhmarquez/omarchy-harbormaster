"""CLI scope parsing cannot silently turn Docker green into full qualification."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import verify


class ScopeArgumentsTests(unittest.TestCase):
    def options(self, *arguments):
        with patch.object(sys, "argv", ["verify.py", *arguments]):
            return verify.arguments()

    def test_local_defaults_require_all_qualifications(self):
        options = self.options()
        self.assertTrue(hasattr(options, "scope"), "explicit qualification scopes missing")
        self.assertEqual((options.backend, options.scope), ("bwrap", "all"))

    def test_portable_and_native_are_explicit(self):
        self.assertEqual(self.options("--scope", "native").scope, "native")
        options = self.options("--backend", "docker", "--scope", "portable", "--image", "fixture")
        self.assertEqual((options.backend, options.scope), ("docker", "portable"))

    def test_docker_cannot_claim_native_or_default_full_qualification(self):
        for arguments in (("--backend", "docker"),
                          ("--backend", "docker", "--scope", "native"),
                          ("--backend", "docker", "--scope", "all")):
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    self.options(*arguments)
                self.assertEqual(raised.exception.code, 2)

    def test_paired_qualification_requires_revision_and_rejects_run_options(self):
        revision = "a" * 40
        options = self.options("--qualify", "/portable", "/native", "--revision", revision)
        self.assertEqual(options.qualify, [Path("/portable"), Path("/native")])
        self.assertEqual(options.revision, revision)
        cases = (("--qualify", "/portable", "/native"),
                 ("--revision", revision),
                 ("--qualify", "/portable", "/native", "--revision", revision,
                  "--scope", "portable"),
                 ("--qualify", "/portable", "/native", "--revision", revision,
                  "--image", ""))
        for arguments in cases:
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.options(*arguments)

    def test_empty_revision_cli_rejects_before_creating_execution_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for index, arguments in enumerate((("--revision", ""), ("--revision=", "--scope", "native"))):
                output = root / f"output-{index}"
                # Missing fixture tools prevent any real execution on the RED
                # path; correct argument rejection must happen even earlier.
                command = [sys.executable, "-B", str(Path(verify.__file__)), *arguments,
                           "--tools", str(root / "missing-tools"), "--output", str(output)]
                with self.subTest(arguments=arguments):
                    result = subprocess.run(command, cwd=root, env={"PATH": "/usr/bin"},
                                            stdin=subprocess.DEVNULL, capture_output=True,
                                            text=True, timeout=5)
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn("--revision requires paired --qualify reports", result.stderr)
                    self.assertEqual(result.stdout, "")
                    self.assertFalse(output.exists())
                    self.assertFalse((root / ".verify").exists())


if __name__ == "__main__":
    unittest.main()
