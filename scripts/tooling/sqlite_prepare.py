"""Install only the two reviewed SQLite source files plus their exact archive."""
from pathlib import Path
import zipfile

from .download import download
from .install import MARKER, owned_stage
from .sqlite_source import read_pins, verify_sqlite


def prepare_sqlite(project, tools):
    project, tools = Path(project), Path(tools)
    pins = read_pins(project)
    with owned_stage(tools, "sqlite-source") as stage:
        group = stage / "sqlite-source"
        group.mkdir(mode=0o700)
        archive = group / pins["url"].rsplit("/", 1)[1]
        download(pins["url"], pins["archive_sha256"], archive, max_bytes=5_000_000)
        with zipfile.ZipFile(archive) as source:
            members = source.infolist()
            if len(members) > 10 or sum(item.file_size for item in members) > 15_000_000:
                raise ValueError("SQLite archive expansion limit exceeded")
            if len({item.filename for item in members}) != len(members):
                raise ValueError("duplicate SQLite archive member")
            for item in pins["files"]:
                member = source.getinfo(item["path"])
                if member.is_dir() or member.file_size > 11_000_000:
                    raise ValueError("invalid SQLite source member")
                # Never extract archive paths, permissions, links or executables.
                (group / Path(item["path"]).name).write_bytes(source.read(member))
        (stage / MARKER).write_bytes((tools / MARKER).read_bytes())
        (stage / MARKER).chmod(0o600)
        report = verify_sqlite(project, stage)
    return report
