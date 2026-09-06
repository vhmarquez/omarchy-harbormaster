"""Private filesystem and process boundaries for standalone tool installation."""
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import contextlib
import fcntl
import json
import os
import signal
import stat
import subprocess
import tempfile

MARKER = ".harbormaster-tools.json"
IDENTITY = {"owner": "harbormaster-m1-tool-preparation", "schema": 1}


def private_root(root):
    root = Path(root).absolute()
    no_symlinks(root)
    if not root.exists():
        root.mkdir(mode=0o700)
        with (root / MARKER).open("x") as marker:
            json.dump(IDENTITY, marker)
        (root / MARKER).chmod(0o600)
    info = root.stat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError(f"tools root must be owned by current user and private (0700): {root}")
    marker = root / MARKER
    no_symlinks(marker)
    if not marker.is_file() or marker.stat().st_uid != os.getuid() or marker.stat().st_mode & 0o077:
        raise ValueError(f"missing/private ownership marker: {marker}; choose a NEW tools directory")
    if json.loads(marker.read_text()) != IDENTITY:
        raise ValueError(f"unrecognized ownership marker: {marker}")
    return root


@contextlib.contextmanager
def owned_stage(root, name):
    if name not in {"rust", "bin", "advisory-db", "node", "dependencies"}:
        raise ValueError("unsupported tool installation name")
    root = private_root(root)
    with (root / MARKER).open("r") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        target = root / name
        if target.exists() or target.is_symlink():
            raise ValueError(f"installation already exists: {target}; verify it or choose a NEW tools root")
        with tempfile.TemporaryDirectory(prefix=".prepare-", dir=root) as temporary:
            stage = Path(temporary)
            yield stage
            no_symlinks(stage / name)
            (stage / name).rename(target)


def clean_env(home):
    return {"PATH": "/usr/bin", "HOME": str(home), "TMPDIR": str(home),
            "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "CARGO_HOME": str(home / "cargo"),
            "CARGO_NET_OFFLINE": "true", "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_TERMINAL_PROMPT": "0",
            "GIT_CONFIG_COUNT": "3", "GIT_CONFIG_KEY_0": "credential.helper",
            "GIT_CONFIG_VALUE_0": "", "GIT_CONFIG_KEY_1": "core.hooksPath",
            "GIT_CONFIG_VALUE_1": "/dev/null", "GIT_CONFIG_KEY_2": "init.templateDir",
            "GIT_CONFIG_VALUE_2": ""}


def run(argv, home, *, timeout=180, extra_env=None, cwd=None):
    home = Path(home)
    no_symlinks(home)
    home.mkdir(mode=0o700, exist_ok=True)
    env = clean_env(home)
    env.update(extra_env or {})
    with tempfile.TemporaryFile(dir=home) as output:
        with subprocess.Popen(argv, env=env, cwd=cwd or home, stdin=subprocess.DEVNULL,
                              stdout=output, stderr=subprocess.STDOUT, start_new_session=True) as child:
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired as error:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise RuntimeError(f"command timed out after {timeout}s: {argv[0]}") from error
            output.seek(0)
            text = output.read(1_000_001).decode(errors="replace")
            if child.returncode or len(text) > 1_000_000:
                raise RuntimeError(f"command failed ({child.returncode}): {argv!r}\n{text[:1_000_000]}")
    return text.strip()


def no_symlinks(path):
    path = Path(path).absolute()
    if ".." in path.parts:
        raise ValueError(f"parent traversal in destination: {path}")
    for ancestor in (path, *path.parents):
        if ancestor.is_symlink():
            raise ValueError(f"symlink in destination: {ancestor}")


def extract(archive, destination):
    destination = Path(destination)
    no_symlinks(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError(f"extraction destination already exists: {destination}")
    try:
        with tarfile.open(archive) as source:
            members = source.getmembers()
            if len(members) > 100_000 or sum(m.size for m in members) > 3_000_000_000:
                raise ValueError("unsafe archive: expansion limit exceeded")
            for member in members:
                name = PurePosixPath(member.name)
                if name.is_absolute() or ".." in name.parts:
                    raise ValueError(f"unsafe archive path: {name}")
                try:
                    tarfile.data_filter(member, str(destination))
                except tarfile.FilterError as error:
                    raise ValueError(f"unsafe archive: {error}") from error
            destination.mkdir(mode=0o700)
            source.extractall(destination, members=members, filter="data")
    except BaseException:
        if destination.exists():
            shutil.rmtree(destination)
        raise
    return destination
