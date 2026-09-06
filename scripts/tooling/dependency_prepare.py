"""Explicit online download only; dependency code never executes during preparation."""
from pathlib import Path

from .dependencies import INDEX_BASE, REGISTRY, cache_record, index_path, read_pins, verify_dependencies
from .download import download
from .install import owned_stage


def prepare_dependencies(project, tools):
    project, tools = Path(project), Path(tools)
    pins = read_pins(project)
    with owned_stage(tools, "dependencies") as stage:
        group = stage / "dependencies"
        indexes = group / "indexes"
        indexes.mkdir(parents=True)
        registry = group / "cargo/registry"
        index = registry / "index" / REGISTRY
        index.mkdir(parents=True)
        archives = registry / "cache" / REGISTRY
        archives.mkdir(parents=True)
        download(INDEX_BASE + pins["index_commit"] + "/config.json", pins["config_sha256"],
                 index / "config.json", max_bytes=10_000_000)
        for package in pins["packages"]:
            name, version = package["name"], package["version"]
            raw = indexes / name
            download(INDEX_BASE + pins["index_commit"] + "/" + index_path(name),
                     package["index_sha256"], raw, max_bytes=10_000_000)
            cached = index / ".cache" / index_path(name)
            cached.parent.mkdir(parents=True, exist_ok=True)
            cached.write_bytes(cache_record(raw.read_bytes()))
            download(f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                     package["checksum"], archives / f"{name}-{version}.crate", max_bytes=25_000_000)
        # Validate before owned_stage atomically installs the complete group.
        # The staging root shares the owner's marker solely for this check.
        (stage / ".harbormaster-tools.json").write_bytes((tools / ".harbormaster-tools.json").read_bytes())
        (stage / ".harbormaster-tools.json").chmod(0o600)
        report = verify_dependencies(project, stage)
    return report
