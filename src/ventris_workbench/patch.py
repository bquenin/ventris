"""Apply and create binary patches (IPS, BPS)."""

import zlib

from ventris_workbench.bindiff import diff_ranges, merge_ranges


class PatchError(Exception):
    """The patch is malformed or does not match the file it is applied to."""


def detect_format(patch: bytes) -> str:
    if patch.startswith(b"PATCH"):
        return "ips"
    if patch.startswith(b"BPS1"):
        return "bps"
    raise PatchError("unrecognized patch format (expected IPS or BPS)")


def apply_patch(source: bytes, patch: bytes) -> bytes:
    fmt = detect_format(patch)
    return apply_ips(source, patch) if fmt == "ips" else apply_bps(source, patch)


# --- IPS -------------------------------------------------------------------


def apply_ips(source: bytes, patch: bytes) -> bytes:
    if not patch.startswith(b"PATCH"):
        raise PatchError("not an IPS patch")
    out = bytearray(source)
    pos = 5
    end = len(patch)
    while True:
        if pos + 3 > end:
            raise PatchError("IPS patch ends without an EOF marker")
        marker = patch[pos : pos + 3]
        pos += 3
        # "EOF" is also a valid offset (0x454F46), so only treat it as the end
        # marker when nothing but an optional truncation length follows.
        if marker == b"EOF" and end - pos in (0, 3):
            if end - pos == 3:
                del out[int.from_bytes(patch[pos:end], "big") :]
            return bytes(out)
        offset = int.from_bytes(marker, "big")
        if pos + 2 > end:
            raise PatchError("truncated IPS record")
        size = int.from_bytes(patch[pos : pos + 2], "big")
        pos += 2
        if size:
            data = patch[pos : pos + size]
            pos += size
            if len(data) != size:
                raise PatchError("truncated IPS record")
        else:
            if pos + 3 > end:
                raise PatchError("truncated IPS run-length record")
            run = int.from_bytes(patch[pos : pos + 2], "big")
            data = patch[pos + 2 : pos + 3] * run
            pos += 3
        if offset > len(out):
            out.extend(b"\x00" * (offset - len(out)))
        out[offset : offset + len(data)] = data


# --- BPS -------------------------------------------------------------------


def _read_varint(data: bytes, pos: int) -> tuple[int, int]:
    value = 0
    shift = 1
    while True:
        if pos >= len(data):
            raise PatchError("truncated BPS patch")
        byte = data[pos]
        pos += 1
        value += (byte & 0x7F) * shift
        if byte & 0x80:
            return value, pos
        shift <<= 7
        value += shift


def _write_varint(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value == 0:
            out.append(0x80 | byte)
            return bytes(out)
        out.append(byte)
        value -= 1


def apply_bps(source: bytes, patch: bytes) -> bytes:
    if not patch.startswith(b"BPS1") or len(patch) < 4 + 3 + 12:
        raise PatchError("not a BPS patch")
    body_end = len(patch) - 12
    source_crc = int.from_bytes(patch[body_end : body_end + 4], "little")
    target_crc = int.from_bytes(patch[body_end + 4 : body_end + 8], "little")
    patch_crc = int.from_bytes(patch[body_end + 8 :], "little")
    if zlib.crc32(patch[:-4]) != patch_crc:
        raise PatchError("BPS patch is corrupt (patch checksum mismatch)")

    pos = 4
    source_size, pos = _read_varint(patch, pos)
    target_size, pos = _read_varint(patch, pos)
    metadata_size, pos = _read_varint(patch, pos)
    pos += metadata_size

    if len(source) != source_size or zlib.crc32(source) != source_crc:
        raise PatchError(
            f"BPS patch was made for a different file (expected {source_size} bytes, crc32 {source_crc:08x}; "
            f"got {len(source)} bytes, crc32 {zlib.crc32(source):08x})"
        )

    out = bytearray()
    source_rel = 0
    target_rel = 0
    while pos < body_end:
        data, pos = _read_varint(patch, pos)
        command = data & 3
        length = (data >> 2) + 1
        if command == 0:  # SourceRead
            start = len(out)
            out += source[start : start + length]
        elif command == 1:  # TargetRead
            out += patch[pos : pos + length]
            pos += length
        elif command == 2:  # SourceCopy
            data, pos = _read_varint(patch, pos)
            source_rel += (-1 if data & 1 else 1) * (data >> 1)
            if source_rel < 0 or source_rel + length > len(source):
                raise PatchError("BPS SourceCopy reads outside the source file")
            out += source[source_rel : source_rel + length]
            source_rel += length
        else:  # TargetCopy, which may overlap the bytes it is writing
            data, pos = _read_varint(patch, pos)
            target_rel += (-1 if data & 1 else 1) * (data >> 1)
            if target_rel < 0 or target_rel >= len(out):
                raise PatchError("BPS TargetCopy reads outside the written output")
            for _ in range(length):
                out.append(out[target_rel])
                target_rel += 1

    if len(out) != target_size or zlib.crc32(out) != target_crc:
        raise PatchError("BPS patch produced an unexpected result (target checksum mismatch)")
    return bytes(out)


def create_bps(source: bytes, target: bytes, metadata: bytes = b"") -> bytes:
    """Create a BPS patch with linear encoding (no copy detection).

    The result is valid and deterministic but not minimal: data that moved is stored in full.
    """
    out = bytearray(b"BPS1")
    out += _write_varint(len(source))
    out += _write_varint(len(target))
    out += _write_varint(len(metadata))
    out += metadata

    def emit(command: int, length: int) -> None:
        out.extend(_write_varint(((length - 1) << 2) | command))

    def target_read(start: int, end: int) -> None:
        if end > start:
            emit(1, end - start)
            out.extend(target[start:end])

    common = min(len(source), len(target))
    # A SourceRead costs a byte or more, so tiny unchanged gaps are cheaper stored inline.
    changed = merge_ranges(diff_ranges(source, target), gap=3)
    pos = 0
    for start, end in changed:
        if start > pos:
            emit(0, start - pos)
        target_read(start, end)
        pos = end
    if common > pos:
        emit(0, common - pos)
    target_read(common, len(target))

    out += zlib.crc32(source).to_bytes(4, "little")
    out += zlib.crc32(target).to_bytes(4, "little")
    out += zlib.crc32(out).to_bytes(4, "little")
    return bytes(out)
