"""Tests for the lossless record mirror and its N-dimensional projections."""

from __future__ import annotations

import numpy as np
import pytest

from nirs4all_formats import SpectralRecordSet


def _record_2d() -> dict:
    # A 2x3 image slice: dims ["y", "x"], spectral axis "x" (3 points),
    # a "y" coordinate of length 2. Flat values are C-order.
    return {
        "signals": {
            "intensity": {
                "axis": {
                    "values": [900.0, 1000.0, 1100.0],
                    "unit": "nm",
                    "kind": "wavelength",
                    "order": "ascending",
                },
                "values": [0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                "shape": [2, 3],
                "dims": ["y", "x"],
                "coords": {
                    "y": {
                        "values": [10.0, 20.0],
                        "unit": "us",
                        "kind": "time",
                        "order": "ascending",
                    }
                },
                "signal_type": "raw_counts",
                "unit": "counts",
                "role": "intensity",
                "source": "file",
            }
        },
        "signal_type": "raw_counts",
        "targets": {},
        "metadata": {"sample_id": "frame_0"},
        "provenance": {
            "format": "test-2d",
            "reader": "test",
            "reader_version": "0",
            "sources": [],
            "record_schema_version": "0.2.0",
            "warnings": [],
        },
        "quality_flags": [],
    }


def test_nd_array_is_reshaped_losslessly() -> None:
    rs = SpectralRecordSet.from_dicts([_record_2d()])
    array = rs.records[0].signals["intensity"]

    assert array.shape == (2, 3)
    assert array.dims == ("y", "x")
    assert array.ndim == 2
    assert array.x_dim_index == 1
    # C-order reshape: row 0 is the first three values.
    assert array.values.tolist() == [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]
    assert array.coords["y"].values.tolist() == [10.0, 20.0]


def test_nd_projection_flattens_non_x_dims_into_samples() -> None:
    rs = SpectralRecordSet.from_dicts([_record_2d()])

    x, axis = rs.to_numpy(signal="intensity")
    # 2 rows (the "y" dimension) x 3 features (the "x" dimension).
    assert x.shape == (2, 3)
    assert axis.tolist() == [900.0, 1000.0, 1100.0]
    assert x[0].tolist() == [0.0, 1.0, 2.0]
    assert x[1].tolist() == [3.0, 4.0, 5.0]


def test_nd_projection_records_sample_coordinates() -> None:
    import pandas as pd  # noqa: F401  (ensures pandas available)

    rs = SpectralRecordSet.from_dicts([_record_2d()])
    frame = rs.to_pandas(signal="intensity")

    assert frame.shape[0] == 2
    assert "coord_y" in frame.columns
    assert frame["coord_y"].tolist() == [10.0, 20.0]


def test_to_pandas_long_lists_every_point() -> None:
    pytest.importorskip("pandas")
    rs = SpectralRecordSet.from_dicts([_record_2d()])
    long = rs.to_pandas_long()

    # 2 samples x 3 points = 6 rows for the single signal.
    assert len(long) == 6
    assert set(long["signal"]) == {"intensity"}
    assert long["value"].tolist() == [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]


def test_to_polars_matches_pandas_columns() -> None:
    pl = pytest.importorskip("polars")
    pytest.importorskip("pandas")
    rs = SpectralRecordSet.from_dicts([_record_2d()])

    pdf = rs.to_pandas(signal="intensity")
    pldf = rs.to_polars(signal="intensity")

    assert isinstance(pldf, pl.DataFrame)
    assert pldf.height == pdf.shape[0] == 2
    assert list(pldf.columns) == list(pdf.columns)
    assert pldf["coord_y"].to_list() == [10.0, 20.0]


def test_to_xarray_when_available() -> None:
    xr = pytest.importorskip("xarray")
    rs = SpectralRecordSet.from_dicts([_record_2d()])
    da = rs.records[0].signals["intensity"].to_xarray()

    assert isinstance(da, xr.DataArray)
    assert da.dims == ("y", "x")
    assert da.shape == (2, 3)


def test_to_torch_when_available() -> None:
    torch = pytest.importorskip("torch")
    payload = [
        {
            "signals": {
                "absorbance": {
                    "axis": {
                        "values": [1.0, 2.0],
                        "unit": "nm",
                        "kind": "wavelength",
                        "order": "ascending",
                    },
                    "values": [0.1, 0.2],
                    "shape": [2],
                    "dims": ["x"],
                    "signal_type": "absorbance",
                    "unit": None,
                    "role": "absorbance",
                    "source": "file",
                }
            },
            "signal_type": "absorbance",
            "targets": {"y": 1.0},
            "metadata": {},
            "provenance": {
                "format": "t",
                "reader": "t",
                "reader_version": "0",
                "sources": [],
                "record_schema_version": "0.2.0",
                "warnings": [],
            },
            "quality_flags": [],
        }
    ]
    rs = SpectralRecordSet.from_dicts(payload)
    dataset = rs.to_torch(signal="absorbance", target="y")
    assert len(dataset) == 1
    item = dataset[0]
    assert item[0].dtype == torch.float32
