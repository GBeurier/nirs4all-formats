#!/usr/bin/env python3
"""Rebuild the small registry fuzz corpus from tracked public fixtures."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "fuzz" / "corpus" / "registry_open_bytes"
SEEDS = (
    ("00-jcamp-peak-assignments", 0, "samples/jcamp_dx/synthetic_peak_assignments.jdx"),
    ("01-perkin-elmer-trailing", 1, "samples/perkin_elmer/synthetic_trailing.sp"),
    ("02-msa-minimum-metadata", 2, "samples/msa_iso22029/minimum_metadata.msa"),
    ("03-viavi-micronir", 3, "samples/viavi_micronir/synthetic_micronir.sam"),
    ("04-animl-autoincrement", 4, "samples/animl/synthetic_nirs_autoincrement.animl"),
)


def main() -> None:
    CORPUS.mkdir(parents=True, exist_ok=True)
    for name, selector, relative_source in SEEDS:
        source = ROOT / relative_source
        if "samples_local" in source.parts:
            raise ValueError(f"private fixture refused: {source}")
        payload = source.read_bytes()
        if len(payload) > 1024 * 1024:
            raise ValueError(f"seed exceeds target payload limit: {source}")
        (CORPUS / name).write_bytes(bytes([selector]) + payload)


if __name__ == "__main__":
    main()
