import random

import pytest

from ventris_workbench.codecs import lzss_ring
from ventris_workbench.codecs.lzss_ring import MODE_LZSS, MODE_LZSS_RLE, MODE_STORED, CodecError, decompress


def _block(mode: int, payload: bytes, size: int) -> bytes:
    return bytes([mode]) + len(payload).to_bytes(4, "little") + size.to_bytes(4, "little") + payload


def test_literals_only():
    assert decompress(b"\xff" + b"ABCDEFGH" + b"\x01" + b"I", MODE_LZSS, 9) == b"ABCDEFGHI"


def test_the_ring_starts_out_filled():
    ring = lzss_ring.initial_ring()
    assert len(ring) == 0x1000
    assert ring[0x480:0x488] == bytes.fromhex("90 00 90 00 90 00 90 00")  # value, 0, four times
    assert ring[0x800:0x807] == bytes.fromhex("00 ff 00 ff 00 ff 00")  # value, FF, three times, value
    assert ring[0x807:0x80E] == bytes.fromhex("01 ff 01 ff 01 ff 01")
    assert not any(ring[0xF00:])


def test_a_reference_can_read_what_was_never_written():
    # Place 0x485 in the untouched ring holds 00 90 00: a file can start by copying from it.
    assert decompress(bytes([0b0]) + bytes([0x85, 0x40]), MODE_LZSS, 3) == bytes.fromhex("00 90 00")


def test_a_reference_names_a_place_not_a_distance():
    # Mode 1 writes from 0xFEE: after "abc", place 0xFEE holds "a". Five bytes from there overlap the copy.
    payload = bytes([0b0111]) + b"abc" + bytes([0xEE, 0xF2])
    assert decompress(payload, MODE_LZSS, 8) == b"abcabcab"


def test_the_two_modes_start_writing_in_different_places():
    payload = bytes([0b01]) + b"x" + bytes([0xEE, 0xF0])  # one literal, then three bytes from 0xFEE
    assert decompress(payload, MODE_LZSS, 4) == b"xxxx"  # the literal went to 0xFEE, and the copy follows itself
    assert decompress(payload, MODE_LZSS_RLE, 4) == b"x\x00x\x00"  # here the literal went to 0xFEF


def test_runs():
    assert decompress(bytes([0b0]) + bytes([0x41, 0x2F]), MODE_LZSS_RLE, 5) == b"A" * 5  # short: 2 + 3
    assert decompress(bytes([0b0]) + bytes([0x01, 0x0F, 0x42]), MODE_LZSS_RLE, 20) == b"B" * 20  # long: 1 + 19
    # A run goes into the ring like anything else.
    payload = bytes([0b00]) + bytes([0x41, 0x1F]) + bytes([0xEF, 0xF1])
    assert decompress(payload, MODE_LZSS_RLE, 8) == b"A" * 8


def test_mode_1_has_no_runs():
    # The same two bytes are a reference of 18 bytes there, to place 0x041: the table entry for 8.
    assert decompress(bytes([0b0]) + bytes([0x41, 0x0F]), MODE_LZSS, 18) == bytes.fromhex("00 08 00 08 00 08 00") + (
        bytes.fromhex("09 00 09 00 09 00 09 00") + bytes.fromhex("0a 00 0a")
    )


def test_the_last_token_may_run_past_the_end():
    assert decompress(bytes([0b0]) + bytes([0x41, 0x2F]), MODE_LZSS_RLE, 2) == b"AA"


def test_blocks():
    packed = _block(MODE_LZSS_RLE, bytes([0b01]) + b"Z" + bytes([0x5A, 0x1F]), 5)
    assert lzss_ring.is_block(packed)
    assert lzss_ring.read_header(packed) == (MODE_LZSS_RLE, 4, 5)
    assert lzss_ring.decompress_block(packed) == b"ZZZZZ"
    assert lzss_ring.decompress_block(b"??" + packed, 2) == b"ZZZZZ"
    assert lzss_ring.decompress_block(_block(MODE_STORED, b"plain", 5)) == b"plain"
    assert not lzss_ring.is_block(packed + b"\x00")  # the payload size must account for everything
    assert not lzss_ring.is_block(b"SCPK" + bytes(12))
    assert not lzss_ring.is_block(b"\x03")


def test_errors():
    with pytest.raises(CodecError, match="ends before"):
        decompress(b"\xff" + b"AB", MODE_LZSS, 8)
    with pytest.raises(CodecError, match="unused bytes"):
        decompress(b"\xff" + b"ABCDEFGH" + b"\x00", MODE_LZSS, 8)
    with pytest.raises(CodecError, match="unsupported"):
        decompress(b"", 2, 0)
    with pytest.raises(CodecError, match="stored block"):
        decompress(b"abc", MODE_STORED, 4)
    with pytest.raises(CodecError, match="truncated block header"):
        lzss_ring.read_header(b"\x03\x00")
    with pytest.raises(CodecError, match="truncated block payload"):
        lzss_ring.decompress_block(_block(MODE_LZSS, b"\xff" + b"ABCDEFGH", 8)[:-2])


def test_compress_round_trips():
    generator = random.Random(1)
    samples = [b"", b"a", b"ab" * 40, bytes(600), bytes(range(256)) * 20, b"abc" + bytes(300) + b"abc" * 9]
    samples += [bytes(generator.choice(b"ab\x00") for _ in range(size)) for size in (5, 300, 9000)]
    samples += [generator.randbytes(2000)]
    for data in samples:
        for mode in (MODE_LZSS, MODE_LZSS_RLE, MODE_STORED):
            assert decompress(lzss_ring.compress(data, mode), mode, len(data)) == data
            assert lzss_ring.decompress_block(lzss_ring.compress_block(data, mode)) == data


def test_compress_uses_runs_and_references():
    assert lzss_ring.compress(b"A" * 5, MODE_LZSS_RLE) == bytes([0b0]) + bytes([0x41, 0x2F])
    assert lzss_ring.compress(b"B" * 20, MODE_LZSS_RLE) == bytes([0b0]) + bytes([0x01, 0x0F, 0x42])
    # "abc" again, three bytes back: in mode 3 the first byte went to 0xFEF.
    assert lzss_ring.compress(b"abcabc", MODE_LZSS_RLE) == bytes([0b0111]) + b"abc" + bytes([0xEF, 0xF0])
    assert lzss_ring.compress(b"abcabc", MODE_LZSS) == bytes([0b0111]) + b"abc" + bytes([0xEE, 0xF0])
    assert len(lzss_ring.compress(bytes(4000) + b"xyz" * 500, MODE_LZSS_RLE)) < 300
    assert lzss_ring.compress_block(b"A" * 5) == bytes([3, 3, 0, 0, 0, 5, 0, 0, 0, 0b0, 0x41, 0x2F])
    with pytest.raises(CodecError, match="unsupported"):
        lzss_ring.compress(b"abc", 2)
