"""Required-log consistency regressions; synthetic inputs are not execution proof."""
import contextlib
import io
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from qualification_fixtures import CliSyntheticPair, SYNTHETIC_LOGS, SYNTHETIC_NATIVE_LOG
from verification.checks import plan, validate
from verification.runner import execute_checks


def native_log_mutations():
    first, second, summary = SYNTHETIC_NATIVE_LOG.split("\n", 2)
    return {
        "original-only": first + "\n" + summary.replace("Ran 2 tests", "Ran 1 test"),
        "missing-namespace-result-count-two": first + "\n" + summary,
        "missing-original-result-count-two": second + "\n" + summary,
        "namespace-heading-without-ok": SYNTHETIC_NATIVE_LOG.replace(second, second[:-2]),
        "wrong-method": SYNTHETIC_NATIVE_LOG.replace("(__main__.", "(other.", 1),
        "wrong-count": SYNTHETIC_NATIVE_LOG.replace("Ran 2 tests", "Ran 3 tests"),
        "extra-test": first + "\n" + SYNTHETIC_NATIVE_LOG.replace("Ran 2 tests", "Ran 3 tests"),
        "duplicate-result": first + "\n" + SYNTHETIC_NATIVE_LOG,
        "fail-after-ok": SYNTHETIC_NATIVE_LOG + "FAILED (failures=1)\n",
        "fail-before-ok": SYNTHETIC_NATIVE_LOG.replace("\nOK", "\nFAILED (failures=1)\nOK"),
        "duplicate-ok": SYNTHETIC_NATIVE_LOG + "OK\n",
        "duplicate-summary": SYNTHETIC_NATIVE_LOG + summary,
        "skip": SYNTHETIC_NATIVE_LOG.replace(" ... ok", " ... skipped 'fixture'", 1)
                .replace("\nOK", "\nOK (skipped=1)"),
        "expected-failure": SYNTHETIC_NATIVE_LOG.replace(" ... ok", " ... expected failure", 1)
                            .replace("\nOK", "\nOK (expected failures=1)"),
        "unexpected-success": SYNTHETIC_NATIVE_LOG.replace(" ... ok", " ... unexpected success", 1),
        "missing-summary": first + "\n" + second + "\n",
    }


class NativeResultValidationTests(unittest.TestCase):
    def test_native_gate_requires_exact_successful_method_results(self):
        target = plan(scope="native")[-1]
        logs = {"synthetic-positive": SYNTHETIC_NATIVE_LOG, **native_log_mutations()}
        # Match the real merged stdout/stderr shape: diagnostic follows the
        # verbose method heading inline, and unittest's 'ok' is on the next line.
        heading = SYNTHETIC_NATIVE_LOG.splitlines()[1]
        logs["synthetic-inline-diagnostic"] = SYNTHETIC_NATIVE_LOG.replace(
            heading, heading[:-2] + 'network namespace identity: {"synthetic": true}\nok')
        with tempfile.TemporaryDirectory() as tmp:
            for label, text in logs.items():
                with self.subTest(log=label), contextlib.redirect_stdout(io.StringIO()):
                    def executor(argv):
                        return {"exit_code": 0, "timed_out": False, "output_limited": False,
                                "elapsed_seconds": 0.001, "output": text}
                    result, = execute_checks([target], executor, Path(tmp))
                    expected = "PASS" if label.startswith("synthetic-") else "FAIL"
                    self.assertEqual(result["status"], expected)

    def test_paired_cli_rejects_incomplete_or_ambiguous_native_logs(self):
        pair = CliSyntheticPair(self)
        result, report = pair.cli()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIs(report["qualification_complete"], True)
        path = pair.directories["native"] / "m0-harness-sandbox.txt"
        for label, text in native_log_mutations().items():
            with self.subTest(log=label):
                path.write_text(text)
                result, report = pair.cli()
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIs(report["passed"], False)
                self.assertIs(report["qualification_complete"], False)
                self.assertIn("result validator", result.stdout)
                self.assertNotIn("Traceback", result.stderr)


class MetricsResultValidationTests(unittest.TestCase):
    def test_paired_cli_requires_strict_metrics_and_positive_integer_coverage(self):
        pair = CliSyntheticPair(self)
        path = pair.directories["portable"] / "maintainability.txt"
        valid = SYNTHETIC_LOGS["metrics"]
        mutations = {
            "duplicate-complete": valid.replace('"complete":true', '"complete":false,"complete":true'),
            "extra-NaN": valid[:-1] + ',"extra":NaN}',
            "depth-2000": valid[:-1] + ',"extra":' + '[' * 2000 + '0' + ']' * 2000 + '}',
            "depth-65": valid[:-1] + ',"extra":' + '[' * 65 + '0' + ']' * 65 + '}',
            "invalid-syntax": valid + '{}',
            "unpaired-surrogate": valid[:-1] + ',"extra":"\\ud800"}',
            "invalid-utf8": valid.encode() + b'\xff',
        }
        for value in ("true", "false", "0.5", "Infinity", "-Infinity", "1e10000",
                      "0", "-1", "1.0", '"1"', "null"):
            mutations[f"count-{value}"] = valid.replace('"files_analyzed":1', '"files_analyzed":' + value)
        result, report = pair.cli()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIs(report["qualification_complete"], True)
        for label, text in mutations.items():
            with self.subTest(log=label):
                path.write_bytes(text if isinstance(text, bytes) else text.encode())
                result, report = pair.cli()
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIs(report["passed"], False)
                self.assertIs(report["qualification_complete"], False)
                self.assertEqual(report["checks"][0]["name"], "qualification")
                self.assertNotIn("Traceback", result.stderr)

    def test_records_and_metrics_share_json_structure_and_unicode_rejection(self):
        from verification.qualification import read_json
        valid = SYNTHETIC_LOGS["metrics"]
        malformed_extras = ('"x":1,"x":2', '"x":NaN', '"x":Infinity',
                            '"x":1e10000', '"x":"\\ud800"', '"\\udfff":1',
                            '"x":' + '[' * 65 + '0' + ']' * 65,
                            '"x":' + '[' * 2000 + '0' + ']' * 2000, '"x":')
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "record.json"
            for index, extra in enumerate(malformed_extras):
                text = valid[:-1] + ',' + extra + '}'
                path.write_text(text)
                with self.subTest(extra=index, reader="record"), self.assertRaises(ValueError):
                    read_json(path)
                with self.subTest(extra=index, reader="metrics"):
                    self.assertFalse(validate("metrics", text))
            for extra in ('"text":"\\ud83d\\ude00"', '"x":' + '[' * 63 + '0' + ']' * 63):
                text = valid[:-1] + ',' + extra + '}'
                path.write_text(text)
                self.assertTrue(validate("metrics", text))
                self.assertIs(read_json(path)["coverage"]["complete"], True)

    def test_metrics_log_keeps_its_four_mib_limit_not_the_record_limit(self):
        pair = CliSyntheticPair(self)
        path = pair.directories["portable"] / "maintainability.txt"
        valid = SYNTHETIC_LOGS["metrics"].encode()
        path.write_bytes(valid + b" " * (4 * 1024 * 1024 - len(valid)))
        result, report = pair.cli()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIs(report["qualification_complete"], True)
        with path.open("ab") as stream:
            stream.write(b" ")
        result, report = pair.cli()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIs(report["qualification_complete"], False)
        self.assertIn("bounded regular file", result.stdout)


if __name__ == "__main__":
    unittest.main()
