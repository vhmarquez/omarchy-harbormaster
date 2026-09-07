#!/usr/bin/env python3
"""Opt-in #12 demonstration with temporary user services and a synthetic worker."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import time
import uuid

WORKER = '''#!/usr/bin/python3
import json,os,time,sys
from pathlib import Path
for tick in range(900):
    state = {"pid":os.getpid(), "tick":tick, "tty":os.isatty(0) and os.isatty(1),
             "argv":sys.argv[1:], "clean":all(k not in os.environ for k in
             ("OPENAI_API_KEY","HERMES_HOME","PYTHONPATH","TMUX"))}
    Path("heartbeat.tmp").write_text(json.dumps(state))
    Path("heartbeat.tmp").replace("heartbeat.json")
    time.sleep(0.1)
'''


def run(argv, env, check=True):
    result = subprocess.run([str(a) for a in argv], env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=8)
    if check and result.returncode:
        raise RuntimeError("disposable command failed: " + Path(str(argv[0])).name + ": " + result.stderr[:1024].decode(errors="replace"))
    return result


def wait_for(predicate, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise TimeoutError("disposable runtime deadline")


def metadata(paths):
    result = []
    for path in paths:
        try:
            s = path.lstat()
            result.append((s.st_dev, s.st_ino, s.st_mode, s.st_size, s.st_mtime_ns))
        except FileNotFoundError:
            result.append(None)
    return result


def identity(pid):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return fields[19] if fields[0] not in ("Z", "X", "x") else None
    except FileNotFoundError:
        return None


class Demo:
    def __init__(self, binary, root, session):
        self.binary, self.root = binary, root
        self.manager = "harbormaster-demo-manager-" + uuid.uuid4().hex + ".service"
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(root / "home"),
                    "XDG_STATE_HOME": str(root / "state"), "XDG_RUNTIME_DIR": str(session),
                    "HARBORMASTER_RUNTIME_DIR": str(root),
                    "OPENAI_API_KEY": "synthetic-demo-canary", "HERMES_HOME": "synthetic"}
        self.units, self.project = [self.manager], None
        self.phase = "setup"
        for name in ("home", "state", "project"):
            (root / name).mkdir(mode=0o700)
        (root / "hermes").write_text(WORKER)
        (root / "hermes").chmod(0o700)

    def cli(self, *args):
        result = run([self.binary, *args], self.env)
        assert b"synthetic-demo-canary" not in result.stdout + result.stderr
        return json.loads(result.stdout)

    def start_manager(self):
        argv = ["/usr/bin/systemd-run", "--user", "--quiet", "--collect",
                "--no-ask-password", "--service-type=exec", "--expand-environment=no",
                "--unit=" + self.manager, "--property=KillMode=control-group",
                "--property=TimeoutStopSec=5", "--property=RuntimeMaxSec=180",
                "--property=StandardOutput=null", "--property=StandardError=null",
                "--", "/usr/bin/env", "-i"]
        argv += [key + "=" + value for key, value in self.env.items()]
        run([*argv, self.binary, "manager", "serve"], self.env)
        wait_for(lambda: run([self.binary, "manager", "status"], self.env, False).returncode == 0)

    def property(self, unit, name):
        return run(["/usr/bin/systemctl", "--user", "show", unit, "--property=" + name,
                    "--value"], self.env).stdout.decode().strip()

    def stop(self, unit):
        run(["/usr/bin/systemctl", "--user", "stop", unit], self.env)
        wait_for(lambda: self.property(unit, "ActiveState") in ("inactive", "failed"))

    def heartbeat(self):
        try:
            return json.loads((self.root / "project/heartbeat.json").read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            return None

    def exercise(self):
        self.phase = "start manager and register"
        self.start_manager()
        project = self.cli("project", "add", self.root / "project", "Disposable runtime")["Project"]
        self.project = project["id"]
        self.cli("preset", "add", "demo", self.root / "hermes", "synthetic")
        task = self.cli("task", "add", self.project, "demo", "Metadata $(touch injected)")["Task"]
        self.phase = "launch runtime"
        launched = self.cli("run", "launch", task["id"], "--logout-policy", "existing")
        unit = "harbormaster-runtime-" + str(uuid.UUID(launched["run"]["id"])) + ".service"
        self.units.append(unit)
        self.phase = "observe runtime"
        before = wait_for(self.heartbeat)
        worker_identity = identity(before["pid"])
        current = self.cli("run", "list", self.project)["items"][0]
        assert current["state"] == "running" and current["limited_visibility"]
        assert before["tty"] and before["clean"] and before["argv"] == ["-p", "synthetic", "chat"]
        assert self.property(unit, "PartOf") == self.property(unit, "BindsTo") == ""
        assert self.property(unit, "ControlGroup") != self.property(self.manager, "ControlGroup")
        scope = Path(f"/proc/{before['pid']}/cgroup").read_text().strip().rsplit("/", 1)[-1]
        assert re.fullmatch(r"tmux-spawn-[0-9a-f-]+\.scope", scope)
        assert self.property(scope, "PartOf") == unit
        self.phase = "restart manager service"
        self.stop(self.manager)
        progressed = wait_for(lambda: (h := self.heartbeat()) and h["tick"] > before["tick"] and h)
        assert progressed["pid"] == before["pid"]
        self.start_manager()
        restored = self.cli("run", "launch", task["id"], "--logout-policy", "existing")
        assert restored["run"] == launched["run"] and restored["state"] == "running"
        assert len(self.cli("run", "list", self.project)["items"]) == 1
        assert not (self.root / "project/injected").exists()
        self.phase = "stop owned runtime"
        self.stop(unit)
        wait_for(lambda: identity(before["pid"]) != worker_identity)
        assert self.cli("run", "list", self.project)["items"][0]["state"] == "unavailable"
        assert self.cli("manager", "status")["manager"] == "running"
        return {"ipc_registration": True, "real_pty_worker": True, "filtered_environment": True,
                "independent_cgroups_and_pane_scope": True, "survived_manager_service_restart": True,
                "same_runner_on_repeat": True, "runtime_stop_preserved_manager": True}

    def observation(self):
        if not self.project:
            return []
        result = []
        for item in self.cli("run", "list", self.project)["items"]:
            run_id = str(uuid.UUID(item["run"]["id"]))
            directory = self.root / "harbormaster/runners" / run_id
            socket = directory / "tmux.sock"
            query = run(["/usr/bin/tmux", "-S", socket, "display-message", "-p", "-t", "=managed:",
                         "#{pid}:#{pane_dead}:#{pane_pid}"], self.env, False)
            result.append({"tmux_status": query.returncode, "tmux_metadata": query.stdout.decode().strip(),
                           "recorded_server_pid": item["run"]["server"]["pid"] if item["run"]["server"] else None,
                           "state": item["state"], "server_recorded": item["run"]["server"] is not None,
                           "socket_mode": oct(stat.S_IMODE(socket.lstat().st_mode)) if socket.exists() else None,
                           "handoff_present": (directory / "launch.json").exists(),
                           "heartbeat_present": bool(self.heartbeat())})
        return result

    def cleanup(self):
        errors = []
        if self.project:
            try:
                for item in self.cli("run", "list", self.project)["items"]:
                    unit = "harbormaster-runtime-" + str(uuid.UUID(item["run"]["id"])) + ".service"
                    if unit not in self.units:
                        self.units.append(unit)
            except (RuntimeError, OSError, ValueError, subprocess.SubprocessError):
                errors.append("runner discovery")
        for unit in reversed(self.units):
            try:
                run(["/usr/bin/systemctl", "--user", "stop", unit], self.env, False)
                wait_for(lambda: self.property(unit, "ActiveState") in ("inactive", "failed"))
            except (RuntimeError, OSError, ValueError, subprocess.SubprocessError, TimeoutError):
                errors.append("owned unit cleanup")
        if not errors:
            shutil.rmtree(self.root)
        return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-user-services", action="store_true", required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error("binary must be executable")
    session = Path(os.environ["XDG_RUNTIME_DIR"])
    info = session.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        parser.error("unsafe user runtime directory")
    args.output.mkdir(mode=0o700)
    root = session / ("h12-" + uuid.uuid4().hex[:8])
    root.mkdir(mode=0o700)
    paths = [Path.home() / ".tmux.conf", Path.home() / ".config/tmux/tmux.conf",
             Path(f"/tmp/tmux-{os.getuid()}/default")]
    original = metadata(paths)
    report = {"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "checks": {}, "owned_root": str(root)}
    demo = Demo(binary, root, session)
    completed = False
    try:
        report["checks"] = demo.exercise()
        completed = True
    except (RuntimeError, OSError, ValueError, AssertionError, subprocess.SubprocessError, TimeoutError) as error:
        report["error"] = {"type": type(error).__name__, "phase": demo.phase, "detail": str(error)[:1024]}
        try:
            report["runtime_observation"] = demo.observation()
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError):
            report["runtime_observation"] = "unavailable"
    finally:
        report["cleanup_errors"] = demo.cleanup()
        report["checks"]["default_tmux_metadata_unchanged"] = metadata(paths) == original
        report["passed"] = completed and not report.get("error") and not report["cleanup_errors"] and all(report["checks"].values())
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
