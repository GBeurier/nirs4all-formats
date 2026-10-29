"""Verify the provenance and license boundary of bundled security backports."""

import hashlib
import json
from pathlib import Path
import tomllib

import pytest


ROOT = Path(__file__).resolve().parents[2]


@pytest.mark.parametrize("reader", ["hdf5-reader", "netcdf-reader"])
def test_backport_preserves_upstream_source_and_bundled_license(reader):
    directory = ROOT / "vendor" / reader
    provenance = json.loads((directory / "PROVENANCE.json").read_text())
    manifest = tomllib.loads((directory / "Cargo.toml").read_text())
    assert manifest["package"]["name"] == provenance["downstream_package"]
    assert manifest["package"]["version"] == provenance["downstream_version"]
    assert provenance["downstream_package"] != provenance["upstream_package"]
    assert len(provenance["upstream_crate_sha256"]) == 64
    actual_sources = {
        str(path.relative_to(directory))
        for path in (directory / "src").rglob("*")
        if path.is_file()
    }
    assert actual_sources == set(provenance["upstream_source_sha256"])
    for name, digest in provenance["upstream_source_sha256"].items():
        assert hashlib.sha256((directory / name).read_bytes()).hexdigest() == digest
    for name, evidence in provenance["bundled_license_texts"].items():
        assert hashlib.sha256((directory / name).read_bytes()).hexdigest() == evidence["sha256"]
        assert provenance["upstream_vcs"]["git"]["sha1"] in evidence["source"]
    assert {"LICENSE-MIT", "LICENSE-APACHE"} == set(provenance["bundled_license_texts"])
    if reader == "hdf5-reader":
        assert manifest["dependencies"]["lru"]["version"] == "=0.18.5"
    else:
        dependency = manifest["dependencies"]["hdf5-reader"]
        assert dependency["package"] == "nirs4all-hdf5-reader"
        assert dependency["version"] == "=0.9.3-nirs4all.1"
        assert dependency["path"] == "../hdf5-reader"
