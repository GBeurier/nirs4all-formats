"""Byte-level exploration helpers."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ByteDiff:
    offset: int
    left: int | None
    right: int | None


def compare_bytes(left: bytes, right: bytes, *, limit: int | None = None) -> list[ByteDiff]:
    """Return differing byte offsets between two buffers."""

    max_len = max(len(left), len(right))
    diffs: list[ByteDiff] = []
    for offset in range(max_len):
        left_byte = left[offset] if offset < len(left) else None
        right_byte = right[offset] if offset < len(right) else None
        if left_byte != right_byte:
            diffs.append(ByteDiff(offset=offset, left=left_byte, right=right_byte))
            if limit is not None and len(diffs) >= limit:
                break
    return diffs


def find_pattern(haystack: bytes, pattern: bytes) -> list[int]:
    """Return all offsets where `pattern` occurs."""

    if not pattern:
        raise ValueError("pattern must not be empty")
    offsets: list[int] = []
    start = 0
    while True:
        offset = haystack.find(pattern, start)
        if offset < 0:
            return offsets
        offsets.append(offset)
        start = offset + 1
