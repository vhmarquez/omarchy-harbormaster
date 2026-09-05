"""Read-only native QML format/lint adapter, executed inside isolation."""
from pathlib import Path
import json
import subprocess
import sys

BIN = Path("/usr/lib/qt6/bin")


def verify(mode, paths):
    if not paths or mode not in {"lint", "format"}:
        raise ValueError("QML verification requires an explicit nonempty suite")
    passed = True
    for path in paths:
        if mode == "lint":
            argv = [str(BIN / "qmllint"), "--ignore-settings", "--bare", "-I",
                    "/usr/lib/qt6/qml", "--max-warnings", "0", str(path)]
        else:
            argv = [str(BIN / "qmlformat"), "--ignore-settings", "--normalize", str(path)]
        result = subprocess.run(argv, capture_output=True, text=True, timeout=30, check=False)
        canonical = mode == "lint" or result.stdout == path.read_text(encoding="utf-8")
        ok = result.returncode == 0 and canonical and not result.stderr.strip()
        print(f"{'PASS' if ok else 'FAIL'} {mode}: {path}")
        if result.stderr:
            print(result.stderr)
        passed = passed and ok
    return passed


def versions(expected):
    actual = {"python": ".".join(str(part) for part in sys.version_info[:3])}
    for name in ("qmllint", "qmlformat"):
        result = subprocess.run([str(BIN / name), "--version"], capture_output=True,
                                text=True, timeout=15, check=True)
        actual[name] = result.stdout.strip().split()[-1]
    print(json.dumps(actual, sort_keys=True))
    return (actual["python"] == expected["python"]
            and actual["qmllint"] == actual["qmlformat"] == expected["qt"])


if __name__ == "__main__":
    try:
        if sys.argv[1] == "versions":
            expected = json.loads(Path("tools/platform-lock.json").read_text())["tool_versions"]
            success = versions(expected)
        else:
            success = verify(sys.argv[1], sorted(Path("qml").rglob("*.qml")))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL QML tooling: {type(error).__name__}")
        success = False
    raise SystemExit(0 if success else 1)
