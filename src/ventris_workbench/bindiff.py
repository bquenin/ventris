"""Byte-level comparison of two binaries."""

from collections.abc import Iterator

_CHUNK = 4096


def diff_ranges(a: bytes, b: bytes) -> Iterator[tuple[int, int]]:
    """Yield half-open ``(start, end)`` ranges where ``a`` and ``b`` differ.

    Only the common length is compared; a size difference is the caller's concern.
    """
    common = min(len(a), len(b))
    va, vb = memoryview(a), memoryview(b)
    start = -1
    for base in range(0, common, _CHUNK):
        stop = min(base + _CHUNK, common)
        if va[base:stop] == vb[base:stop]:
            if start >= 0:
                yield start, base
                start = -1
            continue
        for i in range(base, stop):
            if a[i] != b[i]:
                if start < 0:
                    start = i
            elif start >= 0:
                yield start, i
                start = -1
    if start >= 0:
        yield start, common


def merge_ranges(ranges: Iterator[tuple[int, int]], gap: int) -> list[tuple[int, int]]:
    """Merge ranges separated by at most ``gap`` unchanged bytes."""
    merged: list[tuple[int, int]] = []
    for start, end in ranges:
        if merged and start - merged[-1][1] <= gap:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged
