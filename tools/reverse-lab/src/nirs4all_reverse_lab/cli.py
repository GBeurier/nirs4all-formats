"""Command-line entrypoint for reverse-lab helpers."""

from __future__ import annotations

import argparse
from pathlib import Path

from .bitdiff import compare_bytes, find_pattern


def main() -> None:
    parser = argparse.ArgumentParser(prog="nirs4all-reverse-lab")
    sub = parser.add_subparsers(dest="command", required=True)

    diff = sub.add_parser("diff", help="show differing byte offsets")
    diff.add_argument("left", type=Path)
    diff.add_argument("right", type=Path)
    diff.add_argument("--limit", type=int, default=32)

    scan = sub.add_parser("scan", help="scan for a byte pattern")
    scan.add_argument("path", type=Path)
    scan.add_argument("pattern", help="pattern encoded as hex, for example 4a43414d50")

    args = parser.parse_args()
    if args.command == "diff":
        diffs = compare_bytes(args.left.read_bytes(), args.right.read_bytes(), limit=args.limit)
        for diff_item in diffs:
            print(f"{diff_item.offset}: {diff_item.left!r} -> {diff_item.right!r}")
    elif args.command == "scan":
        pattern = bytes.fromhex(args.pattern)
        for offset in find_pattern(args.path.read_bytes(), pattern):
            print(offset)
