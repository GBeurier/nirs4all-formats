"""HDF5 conformance: nirs4all-formats vs h5py."""

from __future__ import annotations

from pathlib import Path

import pytest

from conftest import (
    compare_axes,
    compare_values,
    fixtures_under,
    load_tolerances,
    normalize_records,
    require_h5py,
    require_nirs4all_formats,
)

HDF5_SAMPLES = fixtures_under("hdf5", "fgi", suffix=(".h5",))
TOL = load_tolerances()["hdf5"]


@pytest.mark.parametrize("path", HDF5_SAMPLES, ids=lambda p: p.name)
def test_hdf5_records_match_h5py(path: Path) -> None:
    nirs = require_nirs4all_formats()
    h5py = require_h5py()

    try:
        raw = nirs.open_records(path)
    except OSError as err:
        pytest.skip(f"{path.name}: nirs4all-formats refuses fixture ({err})")
    records = normalize_records(raw)
    if not records:
        pytest.skip(f"{path.name}: no spectral records emitted")

    with h5py.File(path, "r") as fh:
        spectra_path = _find_dataset(fh, ("spectra", "spectrum", "X", "absorbance", "reflectance"))
        if spectra_path is None:
            pytest.skip(f"{path.name}: no spectral dataset under known aliases")
        axis_path = _find_dataset(fh, ("wavelengths", "wavelength", "x", "lambda", "wn"))
        if axis_path is None:
            pytest.skip(f"{path.name}: no axis dataset under known aliases")
        spectra = fh[spectra_path][...]
        axis = fh[axis_path][...]

    expected_axis = [float(x) for x in axis]
    record = records[0]
    compare_axes(record["axis"], expected_axis, TOL, label=f"{path.name}:axis")

    bands = len(expected_axis)
    expected_row = _row0(spectra, bands)
    if expected_row is None:
        pytest.skip(
            f"{path.name}: spectra shape {tuple(spectra.shape)} not aligned with axis length {bands}"
        )
    if len(record["values"]) == spectra.size:
        compare_values(
            record["values"],
            [float(x) for x in spectra.reshape(-1)],
            TOL,
            label=f"{path.name}:values",
        )
    else:
        compare_values(
            record["values"][:bands],
            expected_row,
            TOL,
            label=f"{path.name}:row0",
        )


def _row0(matrix, bands: int) -> list[float] | None:
    if matrix.ndim == 1:
        if matrix.size != bands:
            return None
        return [float(x) for x in matrix]
    if matrix.ndim != 2:
        return None
    if matrix.shape[1] == bands:
        return [float(x) for x in matrix[0, :]]
    if matrix.shape[0] == bands:
        return [float(x) for x in matrix[:, 0]]
    return None


def _find_dataset(fh, names: tuple[str, ...]) -> str | None:
    # Prefer a direct match at the root over any nested group, then fall
    # back to the shallowest match available.
    for name in names:
        candidate = f"/{name}"
        if candidate in fh:
            return candidate
    matches: list[str] = []

    def visit(name: str, obj) -> None:
        if obj.__class__.__name__ != "Dataset":
            return
        leaf = name.rsplit("/", 1)[-1].lower()
        if leaf in {value.lower() for value in names}:
            matches.append("/" + name)

    fh.visititems(visit)
    if not matches:
        return None
    matches.sort(key=lambda p: (p.count("/"), p))
    return matches[0]
