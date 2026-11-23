"""LZSS over a ring of 4096 bytes that starts out filled, as used by Tales of Destiny 2 (PlayStation 2).

It is a close relative of ``lzss_rle``, from the same developer: the same 9-byte header, the same flag
bytes and the same idea of runs. What differs is that a reference names a place in a ring buffer
instead of a distance back, and that the ring holds a fixed pattern before anything is written, so
that even the first bytes of a file can be copied from it.

A block is a 9-byte header followed by the payload:

    byte 0      mode: 0 stored, 1 LZSS, 3 LZSS with runs
    bytes 1-4   payload size, little-endian
    bytes 5-8   size once expanded, little-endian

The payload is a sequence of flag bytes, each followed by eight tokens, read from the low bit up.
A set bit is a literal byte. A clear bit is a two-byte token ``b1 b2``:

    reference    place in the ring = b1 | (b2 & 0xF0) << 4,  length = (b2 & 0x0F) + 3
    short run    mode 3, b2 & 0x0F == 0x0F, b2 >> 4 != 0:  (b2 >> 4) + 3 copies of b1
    long run     mode 3, b2 == 0x0F:                        b1 + 19 copies of the next byte

Every byte that comes out is also written to the ring. Writing starts at 0xFEE in mode 1 and at 0xFEF
in mode 3, and a reference may read what it is writing.

The compressor here never copies from the pattern the ring starts with, only from what it has
written, so its output is valid but not byte-identical to the original compressor's.
"""

from ventris_workbench.codecs.lzss_rle import CodecError

HEADER_SIZE = 9
RING_SIZE = 0x1000

MODE_STORED = 0
MODE_LZSS = 1
MODE_LZSS_RLE = 3

_FIRST_WRITE = {MODE_LZSS: 0xFEE, MODE_LZSS_RLE: 0xFEF}


def initial_ring() -> bytearray:
    """The ring before anything is written: two tables of every byte value, then zeros."""
    ring = bytearray(RING_SIZE)
    position = 0
    for value in range(0x100):  # value, 0, four times over
        ring[position : position + 8] = bytes([value, 0]) * 4
        position += 8
    for value in range(0x100):  # value, 0xFF, three times over, and the value once more
        ring[position : position + 7] = bytes([value, 0xFF]) * 3 + bytes([value])
        position += 7
    return ring


def read_header(data: bytes, pos: int = 0) -> tuple[int, int, int]:
    """Return ``(mode, payload size, expanded size)`` for the block at ``pos``."""
    if pos + HEADER_SIZE > len(data):
        raise CodecError("truncated block header")
    return (
        data[pos],
        int.from_bytes(data[pos + 1 : pos + 5], "little"),
        int.from_bytes(data[pos + 5 : pos + 9], "little"),
    )


def is_block(data: bytes) -> bool:
    """Whether ``data`` is exactly one block: a known mode, and a payload size that accounts for all of it."""
    if len(data) < HEADER_SIZE:
        return False
    mode, payload_size, _ = read_header(data)
    return mode in (MODE_STORED, MODE_LZSS, MODE_LZSS_RLE) and HEADER_SIZE + payload_size == len(data)


def decompress_block(data: bytes, pos: int = 0) -> bytes:
    """Expand the block that starts at ``pos``."""
    mode, payload_size, size = read_header(data, pos)
    start = pos + HEADER_SIZE
    payload = data[start : start + payload_size]
    if len(payload) != payload_size:
        raise CodecError("truncated block payload")
    return decompress(payload, mode, size)


def decompress(payload: bytes, mode: int, size: int) -> bytes:
    if mode == MODE_STORED:
        if len(payload) != size:
            raise CodecError("stored block size does not match its header")
        return bytes(payload)
    if mode not in _FIRST_WRITE:
        raise CodecError(f"unsupported compression mode {mode}")

    ring = initial_ring()
    write = _FIRST_WRITE[mode]
    out = bytearray()
    pos = 0
    try:
        while len(out) < size:
            flags = payload[pos]
            pos += 1
            for bit in range(8):
                if len(out) >= size:
                    break
                if flags >> bit & 1:
                    run = payload[pos : pos + 1]
                    pos += 1
                else:
                    b1, b2 = payload[pos], payload[pos + 1]
                    pos += 2
                    if mode == MODE_LZSS_RLE and b2 & 0x0F == 0x0F:
                        if b2 >> 4:
                            run = bytes([b1]) * ((b2 >> 4) + 3)
                        else:
                            run = bytes([payload[pos]]) * (b1 + 19)
                            pos += 1
                    else:
                        # Byte by byte, because the place read may be the place being written.
                        read = b1 | (b2 & 0xF0) << 4
                        for _ in range((b2 & 0x0F) + 3):
                            byte = ring[read]
                            ring[write] = byte
                            out.append(byte)
                            read = (read + 1) % RING_SIZE
                            write = (write + 1) % RING_SIZE
                        continue
                for byte in run:
                    ring[write] = byte
                    write = (write + 1) % RING_SIZE
                out += run
    except IndexError:
        raise CodecError("block payload ends before the expanded size is reached") from None
    if pos != len(payload):
        raise CodecError(f"block payload has {len(payload) - pos} unused bytes")
    # The original compressor lets the last token run past the end; the excess is dropped.
    return bytes(out[:size])


def compress_block(data: bytes, mode: int = MODE_LZSS_RLE) -> bytes:
    """Compress ``data`` into a block, header included."""
    payload = compress(data, mode)
    return bytes([mode]) + len(payload).to_bytes(4, "little") + len(data).to_bytes(4, "little") + payload


def compress(data: bytes, mode: int) -> bytes:
    """Compress ``data`` to a payload for ``mode``.

    The cheapest sequence of tokens for the whole block is chosen, as in ``lzss_rle.compress``.
    """
    if mode == MODE_STORED:
        return bytes(data)
    if mode not in _FIRST_WRITE:
        raise CodecError(f"unsupported compression mode {mode}")
    rle = mode == MODE_LZSS_RLE
    max_match = 17 if rle else 18  # in mode 3 a length nibble of 0xF marks a run instead
    size = len(data)

    # The longest earlier match at each position. Any shorter length is available at the same distance.
    longest = [(0, 0)] * size
    positions: dict[bytes, list[int]] = {}
    for pos in range(size):
        limit = min(max_match, size - pos)
        best_length, best_distance = 0, 0
        if limit >= 3:
            for start in reversed(positions.get(data[pos : pos + 3], ())):
                distance = pos - start
                if distance >= RING_SIZE:  # by then the ring has been written over
                    break
                length = 3
                while length < limit and data[start + length] == data[pos + length]:
                    length += 1
                if length > best_length:
                    best_length, best_distance = length, distance
                    if length == limit:
                        break
        longest[pos] = (best_length, best_distance)
        positions.setdefault(data[pos : pos + 3], []).append(pos)

    # How many times the byte at each position repeats from there on.
    run = [1] * size
    for pos in range(size - 2, -1, -1):
        if data[pos] == data[pos + 1]:
            run[pos] = min(run[pos + 1] + 1, 274)

    # Cheapest cost in bits from each position to the end: a literal is 9, a two-byte token 17,
    # a long run 25. ``step`` records the token chosen: (length, distance), distance 0 for a run.
    cost = [0] * (size + 1)
    step: list[tuple[int, int]] = [(1, -1)] * size
    for pos in range(size - 1, -1, -1):
        best, choice = cost[pos + 1] + 9, (1, -1)
        length, distance = longest[pos]
        for take in range(3, length + 1):
            if cost[pos + take] + 17 < best:
                best, choice = cost[pos + take] + 17, (take, distance)
        if rle:
            for take in range(4, run[pos] + 1):
                price = cost[pos + take] + (17 if take <= 18 else 25)
                if price < best:
                    best, choice = price, (take, 0)
        cost[pos], step[pos] = best, choice

    out = bytearray()
    flag_at, bit = 0, 8  # forces a new flag byte before the first token
    pos = 0
    while pos < size:
        if bit == 8:
            flag_at, bit = len(out), 0
            out.append(0)
        length, distance = step[pos]
        if distance < 0:
            out[flag_at] |= 1 << bit
            out.append(data[pos])
        elif distance == 0:
            out += (
                bytes([data[pos], (length - 3) << 4 | 0x0F]) if length <= 18 else bytes([length - 19, 0x0F, data[pos]])
            )
        else:
            place = (_FIRST_WRITE[mode] + pos - distance) % RING_SIZE
            out += bytes([place & 0xFF, place >> 4 & 0xF0 | (length - 3)])
        bit += 1
        pos += length
    return bytes(out)
