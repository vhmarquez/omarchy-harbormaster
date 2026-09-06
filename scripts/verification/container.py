"""Hosted CI container boundary; no Docker daemon changes or live mounts."""
import os
import re
import subprocess


def command(name, source, tools, state, image, environment):
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", image or ""):
        raise ValueError("Docker verification requires a built immutable image ID")
    argv = ["/usr/bin/docker", "run", "--rm", "--name", name, "--pull", "never",
            "--network", "none", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            "--read-only", "--pids-limit", "256", "--memory", "4g", "--cpus", "2",
            "--user", f"{os.getuid()}:{os.getgid()}", "--workdir", "/work",
            "--tmpfs", "/tmp:rw,nosuid,nodev,size=256m",
            "--tmpfs", "/run:ro,nosuid,nodev,noexec,size=16m,mode=755"]
    for host, guest, readonly in ((source, "/work", True), (tools, "/tools", True),
                                   (state, "/state", False)):
        if "," in str(host):
            raise ValueError("Docker bind paths cannot contain commas")
        mount = f"type=bind,src={host},dst={guest}" + (",readonly" if readonly else "")
        argv += ["--mount", mount]
    for key, value in environment.items():
        argv += ["--env", f"{key}={value}"]
    return argv + [image]


def cleanup(name):
    """An exact random owned name only; bound cleanup even after interrupted run."""
    result = subprocess.run(["/usr/bin/docker", "rm", "--force", name],
                            env={"PATH": "/usr/bin"}, stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=10, check=False)
    if result.returncode and b"No such container" not in result.stderr:
        raise OSError("verification container cleanup failed")
