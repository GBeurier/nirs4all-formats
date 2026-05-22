"""Python binding surface for nirs4all-io."""

from ._compat import (
    NirsDataset,
    SklearnDatasetProvider,
    TorchSpectralDataset,
    open_dataset,
    open_records,
    probe_path,
    to_nirs4all_spectrodataset,
    to_numpy_matrix,
    to_pandas_frame,
    to_sklearn_bunch,
    walk_path,
)
from ._version import __version__

__all__ = [
    "NirsDataset",
    "SklearnDatasetProvider",
    "TorchSpectralDataset",
    "__version__",
    "open_dataset",
    "open_records",
    "probe_path",
    "to_nirs4all_spectrodataset",
    "to_numpy_matrix",
    "to_pandas_frame",
    "to_sklearn_bunch",
    "walk_path",
]
