"""SNES / Super Famicom cartridge layout: copier headers, internal header, address mapping."""

from dataclasses import dataclass

COPIER_HEADER_SIZE = 512

# File offset of the internal header for each cartridge mapping.
_HEADER_OFFSETS = {
    "LoROM": 0x7FC0,
    "HiROM": 0xFFC0,
    "ExHiROM": 0x40FFC0,
}

# Low nibble of the map-mode byte expected for each mapping.
_MAP_MODE_NIBBLES = {
    "LoROM": {0x0, 0x2, 0x3},
    "HiROM": {0x1},
    "ExHiROM": {0x5},
}

_REGIONS = {
    0x00: "Japan",
    0x01: "North America",
    0x02: "Europe",
    0x03: "Sweden/Scandinavia",
    0x04: "Finland",
    0x05: "Denmark",
    0x06: "France",
    0x07: "Netherlands",
    0x08: "Spain",
    0x09: "Germany",
    0x0A: "Italy",
    0x0B: "China",
    0x0D: "Korea",
    0x0F: "Canada",
    0x10: "Brazil",
    0x11: "Australia",
}


class AddressError(ValueError):
    """The address has no equivalent in the other address space."""


@dataclass(frozen=True)
class SnesHeader:
    mapping: str
    offset: int
    title: str
    map_mode: int
    cartridge_type: int
    rom_size_kb: int
    ram_size_kb: int
    region_code: int
    developer_id: int
    version: int
    checksum: int
    checksum_complement: int
    reset_vector: int
    score: int

    @property
    def region(self) -> str:
        return _REGIONS.get(self.region_code, f"unknown (0x{self.region_code:02x})")

    @property
    def fast_rom(self) -> bool:
        return bool(self.map_mode & 0x10)


def has_copier_header(size: int) -> bool:
    """Copier headers add 512 bytes to an image whose size is otherwise a multiple of 1 KiB."""
    return size % 1024 == COPIER_HEADER_SIZE


def strip_copier_header(data: bytes) -> bytes:
    return data[COPIER_HEADER_SIZE:] if has_copier_header(len(data)) else data


def _parse_header(rom: bytes, mapping: str) -> SnesHeader | None:
    offset = _HEADER_OFFSETS[mapping]
    if offset + 0x40 > len(rom):
        return None
    raw = rom[offset : offset + 0x40]
    map_mode = raw[0x15]
    checksum_complement = int.from_bytes(raw[0x1C:0x1E], "little")
    checksum = int.from_bytes(raw[0x1E:0x20], "little")
    reset_vector = int.from_bytes(raw[0x3C:0x3E], "little")
    title_bytes = raw[:0x15]

    score = 0
    if checksum + checksum_complement == 0xFFFF and checksum != 0:
        score += 4
    if map_mode & 0x0F in _MAP_MODE_NIBBLES[mapping]:
        score += 2
    if map_mode & 0xE0 == 0x20:
        score += 1
    # The reset vector must point into ROM, which is only mapped at $8000 and above in bank 0.
    score += 2 if reset_vector >= 0x8000 else -4
    if all(byte == 0x20 or 0x21 <= byte <= 0x7E or 0xA1 <= byte <= 0xDF for byte in title_bytes):
        score += 1
    if 0x07 <= raw[0x17] <= 0x0D:
        score += 1

    return SnesHeader(
        mapping=mapping,
        offset=offset,
        title=title_bytes.decode("shift_jis", errors="replace").rstrip(),
        map_mode=map_mode,
        cartridge_type=raw[0x16],
        rom_size_kb=1 << raw[0x17] if raw[0x17] < 0x10 else 0,
        ram_size_kb=1 << raw[0x18] if 0 < raw[0x18] < 0x10 else 0,
        region_code=raw[0x19],
        developer_id=raw[0x1A],
        version=raw[0x1B],
        checksum=checksum,
        checksum_complement=checksum_complement,
        reset_vector=reset_vector,
        score=score,
    )


def detect_header(rom: bytes, min_score: int = 5) -> SnesHeader | None:
    """Find the internal header of a ROM image that has no copier header.

    Returns None when no candidate location looks like a SNES header.
    """
    candidates = [header for mapping in _HEADER_OFFSETS if (header := _parse_header(rom, mapping))]
    if not candidates:
        return None
    best = max(candidates, key=lambda header: header.score)
    return best if best.score >= min_score else None


def compute_checksum(rom: bytes) -> int:
    """Cartridge checksum: the byte sum with the last part mirrored up to a power of two."""

    def mirror_sum(start: int, length: int, mask: int) -> int:
        while mask and not length & mask:
            mask >>= 1
        if not mask:
            return 0
        part1 = sum(rom[start : start + mask])
        part2 = 0
        rest = length - mask
        if rest:
            part2 = mirror_sum(start + mask, rest, mask >> 1)
            while rest < mask:
                rest += rest
                part2 += part2
        return part1 + part2

    return mirror_sum(0, len(rom), 0x800000) & 0xFFFF


def update_checksum(rom: bytes) -> bytes:
    """Return ``rom`` with its internal checksum and complement recomputed."""
    header = detect_header(rom, min_score=0)
    if header is None:
        raise ValueError("no SNES header found")
    out = bytearray(rom)
    places = [header.offset + 0x1C]
    # An ExHiROM image made from a HiROM one keeps a second copy of the header where it used to be,
    # checksum included. A game made as ExHiROM may have its title there and no checksum; that is left
    # as it is.
    low = _LOW_HEADER + 0x1C
    main = header.offset + 0x1C
    if header.mapping == "ExHiROM" and out[_LOW_HEADER : low + 4] == out[header.offset : main + 4]:
        places.append(low)
    # The checksum covers its own four bytes, which always add up to 0x1FE once they are consistent.
    for fields in places:
        out[fields : fields + 4] = b"\xff\xff\x00\x00"
    checksum = compute_checksum(bytes(out))
    for fields in places:
        out[fields : fields + 2] = (checksum ^ 0xFFFF).to_bytes(2, "little")
        out[fields + 2 : fields + 4] = checksum.to_bytes(2, "little")
    return bytes(out)


def expand(rom: bytes, size: int) -> bytes:
    """Pad ``rom`` with $FF to ``size`` bytes and record the new size in its header."""
    if size < len(rom):
        raise ValueError("a ROM cannot be expanded to a smaller size")
    header = detect_header(rom, min_score=0)
    if header is None:
        raise ValueError("no SNES header found")
    out = bytearray(rom) + b"\xff" * (size - len(rom))
    out[header.offset + 0x17] = max((size - 1).bit_length() - 10, 0)  # log2 of the size in KiB, rounded up
    return bytes(out)


HIROM_SIZE = 0x400000
EXHIROM_SIZE = 0x800000
_LOW_HEADER = _HEADER_OFFSETS["HiROM"]


def to_exhirom(rom: bytes) -> bytes:
    """Turn a 4 MiB HiROM image into an 8 MiB ExHiROM one, which adds banks $40-$7D of free space.

    The first 4 MiB stay where the game expects them, in banks $80-$FF. The catch is banks $00-$3F:
    in ExHiROM their upper halves show the new 4 MiB, and the console starts in bank $00. So the top
    half of the first bank, which holds the header, the vectors and the start-up code, is copied to
    where bank $00 now looks (file 0x408000). Code that runs in banks $01-$3F would need the same;
    games made for fast ROM run in $80 and up, and the copy of bank $00 is enough for them.
    """
    header = detect_header(rom, min_score=0)
    if header is None or header.mapping != "HiROM" or len(rom) != HIROM_SIZE:
        raise ValueError("only a 4 MiB HiROM image can be turned into ExHiROM")
    out = bytearray(rom) + b"\xff" * (EXHIROM_SIZE - len(rom))
    out[HIROM_SIZE + 0x8000 : HIROM_SIZE + 0x10000] = rom[0x8000:0x10000]
    for offset in (_LOW_HEADER, _HEADER_OFFSETS["ExHiROM"]):
        out[offset + 0x15] = (out[offset + 0x15] & 0xF0) | 0x05
        out[offset + 0x17] = 0x0D  # 8 MiB
    return bytes(out)


def write_code(rom: bytes, segments: list[tuple[int, bytes]], mapping: str) -> bytes:
    """Write assembled code into ``rom``; each segment is a CPU address and the bytes that go there."""
    out = bytearray(rom)
    for address, code in segments:
        offset = snes_to_pc(address, mapping)
        if offset + len(code) > len(out):
            raise AddressError(f"${address:06x} is past the end of the ROM")
        out[offset : offset + len(code)] = code
    return bytes(out)


class BankAllocator:
    """Hands out free space in a file range, never letting a piece cross a 64 KiB bank boundary.

    Games index data with 16-bit registers inside one bank, so data that straddles two banks is read wrongly.
    """

    def __init__(self, start: int, end: int):
        self.position = start
        self.end = end

    def allocate(self, size: int) -> int:
        """Return the file offset of ``size`` free bytes."""
        if size > 0x10000:
            raise AddressError(f"{size} bytes cannot fit inside one bank")
        position = self.position
        if (position & 0xFFFF) + size > 0x10000:
            position = (position | 0xFFFF) + 1
        if position + size > self.end:
            raise AddressError(f"no room left for {size} bytes")
        self.position = position + size
        return position


def pc_to_snes(offset: int, mapping: str) -> int:
    """Convert a file offset (no copier header) to a CPU address."""
    if offset < 0:
        raise AddressError(f"negative offset {offset}")
    if mapping == "LoROM":
        if offset >= 0x400000:
            raise AddressError(f"offset 0x{offset:06x} is outside LoROM space")
        return ((offset >> 15) | 0x80) << 16 | (offset & 0x7FFF) | 0x8000
    if mapping == "HiROM":
        if offset >= 0x400000:
            raise AddressError(f"offset 0x{offset:06x} is outside HiROM space")
        return 0xC00000 | offset
    if mapping == "ExHiROM":
        if offset < 0x400000:
            return 0xC00000 | offset
        if offset < 0x7E0000:
            return offset
        # Banks $7E-$7F are work RAM, so the last 128 KiB is only visible in the upper half of $3E-$3F.
        if offset < 0x800000 and offset & 0x8000:
            return offset - 0x400000
        raise AddressError(f"offset 0x{offset:06x} is not mapped in ExHiROM")
    raise ValueError(f"unknown mapping {mapping!r}")


def snes_to_pc(address: int, mapping: str) -> int:
    """Convert a CPU address to a file offset (no copier header)."""
    bank = (address >> 16) & 0xFF
    low = address & 0xFFFF
    if bank in (0x7E, 0x7F):
        raise AddressError(f"${address:06x} is work RAM")
    if mapping == "LoROM":
        if low < 0x8000:
            raise AddressError(f"${address:06x} is not ROM in LoROM")
        return ((bank & 0x7F) << 15) | (low & 0x7FFF)
    if mapping in ("HiROM", "ExHiROM"):
        if bank & 0x7F < 0x40 and low < 0x8000:
            raise AddressError(f"${address:06x} is not ROM in {mapping}")
        offset = address & 0x3FFFFF
        if mapping == "ExHiROM" and bank < 0x80:
            offset += 0x400000
        return offset
    raise ValueError(f"unknown mapping {mapping!r}")
