"""Full-array DA reference comparison; GPL aquap2 stays in an R subprocess."""

import hashlib
import os
import shutil
import struct
import subprocess
from pathlib import Path

import pytest
from conftest import REPO_ROOT, require_nirs4all_formats

# bpollner/aquap2 commit 57ee096fbecf28a321eed5884784e01b0bca036b,
# R/prep_importData_specific.r. Neither fetched by tests nor vendored in runtime.
SOURCE_SHA256 = "2d96e735d82a2753f63126ebbb8ff13092f1e2c50970783189b322b7a76222b4"


def da_files() -> list[Path]:
    public = [REPO_ROOT / "samples/foss_nsas_da/synthetic.da"]
    local = REPO_ROOT / "samples_local/foss_nsas_da"
    return public + [
        p
        for name in ("0LC1B44.DA", "0LC1B70.DA", "0LC1B83.DA")
        if (p := local / name).is_file()
    ]


@pytest.mark.parametrize("path", da_files(), ids=lambda p: p.stem)
def test_every_da_value_against_aquap2(path: Path, tmp_path: Path) -> None:
    source_env = os.environ.get("NIRS4ALL_AQUAP2_NSAS_SOURCE")
    if not source_env:
        pytest.skip("set NIRS4ALL_AQUAP2_NSAS_SOURCE to the pinned aquap2 R source")
    source = Path(source_env).resolve(strict=True)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == SOURCE_SHA256
    rscript = shutil.which("Rscript")
    if rscript is None:
        pytest.skip("Rscript not available for the isolated aquap2 reference")
    prefix = tmp_path / "reference"
    subprocess.run(
        [
            rscript,
            "--vanilla",
            str(Path(__file__).parent / "refreaders/nsas_aquap2_dump.R"),
            str(source),
            str(path),
            str(prefix),
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=60,
    )
    raw = require_nirs4all_formats().open_records(path)
    records = raw.to_dicts() if hasattr(raw, "to_dicts") else raw
    values = [y for r in records for y in r["signals"]["absorbance"]["values"]]
    assert (
        struct.pack(f"<{len(values)}d", *values)
        == prefix.with_suffix(".f64").read_bytes()
    )
    axis = [float(x) for x in prefix.with_suffix(".axis.txt").read_text().splitlines()]
    assert all(r["signals"]["absorbance"]["axis"]["values"] == axis for r in records)
    timestamps = prefix.with_suffix(".timestamps.txt").read_text().splitlines()
    assert [r["metadata"]["acquired_local"] for r in records] == timestamps
    # aquap2 make.unique() alters repeated names and drops stored record numbers;
    # neither is an independent identity reference, so they are not compared here.
