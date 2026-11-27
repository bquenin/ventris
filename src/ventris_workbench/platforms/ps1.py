"""PlayStation disc images: finding and reading the files inside one.

A PlayStation game disc is a CD whose first track holds the data, as an ISO 9660 file system. An image
of one is usually a BIN/CUE pair: the ``.bin`` holds every sector whole, 2352 bytes each, and the
``.cue`` is a short text that names the ``.bin`` and says what its tracks are. Audio tracks, when a game
has them, follow the data track, in the same ``.bin`` or in files of their own.

A whole sector starts with 12 sync bytes and a 4-byte header: the sector's place on the disc as minute,
second and frame (in BCD), and its mode. A PlayStation's data track is in mode 2, where an 8-byte
subheader comes next: file, channel, submode and coding, written twice. Bit 0x20 of the submode says
which form the sector is in. Form 1 carries 2048 bytes of data at offset 24 and error correction after
them; that is what the file system and ordinary files are made of. Form 2 carries 2324 bytes at
offset 24 with no correction, for streams that can bear an error: movies (``.STR``) and XA audio. An
image of only the 2048 data bytes of each sector (an ``.iso``) is read too, though it has lost what
Form 2 sectors carried beyond those bytes.

``Disc`` reads the file system the way ``platforms.ps2`` does, through the 2048 data bytes of each
sector. ``SYSTEM.CNF`` in the root names the program the console starts; its name is the game's serial.
"""

import re
from pathlib import Path

from ventris_workbench.platforms import ps2
from ventris_workbench.platforms.ps2 import DiscError

RAW_SECTOR = 2352  # a whole sector, as a BIN image holds it
DATA = 2048  # the data a Form 1 sector carries
FORM2_DATA = 2324  # the data a Form 2 sector carries
SYNC = b"\x00" + b"\xff" * 10 + b"\x00"  # how every whole data sector starts
_HEADER = 12  # where the header follows the sync bytes: minute, second, frame, mode
_SUBMODE = 18  # in mode 2, the submode byte of the subheader
_FORM2 = 0x20  # the submode's bit for Form 2
_VOLUME_SECTOR = 16  # where the primary volume descriptor is
_DATA_TRACKS = ("MODE1/2352", "MODE2/2352", "MODE1/2048")


def read_cue(cue: Path) -> Path:
    """The image file a cue sheet names for its data track.

    The data track must be the first track and start the file it is in. Audio tracks after it are
    allowed, in the same file or in others, since nothing here reads them. A cue with a second data
    track, or whose first track is audio, is refused.
    """
    cue = Path(cue)
    raw = cue.read_bytes()
    try:
        text = raw.decode("utf-8-sig")
    except UnicodeDecodeError:  # a cue written on a Japanese system may name its files in Shift JIS
        text = raw.decode("cp932", "replace")
    first_file: str | None = None
    current_file: str | None = None
    tracks: list[tuple[int, str, str]] = []  # (number, type, file it is in)
    first_index: str | None = None
    for line in text.splitlines():
        words = line.strip()
        if match := re.match(r'FILE\s+"([^"]+)"\s+\S+', words, re.IGNORECASE) or re.match(
            r"FILE\s+(\S+)\s+\S+", words, re.IGNORECASE
        ):
            current_file = match.group(1)
            first_file = first_file or current_file
        elif match := re.match(r"TRACK\s+(\d+)\s+(\S+)", words, re.IGNORECASE):
            if current_file is None:
                raise DiscError(f"{cue}: TRACK {match.group(1)} comes before any FILE")
            tracks.append((int(match.group(1)), match.group(2).upper(), current_file))
        elif (match := re.match(r"INDEX\s+01\s+(\S+)", words, re.IGNORECASE)) and len(tracks) == 1:
            first_index = match.group(1)
    if not tracks or first_file is None:
        raise DiscError(f"{cue} names no track")
    number, kind, file = tracks[0]
    if kind not in _DATA_TRACKS:
        raise DiscError(f"{cue}: the first track is {kind}, not a data track ({', '.join(_DATA_TRACKS)})")
    if file != first_file or (first_index is not None and first_index != "00:00:00"):
        raise DiscError(f"{cue}: the data track does not start its file, which is not supported yet")
    for number, kind, _ in tracks[1:]:
        if kind != "AUDIO":
            raise DiscError(f"{cue}: track {number} is a second data track ({kind}), which is not supported yet")
    image = cue.parent / first_file
    if not image.is_file():
        raise DiscError(f"{cue} names {first_file}, which is not in {cue.parent}")
    return image


def _layout(image: Path) -> tuple[int, int]:
    """How an image stores its sectors: (bytes per sector, where in a sector the 2048 data bytes are)."""
    with image.open("rb") as source:
        source.seek(_VOLUME_SECTOR * RAW_SECTOR)
        whole = source.read(RAW_SECTOR)
        if whole[:12] == SYNC:
            mode = whole[_HEADER + 3]
            if mode == 2:
                return RAW_SECTOR, 24
            if mode == 1:
                return RAW_SECTOR, 16
            raise DiscError(f"{image}: sector {_VOLUME_SECTOR} is in mode {mode}, not a data sector")
        source.seek(_VOLUME_SECTOR * DATA)
        if source.read(6) == b"\x01CD001":
            return DATA, 0
    raise DiscError(f"{image} is not a CD image: no data sector where its file system should start")


class Disc(ps2.Disc):
    """An open PlayStation disc image, from its ``.cue`` or its ``.bin`` (or an ``.iso``).

    ``files`` maps each file's path, such as ``MOVIE/OPEN.STR``, to its first sector and its size in
    bytes, as ISO 9660 records them. ``path`` is the image file itself, ``cue`` the cue sheet if one was
    opened.
    """

    def __init__(self, path: Path):
        path = Path(path)
        self.cue: Path | None = None
        if path.suffix.lower() == ".cue":
            self.cue = path
            path = read_cue(path)
        self.sector_size, self._data_at = _layout(path)
        super().__init__(path)
        # The directory records were found in the stream of 2048-byte data; say where they are in the image.
        self.records = {name: self._image_offset(where) for name, where in self.records.items()}

    def _image_offset(self, data_offset: int) -> int:
        sector, within = divmod(data_offset, DATA)
        return sector * self.sector_size + self._data_at + within

    @property
    def sector_count(self) -> int:
        return self.path.stat().st_size // self.sector_size

    def _sectors(self, sector: int, size: int) -> bytes:
        """``size`` bytes of the 2048-byte data of the sectors from ``sector`` on, joined."""
        count = -(-size // DATA)
        self._image.seek(sector * self.sector_size)
        block = self._image.read(count * self.sector_size)
        if self.sector_size == DATA:
            return block[:size]
        at = self._data_at
        data = b"".join(block[n + at : n + at + DATA] for n in range(0, len(block), self.sector_size))
        return data[:size]

    def raw_sector(self, sector: int) -> bytes:
        """A whole 2352-byte sector, with its sync bytes, header and subheader."""
        if self.sector_size != RAW_SECTOR:
            raise DiscError(f"{self.path} holds only 2048 bytes of each sector, not whole sectors")
        if not 0 <= sector < self.sector_count:
            raise DiscError(f"{self.path} has no sector {sector}: it has {self.sector_count}")
        self._image.seek(sector * RAW_SECTOR)
        return self._image.read(RAW_SECTOR)

    def sector(self, sector: int) -> bytes:
        """The data a sector carries: 2048 bytes in Form 1, 2324 in Form 2."""
        if self.sector_size == DATA:
            return self._sectors(sector, DATA)
        whole = self.raw_sector(sector)
        if self._data_at == 24 and whole[_SUBMODE] & _FORM2:
            return whole[24 : 24 + FORM2_DATA]
        return whole[self._data_at : self._data_at + DATA]

    def offset(self, name: str) -> int:
        """Where a file's first sector starts in the image, in bytes."""
        return self._entry(name)[0] * self.sector_size

    def read(self, name: str, start: int = 0, length: int | None = None) -> bytes:
        """A file's contents, or ``length`` bytes of it from ``start``, through the 2048 data bytes of each
        sector. That is right for ordinary files; a movie or XA audio file is in Form 2 sectors, which
        ``read_raw`` gives whole."""
        sector, size = self._entry(name)
        if length is None:
            length = size - start
        if start < 0 or length < 0 or start + length > size:
            raise DiscError(f"{name} has no bytes {start:#x} to {start + length:#x}: it is {size:#x} long")
        first, skip = divmod(start, DATA)
        return self._sectors(sector + first, skip + length)[skip:]

    def read_raw(self, name: str) -> bytes:
        """The whole sectors a file covers, 2352 bytes each: for files whose sectors are in Form 2."""
        sector, size = self._entry(name)
        return b"".join(self.raw_sector(sector + n) for n in range(-(-size // DATA)))

    def config(self) -> dict[str, str]:
        """``SYSTEM.CNF`` as names and values, such as ``BOOT`` and ``STACK``."""
        text = self.read("SYSTEM.CNF").decode("ascii", "replace")
        entries = {}
        for line in text.splitlines():
            name, equals, value = line.partition("=")
            if equals:
                entries[name.strip().upper()] = value.strip()
        return entries

    def boot_name(self) -> str:
        """The path of the program the console starts, from ``SYSTEM.CNF``: ``SLPS_009.44`` for
        ``BOOT = cdrom:\\SLPS_009.44;1``. A disc with no ``SYSTEM.CNF`` starts ``PSX.EXE``."""
        if "SYSTEM.CNF" not in self.files:
            return "PSX.EXE"
        match = re.match(r"cdrom:\\?([^;\s]+)", self.config().get("BOOT", ""), re.IGNORECASE)
        if not match:
            raise DiscError(f"{self.path}: SYSTEM.CNF does not name a program to start")
        return match.group(1).replace("\\", "/")

    def serial(self) -> str:
        """The game's serial number as it is printed on the case: ``SLPS-00944`` for ``SLPS_009.44``."""
        name = self.boot_name().rsplit("/", 1)[-1]
        if not re.fullmatch(r"[A-Z]{4}_\d{3}\.\d{2}", name, re.IGNORECASE):
            raise DiscError(f"{self.path}: the program it starts, {name}, is not named after a serial")
        return name.replace("_", "-").replace(".", "").upper()


__all__ = ["DATA", "FORM2_DATA", "RAW_SECTOR", "Disc", "DiscError", "read_cue"]
