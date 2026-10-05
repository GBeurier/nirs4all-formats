"""Independent SciPy comparison for every variable in labelled MAT exports."""

from pathlib import Path

import numpy as np
import pytest
from conftest import REPO_ROOT, require_nirs4all_formats


def _pairs() -> list[tuple[Path, Path]]:
    pairs = [
        (
            REPO_ROOT / "samples/unscrambler/synthetic.00D",
            REPO_ROOT / "samples/matlab/synthetic_unscrambler.mat",
        )
    ]
    native = REPO_ROOT / "samples_local/unscrambler/SENUCSPE.00D"
    mat = REPO_ROOT / "samples_local/matlab/SENUCSPE.mat"
    if not native.exists():
        native = REPO_ROOT / "samples_todo/SENUCSPE.00D"
    if not mat.exists():
        mat = REPO_ROOT / "samples_todo/SENUCSPE.mat"
    if native.exists() and mat.exists():
        pairs.append((native, mat))
    return pairs


@pytest.mark.parametrize("paths", _pairs(), ids=lambda p: p[0].stem)
def test_all_unscrambler_variables_match_scipy(paths: tuple[Path, Path]) -> None:
    scipy_io = pytest.importorskip("scipy.io")
    nirs = require_nirs4all_formats()
    native_path, mat_path = paths
    reference = scipy_io.loadmat(mat_path)
    labels = [str(s).strip() for s in reference["VarLabels0"]]
    objects = [str(s).strip() for s in reference["ObjLabels"]]
    matrices = [
        v
        for k, v in reference.items()
        if not k.startswith("__")
        and v.shape == (len(objects), len(labels))
        and v.dtype.kind in "fiu"
    ]
    assert len(matrices) == 1
    matrix = matrices[0]
    for path in (native_path, mat_path):
        raw = nirs.open_records(path)
        records = raw.to_dicts() if hasattr(raw, "to_dicts") else raw
        assert len(records) == len(objects)
        for row, record in enumerate(records):
            metadata = record["metadata"]
            assert metadata["sample_id"] == objects[row]
            assert metadata["variable_labels"] == labels
            assert metadata["missing_value_count"] == int(
                np.count_nonzero(~np.isfinite(matrix[row]))
            )
            covered: set[int] = set()
            for name, signal in record["signals"].items():
                columns = metadata["signal_columns"][name]
                assert not covered.intersection(columns)
                covered.update(columns)
                values = [float("nan") if v is None else v for v in signal["values"]]
                np.testing.assert_allclose(
                    values, matrix[row, columns], rtol=1e-6, atol=1e-9, equal_nan=True
                )
                assert signal["axis"]["kind"] == "index"
                assert signal["signal_type"] == "unknown"
            assert len(record["targets"]) == len(labels) - len(covered)
            keys: set[str] = set()
            for col, label in enumerate(labels):
                if col in covered:
                    continue
                key = label if label not in keys else f"{label}_{col + 1}"
                keys.add(key)
                actual = record["targets"][key]
                expected = matrix[row, col]
                if not np.isfinite(expected):
                    assert actual is None
                else:
                    assert actual == pytest.approx(expected, rel=1e-6, abs=1e-9)
            assert len(covered) + len(keys) == matrix.shape[1]
