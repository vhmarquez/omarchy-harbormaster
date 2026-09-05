"""Native QML tools: formatting/lint failures cannot be hidden by exit 0."""
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class NativeTests(unittest.TestCase):
    def test_empty_qml_inventory_fails(self):
        self.assertTrue((SCRIPTS / "verification/native.py").is_file(), "native gate missing")
        from verification.native import verify
        with self.assertRaises(ValueError):
            verify("lint", [])

    def test_format_gate_detects_noncanonical_qml(self):
        self.assertTrue((SCRIPTS / "verification/native.py").is_file(), "native gate missing")
        from verification.native import verify
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Fixture.qml"
            path.write_text("import QtQuick\nItem {width:64}\n")
            self.assertFalse(verify("format", [path]))


    def test_wrong_native_version_cannot_pass(self):
        from verification import native
        self.assertTrue(hasattr(native, "versions"), "native pin check missing")
        self.assertFalse(native.versions({"qt": "0.0.0", "python": "0.0.0"}))


if __name__ == "__main__":
    unittest.main()
