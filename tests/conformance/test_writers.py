"""Exported files read by independent libraries.

Every written spectrum is compared with the records it came from, so a writer
error mirrored by our own reader cannot go unnoticed.
"""

from __future__ import annotations

import io
import math
from pathlib import Path
from typing import Any

import pytest

from conftest import SAMPLES_ROOT, require_nirs4all_formats, require_spc


def _samples() -> list[Path]:
    """Two files per sample family; unreadable ones are skipped by the tests."""

    out: list[Path] = []
    for family in sorted(path for path in SAMPLES_ROOT.iterdir() if path.is_dir()):
        files = sorted(p for p in family.iterdir() if p.is_file() and p.suffix != ".md")
        out.extend(files[:2])
    return out


def _spectra(path: Path) -> list[dict[str, Any]]:
    formats = require_nirs4all_formats()
    try:
        records = formats.open_records(path)[:3]
    except (OSError, ValueError) as error:
        pytest.skip(f"{path.name}: not a standalone spectral file ({error})")
    if not records:
        pytest.skip(f"{path.name}: no records")
    out = []
    for record in records:
        for signal in record["signals"].values():
            values = signal["values"]
            if signal["dims"] != ["x"] or any(v is None or not math.isfinite(v) for v in values):
                pytest.skip(f"{path.name}: not a finite one-dimensional signal")
            out.append({"x": signal["axis"]["values"], "y": values})
    return out


def _write(path: Path, target: str, **options: Any) -> tuple[list[dict[str, Any]], float, float]:
    formats = require_nirs4all_formats()
    output = formats.write_records(formats.open_records(path)[:3], target, **options)
    errors = {loss["code"]: loss.get("max_error", 0.0) for loss in output["losses"]}
    axis = errors.get("axis_rounded_f32", 0.0)
    ordinate = errors.get("ordinate_rounded_f32", errors.get("ordinate_quantized", 0.0))
    return output["files"], axis, ordinate


def _close(actual: Any, expected: list[float], error: float, label: str) -> None:
    actual = [float(value) for value in actual]
    assert len(actual) == len(expected), label
    for index, (left, right) in enumerate(zip(actual, expected, strict=True)):
        bound = error * (1 + 1e-9) + 1e-10 * max(abs(left), abs(right))
        assert abs(left - right) <= bound, f"{label}: point {index}: {left} != {right}"


@pytest.mark.parametrize("path", _samples(), ids=lambda p: f"{p.parent.name}/{p.name}")
def test_jcamp_matches_independent_reader(path: Path, tmp_path: Path) -> None:
    jcamp = pytest.importorskip("jcamp")
    spectra = _spectra(path)
    files, axis_error, value_error = _write(path, "jcamp-dx")
    assert len(files) == len(spectra)
    for file, spectrum in zip(files, spectra, strict=True):
        target = tmp_path / file["name"]
        target.write_bytes(file["bytes"])
        read = jcamp.readfile(str(target))
        _close(read["x"], spectrum["x"], axis_error, f"{file['name']}:axis")
        _close(read["y"], spectrum["y"], value_error, f"{file['name']}:values")


@pytest.mark.parametrize("multi", [False, True], ids=["single", "multi"])
@pytest.mark.parametrize("path", _samples(), ids=lambda p: f"{p.parent.name}/{p.name}")
def test_spc_matches_independent_reader(path: Path, multi: bool, tmp_path: Path) -> None:
    spc = require_spc()
    spectra = _spectra(path)
    files, axis_error, value_error = _write(path, "galactic-spc", multi=multi)
    read = []
    for file in files:
        target = tmp_path / file["name"]
        target.write_bytes(file["bytes"])
        parsed = spc.File(str(target))
        read.extend({"x": list(parsed.x), "y": list(sub.y)} for sub in parsed.sub)
    assert len(read) == len(spectra)
    if multi:
        # Files group spectra by signal; compare them as a set keyed by content order.
        key = lambda item: (len(item["y"]), [round(float(v), 6) for v in item["y"]])  # noqa: E731
        read.sort(key=key)
        spectra.sort(key=key)
    for actual, expected in zip(read, spectra, strict=True):
        _close(actual["x"], expected["x"], axis_error, f"{path.name}:axis")
        _close(actual["y"], expected["y"], value_error, f"{path.name}:values")


@pytest.mark.parametrize("path", _samples(), ids=lambda p: f"{p.parent.name}/{p.name}")
def test_workbook_matches_independent_reader(path: Path) -> None:
    openpyxl = pytest.importorskip("openpyxl")
    spectra = _spectra(path)
    if any(len(spectrum["x"]) > 16_000 for spectrum in spectra):
        with pytest.raises(ValueError, match="exceed the Excel limits"):
            _write(path, "xlsx")
        return
    files, _, _ = _write(path, "xlsx")
    read = []
    for file in files:
        book = openpyxl.load_workbook(io.BytesIO(file["bytes"]), read_only=True)
        assert book.sheetnames == ["spectra", "info"]
        rows = list(book["spectra"].iter_rows(values_only=True))
        start = sum(isinstance(cell, str) for cell in rows[0])
        for row in rows[1:]:
            read.append((list(rows[0][start:]), list(row[start : len(rows[0])])))
    key = lambda item: (len(item[1]), [float(v) for v in item[1]])  # noqa: E731
    wanted = sorted(((s["x"], s["y"]) for s in spectra), key=key)
    for (x, y), (wanted_x, wanted_y) in zip(sorted(read, key=key), wanted, strict=True):
        assert [float(v) for v in x] == wanted_x
        assert [float(v) for v in y] == wanted_y
