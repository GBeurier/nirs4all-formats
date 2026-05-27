"""Shared fixtures and helpers for the conformance harness.

Tests in this folder are marked with `@pytest.mark.conformance`. Run them
explicitly with:

    pytest -m conformance tests/conformance/

Reference readers that are not installed cause the matching tests to
skip with a descriptive reason rather than failing.
"""

from __future__ import annotations

import importlib
import json
import os
import shutil
import subprocess
import tomllib
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SAMPLES_ROOT = REPO_ROOT / "samples"
HERE = Path(__file__).resolve().parent


def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    """Auto-apply the `conformance` marker to every test in this folder."""

    marker = pytest.mark.conformance
    for item in items:
        item.add_marker(marker)


@dataclass(frozen=True)
class Tolerance:
    axis_abs: float
    axis_rel: float
    values_abs: float
    values_rel: float


def _load_toml(name: str) -> dict[str, Any]:
    path = HERE / name
    with path.open("rb") as fh:
        return tomllib.load(fh)


def load_tolerances() -> dict[str, Tolerance]:
    raw = _load_toml("tolerances.toml")
    return {
        key: Tolerance(
            axis_abs=float(payload["axis_abs_tol"]),
            axis_rel=float(payload["axis_rel_tol"]),
            values_abs=float(payload["values_abs_tol"]),
            values_rel=float(payload["values_rel_tol"]),
        )
        for key, payload in raw.items()
    }


def load_known_skips() -> dict[str, dict[str, str]]:
    raw = _load_toml("known_skips.toml")
    return {key: dict(value) for key, value in raw.items()}


def _import_or_skip(module: str, *, reason: str | None = None) -> Any:
    try:
        return importlib.import_module(module)
    except ImportError:
        pytest.skip(reason or f"{module} is not installed in the conformance env")


def require_nirs4all_formats():
    return _import_or_skip(
        "nirs4all_formats",
        reason="nirs4all_formats PyO3 binding is required; run `maturin develop` in bindings/python",
    )


def require_brukeropus():
    return _import_or_skip("brukeropus")


def require_spc():
    return _import_or_skip("spc_spectra")


def require_jcamp():
    return _import_or_skip("jcamp")


def require_allotropy():
    return _import_or_skip("allotropy")


def require_h5py():
    return _import_or_skip("h5py")


def require_rscript_with(package: str):
    """Return the absolute path to Rscript when the named R package is
    installed; skip otherwise."""

    rscript = shutil.which("Rscript")
    if rscript is None:
        pytest.skip("Rscript not available")
    probe = subprocess.run(
        [rscript, "-e", f"requireNamespace('{package}', quietly=TRUE) || quit(status=1)"],
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        pytest.skip(f"R package '{package}' not installed: {probe.stderr.strip()}")
    return rscript


def fixtures_under(*relative_dirs: str, suffix: tuple[str, ...] = (), exclude: Iterable[str] = ()) -> list[Path]:
    """Enumerate sample files under `samples/<dir>/` matching the suffix
    list, excluding any path stems explicitly listed.
    """

    excluded = {value.lower() for value in exclude}
    out: list[Path] = []
    for relative in relative_dirs:
        directory = SAMPLES_ROOT / relative
        if not directory.exists():
            continue
        for entry in sorted(directory.iterdir()):
            if not entry.is_file():
                continue
            if suffix and entry.suffix.lower() not in suffix:
                continue
            stem_key = entry.stem.lower()
            if stem_key in excluded:
                continue
            out.append(entry)
    return out


def within(value: float, expected: float, tol: Tolerance, axis: bool) -> bool:
    abs_tol = tol.axis_abs if axis else tol.values_abs
    rel_tol = tol.axis_rel if axis else tol.values_rel
    diff = abs(value - expected)
    bound = max(abs_tol, rel_tol * max(abs(value), abs(expected)))
    return diff <= bound


def compare_axes(left: list[float], right: list[float], tol: Tolerance, *, label: str) -> None:
    assert len(left) == len(right), f"{label}: axis length mismatch ({len(left)} vs {len(right)})"
    for index, (a, b) in enumerate(zip(left, right)):
        assert within(a, b, tol, axis=True), (
            f"{label}: axis[{index}] differs ({a} vs {b}, tolerance "
            f"abs={tol.axis_abs}, rel={tol.axis_rel})"
        )


def compare_values(left: list[float], right: list[float], tol: Tolerance, *, label: str) -> None:
    assert len(left) == len(right), f"{label}: values length mismatch ({len(left)} vs {len(right)})"
    for index, (a, b) in enumerate(zip(left, right)):
        assert within(a, b, tol, axis=False), (
            f"{label}: values[{index}] differs ({a} vs {b}, tolerance "
            f"abs={tol.values_abs}, rel={tol.values_rel})"
        )


def normalize_records(records: list[Mapping[str, Any]]) -> list[dict[str, list[float]]]:
    """Reduce nirs4all-formats records to {axis, values_by_signal} dicts."""

    normalized: list[dict[str, Any]] = []
    for record in records:
        signals = record.get("signals", {})
        for name, signal in signals.items():
            axis = signal["axis"]["values"]
            values = signal["values"]
            normalized.append(
                {
                    "signal_name": name,
                    "signal_type": signal.get("signal_type", "unknown"),
                    "axis": list(axis),
                    "values": list(values),
                }
            )
    return normalized


def conformance_skip(reason: str) -> None:
    pytest.skip(reason)


def run_rscript(rscript: str, script_path: Path, *args: str) -> dict[str, Any]:
    result = subprocess.run(
        [rscript, "--vanilla", str(script_path), *args],
        capture_output=True,
        text=True,
        env={**os.environ, "R_LIBS_USER": os.environ.get("R_LIBS_USER", "")},
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"Rscript {script_path.name} failed: {result.stderr.strip()}"
        )
    return json.loads(result.stdout)
