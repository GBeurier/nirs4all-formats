import sys
from pathlib import Path

import pytest

from nirs4all_io import (
    SpectralRecordSet,
    open_bytes,
    open_records,
    open_recordset,
    probe_path,
    walk_path,
)
from nirs4all_io._compat import _native


def test_open_records_uses_rust_backend() -> None:
    records = open_records(sample("samples/csv_tsv/synthetic_nirs.csv"))

    assert len(records) == 50
    assert records[0]["provenance"]["format"] == "delimited-text"


def test_recordset_is_lossless_mirror() -> None:
    rs = open_recordset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    assert isinstance(rs, SpectralRecordSet)
    assert len(rs) == 50
    record = rs.records[0]
    assert record.provenance.format == "delimited-text"
    signal = next(iter(record.signals.values()))
    # 1-D spectrum: shape == [n], dims == ["x"], coords empty, axis preserved.
    assert signal.dims == ("x",)
    assert signal.shape == (signal.axis.values.shape[0],)
    assert signal.coords == {}
    assert signal.values.shape == signal.axis.values.shape


def test_to_numpy_and_sklearn_shapes() -> None:
    rs = open_recordset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    x, axis = rs.to_numpy()
    bunch = rs.to_sklearn()

    assert x.shape == (50, 200)
    assert axis.shape == (200,)
    assert bunch.data.shape == (50, 200)
    assert bunch.target.shape == (50,)
    assert bunch.target_name == "protein"


def test_to_pandas_carries_provenance_columns() -> None:
    pytest.importorskip("pandas")
    rs = open_recordset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    frame = rs.to_pandas()
    assert frame.shape[0] == 50
    assert "nirs4all_io.format" in frame.columns
    assert frame["nirs4all_io.format"].iloc[0] == "delimited-text"
    assert "protein" in frame.columns


def test_heterogeneous_axes_raise_in_strict_projection() -> None:
    # Two records with different feature axes must refuse to project.
    payload = [
        _fake_record(axis=[1.0, 2.0, 3.0], values=[0.1, 0.2, 0.3]),
        _fake_record(axis=[1.0, 2.0, 3.0, 4.0], values=[0.1, 0.2, 0.3, 0.4]),
    ]
    rs = SpectralRecordSet.from_dicts(payload)
    with pytest.raises(ValueError, match="different feature axes"):
        rs.to_numpy(signal="absorbance")


def test_missing_signal_is_nan_filled() -> None:
    import numpy as np

    payload = [
        _fake_record(axis=[1.0, 2.0], values=[0.1, 0.2]),
        _fake_record(axis=[1.0, 2.0], values=[0.3, 0.4], signal="reflectance"),
    ]
    rs = SpectralRecordSet.from_dicts(payload)
    x, _axis = rs.to_numpy(signal="absorbance")
    assert x.shape == (2, 2)
    assert np.isnan(x[1]).all()  # second record lacks "absorbance"


def test_nirs4all_spectrodataset_when_checkout_is_available() -> None:
    checkout = Path("/home/delete/nirs4all/nirs4all")
    if not checkout.exists():
        pytest.skip("nirs4all checkout not available")
    pytest.importorskip("nirs4all")
    sys.path.insert(0, str(checkout))
    rs = open_recordset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    spectro_dataset = rs.to_spectrodataset(name="smoke")

    assert type(spectro_dataset).__name__ == "SpectroDataset"
    assert spectro_dataset.x(None).shape == (50, 200)


def test_probe_path_returns_candidates() -> None:
    probes = probe_path(sample("samples/csv_tsv/synthetic_nirs.csv"))
    assert probes
    assert probes[0]["format"] == "delimited-text"


def test_walk_path_returns_outcomes() -> None:
    entries = walk_path(sample("samples/asd"))
    assert entries
    assert all(entry["status"] == "parsed" for entry in entries)
    assert {entry["format"] for entry in entries} == {"asd-fieldspec"}


def test_native_extension_is_built_for_this_wheel() -> None:
    assert _native is not None, "native PyO3 extension was not built"
    records = _native.open_path(
        str(sample("samples/hyperspectral_cubes/92AV3C.lan")),
        pixels=[(0, 0), (10, 10)],
    )
    assert len(records) == 2
    assert records[0]["metadata"]["sample_id"] == "pixel_y0_x0"


def test_open_bytes_matches_open_records_for_text_fixture() -> None:
    path = sample("samples/csv_tsv/synthetic_nirs.csv")
    payload = path.read_bytes()
    from_records = open_records(path)
    from_bytes = open_bytes(path.name, payload)
    assert len(from_bytes) == len(from_records) == 50
    signal_key = next(iter(from_records[0]["signals"]))
    assert (
        from_bytes[0]["signals"][signal_key]["values"]
        == from_records[0]["signals"][signal_key]["values"]
    )


def test_open_bytes_works_for_binary_jcamp_and_asd() -> None:
    for relative in (
        "samples/jcamp_dx/TESTSPEC.DX",
        "samples/asd/soil.asd",
    ):
        path = sample(relative)
        payload = path.read_bytes()
        records = open_bytes(path.name, payload)
        assert records, relative
        assert records[0]["signals"], relative


def _fake_record(
    *, axis: list[float], values: list[float], signal: str = "absorbance"
) -> dict:
    return {
        "signals": {
            signal: {
                "axis": {"values": axis, "unit": "nm", "kind": "wavelength", "order": "ascending"},
                "values": values,
                "shape": [len(values)],
                "dims": ["x"],
                "signal_type": signal,
                "unit": None,
                "role": signal,
                "source": "file",
            }
        },
        "signal_type": signal,
        "targets": {},
        "metadata": {},
        "provenance": {
            "format": "test",
            "reader": "test",
            "reader_version": "0",
            "sources": [],
            "record_schema_version": "0.2.0",
            "warnings": [],
        },
        "quality_flags": [],
    }


def sample(relative: str) -> Path:
    return Path(__file__).resolve().parents[3] / relative
