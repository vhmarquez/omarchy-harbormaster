"""Self-probe of the actual selected offline boundary, using synthetic data."""
import errno
import os
from pathlib import Path
import socket
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verification.sandbox import environment


def inspect(outside_canary):
    if outside_canary.exists():
        raise ValueError("outside canary is visible")
    expected = environment()
    if any(os.environ.get(key) != value for key, value in expected.items()):
        raise ValueError("verification environment differs from allowlist")
    if set(os.environ) - set(expected) - {"HOSTNAME"}:
        raise ValueError("unexpected inherited environment names")
    if Path("/work/.git").exists() or Path("/run/user").exists():
        raise ValueError("host credentials or desktop paths are exposed")
    for parent in (Path("/work"), Path("/tools")):
        if not parent.is_dir():
            raise ValueError("verification input mount is missing")
        try:
            with (parent / ".boundary-write-probe").open("x"):
                pass
        except OSError as error:
            if error.errno not in (errno.EROFS, errno.EACCES, errno.EPERM):
                raise
            continue
        (parent / ".boundary-write-probe").unlink()
        raise ValueError("verification input mount is writable")
    interfaces = {line.split(":")[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()
                  if ":" in line}
    if interfaces - {"lo"} or len(Path("/proc/net/route").read_text().splitlines()) > 1:
        raise ValueError("verification network has an external interface or route")
    with socket.socket() as connection:
        connection.settimeout(0.2)
        if connection.connect_ex(("192.0.2.1", 9)) == 0:
            raise ValueError("external networking is unexpectedly available")
    print("PASS outside canary, allowlisted environment, read-only inputs, offline network probe")


if __name__ == "__main__":
    inspect(Path(sys.argv[1]))
