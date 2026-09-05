"""Scoped inventories and inert subprocess fixtures; no native sandbox launch."""
import ast
import inspect
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification.checks import NATIVE_METHODS, passed, plan
from verification.runner import execute_checks
from verification.sandbox import _execute

PORTABLE_NAMES = [
    "isolation-probe", "tool-pins", "native-tool-pins", "rust-format",
    "rust-clippy", "rust-unit", "rust-integration", "qml-lint", "qml-format",
    "qml-native", "python-verification", "python-tooling", "m0-contract-tests",
    "m0-fixture-contract", "m0-runtime-identity", "m0-runtime-cleanup",
    "m0-harness-helpers", "m0-design-model", "frozen-design-integrity",
    "advisory-license", "maintainability",
]
NATIVE_TARGET = (
    "m0-harness-sandbox",
    ["/usr/bin/python3", "-B", "spikes/harnesses/test_sandbox.py", *NATIVE_METHODS, "-v"],
    "native",
)


class ScopePlanTests(unittest.TestCase):
    def test_portable_preserves_unique_common_inventory_without_native_target(self):
        self.assertIn("scope", inspect.signature(plan).parameters,
                      "explicit verification scopes missing")
        portable = plan(scope="portable")
        names = [name for name, _, _ in portable]
        self.assertEqual(names, PORTABLE_NAMES)
        self.assertEqual(len(names), 21)
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(next(item for item in portable if item[0] == "m0-harness-helpers"),
                         ("m0-harness-helpers", ["/usr/bin/python3", "-B", "-m", "unittest",
                          "discover", "-s", "spikes/harnesses", "-p", "test_probe.py", "-v"],
                          "python"))

    def test_native_contains_only_shared_preflights_and_explicit_sandbox_target(self):
        portable = plan(scope="portable")
        native = plan(scope="native")
        self.assertEqual(native, portable[:2] + [NATIVE_TARGET])
        self.assertEqual(len({name for name, _, _ in native}), 3)

    def test_unknown_scope_is_rejected_instead_of_becoming_all(self):
        for scope in ("", "optional", "ALL", "native ", None, []):
            with self.subTest(scope=scope), self.assertRaisesRegex(ValueError, "scope"):
                plan(scope=scope)

    def test_all_is_the_unique_union_with_shared_preflights_once(self):
        portable = plan(scope="portable")
        native = plan(scope="native")
        all_checks = plan(scope="all")
        self.assertEqual(plan(), all_checks)
        self.assertEqual(all_checks, portable + [NATIVE_TARGET])
        names = [name for name, _, _ in all_checks]
        self.assertEqual(len(names), 22)
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(set(names), {item[0] for item in portable + native})
        self.assertEqual({item[0] for item in portable} & {item[0] for item in native},
                         {"isolation-probe", "tool-pins"})

    def test_custom_canary_is_preserved_in_each_scope(self):
        canary = "/synthetic outside-canary/unchanged"
        for scope in ("all", "portable", "native"):
            with self.subTest(scope=scope):
                self.assertEqual(plan(canary, scope)[0],
                                 ("isolation-probe", ["/usr/bin/python3", "-B",
                                  "scripts/verification/probe.py", canary], "exit"))


class HarnessPartitionTests(unittest.TestCase):
    def test_harness_files_partition_four_portable_tests_and_two_native_tests(self):
        expected = {
            "test_probe.py": {
                "Install.test_overlay_refuses_symlink_and_concurrent_edits",
                "Install.test_reversible_overlay_preserves_unrelated_hooks_and_exact_original_bytes",
                "EventProjection.test_invalid_oversized_and_unwritable_payloads_fail_open_silently",
                "EventProjection.test_command_hook_emits_only_allowlisted_metadata_without_stdout",
            },
            "test_sandbox.py": set(NATIVE_TARGET[1][3:-1]),
        }
        for filename, names in expected.items():
            with self.subTest(filename=filename):
                path = ROOT / "spikes/harnesses" / filename
                self.assertTrue(path.is_file(), "standalone native sandbox test missing")
                tree = ast.parse(path.read_text(encoding="utf-8"))
                actual = [f"{node.name}.{method.name}" for node in tree.body
                          if isinstance(node, ast.ClassDef) for method in node.body
                          if isinstance(method, ast.FunctionDef) and method.name.startswith("test_")]
                self.assertEqual(set(actual), names)
                self.assertEqual(len(actual), len(names))


class NativeTargetTests(unittest.TestCase):
    def run_fixture(self, source):
        target = plan(scope="native")[-1]
        self.assertEqual(target, NATIVE_TARGET)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "spikes/harnesses/test_sandbox.py"
            path.parent.mkdir(parents=True)
            if source is not None:
                path.write_text(source, encoding="utf-8")

            def executor(argv):
                # Inert fixtures run inside the enclosing verifier boundary. This
                # child changes only cwd, retaining existing time/output bounds;
                # it never calls bwrap or imports the real sandbox test.
                wrapper = ["/usr/bin/python3", "-B", "-c",
                           "import os,sys; os.chdir(sys.argv[1]); os.execv(sys.argv[2], sys.argv[2:])",
                           str(root)]
                return _execute(wrapper + argv, timeout=5)

            results = execute_checks([target], executor, root)
            self.assertEqual(len(results), 1)
            result = results[0]
            self.assertEqual(result["name"], NATIVE_TARGET[0])
            self.assertEqual(result["command"], NATIVE_TARGET[1])
            self.assertIs(result["required"], True)
            self.assertEqual(result["status"], "FAIL")
            self.assertFalse(passed(results))
            self.assertFalse(result["timed_out"])
            self.assertFalse(result["output_limited"])
            return result, (root / result["log"]).read_text(encoding="utf-8")

    def test_missing_empty_or_renamed_native_target_fails_required_check(self):
        fixtures = {
            "missing": None,
            "empty": "",
            "renamed": (
                "import unittest\n"
                "class Helpers(unittest.TestCase):\n"
                "    def test_renamed(self):\n"
                "        self.assertTrue(True)\n"
                "if __name__ == '__main__':\n"
                "    unittest.main()\n"
            ),
        }
        for name, source in fixtures.items():
            with self.subTest(fixture=name):
                result, output = self.run_fixture(source)
                if name == "empty":
                    self.assertEqual(result["exit_code"], 0, output)
                    self.assertEqual(output, "")
                else:
                    self.assertNotEqual(result["exit_code"], 0, output)

    def test_missing_namespace_method_fails_when_original_target_exists(self):
        source = (
            "import unittest\n"
            "class Helpers(unittest.TestCase):\n"
            "    def test_sandbox_denies_network_and_inherited_credentials(self):\n"
            "        self.assertTrue(True)\n"
            "if __name__ == '__main__':\n"
            "    unittest.main()\n"
        )
        result, output = self.run_fixture(source)
        self.assertNotEqual(result["exit_code"], 0, output)
        self.assertIn("Helpers.test_sandbox_denies_network_and_inherited_credentials) ... ok", output)
        self.assertIn("NetworkNamespace", output)

    def test_either_skipped_native_method_cannot_become_optional_success(self):
        # Synthetic negative fixtures only: neither stub is native qualification.
        methods = NATIVE_TARGET[1][3:-1]
        for skipped in methods:
            with self.subTest(skipped=skipped):
                lines = ["import unittest"]
                for target in methods:
                    class_name, method = target.split(".")
                    lines.append(f"class {class_name}(unittest.TestCase):")
                    if target == skipped:
                        lines.append("    @unittest.skip('synthetic unavailable native backend')")
                    lines.extend([f"    def {method}(self):", "        self.assertTrue(True)"])
                lines.extend(["if __name__ == '__main__':", "    unittest.main()"])
                result, output = self.run_fixture("\n".join(lines) + "\n")
                self.assertEqual(result["exit_code"], 0, output)
                self.assertIn("skipped=1", output)


if __name__ == "__main__":
    unittest.main()
