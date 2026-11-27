import os
import struct
import tomllib
from pathlib import Path

import pytest

from ventris_workbench.platforms import ps1
from ventris_workbench.platforms.ps1 import DATA, FORM2_DATA, RAW_SECTOR, SYNC, Disc, DiscError

PROJECT = Path(__file__).parent.parent
CONFIG = b"BOOT = cdrom:\\SLPS_123.45;1\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801ffff0\r\n"
PROGRAM = b"PS-X EXE" + bytes(range(256)) * 12  # a file of a sector and a half
FORM1 = 0x08  # a submode for data
FORM2 = 0x64  # a submode for XA audio: Form 2


def _record(name: bytes, sector: int, size: int, directory: bool = False) -> bytes:
    """One entry of an ISO 9660 directory."""
    body = struct.pack("<I", sector) + struct.pack(">I", sector) + struct.pack("<I", size) + struct.pack(">I", size)
    body += bytes(7) + bytes([2 if directory else 0, 0, 0]) + b"\x01\x00\x00\x01" + bytes([len(name)]) + name
    body += bytes(len(body) % 2)  # records have an even length
    return bytes([len(body) + 2, 0]) + body


def _bcd(number: int) -> int:
    return (number // 10) << 4 | number % 10


def _whole(sector: int, data: bytes, submode: int = FORM1) -> bytes:
    """A whole mode 2 sector, as a BIN image holds it: the error correction is left as zeros."""
    minute, rest = divmod(sector + 150, 75 * 60)  # a disc's first sector is two seconds in
    second, frame = divmod(rest, 75)
    header = SYNC + bytes([_bcd(minute), _bcd(second), _bcd(frame), 2]) + bytes([0, 0, submode, 0]) * 2
    size = FORM2_DATA if submode & 0x20 else DATA
    return (header + data.ljust(size, b"\x00")).ljust(RAW_SECTOR, b"\x00")


def _disc() -> tuple[list[bytes], list[int]]:
    """The data of each sector of a small disc, and the submode of each: a program and SYSTEM.CNF in its
    root, and in a folder a movie of one data sector and one sector of XA audio."""
    root = _record(b"\x00", 18, DATA, True) + _record(b"\x01", 18, DATA, True)
    root += _record(b"MOVIE", 19, DATA, True)
    root += _record(b"SLPS_123.45;1", 21, len(PROGRAM)) + _record(b"SYSTEM.CNF;1", 20, len(CONFIG))
    movie = _record(b"\x00", 19, DATA, True) + _record(b"\x01", 18, DATA, True) + _record(b"A.STR;1", 23, 2 * DATA)
    volume = bytearray(DATA)
    volume[0:7] = b"\x01CD001\x01"
    volume[40:72] = b"PS1DISC".ljust(32)
    volume[156:190] = _record(b"\x00", 18, DATA, True)
    sectors = {
        16: bytes(volume),
        17: b"\xffCD001\x01",
        18: root,
        19: movie,
        20: CONFIG,
        21: PROGRAM[:DATA],
        22: PROGRAM[DATA:],
        23: b"video" * 10,
        24: b"\xaa" * FORM2_DATA,
    }
    submodes = [FORM2 if n == 24 else FORM1 for n in range(26)]
    return [sectors.get(n, b"") for n in range(26)], submodes


@pytest.fixture
def bin_image(tmp_path: Path) -> Path:
    data, submodes = _disc()
    path = tmp_path / "game.bin"
    path.write_bytes(b"".join(_whole(n, d, s) for n, (d, s) in enumerate(zip(data, submodes, strict=True))))
    return path


@pytest.fixture
def cue(bin_image: Path) -> Path:
    path = bin_image.with_suffix(".cue")
    path.write_text(f'FILE "{bin_image.name}" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n')
    return path


def test_lists_the_files_with_their_first_sector_and_size(cue: Path):
    with Disc(cue) as disc:
        assert disc.path.name == "game.bin"
        assert disc.cue == cue
        assert disc.label == "PS1DISC"
        assert disc.sector_size == RAW_SECTOR
        assert disc.sector_count == 26
        assert disc.files == {
            "SLPS_123.45": (21, len(PROGRAM)),
            "SYSTEM.CNF": (20, len(CONFIG)),
            "MOVIE/A.STR": (23, 2 * DATA),
        }


def test_reads_a_file_across_sectors_and_a_piece_of_one(cue: Path):
    with Disc(cue) as disc:
        assert disc.read("SLPS_123.45") == PROGRAM
        assert disc.read("SLPS_123.45", DATA - 4, 12) == PROGRAM[DATA - 4 : DATA + 8]
        assert disc.read("SYSTEM.CNF") == CONFIG
        with pytest.raises(DiscError, match="no bytes"):
            disc.read("SYSTEM.CNF", 0, len(CONFIG) + 1)
        with pytest.raises(DiscError, match="no file named"):
            disc.read("NOPE.BIN")


def test_names_the_program_it_starts_and_the_serial(cue: Path):
    with Disc(cue) as disc:
        assert disc.boot_name() == "SLPS_123.45"
        assert disc.serial() == "SLPS-12345"
        assert disc.config()["STACK"] == "801ffff0"


def test_a_sector_carries_2048_bytes_in_form_1_and_2324_in_form_2(cue: Path):
    with Disc(cue) as disc:
        assert disc.sector(23)[:5] == b"video"
        assert len(disc.sector(23)) == DATA
        assert disc.sector(24) == b"\xaa" * FORM2_DATA
        raw = disc.read_raw("MOVIE/A.STR")
        assert len(raw) == 2 * RAW_SECTOR
        assert raw[:12] == SYNC
        assert raw[RAW_SECTOR + 18] == FORM2
        assert disc.offset("MOVIE/A.STR") == 23 * RAW_SECTOR
        with pytest.raises(DiscError, match="no sector"):
            disc.raw_sector(26)


def test_opens_the_bin_without_its_cue(bin_image: Path):
    with Disc(bin_image) as disc:
        assert disc.cue is None
        assert disc.read("SYSTEM.CNF") == CONFIG


def test_reads_an_image_of_only_the_data_of_each_sector(tmp_path: Path):
    data, _ = _disc()
    path = tmp_path / "game.iso"
    path.write_bytes(b"".join(d.ljust(DATA, b"\x00")[:DATA] for d in data))
    with Disc(path) as disc:
        assert disc.sector_size == DATA
        assert disc.read("SLPS_123.45") == PROGRAM
        assert disc.serial() == "SLPS-12345"
        with pytest.raises(DiscError, match="whole sectors"):
            disc.raw_sector(16)


def test_the_directory_records_are_found_where_they_are_in_the_image(cue: Path):
    with Disc(cue) as disc:
        where = disc.records["SYSTEM.CNF"]
        image = disc.path.read_bytes()
        assert image[where + 33 : where + 33 + 12] == b"SYSTEM.CNF;1"


def test_a_disc_with_no_system_cnf_starts_psx_exe(tmp_path: Path):
    data, _ = _disc()
    root = _record(b"\x00", 18, DATA, True) + _record(b"\x01", 18, DATA, True)
    root += _record(b"PSX.EXE;1", 21, len(PROGRAM))
    data[18] = root
    path = tmp_path / "game.iso"
    path.write_bytes(b"".join(d.ljust(DATA, b"\x00")[:DATA] for d in data))
    with Disc(path) as disc:
        assert disc.boot_name() == "PSX.EXE"
        with pytest.raises(DiscError, match="not named after a serial"):
            disc.serial()


def test_a_cue_may_have_audio_tracks_after_the_data_track(bin_image: Path):
    cue = bin_image.with_suffix(".cue")
    (bin_image.parent / "track2.bin").write_bytes(bytes(RAW_SECTOR))
    cue.write_text(
        f'FILE "{bin_image.name}" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n'
        "  TRACK 02 AUDIO\n    INDEX 00 00:20:00\n    INDEX 01 00:22:00\n"
        'FILE "track2.bin" BINARY\n  TRACK 03 AUDIO\n    INDEX 01 00:00:00\n'
    )
    assert ps1.read_cue(cue) == bin_image


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ('FILE "game.bin" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n', "not a data track"),
        (
            'FILE "game.bin" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n'
            'FILE "game.bin" BINARY\n  TRACK 02 MODE2/2352\n    INDEX 01 00:00:00\n',
            "second data track",
        ),
        ('FILE "game.bin" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:02:00\n', "does not start its file"),
        ('FILE "other.bin" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n', "which is not in"),
        ("REM nothing\n", "names no track"),
    ],
)
def test_refuses_a_cue_it_cannot_read(bin_image: Path, text: str, message: str):
    cue = bin_image.with_suffix(".cue")
    cue.write_text(text)
    with pytest.raises(DiscError, match=message):
        Disc(cue)


def test_refuses_a_file_that_is_not_a_cd_image(tmp_path: Path):
    path = tmp_path / "game.bin"
    path.write_bytes(bytes(RAW_SECTOR * 20))
    with pytest.raises(DiscError, match="not a CD image"):
        Disc(path)


def _games_folder() -> Path:
    """Where games are looked for: the folder `VENTRIS_ROMS` names (also read from `.cargo/config.toml`,
    as the Rust tests do), or `roms/` in the project."""
    named = os.environ.get("VENTRIS_ROMS")
    settings = PROJECT / ".cargo" / "config.toml"
    if not named and settings.exists():
        named = tomllib.loads(settings.read_text(encoding="utf-8")).get("env", {}).get("VENTRIS_ROMS")
    return Path(named) if isinstance(named, str) and named else PROJECT / "roms"


def test_reads_a_real_disc_when_there_is_one():
    """G.O.D pure (Japan), found by its serial: what its file system holds. Passes when it is not there."""
    for cue in sorted(_games_folder().rglob("*.cue")):
        try:
            disc = Disc(cue)
        except DiscError:
            continue
        with disc:
            if "SYSTEM.CNF" not in disc.files or disc.boot_name() != "SLPS_009.44":
                continue
            assert disc.read("SYSTEM.CNF").startswith(b"BOOT = cdrom:\\SLPS_009.44;1")
            assert disc.files["GOD.DAT"] == (447, 148281344)
            assert "MOVIE/OPENNING.STR" in disc.files
            assert disc.read("SLPS_009.44")[:8] == b"PS-X EXE"
            return
