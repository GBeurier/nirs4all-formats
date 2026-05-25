"""Lossless Python mirror of the Rust ``SpectralRecord`` model plus explicit
lossy projections to the formats users actually consume.

The mirror (``SpectralRecordSet`` and friends) reproduces exactly what the Rust
core emits: every signal, its N-dimensional ``shape``/``dims``, the spectral
``axis`` and the per-dimension ``coords``, full ``metadata`` and ``provenance``.
Nothing is reshaped, aligned, dropped or inferred when loading.

Projections (``to_numpy``/``to_pandas``/``to_sklearn``/``to_torch``/
``to_spectrodataset``) are explicit and *may* be lossy. They flatten the chosen
feature dimension into columns and every other dimension into samples, and they
fail with a clear report when records disagree on the feature axis.
"""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from ._compat import open_records

# Reserved metadata column prefix used to carry provenance / quality flags into
# nirs4all SpectroDataset without colliding with sample metadata.
_RESERVED = "nirs4all_io."
_NIRS4ALL_UNITS = {"cm-1", "nm", "none", "text", "index"}


@dataclass(frozen=True)
class SpectralAxis:
    values: np.ndarray
    unit: str
    kind: str
    order: str

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "SpectralAxis":
        return cls(
            values=np.asarray(payload.get("values", []), dtype=float),
            unit=str(payload.get("unit", "index")),
            kind=str(payload.get("kind", "index")),
            order=str(payload.get("order", "ascending")),
        )


@dataclass(frozen=True)
class SpectralArray:
    """One named signal channel, N-dimensional and lossless.

    ``values`` is reshaped to ``shape`` (C-order). The spectral dimension is
    ``"x"`` with coordinate ``axis``; other dimensions keep their coordinate in
    ``coords``.
    """

    axis: SpectralAxis
    values: np.ndarray
    shape: tuple[int, ...]
    dims: tuple[str, ...]
    coords: dict[str, SpectralAxis]
    signal_type: str
    unit: str | None
    role: str
    source: str

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "SpectralArray":
        shape = tuple(int(extent) for extent in payload.get("shape", []))
        flat = np.asarray(payload.get("values", []), dtype=float)
        values = flat.reshape(shape) if shape else flat
        coords = {
            str(dim): SpectralAxis.from_dict(coord)
            for dim, coord in (payload.get("coords") or {}).items()
        }
        return cls(
            axis=SpectralAxis.from_dict(payload.get("axis", {})),
            values=values,
            shape=shape,
            dims=tuple(str(dim) for dim in payload.get("dims", [])),
            coords=coords,
            signal_type=str(payload.get("signal_type", "unknown")),
            unit=payload.get("unit"),
            role=str(payload.get("role", "")),
            source=str(payload.get("source", "file")),
        )

    @property
    def ndim(self) -> int:
        return len(self.shape)

    @property
    def x_dim_index(self) -> int:
        return self.dims.index("x")

    def coordinate(self, dim: str) -> SpectralAxis:
        """Coordinate for ``dim`` (the spectral ``axis`` for ``"x"``)."""

        return self.axis if dim == "x" else self.coords[dim]

    def to_xarray(self) -> Any:
        """Return an ``xarray.DataArray`` (dims + coords). Requires xarray."""

        import xarray as xr  # type: ignore[import-not-found]

        coords = {dim: self.coordinate(dim).values for dim in self.dims}
        return xr.DataArray(self.values, dims=list(self.dims), coords=coords, name=self.role)


@dataclass(frozen=True)
class SourceFile:
    path: str
    archive: str | None
    sha256: str
    role: str

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "SourceFile":
        return cls(
            path=str(payload.get("path", "")),
            archive=payload.get("archive"),
            sha256=str(payload.get("sha256", "")),
            role=str(payload.get("role", "")),
        )


@dataclass(frozen=True)
class Provenance:
    format: str
    reader: str
    reader_version: str
    sources: tuple[SourceFile, ...]
    parsed_at_utc: str | None
    record_schema_version: str
    warnings: tuple[str, ...]

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "Provenance":
        return cls(
            format=str(payload.get("format", "unknown")),
            reader=str(payload.get("reader", "")),
            reader_version=str(payload.get("reader_version", "")),
            sources=tuple(SourceFile.from_dict(s) for s in payload.get("sources", [])),
            parsed_at_utc=payload.get("parsed_at_utc"),
            record_schema_version=str(payload.get("record_schema_version", "")),
            warnings=tuple(str(w) for w in payload.get("warnings", [])),
        )


@dataclass(frozen=True)
class SpectralRecord:
    signals: dict[str, SpectralArray]
    signal_type: str
    targets: dict[str, Any]
    metadata: dict[str, Any]
    provenance: Provenance
    quality_flags: tuple[str, ...]

    @classmethod
    def from_dict(cls, payload: Mapping[str, Any]) -> "SpectralRecord":
        signals = {
            str(name): SpectralArray.from_dict(arr)
            for name, arr in (payload.get("signals") or {}).items()
        }
        return cls(
            signals=signals,
            signal_type=str(payload.get("signal_type", "unknown")),
            targets=dict(payload.get("targets") or {}),
            metadata=dict(payload.get("metadata") or {}),
            provenance=Provenance.from_dict(payload.get("provenance", {})),
            quality_flags=tuple(str(f) for f in payload.get("quality_flags", [])),
        )


@dataclass(frozen=True)
class SpectralRecordSet:
    """Lossless, ordered collection of records — the canonical Python object."""

    records: tuple[SpectralRecord, ...]
    schema_version: str = ""

    # ----- builders -------------------------------------------------------
    @classmethod
    def from_dicts(cls, payload: Sequence[Mapping[str, Any]]) -> "SpectralRecordSet":
        records = tuple(SpectralRecord.from_dict(r) for r in payload)
        schema = (
            records[0].provenance.record_schema_version if records else ""
        )
        return cls(records=records, schema_version=schema)

    def __len__(self) -> int:
        return len(self.records)

    def __iter__(self):
        return iter(self.records)

    def signal_names(self) -> list[str]:
        """Sorted union of signal names across all records."""

        names: set[str] = set()
        for record in self.records:
            names.update(record.signals)
        return sorted(names)

    def default_signal(self) -> str:
        names = self.signal_names()
        if not names:
            raise ValueError("record set has no signals")
        for preferred in ("absorbance", "reflectance", "transmittance", "signal"):
            if preferred in names:
                return preferred
        return names[0]

    # ----- projection core ------------------------------------------------
    def _project(
        self, signal: str | None, feature_dim: str
    ) -> tuple[np.ndarray, np.ndarray, str, list[dict[str, Any]]]:
        """Flatten one signal into ``(X, feature_axis_values, unit, sample_meta)``.

        The feature dimension becomes columns; every other dimension becomes
        rows. Records that disagree on the feature axis raise a strict error.
        Records missing the signal contribute a single NaN row (NaN-fill).
        """

        name = signal or self.default_signal()
        feature_axis: np.ndarray | None = None
        feature_unit = "index"
        signatures: list[tuple[int, float, float]] = []
        # First pass: establish and validate the feature axis.
        for record in self.records:
            array = record.signals.get(name)
            if array is None:
                continue
            axis = array.coordinate(feature_dim)
            sig = (len(axis.values), float(axis.values[0]), float(axis.values[-1]))
            if feature_axis is None:
                feature_axis = axis.values
                feature_unit = axis.unit
                signatures.append(sig)
            elif not np.array_equal(axis.values, feature_axis):
                if sig not in signatures:
                    signatures.append(sig)
        if feature_axis is None:
            raise ValueError(f"no record carries signal {name!r}")
        if len(signatures) > 1:
            raise ValueError(
                "cannot project records with different feature axes (strict mode). "
                f"signal {name!r} dim {feature_dim!r} has {len(signatures)} distinct "
                f"axis signatures (len, first, last): {signatures}. Resample with "
                "nirs4all before projecting, or select a homogeneous subset."
            )
        n_features = feature_axis.shape[0]

        rows: list[np.ndarray] = []
        sample_meta: list[dict[str, Any]] = []
        for index, record in enumerate(self.records):
            array = record.signals.get(name)
            if array is None:
                rows.append(np.full(n_features, np.nan))
                sample_meta.append(self._base_meta(record, index, {}))
                continue
            fidx = array.dims.index(feature_dim)
            moved = np.moveaxis(array.values, fidx, -1)
            sample_shape = moved.shape[:-1]
            flat = moved.reshape(-1, n_features)
            sample_dims = [d for i, d in enumerate(array.dims) if i != fidx]
            for sample_index in range(flat.shape[0]):
                rows.append(flat[sample_index])
                coord_values: dict[str, Any] = {}
                if sample_shape:
                    multi = np.unravel_index(sample_index, sample_shape)
                    for dim, position in zip(sample_dims, multi):
                        coord = array.coords.get(dim)
                        coord_values[f"coord_{dim}"] = (
                            float(coord.values[position]) if coord is not None else int(position)
                        )
                sample_meta.append(self._base_meta(record, index, coord_values))
        return np.asarray(rows, dtype=float), feature_axis, feature_unit, sample_meta

    @staticmethod
    def _base_meta(record: SpectralRecord, index: int, extra: Mapping[str, Any]) -> dict[str, Any]:
        prov = record.provenance
        meta: dict[str, Any] = {}
        sample_id = record.metadata.get("sample_id")
        meta["sample_id"] = sample_id if sample_id is not None else f"record_{index}"
        for key, value in record.metadata.items():
            if key == "sample_id" or isinstance(value, (dict, list)):
                continue
            meta[key] = value
        meta.update(extra)
        meta[f"{_RESERVED}record_index"] = index
        meta[f"{_RESERVED}format"] = prov.format
        meta[f"{_RESERVED}reader"] = prov.reader
        meta[f"{_RESERVED}reader_version"] = prov.reader_version
        meta[f"{_RESERVED}source_sha256"] = prov.sources[0].sha256 if prov.sources else None
        meta[f"{_RESERVED}provenance_json"] = json.dumps(
            {
                "format": prov.format,
                "reader": prov.reader,
                "reader_version": prov.reader_version,
                "sources": [s.__dict__ for s in prov.sources],
                "warnings": list(prov.warnings),
            }
        )
        meta[f"{_RESERVED}quality_flags_json"] = json.dumps(list(record.quality_flags))
        return meta

    def _targets(self) -> dict[str, list[Any]]:
        """Per-record targets, aligned to record order (None where absent)."""

        names: list[str] = []
        for record in self.records:
            for key in record.targets:
                if key not in names:
                    names.append(key)
        return {
            name: [record.targets.get(name) for record in self.records] for name in names
        }

    # ----- projections ----------------------------------------------------
    def to_numpy(
        self, *, signal: str | None = None, feature_dim: str = "x"
    ) -> tuple[np.ndarray, np.ndarray]:
        """Return ``(X[n_samples, n_features], feature_axis_values)``."""

        x, axis, _unit, _meta = self._project(signal, feature_dim)
        return x, axis

    def to_pandas_long(self) -> Any:
        """Loss-minimising long frame: one row per (record, signal, point)."""

        import pandas as pd

        rows: list[dict[str, Any]] = []
        for index, record in enumerate(self.records):
            sample_id = record.metadata.get("sample_id", f"record_{index}")
            for name, array in record.signals.items():
                # Multi-dimensional arrays are flattened along the "x" axis;
                # each non-x position becomes a separate sample_in_record.
                axis_values = array.axis.values
                flat = np.moveaxis(array.values, array.x_dim_index, -1).reshape(-1, len(axis_values))
                for sample_index in range(flat.shape[0]):
                    for point, (coord, value) in enumerate(zip(axis_values, flat[sample_index])):
                        rows.append(
                            {
                                "record_index": index,
                                "sample_id": sample_id,
                                "signal": name,
                                "sample_in_record": sample_index,
                                "point": point,
                                "axis": float(coord),
                                "value": float(value),
                            }
                        )
        return pd.DataFrame(rows)

    def _wide_table(
        self, signal: str | None, feature_dim: str
    ) -> dict[str, list[Any]]:
        """Ordered column dict shared by ``to_pandas`` and ``to_polars``:
        metadata + reserved provenance columns, per-record targets broadcast
        onto sample rows, then one ``x_<axis>`` column per feature."""

        x, axis, _unit, meta = self._project(signal, feature_dim)
        record_index = [int(m[f"{_RESERVED}record_index"]) for m in meta]
        table: dict[str, list[Any]] = {
            key: [row.get(key) for row in meta] for key in meta[0]
        } if meta else {}
        for name, values in self._targets().items():
            table[name] = [values[i] for i in record_index]
        for col, coord in enumerate(axis):
            table[f"x_{float(coord):g}"] = x[:, col].tolist()
        return table

    def to_pandas(self, *, signal: str | None = None, feature_dim: str = "x") -> Any:
        """Wide pandas frame: metadata + provenance columns followed by
        ``x_<axis>``."""

        import pandas as pd

        return pd.DataFrame(self._wide_table(signal, feature_dim))

    def to_polars(self, *, signal: str | None = None, feature_dim: str = "x") -> Any:
        """Lower-level wide polars frame (same columns as :meth:`to_pandas`).

        polars is the backend nirs4all's own ``SpectroDataset.metadata()``
        uses, so this is the natural zero-copy-ish hand-off for that side."""

        import polars as pl

        return pl.DataFrame(self._wide_table(signal, feature_dim))

    def to_sklearn(self, *, signal: str | None = None, target: str | None = None) -> Any:
        """Return a scikit-learn ``Bunch`` with data/target/feature_names."""

        try:
            from sklearn.utils import Bunch
        except ImportError:

            class Bunch(dict):  # type: ignore[no-redef]
                def __getattr__(self, key: str) -> Any:
                    try:
                        return self[key]
                    except KeyError as exc:
                        raise AttributeError(key) from exc

        x, axis, _unit, meta = self._project(signal, "x")
        targets = self._targets()
        target_name = target or (next(iter(targets)) if len(targets) == 1 else None)
        y = None
        if target_name is not None:
            record_index = [int(m[f"{_RESERVED}record_index"]) for m in meta]
            y = np.asarray([targets[target_name][i] for i in record_index])
        return Bunch(
            data=x,
            target=y,
            target_name=target_name,
            feature_names=[f"x_{float(v):g}" for v in axis],
            wavelengths=axis,
            sample_ids=[m["sample_id"] for m in meta],
        )

    def to_torch(self, *, signal: str | None = None, target: str | None = None) -> Any:
        """Return a torch ``TensorDataset``-like object (float32)."""

        import torch  # type: ignore[import-not-found]

        bunch = self.to_sklearn(signal=signal, target=target)
        x = torch.as_tensor(np.asarray(bunch.data), dtype=torch.float32)
        if bunch.target is None:
            return torch.utils.data.TensorDataset(x)
        y = torch.as_tensor(np.asarray(bunch.target, dtype=float), dtype=torch.float32)
        return torch.utils.data.TensorDataset(x, y)

    def to_spectrodataset(
        self,
        *,
        name: str = "nirs4all_io",
        signals: Sequence[str] | None = None,
        target: str | None = None,
    ) -> Any:
        """Build a nirs4all ``SpectroDataset``: each signal becomes a source.

        Provenance and quality flags travel as reserved ``nirs4all_io.*``
        metadata columns (including JSON blobs) so model reports can trace file
        origin. Requires every selected signal to yield the same sample count
        (the 1-sample-per-record case); mixed N-D geometries raise.
        """

        from nirs4all.data import SpectroDataset  # type: ignore[import-not-found]

        chosen = list(signals) if signals is not None else self.signal_names()
        if not chosen:
            raise ValueError("record set has no signals to project")

        blocks: list[np.ndarray] = []
        headers: list[list[str]] = []
        units: list[str] = []
        signal_types: list[str] = []
        reference_meta: list[dict[str, Any]] | None = None
        n_samples: int | None = None
        for sig in chosen:
            x, axis, unit, meta = self._project(sig, "x")
            if n_samples is None:
                n_samples = x.shape[0]
                reference_meta = meta
            elif x.shape[0] != n_samples:
                raise ValueError(
                    f"signal {sig!r} yields {x.shape[0]} samples but the first "
                    f"source yields {n_samples}; multi-source SpectroDataset needs "
                    "aligned sample counts"
                )
            blocks.append(x.astype("float32"))
            headers.append([f"{float(v):g}" for v in axis])
            units.append(unit if unit in _NIRS4ALL_UNITS else "index")
            present = next(
                (r.signals[sig].signal_type for r in self.records if sig in r.signals),
                "unknown",
            )
            signal_types.append(present)

        dataset = SpectroDataset(name=name)
        dataset.add_samples(blocks, headers=headers, header_unit=units)
        for src, signal_type in enumerate(signal_types):
            if signal_type and signal_type != "unknown":
                dataset.set_signal_type(signal_type, src=src)

        targets = self._targets()
        target_name = target or (next(iter(targets)) if len(targets) == 1 else None)
        if target_name is not None and reference_meta is not None:
            record_index = [int(m[f"{_RESERVED}record_index"]) for m in reference_meta]
            dataset.add_targets(np.asarray([targets[target_name][i] for i in record_index]))

        if reference_meta is not None:
            import pandas as pd

            frame = pd.DataFrame(reference_meta)
            for key, values in targets.items():
                record_index = [int(m[f"{_RESERVED}record_index"]) for m in reference_meta]
                frame[f"target_{key}"] = [values[i] for i in record_index]
            dataset.add_metadata(frame)
        return dataset


def open_recordset(path: str | Path, *, single_record: bool = False) -> SpectralRecordSet:
    """Read a file losslessly into a :class:`SpectralRecordSet`.

    ``single_record=True`` asks cube readers to emit one N-dimensional record
    (``dims = ["row", "col", "x"]``) instead of one record per pixel.
    """

    return SpectralRecordSet.from_dicts(open_records(path, single_record=single_record))
