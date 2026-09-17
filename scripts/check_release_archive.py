#!/usr/bin/env python3
"""Fail when a public source archive contains private/local corpus material."""

from __future__ import annotations

import argparse
import tarfile
import zipfile
from collections.abc import Iterable
from pathlib import Path, PurePosixPath


def archive_members(path: Path) -> Iterable[str]:
    """Yield normalized member paths from a release tarball or zip archive."""

    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            yield from archive.namelist()
        return

    if tarfile.is_tarfile(path):
        with tarfile.open(path, "r:*") as archive:
            yield from archive.getnames()
        return

    raise ValueError(f"unsupported or invalid archive: {path}")


def forbidden_member(name: str) -> bool:
    """Return whether an archive member violates the public-source policy."""

    # Archive member names are specified with POSIX separators, but defensive
    # validation also treats backslashes as separators so a crafted zip cannot
    # disguise a private directory on either host platform. Policy names and
    # sensitive extensions are case-insensitive.
    member = PurePosixPath(name.replace("\\", "/"))
    parts = {part.casefold() for part in member.parts}
    member_name = member.name.casefold()
    suffix = member.suffix.casefold()
    return (
        "samples_local" in parts
        or member_name == "samples_local.tar.gz.enc"
        or suffix in {".enc", ".key"}
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("archives", nargs="+", type=Path)
    args = parser.parse_args()

    violations: list[str] = []
    for archive in args.archives:
        for member in archive_members(archive):
            if forbidden_member(member):
                violations.append(f"{archive}: {member}")

    if violations:
        print("private/local material found in public release archive:")
        print("\n".join(f"  {violation}" for violation in violations))
        return 1

    print(f"release archive policy passed for {len(args.archives)} archive(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
