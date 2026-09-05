"""Offline M0 harness probe. Not a production installer or control adapter."""
from pathlib import Path
from contextlib import contextmanager
import json


@contextmanager
def hook_overlay(path, additions):
    """Fixture-only merge with byte-exact rollback; not a real-profile installer."""
    if path.is_symlink():
        raise ValueError("fixture path must not be a symlink")
    original = path.read_bytes() if path.exists() else None
    data = json.loads(original) if original is not None else {}
    hooks = data.setdefault("hooks", {})
    for event, entries in additions.items():
        hooks.setdefault(event, []).extend(entries)
    installed = (json.dumps(data, indent=2) + "\n").encode()
    path.write_bytes(installed)
    try:
        yield
    finally:
        if path.is_symlink() or not path.is_file() or path.read_bytes() != installed:
            raise RuntimeError("fixture changed concurrently; refusing destructive rollback")
        if original is None:
            path.unlink()
        else:
            path.write_bytes(original)



def sandbox(root: Path, mounts: dict[str, str]) -> list[str]:
    """Private network/PIDs/devices/home; installed artifacts are read-only."""
    (root / "home").mkdir(exist_ok=True)
    env = {
        "HOME": "/probe/home", "PATH": "/usr/bin:/bin", "TERM": "dumb",
        "LANG": "C.UTF-8", "HERMES_HOME": "/probe/home/.hermes",
        "CLAUDE_CONFIG_DIR": "/probe/home/.claude", "CODEX_HOME": "/probe/home/.codex",
        "XDG_CONFIG_HOME": "/probe/home/.config", "XDG_CACHE_HOME": "/probe/home/.cache",
        "XDG_DATA_HOME": "/probe/home/.local/share", "PYTHONDONTWRITEBYTECODE": "1",
        "DISABLE_AUTOUPDATER": "1", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
    }
    cmd = ["/usr/bin/bwrap", "--unshare-net", "--unshare-pid", "--die-with-parent",
           "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin", "/bin",
           "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
           "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
           "--dir", "/home", "--dir", "/run", "--bind", str(root), "/probe",
           "--clearenv", "--chdir", "/probe"]
    for source, target in mounts.items():
        cmd.extend(["--ro-bind", str(Path(source).resolve()), target])
    for key, value in env.items():
        cmd.extend(["--setenv", key, value])
    return cmd
