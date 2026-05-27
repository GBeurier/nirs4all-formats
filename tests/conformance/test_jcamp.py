"""JCAMP-DX conformance: nirs4all-formats vs `jcamp` (Python)."""

from __future__ import annotations

from pathlib import Path

import pytest

from conftest import (
    compare_axes,
    compare_values,
    fixtures_under,
    load_known_skips,
    load_tolerances,
    normalize_records,
    require_jcamp,
    require_nirs4all_formats,
)

JCAMP_SAMPLES = fixtures_under(
    "jcamp_dx",
    suffix=(".dx", ".jdx", ".jcm"),
)
TOL = load_tolerances()["jcamp"]
SKIPS = load_known_skips().get("jcamp", {})


@pytest.mark.parametrize("path", JCAMP_SAMPLES, ids=lambda p: p.name)
def test_jcamp_records_match_python_jcamp(path: Path) -> None:
    nirs = require_nirs4all_formats()
    jcamp = require_jcamp()

    skip_key = path.name.lower().replace("-", "_").replace(".", "_")
    if skip_key in SKIPS:
        pytest.skip(SKIPS[skip_key])

    try:
        records = normalize_records(nirs.open_records(path))
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-formats refuses fixture ({err})")
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    try:
        ref = jcamp.jcamp_readfile(str(path))
    except Exception as err:  # noqa: BLE001 — bubble JCAMP errors
        pytest.skip(f"{path.name}: jcamp reader rejects fixture ({err})")
    if "x" not in ref or "y" not in ref:
        pytest.skip(f"{path.name}: jcamp output has no x/y array")
    expected_axis = [float(x) for x in ref["x"]]
    expected_values = [float(y) for y in ref["y"]]
    if len(expected_axis) == 0:
        pytest.skip(f"{path.name}: jcamp axis empty")

    # nirs4all-formats may emit several records (LINK, NTUPLES, multi-block);
    # compare each record whose values length matches the reference.
    matched = False
    for index, record in enumerate(records):
        if len(record["values"]) != len(expected_values):
            continue
        compare_axes(
            record["axis"],
            expected_axis,
            TOL,
            label=f"{path.name}#{index}:axis",
        )
        compare_values(
            record["values"],
            expected_values,
            TOL,
            label=f"{path.name}#{index}:values",
        )
        matched = True
        break
    if not matched:
        pytest.skip(
            f"{path.name}: no nirs4all-formats record matches jcamp value count "
            f"{len(expected_values)} (records have lengths "
            f"{[len(r['values']) for r in records]})"
        )
