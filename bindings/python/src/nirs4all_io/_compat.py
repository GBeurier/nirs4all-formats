"""Small compatibility helpers for downstream Python libraries.

The Rust binding will replace the record transport. These helpers define the
shape expected by numpy, pandas, sklearn and torch integrations.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence


@dataclass(frozen=True)
class NirsDataset:
    """Tabular spectral dataset representation used by Python integrations."""

    x: Sequence[Sequence[float]]
    wavelengths: Sequence[float]
    targets: Mapping[str, Sequence[Any]]
    sample_ids: Sequence[str]


def to_numpy_matrix(dataset: NirsDataset) -> Any:
    """Return `(X, wavelengths, targets)` as numpy arrays when numpy is installed."""

    import numpy as np  # type: ignore[import-not-found]

    targets = {name: np.asarray(values) for name, values in dataset.targets.items()}
    return np.asarray(dataset.x, dtype=float), np.asarray(dataset.wavelengths, dtype=float), targets


def to_pandas_frame(dataset: NirsDataset) -> Any:
    """Return one pandas DataFrame with metadata/targets followed by spectral columns."""

    import pandas as pd  # type: ignore[import-not-found]

    data: dict[str, Any] = {"sample_id": list(dataset.sample_ids)}
    data.update({name: list(values) for name, values in dataset.targets.items()})
    for index, wavelength in enumerate(dataset.wavelengths):
        data[f"x_{float(wavelength):g}"] = [row[index] for row in dataset.x]
    return pd.DataFrame(data)
