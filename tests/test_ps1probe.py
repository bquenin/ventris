import os
import struct
import zlib
from pathlib import Path

import pytest

from ventris_workbench import ps1probe
from ventris_workbench.ps1probe import Plan, ProbeError
from ventris_workbench.ps2probe import PRESS_FRAMES


def test_an_address_in_ram_is_shown_in_the_80000000_range():
    assert ps1probe.address("1F2000") == 0x801F2000
    assert ps1probe.address("801F2000") == 0x801F2000
    assert ps1probe.address("0xA01F2000") == 0x801F2000
    assert ps1probe.address("1F800010") == 0x1F800010  # the scratchpad stays where it is
    assert ps1probe.address("BFC00000") == 0xBFC00000  # and so does the BIOS
    with pytest.raises(ProbeError, match="hexadecimal"):
        ps1probe.address("80zz")


def test_a_range_must_be_in_ram_or_the_scratchpad():
    assert ps1probe.memory_range("DB600-DB700") == (0x800DB600, 0x100)
    assert ps1probe.memory_range("1F800000-1F800400") == (0x1F800000, 0x400)
    with pytest.raises(ProbeError, match="not inside RAM"):
        ps1probe.memory_range("300000-300010")
    with pytest.raises(ProbeError, match="ends before"):
        ps1probe.memory_range("800DB700-800DB600")
    with pytest.raises(ProbeError, match="not a range"):
        ps1probe.memory_range("800DB700")


def test_dumps_name_a_memory_or_a_piece_of_ram():
    assert ps1probe.parse_dump("ram@300") == {"frame": 300, "kind": "ram", "name": "ram"}
    assert ps1probe.parse_dump("VRAM@0")["kind"] == "vram"
    assert ps1probe.parse_dump("DB600:100@12") == {
        "frame": 12,
        "kind": "piece",
        "name": "dump-800db600",
        "start": 0x800DB600,
        "length": 0x100,
    }
    with pytest.raises(ProbeError, match="not a dump"):
        ps1probe.parse_dump("ram")
    with pytest.raises(ProbeError, match="not inside RAM"):
        ps1probe.parse_dump("1FFF00:200@1")


def test_a_state_to_save_is_frame_then_file_or_file_at_frame():
    assert ps1probe.parse_save("900:work/town.state") == (900, Path("work/town.state"))
    assert ps1probe.parse_save("900:C:\\work\\town.state") == (900, Path("C:\\work\\town.state"))
    assert ps1probe.parse_save("work/town.state@900") == (900, Path("work/town.state"))
    with pytest.raises(ProbeError):
        ps1probe.parse_save("town.state")


def test_python_values_are_written_as_lua():
    assert ps1probe.lua({"a": [1, True, None], 5: 'x\\y"z'}) == '{a = {1, true, nil}, [5] = "x\\\\y\\"z"}'


def test_the_plan_gives_the_script_button_bits_and_checked_addresses(tmp_path: Path):
    plan = Plan(
        frames=100,
        input=["10:press:circle,start"],
        shots=[0, 100],
        dumps=["ram@50"],
        reads=["DB600-DB700"],
        writes=["801F0000-801F0004"],
        exec=["79800"],
        save="60:" + str(tmp_path / "a.state"),
    )
    settings = ps1probe.build_plan(plan, tmp_path / "run", tmp_path, None)
    pressed = 10 + PRESS_FRAMES
    assert settings["input"] == {10: [[13, True], [3, True]], pressed: [[13, False], [3, False]]}
    assert settings["watches"] == [
        {"kind": "Read", "start": 0x800DB600, "length": 0x100},
        {"kind": "Write", "start": 0x801F0000, "length": 4},
    ]
    assert settings["exec"] == [0x80079800]
    assert settings["shots"] == {0: True, 100: True}
    assert settings["save"] == {"frame": 60, "path": (tmp_path / "a.state").resolve().as_posix()}
    assert settings["state"] is None
    with pytest.raises(ProbeError, match="not a button"):
        ps1probe.build_plan(Plan(input=["0:press:a"]), tmp_path / "run", tmp_path, None)


def test_16_bit_pixels_become_red_green_blue():
    white, red, blue = 0x7FFF, 0x001F, 0x7C00
    assert ps1probe.bgr555_to_rgb(struct.pack("<3H", white, red, blue)) == bytes([255, 255, 255, 255, 0, 0, 0, 0, 255])


def _png_pixels(data: bytes) -> tuple[int, int, bytes]:
    """Read back a PNG that ``png`` wrote: width, height and the pixels without each row's filter byte."""
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    position, chunks = 8, {}
    while position < len(data):
        (length,) = struct.unpack_from(">I", data, position)
        kind = data[position + 4 : position + 8]
        body = data[position + 8 : position + 8 + length]
        assert struct.unpack_from(">I", data, position + 8 + length)[0] == zlib.crc32(kind + body)
        chunks[kind] = body
        position += 12 + length
    width, height = struct.unpack_from(">II", chunks[b"IHDR"])
    rows = zlib.decompress(chunks[b"IDAT"])
    stride = width * 3 + 1
    return width, height, b"".join(rows[y * stride + 1 : (y + 1) * stride] for y in range(height))


def test_a_screenshot_becomes_a_png():
    raw = struct.pack("<4H", 0x001F, 0x03E0, 0x7C00, 0x7FFF)  # red, green, blue, white; two by two
    width, height, pixels = _png_pixels(ps1probe.screenshot_png(raw, 2, 2, 0))
    assert (width, height) == (2, 2)
    assert pixels == bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255])
    rgb = bytes(range(12))
    assert _png_pixels(ps1probe.screenshot_png(rgb, 2, 2, 1))[2] == rgb


def test_the_summary_has_a_line_for_each_instruction(tmp_path: Path):
    log = tmp_path / "run-writes.txt"
    log.write_text(
        "frame=2 pc=80075090 op=a0820000 address=800db68c width=1 value=00 a0=800db68c\n"
        "frame=3 pc=80075090 op=a0820000 address=800db690 width=1 value=00 a0=800db690\n"
        "frame=3 pc=8007b078 op=ac22b628 address=800db628 width=4 value=00000001 a0=00000000\n"
    )
    assert ps1probe.summary(log).splitlines()[1:] == [
        "80075090 a0820000 n=2 frames=2-3 addresses=800db68c-800db691",
        "8007b078 ac22b628 n=1 frames=3-3 addresses=800db628-800db62c",
    ]


def test_says_what_to_set_when_the_emulator_or_bios_is_not_given(monkeypatch, tmp_path: Path):
    monkeypatch.delenv(ps1probe.EMULATOR_VARIABLE, raising=False)
    monkeypatch.delenv(ps1probe.BIOS_VARIABLE, raising=False)
    with pytest.raises(ProbeError, match="PCSX_REDUX environment variable"):
        ps1probe.find_emulator()
    with pytest.raises(ProbeError, match="PCSX_REDUX_BIOS environment variable"):
        ps1probe.find_bios()
    monkeypatch.setenv(ps1probe.BIOS_VARIABLE, str(tmp_path / "missing.bin"))
    with pytest.raises(ProbeError, match="is not a file"):
        ps1probe.find_bios()


def test_runs_the_game_in_the_emulator_when_there_is_one(tmp_path: Path):
    """From power-on to the console's start-up screen, with a screenshot and a dump of RAM. Passes when
    the emulator, the BIOS or G.O.D pure (Japan) is not there."""
    from test_ps1 import _games_folder

    from ventris_workbench.platforms.ps1 import Disc

    if not (os.environ.get(ps1probe.EMULATOR_VARIABLE) and os.environ.get(ps1probe.BIOS_VARIABLE)):
        return
    game = None
    for cue in sorted(_games_folder().rglob("*.cue")):
        with Disc(cue) as disc:
            if "SYSTEM.CNF" in disc.files and disc.boot_name() == "SLPS_009.44":
                game = cue
    if game is None:
        return
    files = ps1probe.run(game, tmp_path / "run", Plan(frames=300, shots=[300], dumps=["ram@300"]))
    assert [path.name for path in files] == ["run-ram-300.bin", "run-shot-300.png"]
    assert files[0].stat().st_size == 0x200000
    width, height, pixels = _png_pixels(files[1].read_bytes())
    assert width >= 256 and height >= 224 and len(set(pixels)) > 2
