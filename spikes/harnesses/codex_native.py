"""Native Codex PTY startup without entering a prompt, login, or trust response.

Runs INSIDE the offline namespace. Output is discarded after local gate matching.
Only fixed match names and byte counts are exported, never terminal content.
"""
import errno
import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import termios
import time

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
env = dict(os.environ, TERM="xterm-256color")
process = subprocess.Popen(["/opt/codex", "--no-alt-screen"], stdin=slave, stdout=slave, stderr=slave,
                           env=env, start_new_session=True)
os.close(slave)
screen = b""
protocol_bytes = 0
signals = []
try:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline and len(screen) < 65536:
        if not select.select([master], [], [], .1)[0]:
            if process.poll() is not None:
                break
            continue
        try:
            chunk = os.read(master, 8192)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        screen += chunk
        # Terminal-device status response only; no semantic/user input.
        if b"\x1b[6n" in chunk:
            protocol_bytes += os.write(master, b"\x1b[1;1R")
finally:
    if process.poll() is None:
        signals.append("SIGTERM")
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            signals.append("SIGKILL")
            os.killpg(process.pid, signal.SIGKILL)
    process.wait()
    os.close(master)
text = screen.decode(errors="replace").lower()
patterns = {"auth_required": ["sign in", "log in", "login", "api key"],
            "hook_review_required": ["/hooks", "hooks need", "review hooks"],
            "project_trust_required": ["trust this", "trust the files"],
            "startup_error": ["error:", "panicked"]}
print(json.dumps({"kind": "live-native-cli-startup", "argv": ["/opt/codex", "--no-alt-screen"],
                  "semantic_input_bytes": 0, "terminal_protocol_bytes": protocol_bytes,
                  "screen_bytes_seen": len(screen), "screen_exported": False,
                  "matched_gates": [name for name, needles in patterns.items() if any(n in text for n in needles)],
                  "exit_code": process.returncode, "cleanup_signals": signals}))
