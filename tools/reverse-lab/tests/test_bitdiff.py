from nirs4all_reverse_lab import compare_bytes, find_pattern


def test_compare_bytes_reports_offsets() -> None:
    diffs = compare_bytes(b"abc", b"axcd")
    assert [diff.offset for diff in diffs] == [1, 3]


def test_find_pattern_reports_overlaps() -> None:
    assert find_pattern(b"aaaa", b"aa") == [0, 1, 2]
