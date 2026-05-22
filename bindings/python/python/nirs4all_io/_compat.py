"""Python compatibility helpers backed by the Rust reader pipeline."""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

try:  # native PyO3 extension built by maturin
    from . import _native  # type: ignore[attr-defined]
except ImportError:  # pragma: no cover - fallback when the wheel was not built
    _native = None  # type: ignore[assignment]


@dataclass(frozen=True)
class NirsDataset:
    """Tabular spectral dataset representation used by Python integrations."""

    x: Sequence[Sequence[float]]
    wavelengths: Sequence[float]
    targets: Mapping[str, Sequence[Any]]
    sample_ids: Sequence[str]
    metadata: Sequence[Mapping[str, Any]] = field(default_factory=tuple)
    signal_type: str = "unknown"
    axis_unit: str = "index"
    formats: Sequence[str] = field(default_factory=tuple)


def open_records(path: str | Path) -> list[dict[str, Any]]:
    """Read a file through the Rust backend and return normalized records."""

    if _native is not None:
        records = _native.open_path(str(path))
        if not isinstance(records, list):
            raise RuntimeError("native reader returned a non-list payload")
        return [record for record in records if isinstance(record, dict)]

    raw = _run_rust_reader(path)
    records = json.loads(raw)
    if not isinstance(records, list):
        raise RuntimeError("Rust reader returned a non-list JSON payload")
    return [record for record in records if isinstance(record, dict)]


def open_bytes(name: str | Path, payload: bytes) -> list[dict[str, Any]]:
    """Read raw bytes through the native Rust backend.

    `name` is the input file name (used for extension sniffing and
    provenance). Sidecar formats (ENVI Standard, AVIRIS LAN) require a real
    filesystem and raise an error here; use ``open_records(path)`` instead.
    """

    if _native is None:
        raise RuntimeError(
            "open_bytes requires the native PyO3 extension. Reinstall the wheel "
            "via `maturin develop` or `pip install nirs4all-io`."
        )
    records = _native.open_bytes(str(name), payload)
    if not isinstance(records, list):
        raise RuntimeError("native open_bytes returned a non-list payload")
    return [record for record in records if isinstance(record, dict)]


def probe_path(path: str | Path) -> list[dict[str, Any]]:
    """Return ordered candidate readers without parsing the full file."""

    if _native is not None:
        probes = _native.probe_path(str(path))
        if not isinstance(probes, list):
            raise RuntimeError("native probe_path returned a non-list payload")
        return [probe for probe in probes if isinstance(probe, dict)]

    raw = _run_rust_reader_command(["probe", str(path)])
    probes = json.loads(raw)
    if not isinstance(probes, list):
        raise RuntimeError("probe returned a non-list payload")
    return [probe for probe in probes if isinstance(probe, dict)]


def walk_path(
    path: str | Path,
    *,
    max_depth: int | None = None,
    include_hidden: bool = False,
    follow_symlinks: bool = False,
    include_unsupported: bool = False,
) -> list[dict[str, Any]]:
    """Recursively scan a tree and return per-file outcomes.

    Each entry is a dict with at least `path` and `status` ∈ {`parsed`,
    `error`, `unsupported`}. Parsed entries also carry `format` and `records`.
    """

    if _native is not None:
        entries = _native.walk_path(
            str(path),
            max_depth=max_depth,
            include_hidden=include_hidden,
            follow_symlinks=follow_symlinks,
            include_unsupported=include_unsupported,
        )
        if not isinstance(entries, list):
            raise RuntimeError("native walk_path returned a non-list payload")
        return [entry for entry in entries if isinstance(entry, dict)]

    args = ["scan", str(path)]
    if max_depth is not None:
        args.extend(["--max-depth", str(max_depth)])
    if include_hidden:
        args.append("--include-hidden")
    if follow_symlinks:
        args.append("--follow-symlinks")
    if include_unsupported:
        args.append("--include-unsupported")
    args.append("--json")
    raw = _run_rust_reader_command(args)
    payload = json.loads(raw)
    entries = payload.get("entries") if isinstance(payload, Mapping) else None
    if not isinstance(entries, list):
        raise RuntimeError("scan returned no entries array")
    return [entry for entry in entries if isinstance(entry, dict)]


def open_dataset(path: str | Path, *, signal: str | None = None) -> NirsDataset:
    """Read one spectral file and collapse records into a tabular dataset."""

    records = open_records(path)
    if not records:
        raise RuntimeError(f"Rust reader returned no records for {path}")

    rows: list[list[float]] = []
    sample_ids: list[str] = []
    metadata_rows: list[Mapping[str, Any]] = []
    target_values: dict[str, list[Any]] = {}
    formats: list[str] = []
    wavelengths: list[float] | None = None
    axis_unit = "index"
    signal_type = "unknown"

    for row_index, record in enumerate(records):
        name, signal_payload = _select_signal(record, signal)
        values = [float(value) for value in signal_payload.get("values", [])]
        axis = signal_payload.get("axis", {})
        axis_values = [float(value) for value in axis.get("values", [])]
        if not values or not axis_values:
            raise RuntimeError(f"Record {row_index} signal {name!r} is empty")
        if len(values) != len(axis_values):
            raise RuntimeError(f"Record {row_index} signal {name!r} has mismatched axis length")

        if wavelengths is None:
            wavelengths = axis_values
            axis_unit = str(axis.get("unit", "index"))
            signal_type = str(
                signal_payload.get("signal_type", record.get("signal_type", "unknown"))
            )
        elif axis_values != wavelengths:
            raise RuntimeError("Cannot build one dataset from records with different axes")

        rows.append(values)
        metadata = _dict_or_empty(record.get("metadata"))
        metadata_rows.append(metadata)
        sample_ids.append(_sample_id(record, metadata, row_index))
        formats.append(str(_dict_or_empty(record.get("provenance")).get("format", "unknown")))

        targets = _dict_or_empty(record.get("targets"))
        for key in list(target_values):
            target_values[key].append(targets.get(key))
        for key, value in targets.items():
            if key not in target_values:
                target_values[key] = [None] * row_index
                target_values[key].append(value)

    return NirsDataset(
        x=rows,
        wavelengths=wavelengths or [],
        targets=target_values,
        sample_ids=sample_ids,
        metadata=metadata_rows,
        signal_type=signal_type,
        axis_unit=axis_unit,
        formats=formats,
    )


def to_numpy_matrix(dataset: NirsDataset) -> Any:
    """Return `(X, wavelengths, targets)` as numpy arrays."""

    import numpy as np  # type: ignore[import-not-found]

    targets = {name: np.asarray(values) for name, values in dataset.targets.items()}
    return np.asarray(dataset.x, dtype=float), np.asarray(dataset.wavelengths, dtype=float), targets


def to_pandas_frame(dataset: NirsDataset) -> Any:
    """Return one pandas DataFrame with metadata/targets followed by spectral columns."""

    import pandas as pd  # type: ignore[import-not-found]

    data: dict[str, Any] = {"sample_id": list(dataset.sample_ids)}
    metadata_keys = sorted({key for row in dataset.metadata for key in row if key != "sample_id"})
    for key in metadata_keys:
        data[f"meta_{key}"] = [row.get(key) for row in dataset.metadata]
    data.update({name: list(values) for name, values in dataset.targets.items()})
    for index, wavelength in enumerate(dataset.wavelengths):
        data[f"x_{float(wavelength):g}"] = [row[index] for row in dataset.x]
    return pd.DataFrame(data)


def to_sklearn_bunch(dataset: NirsDataset, *, target: str | None = None) -> Any:
    """Return a scikit-learn-style Bunch with data, target and feature names."""

    import numpy as np  # type: ignore[import-not-found]

    try:
        from sklearn.utils import Bunch  # type: ignore[import-not-found]
    except ImportError:

        class Bunch(dict):  # type: ignore[no-redef]
            def __getattr__(self, key: str) -> Any:
                try:
                    return self[key]
                except KeyError as exc:
                    raise AttributeError(key) from exc

    x, wavelengths, targets = to_numpy_matrix(dataset)
    target_name = target or (next(iter(targets)) if len(targets) == 1 else None)
    y = np.asarray(targets[target_name]) if target_name else None
    return Bunch(
        data=x,
        target=y,
        target_name=target_name,
        feature_names=[f"x_{float(value):g}" for value in wavelengths],
        wavelengths=wavelengths,
        sample_ids=list(dataset.sample_ids),
        signal_type=dataset.signal_type,
    )


@dataclass(frozen=True)
class SklearnDatasetProvider:
    """Small provider object for sklearn pipelines and examples."""

    path: str | Path
    target: str | None = None
    signal: str | None = None

    def load(self) -> Any:
        return to_sklearn_bunch(open_dataset(self.path, signal=self.signal), target=self.target)

    def as_arrays(self) -> tuple[Any, Any]:
        bunch = self.load()
        return bunch.data, bunch.target


class TorchSpectralDataset:
    """Torch Dataset adapter around a Rust-loaded NIRS dataset."""

    def __init__(self, dataset: NirsDataset, *, target: str | None = None) -> None:
        import torch  # type: ignore[import-not-found]

        x, _, targets = to_numpy_matrix(dataset)
        target_name = target or (next(iter(targets)) if len(targets) == 1 else None)
        self.x = torch.as_tensor(x, dtype=torch.float32)
        self.y = torch.as_tensor(targets[target_name], dtype=torch.float32) if target_name else None
        self.sample_ids = list(dataset.sample_ids)
        self.wavelengths = dataset.wavelengths

    def __len__(self) -> int:
        return int(self.x.shape[0])

    def __getitem__(self, index: int) -> Any:
        if self.y is None:
            return self.x[index]
        return self.x[index], self.y[index]


def to_nirs4all_spectrodataset(
    dataset: NirsDataset,
    *,
    name: str = "nirs4all_io_dataset",
    target: str | None = None,
    add_metadata: bool = True,
) -> Any:
    """Create a nirs4all SpectroDataset and fill samples, targets and metadata."""

    import numpy as np  # type: ignore[import-not-found]
    import pandas as pd  # type: ignore[import-not-found]
    from nirs4all.data import SpectroDataset  # type: ignore[import-not-found]

    x, _, targets = to_numpy_matrix(dataset)
    spectro_dataset = SpectroDataset(name=name)
    header_unit = (
        dataset.axis_unit
        if dataset.axis_unit in {"cm-1", "nm", "none", "text", "index"}
        else "index"
    )
    spectro_dataset.add_samples(
        x.astype("float32"),
        headers=[f"{float(value):g}" for value in dataset.wavelengths],
        header_unit=header_unit,
    )

    target_name = target or (next(iter(targets)) if len(targets) == 1 else None)
    if target_name:
        spectro_dataset.add_targets(np.asarray(targets[target_name]))

    if add_metadata:
        metadata = pd.DataFrame({"sample_id": list(dataset.sample_ids)})
        for key in sorted(
            {key for row in dataset.metadata for key in row if key != "sample_id"}
        ):
            metadata[key] = [row.get(key) for row in dataset.metadata]
        for key, values in dataset.targets.items():
            metadata[f"target_{key}"] = list(values)
        spectro_dataset.add_metadata(metadata)

    if dataset.signal_type and dataset.signal_type != "unknown":
        spectro_dataset.set_signal_type(dataset.signal_type)

    return spectro_dataset


def _run_rust_reader(path: str | Path) -> str:
    return _run_rust_reader_command(["read-json", str(path)])


def _run_rust_reader_command(args: Sequence[str]) -> str:
    command = _reader_command()
    process = subprocess.run(
        [*command, *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or f"Rust reader failed with {process.returncode}")
    return process.stdout


def _reader_command() -> list[str]:
    explicit = os.environ.get("NIRS4ALL_IO_CLI")
    if explicit:
        return shlex.split(explicit)

    binary = shutil.which("nirs4all-io")
    if binary:
        return [binary]

    cargo = shutil.which("cargo")
    if not cargo:
        rustup_cargo = Path.home() / ".cargo" / "bin" / "cargo"
        cargo = str(rustup_cargo) if rustup_cargo.exists() else None
    repo_root = _repo_root()
    if cargo and repo_root:
        return [
            cargo,
            "run",
            "-q",
            "-p",
            "nirs4all-io-cli",
            "--manifest-path",
            str(repo_root / "Cargo.toml"),
            "--",
        ]

    raise RuntimeError("Cannot find nirs4all-io CLI binary or source workspace")


def _repo_root() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").exists() and (
            parent / "crates" / "nirs4all-io-cli"
        ).exists():
            return parent
    return None


def _select_signal(record: Mapping[str, Any], requested: str | None) -> tuple[str, Mapping[str, Any]]:
    signals = record.get("signals")
    if not isinstance(signals, Mapping) or not signals:
        raise RuntimeError("Record has no signals")

    if requested:
        payload = signals.get(requested)
        if not isinstance(payload, Mapping):
            raise RuntimeError(f"Record does not contain signal {requested!r}")
        return requested, payload

    preferred_type = record.get("signal_type")
    for key, payload in signals.items():
        if isinstance(payload, Mapping) and payload.get("signal_type") == preferred_type:
            return str(key), payload
    for key in ("reflectance", "absorbance", "transmittance", "signal"):
        payload = signals.get(key)
        if isinstance(payload, Mapping):
            return key, payload
    key = sorted(str(name) for name in signals)[0]
    payload = signals[key]
    if not isinstance(payload, Mapping):
        raise RuntimeError(f"Signal {key!r} is not an object")
    return key, payload


def _sample_id(record: Mapping[str, Any], metadata: Mapping[str, Any], row_index: int) -> str:
    sample_id = metadata.get("sample_id")
    if sample_id is not None:
        return str(sample_id)
    provenance = _dict_or_empty(record.get("provenance"))
    sources = provenance.get("sources")
    if isinstance(sources, Sequence) and sources:
        source = _dict_or_empty(sources[0])
        source_path = source.get("path")
        if source_path:
            return f"{Path(str(source_path)).stem}:{row_index}"
    return f"record:{row_index}"


def _dict_or_empty(value: Any) -> Mapping[str, Any]:
    return value if isinstance(value, Mapping) else {}
