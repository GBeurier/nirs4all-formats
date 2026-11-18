"""Sidecar-aware decoding through the PyO3 bridge."""

from __future__ import annotations

from pathlib import Path

import pytest
from nirs4all_formats import open_records, open_with_sidecars

REPO_ROOT = Path(__file__).resolve().parents[3]


def _read_fixture(*parts: str) -> Path:
    return REPO_ROOT.joinpath(*parts)


def _records_match(from_bytes, from_path, label: str) -> None:
    assert len(from_bytes) == len(from_path), f"{label}: record count"
    for a, b in zip(from_bytes, from_path):
        assert a["signal_type"] == b["signal_type"], f"{label}: signal_type"
        assert set(a["signals"]) == set(b["signals"]), f"{label}: signals keys"
        for key, sig_b in a["signals"].items():
            sig_p = b["signals"][key]
            assert sig_b["values"] == sig_p["values"], f"{label}: {key} values"
            assert sig_b["axis"]["values"] == sig_p["axis"]["values"], f"{label}: {key} axis"
        assert a["metadata"] == b["metadata"], f"{label}: metadata"
        assert a["targets"] == b["targets"], f"{label}: targets"


@pytest.mark.parametrize(
    "primary, sidecars",
    [
        (
            "samples/envi_sli/cubescope-mini-cube.img",
            ["cubescope-mini-cube.hdr"],
        ),
        (
            "samples/envi_sli/synthetic_lib.sli",
            ["synthetic_lib.hdr"],
        ),
        (
            "samples/hyperspectral_cubes/92AV3C.lan",
            ["92AV3C.spc", "92AV3GT.GIS"],
        ),
        ("samples/fgi/synthetic_fgi.xml", ["synthetic_fgi.h5"]),
        ("samples/matlab/synthetic_nirs_v73.mat", []),
        ("samples/foss_nsas_da/references700.da", ["references700.cn"]),
    ],
)
def test_open_with_sidecars_matches_open_path(primary: str, sidecars: list[str]) -> None:
    primary_path = _read_fixture(primary)
    if not primary_path.exists():
        pytest.skip(f"fixture missing: {primary_path}")
    primary_bytes = primary_path.read_bytes()
    sidecar_map = {name: (primary_path.parent / name).read_bytes() for name in sidecars}
    from_bytes = open_with_sidecars(primary_path.name, primary_bytes, sidecar_map)
    from_path = open_records(primary_path)
    _records_match(from_bytes, from_path, primary)


def test_nsas_cn_references_reach_the_python_record_model() -> None:
    primary = _read_fixture("samples/foss_nsas_da/references700.da")
    companion = primary.with_suffix(".cn")
    records = open_with_sidecars(
        primary.name, primary.read_bytes(), {companion.name: companion.read_bytes()}
    )
    assert [r["targets"] for r in records] == [
        {"water": 1.25, "oil": 0.0},
        {"water": 0.0, "oil": 3.25},
        {"water": 2.5, "oil": 1.5},
    ]
    assert records[0]["metadata"]["da_constituent_count"] == 2
    assert records[0]["signals"]["spectrum"]["axis"]["values"][-1] == 2498.0
    assert records[0]["provenance"]["sources"][1]["role"] == "constituent_reference"
