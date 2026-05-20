"""Python binding surface for nirs4all-io."""

from ._compat import NirsDataset, to_numpy_matrix, to_pandas_frame
from ._version import __version__

__all__ = ["NirsDataset", "__version__", "to_numpy_matrix", "to_pandas_frame"]
