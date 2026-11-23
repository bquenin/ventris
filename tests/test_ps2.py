import hashlib
import struct
from pathlib import Path

import pytest

from ventris_workbench.platforms import ps2
from ventris_workbench.platforms.ps2 import SECTOR, Disc, DiscError


def _record(name: bytes, sector: int, size: int, directory: bool = False) -> bytes:
    """One entry of an ISO 9660 directory."""
    body = struct.pack("<I", sector) + struct.pack(">I", sector) + struct.pack("<I", size) + struct.pack(">I", size)
    body += bytes(7) + bytes([2 if directory else 0, 0, 0]) + b"\x01\x00\x00\x01" + bytes([len(name)]) + name
    body += bytes(len(body) % 2)  # records have an even length
    return bytes([len(body) + 2, 0]) + body


def _sector(data: bytes) -> bytes:
    return data.ljust(-(-len(data) // SECTOR) * SECTOR, b"\x00")


@pytest.fixture
def image(tmp_path: Path) -> Path:
    """A disc with two files in its root and one in a folder."""
    config = b"BOOT2 = cdrom0:\\SLPS_123.45;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n"
    data = bytes(range(256)) * 12  # a file of a sector and a half
    note = b"in a folder"
    root = _record(b"\x00", 18, SECTOR, True) + _record(b"\x01", 18, SECTOR, True)
    root += _record(b"SYSTEM.CNF;1", 20, len(config)) + _record(b"DATA.BIN;1", 21, len(data))
    root += _record(b"SUB", 19, SECTOR, True)
    sub = (
        _record(b"\x00", 19, SECTOR, True) + _record(b"\x01", 18, SECTOR, True) + _record(b"NOTE.TXT;1", 23, len(note))
    )
    volume = bytearray(SECTOR)
    volume[0:7] = b"\x01CD001\x01"
    volume[40:72] = b"TESTDISC".ljust(32)
    volume[318:446] = b"A PUBLISHER".ljust(128)
    volume[156:190] = _record(b"\x00", 18, SECTOR, True)
    volume[80:88] = struct.pack("<I", 26) + struct.pack(">I", 26)
    path = tmp_path / "disc.iso"
    path.write_bytes(bytes(16 * SECTOR) + bytes(volume) + bytes(SECTOR) + _sector(root) + _sector(sub)
                     + _sector(config) + _sector(data) + _sector(note)
                     + _sector(b"after the last file") + _sector(_anchor(25)))  # fmt: skip
    return path


def _anchor(sector: int) -> bytes:
    """The start of the UDF marker a disc has in its last sector: it names that sector, with a checksum."""
    tag = bytearray(struct.pack("<HHBBHHHI", 2, 2, 0, 0, 0, 0x1234, 0x01F0, sector))
    tag[4] = sum(tag[:4] + tag[5:]) & 0xFF
    return bytes(tag) + b"the rest of the marker"


def test_write_image_changes_bytes_inside_files(image: Path, tmp_path: Path):
    out = tmp_path / "out.iso"
    with Disc(image) as disc:
        ps2.write_image(disc, out)
        assert out.read_bytes() == image.read_bytes()
        ps2.write_image(disc, out, patches=[("DATA.BIN", 0x7FE, b"WXYZ"), ("SYSTEM.CNF", 0, b"boot")])
        with pytest.raises(DiscError, match="has no bytes"):
            ps2.write_image(disc, tmp_path / "bad.iso", patches=[("SUB/NOTE.TXT", 8, b"too long")])
    with Disc(out) as built:
        assert built.read("DATA.BIN", 0x7FC, 8) == bytes([0xFC, 0xFD]) + b"WXYZ" + bytes([0x02, 0x03])
        assert built.read("SYSTEM.CNF", 0, 8) == b"boot2 = "[:4] + b"2 = "
        assert built.read("SUB/NOTE.TXT") == b"in a folder"
    assert out.stat().st_size == image.stat().st_size


def test_write_image_gives_the_last_file_any_size(image: Path, tmp_path: Path):
    out = tmp_path / "out.iso"
    with Disc(image) as disc:
        # The same size: nothing else changes.
        ps2.write_image(disc, out, tail=("SUB/NOTE.TXT", [b"IN A ", b"FOLDER"]))
        assert out.read_bytes() == image.read_bytes().replace(b"in a folder", b"IN A FOLDER")
        # Three sectors where there was one: what followed still follows, two sectors later.
        grown = bytes(range(256)) * 20
        ps2.write_image(disc, out, tail=("SUB/NOTE.TXT", [grown[:3000], grown[3000:]]))
        with pytest.raises(DiscError, match="not the file that ends"):
            ps2.write_image(disc, tmp_path / "bad.iso", tail=("DATA.BIN", [b""]))
    data = out.read_bytes()
    assert len(data) == 28 * SECTOR
    assert data[26 * SECTOR :].startswith(b"after the last file")
    assert data[27 * SECTOR :].startswith(_anchor(27))  # the marker names its new sector
    assert data[16 * SECTOR + 80 :][:8] == struct.pack("<I", 28) + struct.pack(">I", 28)
    with Disc(out) as built:
        assert built.files["SUB/NOTE.TXT"] == (23, len(grown))
        assert built.read("SUB/NOTE.TXT") == grown
        assert built.read("DATA.BIN") == bytes(range(256)) * 12


def test_lists_files_in_folders(image: Path):
    with Disc(image) as disc:
        assert disc.files == {
            "SYSTEM.CNF": (20, 57),
            "DATA.BIN": (21, 3072),
            "SUB/NOTE.TXT": (23, 11),
        }
        assert disc.label == "TESTDISC"
        assert disc.publisher == "A PUBLISHER"


def test_reads_a_file_or_a_part_of_one(image: Path):
    with Disc(image) as disc:
        assert disc.read("SUB/NOTE.TXT") == b"in a folder"
        assert disc.read("DATA.BIN") == bytes(range(256)) * 12
        assert disc.read("DATA.BIN", 0x7FE, 4) == bytes([0xFE, 0xFF, 0x00, 0x01])  # across two sectors
        assert disc.read("DATA.BIN", 3072) == b""
        assert disc.size("DATA.BIN") == 3072
        assert disc.offset("DATA.BIN") == 21 * SECTOR


def test_names_the_program_the_console_starts(image: Path):
    with Disc(image) as disc:
        assert disc.boot_name() == "SLPS_123.45"
        assert disc.serial() == "SLPS-12345"


def test_hashes_the_whole_image(image: Path):
    with Disc(image) as disc:
        assert disc.sha1() == hashlib.sha1(image.read_bytes()).hexdigest()
        assert disc.read("SUB/NOTE.TXT") == b"in a folder"  # reading still works afterwards


def test_errors(image: Path, tmp_path: Path):
    with Disc(image) as disc:
        with pytest.raises(DiscError, match=r"no file named MISSING\.BIN"):
            disc.read("MISSING.BIN")
        with pytest.raises(DiscError, match="has no bytes"):
            disc.read("SUB/NOTE.TXT", 8, 8)
    other = tmp_path / "game.sfc"
    other.write_bytes(bytes(0x10000))
    assert ps2.is_disc(image)
    assert not ps2.is_disc(other)
    assert not ps2.is_disc(tmp_path / "absent.iso")
    with pytest.raises(DiscError, match="not an ISO 9660"):
        Disc(other)
