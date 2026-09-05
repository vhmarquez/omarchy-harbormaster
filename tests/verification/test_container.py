"""Container launch policy tests; real container evidence comes from CI."""
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class ContainerPolicyTests(unittest.TestCase):
    def test_unpinned_image_is_rejected(self):
        self.assertTrue((SCRIPTS / "verification/container.py").is_file(),
                        "container boundary missing")
        from verification.container import command
        with self.assertRaises(ValueError):
            command("fixture", Path("/src"), Path("/tools"), Path("/state"), "latest", {})

    def test_container_has_no_network_privileges_or_host_home(self):
        self.assertTrue((SCRIPTS / "verification/container.py").is_file(),
                        "container boundary missing")
        from verification.container import command
        argv = command("fixture", Path("/src"), Path("/tools"), Path("/state"),
                       "sha256:" + "1" * 64, {"HOME": "/state/home"})
        for flag, value in (("--network", "none"), ("--cap-drop", "ALL"),
                            ("--security-opt", "no-new-privileges"), ("--pids-limit", "256")):
            self.assertEqual(argv[argv.index(flag) + 1], value)
        self.assertIn("--read-only", argv)
        self.assertNotIn("--privileged", argv)
        self.assertIn("HOME=/state/home", argv)
        self.assertTrue(any("dst=/work,readonly" in arg for arg in argv))
        self.assertTrue(any("dst=/tools,readonly" in arg for arg in argv))


if __name__ == "__main__":
    unittest.main()
