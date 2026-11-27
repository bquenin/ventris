import hashlib
import json
import os
import struct
import sys
import zipfile
import zlib
from pathlib import Path

import pytest
from test_emulator import games_folder

from ventris_workbench import dsprobe
from ventris_workbench.cli import main
from ventris_workbench.dsprobe import DesmumeError, Plan
from ventris_workbench.ps2probe import ProbeError

# Ganbare Goemon: Tokaidouchuu Ooedo Tengurigaeshi no Maki, the Japanese release (A5EJ), as its
# adapter knows it: the image's SHA-1 and its size.
GAME_SHA1 = "a90e1c3491a3120567385245fdb7302c9866fcd4"
GAME_SIZE = 0x4000000


def find_game() -> tuple[Path, str | None] | None:
    """The game, by its size and then its SHA-1: a file, or a file and the name of the member of a
    zip archive that is the game, as the Rust tests also take it."""
    folder = games_folder()
    for file in sorted(folder.rglob("*.nds")):
        if file.stat().st_size == GAME_SIZE and hashlib.sha1(file.read_bytes()).hexdigest() == GAME_SHA1:
            return file, None
    for file in sorted(folder.rglob("*.zip")):
        with zipfile.ZipFile(file) as archive:
            for member in archive.infolist():
                if member.file_size == GAME_SIZE and hashlib.sha1(archive.read(member)).hexdigest() == GAME_SHA1:
                    return file, member.filename
    return None


DESMUME = os.environ.get(dsprobe.DESMUME_VARIABLE)
GAME = find_game() if DESMUME else None
needs_emulator = pytest.mark.skipif(not (DESMUME and GAME), reason="needs DESMUME and the game file")


@pytest.fixture
def rom(tmp_path) -> Path:
    """The game as a file the emulator can open, taken out of its archive if it is in one."""
    assert GAME is not None
    path, member = GAME
    if member is None:
        return path
    with zipfile.ZipFile(path) as archive:
        (tmp_path / "game.nds").write_bytes(archive.read(member))
    return tmp_path / "game.nds"


def test_input_steps_become_the_keypad():
    schedule = dsprobe.prepare(Plan(frames=40, input=["0:none", "10:press:a", "20:hold:left,up", "30:none"]))
    a, left, up = 1 << 0, 1 << 5, 1 << 6
    # A press holds for a few frames; a hold lasts until the next step.
    assert schedule.keypad == {10: a, 16: 0, 20: left | up, 30: 0}
    with pytest.raises(ProbeError, match="'cross' in '10:press:cross' is not a button"):
        dsprobe.prepare(Plan(input=["10:press:cross"]))


def test_a_touch_is_read_from_its_name():
    assert dsprobe.parse_touch("90:60,104") == dsprobe.Touch(90, 60, 104, 6)
    assert dsprobe.parse_touch("90:255,191:30") == dsprobe.Touch(90, 255, 191, 30)
    for wrong in ("90", "90:60", "90:60,104,1", "x:60,104", "90:60,104:", "90:60,104:3:4"):
        with pytest.raises(ProbeError, match="is not a touch"):
            dsprobe.parse_touch(wrong)
    with pytest.raises(ProbeError, match="off the screen"):
        dsprobe.parse_touch("90:60,192")


def test_a_dump_names_a_memory_or_an_address():
    assert dsprobe.parse_ds_dump("main@300") == dsprobe.Dump(300, "main", dsprobe.ARM9, 0x02000000, 0x400000)
    assert dsprobe.parse_ds_dump("arm7@1") == dsprobe.Dump(1, "arm7", dsprobe.ARM7, 0x03800000, 0x10000)
    assert dsprobe.parse_ds_dump("27C0000:4000@300") == dsprobe.Dump(
        300, "dump-027c0000", dsprobe.ARM9, 0x027C0000, 0x4000
    )
    for wrong in ("main", "main@", "vram@300", "27C0000@300"):
        with pytest.raises(ProbeError, match="is not a dump"):
            dsprobe.parse_ds_dump(wrong)


def test_what_makes_no_sense_is_refused():
    for plan in (
        Plan(reads=["2200000"]),
        Plan(writes=["6200100-6200000"]),
        Plan(exec_range="here"),
        Plan(save="scene.dst"),
        Plan(touch=["1:300,10"]),
    ):
        with pytest.raises(ProbeError):
            dsprobe.prepare(plan)
    schedule = dsprobe.prepare(
        Plan(reads=["27FF000-27FFFFF"], writes=["6200000-621FFFF"], exec_range="37F8000-380FFFF", save="a.dst@9")
    )
    assert schedule.watches == [
        ("reads", "27FF000-27FFFFF", 0x027FF000, 0x027FFFFF),
        ("writes", "6200000-621FFFF", 0x06200000, 0x0621FFFF),
    ]
    assert (schedule.exec_range, schedule.save) == ((0x037F8000, 0x0380FFFF), ("a.dst", 9))


def test_a_plan_reaches_the_run_process_whole(tmp_path):
    plan = Plan(
        frames=300,
        input=["0:mash:start,a"],
        touch=["90:60,104:8"],
        shots=[100, 300],
        dumps=["main@300"],
        registers=[300],
        reads=["2200000-23BFFFF"],
        exec_range="2000000-2000FFF",
        trace_from=10,
        state=tmp_path / "start.dst",
        save="later.dst@200",
    )
    task = json.loads(dsprobe.request(tmp_path / "desmume.dll", tmp_path / "game.nds", tmp_path / "run", plan))
    assert task["library"] == str(tmp_path / "desmume.dll") and task["prefix"] == str(tmp_path / "run")
    assert task["rom"] == str((tmp_path / "game.nds").resolve())
    assert task["plan"]["state"] == str((tmp_path / "start.dst").resolve())
    assert Plan.from_json(task["plan"]) == Plan(**{**plan.__dict__, "state": (tmp_path / "start.dst").resolve()})
    assert Plan.from_json(json.loads(json.dumps(Plan().to_json()))) == Plan()
    # The run is a process of its own, on the same Python.
    assert dsprobe.command() == [sys.executable, "-m", "ventris_workbench.dsprobe"]


def test_reports_are_read_back(tmp_path):
    seen = {
        # Two instructions of the ARM9 read the range, and a DMA, which has no instruction.
        (dsprobe.ARM9, 1, 0x0200A0D0, 2): [101, 0x027FFFA8, 0x027FFFA9, 100, 200, 0],
        (dsprobe.ARM9, 3, None, 4): [3, 0x027FF000, 0x027FF00B, 150, 151, 0],
        (dsprobe.ARM7, 1, 0x037F8010, 4): [1, 0x027FF800, 0x027FF803, 120, 120, 0],
    }
    reads = dsprobe.report("reads", [("27FF000-27FFFFF", seen)])
    lines = reads.splitlines()
    assert lines[0].startswith("# range") and "value" not in lines[0]
    # By the frame each was first seen.
    assert lines[1:] == [
        "27FF000-27FFFFF arm9 data 0200a0d0 size=2 n=101 span=027fffa8-027fffa9 frames=100-200",
        "27FF000-27FFFFF arm7 data 037f8010 size=4 n=1 span=027ff800-027ff803 frames=120-120",
        "27FF000-27FFFFF arm9 dma  -------- size=4 n=3 span=027ff000-027ff00b frames=150-151",
    ]
    path = tmp_path / "run-reads.txt"
    path.write_text(reads, encoding="utf-8")
    assert dsprobe.read_accesses(path) == [
        ("27FF000-27FFFFF", "arm9", "data", 0x0200A0D0, 2, 101),
        ("27FF000-27FFFFF", "arm7", "data", 0x037F8010, 4, 1),
        ("27FF000-27FFFFF", "arm9", "dma", None, 4, 3),
    ]
    # A write says what it wrote the first time.
    writes = dsprobe.report("writes", [("6200000-621FFFF", {(dsprobe.ARM9, 1, 0x02001000, 2): [1, 6, 7, 5, 5, 0x1F]})])
    assert writes.splitlines()[1].endswith("frames=5-5 value=001f")

    executed = {(dsprobe.ARM7, 0x037F8000): [101, 0xE92D4000, 4], (dsprobe.ARM9, 0x02000800): [2, 0x4770, 2]}
    path = tmp_path / "run-exec.txt"
    path.write_text(dsprobe.exec_report(executed), encoding="utf-8")
    assert path.read_text().splitlines()[1:] == ["arm9 02000800 2 4770 n=2", "arm7 037f8000 4 e92d4000 n=101"]
    assert dsprobe.read_executed(path) == [("arm9", 0x02000800, 2, 0x4770, 2), ("arm7", 0x037F8000, 4, 0xE92D4000, 101)]


def test_a_screenshot_is_a_png(tmp_path):
    path = tmp_path / "shot.png"
    rgb = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9])  # two by two
    dsprobe.write_png(path, rgb, 2, 2)
    data = path.read_bytes()
    assert data.startswith(b"\x89PNG\r\n\x1a\n")
    assert struct.unpack(">II", data[16:24]) == (2, 2)
    start = data.index(b"IDAT") + 4
    length = struct.unpack(">I", data[start - 8 : start - 4])[0]
    assert zlib.decompress(data[start : start + length]) == b"\x00" + rgb[:6] + b"\x00" + rgb[6:]


def test_desmume_must_be_configured(tmp_path, monkeypatch, capsys):
    monkeypatch.delenv(dsprobe.DESMUME_VARIABLE, raising=False)
    with pytest.raises(DesmumeError, match="set the DESMUME environment variable"):
        dsprobe.find_desmume()
    monkeypatch.setenv(dsprobe.DESMUME_VARIABLE, str(tmp_path / "missing.dll"))
    with pytest.raises(DesmumeError, match="not a file"):
        dsprobe.find_desmume()
    # DeSmuME's own library, without the patch, lacks the functions it adds.
    library = tmp_path / "desmume.dll"
    library.write_bytes(b"MZ desmume_open desmume_cycle desmume_memory_register_read")
    monkeypatch.setenv(dsprobe.DESMUME_VARIABLE, str(library))
    with pytest.raises(DesmumeError, match="lacks desmume_memory_hook_pc, desmume_memory_read_block"):
        dsprobe.find_desmume()
    library.write_bytes(b"MZ " + b" ".join(dsprobe.PATCHED_FUNCTIONS))
    assert dsprobe.find_desmume() == library
    monkeypatch.delenv(dsprobe.DESMUME_VARIABLE)
    assert main(["ds-probe", str(tmp_path / "game.nds"), "-o", str(tmp_path / "run")]) == 1
    assert "DESMUME" in capsys.readouterr().err


def test_a_run_that_fails_says_why(tmp_path, monkeypatch):
    library = tmp_path / "desmume.dll"
    library.write_bytes(b" ".join(dsprobe.PATCHED_FUNCTIONS))
    monkeypatch.setenv(dsprobe.DESMUME_VARIABLE, str(library))
    rom = tmp_path / "game.nds"
    rom.write_bytes(bytes(512))
    # The plan is checked before anything starts.
    with pytest.raises(ProbeError):
        dsprobe.run(rom, tmp_path / "run", Plan(reads=["here"]))
    with pytest.raises(DesmumeError, match="is not a file"):
        dsprobe.run(tmp_path / "other.nds", tmp_path / "run", Plan())
    # The library here is no library at all: the run's process says so, and the workbench goes on.
    (tmp_path / "run-shot-1.png").write_bytes(b"from an earlier run")
    with pytest.raises(DesmumeError, match="the emulator run failed"):
        dsprobe.run(rom, tmp_path / "run", Plan(frames=1))
    assert not (tmp_path / "run-shot-1.png").exists()


@needs_emulator
def test_probe_runs_a_game(tmp_path, rom):
    state = tmp_path / "scene.dst"
    files = dsprobe.run(
        rom,
        tmp_path / "run",
        Plan(
            frames=120,
            input=["0:none", "60:mash:start,a"],
            touch=["100:128,96"],
            shots=[120],
            dumps=["main@120", "pal@120"],
            registers=[120],
            reads=["027FF000-027FFFFF"],
            writes=["02000000-023FFFFF"],
            exec_range="02000000-0200FFFF",
            trace_from=100,
            save=f"{state}@120",
        ),
        timeout=120,
    )
    names = [file.name for file in files]
    assert names == [
        "run-exec.txt",
        "run-main-120.bin",
        "run-pal-120.bin",
        "run-reads.txt",
        "run-registers-120.txt",
        "run-shot-120.png",
        "run-writes.txt",
        "scene.dst",
    ]
    # Both screens, one above the other.
    assert struct.unpack(">II", (tmp_path / "run-shot-120.png").read_bytes()[16:24]) == (256, 384)
    assert (tmp_path / "run-main-120.bin").stat().st_size == 0x400000
    assert (tmp_path / "run-registers-120.txt").read_text().splitlines()[1].startswith("arm7 r0=")
    assert state.stat().st_size > 100000
    # The watches record from frame 100 only, and each line says which processor it was.
    writes = dsprobe.read_accesses(tmp_path / "run-writes.txt")
    assert writes and {cpu for _, cpu, *_ in writes} <= {"arm9", "arm7"}
    assert all(
        int(line.split("frames=")[1].split("-")[0]) >= 100
        for line in (tmp_path / "run-writes.txt").read_text().splitlines()[1:]
    )
    assert len(dsprobe.read_executed(tmp_path / "run-exec.txt")) > 20
    # The battery save is kept beside what the run wrote.
    assert (tmp_path / "run-battery").is_dir()

    # A run from the state goes on from it.
    files = dsprobe.run(rom, tmp_path / "again", Plan(frames=10, state=state, shots=[10]), timeout=120)
    assert [file.name for file in files] == ["again-shot-10.png"]
