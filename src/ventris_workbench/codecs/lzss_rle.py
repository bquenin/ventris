"""LZSS with optional run-length encoding, as used by Tales of Phantasia (SNES).

A block is a 9-byte header followed by the payload:

    byte 0      mode: 0x80 stored, 0x81 LZSS, 0x83 LZSS + RLE
    bytes 1-4   payload size, little-endian
    bytes 5-8   decompressed size, little-endian

The payload is a sequence of flag bytes, each followed by eight tokens, read from the low bit up.
A set bit is a literal byte. A clear bit is a two-byte token ``b1 b2``:

    back-reference   distance = b1 | (b2 & 0x0F) << 8,  length = (b2 >> 4) + 3
    short run        mode 0x83, b2 >> 4 == 0xF:  (b2 & 0x0F) + 3 copies of b1
    long run         mode 0x83, b2 == 0xF0:      b1 + 19 copies of the next byte

The distance counts back from the current end of the output, and a copy may overlap the bytes it writes.
"""

HEADER_SIZE = 9
MAX_DISTANCE = 0xFFF

MODE_STORED = 0x80
MODE_LZSS = 0x81
MODE_LZSS_RLE = 0x83


class CodecError(Exception):
    """The data is not a valid block."""


def read_header(data: bytes, pos: int = 0) -> tuple[int, int, int]:
    """Return ``(mode, payload size, decompressed size)`` for the block at ``pos``."""
    if pos + HEADER_SIZE > len(data):
        raise CodecError("truncated block header")
    mode = data[pos]
    payload_size = int.from_bytes(data[pos + 1 : pos + 5], "little")
    size = int.from_bytes(data[pos + 5 : pos + 9], "little")
    return mode, payload_size, size


def block_size(data: bytes, pos: int = 0) -> int:
    """Total size of the block at ``pos``, header included."""
    return HEADER_SIZE + read_header(data, pos)[1]


def decompress_block(data: bytes, pos: int = 0) -> bytes:
    """Decompress the block that starts at ``pos``."""
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
    if mode not in (MODE_LZSS, MODE_LZSS_RLE):
        raise CodecError(f"unsupported compression mode 0x{mode:02x}")

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
                    out.append(payload[pos])
                    pos += 1
                    continue
                b1, b2 = payload[pos], payload[pos + 1]
                pos += 2
                high, low = b2 >> 4, b2 & 0x0F
                if mode == MODE_LZSS_RLE and high == 0x0F:
                    if low:
                        out += bytes([b1]) * (low + 3)
                    else:
                        out += bytes([payload[pos]]) * (b1 + 19)
                        pos += 1
                    continue
                distance = b1 | (low << 8)
                if distance == 0 or distance > len(out):
                    raise CodecError(f"back-reference distance {distance} reaches before the start of the output")
                for _ in range(high + 3):
                    out.append(out[-distance])
    except IndexError:
        raise CodecError("block payload ends before the decompressed size is reached") from None
    if pos != len(payload):
        raise CodecError(f"block payload has {len(payload) - pos} unused bytes")
    # The original compressor lets the last token run past the end; the excess is dropped.
    return bytes(out[:size])


def compress_block(data: bytes) -> bytes:
    """Compress ``data`` into a block, header included, using whichever mode is smallest."""
    mode, payload = MODE_STORED, bytes(data)
    for candidate in (MODE_LZSS, MODE_LZSS_RLE):
        packed = compress(data, candidate)
        if len(packed) < len(payload):
            mode, payload = candidate, packed
    return bytes([mode]) + len(payload).to_bytes(4, "little") + len(data).to_bytes(4, "little") + payload


def compress(data: bytes, mode: int) -> bytes:
    """Compress ``data`` to a payload for ``mode``.

    The cheapest sequence of tokens for the whole block is chosen, not the longest match at each step,
    so the output is usually a little smaller than the original compressor's and never relies on its
    quirks. It decompresses to the same data but is not byte-identical to the original.
    """
    if mode == MODE_STORED:
        return bytes(data)
    if mode not in (MODE_LZSS, MODE_LZSS_RLE):
        raise CodecError(f"unsupported compression mode 0x{mode:02x}")
    rle = mode == MODE_LZSS_RLE
    max_match = 17 if rle else 18  # in RLE mode a length nibble of 0xF marks a run instead
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
                if distance > MAX_DISTANCE:
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
            out += bytes([data[pos], 0xF0 | (length - 3)]) if length <= 18 else bytes([length - 19, 0xF0, data[pos]])
        else:
            out += bytes([distance & 0xFF, (length - 3) << 4 | distance >> 8])
        bit += 1
        pos += length
    return bytes(out)
