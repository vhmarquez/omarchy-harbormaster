"""Offline Linux verification boundary, not an agent execution sandbox."""
import os
from pathlib import Path
import resource
import selectors
import shutil
import signal
import stat
import subprocess
import tempfile
import time
import uuid

from . import container

SOURCES = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "clippy.toml",
    "README.md", "LICENSE", "crates", "qml", "scripts", "tests", "tools",
    "contracts", "docs", "spikes", ".github",
)
EXCLUDED = {".git", ".tools", ".verify", ".venv", "__pycache__", "target"}


def snapshot(root, destination):
    """Copy reviewed source roots only; never follow source symlinks.

    Verification assumes a trusted, quiescent checkout. This is not a race-free
    scanner of a hostile concurrently mutated repository.
    """
    destination.mkdir(mode=0o700)
    for name in SOURCES:
        source = root / name
        if source.exists() or source.is_symlink():
            _copy_source(source, destination / name)


def _copy_source(source, destination):
    if source.name in EXCLUDED or source.name.startswith(".env"):
        return
    mode = source.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise ValueError("symlink in verification source")
    if stat.S_ISDIR(mode):
        destination.mkdir(mode=0o700)
        for child in sorted(source.iterdir()):
            _copy_source(child, destination / child.name)
    elif stat.S_ISREG(mode):
        if source.stat().st_size > 32 * 1024 * 1024:
            raise ValueError("verification source file exceeds 32 MiB")
        shutil.copyfile(source, destination)
    else:
        raise ValueError("non-regular verification source")


def _bubblewrap(source, tools, state):
    command = ["/usr/bin/bwrap", "--unshare-all", "--die-with-parent",
               "--new-session", "--cap-drop", "ALL", "--ro-bind", "/usr", "/usr"]
    for name in ("/bin", "/lib", "/lib64"):
        path = Path(name)
        if path.is_symlink():
            command += ["--symlink", os.readlink(path), name]
        elif path.exists():
            command += ["--ro-bind", name, name]
    for name in ("/etc/ld.so.cache", "/etc/fonts"):
        if Path(name).exists():
            command += ["--ro-bind", name, name]
    command += ["--ro-bind", str(source), "/work", "--ro-bind", str(tools), "/tools",
                "--bind", str(state), "/state", "--proc", "/proc", "--dev", "/dev",
                "--tmpfs", "/tmp", "--perms", "0700", "--size", "1048576", "--tmpfs", "/fault-fs",
                "--remount-ro", "/", "--chdir", "/work", "--clearenv"]
    for key, value in environment().items():
        command += ["--setenv", key, value]
    return command + ["--"]


def _limits():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (512 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_AS, (4 * 1024 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_CPU, (240, 240))


def _collect(process, timeout):
    deadline = time.monotonic() + timeout
    output = bytearray()
    timed_out = limited = False
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            if not selector.select(remaining):
                continue
            chunk = os.read(process.stdout.fileno(), 65536)
            if not chunk:
                break
            room = 4 * 1024 * 1024 - len(output)
            output.extend(chunk[:room])
            if len(chunk) > room:
                limited = True
                break
    if timed_out or limited:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=max(0.01, deadline - time.monotonic()))
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)
    process.stdout.close()
    return {"exit_code": process.returncode if not limited else 1,
            "output": output.decode("utf-8", errors="replace"),
            "timed_out": timed_out, "output_limited": limited}


def _execute(command, timeout):
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, close_fds=True,
                               env={"PATH": "/usr/bin", "LANG": "C.UTF-8"},
                               start_new_session=True, preexec_fn=_limits)
    return _collect(process, timeout)


def run(argv, source, tools, state, timeout, backend="bwrap", image=None):
    """Run one check offline, returning bounded output and explicit failure."""
    for name in ("home", "cargo", "target", "config", "cache", "data", "state", "runtime", "tmp"):
        (state / name).mkdir(mode=0o700, exist_ok=True)
    name = "harbormaster-verify-" + uuid.uuid4().hex
    if backend == "bwrap":
        command = _bubblewrap(source, tools, state) + argv
    elif backend == "docker":
        command = container.command(name, source, tools, state, image, environment()) + argv
    else:
        raise ValueError("unknown verification backend")
    start = time.monotonic()
    try:
        result = _execute(command, timeout)
    finally:
        if backend == "docker":
            container.cleanup(name)
    result["elapsed_seconds"] = round(time.monotonic() - start, 3)
    return result


def environment():
    """Build from scratch: never forward credentials or executable overrides."""
    return {
        "PATH": "/tools/rust/bin:/tools/bin:/tools/node/bin:/usr/bin",
        "HOME": "/state/home",
        "PWD": "/work",
        "CARGO_HOME": "/state/cargo",
        "CARGO_TARGET_DIR": "/state/target",
        "CARGO_NET_OFFLINE": "true",
        "SQLITE3_NO_PKG_CONFIG": "1",
        "SQLITE3_STATIC": "1",
        "SQLITE3_LIB_DIR": "/state/sqlite/lib",
        "SQLITE3_INCLUDE_DIR": "/state/sqlite/include",
        "XDG_CONFIG_HOME": "/state/config",
        "XDG_CACHE_HOME": "/state/cache",
        "XDG_DATA_HOME": "/state/data",
        "XDG_STATE_HOME": "/state/state",
        "XDG_RUNTIME_DIR": "/state/runtime",
        "TMPDIR": "/state/tmp",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "TZ": "UTC",
        "QT_QPA_PLATFORM": "offscreen",
        "QT_QUICK_BACKEND": "software",
        "QT_QPA_OFFSCREEN_NO_GLX": "1",
        "QML_DISABLE_DISK_CACHE": "1",
        "QT_IM_MODULE": "compose",
        "QT_ACCESSIBILITY": "0",
        "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "0",
        "DBUS_SESSION_BUS_ADDRESS": "unix:path=/state/runtime/no-session-bus",
        "DBUS_SYSTEM_BUS_ADDRESS": "unix:path=/state/runtime/no-system-bus",
        "PYTHONDONTWRITEBYTECODE": "1",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
    }
