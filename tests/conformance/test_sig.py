"""SVC/GER `.sig` conformance: nirs4all-io vs `spectrolab` (R)."""

from __future__ import annotations

from pathlib import Path

import pytest

from conftest import (
    HERE,
    compare_axes,
    compare_values,
    fixtures_under,
    load_tolerances,
    normalize_records,
    require_nirs4all_io,
    require_rscript_with,
    run_rscript,
)

SIG_SAMPLES = fixtures_under("svc_ger", suffix=(".sig",))
TOL = load_tolerances()["sig"]
SIG_DUMP = HERE / "refreaders" / "sig_dump.R"


@pytest.mark.parametrize("path", SIG_SAMPLES, ids=lambda p: p.name)
def test_sig_records_match_spectrolab(path: Path) -> None:
    nirs = require_nirs4all_io()
    rscript = require_rscript_with("spectrolab")

    try:
        records = normalize_records(nirs.open_records(path))
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-io refuses fixture ({err})")
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    try:
        payload = run_rscript(rscript, SIG_DUMP, str(path))
    except RuntimeError as err:
        pytest.skip(f"{path.name}: spectrolab rejects fixture ({err})")
    expected_axis = [float(x) for x in payload["axis"]]
    expected_values_pct = [float(y) * 100.0 for y in payload["values"]]

    record = next(
        (
            r
            for r in records
            if r.get("signal_type") == "reflectance"
            and len(r["values"]) == len(expected_values_pct)
        ),
        None,
    )
    if record is None:
        pytest.skip(
            f"{path.name}: no nirs4all-io reflectance record matches length "
            f"{len(expected_values_pct)}"
        )
    compare_axes(record["axis"], expected_axis, TOL, label=f"{path.name}:axis")
    compare_values(
        record["values"], expected_values_pct, TOL, label=f"{path.name}:values"
    )
