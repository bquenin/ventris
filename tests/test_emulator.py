import hashlib
import os
import tomllib
from pathlib import Path

import pytest

from ventris_workbench import emulator
from ventris_workbench.cli import main
from ventris_workbench.emulator import EmulatorError, Probe

PROJECT = Path(__file__).parent.parent
# Tales of Phantasia, as its adapter knows it: the image's SHA-1 and its size without a copier's bytes.
GAME_SHA1 = "fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0"
GAME_SIZE = 0x600000
COPIER_HEADER = 512


def games_folder() -> Path:
    """Where games are looked for: the folder `VENTRIS_ROMS` names, or `roms/` in the project.

    The Rust tests read the same variable. `cargo` can be given it in `.cargo/config.toml`, which
    pytest does not read, so it is read from there too: one setting on a machine serves both.
    """
    named = os.environ.get("VENTRIS_ROMS")
    settings = PROJECT / ".cargo" / "config.toml"
    if not named and settings.exists():
        named = tomllib.loads(settings.read_text(encoding="utf-8")).get("env", {}).get("VENTRIS_ROMS")
    return Path(named) if isinstance(named, str) and named else PROJECT / "roms"


def find_game() -> Path | None:
    """The game as a file the emulator can be given, found by its size and then its SHA-1."""
    for file in sorted(games_folder().rglob("*.s[fm]c")):
        extra = file.stat().st_size - GAME_SIZE
        if extra in (0, COPIER_HEADER) and hashlib.sha1(file.read_bytes()[extra:]).hexdigest() == GAME_SHA1:
            return file
    return None


ROM = find_game()
MESEN = os.environ.get(emulator.MESEN_VARIABLE)
needs_emulator = pytest.mark.skipif(not (MESEN and ROM), reason="needs MESEN and the game file")


def test_probe_settings_become_environment_variables(tmp_path):
    probe = Probe(
        frames=300,
        input="0:mash:start,a",
        trigger="C8E555@6000",
        input_after="0:press:a",
        shots=[120, 240],
        dumps=["snesWorkRam@240"],
        reads=["snesPrgRom:300000-30C81F", "snesWorkRam:7969-7FFF"],
        exec_range="C8E500-C8E6FF",
        log=["C8E550", "C8E5A9:snesWorkRam:24-26"],
        trace_from=10,
        state=tmp_path / "a.mss",
        save="b.mss@200",
    )
    env = probe.environment(tmp_path / "run")
    assert env["PROBE_OUT"] == str(tmp_path / "run")
    assert (env["PROBE_FRAMES"], env["PROBE_TRACE_FROM"]) == ("300", "10")
    assert env["PROBE_SHOTS"] == "120 240"
    assert env["PROBE_READS"] == "snesPrgRom:300000-30C81F snesWorkRam:7969-7FFF"
    assert (env["PROBE_TRIGGER"], env["PROBE_EXEC"], env["PROBE_SAVE"]) == ("C8E555@6000", "C8E500-C8E6FF", "b.mss@200")
    assert env["PROBE_STATE"] == str((tmp_path / "a.mss").resolve())
    assert env["PROBE_LOG"] == "C8E550 C8E5A9:snesWorkRam:24-26"
    assert Probe().environment(tmp_path / "run")["PROBE_STATE"] == ""


def test_listing_uses_the_flags_seen_at_each_instruction(tmp_path):
    # REP #$30 / LDA #$1234 / RTS at $C02000, then, after a gap, LDA #$12 with an 8-bit accumulator.
    rom = bytearray(0x10000)
    rom[0x2000:0x2006] = bytes.fromhex("C2 30 A9 34 12 60")
    rom[0x2010:0x2012] = bytes.fromhex("A9 12")
    trace = tmp_path / "run-exec.txt"
    trace.write_text(
        "c02000 30 a=0000 x=0000 y=0000 dbr=00 n=3\n"
        "c02002 00 a=0000 x=0000 y=0000 dbr=00 n=3\n"
        "c02005 00 a=1234 x=0000 y=0000 dbr=00 n=3\n"
        "c02010 20 a=0000 x=0000 y=0000 dbr=00 n=1\n"
    )
    executed = emulator.read_executed(trace)
    assert executed == [(0xC02000, 0x30, 3), (0xC02002, 0x00, 3), (0xC02005, 0x00, 3), (0xC02010, 0x20, 1)]
    lines = emulator.listing(bytes(rom), "HiROM", executed).splitlines()
    assert [line.split(";")[0].split(None, 1)[0] for line in lines if line] == ["C02000", "C02002", "C02005", "C02010"]
    assert "LDA #$1234" in lines[1] and lines[1].endswith("; x3")
    assert lines[3] == ""  # a gap between two runs of instructions
    assert "LDA #$12 " in lines[4]


def test_mesen_must_be_configured(tmp_path, monkeypatch, capsys):
    monkeypatch.delenv(emulator.MESEN_VARIABLE, raising=False)
    with pytest.raises(EmulatorError, match="set the MESEN environment variable"):
        emulator.find_mesen()
    monkeypatch.setenv(emulator.MESEN_VARIABLE, str(tmp_path / "missing"))
    with pytest.raises(EmulatorError, match="not a file"):
        emulator.find_mesen()
    binary = tmp_path / "Mesen"
    binary.write_text("")
    monkeypatch.setenv(emulator.MESEN_VARIABLE, str(binary))
    with pytest.raises(EmulatorError, match="has not been set up yet"):
        emulator.find_mesen()
    (tmp_path / "settings.json").write_text("{}")
    assert emulator.find_mesen() == binary
    monkeypatch.delenv(emulator.MESEN_VARIABLE)
    assert main(["probe", str(tmp_path / "game.sfc"), "-o", str(tmp_path / "run")]) == 1
    assert "MESEN" in capsys.readouterr().err


@needs_emulator
def test_probe_runs_a_game(tmp_path):
    files = emulator.run_probe(
        ROM,
        tmp_path / "run",
        Probe(
            frames=120,
            shots=[60, 120],
            reads=["snesPrgRom:00FFC0-00FFFF"],
            writes=["snesWorkRam:000000-0000FF"],
            exec_range="C00000-C0FFFF",
            log=["C7F4CF:snesWorkRam:0-3"],
            dumps=["snesWorkRam@120"],
            registers=[120],
            save=f"{tmp_path / 'state.mss'}@120",
        ),
        timeout=120,
    )
    names = {file.name for file in files}
    assert names == {
        "run-shot-60.png",
        "run-shot-120.png",
        "run-reads.txt",
        "run-writes.txt",
        "run-exec.txt",
        "run-log.txt",
        "run-snesWorkRam-120.bin",
        "run-registers-120.txt",
    }
    # A state asked for on the very last frame is still written before the run stops.
    assert (tmp_path / "state.mss").stat().st_size > 10000
    assert "ppu.bgMode=" in (tmp_path / "run-registers-120.txt").read_text()
    assert len((tmp_path / "run-writes.txt").read_text().splitlines()) > 1
    assert (tmp_path / "run-shot-60.png").read_bytes().startswith(b"\x89PNG")
    assert (tmp_path / "run-snesWorkRam-120.bin").stat().st_size == 0x20000
    assert len(emulator.read_executed(tmp_path / "run-exec.txt")) > 20

    with pytest.raises(EmulatorError, match="never fired"):
        emulator.run_probe(
            ROM, tmp_path / "none", Probe(frames=10, trigger="FFFFFF@0", trigger_timeout=120), timeout=120
        )
