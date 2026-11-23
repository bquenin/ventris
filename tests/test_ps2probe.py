from pathlib import Path

import pytest

from ventris_workbench import ps2probe
from ventris_workbench.pcsx2 import MAIN, READ, WRITE, Hit

# The emulator's own count of frames at the probe's first frame: any number will do.
STARTED = 5000


class Recorded:
    """Stands in for the emulator's client: notes what it is asked, and hands out hits it was given."""

    def __init__(self):
        self.now = 0  # frames since the probe's first
        self.asked: list[tuple] = []
        self.pending: dict[int, list[Hit]] = {}  # hits by the probe's frame they happen in

    def _note(self, *what):
        self.asked.append((self.now, *what))

    def pause(self):
        self._note("pause")

    def clear_watches(self):
        self._note("clear")

    def load_state_file(self, path):
        self._note("load", path.name)
        # The probe runs a couple of frames after a load that it does not count.
        self.now = -ps2probe.SETTLE_FRAMES

    def save_state_file(self, path):
        self._note("save", path.name)

    def frame(self):
        return STARTED + self.now

    def set_speed(self, fast):
        self._note("fast" if fast else "real time")

    def advance(self, frames, timeout=60.0):
        self.now += frames

    def set_button(self, name, pressed):
        if pressed or any(note[1:] == ("button", name, True) for note in self.asked):
            self._note("button", name, pressed)

    def watch(self, start, end, kind, cpu):
        self._note("watch", start, end, kind, cpu)

    def trace(self, address, cpu):
        self._note("trace", address, cpu)

    def hits(self):
        self._note("collect")
        return [hit for frame in sorted(self.pending) if frame <= self.now for hit in self.pending.pop(frame)]

    def screenshot(self, path):
        self._note("shot", path.name)
        if "missing" not in path.name:
            path.write_bytes(b"picture" + ps2probe.PNG_END)

    def read(self, address, length):
        return bytes(length)

    def write(self, address, data):
        self._note("write", address, data)


def test_input_steps_become_button_changes():
    changes = ps2probe.button_changes(["10:press:cross", "30:hold:left,up", "50:none:", "60:mash:start"], 80)
    assert changes[10] == [("cross", True)] and changes[16] == [("cross", False)]
    # A hold lasts until the next step.
    assert changes[30] == [("left", True), ("up", True)] and changes[50] == [("left", False), ("up", False)]
    # A mash presses and lets go, four frames each, until the run ends.
    assert [frame for frame in changes if frame >= 60] == [60, 64, 68, 72, 76]
    assert changes[60] == [("start", True)] and changes[64] == [("start", False)]
    for wrong in ("ten:press:cross", "10:tap:cross", "10:press:fire", "10:press"):
        with pytest.raises(ps2probe.ProbeError):
            ps2probe.button_changes([wrong], 80)


def test_a_plan_is_carried_out_in_order(tmp_path):
    emulator = Recorded()
    registers = tuple(range(32))
    emulator.pending = {
        130: [
            Hit(True, MAIN, 0x200000, 0x1F27A0, False, STARTED + 130, registers),
            Hit(False, MAIN, 0x14AF3C, 0x14AF3C, False, STARTED + 130, registers),
        ],
        190: [Hit(True, MAIN, 0x200010, 0x1F27A0, True, STARTED + 190, registers)],
    }
    plan = ps2probe.Plan(
        frames=200,
        input=["20:press:cross"],
        shots=[50],
        dumps=["1F2000:10@60"],
        reads=["1F27A0-1F27A4"],
        writes=["300000-300010"],
        log=["14AF3C"],
        trace_from=100,
        state=tmp_path / "start.p2s",
        save=f"{tmp_path / 'later.p2s'}@150",
    )
    written = ps2probe.run(emulator, tmp_path / "run", plan)

    # The speed is set after the load, which puts it back to the game's own.
    assert emulator.asked[:4] == [(0, "pause"), (0, "clear"), (0, "load", "start.p2s"), (0, "fast")]
    during = [note for note in emulator.asked[4:] if note[1] not in ("clear", "real time", "collect")]
    assert during == [
        (20, "button", "cross", True),
        (26, "button", "cross", False),
        (50, "shot", "run-shot-50.png"),
        (100, "watch", 0x1F27A0, 0x1F27A4, READ, MAIN),
        (100, "watch", 0x300000, 0x300010, WRITE, MAIN),
        (100, "trace", 0x14AF3C, MAIN),
        (150, "save", "later.p2s"),
        # At the end every button is let go, whatever the plan left pressed.
        (200, "button", "cross", False),
    ]
    # At the end every button is let go, the watches are cleared and the game runs at its own speed.
    assert emulator.asked[-2:] == [(200, "clear"), (200, "real time")] and emulator.now == 200
    # Hits are collected 30 frames after the watches are set, and then at longer waits while few
    # come back: not at every one of the hundred frames.
    assert [note[0] for note in emulator.asked if note[1] == "collect"] == [130, 150, 200]

    names = sorted(path.name for path in written)
    assert names == ["later.p2s", "run-dump-001f2000-60.bin", "run-log.txt", "run-shot-50.png", "run-watch.txt"]
    assert (tmp_path / "run-dump-001f2000-60.bin").read_bytes() == bytes(16)
    # One line for each instruction that touched each range; in a delay slot it is the one after the
    # branch. A hit keeps the frame it happened in, though it was collected ten frames later.
    watch = (tmp_path / "run-watch.txt").read_text().splitlines()
    assert watch[1].startswith("001f27a0 00200000 n=1 frames=130-130 a0=00000004")
    assert watch[2].startswith("001f27a0 00200014 n=1 frames=190-190")
    log = (tmp_path / "run-log.txt").read_text().splitlines()
    assert len(log) == 1 and log[0].startswith("0014af3c frame=130 at=00000001") and log[0].endswith("ra=0000001f")


def test_a_screenshot_is_waited_for(tmp_path, monkeypatch):
    # One asked for at the last frame needs a frame more to be drawn.
    emulator = Recorded()
    ps2probe.run(emulator, tmp_path / "run", ps2probe.Plan(frames=40, shots=[40]))
    assert emulator.now == 41 and (tmp_path / "run-shot-40.png").exists()
    # One the emulator never writes is reported, not passed over.
    monkeypatch.setattr(ps2probe, "SHOT_TIMEOUT", 0.2)
    with pytest.raises(ps2probe.ProbeError, match="did not write the screenshot missing-shot-10"):
        ps2probe.run(Recorded(), tmp_path / "missing", ps2probe.Plan(frames=40, shots=[10]))


def test_what_makes_no_sense_is_refused(tmp_path):
    for plan in (
        ps2probe.Plan(reads=["1F27A4-1F27A0"]),
        ps2probe.Plan(writes=["1F27A0"]),
        ps2probe.Plan(log=["here"]),
        ps2probe.Plan(dumps=["1F2000@60"]),
        ps2probe.Plan(save="later.p2s"),
    ):
        with pytest.raises(ps2probe.ProbeError):
            ps2probe.run(Recorded(), tmp_path / "run", plan)


def test_a_poke_is_read_from_its_name(tmp_path):
    assert ps2probe.parse_poke("1DC320:table.bin") == ps2probe.Poke(0x1DC320, Path("table.bin"))
    # An offset and a length take part of the file; a frame says when.
    assert ps2probe.parse_poke("1dc320:SLPS_251.72:DD320:8FD0@40") == ps2probe.Poke(
        0x1DC320, Path("SLPS_251.72"), 0xDD320, 0x8FD0, 40
    )
    # A Windows path keeps its colon, with or without the offset and length.
    assert ps2probe.parse_poke(r"10:C:\work\a.bin") == ps2probe.Poke(0x10, Path(r"C:\work\a.bin"))
    assert ps2probe.parse_poke(r"10:C:\work\a.bin:4:2@0") == ps2probe.Poke(0x10, Path(r"C:\work\a.bin"), 4, 2, 0)
    for wrong in ("table.bin", "1DC320", "1DC320:", "here:table.bin"):
        with pytest.raises(ps2probe.ProbeError):
            ps2probe.parse_poke(wrong)
    # The bytes are the part of the file asked for, and a file too short for them is an error.
    source = tmp_path / "source.bin"
    source.write_bytes(bytes(range(16)))
    assert ps2probe.parse_poke(f"0:{source}:4:3").data() == bytes([4, 5, 6])
    assert ps2probe.parse_poke(f"0:{source}").data() == bytes(range(16))
    with pytest.raises(ps2probe.ProbeError):
        ps2probe.parse_poke(f"0:{source}:E:4").data()


def test_a_poke_comes_before_the_game_runs_on_the_state(tmp_path):
    source = tmp_path / "table.bin"
    source.write_bytes(b"abcd")
    emulator = Recorded()
    plan = ps2probe.Plan(frames=20, state=tmp_path / "old.p2s", pokes=[f"100:{source}", f"200:{source}:1:2@10"])
    ps2probe.run(emulator, tmp_path / "run", plan)
    loaded = emulator.asked.index((0, "load", "old.p2s"))
    # Right after the load, before the frames that let the game draw again.
    assert emulator.asked[loaded + 1] == (-ps2probe.SETTLE_FRAMES, "write", 0x100, b"abcd")
    assert (10, "write", 0x200, b"bc") in emulator.asked
