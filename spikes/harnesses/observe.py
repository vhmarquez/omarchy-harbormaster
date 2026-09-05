"""Disposable M0 observer: never emits hook decisions/context or opens transcripts."""
import json
import os
from pathlib import Path
import re
import sys

EVENTS = {
    "claude": {"Setup", "SessionStart", "SessionEnd", "UserPromptSubmit", "Stop", "StopFailure",
               "PermissionRequest", "Notification", "SubagentStart", "SubagentStop"},
    "codex": {"SessionStart", "SessionEnd", "UserPromptSubmit", "Stop", "PermissionRequest",
              "SubagentStart", "SubagentStop", "Interrupt"},
    "hermes": {"on_session_start", "on_session_end", "on_session_finalize", "on_session_reset",
               "pre_approval_request", "post_approval_response", "subagent_start", "subagent_stop"},
}
ID_KEYS = {"session_id", "turn_id", "parent_session_id", "parent_turn_id", "child_session_id",
           "child_subagent_id", "parent_subagent_id", "agent_id", "thread_id"}
ENUMS = {"source": {"startup", "resume", "clear", "compact"},
         "surface": {"cli", "gateway", "smart", "acp"},
         "notification_type": {"permission_prompt", "idle_prompt", "auth_success", "elicitation_dialog"}}


def project(harness, payload):
    if not isinstance(payload, dict):
        return None
    event = payload.get("hook_event_name")
    if not isinstance(event, str) or event not in EVENTS.get(harness, set()):
        return None
    record = {"harness": harness, "event": event}
    for key in ID_KEYS:
        value = payload.get(key)
        if isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,80}", value):
            record[key] = value
    for key, values in ENUMS.items():
        value = payload.get(key)
        if isinstance(value, str) and value in values:
            record[key] = value
    return record


def emit(harness, payload, path):
    # This is a bounded disposable-file sink, not an IPC/durable-spool design.
    import fcntl
    import stat
    try:
        record = project(harness, payload)
        if record is not None:
            line = (json.dumps(record, sort_keys=True) + "\n").encode()
            fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                info = os.fstat(fd)
                if stat.S_ISREG(info.st_mode) and info.st_size + len(line) <= 65536:
                    os.write(fd, line)
            finally:
                os.close(fd)
    except (OSError, TypeError, ValueError):
        pass
    return None


if __name__ == "__main__":
    try:
        # Bound bytes and read time, including a producer that never closes stdin.
        import signal
        signal.signal(signal.SIGALRM, lambda *_: sys.exit(0))
        signal.alarm(1)
        raw = sys.stdin.buffer.read(16385)
        if len(raw) <= 16384:
            emit(sys.argv[1], json.loads(raw), Path(sys.argv[2]))
    except (OSError, ValueError, TypeError, RecursionError, IndexError):
        pass
