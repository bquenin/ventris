"""Run a Nintendo DS game in DeSmuME, headless, and record what it does.

This is the Nintendo DS's counterpart of ``emulator.run_probe``, which drives Mesen, and of
``ps2probe.run``, which drives PCSX2: start from power-on or a saved state, press buttons and touch the
bottom screen at given frames, take screenshots of both screens, save memory and the registers, record
which code reads or writes a range and which instructions run, save a state on the way, stop.

DeSmuME is used as a library: its interface frontend, a DLL on Windows, called through ``ctypes``. It
has to be built with the patch in ``tools/desmume``, which adds what a probe needs: which processor and
which instruction made a watched access, reading memory without setting a watch off, and where the
battery save goes. The library is found through the ``DESMUME`` environment variable, which should name
the file.

Each run is a process of its own (``python -m ventris_workbench.dsprobe``, started by ``run``): the
library keeps the whole console in global variables, and a fault inside it ends the process it is in.
Started that way, a crash ends the run, not the workbench.

Frames count from power-on, or from the state when one is given. Frame N is the Nth frame the console
runs: a screenshot, a dump or a state at frame N is taken once it has run, and a watch says an access
happened in frame N when it happened while that frame ran. Addresses are hexadecimal, as the processor
that made the access sees them; both processors are watched at once, and each line says which it was.
"""

import ctypes
import json
import os
import re
import struct
import subprocess
import sys
import time
import zlib
from dataclasses import asdict, dataclass, field
from pathlib import Path

from ventris_workbench.ps2probe import (
    PRESS_FRAMES,
    ProbeError,
    button_changes,
    parse_dump,
    parse_range,
    parse_save,
)

DESMUME_VARIABLE = "DESMUME"
# Functions only the patched library has. One without them is DeSmuME's own, which can run a game
# but cannot say who made an access.
PATCHED_FUNCTIONS = (b"desmume_memory_hook_pc", b"desmume_memory_read_block", b"desmume_set_battery_dir")
# The console's buttons, in the order of the bits the library takes them in.
BUTTONS = ("a", "b", "select", "start", "right", "left", "up", "down", "r", "l", "x", "y")
# What a screenshot holds: both screens, 256 by 192 pixels each, the top one above the bottom one.
WIDTH, HEIGHT = 256, 384
SCREEN_HEIGHT = 192
# The two processors, as the library numbers them.
ARM9, ARM7 = 0, 1
CPU_NAMES = {ARM9: "arm9", ARM7: "arm7"}
# The kinds of access a watch tells apart. For a DMA the instruction the processor was at has
# nothing to do with it, so none is given.
ACCESSES = {0: "code", 1: "data", 3: "dma"}
DMA = 3
# Memory a dump can name: (processor that sees it, address, length).
MEMORIES = {
    "main": (ARM9, 0x02000000, 0x400000),  # main memory, 4 MiB
    "arm7": (ARM7, 0x03800000, 0x10000),  # the ARM7's own work memory
    "wram": (ARM9, 0x03000000, 0x8000),  # the shared work memory, as the ARM9 sees it
    "bga": (ARM9, 0x06000000, 0x80000),  # video memory, through the main engine's background window
    "bgb": (ARM9, 0x06200000, 0x20000),  # the same for the sub engine
    "obja": (ARM9, 0x06400000, 0x40000),  # video memory, through the main engine's sprite window
    "objb": (ARM9, 0x06600000, 0x20000),  # the same for the sub engine
    "pal": (ARM9, 0x05000000, 0x800),  # both engines' palettes
    "oam": (ARM9, 0x07000000, 0x800),  # both engines' sprite tables
    "itcm": (ARM9, 0x01FF8000, 0x8000),  # the ARM9's fast instruction memory, where games usually map it
}
REGISTERS = (*(f"r{n}" for n in range(16)), "cpsr", "spsr")
# The firmware's language: 0 is Japanese. A game made for several languages picks its own by it.
LANGUAGE = 0


class DesmumeError(Exception):
    """The emulator is missing or not patched, or the run failed."""


@dataclass
class Plan:
    """What to do during a run."""

    frames: int = 600  # when to stop
    input: list[str] = field(default_factory=list)  # steps "FRAME:KIND:BUTTONS" (mash, press, hold, none)
    touch: list[str] = field(default_factory=list)  # "FRAME:X,Y[:FRAMES]": touch the bottom screen
    shots: list[int] = field(default_factory=list)  # frames at which to take a screenshot
    dumps: list[str] = field(default_factory=list)  # "MEMORY@FRAME", or "ADDRESS:LENGTH@FRAME" as the ARM9 sees it
    registers: list[int] = field(default_factory=list)  # frames at which to save both processors' registers
    reads: list[str] = field(default_factory=list)  # "START-END": record which code reads the range
    writes: list[str] = field(default_factory=list)  # the same for writes
    exec_range: str = ""  # "START-END": record the instructions run in this range
    trace_from: int = 0  # the first frame the watches record
    state: Path | None = None  # saved state to start from
    save: str = ""  # "FILE@FRAME": save a state at this frame

    def to_json(self) -> dict:
        return {**asdict(self), "state": str(self.state.resolve()) if self.state else None}

    @classmethod
    def from_json(cls, data: dict) -> "Plan":
        return cls(**{**data, "state": Path(data["state"]) if data.get("state") else None})


@dataclass(frozen=True)
class Touch:
    frame: int
    x: int
    y: int
    frames: int = PRESS_FRAMES


@dataclass(frozen=True)
class Dump:
    frame: int
    name: str  # what the file is called after
    cpu: int
    address: int
    length: int


@dataclass
class Schedule:
    """A plan, checked and worked out into what to do at each frame."""

    keypad: dict[int, int]  # the buttons held, as the library's bits, from each frame where they change
    touches: list[Touch]
    dumps: list[Dump]
    watches: list[tuple[str, str, int, int]]  # ("reads" or "writes", the range as given, first, last)
    exec_range: tuple[int, int] | None
    save: tuple[str, int] | None


def parse_touch(text: str) -> Touch:
    """Read ``FRAME:X,Y[:FRAMES]``: touch the bottom screen at X, Y (in pixels from its top left) for
    some frames, a press's worth when not given."""
    parts = text.split(":")
    point = parts[1].split(",") if len(parts) > 1 else []
    numbers = [parts[0], *point, *parts[2:]]
    if len(parts) not in (2, 3) or len(point) != 2 or not all(part.isdigit() for part in numbers):
        raise ProbeError(f"{text!r} is not a touch such as 90:128,96 or 90:128,96:30")
    touch = Touch(int(parts[0]), int(point[0]), int(point[1]), int(parts[2]) if len(parts) == 3 else PRESS_FRAMES)
    if touch.x >= WIDTH or touch.y >= SCREEN_HEIGHT:
        raise ProbeError(f"{text!r} is off the screen, which is {WIDTH} by {SCREEN_HEIGHT}")
    return touch


def parse_ds_dump(text: str) -> Dump:
    """Read ``MEMORY@FRAME``, a memory in ``MEMORIES``, or ``ADDRESS:LENGTH@FRAME``, as the ARM9 sees it."""
    name, at, frame = text.partition("@")
    if name in MEMORIES and at and frame.isdigit():
        return Dump(int(frame), name, *MEMORIES[name])
    try:
        frame_number, address, length = parse_dump(text)
    except ProbeError:
        raise ProbeError(
            f"{text!r} is not a dump such as main@300 or 2200000:400@300; memories: {', '.join(MEMORIES)}"
        ) from None
    return Dump(frame_number, f"dump-{address:08x}", ARM9, address, length)


def keypad(changes: dict[int, list[tuple[str, bool]]]) -> dict[int, int]:
    """The buttons held from each frame where they change, as the library's bits."""
    held: set[str] = set()
    masks = {}
    for frame in sorted(changes):
        for name, pressed in changes[frame]:
            (held.add if pressed else held.discard)(name)
        masks[frame] = sum(1 << BUTTONS.index(name) for name in held)
    return masks


def prepare(plan: Plan) -> Schedule:
    """Check a plan and work out what to do at each frame. Raises ``ProbeError`` for what makes no sense."""
    watches = [("reads", text, *parse_range(text)) for text in plan.reads]
    watches += [("writes", text, *parse_range(text)) for text in plan.writes]
    schedule = Schedule(
        keypad=keypad(button_changes(plan.input, plan.frames, BUTTONS)),
        touches=[parse_touch(text) for text in plan.touch],
        dumps=[parse_ds_dump(text) for text in plan.dumps],
        watches=watches,
        exec_range=parse_range(plan.exec_range) if plan.exec_range else None,
        save=parse_save(plan.save) if plan.save else None,
    )
    # Something asked for at a frame the run never reaches would silently not be there.
    asked = [("a screenshot", frame) for frame in plan.shots]
    asked += [("the registers", frame) for frame in plan.registers]
    asked += [(f"a dump of {dump.name}", dump.frame) for dump in schedule.dumps]
    if schedule.save:
        asked.append((f"the state {schedule.save[0]}", schedule.save[1]))
    for what, frame in asked:
        if frame > plan.frames:
            raise ProbeError(f"{what} is asked for at frame {frame}, after the run ends at frame {plan.frames}")
    return schedule


def write_png(path: Path, rgb: bytes, width: int, height: int) -> None:
    """Write 8-bit RGB pixels, a row at a time from the top, as a PNG file."""
    rows = b"".join(b"\x00" + rgb[y * width * 3 : (y + 1) * width * 3] for y in range(height))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b"")
    )


# What a watch keeps for each kind of access it sees: by (processor, kind of access, address of the
# instruction or None for a DMA, size in bytes), a list [accesses, lowest byte, highest byte, first
# frame, last frame, value written the first time].
Seen = dict[tuple[int, int, int | None, int], list[int]]


def report(kind: str, watches: list[tuple[str, Seen]]) -> str:
    """What the watches of one kind saw: a line for each instruction that touched each range, by the
    frame it first did."""
    value = "value written the first time" if kind == "writes" else ""
    lines = [f"# range, processor, access, instruction, size, accesses, span, frames{', ' if value else ''}{value}"]
    for text, seen in watches:
        for (cpu, access, pc, size), (count, low, high, first, last, written) in sorted(
            seen.items(), key=lambda item: (item[1][3], item[0][0], item[0][2] or 0)
        ):
            where = f"{pc:08x}" if pc is not None else "--------"
            line = (
                f"{text} {CPU_NAMES[cpu]} {ACCESSES.get(access, str(access)):<4} {where} size={size} n={count}"
                f" span={low:08x}-{high:08x} frames={first}-{last}"
            )
            lines.append(line + (f" value={written:0{size * 2}x}" if kind == "writes" else ""))
    return "\n".join(lines) + "\n"


def read_accesses(path: Path) -> list[tuple[str, str, str, int | None, int, int]]:
    """Parse ``PREFIX-reads.txt`` or ``-writes.txt``: (range, processor, access, instruction, size,
    accesses) per line."""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        text, cpu, access, where, size, count, *_ = line.split()
        pc = None if where.startswith("-") else int(where, 16)
        rows.append((text, cpu, access, pc, int(size.removeprefix("size=")), int(count.removeprefix("n="))))
    return rows


def exec_report(executed: dict[tuple[int, int], list[int]]) -> str:
    """The instructions that ran: by (processor, address), [times, the instruction, its size]."""
    lines = ["# processor, address, size, instruction, times run"]
    for (cpu, address), (count, instruction, size) in sorted(executed.items()):
        lines.append(f"{CPU_NAMES[cpu]} {address:08x} {size} {instruction:0{size * 2}x} n={count}")
    return "\n".join(lines) + "\n"


def read_executed(path: Path) -> list[tuple[str, int, int, int, int]]:
    """Parse ``PREFIX-exec.txt``: (processor, address, size, instruction, times run) per instruction.
    A size of 4 is an ARM instruction, 2 a Thumb one."""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        cpu, address, size, instruction, count = line.split()
        rows.append((cpu, int(address, 16), int(size), int(instruction, 16), int(count.removeprefix("n="))))
    return rows


def find_desmume() -> Path:
    """The patched DeSmuME library, from the DESMUME environment variable."""
    value = os.environ.get(DESMUME_VARIABLE)
    if not value:
        raise DesmumeError(
            f"set the {DESMUME_VARIABLE} environment variable to the path of DeSmuME's interface library,"
            " built with tools/desmume/interface-hooks.patch"
        )
    library = Path(value).expanduser()
    if not library.is_file():
        raise DesmumeError(f"{DESMUME_VARIABLE} points to {library}, which is not a file")
    data = library.read_bytes()
    missing = [name.decode() for name in PATCHED_FUNCTIONS if name not in data]
    if missing:
        raise DesmumeError(
            f"{library} is not DeSmuME's interface library built with tools/desmume/interface-hooks.patch:"
            f" it lacks {', '.join(missing)}"
        )
    return library


def command() -> list[str]:
    """The command that carries out one run, in a process of its own. It reads what to do as JSON."""
    return [sys.executable, "-m", "ventris_workbench.dsprobe"]


def request(library: Path, rom: Path, prefix: Path, plan: Plan) -> str:
    """What the run's process is given on its input."""
    return json.dumps(
        {"library": str(library), "rom": str(rom.resolve()), "prefix": str(prefix), "plan": plan.to_json()}
    )


# The names a run gives what it writes, after its prefix and a hyphen: nothing else is removed before a run.
_WRITTEN = re.compile(
    r"(shot-\d+\.png|[a-z0-9]+-\d+\.bin|dump-[0-9a-f]+-\d+\.bin|registers-\d+\.txt|reads\.txt|writes\.txt|exec\.txt)"
)


def _clear(prefix: Path, keep: list[Path]) -> None:
    """Remove what an earlier run with the same prefix wrote: its screenshots, dumps and reports, and its
    battery save. Only files named as a run names them go, and never one of ``keep``, the files this run
    is given or asked to write, so a state kept beside the outputs, or in a folder whose name starts with
    the prefix, is safe."""
    kept = {path.resolve() for path in keep}
    for old in prefix.parent.glob(prefix.name + "-*"):
        if old.is_file() and old.resolve() not in kept and _WRITTEN.fullmatch(old.name[len(prefix.name) + 1 :]):
            old.unlink()
    battery = prefix.parent / (prefix.name + "-battery")
    if battery.is_dir():
        for old in battery.iterdir():
            if old.is_file() and old.resolve() not in kept:
                old.unlink()


def run(rom: Path, output: Path, plan: Plan, timeout: int = 600) -> list[Path]:
    """Carry out ``plan`` on ``rom`` and return the files written: those named ``output``-something,
    sorted, then the state saved, if one was asked for. The battery save is kept in the folder
    ``output-battery``, emptied first, so a run never starts from another's."""
    library = find_desmume()
    schedule = prepare(plan)  # a mistake in the plan is reported before anything starts
    if not rom.is_file():
        raise DesmumeError(f"{rom} is not a file")
    if plan.state and not plan.state.is_file():
        raise DesmumeError(f"the state {plan.state} is not a file")
    prefix = output.resolve()
    prefix.parent.mkdir(parents=True, exist_ok=True)
    keep = [rom] + ([plan.state] if plan.state else []) + ([Path(schedule.save[0])] if schedule.save else [])
    _clear(prefix, keep)
    try:
        result = subprocess.run(
            command(),
            input=request(library, rom, prefix, plan),
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        raise DesmumeError(f"the emulator did not finish within {timeout} seconds") from None
    if result.returncode != 0:
        errors = [line.removeprefix("error: ") for line in result.stderr.splitlines() if line.startswith("error: ")]
        code = result.returncode if result.returncode >= 0 else result.returncode & 0xFFFFFFFF
        detail = "; ".join(errors[-3:]) or f"it stopped with exit code {code:#x}, with no error message"
        raise DesmumeError(f"the emulator run failed: {detail}")
    written = prefix.parent.glob(prefix.name + "-*")
    files = sorted(path for path in written if path.is_file() and _WRITTEN.fullmatch(path.name[len(prefix.name) + 1 :]))
    state = Path(schedule.save[0]).resolve() if schedule.save else None
    if state and state.is_file() and state not in files:
        files.append(state)
    return files


# What follows runs in the run's own process.

_HOOK = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_uint, ctypes.c_int)


def _signed(address: int) -> int:
    """An address as the library's ``int`` parameters take it."""
    return address - (1 << 32) if address >= 1 << 31 else address


def _load(library: Path) -> ctypes.CDLL:
    if hasattr(os, "add_dll_directory"):
        os.add_dll_directory(str(library.parent))  # SDL2, which the library needs, is beside it
    lib = ctypes.CDLL(str(library))
    lib.desmume_open.argtypes = [ctypes.c_char_p]
    lib.desmume_set_battery_dir.argtypes = [ctypes.c_char_p]
    lib.desmume_savestate_save.argtypes = [ctypes.c_char_p]
    lib.desmume_savestate_load.argtypes = [ctypes.c_char_p]
    lib.desmume_memory_read_block.argtypes = [ctypes.c_int, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p]
    lib.desmume_memory_read_register.argtypes = [ctypes.c_char_p]
    lib.desmume_memory_read_register.restype = ctypes.c_uint32
    lib.desmume_memory_hook_pc.restype = ctypes.c_uint32
    lib.desmume_memory_hook_value.restype = ctypes.c_uint32
    lib.desmume_input_keypad_update.argtypes = [ctypes.c_uint16]
    lib.desmume_input_set_touch_pos.argtypes = [ctypes.c_uint16, ctypes.c_uint16]
    for name in ("register_read", "register_write", "register_exec"):
        getattr(lib, "desmume_memory_" + name).argtypes = [ctypes.c_int, ctypes.c_int, _HOOK]
    return lib


def _carry_out(library: Path, rom: Path, prefix: Path, plan: Plan) -> None:
    schedule = prepare(plan)
    lib = _load(library)
    if lib.desmume_init() != 0:
        raise ProbeError("the emulator could not start")
    lib.desmume_set_language(LANGUAGE)
    battery = Path(f"{prefix}-battery")
    battery.mkdir(parents=True, exist_ok=True)
    lib.desmume_set_battery_dir(str(battery).encode())
    if lib.desmume_open(str(rom).encode()) < 0:
        raise ProbeError(f"the emulator could not open {rom}")
    lib.desmume_volume_set(0)
    lib.desmume_resume()
    if plan.state and not lib.desmume_savestate_load(str(plan.state).encode()):
        raise ProbeError(f"the emulator could not load the state {plan.state}")

    now = 0
    callbacks = []  # the library holds bare pointers to these, so they must outlive the run

    def watch(seen: Seen):
        def on_access(address: int, size: int) -> int:
            access = lib.desmume_memory_hook_access()
            key = (lib.desmume_memory_hook_cpu(), access, None if access == DMA else lib.desmume_memory_hook_pc(), size)
            entry = seen.get(key)
            if entry is None:
                seen[key] = [1, address, address + size - 1, now + 1, now + 1, lib.desmume_memory_hook_value()]
            else:
                entry[0] += 1
                entry[1] = min(entry[1], address)
                entry[2] = max(entry[2], address + size - 1)
                entry[4] = now + 1
            return 0

        callback = _HOOK(on_access)
        callbacks.append(callback)
        return callback

    executed: dict[tuple[int, int], list[int]] = {}

    def on_exec(address: int, size: int) -> int:
        key = (lib.desmume_memory_hook_cpu(), address)
        entry = executed.get(key)
        if entry is None:
            executed[key] = [1, lib.desmume_memory_hook_value(), size]
        else:
            entry[0] += 1
        return 0

    seen_by_kind: dict[str, list[tuple[str, Seen]]] = {"reads": [], "writes": []}
    set_watch = {"reads": lib.desmume_memory_register_read, "writes": lib.desmume_memory_register_write}
    screen = ctypes.create_string_buffer(WIDTH * HEIGHT * 3)
    mask = 0
    # The watches record from trace_from on: they are set just before that frame runs.
    watch_at = max(plan.trace_from - 1, 0)
    started = time.monotonic()
    while True:
        if now == watch_at:
            for kind, text, first, last in schedule.watches:
                seen: Seen = {}
                seen_by_kind[kind].append((text, seen))
                set_watch[kind](_signed(first), last - first + 1, watch(seen))
            if schedule.exec_range:
                first, last = schedule.exec_range
                callback = _HOOK(on_exec)
                callbacks.append(callback)
                lib.desmume_memory_register_exec(_signed(first), last - first + 1, callback)
        if now in plan.shots:
            lib.desmume_screenshot(screen)
            write_png(Path(f"{prefix}-shot-{now}.png"), screen.raw, WIDTH, HEIGHT)
        for dump in schedule.dumps:
            if dump.frame == now:
                buffer = ctypes.create_string_buffer(dump.length)
                lib.desmume_memory_read_block(dump.cpu, dump.address, dump.length, buffer)
                Path(f"{prefix}-{dump.name}-{now}.bin").write_bytes(buffer.raw)
        if now in plan.registers:
            lines = []
            for cpu in CPU_NAMES.values():
                values = (lib.desmume_memory_read_register(f"{cpu}.{name}".encode()) for name in REGISTERS)
                lines.append(
                    cpu + " " + " ".join(f"{name}={value:08x}" for name, value in zip(REGISTERS, values, strict=True))
                )
            Path(f"{prefix}-registers-{now}.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
        if schedule.save and schedule.save[1] == now:
            state = Path(schedule.save[0]).resolve()
            state.parent.mkdir(parents=True, exist_ok=True)
            if not lib.desmume_savestate_save(str(state).encode()):
                raise ProbeError(f"the emulator could not save the state {state}")
        if now >= plan.frames:
            break
        mask = schedule.keypad.get(now, mask)
        lib.desmume_input_keypad_update(mask)
        touching = [touch for touch in schedule.touches if touch.frame <= now < touch.frame + touch.frames]
        if touching:
            lib.desmume_input_set_touch_pos(touching[-1].x, touching[-1].y)
        else:
            lib.desmume_input_release_touch()
        lib.desmume_cycle(0)
        now += 1
    elapsed = time.monotonic() - started

    for kind, watches in seen_by_kind.items():
        if watches:
            Path(f"{prefix}-{kind}.txt").write_text(report(kind, watches), encoding="utf-8")
    if schedule.exec_range:
        Path(f"{prefix}-exec.txt").write_text(exec_report(executed), encoding="utf-8")
    print(f"{plan.frames} frames in {elapsed:.1f} seconds", file=sys.stderr)
    lib.desmume_free()


def _main() -> int:
    task = json.loads(sys.stdin.read())
    try:
        _carry_out(Path(task["library"]), Path(task["rom"]), Path(task["prefix"]), Plan.from_json(task["plan"]))
    except (ProbeError, OSError, AttributeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(_main())
