"""Python binding surface for nirs4all-formats.

Two layers:

- raw access — :func:`open_records`, :func:`open_bytes`,
  :func:`open_with_sidecars`, :func:`probe_path`, :func:`walk_path` return the
  records exactly as the Rust core emits them (plain dicts);
- the lossless object model — :func:`open_recordset` returns a
  :class:`SpectralRecordSet` (faithful mirror of the Rust ``SpectralRecord``,
  N-dimensional, nothing dropped). Its ``to_*`` methods are the explicit,
  possibly lossy projections to numpy / pandas / scikit-learn / PyTorch /
  nirs4all ``SpectroDataset``.

:func:`write_records` and :func:`convert_path` export records to exchange
formats read by third-party software; see :func:`export_targets`.
"""

from ._compat import (
    convert_path,
    export_targets,
    open_bytes,
    open_records,
    open_with_sidecars,
    probe_path,
    walk_path,
    write_records,
)
from ._version import __version__
from .records import (
    Provenance,
    SourceFile,
    SpectralArray,
    SpectralAxis,
    SpectralRecord,
    SpectralRecordSet,
    open_recordset,
)

__all__ = [
    "Provenance",
    "SourceFile",
    "SpectralArray",
    "SpectralAxis",
    "SpectralRecord",
    "SpectralRecordSet",
    "__version__",
    "convert_path",
    "export_targets",
    "open_bytes",
    "open_records",
    "open_recordset",
    "open_with_sidecars",
    "probe_path",
    "walk_path",
    "write_records",
]
