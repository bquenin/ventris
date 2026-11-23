import random
import zlib

import pytest

from ventris_workbench.bindiff import diff_ranges, merge_ranges
from ventris_workbench.patch import PatchError, _write_varint, apply_bps, apply_ips, apply_patch, create_bps


def _ips(*records: bytes, truncate: int | None = None) -> bytes:
    tail = b"EOF" + (truncate.to_bytes(3, "big") if truncate is not None else b"")
    return b"PATCH" + b"".join(records) + tail


def _record(offset: int, data: bytes) -> bytes:
    return offset.to_bytes(3, "big") + len(data).to_bytes(2, "big") + data


def _rle(offset: int, count: int, value: int) -> bytes:
    return offset.to_bytes(3, "big") + b"\x00\x00" + count.to_bytes(2, "big") + bytes([value])


def test_ips_plain_rle_and_extension():
    source = bytes(16)
    patch = _ips(_record(2, b"\xaa\xbb"), _rle(8, 4, 0x11), _record(20, b"\x99"))
    result = apply_ips(source, patch)
    assert result[2:4] == b"\xaa\xbb"
    assert result[8:12] == b"\x11" * 4
    assert len(result) == 21
    assert result[16:20] == bytes(4)
    assert result[20] == 0x99


def test_ips_truncation():
    assert apply_ips(bytes(16), _ips(_record(0, b"\x01"), truncate=8)) == b"\x01" + bytes(7)


def test_ips_offset_that_spells_eof():
    offset = int.from_bytes(b"EOF", "big")
    result = apply_ips(b"", _ips(_record(offset, b"\x42")))
    assert len(result) == offset + 1
    assert result[offset] == 0x42


def test_ips_missing_eof():
    with pytest.raises(PatchError):
        apply_ips(bytes(4), b"PATCH" + _record(0, b"\x01"))


def test_diff_ranges_and_merge():
    a = bytearray(10000)
    b = bytearray(10000)
    b[5] = 1
    b[4095:4098] = b"\x01\x01\x01"  # spans a chunk boundary
    b[9999] = 1
    ranges = list(diff_ranges(bytes(a), bytes(b)))
    assert ranges == [(5, 6), (4095, 4098), (9999, 10000)]
    assert merge_ranges(iter([(0, 2), (4, 6), (20, 22)]), gap=2) == [(0, 6), (20, 22)]


@pytest.mark.parametrize("target_size", [3000, 5000, 8000])
def test_bps_round_trip(target_size):
    rng = random.Random(target_size)
    source = bytes(rng.randrange(256) for _ in range(5000))
    target = bytearray(source[:target_size].ljust(target_size, b"\x00"))
    for _ in range(40):
        target[rng.randrange(target_size)] = rng.randrange(256)
    target = bytes(target)
    patch = create_bps(source, target)
    assert apply_patch(source, patch) == target
    assert len(patch) < len(target)


def test_bps_identical_and_empty():
    data = bytes(range(200))
    assert apply_bps(data, create_bps(data, data)) == data
    assert apply_bps(b"", create_bps(b"", b"abc")) == b"abc"
    assert apply_bps(b"abc", create_bps(b"abc", b"")) == b""


def test_bps_copy_commands():
    source = b"0123456789"
    target = b"6789" + b"ab" * 4
    body = bytearray(b"BPS1")
    body += _write_varint(len(source)) + _write_varint(len(target)) + _write_varint(0)
    body += _write_varint(((4 - 1) << 2) | 2) + _write_varint(6 << 1)  # SourceCopy 4 bytes from offset 6
    body += _write_varint(((2 - 1) << 2) | 1) + b"ab"  # TargetRead "ab"
    body += _write_varint(((6 - 1) << 2) | 3) + _write_varint(4 << 1)  # TargetCopy 6 bytes from output offset 4
    body += zlib.crc32(source).to_bytes(4, "little") + zlib.crc32(target).to_bytes(4, "little")
    body += zlib.crc32(body).to_bytes(4, "little")
    assert apply_bps(source, bytes(body)) == target


def test_bps_rejects_wrong_source_and_corruption():
    source, target = b"hello world" * 10, b"hello there" * 10
    patch = create_bps(source, target)
    with pytest.raises(PatchError, match="different file"):
        apply_bps(source[:-1] + b"!", patch)
    corrupt = bytearray(patch)
    corrupt[10] ^= 0xFF
    with pytest.raises(PatchError, match="corrupt"):
        apply_bps(source, bytes(corrupt))
