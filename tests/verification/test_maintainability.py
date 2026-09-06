"""Cohesive public-API contract tests using only temporary repositories.

Rationale for suite size: scope, security, parsing and threshold regressions
share the same tiny repository fixture. Large source fixtures are generated,
not pasted; keeping these cases together makes the reporting contract visible.
"""
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).resolve().parents[2] / "scripts/verification/maintainability.py"


class MaintainabilityTests(unittest.TestCase):
    def setUp(self):
        self.sandbox = tempfile.TemporaryDirectory()
        self.addCleanup(self.sandbox.cleanup)
        self.root = Path(self.sandbox.name) / "repo"
        self.root.mkdir()

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        return target

    def report(self, root=None):
        self.assertTrue(MODULE_PATH.is_file(), "maintainability reporter must exist")
        spec = importlib.util.spec_from_file_location("maintainability", MODULE_PATH)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module.report(self.root if root is None else root)

    def test_empty_report_has_stable_public_contract(self):
        result = self.report()
        self.assertEqual(result["schema_version"], 1)
        self.assertEqual(result["production"]["files"], [])
        self.assertEqual(result["production"]["hotspots"], [])
        self.assertEqual(result["tests"]["files"], [])
        self.assertIn("rationale", result["tests"])
        self.assertEqual(result["coverage"]["files_analyzed"], 0)
        self.assertTrue(result["coverage"]["complete"])
        self.assertEqual(result, self.report())
        self.assertNotIn(str(self.root), json.dumps(result))

    def test_scope_separates_languages_tests_and_exclusions(self):
        production = {
            "crates/engine/src/lib.rs": "fn run() {}\n",
            "qml/Main.qml": "Item {}\n",
            "scripts/verify.py": "pass\n",
            "scripts/verification/check.py": "pass\n",
            "scripts/preparation/tools.py": "pass\n",
        }
        tests = {
            "crates/engine/tests/cli.rs": "fn check() {}\n",
            "qml/tests/tst_main.qml": "TestCase {}\n",
            "tests/verification/test_check.py": "pass\n",
            "scripts/verification/test_inline.py": "pass\n",
        }
        excluded = [
            "spikes/bad.py", "docs/design/bad.qml", "frozen/bad.qml",
            "vendor/bad.rs", "target/bad.rs", ".tools/bad.py", ".git/bad.py",
            "qml/generated/Auto.qml", "crates/engine/build.rs",
            "scripts/verify-m0.py", "tests/test_m0_contracts.py", "README.md",
        ]
        for path, text in {**production, **tests}.items():
            self.write(path, text)
        for path in excluded:
            self.write(path, "not source code")
        result = self.report()
        self.assertEqual([f["path"] for f in result["production"]["files"]], sorted(production))
        self.assertEqual([f["path"] for f in result["tests"]["files"]], sorted(tests))
        self.assertEqual(result["coverage"]["files_analyzed"], 9)
        self.assertEqual(result["coverage"]["by_language"], {"python": 5, "rust": 2, "qml": 2})
        exclusions = result["coverage"]["exclusions"]
        for path in excluded:
            self.assertTrue(any(path == e["path"] or path.startswith(e["path"] + "/") for e in exclusions), path)
        self.assertEqual(result, self.report())
        self.assertTrue(result["coverage"]["scope"])

    def test_symlinks_are_reported_without_following_targets(self):
        outside = Path(self.sandbox.name) / "outside"
        outside.mkdir()
        (outside / "secret.py").write_text("def secret():\n    pass\n", encoding="utf-8")
        self.write("scripts/safe.py", "pass\n")
        (self.root / "scripts/leak.py").symlink_to(outside / "secret.py")
        (self.root / "scripts/external").symlink_to(outside, target_is_directory=True)
        (self.root / "scripts/loop").symlink_to(self.root, target_is_directory=True)
        (self.root / "scripts/broken.py").symlink_to(outside / "missing")
        result = self.report()
        self.assertEqual([f["path"] for f in result["production"]["files"]], ["scripts/safe.py"])
        skipped = [e for e in result["coverage"]["exclusions"] if e["reason"] == "symlink not followed"]
        self.assertEqual(len(skipped), 4)
        self.assertFalse(result["coverage"]["complete"])
        self.assertNotIn(str(outside), json.dumps(result))

    def test_symlink_root_and_ancestor_are_rejected(self):
        alias = Path(self.sandbox.name) / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        for root in (alias, alias / "child", self.root / "missing"):
            with self.subTest(root=root):
                result = self.report(root)
                self.assertFalse(result["coverage"]["complete"])
                self.assertEqual(result["coverage"]["errors"][0]["path"], ".")
                self.assertEqual(result["production"]["files"], [])
                self.assertNotIn(str(root), json.dumps(result))

    def test_file_threshold_is_strict_nonblank_and_advisory(self):
        for size in (300, 301):
            self.write(f"scripts/size_{size}.py", "\nvalue = 1\n   \n" * size)
        self.write("tests/verification/test_large.py", "assert True\n" * 301)
        result = self.report()
        self.assertEqual([f.get("nonblank_lines") for f in result["production"]["files"]], [300, 301])
        self.assertEqual(result["thresholds"], {"file_nonblank": 300, "function_nonblank": 50, "policy": "review only; strictly greater than threshold"})
        hotspots = result["production"]["hotspots"]
        self.assertEqual(len(hotspots), 1)
        self.assertEqual((hotspots[0]["path"], hotspots[0]["kind"], hotspots[0]["nonblank_lines"]), ("scripts/size_301.py", "file", 301))
        self.assertEqual(hotspots[0]["disposition"], "review required")
        self.assertEqual(len(result["tests"]["hotspots"]), 1)
        self.assertTrue(result["coverage"]["complete"])

    def test_python_ast_spans_nested_functions_and_control_counts(self):
        lines = [
            "@decorate", "async def outer(", "    value,", "):",
            "    # Braces and fake keywords: if { fn fake() }",
            "    def inner():", "        for item in []:",
            "            if item:", "                return item",
            "    if value and await ready():", "        return inner()",
            "", "class Service:", "    def check(self):", "        return 1",
        ]
        self.write("scripts/verification/flow.py", "\n".join(lines))
        file = self.report()["production"]["files"][0]
        functions = {f["name"]: f for f in file.get("functions", [])}
        self.assertEqual(list(functions), ["outer", "outer.inner", "Service.check"])
        self.assertEqual(file["metric_kind"], "python-ast-exact")
        outer, inner = functions["outer"], functions["outer.inner"]
        self.assertEqual((outer["line"], outer["end_line"], outer["nonblank_lines"]), (1, 11, 11))
        self.assertEqual((outer["branches"], outer["nesting"]), (2, 1))
        self.assertEqual((inner["line"], inner["end_line"], inner["branches"], inner["nesting"]), (6, 9, 2, 2))

    def test_python_function_threshold_is_strict_nonblank(self):
        for size in (50, 51):
            text = f"def size_{size}():\n" + "    value = 1\n\n" * (size - 1)
            self.write(f"scripts/size_{size}.py", text)
        result = self.report()
        hotspots = result["production"]["hotspots"]
        self.assertEqual(len(hotspots), 1)
        self.assertEqual((hotspots[0]["name"], hotspots[0]["kind"], hotspots[0]["nonblank_lines"]), ("size_51", "function", 51))
        self.assertEqual(hotspots[0]["path"], "scripts/size_51.py")
        self.assertEqual(hotspots[0]["line"], 1)
        self.assertTrue(result["coverage"]["complete"])

    def test_lexical_multiline_and_nested_functions(self):
        for language, path, keyword in (
            ("rust", "crates/engine/src/lib.rs", "fn"),
            ("qml", "qml/Main.qml", "function"),
        ):
            with self.subTest(language=language):
                lines = [f"{keyword} outer(", "    value", ") {", f"    {keyword} inner() {{",
                         "        if (value) {", "            while (value) {}", "        }", "    }",
                         "    if (value && value) {}", "}"]
                self.write(path, "\n".join(lines))
                file = next(f for f in self.report()["production"]["files"] if f["path"] == path)
                functions = file.get("functions", [])
                self.assertEqual([f["name"] for f in functions], ["outer", "outer.inner"])
                self.assertEqual(file["metric_kind"], "lexical-approximate")
                outer, inner = functions
                self.assertEqual((outer["line"], outer["end_line"], outer["nonblank_lines"]), (1, 10, 10))
                self.assertEqual((outer["branches"], outer["nesting"]), (2, 1))
                self.assertEqual((inner["branches"], inner["nesting"]), (2, 2))

    def test_lexical_ignores_quotes_raw_strings_and_nested_comments(self):
        snippets = {
            "crates/engine/src/lib.rs": [
                'fn real<\'a>(text: &\'a str) {',
                '    let text = "} fn fake() { if while &&";',
                '    let raw = br##"} fn raw_fake() { "# if"##;',
                "    let ch = '}'; let quote = '\\\'';",
                '    /* } fn comment() { /* nested */ if */',
                '    // } fn line_comment() { if',
                '    if true { println!("\\\"{"); }',
                '}', 'fn after() {}',
            ],
            "qml/Main.qml": [
                'function real() {',
                '    let text = "} function fake() { if while &&";',
                "    let single = '} function single() { if';",
                '    let template = `} function template() { ${value}`;',
                '    /* } function comment() { if */',
                '    // } function line_comment() { if',
                '    if (true) { console.log("\\\"{"); }',
                '}', 'function after() {}',
            ],
        }
        for path, lines in snippets.items():
            self.write(path, "\n".join(lines))
        for file in self.report()["production"]["files"]:
            with self.subTest(path=file["path"]):
                self.assertEqual([f["name"] for f in file["functions"]], ["real", "after"])
                real = file["functions"][0]
                self.assertEqual((real["end_line"], real["nonblank_lines"], real["branches"], real["nesting"]), (8, 8, 1, 1))

    def test_lexical_signatures_skip_declarations_and_parameter_braces(self):
        self.write("crates/engine/src/lib.rs", "\n".join([
            "trait Api {", "    fn declared(&self);", "}",
            "fn r#match(", "    values: [u8; 4],", ") -> [u8; 4] {", "    values", "}",
        ]))
        self.write("qml/Main.qml", "function real(value = {key: 1}) {\n    return value\n}\n")
        files = self.report()["production"]["files"]
        self.assertEqual([f["name"] for f in files[0]["functions"]], ["r#match"])
        self.assertEqual((files[0]["functions"][0]["line"], files[0]["functions"][0]["end_line"]), (4, 8))
        self.assertEqual(files[1]["functions"][0]["end_line"], 3)

    def test_invalid_sources_are_explicit_partial_coverage(self):
        bad = {
            "scripts/invalid.py": "def broken(:\n",
            "crates/engine/src/open.rs": "fn broken() {\n",
            "qml/Close.qml": "}\n",
            "qml/String.qml": 'function broken() { let s = "unterminated',
            "crates/engine/src/raw.rs": 'fn broken() { let s = r##"unterminated',
            "qml/Comment.qml": "/* unterminated",
        }
        for path, text in bad.items():
            self.write(path, text)
        self.write("scripts/good.py", "def good():\n    return 1\n")
        self.write("scripts/encoding.py", "").write_bytes(b"\xff")
        try:
            result = self.report()
        except Exception as exc:
            self.fail(f"source errors must be reported, not raised: {type(exc).__name__}")
        self.assertFalse(result["coverage"]["complete"])
        self.assertEqual({e["path"] for e in result["coverage"]["errors"]}, set(bad) | {"scripts/encoding.py"})
        files = {f["path"]: f for f in result["production"]["files"]}
        self.assertEqual(files["scripts/good.py"]["functions"][0]["name"], "good")
        self.assertTrue(files["scripts/invalid.py"]["analysis_error"])
        self.assertNotIn(str(self.root), json.dumps(result))

    def test_generated_headers_and_test_file_names_are_partitioned(self):
        self.write("qml/Auto.qml", "// @generated\nnot valid source")
        self.write("scripts/tool.py", "# Code generated by generator; DO NOT EDIT.\nnot valid")
        test_paths = ["crates/engine/src/tests.rs", "crates/engine/src/parse_tests.rs", "qml/tst_window.qml"]
        for path in test_paths:
            self.write(path, "// test fixture\n")
        result = self.report()
        self.assertEqual(result["production"]["files"], [])
        self.assertEqual([f["path"] for f in result["tests"]["files"]], sorted(test_paths))
        self.assertEqual({e["path"] for e in result["coverage"]["exclusions"] if e["reason"] == "generated header"}, {"qml/Auto.qml", "scripts/tool.py"})
        self.assertTrue(result["coverage"]["complete"])


    def test_cli_is_json_advisory_and_documents_metric_limits(self):
        self.write("scripts/large.py", "value = 1\n" * 301)
        process = subprocess.run(
            [sys.executable, "-B", str(MODULE_PATH), str(self.root)],
            cwd=self.sandbox.name, capture_output=True, text=True, timeout=10,
            env={"HOME": self.sandbox.name, "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin"},
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertTrue(process.stdout.strip(), "CLI must print the report")
        result = json.loads(process.stdout)
        self.assertEqual(result, self.report())
        self.assertEqual(len(result["production"]["hotspots"]), 1)
        coverage = result["coverage"]
        self.assertIn("inline", " ".join(coverage["limitations"]).lower())
        self.assertIn("lexical", " ".join(coverage["limitations"]).lower())
        self.assertIn("branches", coverage["metric_definitions"])
        self.assertIn("nesting", coverage["metric_definitions"])
        self.assertEqual(coverage["excluded_directory_names"], sorted(coverage["excluded_directory_names"]))


    def test_special_files_are_reported_without_reading(self):
        self.write("scripts/good.py", "pass\n")
        os.mkfifo(self.root / "scripts/pipe.py")
        result = self.report()
        self.assertFalse(result["coverage"]["complete"])
        self.assertEqual(result["coverage"]["errors"], [{"path": "scripts/pipe.py", "reason": "not a regular source file"}])
        self.assertEqual(result["coverage"]["files_analyzed"], 1)

    def test_analysis_totals_exclude_failed_files(self):
        self.write("scripts/good.py", "pass\n")
        self.write("scripts/bad.py", "def syntax(:\n")
        result = self.report()
        self.assertEqual(result["coverage"]["files_analyzed"], 1)
        self.assertEqual(result["coverage"]["files_listed"], 2)
        self.assertEqual(result["coverage"]["by_language"], {"python": 1, "qml": 0, "rust": 0})

    def test_lexical_function_thresholds_with_generated_fixtures(self):
        for directory, extension, keyword in (("crates/engine/src", "rs", "fn"), ("qml", "qml", "function")):
            for size in (50, 51):
                lines = [f"{keyword} size_{size}() {{"] + ['    let value = "} if {";'] * (size - 2) + ["}"]
                self.write(f"{directory}/size_{size}.{extension}", "\n\n".join(lines))
        result = self.report()
        self.assertEqual(len(result["production"]["hotspots"]), 2)
        for hotspot in result["production"]["hotspots"]:
            self.assertEqual((hotspot["kind"], hotspot["name"], hotspot["nonblank_lines"], hotspot["branches"]), ("function", "size_51", 51, 0))

    def test_cli_module_invocation(self):
        process = subprocess.run(
            [sys.executable, "-B", "-m", "scripts.verification.maintainability", str(self.root)],
            cwd=MODULE_PATH.parents[2], capture_output=True, text=True, timeout=10,
            env={"HOME": self.sandbox.name, "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin"},
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(json.loads(process.stdout), self.report())


if __name__ == "__main__":
    unittest.main()
