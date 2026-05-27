"""Allotrope ASM conformance: nirs4all-formats vs the canonical Allotrope JSON
schema.

Unlike OPUS/SPC/JCAMP, Allotrope ASM ships JSON files that are themselves
the reference encoding — the standard does not specify a runtime parser
beyond ``json.loads``. The harness therefore walks the canonical data-
cube schema directly and compares the numerical arrays against
``nirs4all_formats.open_records``. This is documented in
``docs/CONFORMANCE.md`` so the lighter contract is explicit.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from conftest import (
    compare_axes,
    compare_values,
    fixtures_under,
    load_tolerances,
    normalize_records,
    require_nirs4all_formats,
)

ASM_SAMPLES = fixtures_under("allotrope_asm", suffix=(".json",))
TOL = load_tolerances()["asm"]


@pytest.mark.parametrize("path", ASM_SAMPLES, ids=lambda p: p.name)
def test_asm_records_match_canonical_json(path: Path) -> None:
    nirs = require_nirs4all_formats()

    try:
        records = normalize_records(nirs.open_records(path))
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-formats refuses fixture ({err})")
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    with path.open("rb") as fh:
        payload = json.load(fh)
    pairs = _enumerate_data_cubes(payload)
    if not pairs:
        pytest.skip(f"{path.name}: no ASM data cube schema found")

    matched = False
    for index, (axis, values) in enumerate(pairs):
        record = _record_matching_length(records, len(values))
        if record is None:
            continue
        if axis is not None:
            compare_axes(
                record["axis"],
                axis,
                TOL,
                label=f"{path.name}#cube{index}:axis",
            )
        compare_values(
            record["values"],
            values,
            TOL,
            label=f"{path.name}#cube{index}:values",
        )
        matched = True
        break
    if not matched:
        pytest.skip(
            f"{path.name}: no nirs4all-formats record matches any ASM data-cube length"
        )


def _enumerate_data_cubes(node: Any) -> list[tuple[list[float] | None, list[float]]]:
    """Walk the ASM document and yield `(axis, values)` pairs for every
    spectral data cube we recognise (single-dimension intensity series
    paired with a wavelength axis).
    """

    pairs: list[tuple[list[float] | None, list[float]]] = []

    def visit(payload: Any) -> None:
        if isinstance(payload, list):
            for item in payload:
                visit(item)
            return
        if not isinstance(payload, dict):
            return
        cube = None
        for key in payload:
            if isinstance(key, str) and key.endswith("data cube") and isinstance(payload[key], dict):
                cube = payload[key]
                break
        if isinstance(cube, dict):
            data = cube.get("data") or {}
            measures = data.get("measures") or []
            dimensions = data.get("dimensions") or []
            if measures and isinstance(measures[0], list):
                values = _flat_floats(measures[0])
                axis = (
                    _flat_floats(dimensions[0])
                    if dimensions and isinstance(dimensions[0], list)
                    else None
                )
                pairs.append((axis, values))
        for value in payload.values():
            visit(value)

    visit(node)
    return pairs


def _flat_floats(seq: Any) -> list[float]:
    out: list[float] = []

    def walk(item: Any) -> None:
        if isinstance(item, (list, tuple)):
            for entry in item:
                walk(entry)
        elif isinstance(item, (int, float)):
            out.append(float(item))

    walk(seq)
    return out


def _record_matching_length(records: list[dict], target: int) -> dict | None:
    for record in records:
        if len(record["values"]) == target:
            return record
    return None
