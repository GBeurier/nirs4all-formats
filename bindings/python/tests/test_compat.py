import sys
from pathlib import Path

import pytest

from nirs4all_io import (
    NirsDataset,
    open_dataset,
    open_records,
    probe_path,
    to_nirs4all_spectrodataset,
    to_numpy_matrix,
    to_sklearn_bunch,
    walk_path,
)
from nirs4all_io._compat import _native


def test_dataset_shape_contract() -> None:
    dataset = NirsDataset(
        x=[[0.1, 0.2], [0.3, 0.4]],
        wavelengths=[1100.0, 1110.0],
        targets={"protein": [12.0, 13.0]},
        sample_ids=["S001", "S002"],
    )

    assert len(dataset.x) == 2
    assert list(dataset.targets) == ["protein"]


def test_open_records_uses_rust_backend() -> None:
    records = open_records(sample("samples/csv_tsv/synthetic_nirs.csv"))

    assert len(records) == 50
    assert records[0]["provenance"]["format"] == "delimited-text"


def test_open_dataset_from_committed_sample() -> None:
    dataset = open_dataset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    assert len(dataset.x) == 50
    assert len(dataset.wavelengths) == 200
    assert dataset.sample_ids[0] == "S000"
    assert list(dataset.targets) == ["protein"]
    assert dataset.axis_unit == "nm"


def test_numpy_and_sklearn_shapes_when_numpy_is_available() -> None:
    pytest.importorskip("numpy")
    dataset = open_dataset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    x, wavelengths, targets = to_numpy_matrix(dataset)
    bunch = to_sklearn_bunch(dataset)

    assert x.shape == (50, 200)
    assert wavelengths.shape == (200,)
    assert targets["protein"].shape == (50,)
    assert bunch.data.shape == (50, 200)
    assert bunch.target.shape == (50,)


def test_nirs4all_spectrodataset_when_checkout_is_available() -> None:
    checkout = Path("/home/delete/nirs4all/nirs4all")
    if not checkout.exists():
        pytest.skip("nirs4all checkout not available")
    sys.path.insert(0, str(checkout))
    dataset = open_dataset(sample("samples/csv_tsv/synthetic_nirs.csv"))

    spectro_dataset = to_nirs4all_spectrodataset(dataset, name="smoke")

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


def sample(relative: str) -> Path:
    return Path(__file__).resolve().parents[3] / relative
