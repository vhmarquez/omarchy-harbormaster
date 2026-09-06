"""Run the fixed verification inventory and keep explicit bounded evidence."""
from .checks import validate


def execute_checks(checks, executor, output):
    results = []
    blocked_by = None
    for name, argv, kind in checks:
        if blocked_by:
            results.append({"name": name, "required": True, "command": argv,
                            "status": "NOT_RUN", "reason": f"failed preflight: {blocked_by}"})
            continue
        result = {"name": name, "required": True, "command": argv, "status": "FAIL"}
        try:
            execution = executor(argv)
            text = execution.pop("output")
            result.update(execution)
            (output / f"{name}.txt").write_text(text, encoding="utf-8")
            result["log"] = f"{name}.txt"
            if (result["exit_code"] == 0 and not result.get("timed_out", False)
                    and not result.get("output_limited", False) and validate(kind, text)):
                result["status"] = "PASS"
        except (OSError, ValueError) as error:
            result["error"] = type(error).__name__
        results.append(result)
        if name in {"isolation-probe", "tool-pins", "sqlite-build", "native-tool-pins"} and result["status"] != "PASS":
            blocked_by = name
        print(f"{result['status']:4} {name}", flush=True)
    return results
