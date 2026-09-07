#!/usr/bin/env python3
"""Opt-in #13 workflow with synthetic jobs and disposable foot windows."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import stat
import subprocess
import uuid

spec = importlib.util.spec_from_file_location("runtime_demo", Path(__file__).with_name("demo-runtime.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


def windows(env, active=False):
    raw = json.loads(base.run(["/usr/bin/hyprctl", "-j", "activewindow" if active else "clients"], env).stdout)
    return [{k: item.get(k) for k in ("address", "pid", "class")}
            for item in ([raw] if active else raw)]


def restore_focus(env, original):
    address = original["address"]
    if not re.fullmatch(r"0x[0-9a-fA-F]{1,16}", address):
        return False
    if base.identity(original["pid"]) != original["start"]:
        return False
    matches = [w for w in windows(env) if all(w[k] == original[k] for k in ("address", "pid", "class"))]
    if len(matches) != 1:
        return False
    base.run(["/usr/bin/hyprctl", "dispatch", 'hl.dsp.focus({window="address:' + address + '"})'], env)
    return bool(base.wait_for(lambda: windows(env, True)[0]["address"] == address))


class ControlsDemo(base.Demo):
    def __init__(self, binary, root, session):
        super().__init__(binary, root, session)
        self.env.update({key: os.environ[key] for key in ("WAYLAND_DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE")})
        worker = base.WORKER.replace('"heartbeat.tmp"', 'f"heartbeat-{os.getpid()}.tmp"')
        worker = worker.replace('"heartbeat.json"', 'f"heartbeat-{os.getpid()}.json"')
        (root / "hermes").write_text(worker)
        self.neighbor = None

    def beat(self, pid):
        try:
            return json.loads((self.root / "project" / f"heartbeat-{pid}.json").read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            return None

    def launch(self, task, shared=False):
        args = ["run", "launch", task, "--logout-policy", "existing"]
        if shared:
            args.append("--allow-shared-checkout")
        result = self.cli(*args)
        self.units.append(result["unit"])
        return result

    def worker(self, run_id):
        socket = self.root / "harbormaster/runners" / str(uuid.UUID(run_id)) / "tmux.sock"
        result = base.run(["/usr/bin/tmux", "-S", socket, "display-message", "-p", "-t", "=managed:", "#{pane_pid}"], self.env)
        return int(result.stdout)

    def change_records(self, run_id, change=None, restore=None):
        # Only this disposable database, with its manager stopped. Never copy a
        # live SQLite main file. This constructs named crash/identity fixtures.
        assert self.property(self.manager, "ActiveState") in ("inactive", "failed")
        saved = []
        with sqlite3.connect(self.root / "state/harbormaster/state.db") as db:
            for table in ("managed_runs", "runtime_controls"):
                old = db.execute(f"SELECT record FROM {table} WHERE id=?", (run_id,)).fetchone()[0]
                saved.append((table, old))
                value = json.loads(old)
                if restore is not None:
                    data = dict(restore)[table]
                else:
                    change(table, value)
                    data = json.dumps(value, separators=(",", ":")).encode()
                db.execute(f"UPDATE {table} SET record=? WHERE id=?", (data, run_id))
            revision = int.from_bytes(db.execute("SELECT revision FROM metadata WHERE id=1").fetchone()[0], "big")
            db.execute("UPDATE metadata SET revision=? WHERE id=1", ((revision + 1).to_bytes(8, "big"),))
        return saved

    def recovery(self, task, launched, worker):
        self.phase = "reconcile reserved live runtime"
        self.stop(self.manager)

        def lose_observation(table, value):
            if table == "managed_runs":
                value["server"] = None
            else:
                value["invocation"], value["pane"] = None, None

        self.change_records(launched["run"]["id"], change=lose_observation)
        self.start_manager()
        restored = self.launch(task)
        assert restored["run"] == launched["run"]
        assert self.worker(restored["run"]["id"]) == worker

    def terminal_flow(self, run_id, original, worker):
        self.phase = "attach and verify focus"
        attached = self.cli("run", "attach", run_id)
        self.units.append(attached["terminal_unit"])
        duplicate = self.cli("run", "attach", run_id)
        assert duplicate["action"] == "already_attached" and duplicate["terminal"] == attached["terminal"]
        assert restore_focus(self.env, original)
        focused = self.cli("run", "open", run_id)
        assert focused["address"] == attached["terminal"]["window"]["address"]
        assert windows(self.env, True)[0]["address"] == focused["address"]
        self.phase = "reconnect saved terminal after manager restart"
        self.stop(self.manager)
        self.start_manager()
        assert self.cli("run", "attach", run_id)["terminal"] == attached["terminal"]
        self.phase = "reattach after owned terminal closes"
        before = self.beat(worker)
        self.stop(attached["terminal_unit"])
        base.wait_for(lambda: (b := self.beat(worker)) and b["tick"] > before["tick"])
        stale = base.run([self.binary, "run", "open", run_id], self.env, False)
        assert stale.returncode != 0
        reattached = self.cli("run", "attach", run_id)
        self.units.append(reattached["terminal_unit"])
        assert reattached["terminal_unit"] != attached["terminal_unit"]
        assert self.worker(run_id) == worker
        assert self.cli("run", "actions", run_id)["resume"] is False

    def unrelated_target(self, run_id):
        self.phase = "reject unrelated process identity"
        self.neighbor = subprocess.Popen(["/usr/bin/sleep", "60"], env={"PATH": "/usr/bin"},
                                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        pid = self.neighbor.pid
        identity = {"pid": pid, "start_ticks": int(base.identity(pid)),
                    "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip()}
        self.stop(self.manager)
        saved = self.change_records(run_id, change=lambda table, value: value.update(server=identity) if table == "managed_runs" else None)
        self.start_manager()
        denied = base.run([self.binary, "run", "end", run_id, "--confirm"], self.env, False)
        assert denied.returncode != 0 and self.neighbor.poll() is None
        self.stop(self.manager)
        self.change_records(run_id, restore=saved)
        self.start_manager()

    def exercise(self, original):
        self.phase = "register and launch"
        self.start_manager()
        self.project = self.cli("project", "add", self.root / "project", "Control demo")["Project"]["id"]
        self.cli("preset", "add", "demo", self.root / "hermes", "synthetic")
        task = self.cli("task", "add", self.project, "demo", "First worker")["Task"]["id"]
        launched = self.launch(task)
        run_id = launched["run"]["id"]
        worker = self.worker(run_id)
        before = base.wait_for(lambda: self.beat(worker))
        assert before["tty"] and before["clean"] and before["argv"] == ["-p", "synthetic", "chat"]
        self.recovery(task, launched, worker)
        self.phase = "shared project acknowledgement"
        second = self.cli("task", "add", self.project, "demo", "Second worker")["Task"]["id"]
        denied = base.run([self.binary, "run", "launch", second, "--logout-policy", "existing"], self.env, False)
        assert denied.returncode != 0 and b"--allow-shared-checkout" in denied.stderr
        assert len(self.cli("run", "list", self.project)["items"]) == 1
        shared = self.launch(second, True)
        assert self.launch(second)["run"] == shared["run"]
        self.terminal_flow(run_id, original, worker)
        self.unrelated_target(shared["run"]["id"])
        self.phase = "end only verified runtimes"
        for target in (run_id, shared["run"]["id"]):
            assert self.cli("run", "end", target, "--confirm")["runtime"] == "stopped"
            assert self.cli("run", "end", target, "--confirm")["runtime"] == "stopped"
        assert self.neighbor.poll() is None and self.cli("manager", "status")["manager"] == "running"
        return {name: True for name in ("synthetic_pty_environment", "reconciled_reserved_live_runtime",
                "shared_writer_acknowledgement", "idempotent_launch_and_attach", "exact_focus_readback",
                "terminal_survived_manager_restart", "terminal_close_preserved_worker", "fresh_reattach_same_worker",
                "resume_unavailable", "unrelated_process_preserved", "graceful_owned_end")}

    def cleanup(self):
        # Recover only UUID intents from this invocation's private database,
        # including an activation whose reply was lost before its unit was noted.
        errors = []
        try:
            self.stop(self.manager)
            with sqlite3.connect(self.root / "state/harbormaster/state.db") as db:
                for (run_id,) in db.execute("SELECT id FROM managed_runs LIMIT 1001"):
                    self.units.append("harbormaster-runtime-" + str(uuid.UUID(run_id)) + ".service")
                for (record,) in db.execute("SELECT record FROM runtime_controls LIMIT 1001"):
                    terminal = json.loads(record).get("terminal")
                    if terminal:
                        self.units.append("harbormaster-terminal-" + str(uuid.UUID(terminal["nonce"])) + ".service")
        except (OSError, RuntimeError, ValueError, sqlite3.Error, subprocess.SubprocessError, TimeoutError):
            errors.append("terminal intent cleanup lookup")
        for unit in reversed(list(dict.fromkeys(self.units))):
            try:
                base.run(["/usr/bin/systemctl", "--user", "stop", unit], self.env, False)
                base.wait_for(lambda: self.property(unit, "ActiveState") in ("inactive", "failed"))
            except (OSError, RuntimeError, ValueError, subprocess.SubprocessError, TimeoutError):
                errors.append("owned unit cleanup")
        if self.neighbor is not None and self.neighbor.poll() is None:
            try:
                self.neighbor.terminate()
                self.neighbor.wait(timeout=3)
            except (OSError, subprocess.SubprocessError):
                errors.append("disposable neighbor cleanup")
        if not errors:
            shutil.rmtree(self.root)
        return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-desktop", action="store_true", required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    session = Path(os.environ["XDG_RUNTIME_DIR"])
    info = session.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        parser.error("unsafe user runtime directory")
    desktop_env = {"PATH": "/usr/bin:/bin", "HOME": "/nonexistent", "XDG_RUNTIME_DIR": str(session),
                   "HYPRLAND_INSTANCE_SIGNATURE": os.environ["HYPRLAND_INSTANCE_SIGNATURE"]}
    original = windows(desktop_env, True)[0]
    original["start"] = base.identity(original["pid"])
    if not original["start"] or not re.fullmatch(r"0x[0-9a-fA-F]{1,16}", original["address"]):
        parser.error("stable original window required")
    args.output.mkdir(mode=0o700)
    root = session / ("h13-" + uuid.uuid4().hex[:8])
    root.mkdir(mode=0o700)
    paths = [Path.home() / ".tmux.conf", Path.home() / ".config/tmux/tmux.conf", Path(f"/tmp/tmux-{os.getuid()}/default")]
    before = base.metadata(paths)
    demo = ControlsDemo(binary, root, session)
    report = {"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "checks": {}, "owned_root": str(root)}
    completed = False
    try:
        report["checks"] = demo.exercise(original)
        completed = True
    except (OSError, RuntimeError, ValueError, AssertionError, sqlite3.Error, subprocess.SubprocessError, TimeoutError) as error:
        report["error"] = {"phase": demo.phase, "type": type(error).__name__, "detail": str(error)[:1024]}
    finally:
        report["cleanup_errors"] = demo.cleanup()
        try:
            report["checks"]["original_focus_restored"] = restore_focus(demo.env, original)
        except (OSError, RuntimeError, ValueError, subprocess.SubprocessError, TimeoutError):
            report["checks"]["original_focus_restored"] = False
        report["checks"]["default_tmux_metadata_unchanged"] = before == base.metadata(paths)
        report["passed"] = completed and not report["cleanup_errors"] and all(report["checks"].values())
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
