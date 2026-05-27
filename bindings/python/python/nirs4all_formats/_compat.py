"""Raw access to the Rust reader pipeline (native PyO3 with a CLI fallback).

These functions return the records exactly as the Rust core emits them, as
plain dicts/lists. The lossless object model and the lossy projections live in
:mod:`nirs4all_formats.records`.
"""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

try:  # native PyO3 extension built by maturin
    from . import _native  # type: ignore[attr-defined]
except ImportError:  # pragma: no cover - fallback when the wheel was not built
    _native = None  # type: ignore[assignment]


def open_records(
    path: str | Path, *, single_record: bool = False
) -> list[dict[str, Any]]:
    """Read a file through the Rust backend and return normalized records.

    ``single_record=True`` asks cube readers (ENVI Standard, AVIRIS/ERDAS
    LAN) to emit a single N-dimensional record instead of one per pixel.
    """

    if _native is not None:
        records = _native.open_path(str(path), single_record=single_record)
        if not isinstance(records, list):
            raise RuntimeError("native reader returned a non-list payload")
        return [record for record in records if isinstance(record, dict)]

    raw = _run_rust_reader(path, single_record=single_record)
    records = json.loads(raw)
    if not isinstance(records, list):
        raise RuntimeError("Rust reader returned a non-list JSON payload")
    return [record for record in records if isinstance(record, dict)]


def open_bytes(name: str | Path, payload: bytes) -> list[dict[str, Any]]:
    """Read raw bytes through the native Rust backend.

    `name` is the input file name (used for extension sniffing and
    provenance). Sidecar formats (ENVI Standard, AVIRIS LAN, FGI HDF5+XML,
    MATLAB Indian Pines, NetCDF MFRSR with QC sidecar) raise
    ``UnsupportedSidecar`` here; use :func:`open_with_sidecars` to decode
    them without touching the filesystem.
    """

    if _native is None:
        raise RuntimeError(
            "open_bytes requires the native PyO3 extension. Reinstall the wheel "
            "via `maturin develop` or `pip install nirs4all-formats`."
        )
    records = _native.open_bytes(str(name), payload)
    if not isinstance(records, list):
        raise RuntimeError("native open_bytes returned a non-list payload")
    return [record for record in records if isinstance(record, dict)]


def open_with_sidecars(
    name: str | Path,
    payload: bytes,
    sidecars: Mapping[str, bytes],
) -> list[dict[str, Any]]:
    """Read bytes plus a mapping of sidecar names to byte payloads.

    Keys in `sidecars` are interpreted as relative paths next to the
    primary file: ENVI Standard wants ``"<stem>.hdr"``, AVIRIS LAN wants
    ``"<stem>.spc"`` and optionally ``"92AV3GT.GIS"``, FGI XML wants the
    HDF5 path referenced in ``<DataReference>``.
    """

    if _native is None:
        raise RuntimeError(
            "open_with_sidecars requires the native PyO3 extension. Reinstall "
            "the wheel via `maturin develop` or `pip install nirs4all-formats`."
        )
    records = _native.open_with_sidecars(str(name), payload, dict(sidecars))
    if not isinstance(records, list):
        raise RuntimeError("native open_with_sidecars returned a non-list payload")
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


def _run_rust_reader(path: str | Path, *, single_record: bool = False) -> str:
    args = ["read-json", str(path)]
    if single_record:
        args.append("--single-record")
    return _run_rust_reader_command(args)


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
    explicit = os.environ.get("NIRS4ALL_FORMATS_CLI")
    if explicit:
        return shlex.split(explicit)

    binary = shutil.which("nirs4all-formats")
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
            "nirs4all-formats-cli",
            "--manifest-path",
            str(repo_root / "Cargo.toml"),
            "--",
        ]

    raise RuntimeError("Cannot find nirs4all-formats CLI binary or source workspace")


def _repo_root() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").exists() and (
            parent / "crates" / "nirs4all-formats-cli"
        ).exists():
            return parent
    return None
