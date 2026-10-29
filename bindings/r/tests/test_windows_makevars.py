"""Execute Windows R build recipes with a recording Cargo, without cross-compiling."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
MAKEVARS = REPO / "bindings/r/nirs4allformats/src/Makevars.win"
LIBRARY = "libnirs4allformats_r.a"

CARGO = r"""#!__PYTHON__
import json, os, sys
from pathlib import Path
if '--version' in sys.argv:
    print('cargo 1.88.0')
    raise SystemExit(0)
if os.environ.get('FAIL_CARGO'):
    raise SystemExit(17)
args = sys.argv[1:]
target = args[args.index('--target') + 1]
record = {'args': args, 'target': target, 'target_dir': os.environ['CARGO_TARGET_DIR'],
          'x64_linker': os.environ.get('CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER')}
Path(os.environ['CARGO_RECORD']).write_text(json.dumps(record))
assert target in ['x86_64-pc-windows-gnu', 'aarch64-pc-windows-gnullvm']
artifact = Path(record['target_dir']) / target / 'release' / os.environ['CARGO_LIBRARY']
artifact.parent.mkdir(parents=True, exist_ok=True)
artifact.write_bytes(b'recording-Cargo artifact; not a compiled library')
"""


def prepare(
    tmp_path: Path, *, service: bool, win: str, target: str, fail: bool = False
):
    source = tmp_path / "src"
    (source / "rust").mkdir(parents=True)
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    tool_dir = tmp_path / "tool bin"
    tool_dir.mkdir()
    host = tool_dir / "cargo"
    host.write_text(CARGO.replace("__PYTHON__", sys.executable))
    host.chmod(0o755)
    cargo = bin_dir / "cargo"
    cargo.write_text(
        "#!/bin/sh\necho unexpected-service-shim >&2\nexit 91\n"
        if service
        else host.read_text()
    )
    cargo.chmod(0o755)
    rustc = bin_dir / "rustc"
    rustc.write_text("#!/bin/sh\necho 'rustc 1.88.0'\n")
    rustc.chmod(0o755)
    # Exercise the installed, vendored (offline) branch, rather than a dev fallback.
    vendor = tmp_path / "vendor"
    vendor.mkdir()
    (vendor / "marker").write_text("fixture")
    with tarfile.open(source / "rust/vendor.tar.xz", "w:xz") as archive:
        archive.add(vendor, arcname="vendor")
    private = tmp_path / "private"
    private.mkdir()
    makefile = source / "witness.mk"
    makefile.write_text(
        f'SHLIB = package.dll\ninclude {MAKEVARS}\n.PHONY: witness\nwitness: $(STATLIB)\n\t@test -f "$(STATLIB)"\n'
    )
    env = os.environ.copy()
    for key in [
        "HOST_CARGO",
        "CARGO_BUILD_TARGET",
        "TARGET",
        "CARGO_TARGET_DIR",
        "MAKEFLAGS",
        "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER",
    ]:
        env.pop(key, None)
    env.update(
        PATH=str(bin_dir) + os.pathsep + env["PATH"],
        CARGO_RECORD=str(tmp_path / "cargo.json"),
        CARGO_LIBRARY=LIBRARY,
    )
    if service:
        env.update(HOST_CARGO=str(host), CARGO_BUILD_TARGET=target)
    if fail:
        env["FAIL_CARGO"] = "1"
    command = [
        "make",
        "--no-print-directory",
        "-f",
        str(makefile),
        "witness",
        f"WIN={win}",
        f"TARGET_DIR={private / 'target'}",
        f"CARGOTMP={private / 'cargo-home'}",
        f"VENDOR_ROOT={private / 'vendor-root'}",
        f"N4IO_BUILD_DIR_POSIX={private}",
        f"N4IO_BUILD_DIR={private}",
        f"N4DS_CARGO={host if service else cargo}",
        f"N4DS_RUSTC={rustc}",
        "CARGO_LINKER=historical-x64-linker",
    ]
    return source, env, command


@pytest.mark.parametrize(
    ("service", "win", "target"),
    [
        (False, "64", "x86_64-pc-windows-gnu"),
        (True, "", "aarch64-pc-windows-gnullvm"),
        (True, "64", "aarch64-pc-windows-gnullvm"),
        (True, "", "x86_64-pc-windows-gnu"),
    ],
)
def test_recorded_artifact_is_the_library_used_by_r(
    tmp_path: Path, service: bool, win: str, target: str
):
    source, env, command = prepare(tmp_path, service=service, win=win, target=target)
    dry = subprocess.run(
        command + ["--dry-run"],
        cwd=source,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert dry.returncode == 0, dry.stderr
    assert f"--target {target}" in dry.stdout
    assert f"/{target}/release/" in dry.stdout or "test -f" in dry.stdout
    result = subprocess.run(
        command, cwd=source, env=env, capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stdout + result.stderr
    record = json.loads((tmp_path / "cargo.json").read_text())
    assert record["target"] == target
    assert "--offline" in record["args"]
    assert record["x64_linker"] == (
        "historical-x64-linker" if target == "x86_64-pc-windows-gnu" else None
    )
    assert (Path(record["target_dir"]) / target / "release" / LIBRARY).is_file()
    assert not (source / "rust/target").exists()
    assert "unexpected-service-shim" not in result.stderr


def test_cargo_failure_cannot_be_reported_as_a_successful_r_library(tmp_path: Path):
    source, env, command = prepare(
        tmp_path, service=True, win="", target="aarch64-pc-windows-gnullvm", fail=True
    )
    result = subprocess.run(
        command, cwd=source, env=env, capture_output=True, text=True, check=False
    )
    assert result.returncode != 0
    assert not (tmp_path / "cargo.json").exists()
