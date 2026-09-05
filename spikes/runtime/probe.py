#!/usr/bin/env python3
"""M0 disposable feasibility fixture. NOT the Rust supervisor/QML product."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import uuid
from identity import exact_window

HERE = Path(__file__).resolve().parent
ENV = {k: v for k, v in os.environ.items() if k not in ("TMUX", "TMUX_PANE")}
TRACE = []
ROOT = None


def run(argv, check=True):
    args = [str(a) for a in argv]
    result = subprocess.run(args, capture_output=True, text=True, env=ENV, timeout=15)
    TRACE.append({"argv": args, "returncode": result.returncode})
    if check and result.returncode:
        raise RuntimeError(f"command failed: {args[0]} ({result.returncode})")
    return result


def wait_for(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise TimeoutError("bounded condition wait expired")


def proc(pid):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rpartition(")")[2].split()
        return {"pid": int(pid), "state": fields[0], "ppid": int(fields[1]),
                "start_ticks": int(fields[19]),
                "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
                "cgroup": Path(f"/proc/{pid}/cgroup").read_text().strip()}
    except (FileNotFoundError, ProcessLookupError):
        return None


def same_process(expected):
    actual = proc(expected["pid"])
    return bool(actual and actual["state"] not in ("Z", "X", "x")
                and all(actual[k] == expected[k] for k in ("pid", "start_ticks", "boot_id")))


def store(path, data):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(data))
    temporary.replace(path)


def service(unit, command, directory):
    return ["systemd-run", "--user", "--no-ask-password", "--quiet", "--collect",
            f"--unit={unit}", "--service-type=exec", "--expand-environment=no",
            "--property=KillMode=control-group", "--property=Restart=no",
            "--property=TimeoutStopSec=3", "--property=RuntimeMaxSec=180",
            "--property=UMask=0077", "--property=StandardOutput=null",
            "--property=StandardError=null", f"--working-directory={directory}",
            "--", *command]


def unit_info(unit):
    result = run(["systemctl", "--user", "show", unit, "--no-pager",
                  "-p", "MainPID", "-p", "ControlGroup", "-p", "ActiveState",
                  "-p", "KillMode", "-p", "PartOf", "-p", "BindsTo", "-p", "LoadState"],
                 check=False)
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


def tmux(directory, *args, check=True):
    return run(["tmux", "-S", str(directory / "tmux.sock"), *args], check=check)


def fixture_worker(directory):
    tick = 0
    while True:
        tick += 1
        store(directory / "heartbeat.json", {"pid": os.getpid(), "tick": tick})
        time.sleep(0.1)


def fixture_daemon(directory):
    cfg = json.loads((directory / "config.json").read_text())
    server_file = directory / "server.json"
    if not server_file.exists():
        run(service(cfg["runtime"], [shutil.which("tmux"), "-D", "-f", "/dev/null",
                                   "-S", str(directory / "tmux.sock")], directory))
        wait_for(lambda: (directory / "tmux.sock").exists())
        tmux(directory, "new-session", "-d", "-s", "managed", "-c", str(directory),
             sys.executable, "-B", str(HERE / "probe.py"), "fixture-worker", str(directory))
        runtime = unit_info(cfg["runtime"])
        identity = proc(int(runtime["MainPID"]))
        if not identity:
            raise RuntimeError("runtime has no process")
        store(server_file, identity)
    else:
        if not same_process(json.loads(server_file.read_text())):
            raise RuntimeError("refusing stale runtime; no implicit relaunch")
        tmux(directory, "has-session", "-t", "=managed")
    store(directory / "daemon-ready.json", {"pid": os.getpid(), "generation": uuid.uuid4().hex})
    with (directory / "daemon-commands.jsonl").open("a") as log:
        log.write(json.dumps(TRACE) + "\n")
    while True:
        time.sleep(60)


def default_metadata():
    # No default-server connection, no session queries or config content reads.
    paths = {Path(os.environ.get("TMUX_TMPDIR", "/tmp")) / f"tmux-{os.getuid()}" / "default",
             Path.home() / ".tmux.conf",
             Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "tmux/tmux.conf",
             Path("/etc/tmux.conf")}
    result = {}
    for path in paths:
        try:
            s = path.lstat()
            result[str(path)] = (s.st_ino, s.st_mode, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        except FileNotFoundError:
            result[str(path)] = None
    return result


def versions():
    commands = {"tmux": ["tmux", "-V"], "systemd": ["systemctl", "--version"],
                "foot": ["foot", "--version"], "omarchy": ["omarchy", "version"],
                "alacritty": ["alacritty", "--version"], "kitty": ["kitty", "--version"],
                "ghostty": ["ghostty", "--version"]}
    found: dict = {key: run(cmd).stdout.strip().splitlines()[0] if shutil.which(cmd[0]) else "not installed"
             for key, cmd in commands.items()}
    found.update(python=platform.python_version(), kernel=platform.release())
    if os.environ.get("HYPRLAND_INSTANCE_SIGNATURE") and shutil.which("hyprctl"):
        h = json.loads(run(["hyprctl", "-j", "version"]).stdout)
        found["hyprland"] = {key: h[key] for key in ("version", "commit")}
    return found


def windows(active=False):
    raw = json.loads(run(["hyprctl", "-j", "activewindow" if active else "clients"]).stdout)
    # Immediately discard titles/other windows' content. Never retain or export them.
    return [{key: item.get(key) for key in ("address", "pid", "class")}
            for item in ([raw] if active else raw)]


def window_identity(window):
    identity = proc(window["pid"])
    if not identity:
        raise RuntimeError("window process disappeared")
    return {**identity, "address": window["address"], "app_id": window["class"],
            "compositor": os.environ["HYPRLAND_INSTANCE_SIGNATURE"]}


def focus(expected):
    address = exact_window(expected, windows(), proc(expected["pid"]),
                           os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"))
    if address is None:
        return False
    run(["hyprctl", "dispatch", 'hl.dsp.focus({window="address:' + address + '"})'])
    wait_for(lambda: windows(active=True)[0]["address"] == address)
    return same_process(expected)


def attached(directory, terminal, session_id):
    rows = tmux(directory, "list-clients", "-F", "#{client_pid}\t#{session_id}\t#{client_tty}").stdout.splitlines()
    if len(rows) != 1:
        return False
    pid, target, tty = rows[0].split("\t")
    client = proc(int(pid))
    return bool(client and client["ppid"] == terminal.pid and target == session_id
                and os.readlink(f"/proc/{pid}/fd/0") == tty)


def launch_terminal(directory, app_id, terminals):
    # Exact preflight: never fall back to another session or user's default socket.
    tmux(directory, "has-session", "-t", "=managed")
    command = ["foot", "--config=/dev/null", "--log-level=none", "--log-no-syslog",
               "--app-id=" + app_id, "--title=Harbormaster disposable M0 probe",
               "tmux", "-S", str(directory / "tmux.sock"), "attach-session", "-t", "=managed"]
    child = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=ENV)
    terminals.append(child)  # Register before waiting so failure still cleans up.
    TRACE.append({"argv": command, "spawned_pid": child.pid})
    try:
        owned = wait_for(lambda: [w for w in windows() if w["pid"] == child.pid and w["class"] == app_id])
    except TimeoutError as exc:
        raise RuntimeError("terminal mapping failed; child_exit=" + str(child.poll())
                           + "; owned_windows=" + json.dumps([w for w in windows() if w["pid"] == child.pid])) from exc
    if len(owned) != 1:
        raise RuntimeError("ambiguous disposable terminal")
    return child, window_identity(owned[0])


def close_terminal(child):
    # Popen is this invocation's unreaped child: never signal a discovered PID.
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=3)


def probe(args):
    global ROOT
    data = {"schema": 1, "phase": args.phase, "started_utc": datetime.now(timezone.utc).isoformat(),
            "fixture": "Python daemon + sleep UI stand-ins; real tmux/systemd; no live harness",
            "checks": {}, "cleanup": {}, "completed": False, "verdict": "INVALIDATED"}
    units = []
    processes = []
    terminals = []
    original = None
    app_ids = []
    terminal = None
    terminal_identity = {}
    session_id = ""
    directory = None
    baseline = default_metadata()
    stage = "prerequisites"
    try:
        if args.phase == "desktop":
            if not args.allow_desktop:
                raise RuntimeError("desktop phase requires --allow-desktop")
            if not shutil.which("foot") or not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
                raise RuntimeError("desktop phase requires installed foot and live Hyprland")
            original = window_identity(windows(active=True)[0])
        for binary in ("tmux", "systemd-run", "systemctl"):
            if not shutil.which(binary):
                raise RuntimeError(f"missing required binary: {binary}")
        runtime_dir = Path(os.environ["XDG_RUNTIME_DIR"])
        s = runtime_dir.stat()
        if s.st_uid != os.getuid() or not stat.S_ISDIR(s.st_mode) or s.st_mode & 0o077:
            raise RuntimeError("unsafe XDG_RUNTIME_DIR")
        run(["systemctl", "--user", "is-system-running"])
        data["versions"] = versions()
        data["logout_policy"] = {"observed_linger": run(["loginctl", "show-user", str(os.getuid()),
                                                        "-p", "Linger", "--value"]).stdout.strip(),
                                 "modified": False, "logout_suspend_reboot_tested": False}
        directory = Path(tempfile.mkdtemp(prefix="hb-m0-", dir=runtime_dir))
        ROOT = directory
        nonce = uuid.uuid4().hex[:12]
        cfg = {role: f"harbormaster-m0-{role}-{nonce}.service" for role in ("runtime", "daemon", "ui")}
        data["resource_namespace"] = cfg
        for unit in cfg.values():
            if unit_info(unit).get("LoadState") != "not-found":
                raise RuntimeError("unit name collision")
        units = [cfg["ui"], cfg["daemon"], cfg["runtime"]]
        store(directory / "config.json", cfg)
        daemon_command = service(cfg["daemon"], [sys.executable, "-B", str(HERE / "probe.py"),
                                                  "fixture-daemon", str(directory)], directory)
        stage = "launch runtime from daemon cgroup"
        run(daemon_command)
        wait_for(lambda: (directory / "daemon-ready.json").exists())
        wait_for(lambda: (directory / "heartbeat.json").exists())
        server = json.loads((directory / "server.json").read_text())
        worker = proc(json.loads((directory / "heartbeat.json").read_text())["pid"])
        daemon = proc(int(unit_info(cfg["daemon"])["MainPID"]))
        assert worker and daemon
        run(service(cfg["ui"], [shutil.which("sleep"), "infinity"], directory))
        ui = proc(int(unit_info(cfg["ui"])["MainPID"]))
        assert ui
        processes.extend([server, worker, daemon, ui])
        data["units"] = {role: unit_info(unit) for role, unit in cfg.items()}
        data["process_cgroups"] = {"server": server["cgroup"], "worker": worker["cgroup"],
                                  "daemon": daemon["cgroup"], "ui": ui["cgroup"]}
        worker_unit = worker["cgroup"].rsplit("/", 1)[-1]
        worker_unit_info = unit_info(worker_unit)
        data["worker_unit"] = worker_unit_info
        worker_owned = (worker_unit == cfg["runtime"] or
                        (worker_unit.startswith("tmux-spawn-") and worker_unit.endswith(".scope")
                         and worker_unit_info.get("PartOf", "").split() == [cfg["runtime"]]))
        if worker_owned and worker_unit != cfg["runtime"]:
            units.append(worker_unit)
        data["checks"]["worker_lifecycle_owned_by_runtime"] = worker_owned
        data["checks"]["separate_cgroups"] = (
            worker_owned and worker["cgroup"] not in (daemon["cgroup"], ui["cgroup"])
            and len({server["cgroup"], daemon["cgroup"], ui["cgroup"]}) == 3
            and cfg["runtime"] in server["cgroup"]
            and all(v["KillMode"] == "control-group" and not v["PartOf"] and not v["BindsTo"]
                    for v in data["units"].values()))
        data["checks"]["private_socket"] = (directory.stat().st_mode & 0o777 == 0o700
                                               and stat.S_ISSOCK((directory / "tmux.sock").stat().st_mode)
                                               and (directory / "tmux.sock").stat().st_uid == os.getuid())
        observations = []

        def continuity(label):
            before = json.loads((directory / "heartbeat.json").read_text())["tick"]
            wait_for(lambda: json.loads((directory / "heartbeat.json").read_text())["tick"] > before)
            after = json.loads((directory / "heartbeat.json").read_text())["tick"]
            ok = same_process(server) and same_process(worker)
            observations.append({"stage": label, "heartbeat_before": before, "heartbeat_after": after,
                                 "same_server_identity": same_process(server),
                                 "same_worker_identity": same_process(worker)})
            return ok

        if args.phase == "desktop":
            stage = "exact terminal attach and focus"
            session_id = tmux(directory, "display-message", "-p", "-t", "=managed:", "#{session_id}").stdout.strip()
            if not session_id.startswith("$") or not session_id[1:].isdigit():
                raise RuntimeError("exact pane target did not resolve a session ID")
            app_ids.append("org.harbormaster.probe." + nonce + ".first")
            terminal, terminal_identity = launch_terminal(directory, app_ids[-1], terminals)
            processes.append(terminal_identity)
            try:
                wait_for(lambda: attached(directory, terminal, session_id))
            except TimeoutError:
                rows = tmux(directory, "list-clients", "-F", "#{client_pid}\t#{session_id}\t#{client_tty}").stdout.splitlines()
                data["attach_diagnostic"] = {"terminal_pid": terminal.pid, "session_id": session_id, "rows": rows}
                for row in rows:
                    pid = int(row.split("\t")[0])
                    p = proc(pid)
                    data["attach_diagnostic"].update(client_ppid=p["ppid"] if p else None,
                                                     client_fd0=os.readlink(f"/proc/{pid}/fd/0"))
                raise
            data["checks"]["exact_attach"] = attached(directory, terminal, session_id)
            # Switch away then dispatch to the exact address; don't count launch auto-focus.
            assert original
            focus(original)
            data["checks"]["exact_focus"] = focus(terminal_identity)
            focused_before = windows(active=True)[0]["address"]
            dispatch_before = sum(c["argv"][:2] == ["hyprctl", "dispatch"] for c in TRACE)
            cases = [("address", "0x0"), ("pid", terminal.pid + 100000000),
                     ("app_id", app_ids[-1] + ".wrong"),
                     ("start_ticks", terminal_identity["start_ticks"] + 1),
                     ("boot_id", "wrong-boot"), ("compositor", "wrong-instance")]
            negative = {key: not focus({**terminal_identity, key: value}) for key, value in cases}
            dispatch_after = sum(c["argv"][:2] == ["hyprctl", "dispatch"] for c in TRACE)
            data["negative_cases"] = negative
            data["checks"]["wrong_identity_rejected"] = (all(negative.values())
                and dispatch_before == dispatch_after and windows(active=True)[0]["address"] == focused_before)
            data["checks"]["wrong_tmux_session_rejected"] = not attached(directory, terminal, "$999999")
            data["checks"]["missing_session_rejected"] = tmux(directory, "has-session", "-t", "=missing", check=False).returncode != 0

        attachment_checks = {}
        for role, original_process in (("ui", ui), ("daemon", daemon)):
            stage = f"{role} restart"
            run(["systemctl", "--user", "restart", cfg[role]])
            current = proc(int(unit_info(cfg[role])["MainPID"]))
            assert current
            processes.append(current)
            if role == "daemon":
                wait_for(lambda: json.loads((directory / "daemon-ready.json").read_text())["pid"] == current["pid"])
            data["checks"][f"{role}_restart_survival"] = (not same_process(original_process)
                                                          and continuity(stage))
            if args.phase == "desktop":
                assert terminal is not None
                attachment_checks[stage] = attached(directory, terminal, session_id)
        stage = "daemon SIGKILL and explicit recovery"
        crashed = proc(int(unit_info(cfg["daemon"])["MainPID"]))
        assert crashed
        run(["systemctl", "--user", "kill", "--signal=SIGKILL", "--kill-whom=main", cfg["daemon"]])
        wait_for(lambda: not same_process(crashed))
        wait_for(lambda: unit_info(cfg["daemon"]).get("ActiveState") not in ("active", "deactivating"))
        data["checks"]["daemon_sigkill_survival"] = continuity(stage)
        if args.phase == "desktop":
            assert terminal is not None
            attachment_checks[stage] = attached(directory, terminal, session_id)
        # --collect intentionally removes the failed transient daemon definition.
        wait_for(lambda: unit_info(cfg["daemon"]).get("LoadState") == "not-found")
        run(daemon_command)
        recovered = proc(int(unit_info(cfg["daemon"])["MainPID"]))
        assert recovered
        processes.append(recovered)
        wait_for(lambda: json.loads((directory / "daemon-ready.json").read_text())["pid"] == recovered["pid"])
        sessions = tmux(directory, "list-sessions", "-F", "#{session_id}").stdout.splitlines()
        data["checks"]["daemon_recovery_no_duplicate"] = len(sessions) == 1 and continuity("daemon recovery")
        if args.phase == "desktop":
            assert terminal is not None
            stage = "terminal close and reattach"
            attachment_checks["daemon recovery"] = attached(directory, terminal, session_id)
            data["attachments_during_restarts"] = attachment_checks
            data["checks"]["attached_across_restarts"] = all(attachment_checks.values())
            close_terminal(terminal)
            wait_for(lambda: not any(w["address"] == terminal_identity["address"] for w in windows()))
            wait_for(lambda: not tmux(directory, "list-clients", "-F", "#{client_pid}").stdout.strip())
            data["checks"]["terminal_close_survival"] = continuity("terminal closed")
            dispatch_before = sum(c["argv"][:2] == ["hyprctl", "dispatch"] for c in TRACE)
            rejected = not focus(terminal_identity)
            dispatch_after = sum(c["argv"][:2] == ["hyprctl", "dispatch"] for c in TRACE)
            data["checks"]["stale_window_rejected"] = rejected and dispatch_before == dispatch_after
            app_ids.append("org.harbormaster.probe." + nonce + ".second")
            terminal, reattached_identity = launch_terminal(directory, app_ids[-1], terminals)
            processes.append(reattached_identity)
            wait_for(lambda: attached(directory, terminal, session_id))
            data["checks"]["exact_reattach"] = (attached(directory, terminal, session_id)
                and focus(reattached_identity) and continuity("terminal reattached"))
        data["observations"] = observations
        stage = "runtime stop"
        run(["systemctl", "--user", "stop", cfg["runtime"]])
        wait_for(lambda: not same_process(worker) and not same_process(server))
        data["checks"]["runtime_stop_ends_worker"] = (not same_process(worker) and not same_process(server)
                                                       and same_process(recovered))
        data["completed"] = True
    except Exception as exc:
        data["error"] = {"stage": stage, "type": type(exc).__name__, "message": str(exc)}
    finally:
        cleanup_errors = data["cleanup_errors"] = []

        def attempt(stage, operation, target=None):
            try:
                return operation()
            except Exception as exc:
                cleanup_errors.append({"stage": stage, "target": target,
                                       "type": type(exc).__name__, "message": str(exc)})
                data["verdict"] = "INVALIDATED"
                return None

        for child in terminals:
            attempt("close_terminal", lambda: close_terminal(child), child.pid)
        if args.phase == "desktop" and original:
            data["cleanup"]["owned_windows_gone"] = bool(attempt("owned_windows_gone",
                lambda: wait_for(lambda: not any(w["class"] in app_ids for w in windows()))))
            data["cleanup"]["original_focus_restored"] = bool(attempt("original_focus_restored",
                lambda: focus(original)))
        for unit in units:
            for action in ("stop", "reset-failed"):
                attempt(action, lambda: run(["systemctl", "--user", action, unit], check=False), unit)
        # Eager lists: a failed readback must not short-circuit later owned targets.
        data["cleanup"]["units_unloaded"] = all([attempt("unit_readback",
            lambda: unit_info(u).get("LoadState") == "not-found", u) for u in units])
        data["cleanup"]["owned_processes_gone"] = all([attempt("process_readback",
            lambda: not same_process(p), p["pid"]) for p in processes])
        if directory:
            log = directory / "daemon-commands.jsonl"
            data["daemon_command_batches"] = attempt("daemon_command_batches",
                lambda: [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else [], str(log))
            # Unique mkdtemp returned to this invocation; never glob or accept a cleanup path.
            attempt("remove_directory", lambda: shutil.rmtree(directory), str(directory))
        data["cleanup"]["private_directory_removed"] = bool(attempt("private_directory_removed",
            lambda: directory is None or not directory.exists()))
        data["checks"]["default_tmux_unchanged"] = bool(attempt("default_tmux_unchanged",
            lambda: baseline == default_metadata()))
        data["cleanup"]["default_tmux_unchanged"] = data["checks"]["default_tmux_unchanged"]
        data["commands"] = TRACE
        data["source_sha256"] = {name: attempt("source_sha256",
                                 lambda: hashlib.sha256((HERE / name).read_bytes()).hexdigest(), name)
                                 for name in ("probe.py", "identity.py", "test_identity.py", "test_runtime.py", "test_cleanup.py")}
        data["verdict"] = ("VALIDATED" if data["completed"] and not data.get("error") and all(data["checks"].values())
                           and not cleanup_errors and all(data["cleanup"].values()) else "INVALIDATED")
        def sanitized(payload):
            text = json.dumps(payload, indent=2)
            if ROOT:
                text = text.replace(str(ROOT), "$PROBE_DIR")
            if original:
                text = text.replace(original["address"], "$ORIGINAL_WINDOW")
            return text.replace(str(HERE), "$SPIKE_DIR").replace(str(Path.home()), "$HOME")

        def export():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(sanitized(data) + "\n")

        attempt("output_export", export, str(args.output))
        print(sanitized({"verdict": data["verdict"], "checks": data["checks"], "cleanup": data["cleanup"],
                         "error": data.get("error"), "cleanup_errors": cleanup_errors,
                         "output": str(args.output)}))
    return 0 if data["verdict"] == "VALIDATED" else 1


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] in ("fixture-worker", "fixture-daemon"):
        path = Path(sys.argv[2])
        if sys.argv[1] == "fixture-worker":
            fixture_worker(path)
        else:
            fixture_daemon(path)
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--phase", choices=["lifecycle", "desktop"], default="lifecycle")
        parser.add_argument("--allow-desktop", action="store_true",
                            help="allow new disposable foot windows and temporary focus changes")
        parser.add_argument("--output", type=Path, required=True)
        raise SystemExit(probe(parser.parse_args()))
