import random

import pytest

from ventris_workbench.codecs import lzss_rle
from ventris_workbench.codecs.lzss_rle import CodecError, decompress


def _block(mode: int, payload: bytes, size: int) -> bytes:
    return bytes([mode]) + len(payload).to_bytes(4, "little") + size.to_bytes(4, "little") + payload


def test_literals_only():
    assert decompress(b"\xff" + b"ABCDEFGH" + b"\x01" + b"I", lzss_rle.MODE_LZSS, 9) == b"ABCDEFGHI"


def test_back_reference_counts_from_the_end_of_the_output():
    # Three literals, then copy 4 bytes from 3 back: the copy overlaps what it writes.
    payload = bytes([0b0111]) + b"abc" + bytes([0x03, 0x10])
    assert decompress(payload, lzss_rle.MODE_LZSS, 7) == b"abcabca"


def test_twelve_bit_distance():
    body = bytes(range(256)) * 2
    payload = bytearray()
    for start in range(0, len(body), 8):
        payload += b"\xff" + body[start : start + 8]
    payload += bytes([0x00]) + bytes([0x00, 0x02])  # distance 0x200, length 3
    assert decompress(bytes(payload), lzss_rle.MODE_LZSS, len(body) + 3) == body + body[:3]


def test_runs_only_exist_in_rle_mode():
    short = bytes([0b0]) + bytes([0x41, 0xF2])  # 5 x 'A'
    assert decompress(short, lzss_rle.MODE_LZSS_RLE, 5) == b"AAAAA"
    long = bytes([0b0]) + bytes([0x01, 0xF0, 0x42])  # 20 x 'B'
    assert decompress(long, lzss_rle.MODE_LZSS_RLE, 20) == b"B" * 20
    # In plain LZSS mode the same token is a back-reference of length 18.
    plain = bytes([0b01]) + b"Z" + bytes([0x01, 0xF0])
    assert decompress(plain, lzss_rle.MODE_LZSS, 19) == b"Z" * 19


def test_last_token_may_overrun_the_declared_size():
    payload = bytes([0b01]) + b"x" + bytes([0x01, 0x70])  # 1 literal + 10 copies
    assert decompress(payload, lzss_rle.MODE_LZSS, 6) == b"x" * 6


def test_stored_block_and_header():
    data = b"prefix" + _block(lzss_rle.MODE_STORED, b"hello", 5) + b"suffix"
    assert lzss_rle.read_header(data, 6) == (lzss_rle.MODE_STORED, 5, 5)
    assert lzss_rle.block_size(data, 6) == 14
    assert lzss_rle.decompress_block(data, 6) == b"hello"


@pytest.mark.parametrize(
    ("payload", "mode", "size", "message"),
    [
        (b"\xff" + b"ab", lzss_rle.MODE_LZSS, 5, "ends before"),
        (bytes([0b0]) + bytes([0x05, 0x00]), lzss_rle.MODE_LZSS, 3, "before the start"),
        (bytes([0b1]) + b"a", lzss_rle.MODE_LZSS, 1, None),
        (bytes([0b1]) + b"ab", lzss_rle.MODE_LZSS, 1, "unused"),
        (b"abc", 0x85, 3, "unsupported"),
        (b"abc", lzss_rle.MODE_STORED, 4, "stored block"),
    ],
)
def test_errors(payload, mode, size, message):
    if message is None:
        assert decompress(payload, mode, size) == b"a"
        return
    with pytest.raises(CodecError, match=message):
        decompress(payload, mode, size)


@pytest.mark.parametrize("mode", [lzss_rle.MODE_STORED, lzss_rle.MODE_LZSS, lzss_rle.MODE_LZSS_RLE])
def test_compress_round_trip(mode):
    rng = random.Random(mode)
    words = [bytes(rng.randrange(256) for _ in range(rng.randrange(2, 9))) for _ in range(40)]
    samples = [
        b"",
        b"a",
        b"ab",
        b"abcabcabcabcabcabcabcabcabcabcabcabc",
        b"\x00" * 3,
        b"\x00" * 4,
        b"\x00" * 18,
        b"\x00" * 19,
        b"\x00" * 274,
        b"\x00" * 1000 + b"x" + b"\xff" * 300,
        bytes(rng.randrange(256) for _ in range(3000)),
        b"".join(rng.choice(words) for _ in range(3000)),  # longer than the 4095-byte window
    ]
    for data in samples:
        payload = lzss_rle.compress(data, mode)
        assert lzss_rle.decompress(payload, mode, len(data)) == data


def test_compress_block_picks_the_smallest_mode():
    runs = lzss_rle.compress_block(b"\x07" * 500 + b"abc")
    assert runs[0] == lzss_rle.MODE_LZSS_RLE
    assert len(runs) < 20
    assert lzss_rle.decompress_block(runs) == b"\x07" * 500 + b"abc"
    rng = random.Random(1)
    noise = bytes(rng.randrange(256) for _ in range(64))
    stored = lzss_rle.compress_block(noise)
    assert stored[0] == lzss_rle.MODE_STORED
    assert lzss_rle.decompress_block(stored) == noise
    with pytest.raises(CodecError, match="unsupported"):
        lzss_rle.compress(b"abc", 0x85)


def test_truncated_block():
    with pytest.raises(CodecError, match="header"):
        lzss_rle.read_header(b"\x81\x00")
    with pytest.raises(CodecError, match="payload"):
        lzss_rle.decompress_block(_block(lzss_rle.MODE_LZSS, b"\xff", 1)[:-1])
