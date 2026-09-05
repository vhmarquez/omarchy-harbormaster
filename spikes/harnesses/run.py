#!/usr/bin/env python3
"""Reproduce M0 issue #5 evidence. Requires installed binaries + working bwrap.

Every harness runs offline in a disposable mount namespace. No prompts, turns,
trust writes, model calls, real-profile config writes, or generated transcripts
are requested. Only allowlisted metadata leaves the sandbox.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import tempfile
import time

from probe import sandbox, hook_overlay
from observe import EVENTS

HERE = Path(__file__).resolve().parent


def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(base, args, timeout=25):
    with subprocess.Popen(base + args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, env={"PATH": "/usr/bin:/bin"},
                          start_new_session=True) as process:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
            timed_out = False
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
            timed_out = True
    # Startup/help only: no prompt or transcript output is requested.
    return {"argv": args, "exit_code": process.returncode, "timed_out": timed_out,
            "stdout": stdout.decode(errors="replace")[:24000],
            "stderr": stderr.decode(errors="replace")[:4000]}


def must(record):
    if record["exit_code"] != 0 or record["timed_out"]:
        raise RuntimeError(json.dumps(record))
    return record


def hooks(harness, output):
    return {event: [{"hooks": [{"type": "command", "command":
            f"/usr/bin/python3 /opt/spike/observe.py {harness} /probe/{output}", "timeout": 2}]}]
            for event in sorted(EVENTS[harness])}


def events(root, filename):
    path = root / filename
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def app_server(base):
    """Read-only initialization and native-hook discovery, NOT a managed thread."""
    request_log, replies = [], []
    with subprocess.Popen(base + ["/opt/codex", "app-server", "--listen", "stdio://"],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                          env={"PATH": "/usr/bin:/bin"}, start_new_session=True) as process:
        selector = selectors.DefaultSelector()
        assert process.stdin is not None and process.stdout is not None
        selector.register(process.stdout, selectors.EVENT_READ)
        buffer = b""

        def request(method, params, number):
            nonlocal buffer
            message = {"id": number, "method": method, "params": params}
            request_log.append(message)
            process.stdin.write((json.dumps(message) + "\n").encode())
            process.stdin.flush()
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                if not selector.select(max(0, deadline - time.monotonic())):
                    break
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    raise RuntimeError("app-server exited before reply")
                buffer += chunk
                if len(buffer) > 262144:
                    raise RuntimeError("app-server frame budget exceeded")
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    reply = json.loads(line)
                    if reply.get("id") == number:
                        replies.append(reply)
                        return reply
            raise RuntimeError(f"app-server timed out: {method}")

        try:
            request("initialize", {"clientInfo": {"name": "harbormaster_m0_probe", "version": "0.0.0"},
                                   "capabilities": {"experimentalApi": True}}, 1)
            process.stdin.write(b'{"method":"initialized","params":{}}\n')
            process.stdin.flush()
            request("hooks/list", {"cwds": ["/probe"]}, 2)
        finally:
            selector.close()
            process.stdin.close()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    return {"requests": request_log, "replies": replies,
            "exit_code": process.returncode, "kind": "live-app-server-read-only"}


def hermes_probe(root, base):
    home = root / "home/.hermes"
    home.mkdir(parents=True, exist_ok=True)
    config = home / "config.yaml"
    # Test fixture, not a user's configuration. Explicitly opts in only sentinel.
    original = b'plugins:\n  enabled: [harbormaster-m0-sentinel]\n  hook_callback_timeout: 1\ndisplay:\n  compact: true\n'
    config.write_bytes(original)
    for name in ["observer", "sentinel"]:
        plugin = home / f"plugins/harbormaster-m0-{name}"
        plugin.mkdir(parents=True)
        (plugin / "plugin.yaml").write_text(f'name: harbormaster-m0-{name}\nversion: "0.0.0"\ndescription: Reviewed disposable M0 observer\n')
        (plugin / "__init__.py").write_text(
            'import sys\nsys.path.insert(0, "/opt/spike")\nfrom observe import EVENTS, emit\n'
            'def register(ctx):\n'
            '    for event in sorted(EVENTS["hermes"]):\n'
            '        def callback(_event=event, **payload):\n'
            f'            return emit("hermes", dict(payload, hook_event_name=_event), "/probe/hermes-{name}.jsonl")\n'
            '        ctx.register_hook(event, callback)\n')
    cli = ["/opt/hermes/venv/bin/python", "/opt/hermes/hermes"]
    version = must(run(base, cli + ["--version"]))
    disabled = must(run(base, ["/opt/hermes/venv/bin/python", "/opt/spike/hermes_driver.py"]))
    disabled_ok = not events(root, "hermes-observer.jsonl") and bool(events(root, "hermes-sentinel.jsonl"))
    enable = must(run(base, cli + ["plugins", "enable", "harbormaster-m0-observer"]))
    enabled = must(run(base, ["/opt/hermes/venv/bin/python", "/opt/spike/hermes_driver.py"]))
    observed = events(root, "hermes-observer.jsonl")
    verified = {e["event"] for e in observed} == EVENTS["hermes"] and len(observed) == len(EVENTS["hermes"])
    # Supported disable first, then restore fixture bytes; no user's profile touched.
    disable = must(run(base, cli + ["plugins", "disable", "harbormaster-m0-observer"]))
    post_disable = must(run(base, ["/opt/hermes/venv/bin/python", "/opt/spike/hermes_driver.py"]))
    verified = verified and events(root, "hermes-observer.jsonl") == observed
    import_check = run(base, ["/opt/hermes/venv/bin/python", "-c",
                       'import yaml; c=yaml.safe_load(open("/probe/home/.hermes/config.yaml")); assert c["display"]["compact"] is True; assert "harbormaster-m0-sentinel" in c["plugins"]["enabled"]; print("unrelated config preserved")'])
    must(import_check)
    config.write_bytes(original)
    shutil.rmtree(home / "plugins/harbormaster-m0-observer")
    return {"version": version["stdout"].strip(), "version_command": version,
            "kind": "installed-loader-synthetic-dispatch", "events": observed,
            "loader_callbacks_verified": verified, "disabled_plugin_did_not_run": disabled_ok,
            "commands": [disabled, enable, enabled, disable, post_disable, import_check],
            "restored": config.read_bytes() == original and (home / "plugins/harbormaster-m0-sentinel/__init__.py").exists(),
            "config_original_sha256": hashlib.sha256(original).hexdigest(),
            "config_restored_sha256": digest(config)}


def claude_probe(root, base):
    home = root / "home/.claude"
    home.mkdir(parents=True, exist_ok=True)
    config = home / "settings.json"
    original = b'{"permissions":{"defaultMode":"default"},"hooks":{"Setup":[{"hooks":[{"type":"command","command":"/usr/bin/true"}]}]}}\n'
    config.write_bytes(original)
    version = must(run(base, ["/opt/claude", "--version"]))
    help_record = must(run(base, ["/opt/claude", "--help"]))
    with hook_overlay(config, hooks("claude", "claude-events.jsonl")):
        setup = run(base, ["/opt/claude", "--init-only", "--setting-sources", "user", "--strict-mcp-config"], timeout=15)
        observed = events(root, "claude-events.jsonl")
    return {"version": version["stdout"].strip(), "version_command": version,
            "kind": "live-cli-init-only", "events": observed, "commands": [setup],
            "help_capabilities": {token: token in help_record["stdout"] for token in
                                  ["--resume", "--continue", "--include-hook-events", "attach <id>", "stop|kill <id>"]},
            "blocker": None if observed else "No hook callback observed at offline unauthenticated --init-only; see exit/timeout. No trust/auth was bypassed.",
            "restored": config.read_bytes() == original,
            "config_original_sha256": hashlib.sha256(original).hexdigest(),
            "config_restored_sha256": digest(config)}


def codex_probe(root, base):
    home = root / "home/.codex"
    home.mkdir(parents=True, exist_ok=True)
    config = home / "config.toml"
    original = b'# unrelated fixture setting\n[analytics]\nenabled = false\n'
    config.write_bytes(original)
    hook_file = home / "hooks.json"
    hook_original = b'{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/usr/bin/true"}]}]}}\n'
    hook_file.write_bytes(hook_original)
    version = must(run(base, ["/opt/codex", "--version"]))
    schema_cmd = must(run(base, ["/opt/codex", "app-server", "generate-json-schema", "--out", "/probe/schema"]))
    schema_path = next((root / "schema").rglob("HooksListResponse.json"))
    schema = json.loads(schema_path.read_text())
    method_inventory = {}
    for direction in ["Client", "Server"]:
        request_schema = next((root / "schema").rglob(f"{direction}Request.json"))
        variants = json.loads(request_schema.read_text())["oneOf"]
        method_inventory[direction] = sorted({name for variant in variants
                                              for name in variant["properties"]["method"]["enum"]})
    with hook_overlay(hook_file, hooks("codex", "codex-events.jsonl")):
        native_record = must(run(base, ["/usr/bin/python3", "/opt/spike/codex_native.py"], timeout=10))
        native = json.loads(native_record["stdout"])
        rpc = app_server(base)
    listed_reply = rpc["replies"][-1]
    # Traverse only this generated metadata result, never sessions/history.
    def entries(value):
        if isinstance(value, dict):
            if "eventName" in value:
                yield value
            for child in value.values():
                yield from entries(child)
        elif isinstance(value, list):
            for child in value:
                yield from entries(child)
    listed = list(entries(listed_reply))
    return {"version": version["stdout"].strip(), "version_command": version,
            "kind": "live-native-hook-discovery-via-app-server", "rpc": rpc, "native_startup": native,
            "schema_command": schema_cmd, "schema_sha256": digest(schema_path),
            "schema_hook_events": schema["definitions"]["HookEventName"]["enum"],
            "schema_client_methods": method_inventory["Client"], "schema_server_methods": method_inventory["Server"],
            "hooks_listed": len(listed), "hook_metadata": listed,
            "untrusted_hooks_need_review": bool(listed) and all(h.get("trustStatus") == "untrusted" and not h.get("isManaged") for h in listed),
            "events": events(root, "codex-events.jsonl"), "generation_requests_sent": 0, "trust_writes_sent": 0,
            "blocker": "Native startup stops at authentication before /hooks can be reached with the default provider. Native command hooks are reported untrusted and require exact-definition review. No credentials, fake provider/API responses, /hooks acceptance, trust writes, managed-source impersonation or trust bypass were used. No native callback invocation is claimed.",
            "restored": config.read_bytes() == original and hook_file.read_bytes() == hook_original,
            "config_original_sha256": hashlib.sha256(original).hexdigest(), "config_restored_sha256": digest(config),
            "hooks_original_sha256": hashlib.sha256(hook_original).hexdigest(), "hooks_restored_sha256": digest(hook_file)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=HERE / "evidence/installed.json")
    parser.add_argument("--hermes-source", type=Path, default=Path.home() / ".hermes/hermes-agent")
    parser.add_argument("--claude", type=Path, default=Path.home() / ".local/share/mise/installs/claude/latest/claude")
    parser.add_argument("--codex", type=Path, default=Path.home() / ".local/share/mise/installs/codex/latest/bin/codex")
    args = parser.parse_args()
    # Do not invoke mise shims: even 'mise which' may run housekeeping.
    hermes = args.hermes_source.resolve()
    python_link = (hermes / "venv/bin/python").readlink()
    python_root = python_link.parent.parent
    mounts = {str(args.claude): "/opt/claude", str(args.codex): "/opt/codex",
              str(hermes): "/opt/hermes", str(python_root): str(python_root), str(HERE): "/opt/spike"}
    for path in mounts:
        if not Path(path).exists():
            raise RuntimeError(f"required installed path missing: {path}")
    if (hermes / ".env").exists():
        raise RuntimeError("Hermes source .env exists; refuse mounting credential-bearing source. Use reviewed clean source mapping.")
    result = {"schema_version": 1, "issue": 5, "recorded_at": datetime.now(timezone.utc).isoformat(),
              "privacy": {"transcripts_exported": 0, "inherited_credentials": False,
                          "network": "bwrap --unshare-net", "real_profiles_mounted": False,
                          "scope": "M0 only; no generation, no app-server thread/start, no trust writes"},
              "binaries": {"claude": {"path": str(args.claude.resolve()), "sha256": digest(args.claude)},
                           "codex": {"path": str(args.codex.resolve()), "sha256": digest(args.codex)}},
              "hermes_source": {"path": str(hermes), "commit": subprocess.check_output(
                  ["/usr/bin/git", "-C", str(hermes), "rev-parse", "HEAD"], text=True).strip(),
                  "plugins_sha256": digest(hermes / "hermes_cli/plugins.py")}, "harnesses": {}}
    with tempfile.TemporaryDirectory(prefix="harbormaster-m0-harnesses-") as tmp:
        root = Path(tmp)
        base = sandbox(root, mounts)
        for name, probe in [("hermes", hermes_probe), ("claude", claude_probe), ("codex", codex_probe)]:
            result["harnesses"][name] = probe(root, base)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    serialized = json.dumps(result, indent=2, sort_keys=True).replace(str(Path.home()) + "/", "$HOME/")
    args.out.write_text(serialized + "\n")
    print(json.dumps({name: {k: v for k, v in data.items() if k in
                           ["version", "restored", "blocker", "loader_callbacks_verified", "hooks_listed", "untrusted_hooks_need_review"]}
                      for name, data in result["harnesses"].items()}, indent=2))


if __name__ == "__main__":
    main()
