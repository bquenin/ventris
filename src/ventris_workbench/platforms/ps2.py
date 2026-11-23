"""PlayStation 2 disc images: finding and reading the files inside one.

A PS2 game disc is an ISO 9660 file system. An image of one is several gigabytes, so it is never
read whole: ``Disc`` keeps the image open and reads the parts asked for. ``SYSTEM.CNF`` in the root
names the program the console starts.

``write_image`` writes a changed copy: bytes replaced inside files that keep their size, and new
contents of any size for the file that ends the image, which is where a game's large archive tends
to be. A PS2 disc also describes its files a second time, as UDF. That copy is not rewritten, since
the console finds files through ISO 9660; only the UDF marker that must sit in the last sector moves.
"""

import hashlib
import re
import struct
from collections.abc import Iterable
from pathlib import Path

SECTOR = 2048
_VOLUME_SECTOR = 16  # where the primary volume descriptor is
_ROOT_RECORD = 156  # where the root directory's record is inside it
_DIRECTORY = 2  # the flag that marks a directory record


class DiscError(Exception):
    """The file is not a disc image this module can read, or it has no such file."""


def is_disc(path: Path) -> bool:
    """Whether the file at ``path`` starts like an ISO 9660 image."""
    try:
        with path.open("rb") as image:
            image.seek(_VOLUME_SECTOR * SECTOR)
            return image.read(6) == b"\x01CD001"
    except OSError:
        return False


class Disc:
    """An open disc image. ``files`` maps each file's path, such as ``VIRX/PADMAN.IRX``, to where it is."""

    def __init__(self, path: Path):
        self.path = Path(path)
        self._image = self.path.open("rb")
        volume = self._sectors(_VOLUME_SECTOR, SECTOR)
        if volume[:6] != b"\x01CD001":
            raise DiscError(f"{path} is not an ISO 9660 disc image")
        self.label = volume[40:72].decode("ascii", "replace").strip()
        self.publisher = volume[318:446].decode("ascii", "replace").strip()
        first, size = struct.unpack_from("<I4xI", volume, _ROOT_RECORD + 2)
        # File path -> (first sector, size in bytes), and where in the image each file's directory record is.
        self.files: dict[str, tuple[int, int]] = {}
        self.records: dict[str, int] = {}
        for name, entry, record in self._list(first, size, ""):
            self.files[name], self.records[name] = entry, record

    def close(self) -> None:
        self._image.close()

    def __enter__(self) -> "Disc":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def _sectors(self, sector: int, size: int) -> bytes:
        self._image.seek(sector * SECTOR)
        return self._image.read(size)

    def _list(self, sector: int, size: int, prefix: str):
        data = self._sectors(sector, size)
        position = 0
        while position < len(data):
            length = data[position]
            if length == 0:  # a record never crosses a sector; the rest of this one is padding
                position = (position // SECTOR + 1) * SECTOR
                continue
            record = data[position : position + length]
            where = sector * SECTOR + position
            position += length
            first, extent = struct.unpack_from("<I4xI", record, 2)
            name = record[33 : 33 + record[32]]
            if name in (b"\x00", b"\x01"):  # the directory itself and its parent
                continue
            path = prefix + name.decode("ascii", "replace").split(";")[0]  # ";1" is a version number
            if record[25] & _DIRECTORY:
                yield from self._list(first, extent, path + "/")
            else:
                yield path, (first, extent), where

    def size(self, name: str) -> int:
        return self._entry(name)[1]

    def offset(self, name: str) -> int:
        """Where a file starts in the image, in bytes."""
        return self._entry(name)[0] * SECTOR

    def _entry(self, name: str) -> tuple[int, int]:
        if name not in self.files:
            raise DiscError(f"{self.path} has no file named {name}")
        return self.files[name]

    def read(self, name: str, start: int = 0, length: int | None = None) -> bytes:
        """A file's contents, or ``length`` bytes of it from ``start``."""
        sector, size = self._entry(name)
        if length is None:
            length = size - start
        if start < 0 or length < 0 or start + length > size:
            raise DiscError(f"{name} has no bytes {start:#x} to {start + length:#x}: it is {size:#x} long")
        self._image.seek(sector * SECTOR + start)
        return self._image.read(length)

    def boot_name(self) -> str:
        """The name of the program the console starts, from ``SYSTEM.CNF``."""
        config = self.read("SYSTEM.CNF").decode("ascii", "replace")
        match = re.search(r"BOOT2\s*=\s*cdrom0:\\([^;\s]+)", config)
        if not match:
            raise DiscError(f"{self.path}: SYSTEM.CNF does not name a program to start")
        return match.group(1)

    def serial(self) -> str:
        """The game's serial number as it is printed on the case: ``SLPS-25172`` for ``SLPS_251.72``."""
        return self.boot_name().replace("_", "-").replace(".", "")

    def sha1(self) -> str:
        """SHA-1 of the whole image, read in pieces."""
        digest = hashlib.sha1()
        self._image.seek(0)
        while chunk := self._image.read(1 << 24):
            digest.update(chunk)
        return digest.hexdigest()


_CHUNK = 1 << 24
_VOLUME_SIZE = 80  # in the volume descriptor: the number of sectors, little-endian then big-endian
_RECORD_SIZE = 10  # in a directory record: the file's size, little-endian then big-endian
_UDF_ANCHOR = 2  # the kind of UDF descriptor that sits in the image's last sector


def _both_ways(number: int) -> bytes:
    """A 32-bit number as ISO 9660 stores it: little-endian, then big-endian."""
    return struct.pack("<I", number) + struct.pack(">I", number)


def _sectors(size: int) -> int:
    return -(-size // SECTOR)


def write_image(
    disc: Disc,
    output: Path,
    patches: Iterable[tuple[str, int, bytes]] = (),
    tail: tuple[str, Iterable[bytes]] | None = None,
) -> None:
    """Write a copy of the image to ``output`` with some of its files changed.

    ``patches`` are (file name, offset in the file, bytes) to write over what is there. ``tail`` gives
    new contents, in pieces and of any size, for the file that ends the image; whatever followed that
    file follows it again, and the sizes ISO 9660 records are brought up to date.
    """
    image_size = disc.path.stat().st_size
    copy_end, moved, new_size = image_size, 0, 0
    if tail is not None:
        name = tail[0]
        first, old_size = disc.files.get(name, (0, 0))
        if name not in disc.files or first != max(sector for sector, _ in disc.files.values()):
            raise DiscError(f"{name} is not the file that ends {disc.path}")
        copy_end = first * SECTOR
    with disc.path.open("rb") as source, output.open("w+b") as out:
        for start in range(0, copy_end, _CHUNK):
            out.write(source.read(min(_CHUNK, copy_end - start)))
        if tail is not None:
            for piece in tail[1]:
                out.write(piece)
                new_size += len(piece)
            out.write(bytes(-new_size % SECTOR))
            source.seek((first + _sectors(old_size)) * SECTOR)
            while chunk := source.read(_CHUNK):  # what followed the file
                out.write(chunk)
            moved = _sectors(new_size) - _sectors(old_size)
            if new_size != old_size:
                out.seek(disc.records[name] + _RECORD_SIZE)
                out.write(_both_ways(new_size))
            if moved:
                sectors = image_size // SECTOR + moved
                out.seek(_VOLUME_SECTOR * SECTOR + _VOLUME_SIZE)
                out.write(_both_ways(sectors))
                # The UDF marker in the last sector says which sector it is in, with a checksum over that.
                out.seek((sectors - 1) * SECTOR)
                tag = bytearray(out.read(16))
                if struct.unpack_from("<H", tag, 0)[0] == _UDF_ANCHOR:
                    struct.pack_into("<I", tag, 12, sectors - 1)
                    tag[4] = sum(tag[:4] + tag[5:]) & 0xFF
                    out.seek((sectors - 1) * SECTOR)
                    out.write(tag)
        for name, offset, data in patches:
            if offset < 0 or offset + len(data) > disc.size(name):
                raise DiscError(f"{name} has no bytes {offset:#x} to {offset + len(data):#x}")
            out.seek(disc.offset(name) + offset)
            out.write(data)
