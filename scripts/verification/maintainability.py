"""Advisory, deterministic maintainability metrics for handwritten M1 code."""
import argparse
import ast
import json
import os
import re
import stat
from contextlib import contextmanager
from pathlib import Path

if __package__:
    from .metrics_lexical import lexical_functions
elif __name__ == "__main__":
    from metrics_lexical import lexical_functions
else:
    from scripts.verification.metrics_lexical import lexical_functions


@contextmanager
def directory(name, parent=None):
    fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
    try:
        yield fd
    finally:
        os.close(fd)


@contextmanager
def repository(root):
    """Reject symlinks in every root component, without resolving targets."""
    path = root.absolute()
    if ".." in path.parts:
        raise ValueError("parent traversal in root")
    fd = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = child
        yield fd
    finally:
        os.close(fd)


EXCLUDED = {
    "frozen", "design", "spikes", "generated", "vendor", "target",
    ".tools", ".git", "__pycache__",
}
LANGUAGES = {".py": "python", ".rs": "rust", ".qml": "qml"}
LEGACY = {"scripts/verify-m0.py", "tests/test_m0_contracts.py"}
LIMITATIONS = [
    "Path-level test partition: inline Rust test modules remain in production totals "
    "(conservative).",
    "Rust/QML lexical approximations cover named fn/function bodies, not full grammar or "
    "validity.",
    "No macro expansion, cfg evaluation, closures, arrow functions, QML handlers/bindings, "
    "or anonymous functions.",
    "Lexical names start with ASCII letters/underscore; Rust raw identifiers are "
    "supported; module/impl names are not qualified.",
    "Exotic const-generic signature braces and QML regex literals may distort lexical "
    "spans; template literals are opaque, including interpolation.",
    "Python AST-exact spans/node counts use the executing Python grammar; lambdas are not "
    "listed separately.",
    "Generated detection is directory-based plus recognized comment headers in the first "
    "five lines; unmarked generated files may be included.",
    "UTF-8 source only; POSIX no-follow directory-descriptor traversal rejects root "
    "ancestor symlinks. No concurrent-tree snapshot guarantee.",
    "Size/control counts are review signals, not cyclomatic complexity, coupling, "
    "duplication, API, or dead-code analysis.",
]
DEFINITIONS = {
    "nonblank_lines": (
        "Physical nonblank source lines, including comments/docstrings. Function spans "
        "include nested definitions; Python decorators included, Rust attributes "
        "excluded."
    ),
    "branches": (
        "Python: If/For/AsyncFor/While/ExceptHandler/IfExp/match_case/comprehension nodes "
        "plus BoolOp operands minus one. Lexical: "
        "if/for/while/loop/match/switch/case/catch and &&/||/? tokens. Nested function "
        "bodies excluded."
    ),
    "nesting": (
        "Python: maximum selected branch-node plus Try/TryStar/With/AsyncWith/Match "
        "depth. Lexical: maximum brace depth inside function (object literals count). "
        "Nested function bodies excluded."
    ),
}


def classification(path):
    parts = path.parts
    language = LANGUAGES.get(path.suffix)
    test = (
        any(p in {"test", "tests"} for p in parts[:-1])
        or path.stem in {"test", "tests"}
        or path.stem.startswith(("test_", "tst_"))
        or path.stem.endswith(("_test", "_tests"))
    )
    eligible = (
        (parts[0] == "crates" and language == "rust" and ("src" in parts or test))
        or (parts[0] == "qml" and language == "qml")
        or (parts[0] in {"scripts", "tests"} and language == "python")
    )
    if eligible and path.as_posix() not in LEGACY:
        return ("tests" if test else "production", language)
    return None


def source(parent, name):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
    with os.fdopen(fd, encoding="utf-8") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise OSError("not a regular source file")
        return stream.read()


def nonblank(text):
    return sum(bool(line.strip()) for line in text.splitlines())


PY_FUNCTIONS = (ast.FunctionDef, ast.AsyncFunctionDef)
PY_SCOPES = PY_FUNCTIONS + (ast.ClassDef, ast.Lambda)
PY_BRANCHES = (
    ast.If, ast.For, ast.AsyncFor, ast.While, ast.ExceptHandler,
    ast.IfExp, ast.match_case, ast.comprehension,
)
PY_BLOCKS = PY_BRANCHES + (ast.Try, ast.TryStar, ast.With, ast.AsyncWith, ast.Match)


def python_control(node, depth=0):
    if isinstance(node, PY_SCOPES):
        return 0, depth
    branches = int(isinstance(node, PY_BRANCHES))
    if isinstance(node, ast.BoolOp):
        branches += len(node.values) - 1
    depth += isinstance(node, PY_BLOCKS)
    maximum = depth
    for child in ast.iter_child_nodes(node):
        count, nesting = python_control(child, depth)
        branches += count
        maximum = max(maximum, nesting)
    return branches, maximum


def python_functions(text):
    lines = text.splitlines()
    found = []

    def visit(node, prefix=""):
        name = prefix
        if isinstance(node, PY_FUNCTIONS + (ast.ClassDef,)):
            name = prefix + node.name + "."
        if isinstance(node, PY_FUNCTIONS):
            start = min([node.lineno] + [d.lineno for d in node.decorator_list])
            controls = [python_control(child) for child in node.body]
            found.append({
                "name": name[:-1], "line": start, "end_line": node.end_lineno,
                "nonblank_lines": nonblank("\n".join(lines[start - 1:node.end_lineno])),
                "branches": sum(c[0] for c in controls),
                "nesting": max((c[1] for c in controls), default=0),
            })
        for child in ast.iter_child_nodes(node):
            visit(child, name)

    visit(ast.parse(text))
    return sorted(found, key=lambda f: (f["line"], f["name"]))


def file_metrics(text, path, language):
    error = None
    try:
        functions = (
            python_functions(text) if language == "python"
            else lexical_functions(text, language)
        )
    except (SyntaxError, ValueError, RecursionError) as exc:
        functions = []
        error = type(exc).__name__ + " during source analysis"
    return {
        "path": path.as_posix(), "language": language,
        "nonblank_lines": nonblank(text), "functions": functions,
        "analysis_error": error,
        "metric_kind": "python-ast-exact" if language == "python" else "lexical-approximate",
    }


def hotspots(file):
    found = []
    if file["nonblank_lines"] > 300:
        found.append({
            "path": file["path"], "kind": "file",
            "nonblank_lines": file["nonblank_lines"], "disposition": "review required",
        })
    for function in file["functions"]:
        if function["nonblank_lines"] > 50:
            found.append({
                "path": file["path"], "kind": "function", **function,
                "disposition": "review required",
            })
    return found


def issue(result, path, reason, *, error=False):
    coverage = result["coverage"]
    coverage["errors" if error else "exclusions"].append({
        "path": path.as_posix(), "reason": reason,
    })
    if error or reason == "symlink not followed":
        coverage["complete"] = False


def generated_header(text):
    header = "\n".join(text.splitlines()[:5])
    pattern = r"(?im)^\s*(?:#|//|/\*|\*)\s*(?:@generated|auto[- ]generated|code generated\b)"
    return re.search(pattern, header) is not None


def collect(parent, relative, result):
    for name in sorted(os.listdir(parent)):
        rel = relative / name
        try:
            mode = os.stat(name, dir_fd=parent, follow_symlinks=False).st_mode
            if stat.S_ISLNK(mode):
                issue(result, rel, "symlink not followed")
            elif name in EXCLUDED:
                issue(result, rel, "excluded tree")
            elif relative == Path(".") and name not in {"crates", "qml", "scripts", "tests"}:
                issue(result, rel, "outside new-code scope")
            elif stat.S_ISDIR(mode):
                with directory(name, parent) as child:
                    collect(child, rel, result)
            elif category := classification(rel):
                if not stat.S_ISREG(mode):
                    issue(result, rel, "not a regular source file", error=True)
                    continue
                section, language = category
                text = source(parent, name)
                if generated_header(text):
                    issue(result, rel, "generated header")
                    continue
                file = file_metrics(text, rel, language)
                result[section]["files"].append(file)
                if file["analysis_error"]:
                    issue(result, rel, file["analysis_error"], error=True)
            else:
                issue(result, rel, "outside new-code scope")
        except OSError:
            issue(result, rel, "unreadable entry or changed during scan", error=True)
        except UnicodeError:
            issue(result, rel, "source is not UTF-8", error=True)


def report(root: Path) -> dict:
    """Return review signals, never an automatic size-based failure."""
    result = {
        "schema_version": 1,
        "thresholds": {
            "file_nonblank": 300, "function_nonblank": 50,
            "policy": "review only; strictly greater than threshold",
        },
        "production": {"files": [], "hotspots": []},
        "tests": {
            "files": [], "hotspots": [],
            "rationale": "Tests are separate; large tests/fixtures need reviewer rationale.",
        },
        "coverage": {
            "files_analyzed": 0, "files_listed": 0, "complete": True,
            "exclusions": [], "errors": [],
            "excluded_directory_names": sorted(EXCLUDED),
            "legacy_excluded_files": sorted(LEGACY),
            "limitations": list(LIMITATIONS),
            "metric_definitions": dict(DEFINITIONS),
            "scope": (
                "crates/**/src/**/*.rs; qml/**/*.qml; scripts/**/*.py; "
                "tests/**/*.py and Rust test files. Legacy M0 excluded; no git-diff dependency."
            ),
            "by_language": {language: 0 for language in LANGUAGES.values()},
        },
    }
    try:
        with repository(Path(root)) as parent:
            collect(parent, Path("."), result)
    except (OSError, ValueError):
        issue(result, Path("."),
              "root unavailable, symlinked, or contains parent traversal", error=True)
    for section in ("production", "tests"):
        result[section]["files"].sort(key=lambda f: f["path"])
        for file in result[section]["files"]:
            result[section]["hotspots"].extend(hotspots(file))
            result["coverage"]["files_listed"] += 1
            if not file["analysis_error"]:
                result["coverage"]["files_analyzed"] += 1
                result["coverage"]["by_language"][file["language"]] += 1
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", type=Path, default=Path.cwd())
    print(json.dumps(report(parser.parse_args().root), indent=2, sort_keys=True))
