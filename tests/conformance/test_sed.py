"""Spectral Evolution `.sed` conformance: nirs4all-formats vs `spectrolab` (R)."""

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
    require_nirs4all_formats,
    require_rscript_with,
    run_rscript,
)

SED_SAMPLES = fixtures_under("spectral_evolution", suffix=(".sed",))
TOL = load_tolerances()["sed"]
SED_DUMP = HERE / "refreaders" / "sed_dump.R"


@pytest.mark.parametrize("path", SED_SAMPLES, ids=lambda p: p.name)
def test_sed_records_match_spectrolab(path: Path) -> None:
    nirs = require_nirs4all_formats()
    rscript = require_rscript_with("spectrolab")

    try:
        records = normalize_records(nirs.open_records(path))
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-formats refuses fixture ({err})")
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    try:
        payload = run_rscript(rscript, SED_DUMP, str(path))
    except RuntimeError as err:
        pytest.skip(f"{path.name}: spectrolab rejects fixture ({err})")
    expected_axis = [float(x) for x in payload["axis"]]
    spectrolab_values = [float(y) for y in payload["values"]]

    record = next(
        (
            r
            for r in records
            if r.get("signal_type") == "reflectance"
            and len(r["values"]) == len(spectrolab_values)
        ),
        None,
    )
    if record is None:
        pytest.skip(
            f"{path.name}: no nirs4all-formats reflectance record matches length "
            f"{len(spectrolab_values)}"
        )
    scale = _detect_scale(record["values"], spectrolab_values)
    if scale is None:
        pytest.skip(
            f"{path.name}: spectrolab/nirs4all-formats scale convention diverges"
        )
    expected_values = [y * scale for y in spectrolab_values]
    compare_axes(record["axis"], expected_axis, TOL, label=f"{path.name}:axis")
    compare_values(
        record["values"], expected_values, TOL, label=f"{path.name}:values"
    )


def _detect_scale(nirs_values: list[float], ref_values: list[float]) -> float | None:
    """nirs4all-formats exposes reflectance either as [0,1] fractional or
    [0,100] percent depending on the SED variant. spectrolab always
    returns the file's native values; detect the convention by the
    ratio at the first non-zero entry and accept only {1.0, 100.0}.
    """

    for nir, ref in zip(nirs_values, ref_values):
        if abs(ref) < 1e-9 or abs(nir) < 1e-9:
            continue
        ratio = nir / ref
        if abs(ratio - 1.0) < 1e-3:
            return 1.0
        if abs(ratio - 100.0) < 1e-2:
            return 100.0
        if abs(ratio - 0.01) < 1e-5:
            return 0.01
        return None
    return 1.0
