"""Required check definitions and fail-closed result interpretation."""
import re

from .strict_json import loads


NATIVE_METHODS = (
    "Helpers.test_sandbox_denies_network_and_inherited_credentials",
    "NetworkNamespace.test_sandbox_uses_distinct_network_namespace",
)


def plan(outside_canary="/outside-canary", scope="all"):
    """Required check inventories selected by explicit execution scope."""
    if scope not in ("all", "portable", "native"):
        raise ValueError(f"unknown verification scope: {scope!r}")
    python = ["/usr/bin/python3", "-B"]
    unit = python + ["-m", "unittest", "discover"]
    cargo = ["/tools/rust/bin/cargo"]
    native = python + ["scripts/verification/native.py"]
    preflights = [
        ("isolation-probe", python + ["scripts/verification/probe.py", outside_canary], "exit"),
        ("tool-pins", python + ["scripts/verification/preflight.py"], "exit"),
    ]
    portable = preflights + [
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
    native_sandbox = (
        "m0-harness-sandbox",
        python + ["spikes/harnesses/test_sandbox.py", *NATIVE_METHODS, "-v"],
        "native",
    )
    if scope == "native":
        return preflights + [native_sandbox]
    return portable if scope == "portable" else portable + [native_sandbox]


def passed(results):
    required = [item for item in results if item["required"]]
    return bool(required) and all(item["status"] == "PASS" for item in required)


def _native_passed(output):
    """Match the selected verbose results and one final, skip-free summary."""
    results = []
    for target in NATIVE_METHODS:
        method = target.rsplit(".", 1)[1]
        heading = re.escape(f"{method} (__main__.{target}) ... ")
        # The real namespace diagnostic is flushed inline after the heading;
        # unittest writes its successful result on the following line.
        results.append(heading + r"(?:network namespace identity: \{[^\r\n]*\}\n)?ok\n")
    summary = rf"\n-+\nRan {len(NATIVE_METHODS)} tests in [0-9]+\.[0-9]+s\n\nOK\n?"
    return re.fullmatch("".join(results) + summary, output) is not None


def validate(kind, output):
    if kind == "native":
        return _native_passed(output)
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
            coverage = loads(output)["coverage"]
            count = coverage["files_analyzed"]
            return coverage["complete"] is True and type(count) is int and count > 0
        except (ValueError, KeyError, TypeError):
            return False
    return kind == "exit"
