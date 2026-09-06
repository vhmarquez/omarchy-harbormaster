"""Actual kernel ENOSPC in the canonical verifier's private bounded tmpfs."""
import errno
import os
from pathlib import Path
import stat
import tempfile
import unittest


class FaultFilesystemTests(unittest.TestCase):
    def test_regular_file_reaches_enospc_only_in_verified_disposable_filesystem(self):
        root = Path("/fault-fs")
        info = root.lstat()
        self.assertTrue(stat.S_ISDIR(info.st_mode))
        self.assertEqual(stat.S_IMODE(info.st_mode), 0o700)
        self.assertEqual(info.st_uid, os.getuid())
        mounts = [line.split() for line in Path("/proc/self/mountinfo").read_text().splitlines()]
        mount = [row for row in mounts if row[4] == str(root)]
        self.assertEqual(len(mount), 1)
        self.assertEqual(mount[0][mount[0].index("-") + 1], "tmpfs")
        size = os.statvfs(root)
        self.assertGreater(size.f_blocks * size.f_frsize, 0)
        self.assertLessEqual(size.f_blocks * size.f_frsize, 1024 * 1024)
        # Every assertion above precedes writing. No fallback or host-volume fill.
        with tempfile.TemporaryFile(dir=root) as stream:
            total = 0
            with self.assertRaises(OSError) as caught:
                for _ in range(257):
                    total += os.write(stream.fileno(), b"f" * 4096)
            self.assertEqual(caught.exception.errno, errno.ENOSPC)
            self.assertLessEqual(total, 1024 * 1024)
        self.assertEqual(list(root.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
