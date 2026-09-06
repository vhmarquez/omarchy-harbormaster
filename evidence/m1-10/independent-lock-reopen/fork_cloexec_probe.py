"""Isolated kernel probe: CLOEXEC does not prevent fork-time flock inheritance."""
import errno
import fcntl
import json
import os
from pathlib import Path

path = Path('/state/tmp/fork-cloexec.lock')
lock = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
assert fcntl.fcntl(lock, fcntl.F_GETFD) & fcntl.FD_CLOEXEC
identity = os.fstat(lock)
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
ready_r, ready_w = os.pipe2(os.O_CLOEXEC)
release_r, release_w = os.pipe2(os.O_CLOEXEC)
child = os.fork()
if child == 0:
    os.close(ready_r)
    os.close(release_w)
    os.write(ready_w, b'R')
    assert os.read(release_r, 1) == b'X'
    os.execv('/usr/bin/true', ['true'])
os.close(ready_w)
os.close(release_r)
try:
    assert os.read(ready_r, 1) == b'R'
    os.close(lock)
    local_matches = []
    inherited_matches = []
    for pid, found in [(os.getpid(), local_matches), (child, inherited_matches)]:
        for fd in Path(f'/proc/{pid}/fd').iterdir():
            try:
                stat = fd.stat()
            except FileNotFoundError:
                continue
            if (stat.st_dev, stat.st_ino) == (identity.st_dev, identity.st_ino):
                found.append(fd.name)
    assert not local_matches
    assert inherited_matches
    with path.open('rb') as attempt:
        try:
            fcntl.flock(attempt, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            assert error.errno in (errno.EAGAIN, errno.EWOULDBLOCK)
            blocked_before_exec = True
        else:
            raise AssertionError('inherited description must keep ownership')
    os.write(release_w, b'X')
    assert os.read(ready_r, 1) == b'', 'exec closes the CLOEXEC synchronization pipe'
    waited, status = os.waitpid(child, 0)
    assert waited == child and status == 0
    child = 0
    with path.open('rb') as attempt:
        fcntl.flock(attempt, fcntl.LOCK_EX | fcntl.LOCK_NB)
    print(json.dumps({'cloexec_verified': True, 'parent_lock_descriptors_after_close': local_matches,
                      'forked_child_inherited_descriptors': inherited_matches,
                      'reopen_blocked_before_exec': blocked_before_exec,
                      'reopen_after_exec_passed': True}))
finally:
    if child:
        os.kill(child, 9)
        os.waitpid(child, 0)
    os.close(ready_r)
    os.close(release_w)
    path.unlink()
