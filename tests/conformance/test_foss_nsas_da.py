"""DA full-array comparison against the independent MIT spc-spectra reader."""

import re
import struct
from datetime import datetime
from pathlib import Path

import pytest
from conftest import REPO_ROOT, require_nirs4all_formats, require_spc


def pairs() -> list[tuple[Path, Path]]:
    public = REPO_ROOT / "samples/foss_nsas_da"
    result = [(public / "synthetic.da", public / "synthetic.spc")]
    local = REPO_ROOT / "samples_local/foss_nsas_da"
    if (local / "0LC1B44.DA").exists() and (local / "0LC1B44.SPC").exists():
        result.append((local / "0LC1B44.DA", local / "0LC1B44.SPC"))
    return result


@pytest.mark.parametrize("paths", pairs(), ids=lambda p: p[0].stem)
def test_every_da_value_against_spc_spectra(paths: tuple[Path, Path], tmp_path: Path) -> None:
    nirs = require_nirs4all_formats()
    spc = require_spc()
    da_path, spc_path = paths
    raw = nirs.open_records(da_path)
    records = raw.to_dicts() if hasattr(raw, "to_dicts") else raw
    data = bytearray(spc_path.read_bytes())
    if da_path.stem == "0LC1B44":
        # Validate physical boundaries independently of DA before repairing a
        # TEMPORARY copy's inconsistent fnsub. Originals are never modified.
        assert struct.unpack_from("<I", data, 24)[0] == 7
        points = struct.unpack_from("<I", data, 4)[0]
        log_start = struct.unpack_from("<I", data, 248)[0]
        count, remainder = divmod(log_start - 512, 32 + points * 4)
        assert (points, count, remainder) == (1050, 75, 0)
        struct.pack_into("<I", data, 24, count)
    copy = tmp_path / "reference.spc"
    copy.write_bytes(data)
    reference = spc.File(str(copy))
    assert len(records) == len(reference.sub)
    for record, sub in zip(records, reference.sub, strict=True):
        signal = record["signals"]["absorbance"]
        assert signal["axis"]["values"] == [float(x) for x in reference.x]
        assert signal["values"] == [float(y) for y in sub.y]
    # Unknown bytes are part of recovery too: canonical output must permit an
    # exact reconstruction of the original, without requiring field semantics.
    rebuilt = b"".join(
        bytes.fromhex(record["metadata"]["da_header_hex"])
        + struct.pack("<1050f", *record["signals"]["absorbance"]["values"])
        + bytes.fromhex(record["metadata"]["da_trailer_hex"])
        for record in records
    )
    assert rebuilt == da_path.read_bytes()
    if da_path.stem == "0LC1B44":
        assert len({r["metadata"]["sample_id"] for r in records}) == 15
        assert len({tuple(r["signals"]["absorbance"]["values"]) for r in records}) == 75
        log_start = struct.unpack_from("<I", data, 248)[0]
        log = data[log_start + 64 :].decode("ascii")
        descriptions = re.findall(r'SUBFILE\d+ = ([^,]+), ([^,]+), "([^"]+)"', log)
        sensors = re.findall(r"SENSORVAL\d+ = ([^\r\n]+)", log)
        assert len(descriptions) == len(sensors) == 7
        for record, (date, clock, sample_id), sensor in zip(
            records[:7], descriptions, sensors, strict=True
        ):
            metadata = record["metadata"]
            assert metadata["sample_id"] == sample_id.strip()
            assert (
                metadata["acquired_local"]
                # Vendor clock is local; no timezone may be invented here.
                == datetime.strptime(  # noqa: DTZ007
                    f"{date.strip()} {clock.strip()}", "%m/%d/%Y %H:%M:%S"
                ).isoformat()
            )
            assert metadata["sensor_values"] == pytest.approx(
                [float(value) for value in sensor.split(",")], rel=1e-5, abs=1e-10
            )
