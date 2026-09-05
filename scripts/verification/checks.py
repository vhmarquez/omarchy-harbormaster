"""Required check definitions and fail-closed result interpretation."""
import json
import re


def plan(outside_canary="/outside-canary"):
    """One command inventory, shared by local and hosted isolation backends."""
    python = ["/usr/bin/python3", "-B"]
    unit = python + ["-m", "unittest", "discover"]
    cargo = ["/tools/rust/bin/cargo"]
    native = python + ["scripts/verification/native.py"]
    return [
        ("isolation-probe", python + ["scripts/verification/probe.py", outside_canary], "exit"),
        ("tool-pins", python + ["scripts/verification/preflight.py"], "exit"),
        ("native-tool-pins", native + ["versions"], "exit"),
        ("rust-format", cargo + ["fmt", "--all", "--", "--check"], "exit"),
        ("rust-clippy", cargo + ["clippy", "--locked", "--offline", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"], "exit"),
        ("rust-unit", cargo + ["test", "--locked", "--offline", "--workspace", "--lib", "--all-features"], "rust"),
        ("rust-integration", cargo + ["test", "--locked", "--offline", "--workspace", "--test", "cli", "--all-features"], "rust"),
        ("qml-lint", native + ["lint"], "exit"),
        ("qml-format", native + ["format"], "exit"),
        ("qml-native", ["/usr/lib/qt6/bin/qmltestrunner", "-input", "qml/tests", "-maxwarnings", "0"], "qml"),
        ("python-verification", unit + ["-s", "tests/verification", "-v"], "python"),
        ("python-tooling", unit + ["-s", "tests/tooling", "-v"], "python"),
        ("m0-contract-tests", unit + ["-s", "tests", "-v"], "python"),
        ("m0-fixture-contract", python + ["scripts/verify-m0.py"], "exit"),
        ("m0-runtime-identity", unit + ["-s", "spikes/runtime", "-p", "test_identity.py", "-v"], "python"),
        ("m0-runtime-cleanup", unit + ["-s", "spikes/runtime", "-p", "test_cleanup.py", "-v"], "python"),
        ("m0-harness-helpers", unit + ["-s", "spikes/harnesses", "-p", "test_probe.py", "-v"], "python"),
        ("m0-design-model", ["node", "--test", "--test-reporter=tap", "docs/design/tests/study.test.cjs"], "node"),
        ("frozen-design-integrity", python + ["docs/design/tests/verify_bundle.py"], "exit"),
        ("advisory-license", ["/tools/bin/cargo-deny", "--config", "tools/deny.toml", "--frozen", "--workspace", "check", "--deny", "warnings", "all"], "exit"),
        ("maintainability", python + ["scripts/verification/maintainability.py", "."], "metrics"),
    ]


def passed(results):
    required = [item for item in results if item["required"]]
    return bool(required) and all(item["status"] == "PASS" for item in required)


def validate(kind, output):
    if kind == "python":
        count = re.search(r"Ran (\d+) tests?", output)
        return bool(count and int(count[1]) > 0 and "\nOK" in output
                    and "skipped=" not in output)
    if kind == "rust":
        rows = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        return bool(rows and sum(int(row[0]) for row in rows) > 0
                    and all(row[1:] == ("0", "0") for row in rows))
    if kind == "qml":
        rows = re.findall(r"Totals: (\d+) passed, (\d+) failed, (\d+) skipped, (\d+) blacklisted", output)
        executed = re.search(r"^PASS[ \t]+:[ \t]+[^\r\n()]+::test_\w*\([^\r\n]*\)[ \t]*$",
                             output, re.MULTILINE)
        return bool(rows and executed and sum(int(row[0]) for row in rows) > 0
                    and all(row[1:] == ("0", "0", "0") for row in rows))
    if kind == "node":
        fields = dict(re.findall(r"^# (tests|fail|cancelled|skipped) (\d+)$", output, re.MULTILINE))
        return (int(fields.get("tests", "0")) > 0
                and all(fields.get(key) == "0" for key in ("fail", "cancelled", "skipped")))
    if kind == "metrics":
        try:
            coverage = json.loads(output)["coverage"]
            return coverage["complete"] is True and coverage["files_analyzed"] > 0
        except (ValueError, KeyError, TypeError):
            return False
    return kind == "exit"
