"""Drive the PCSX2 emulator (PlayStation 2) from a script.

PCSX2 has a control socket, PINE, which reads and writes memory and saves states. A build with the
local additions in ``tools/pcsx2/pine-automation.patch`` also pauses, advances frame by frame, takes
screenshots, presses buttons and reads memory in bulk; that build is what this module talks to.

The same build answers the two questions that find a game's code by what it does: which code reads
or writes this memory (``watch``), and what the registers hold each time this address is run
(``trace``). The game runs on while they are recorded, and ``hits`` collects what was.

Where the emulator listens: a TCP port on Windows (28011, this machine only), and elsewhere a Unix
socket, ``pcsx2.sock`` in ``$TMPDIR`` on macOS and in ``$XDG_RUNTIME_DIR`` (or ``/tmp``) on Linux. This
module connects to ``127.0.0.1:28011`` unless ``PCSX2_PINE`` says otherwise, so on macOS and Linux set
it to the socket's path. ``PCSX2_PINE`` names either, as ``host:port`` or a socket path; an emulator on
another machine is reached by forwarding its port, for example ``ssh -N -L 28011:127.0.0.1:28011 machine``.
Paths given to the emulator, such as a screenshot's, are paths on the machine it runs on.
"""

import os
import socket
import struct
import time
from dataclasses import dataclass
from pathlib import Path

PINE_VARIABLE = "PCSX2_PINE"
DEFAULT_PORT = 28011
RAM_SIZE = 0x2000000  # the console's 32 MiB of main memory, at address 0
BLOCK = 0x60000  # the most memory one request may carry

# Commands. Those from 0x20 up are the local additions.
_VERSION, _SAVE_STATE, _LOAD_STATE, _TITLE, _SERIAL, _STATUS = 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0F
_PAUSE, _ADVANCE, _SCREENSHOT, _PAD, _READ, _FRAME, _WRITE = 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26
_WATCH, _TRACE, _HITS, _CLEAR_WATCHES = 0x27, 0x28, 0x29, 0x2A
_SPEED, _SAVE_STATE_FILE, _LOAD_STATE_FILE = 0x2B, 0x2C, 0x2D

# The console's two processors, as the emulator numbers them: the main one, which runs the game,
# and the one that handles input and output.
MAIN, IOP = 1, 2
# What a watch records. They add up: READ | WRITE is every access.
READ, WRITE, CHANGE = 1, 2, 4
# A hit as the emulator sends it: kind, processor, delay slot, a spare byte, pc, address, frame, 32 registers.
_HIT = struct.Struct("<BBBxIII32I")

RUNNING, PAUSED, SHUTDOWN = 0, 1, 2

# Controller inputs, in the order the emulator numbers them.
BUTTONS = (
    "up", "right", "down", "left", "triangle", "circle", "cross", "square",
    "select", "start", "l1", "l2", "r1", "r2", "l3", "r3",
)  # fmt: skip


class Pcsx2Error(Exception):
    """The emulator cannot be reached, or it refused a command."""


@dataclass(frozen=True)
class Hit:
    """One time a traced address was run, or watched memory was read or written."""

    #: True for memory read or written, False for an address that was run.
    memory: bool
    #: MAIN or IOP.
    cpu: int
    #: Where the processor was. In a delay slot this is the branch, and the instruction is the next.
    pc: int
    #: The traced address, or the start of the watched range.
    address: int
    delay_slot: bool
    #: The emulator's count of frames when it happened, as ``Pcsx2.frame`` gives it.
    frame: int
    #: The low 32 bits of the 32 general registers, by number.
    registers: tuple[int, ...]

    @property
    def instruction(self) -> int:
        """The address of the instruction that was run or that touched the memory."""
        return self.pc + 4 if self.delay_slot else self.pc


def _connect(address: str | None) -> socket.socket:
    address = address or os.environ.get(PINE_VARIABLE) or f"127.0.0.1:{DEFAULT_PORT}"
    try:
        host, _, port = address.rpartition(":")
        if host and port.isdigit():
            return socket.create_connection((host, int(port)), timeout=10)
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.settimeout(10)
        connection.connect(address)
        return connection
    except OSError as error:
        raise Pcsx2Error(f"cannot reach PCSX2 at {address}: {error}") from None


class Pcsx2:
    """A connection to a running emulator. Use it in a ``with`` block, or call ``close``."""

    def __init__(self, address: str | None = None):
        self._socket = _connect(address)

    def __enter__(self) -> "Pcsx2":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        self._socket.close()

    def _receive(self, count: int) -> bytes:
        data = bytearray()
        while len(data) < count:
            chunk = self._socket.recv(count - len(data))
            if not chunk:
                raise Pcsx2Error("PCSX2 closed the connection")
            data += chunk
        return bytes(data)

    def _call(self, command: int, arguments: bytes = b"") -> bytes:
        """Send one command and return what follows the status byte of the reply."""
        message = bytes([command]) + arguments
        try:
            self._socket.sendall(struct.pack("<I", len(message) + 4) + message)
            (size,) = struct.unpack("<I", self._receive(4))
            reply = self._receive(size - 4)
        except OSError as error:
            raise Pcsx2Error(f"lost the connection to PCSX2: {error}") from None
        if not reply or reply[0] != 0:
            raise Pcsx2Error(f"PCSX2 refused command 0x{command:02X}; no game may be running, or this build lacks it")
        return reply[1:]

    def _text(self, command: int) -> str:
        reply = self._call(command)
        (size,) = struct.unpack("<I", reply[:4])
        return reply[4 : 4 + size].rstrip(b"\0").decode("utf-8", "replace")

    # --- what is running

    def version(self) -> str:
        return self._text(_VERSION)

    def title(self) -> str:
        return self._text(_TITLE)

    def serial(self) -> str:
        return self._text(_SERIAL)

    def status(self) -> int:
        """RUNNING, PAUSED or SHUTDOWN."""
        return struct.unpack("<I", self._call(_STATUS))[0]

    def frame(self) -> int:
        """Frames emulated since the game started."""
        return struct.unpack("<I", self._call(_FRAME))[0]

    # --- time

    def pause(self) -> None:
        self._call(_PAUSE, b"\x01")

    def resume(self) -> None:
        self._call(_PAUSE, b"\x00")

    def advance(self, frames: int, timeout: float = 60.0) -> int:
        """Run exactly ``frames`` frames, leave the game paused, and return the frame number reached."""
        target = self.frame() + frames
        self._call(_ADVANCE, struct.pack("<I", frames))
        deadline = time.monotonic() + timeout
        while self.frame() < target:
            if time.monotonic() > deadline:
                raise Pcsx2Error(f"the emulator did not reach frame {target} in {timeout:.0f} seconds")
            time.sleep(0.01)
        return target

    def save_state(self, slot: int) -> None:
        self._call(_SAVE_STATE, bytes([slot]))

    def load_state(self, slot: int) -> None:
        self._call(_LOAD_STATE, bytes([slot]))

    def save_state_file(self, path: str | Path) -> None:
        """Save a state to a file, at a path on the emulator's machine. The file is whole on return."""
        encoded = str(path).encode()
        self._call(_SAVE_STATE_FILE, struct.pack("<I", len(encoded)) + encoded)

    def load_state_file(self, path: str | Path) -> None:
        """Load a state from a file, at a path on the emulator's machine."""
        encoded = str(path).encode()
        self._call(_LOAD_STATE_FILE, struct.pack("<I", len(encoded)) + encoded)

    def set_speed(self, fast: bool) -> None:
        """Run as fast as the machine can, or at the game's own speed."""
        self._call(_SPEED, bytes([fast]))

    # --- the controller

    def set_button(self, button: str, pressed: bool, port: int = 0) -> None:
        if button not in BUTTONS:
            raise Pcsx2Error(f"unknown button {button!r}: one of {', '.join(BUTTONS)}")
        self._call(_PAD, bytes([port, 0, BUTTONS.index(button), 255 if pressed else 0]))

    def press(self, button: str, frames: int = 6, after: int = 0) -> None:
        """Hold a button for ``frames`` frames, release it, then run ``after`` more. Leaves the game paused."""
        self.set_button(button, True)
        self.advance(frames)
        self.set_button(button, False)
        if after:
            self.advance(after)

    # --- what the game shows and holds

    def screenshot(self, path: str | Path) -> None:
        """Ask for the next frame drawn to be saved as an image, at a path on the emulator's machine."""
        encoded = str(path).encode()
        self._call(_SCREENSHOT, struct.pack("<I", len(encoded)) + encoded)

    def read(self, address: int, length: int) -> bytes:
        data = bytearray()
        while len(data) < length:
            size = min(length - len(data), BLOCK)
            data += self._call(_READ, struct.pack("<II", address + len(data), size))
        return bytes(data)

    def write(self, address: int, data: bytes) -> None:
        for start in range(0, len(data), BLOCK):
            block = data[start : start + BLOCK]
            self._call(_WRITE, struct.pack("<II", address + start, len(block)) + block)

    def ram(self) -> bytes:
        """The whole of main memory."""
        return self.read(0, RAM_SIZE)

    # --- which code does it

    def watch(self, start: int, end: int, kind: int = READ | WRITE, cpu: int = MAIN) -> None:
        """Record every read or write of the bytes from ``start`` up to ``end``, and let the game run on."""
        self._call(_WATCH, struct.pack("<BBII", cpu, kind, start, end))

    def trace(self, address: int, cpu: int = MAIN) -> None:
        """Record the registers every time the instruction at ``address`` is run, and let the game run on."""
        self._call(_TRACE, struct.pack("<BI", cpu, address))

    def hits(self) -> list[Hit]:
        """What the watches and traces recorded since last asked, oldest first. It is forgotten once given."""
        found: list[Hit] = []
        while True:
            reply = self._call(_HITS)
            (count,) = struct.unpack("<I", reply[:4])
            for index in range(count):
                kind, cpu, slot, pc, address, frame, *registers = _HIT.unpack_from(reply, 4 + index * _HIT.size)
                found.append(Hit(kind == 1, cpu, pc, address, bool(slot), frame, tuple(registers)))
            if count == 0:
                return found

    def clear_watches(self) -> None:
        """Remove every watch and trace, and forget what they recorded."""
        self._call(_CLEAR_WATCHES)
