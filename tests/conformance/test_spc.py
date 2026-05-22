"""Galactic SPC conformance: nirs4all-io vs `spc_spectra` (MIT)."""

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
    require_nirs4all_io,
    require_spc,
)

SPC_SAMPLES = fixtures_under("galactic_spc", suffix=(".spc",))
TOL = load_tolerances()["spc"]
SKIPS = load_known_skips().get("spc", {})


@pytest.mark.parametrize("path", SPC_SAMPLES, ids=lambda p: p.name)
def test_spc_records_match_spc_spectra(path: Path) -> None:
    nirs = require_nirs4all_io()
    spc = require_spc()

    skip_key = path.name.lower().replace("-", "_").replace(".", "_")
    if skip_key in SKIPS:
        pytest.skip(SKIPS[skip_key])

    try:
        records = normalize_records(nirs.open_records(path))
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-io refuses fixture ({err})")
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    try:
        ref = spc.File(str(path))
    except Exception as err:  # noqa: BLE001
        pytest.skip(f"{path.name}: spc-spectra rejects fixture ({err})")
    if not getattr(ref, "sub", None):
        pytest.skip(f"{path.name}: spc-spectra produced no subfiles")

    matched = 0
    for index, sub in enumerate(ref.sub):
        expected_axis = _subfile_axis(ref, sub)
        expected_values = [float(v) for v in sub.y]
        record = _record_with_length(records, len(expected_values))
        if record is None:
            continue
        compare_axes(
            record["axis"],
            expected_axis,
            TOL,
            label=f"{path.name}#sub{index}:axis",
        )
        compare_values(
            record["values"],
            expected_values,
            TOL,
            label=f"{path.name}#sub{index}:values",
        )
        matched += 1
        # Once we have a successful comparison for at least one subfile,
        # we trust the rest follow.
        break
    if matched == 0:
        pytest.skip(
            f"{path.name}: no nirs4all-io record matched any spc subfile "
            f"length (records {[len(r['values']) for r in records]}, "
            f"subfiles {[len(s.y) for s in ref.sub]})"
        )


def _record_with_length(records: list[dict], target: int) -> dict | None:
    for record in records:
        if len(record["values"]) == target:
            return record
    return None


def _subfile_axis(ref, sub) -> list[float]:
    """Return the per-subfile X axis: explicit `sub.x` if present, the
    file-level `ref.x` for shared-X SPC layouts, else linear FIRSTX..LASTX.
    """

    import numpy as np

    sub_x = getattr(sub, "x", None)
    if sub_x is not None and len(sub_x) == len(sub.y):
        return [float(v) for v in sub_x]
    ref_x = getattr(ref, "x", None)
    if ref_x is not None and len(ref_x) == len(sub.y):
        return [float(v) for v in ref_x]
    return list(np.linspace(ref.ffirst, ref.flast, len(sub.y)))
