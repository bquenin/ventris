"""Run a game in the Mesen 2 emulator, headless, and record what it does.

This is how a game's text routines are found: play to a point, watch which code reads a piece of data or
which instructions run, take screenshots, dump memory. ``probe.lua`` does the work inside the emulator;
this module starts it and reads back what it wrote.

Mesen is found through the ``MESEN`` environment variable, which should name the ``Mesen`` binary.
"""

import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from ventris_workbench.platforms import snes, w65816

MESEN_VARIABLE = "MESEN"
TRIGGER_NEVER_FIRED = 4


class EmulatorError(Exception):
    """The emulator is missing, not set up, or the run failed."""


@dataclass
class Probe:
    """What to do during a run. Frame numbers count from the trigger when there is one, else from the start."""

    frames: int = 600  # when to stop
    input: str = ""  # input program before the trigger: steps "FRAME:KIND:BUTTONS" (mash, press, hold, none)
    trigger: str = ""  # "ADDRESS@FRAME": restart the clock when the CPU executes ADDRESS at or after FRAME
    trigger_timeout: int = 30000  # frames to wait for the trigger before giving up
    input_after: str = ""  # input program once the trigger has fired
    shots: list[int] = field(default_factory=list)  # frames at which to take a screenshot
    dumps: list[str] = field(default_factory=list)  # "MEMORY@FRAME"
    registers: list[int] = field(default_factory=list)  # frames at which to save the emulator's registers
    reads: list[str] = field(default_factory=list)  # "MEMORY:START-END": record which code reads the range
    writes: list[str] = field(default_factory=list)  # the same for writes
    exec_range: str = ""  # "START-END": record the instructions executed in this range of CPU addresses
    log: list[str] = field(default_factory=list)  # "ADDRESS" or "ADDRESS:MEMORY:START-END": a line per execution
    trace_from: int = 0  # frame from which reads, executed instructions and log lines are recorded
    state: Path | None = None  # saved state to start from
    save: str = ""  # "FILE@FRAME"

    def environment(self, prefix: Path) -> dict[str, str]:
        return {
            "PROBE_OUT": str(prefix),
            "PROBE_FRAMES": str(self.frames),
            "PROBE_INPUT": self.input,
            "PROBE_TRIGGER": self.trigger,
            "PROBE_TRIGGER_TIMEOUT": str(self.trigger_timeout),
            "PROBE_INPUT_AFTER": self.input_after,
            "PROBE_SHOTS": " ".join(str(frame) for frame in self.shots),
            "PROBE_DUMP": " ".join(self.dumps),
            "PROBE_REGISTERS": " ".join(str(frame) for frame in self.registers),
            "PROBE_READS": " ".join(self.reads),
            "PROBE_WRITES": " ".join(self.writes),
            "PROBE_EXEC": self.exec_range,
            "PROBE_LOG": " ".join(self.log),
            "PROBE_TRACE_FROM": str(self.trace_from),
            "PROBE_STATE": str(self.state.resolve()) if self.state else "",
            "PROBE_SAVE": self.save,
        }


def find_mesen() -> Path:
    """The Mesen binary, from the MESEN environment variable."""
    value = os.environ.get(MESEN_VARIABLE)
    if not value:
        raise EmulatorError(f"set the {MESEN_VARIABLE} environment variable to the path of the Mesen binary")
    binary = Path(value).expanduser()
    if not binary.is_file():
        raise EmulatorError(f"{MESEN_VARIABLE} points to {binary}, which is not a file")
    # Without a settings file Mesen opens its first-run wizard, in a window, and never reaches the script.
    if not (binary.parent / "settings.json").exists():
        raise EmulatorError(
            f"Mesen has not been set up yet: there is no settings.json beside {binary}. Run it once and, in its"
            " setup wizard, choose to store its data in the same folder as the application"
        )
    return binary


def run_probe(rom: Path, prefix: Path, probe: Probe, timeout: int = 300) -> list[Path]:
    """Run ``probe`` on ``rom``. Everything written starts with ``prefix``; returns the files, sorted."""
    mesen = find_mesen()
    prefix = prefix.resolve()
    prefix.parent.mkdir(parents=True, exist_ok=True)
    for old in prefix.parent.glob(prefix.name + "-*"):
        old.unlink()
    command = [
        str(mesen),
        "--testRunner",
        str(Path(__file__).with_name("probe.lua")),
        str(rom.resolve()),
        "--enableStdout",
        "--doNotSaveSettings",
        f"--timeout={timeout}",
        "--debug.scriptWindow.allowIoOsAccess=true",
    ]
    try:
        result = subprocess.run(
            command,
            cwd=mesen.parent,
            env={**os.environ, **probe.environment(prefix)},
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout + 30,
            check=False,
        )
    except subprocess.TimeoutExpired:
        raise EmulatorError(f"the emulator did not finish within {timeout} seconds") from None
    if result.returncode == TRIGGER_NEVER_FIRED:
        raise EmulatorError(f"the trigger {probe.trigger} never fired")
    if result.returncode != 0:
        errors = [line for line in result.stdout.splitlines() if "rror" in line or "attempt to" in line]
        detail = "; ".join(errors[-3:]) or "no error message"
        raise EmulatorError(f"the emulator run failed with exit code {result.returncode}: {detail}")
    return sorted(prefix.parent.glob(prefix.name + "-*"))


def read_executed(trace: Path) -> list[tuple[int, int, int]]:
    """Parse ``PREFIX-exec.txt``: (address, processor flags, times executed) per instruction."""
    rows = []
    for line in trace.read_text().splitlines():
        parts = line.split()
        rows.append((int(parts[0], 16), int(parts[1], 16), int(parts[-1].removeprefix("n="))))
    return rows


def listing(rom: bytes, mapping: str, executed: list[tuple[int, int, int]]) -> str:
    """Disassemble the instructions that ran, using the flags the emulator saw at each one.

    Unlike a static disassembly this never loses track of instruction boundaries or register widths, but
    it shows only what actually ran. A blank line marks a gap between runs of instructions.
    """
    lines: list[str] = []
    expected: int | None = None
    for address, flags, count in executed:
        offset = snes.snes_to_pc(address, mapping)
        instruction = w65816.decode(rom, offset, address, bool(flags & w65816.FLAG_M), bool(flags & w65816.FLAG_X))
        if expected is not None and address != expected:
            lines.append("")
        lines.append(f"{address:06X}  {instruction.data.hex(' '):<12} {instruction!s:<22} ; x{count}")
        expected = address + instruction.size
    return "\n".join(lines) + "\n"
