"""Disposable identity gate; no dispatch or process signalling here."""
import re


def exact_window(expected, windows, process, compositor):
    if (not re.fullmatch(r"0x[0-9a-fA-F]+", expected["address"])
            or process is None or compositor != expected["compositor"]
            or process.get("state") in ("Z", "X", "x")
            or any(process.get(key) != expected[key]
                   for key in ("pid", "start_ticks", "boot_id"))):
        return None
    matches = [window for window in windows
               if window.get("address") == expected["address"]
               and window.get("pid") == expected["pid"]
               and window.get("class") == expected["app_id"]]
    return matches[0]["address"] if len(matches) == 1 else None
