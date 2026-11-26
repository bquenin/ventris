"""Run a PlayStation game in PCSX-Redux, headless, with scripted input, and record what it does.

This is the PlayStation's counterpart of ``emulator.run_probe``, which drives Mesen for the Super
Famicom: the workbench starts the emulator for each run with no window, from power-on or from a
saved state, and a Lua script inside it presses buttons at given frames, takes screenshots, dumps
memory, records which code reads or writes a range of memory and the registers each time an address
runs, saves a state on the way, and quits at the last frame. PCSX-Redux has a Lua scripting interface
of its own, so unlike PCSX2 it needs no patch.

``ps1probe.lua`` does the work inside the emulator. This module checks the plan, writes the run's
part of the script (the global ``PLAN``), starts the emulator, and turns what it wrote into files to
read: screenshots as PNG, and a summary of each recording of memory accesses by instruction.

The emulator is found through the ``PCSX_REDUX`` environment variable, which names the
``pcsx-redux`` binary, and the BIOS through ``PCSX_REDUX_BIOS``; options can give either instead.

Addresses in RAM are given and shown in the 80000000 range, where a game's code sees RAM (2 MB, up to
801FFFFF). An address of RAM given without the 80 in front, such as ``1F2000``, means the same place.
"""

import array
import gzip
import os
import re
import struct
import subprocess
import sys
import tempfile
import zlib
from dataclasses import dataclass, field
from pathlib import Path

from ventris_workbench.ps2probe import ProbeError, button_changes

EMULATOR_VARIABLE = "PCSX_REDUX"
BIOS_VARIABLE = "PCSX_REDUX_BIOS"
SCRIPT = Path(__file__).with_name("ps1probe.lua")
RAM = 0x80000000
RAM_SIZE = 0x200000
SCRATCHPAD = 0x1F800000
SCRATCHPAD_SIZE = 0x400
VRAM_SIZE = 1024 * 512 * 2
# The bit of each button in the controller's state, as the console reads it.
BUTTON_BITS = {
    "select": 0, "l3": 1, "r3": 2, "start": 3, "up": 4, "right": 5, "down": 6, "left": 7,
    "l2": 8, "r2": 9, "l1": 10, "r1": 11, "triangle": 12, "circle": 13, "cross": 14, "square": 15,
}  # fmt: skip
STATE_REFUSED = 3
GZIP_MAGIC = b"\x1f\x8b"


@dataclass
class Plan:
    """What to do during a run. Frame numbers count from the run's first frame."""

    frames: int = 600  # when to stop
    input: list[str] = field(default_factory=list)  # steps "FRAME:KIND:BUTTONS" (mash, press, hold, none)
    shots: list[int] = field(default_factory=list)  # frames at which to take a screenshot
    dumps: list[str] = field(default_factory=list)  # "WHAT@FRAME": ram, vram, scratch or ADDRESS:LENGTH
    registers: list[int] = field(default_factory=list)  # frames at which to save the processor's registers
    reads: list[str] = field(default_factory=list)  # "START-END": record every read of the range
    writes: list[str] = field(default_factory=list)  # the same for writes
    exec: list[str] = field(default_factory=list)  # "ADDRESS": record the registers each time it runs
    trace_from: int = 0  # the frame at which the watches and logged addresses are set
    state: Path | None = None  # saved state to start from
    save: str = ""  # "FRAME:FILE" (or "FILE@FRAME"): save a state at this frame
    real_time: bool = False  # run at the game's own speed, not as fast as the machine can


def address(text: str) -> int:
    """An address in hexadecimal, with RAM given in or out of the 80000000 range, as the processor sees it.

    ``1F2000``, ``801F2000`` and ``A01F2000`` all name the same byte of RAM, returned as 801F2000.
    Other addresses, such as the scratchpad at 1F800000 or the BIOS at BFC00000, are returned as given.
    """
    try:
        value = int(text.removeprefix("0x").removeprefix("0X").removeprefix("$"), 16)
    except ValueError:
        raise ProbeError(f"{text!r} is not an address in hexadecimal") from None
    if not 0 <= value <= 0xFFFFFFFF:
        raise ProbeError(f"{text!r} is not a 32-bit address")
    if value < RAM_SIZE or ((value & 0x1FFFFFFF) < RAM_SIZE and value >> 29 in (0, 4, 5)):
        return RAM | (value & (RAM_SIZE - 1))
    return value


def memory_range(text: str) -> tuple[int, int]:
    """``START-END`` in RAM or the scratchpad, END not included: (first address, length)."""
    start, dash, end = text.partition("-")
    if not dash:
        raise ProbeError(f"{text!r} is not a range such as 801F2000-801F2400")
    first, last = address(start), address(end)
    if last <= first:
        raise ProbeError(f"the range {text} ends before it starts")
    in_ram = first >= RAM and last <= RAM + RAM_SIZE
    in_scratchpad = first >= SCRATCHPAD and last <= SCRATCHPAD + SCRATCHPAD_SIZE
    if not (in_ram or in_scratchpad):
        raise ProbeError(f"the range {text} is not inside RAM (80000000-80200000) or the scratchpad")
    return first, last - first


def parse_dump(text: str) -> dict:
    """``WHAT@FRAME``: ``ram`` (2 MB), ``vram`` (1024 by 512 pixels of 16 bits), ``scratch`` (the 1 KB
    scratchpad), or ``ADDRESS:LENGTH`` in hexadecimal for a piece of RAM."""
    what, at, frame = text.rpartition("@")
    if not at or not frame.isdigit():
        raise ProbeError(f"{text!r} is not a dump such as ram@300, vram@300 or 801F2000:400@300")
    kind = what.lower()
    if kind in ("ram", "vram", "scratch"):
        return {"frame": int(frame), "kind": kind, "name": kind}
    start, colon, length = what.partition(":")
    if not colon:
        raise ProbeError(f"{text!r} is not a dump such as ram@300, vram@300 or 801F2000:400@300")
    try:
        size = int(length, 16)
    except ValueError:
        raise ProbeError(f"{length!r} in {text!r} is not a length in hexadecimal") from None
    first, _ = memory_range(f"{start}-{address(start) + size:x}")
    if first >= SCRATCHPAD and first < SCRATCHPAD + SCRATCHPAD_SIZE:
        raise ProbeError(f"{text!r}: dump the scratchpad whole, with scratch@FRAME")
    return {"frame": int(frame), "kind": "piece", "name": f"dump-{first:08x}", "start": first, "length": size}


def parse_save(text: str) -> tuple[int, Path]:
    """``FRAME:FILE``, or ``FILE@FRAME`` as ``probe`` and ``ps2-probe`` take it."""
    if match := re.fullmatch(r"(\d+):(.+)", text):
        return int(match.group(1)), Path(match.group(2))
    file, at, frame = text.rpartition("@")
    if at and frame.isdigit() and file:
        return int(frame), Path(file)
    raise ProbeError(f"{text!r} is not a state to save such as 900:work/game/town.state")


def find_emulator(given: Path | None = None) -> Path:
    """The PCSX-Redux binary: the one given, or the one the PCSX_REDUX environment variable names."""
    if given is None:
        value = os.environ.get(EMULATOR_VARIABLE)
        if not value:
            raise ProbeError(
                f"set the {EMULATOR_VARIABLE} environment variable to the path of the pcsx-redux binary, "
                "or give it with --emulator"
            )
        given = Path(value)
    binary = given.expanduser()
    if not binary.is_file():
        raise ProbeError(f"the PCSX-Redux binary {binary} is not a file (from --emulator or {EMULATOR_VARIABLE})")
    return binary


def find_bios(given: Path | None = None) -> Path:
    """The BIOS: the file given, or the one the PCSX_REDUX_BIOS environment variable names."""
    if given is None:
        value = os.environ.get(BIOS_VARIABLE)
        if not value:
            raise ProbeError(
                f"set the {BIOS_VARIABLE} environment variable to a BIOS dumped from your own console, "
                "such as scph5500.bin, or give it with --bios"
            )
        given = Path(value)
    bios = given.expanduser()
    if not bios.is_file():
        raise ProbeError(f"the BIOS {bios} is not a file (from --bios or {BIOS_VARIABLE})")
    return bios


def lua(value: object) -> str:
    """A Python value written as Lua: numbers, strings, booleans, None, lists and dictionaries."""
    if value is None:
        return "nil"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "\\r")
        return f'"{escaped}"'
    if isinstance(value, list | tuple):
        return "{" + ", ".join(lua(item) for item in value) + "}"
    if isinstance(value, dict):
        entries = []
        for key, item in value.items():
            name = key if isinstance(key, str) and key.isidentifier() else f"[{lua(key)}]"
            entries.append(f"{name} = {lua(item)}")
        return "{" + ", ".join(entries) + "}"
    raise TypeError(f"cannot write {value!r} as Lua")


def _path(path: Path) -> str:
    """A path as the script inside the emulator opens it: whole, with forward slashes."""
    return path.resolve().as_posix()


def build_plan(plan: Plan, prefix: Path, work: Path, state: Path | None) -> dict:
    """The plan as the script inside the emulator reads it, checked. ``state`` is the state file to load."""
    if plan.frames < 0:
        raise ProbeError("--frames cannot be negative")
    changes = button_changes(plan.input, plan.frames + 1)
    watches = [{"kind": "Read", "start": s, "length": n} for s, n in map(memory_range, plan.reads)]
    watches += [{"kind": "Write", "start": s, "length": n} for s, n in map(memory_range, plan.writes)]
    save = None
    if plan.save:
        frame, file = parse_save(plan.save)
        save = {"frame": frame, "path": _path(file)}
    return {
        "prefix": _path(prefix),
        "work": _path(work),
        "frames": plan.frames,
        "input": {frame: [[BUTTON_BITS[name], pressed] for name, pressed in steps] for frame, steps in changes.items()},
        "shots": {frame: True for frame in plan.shots},
        "registers": {frame: True for frame in plan.registers},
        "dumps": [parse_dump(text) for text in plan.dumps],
        "watches": watches,
        "exec": [address(text) for text in plan.exec],
        "trace_from": plan.trace_from,
        "state": _path(state) if state else None,
        "save": save,
        "real_time": plan.real_time,
    }


def bgr555_to_rgb(data: bytes) -> bytes:
    """16-bit pixels as the PlayStation stores them (red in the low five bits, then green, then blue,
    little-endian; the top bit is a mask flag) as 8-bit red, green, blue."""
    levels = [(n << 3) | (n >> 2) for n in range(32)]
    table = [bytes((levels[v & 31], levels[(v >> 5) & 31], levels[(v >> 10) & 31])) for v in range(0x10000)]
    pixels = array.array("H", data[: len(data) // 2 * 2])
    if sys.byteorder == "big":
        pixels.byteswap()
    return b"".join(table[v] for v in pixels)


def png(width: int, height: int, rgb: bytes) -> bytes:
    """A PNG file of 8-bit red, green, blue pixels, row after row."""
    if len(rgb) != width * height * 3:
        raise ValueError(f"{len(rgb)} bytes is not {width} by {height} pixels")
    stride = width * 3
    rows = b"".join(b"\x00" + rgb[y * stride : (y + 1) * stride] for y in range(height))

    def chunk(kind: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b"")


def screenshot_png(raw: bytes, width: int, height: int, bpp: int) -> bytes:
    """A screenshot from the emulator as a PNG: ``bpp`` 0 is 16-bit pixels, otherwise 24-bit."""
    if bpp == 0:
        return png(width, height, bgr555_to_rgb(raw[: width * height * 2]))
    return png(width, height, raw[: width * height * 3])


def summary(log: Path) -> str:
    """A recording of accesses (``-reads.txt`` or ``-writes.txt``) as one line per instruction: how many
    times it touched the range, in which frames, and the span of addresses it touched."""
    groups: dict[str, list[int]] = {}  # pc -> [count, first frame, last frame, lowest, highest, op]
    for line in log.read_text(encoding="utf-8").splitlines():
        fields = dict(part.split("=", 1) for part in line.split() if "=" in part)
        frame, where, width = int(fields["frame"]), int(fields["address"], 16), int(fields["width"])
        group = groups.setdefault(fields["pc"], [0, frame, frame, where, where + width, fields["op"]])
        group[0] += 1
        group[2] = frame
        group[3] = min(group[3], where)
        group[4] = max(group[4], where + width)
    lines = ["# instruction, its code, how many accesses, frames, addresses touched (end not included)"]
    for pc, (count, first, last, low, high, op) in sorted(groups.items()):
        lines.append(f"{pc} {op} n={count} frames={first}-{last} addresses={low:08x}-{high:08x}")
    return "\n".join(lines) + "\n"


def _state_file(state: Path, work: Path) -> Path:
    """A saved state as the emulator's script loads it. States the emulator's own window saves are
    compressed with gzip; those the probe saves are not."""
    data = state.read_bytes()
    if not data.startswith(GZIP_MAGIC):
        return state
    plain = work / "state"
    plain.write_bytes(gzip.decompress(data))
    return plain


def run(
    game: Path,
    output: Path,
    plan: Plan,
    emulator: Path | None = None,
    bios: Path | None = None,
    portable: Path | None = None,
    timeout: int = 300,
) -> list[Path]:
    """Carry out ``plan`` on ``game`` (a .cue, .bin or .iso) and return the files written, all named
    ``output``-something but for a saved state.

    ``portable`` is the folder where the emulator keeps its memory cards and settings; by default a new
    empty one for each run, so that a run does not depend on what an earlier one saved.
    """
    binary, bios = find_emulator(emulator), find_bios(bios)
    if not game.is_file():
        raise ProbeError(f"{game} is not a file")
    prefix = output.resolve()
    prefix.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="ps1probe-") as temporary:
        work = Path(temporary)
        state = _state_file(plan.state, work) if plan.state else None
        settings = build_plan(plan, prefix, work, state)
        for old in prefix.parent.glob(prefix.name + "-*"):
            if old.is_file():
                old.unlink()
        if settings["save"]:
            Path(settings["save"]["path"]).parent.mkdir(parents=True, exist_ok=True)
        plan_script = work / "plan.lua"
        plan_script.write_text(f"PLAN = {lua(settings)}\n", encoding="utf-8")
        folder = portable.resolve() if portable else work / "portable"
        folder.mkdir(parents=True, exist_ok=True)
        command = [
            str(binary),
            "-cli",
            "-portable",
            str(folder),
            "-bios",
            str(bios.resolve()),
            "-iso",
            str(game.resolve()),
            "-dofile",
            str(plan_script),
            "-dofile",
            str(SCRIPT),
            "-run",
            "-stdout",
            "-lua_stdout",
        ]
        if settings["watches"] or settings["exec"]:
            # Breakpoints are checked only by the interpreter, with the debugger on.
            command[1:1] = ["-interpreter", "-debugger"]
        try:
            result = subprocess.run(
                command,
                cwd=work,
                capture_output=True,
                text=True,
                errors="replace",
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            raise ProbeError(f"the emulator did not finish within {timeout} seconds (raise --timeout)") from None
        errors = [
            line.split("ps1probe error: ", 1)[1].strip()
            for line in result.stdout.splitlines()
            if "ps1probe error: " in line
        ]
        if result.returncode == STATE_REFUSED:
            raise ProbeError(errors[-1] if errors else f"the emulator refused the saved state {plan.state}")
        if result.returncode != 0 or not (work / "done").exists():
            lines = [line for line in (result.stdout + result.stderr).splitlines() if line.strip()]
            detail = "; ".join(errors[-3:]) or "; ".join(lines[-3:]) or "no message"
            raise ProbeError(f"the emulator run failed with exit code {result.returncode}: {detail}")

        written = [path for path in prefix.parent.glob(prefix.name + "-*") if path.is_file()]
        for raw in work.glob("shot-*.raw"):
            frame, size, bpp = raw.stem.split("-")[1:]
            width, height = (int(n) for n in size.split("x"))
            picture = Path(f"{prefix}-shot-{frame}.png")
            picture.write_bytes(screenshot_png(raw.read_bytes(), width, height, int(bpp)))
            written.append(picture)
        for vram in [path for path in written if path.name.startswith(prefix.name + "-vram-")]:
            # The emulator keeps room for a second megabyte of video memory, which only some boards
            # made for developers had; a console sold in shops has the first, 1024 by 512 pixels.
            data = vram.read_bytes()[:VRAM_SIZE]
            vram.write_bytes(data)
            picture = vram.with_suffix(".png")
            picture.write_bytes(png(1024, 512, bgr555_to_rgb(data)))
            written.append(picture)
        for kind in ("reads", "writes"):
            log = Path(f"{prefix}-{kind}.txt")
            if getattr(plan, kind) and not log.exists():
                log.write_text("", encoding="utf-8")  # watched, and nothing touched the range
                written.append(log)
            if log.exists():
                report = Path(f"{prefix}-{kind}-summary.txt")
                report.write_text(summary(log), encoding="utf-8")
                written.append(report)
        if settings["save"]:
            written.append(Path(settings["save"]["path"]))
    return sorted(set(written))


__all__ = [
    "Plan",
    "ProbeError",
    "address",
    "bgr555_to_rgb",
    "build_plan",
    "find_bios",
    "find_emulator",
    "lua",
    "memory_range",
    "parse_dump",
    "parse_save",
    "png",
    "run",
    "screenshot_png",
    "summary",
]
