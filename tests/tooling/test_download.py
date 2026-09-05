"""Preparation safety tests; no listeners or live network requests."""
import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))


class DownloadTests(unittest.TestCase):
    def test_credential_urls_and_insecure_redirects_are_refused(self):
        from tooling.download import public_https, PublicRedirect
        for url in ("http://example.com/a", "file:///tmp/a", "https://u:p@example.com/a", "https://@example.com/a"):
            with self.subTest(url=url), self.assertRaisesRegex(ValueError, "HTTPS"):
                public_https(url)
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            PublicRedirect().redirect_request(None, None, 302, "redirect", {}, "http://example.com/a")

    def test_wrong_hash_refuses_artifact_and_cleans_partial_download(self):
        self.assertIsNotNone(importlib.util.find_spec("tooling"), "missing tooling package")
        from tooling.download import download
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "artifact"
            with patch("urllib.request.OpenerDirector.open", return_value=io.BytesIO(b"bad")):
                with self.assertRaisesRegex(ValueError, "SHA256"):
                    download("https://example.com/file", hashlib.sha256(b"good").hexdigest(), target)
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_timeout_and_oversize_leave_no_partial_file(self):
        from tooling.download import download
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "artifact"
            for failure in (TimeoutError("network deadline"), io.BytesIO(b"oversize")):
                with self.subTest(failure=failure):
                    options = {"side_effect": failure} if isinstance(failure, Exception) else {"return_value": failure}
                    with patch("urllib.request.OpenerDirector.open", **options):
                        with self.assertRaises((TimeoutError, ValueError)):
                            download("https://example.com/file", "0" * 64, target, max_bytes=1)
                    self.assertFalse(target.exists())

    def test_refuses_symlink_parent_before_network_or_write(self):
        from tooling.download import download
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "real").mkdir()
            (root / "link").symlink_to(root / "real")
            with patch("urllib.request.OpenerDirector.open", return_value=io.BytesIO(b"good")) as request:
                with self.assertRaisesRegex(ValueError, "symlink"):
                    download("https://example.com/file", hashlib.sha256(b"good").hexdigest(), root / "link" / "file")
                request.assert_not_called()
            self.assertEqual(list((root / "real").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
