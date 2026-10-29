"""Add licensed reader sources and provenance to a generated WASM npm package."""

import argparse
import json
from pathlib import Path
import shutil


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    package = args.package.resolve()
    manifest_path = package / "package.json"
    manifest = json.loads(manifest_path.read_text())
    for reader in ("hdf5-reader", "netcdf-reader"):
        source = root / "vendor" / reader
        target = package / "reader-backports" / reader
        target.mkdir(parents=True, exist_ok=True)
        for name in ("Cargo.toml", "UPSTREAM-Cargo.toml", "README.md", "PROVENANCE.json", "LICENSE-MIT", "LICENSE-APACHE"):
            shutil.copyfile(source / name, target / name)
        shutil.copytree(source / "src", target / "src", dirs_exist_ok=True)
    shutil.copyfile(root / "THIRD_PARTY_NOTICES.md", package / "THIRD_PARTY_NOTICES.md")
    manifest["files"] = list(dict.fromkeys([*manifest["files"], "reader-backports", "THIRD_PARTY_NOTICES.md"]))
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
