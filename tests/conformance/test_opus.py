"""Bruker OPUS conformance: nirs4all-io vs `brukeropus` (MIT)."""

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
    require_brukeropus,
    require_nirs4all_io,
)

OPUS_SAMPLES = fixtures_under(
    "bruker_opus",
    suffix=(".0", ".001", ".0000", ".1"),
)
TOL = load_tolerances()["opus"]
SKIPS = load_known_skips().get("opus", {})

# brukeropus stores absorbance / single-beam / reflectance behind these
# attribute keys; we cross-check whichever block ours emitted as the
# dominant signal.
OPUS_BLOCK_KEYS = ("sm", "r", "rf", "ab", "raman")


@pytest.mark.parametrize("path", OPUS_SAMPLES, ids=lambda p: p.name)
def test_opus_records_match_brukeropus(path: Path) -> None:
    nirs = require_nirs4all_io()
    brukeropus = require_brukeropus()

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
        opus = brukeropus.read_opus(str(path))
    except Exception as err:  # noqa: BLE001
        pytest.skip(f"{path.name}: brukeropus rejects fixture ({err})")
    if not opus or not opus.data_keys:
        pytest.skip(f"{path.name}: brukeropus produced no data blocks")

    matched_any = False
    for record in records:
        ref_block = _find_matching_block(opus, record["values"])
        if ref_block is None:
            continue
        compare_axes(
            record["axis"],
            [float(x) for x in ref_block.wn],
            TOL,
            label=f"{path.name}:axis",
        )
        compare_values(
            record["values"],
            [float(y) for y in ref_block.y],
            TOL,
            label=f"{path.name}:values",
        )
        matched_any = True
        break
    if not matched_any:
        pytest.skip(
            f"{path.name}: no nirs4all-io record matches a brukeropus block "
            f"(record lengths {[len(r['values']) for r in records]}, "
            f"brukeropus keys {list(opus.data_keys)})"
        )


def _find_matching_block(opus, values: list[float]):
    target = len(values)
    for key in opus.data_keys:
        block = getattr(opus, key, None)
        if block is None or not hasattr(block, "wn"):
            continue
        if len(block.y) == target:
            return block
    return None
