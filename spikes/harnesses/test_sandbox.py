"""M0 native sandbox integration. Fixtures are disposable, never profiles."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class Helpers(unittest.TestCase):
    def test_sandbox_denies_network_and_inherited_credentials(self):
        spec = importlib.util.spec_from_file_location("probe", HERE / "probe.py")
        self.assertTrue((HERE / "probe.py").exists(), "sandbox helper not implemented")
        assert spec is not None and spec.loader is not None
        probe = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probe)
        with tempfile.TemporaryDirectory() as tmp:
            cmd = probe.sandbox(Path(tmp), {})
            code = 'import os,socket,json; s=socket.socket(); s.settimeout(.1); print(json.dumps({"env":dict(os.environ),"network":s.connect_ex(("1.1.1.1",443)),"real_home_visible":os.path.exists("/home/vhm/.claude")}))'
            r = subprocess.run(cmd + ["/usr/bin/python3", "-c", code], capture_output=True, text=True,
                               env={"PATH": "/usr/bin:/bin", "ANTHROPIC_API_KEY": "synthetic-secret"}, timeout=5)
            self.assertEqual(r.returncode, 0, r.stderr)
            result = json.loads(r.stdout)
            self.assertNotIn("ANTHROPIC_API_KEY", result["env"])
            self.assertNotEqual(result["network"], 0)
            self.assertFalse(result["real_home_visible"])
            self.assertEqual(result["env"]["HERMES_HOME"], "/probe/home/.hermes")


class NetworkNamespace(unittest.TestCase):
    def test_sandbox_uses_distinct_network_namespace(self):
        # The enclosing verifier is already offline: a failed connection alone
        # cannot prove that the inner sandbox created its own network namespace.
        namespace = "/proc/self/ns/net"
        outer_stat = os.stat(namespace)
        outer = {"readlink": os.readlink(namespace),
                 "stat": [outer_stat.st_dev, outer_stat.st_ino]}
        spec = importlib.util.spec_from_file_location("probe", HERE / "probe.py")
        self.assertTrue((HERE / "probe.py").is_file(), "sandbox helper not implemented")
        assert spec is not None and spec.loader is not None
        probe = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probe)
        with tempfile.TemporaryDirectory() as tmp:
            cmd = probe.sandbox(Path(tmp), {})
            code = (
                'import os,json; path="/proc/self/ns/net"; identity=os.stat(path); '
                'print(json.dumps({"readlink":os.readlink(path),'
                '"stat":[identity.st_dev,identity.st_ino]}))'
            )
            result = subprocess.run(cmd + ["/usr/bin/python3", "-B", "-c", code],
                                    capture_output=True, text=True,
                                    env={"PATH": "/usr/bin:/bin"}, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            inner = json.loads(result.stdout)
        print("network namespace identity: " + json.dumps({"outer": outer, "inner": inner},
                                                        sort_keys=True), flush=True)
        self.assertNotEqual(outer["readlink"], inner["readlink"],
                            "inner sandbox shares outer network namespace readlink")
        self.assertNotEqual(outer["stat"], inner["stat"],
                            "inner sandbox shares outer network namespace device/inode")


if __name__ == "__main__":
    unittest.main()
