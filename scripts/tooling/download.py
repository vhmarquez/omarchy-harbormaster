"""Bounded, credential-free HTTPS downloads of hash-pinned public artifacts."""
import hashlib
from pathlib import Path
import re
import time
import urllib.parse
import urllib.request


from .install import no_symlinks


def public_https(url):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username is not None or parsed.password is not None:
        raise ValueError("artifact URL must be public HTTPS without credentials")
    return url


class PublicRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        public_https(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download(url, sha256, target, *, timeout=30, max_bytes=300_000_000, deadline=600):
    public_https(url)
    if not re.fullmatch(r"[0-9a-f]{64}", sha256):
        raise ValueError("invalid SHA256 pin")
    target = Path(target)
    no_symlinks(target)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), PublicRedirect())
    digest = hashlib.sha256()
    total = 0
    started = time.monotonic()
    created = False
    try:
        with target.open("xb") as output:
            created = True
            with opener.open(url, timeout=timeout) as response:
                while chunk := response.read1(64 * 1024):
                    total += len(chunk)
                    if total > max_bytes or time.monotonic() - started > deadline:
                        raise ValueError("download exceeded byte/time limit")
                    digest.update(chunk)
                    output.write(chunk)
        if digest.hexdigest() != sha256:
            raise ValueError(f"SHA256 mismatch for {url}: expected {sha256}, got {digest.hexdigest()}")
    except BaseException:
        if created:
            target.unlink(missing_ok=True)
        raise
    return {"url": url, "sha256": digest.hexdigest(), "bytes": total}
