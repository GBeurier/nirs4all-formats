"""Export of records through the native writers."""

from __future__ import annotations

from pathlib import Path

import pytest

import nirs4all_formats as formats
from nirs4all_formats import _compat

pytestmark = pytest.mark.skipif(_compat._native is None, reason="native extension not built")

CSV = "sample_id,protein,1000,1002,1004\na,12.5,0.5,0.25,0.75\nb,11,0.5,,0.25\n"


@pytest.fixture
def source(tmp_path: Path) -> Path:
    path = tmp_path / "set.csv"
    path.write_text(CSV)
    return path


def test_targets_list_formats_and_presets() -> None:
    targets = formats.export_targets()
    assert {"jcamp-dx", "galactic-spc", "xlsx"} <= {item["format"] for item in targets["formats"]}
    assert not any(item["tested"] for item in targets["software"])


def test_records_round_trip_through_a_preset(source: Path) -> None:
    record = formats.open_records(source)[0]
    output = formats.write_records([record], "opus", base_name="set")
    assert [file["name"] for file in output["files"]] == ["set.dx"]
    assert "targets_in_comments" in {loss["code"] for loss in output["losses"]}
    back = formats.open_bytes("set.dx", output["files"][0]["bytes"])
    (signal,) = back[0]["signals"].values()
    assert signal["values"] == [0.5, 0.25, 0.75]


def test_missing_values_survive_tables_and_are_refused_elsewhere(source: Path) -> None:
    records = formats.open_records(source)
    table = formats.write_records(records, "csv")["files"][0]["bytes"].decode()
    assert table.splitlines()[2] == "b,11,0.5,,0.25"
    with pytest.raises(ValueError, match="missing or non-finite"):
        formats.write_records(records, "galactic-spc")


def test_invalid_requests_are_refused(source: Path) -> None:
    records = formats.open_records(source)
    with pytest.raises(ValueError, match="unknown export target"):
        formats.write_records(records, "opus-native")
    with pytest.raises(ValueError, match="unknown field"):
        formats.write_records(records, "csv", nope=1)
    with pytest.raises(ValueError):
        formats.write_records([{"signals": {}}], "csv")


def test_convert_path_never_overwrites_silently(source: Path, tmp_path: Path) -> None:
    out = tmp_path / "out"
    out.mkdir()
    report = formats.convert_path(source, "xlsx", out)
    assert report["files"] == [out / "set.xlsx"]
    with pytest.raises(FileExistsError):
        formats.convert_path(source, "xlsx", out)
    formats.convert_path(source, "xlsx", out, overwrite=True)
