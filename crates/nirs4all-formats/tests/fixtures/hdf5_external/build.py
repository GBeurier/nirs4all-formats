#!/usr/bin/env python3
"""Regenerate the synthetic HDF5 external-link and external-file fixtures.

Run this once whenever the fixture shape changes:

    python crates/nirs4all-formats/tests/fixtures/hdf5_external/build.py

Two artifact pairs are written next to this script, each carrying a proper
NIRS spectral schema (`/spectra` 2D + `/wavelengths` 1D) so the generic
HDF5 reader can decode them.

* ``primary_link.h5`` + ``linked.h5`` exercise ``ExternalLinkResolver``:
  ``primary_link.h5`` keeps ``/wavelengths`` locally and exposes
  ``/spectra`` through an HDF5 external link
  (``primary_link.h5::/spectra`` → ``linked.h5::/spectra``).
* ``primary_file.h5`` + ``external_dataset.h5`` exercise
  ``ExternalFileResolver``: ``primary_file.h5`` keeps ``/wavelengths``
  locally and declares ``/spectra`` with the HDF5 "external storage"
  layout. The raw bytes of the dataset live in ``external_dataset.h5``.

The Rust harness re-reads both pairs through ``open_with_sidecars`` and
the ``SidecarBackedExternal`` adapter wired in
``readers/hdf5_helpers.rs``.
"""

from __future__ import annotations

import os
from pathlib import Path

import h5py
import numpy as np

HERE = Path(__file__).resolve().parent

N_SAMPLES = 4
N_BANDS = 8

WAVELENGTHS = np.linspace(900.0, 1700.0, N_BANDS, dtype=np.float64)
SPECTRA = (np.arange(N_SAMPLES * N_BANDS, dtype=np.float64).reshape(N_SAMPLES, N_BANDS) + 1.0)


def _write_linked() -> None:
    target = HERE / "linked.h5"
    with h5py.File(target, "w") as fh:
        fh.create_dataset("spectra", data=SPECTRA)


def _write_primary_link() -> None:
    target = HERE / "primary_link.h5"
    with h5py.File(target, "w") as fh:
        fh.create_dataset("wavelengths", data=WAVELENGTHS)
        fh["spectra"] = h5py.ExternalLink("linked.h5", "/spectra")


def _write_external_pair() -> None:
    # The external file holds the raw float64 bytes of the spectra matrix.
    external_target = HERE / "external_dataset.h5"
    external_target.write_bytes(SPECTRA.tobytes())

    primary_target = HERE / "primary_file.h5"
    if primary_target.exists():
        primary_target.unlink()
    with h5py.File(primary_target, "w") as fh:
        fh.create_dataset("wavelengths", data=WAVELENGTHS)
        dset = fh.create_dataset(
            "spectra",
            shape=SPECTRA.shape,
            dtype=SPECTRA.dtype,
            external=[("external_dataset.h5", 0, SPECTRA.nbytes)],
        )
        # h5py drives external storage as writable; pushing the values
        # ensures the on-disk bytes in `external_dataset.h5` are the canonical
        # spectra matrix (matches the .write_bytes call above).
        dset[...] = SPECTRA


def _roundtrip_check() -> None:
    with h5py.File(HERE / "primary_link.h5", "r") as fh:
        assert np.allclose(fh["spectra"][...], SPECTRA)
        assert np.allclose(fh["wavelengths"][...], WAVELENGTHS)
    with h5py.File(HERE / "primary_file.h5", "r") as fh:
        assert np.allclose(fh["spectra"][...], SPECTRA)
        assert np.allclose(fh["wavelengths"][...], WAVELENGTHS)


def main() -> None:
    os.makedirs(HERE, exist_ok=True)
    _write_linked()
    _write_primary_link()
    _write_external_pair()
    _roundtrip_check()
    print(f"wrote and round-tripped fixtures under {HERE}")


if __name__ == "__main__":
    main()
