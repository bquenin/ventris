"""Run a game in PCSX2 with scripted input and record what it does, in one go.

This is the PlayStation 2's counterpart of ``emulator.run_probe``, which drives Mesen: load a saved
state, press buttons at given frames, take screenshots, save memory, record which code reads or
writes a range, record the registers each time an address is run, write bytes from a file into memory, save a state on
the way, stop. It talks to a patched PCSX2 that is already running the game (see ``pcsx2``).

Writing into memory is how a state made on one build of a game can be played on another: what the
state carries that the new build changed, such as a table of where the game's files are on the disc,
is written over with the new build's own right after the state is loaded.

Frames are counted from where the probe starts. When a state is given, that is two frames after it
is loaded: the game has to draw a frame of its own before there is a picture to take.
"""

import time
from dataclasses import dataclass, field
from pathlib import Path

from ventris_workbench.pcsx2 import BUTTONS, MAIN, READ, WRITE, Hit, Pcsx2, Pcsx2Error

# How long a press holds its buttons, and how long each half of a mash lasts, in frames.
PRESS_FRAMES = 6
MASH_FRAMES = 4
# Hits are collected every so many frames, so that a busy watch does not fill the emulator's recorder,
# which holds RECORDER_HOLDS of them. Each collection stops the game for a moment, so the wait
# between two grows while few hits come back, up to the longest, and shrinks when many do.
COLLECT_EVERY = 30
COLLECT_AT_MOST_EVERY = 480
RECORDER_HOLDS = 65536
# Frames run after a state is loaded and before the probe's first frame. Until the game has drawn
# a frame of its own again there is no picture to take: a screenshot comes out black, or not at all.
SETTLE_FRAMES = 2
# How long to wait for the emulator to write a screenshot it was asked for, in seconds. It writes
# the file on a thread of its own, so the file is there, and growing, before it is whole: it is
# whole when it ends the way a PNG file ends.
SHOT_TIMEOUT = 10.0
PNG_END = bytes.fromhex("49454e44ae426082")
REGISTERS = (
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7",
    "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp", "ra",
)  # fmt: skip


class ProbeError(Exception):
    """What was asked of a probe makes no sense."""


@dataclass
class Plan:
    """What to do during a run."""

    frames: int = 600  # when to stop
    input: list[str] = field(default_factory=list)  # steps "FRAME:KIND:BUTTONS" (mash, press, hold, none)
    shots: list[int] = field(default_factory=list)  # frames at which to take a screenshot
    dumps: list[str] = field(default_factory=list)  # "ADDRESS:LENGTH@FRAME", in hexadecimal but for the frame
    reads: list[str] = field(default_factory=list)  # "START-END": record which code reads the range
    writes: list[str] = field(default_factory=list)  # the same for writes
    log: list[str] = field(default_factory=list)  # "ADDRESS": record the registers each time it is run
    trace_from: int = 0  # the frame at which the watches and logs are set
    state: Path | None = None  # saved state to start from
    save: str = ""  # "FILE@FRAME": save a state at this frame
    pokes: list[str] = field(default_factory=list)  # "ADDRESS:FILE[:OFFSET:LENGTH][@FRAME]", see parse_poke
    fast: bool = True  # run as fast as the machine can, not at the game's own speed


def _number(text: str, what: str) -> int:
    try:
        return int(text, 16)
    except ValueError:
        raise ProbeError(f"{text!r} is not {what} in hexadecimal") from None


def _range(text: str) -> tuple[int, int]:
    start, dash, end = text.partition("-")
    if not dash:
        raise ProbeError(f"{text!r} is not a range such as 1F27A0-1F27A4")
    first, last = _number(start, "an address"), _number(end, "an address")
    if last <= first:
        raise ProbeError(f"the range {text} ends before it starts")
    return first, last


def _is_hex(text: str) -> bool:
    return bool(text) and all(char in "0123456789abcdefABCDEF" for char in text)


@dataclass(frozen=True)
class Poke:
    """Bytes of a file to write into the console's memory: at a frame of the probe, or with no frame
    right after the state is loaded, before the game runs a frame."""

    address: int
    file: Path
    offset: int = 0
    length: int | None = None  # None: to the end of the file
    frame: int | None = None

    def data(self) -> bytes:
        with self.file.open("rb") as source:
            source.seek(self.offset)
            data = source.read() if self.length is None else source.read(self.length)
        if self.length is not None and len(data) != self.length:
            raise ProbeError(f"{self.file} has {len(data)} bytes at {self.offset:X}, not the {self.length:X} asked for")
        return data


def parse_poke(text: str) -> Poke:
    """Read ``ADDRESS:FILE[:OFFSET:LENGTH][@FRAME]``: numbers in hexadecimal but for the frame.

    The file's name may hold colons of its own, as a Windows path does: what follows the address is
    the file, unless its last two parts are hexadecimal numbers, which are then the offset and length.
    """
    where, at, frame = text.rpartition("@")
    if not at or not frame.isdigit():
        where, frame = text, ""
    address, colon, rest = where.partition(":")
    if not colon or not rest or not _is_hex(address):
        raise ProbeError(f"{text!r} is not a poke such as 1DC320:table.bin or 1DC320:SLPS_251.72:DD320:8FD0@0")
    parts = rest.split(":")
    offset, length = 0, None
    if len(parts) >= 3 and _is_hex(parts[-1]) and _is_hex(parts[-2]):
        offset, length = int(parts[-2], 16), int(parts[-1], 16)
        parts = parts[:-2]
    return Poke(int(address, 16), Path(":".join(parts)), offset, length, int(frame) if frame else None)


def _is_whole(picture: Path) -> bool:
    """Whether a screenshot is written to its end. While the emulator is writing it, it may not be
    there yet, or be there and not be readable, or be readable and cut short."""
    try:
        return picture.read_bytes().endswith(PNG_END)
    except OSError:
        return False


def button_changes(steps: list[str], frames: int) -> dict[int, list[tuple[str, bool]]]:
    """What to do to the controller at each frame: the buttons to press or let go, in order.

    A step is ``FRAME:KIND:BUTTONS``. It lasts until the next step. ``press`` holds its buttons for
    a few frames and lets go; ``hold`` keeps them down; ``mash`` presses and lets go again and
    again; ``none`` lets everything go.
    """
    parsed = []
    for step in steps:
        parts = step.split(":")
        if len(parts) != 3 or not parts[0].isdigit() or parts[1] not in ("mash", "press", "hold", "none"):
            raise ProbeError(f"{step!r} is not an input step such as 30:press:cross")
        buttons = [name for name in parts[2].split(",") if name]
        for name in buttons:
            if name not in BUTTONS:
                raise ProbeError(f"{name!r} in {step!r} is not a button: one of {', '.join(BUTTONS)}")
        parsed.append((int(parts[0]), parts[1], buttons))
    parsed.sort(key=lambda step: step[0])

    changes: dict[int, list[tuple[str, bool]]] = {}

    def change(frame: int, buttons: list[str], pressed: bool) -> None:
        if frame < frames:
            changes.setdefault(frame, []).extend((name, pressed) for name in buttons)

    for index, (start, kind, buttons) in enumerate(parsed):
        until = parsed[index + 1][0] if index + 1 < len(parsed) else frames
        if kind == "press":
            change(start, buttons, True)
            change(min(start + PRESS_FRAMES, until), buttons, False)
        elif kind == "hold":
            change(start, buttons, True)
            change(until, buttons, False)
        elif kind == "mash":
            for frame in range(start, until, 2 * MASH_FRAMES):
                change(frame, buttons, True)
                change(min(frame + MASH_FRAMES, until), buttons, False)
    return changes


def report(hits: list[tuple[int, Hit]]) -> str:
    """Hits of watched memory, as one line for each instruction that touched each range."""
    lines = ["# range, the instruction that touched it, how many times, frames, registers the first time"]
    groups: dict[tuple[int, int], list[tuple[int, Hit]]] = {}
    for frame, hit in hits:
        groups.setdefault((hit.address, hit.instruction), []).append((frame, hit))
    for (address, instruction), found in sorted(groups.items()):
        first_frame, first = found[0]
        registers = " ".join(f"{REGISTERS[n]}={first.registers[n]:08x}" for n in (4, 5, 6, 7, 2, 29, 31))
        lines.append(f"{address:08x} {instruction:08x} n={len(found)} frames={first_frame}-{found[-1][0]} {registers}")
    return "\n".join(lines) + "\n"


def log_lines(hits: list[tuple[int, Hit]]) -> str:
    """Hits of traced addresses, one line each with every register."""
    lines = []
    for frame, hit in hits:
        registers = " ".join(f"{REGISTERS[n]}={hit.registers[n]:08x}" for n in range(1, 32))
        lines.append(f"{hit.address:08x} frame={frame} {registers}")
    return "\n".join(lines) + ("\n" if lines else "")


def run(emulator: Pcsx2, output: Path, plan: Plan) -> list[Path]:
    """Carry out a plan on a running emulator and return the files written, all named ``output``-something."""
    prefix = str(output.resolve())
    Path(prefix).parent.mkdir(parents=True, exist_ok=True)
    changes = button_changes(plan.input, plan.frames)
    watches = [(*_range(text), READ) for text in plan.reads] + [(*_range(text), WRITE) for text in plan.writes]
    traces = [_number(text, "an address") for text in plan.log]
    dumps = []
    for text in plan.dumps:
        where, at, frame = text.partition("@")
        address, colon, length = where.partition(":")
        if not at or not colon or not frame.isdigit():
            raise ProbeError(f"{text!r} is not a dump such as 1F2000:400@300")
        dumps.append((int(frame), _number(address, "an address"), _number(length, "a length")))
    pokes = [parse_poke(text) for text in plan.pokes]
    poke_data = [poke.data() for poke in pokes]  # read now, so that a missing file stops the probe first
    save_at, save_file = -1, ""
    if plan.save:
        save_file, at, frame = plan.save.rpartition("@")
        if not at or not frame.isdigit():
            raise ProbeError(f"{plan.save!r} is not a state to save such as scene.p2s@900")
        save_at = int(frame)

    # The frames at which something happens. The emulator runs freely between them.
    planned = {0, plan.frames, plan.trace_from, save_at, *changes, *plan.shots, *(frame for frame, _, _ in dumps)}
    planned |= {poke.frame for poke in pokes if poke.frame is not None}
    planned = sorted(frame for frame in planned if 0 <= frame <= plan.frames)
    watching = bool(watches or traces)

    written: list[Path] = []
    shots: list[Path] = []
    memory_hits: list[tuple[int, Hit]] = []
    run_hits: list[tuple[int, Hit]] = []

    def collect() -> int:
        # A hit says in which of the emulator's frames it happened, so it does not matter when
        # it is collected.
        found = emulator.hits()
        for hit in found:
            (memory_hits if hit.memory else run_hits).append((hit.frame - first_frame, hit))
        return len(found)

    emulator.pause()
    emulator.clear_watches()
    if plan.state:
        emulator.load_state_file(plan.state.resolve())
    # A poke with no frame refreshes what the state brought before the game runs on it.
    for poke, data in zip(pokes, poke_data, strict=True):
        if poke.frame is None:
            emulator.write(poke.address, data)
    if plan.state:
        emulator.advance(SETTLE_FRAMES, timeout=600)
    # After the load, which puts the speed back to the game's own.
    emulator.set_speed(fast=plan.fast)
    first_frame = emulator.frame()
    try:
        now = 0
        every = COLLECT_EVERY
        next_collect = plan.trace_from + every
        while True:
            if watching and now > plan.trace_from:
                # Few hits and the next collection can wait longer; many and it must come sooner.
                count = collect()
                if count > RECORDER_HOLDS // 2:
                    every = max(every // 2, 1)
                elif count < RECORDER_HOLDS // 8:
                    every = min(every * 2, COLLECT_AT_MOST_EVERY)
                next_collect = now + every
            if watching and now == plan.trace_from:
                for start, end, kind in watches:
                    emulator.watch(start, end, kind, MAIN)
                for address in traces:
                    emulator.trace(address, MAIN)
            for poke, data in zip(pokes, poke_data, strict=True):
                if poke.frame == now:
                    emulator.write(poke.address, data)
            for name, pressed in changes.get(now, []):
                emulator.set_button(name, pressed)
            for frame, address, length in dumps:
                if frame == now:
                    path = Path(f"{prefix}-dump-{address:08x}-{now}.bin")
                    path.write_bytes(emulator.read(address, length))
                    written.append(path)
            if now in plan.shots:
                path = Path(f"{prefix}-shot-{now}.png")
                path.unlink(missing_ok=True)
                emulator.screenshot(path)
                shots.append(path)
                written.append(path)
            if now == save_at:
                emulator.save_state_file(Path(save_file).resolve())
                written.append(Path(save_file).resolve())
            if now >= plan.frames:
                break
            stop = next(frame for frame in planned if frame > now)
            if watching and now >= plan.trace_from:
                stop = min(stop, next_collect)
            emulator.advance(stop - now, timeout=600)
            now = stop
        # The emulator saves the next frame it draws. One asked for at the last frame needs that
        # frame run, and every one is waited for, so that a picture that is missing is reported.
        if shots and plan.frames in plan.shots:
            emulator.advance(1, timeout=600)
        deadline = time.monotonic() + SHOT_TIMEOUT
        for path in shots:
            while not _is_whole(path):
                if time.monotonic() > deadline:
                    raise ProbeError(f"the emulator did not write the screenshot {path.name}")
                time.sleep(0.05)
    finally:
        for name in BUTTONS:
            emulator.set_button(name, False)
        emulator.clear_watches()
        emulator.set_speed(fast=False)

    if plan.reads or plan.writes:
        path = Path(f"{prefix}-watch.txt")
        path.write_text(report(memory_hits), encoding="utf-8")
        written.append(path)
    if plan.log:
        path = Path(f"{prefix}-log.txt")
        path.write_text(log_lines(run_hits), encoding="utf-8")
        written.append(path)
    return written


__all__ = ["Pcsx2Error", "Plan", "Poke", "ProbeError", "button_changes", "log_lines", "parse_poke", "report", "run"]
