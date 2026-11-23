import socket
import struct
import threading

import pytest

from ventris_workbench.pcsx2 import IOP, MAIN, PAUSED, READ, RUNNING, WRITE, Hit, Pcsx2, Pcsx2Error


class FakeEmulator:
    """Answers the control protocol like a patched PCSX2 with a tiny memory."""

    def __init__(self):
        self.memory = bytearray(range(256)) * 16
        self.frame = 100
        self.paused = False
        self.pad: dict[int, int] = {}
        self.shots: list[str] = []
        self.fast = False
        self.states: list[tuple[str, str]] = []
        self.watches: list[tuple] = []
        self.traces: list[tuple] = []
        # Hits waiting to be collected: kind, processor, delay slot, pc, address, frame, registers.
        self.waiting: list[tuple] = []
        self.server = socket.create_server(("127.0.0.1", 0))
        self.address = f"127.0.0.1:{self.server.getsockname()[1]}"
        threading.Thread(target=self._serve, daemon=True).start()

    def _serve(self):
        connection, _ = self.server.accept()
        with connection:
            while True:
                head = connection.recv(4)
                if len(head) < 4:
                    return
                message = connection.recv(struct.unpack("<I", head)[0] - 4)
                reply = self._answer(message[0], message[1:])
                body = b"\xff" if reply is None else b"\x00" + reply
                connection.sendall(struct.pack("<I", len(body) + 4) + body)

    def _answer(self, command, arguments):
        if command == 0x08:
            return struct.pack("<I", 6) + b"v2.9\x00\x00"
        if command == 0x0F:
            return struct.pack("<I", PAUSED if self.paused else RUNNING)
        if command == 0x20:
            self.paused = bool(arguments[0])
            return b""
        if command == 0x21:
            self.frame += struct.unpack("<I", arguments)[0]
            self.paused = True
            return b""
        if command == 0x22:
            self.shots.append(arguments[4:].decode())
            return b""
        if command == 0x23:
            self.pad[arguments[2]] = arguments[3]
            return b""
        if command == 0x24:
            address, length = struct.unpack("<II", arguments)
            return bytes(self.memory[address : address + length])
        if command == 0x25:
            return struct.pack("<I", self.frame)
        if command == 0x26:
            address, length = struct.unpack("<II", arguments[:8])
            self.memory[address : address + length] = arguments[8:]
            return b""
        if command == 0x27:
            self.watches.append(struct.unpack("<BBII", arguments))
            return b""
        if command == 0x28:
            self.traces.append(struct.unpack("<BI", arguments))
            return b""
        if command == 0x29:
            # Two at a time, as the emulator gives them in batches.
            batch, self.waiting = self.waiting[:2], self.waiting[2:]
            body = struct.pack("<I", len(batch))
            for kind, cpu, slot, pc, address, frame, registers in batch:
                body += struct.pack("<BBBxIII32I", kind, cpu, slot, pc, address, frame, *registers)
            return body
        if command == 0x2A:
            self.watches, self.traces, self.waiting = [], [], []
            return b""
        if command == 0x2B:
            self.fast = bool(arguments[0])
            return b""
        if command in (0x2C, 0x2D):
            path = arguments[4:].decode()
            # A state that cannot be saved or loaded is refused.
            if "missing" in path:
                return None
            self.states.append(("save" if command == 0x2C else "load", path))
            return b""
        return None


def test_client_speaks_the_protocol():
    fake = FakeEmulator()
    with Pcsx2(fake.address) as emulator:
        assert emulator.version() == "v2.9"
        assert emulator.status() == RUNNING
        emulator.pause()
        assert emulator.status() == PAUSED and fake.paused
        assert emulator.advance(60) == 160 and emulator.frame() == 160
        emulator.press("start", frames=6, after=30)
        assert fake.pad == {9: 0} and emulator.frame() == 196
        emulator.set_button("circle", True)
        assert fake.pad[5] == 255
        emulator.screenshot(r"C:\shots\a.png")
        assert fake.shots == [r"C:\shots\a.png"]
        assert emulator.read(0x10, 4) == bytes([0x10, 0x11, 0x12, 0x13])
        emulator.write(0x10, b"text")
        assert emulator.read(0x0F, 6) == b"\x0ftext\x14"
        with pytest.raises(Pcsx2Error, match="unknown button"):
            emulator.set_button("turbo", True)
        with pytest.raises(Pcsx2Error, match="refused command 0x09"):
            emulator.save_state(1)


def test_an_unreachable_emulator_is_reported(monkeypatch):
    unused = socket.create_server(("127.0.0.1", 0))
    port = unused.getsockname()[1]
    unused.close()
    monkeypatch.setenv("PCSX2_PINE", f"127.0.0.1:{port}")
    with pytest.raises(Pcsx2Error, match="cannot reach PCSX2"):
        Pcsx2()


def test_watches_and_traces_are_set_and_their_hits_collected():
    fake = FakeEmulator()
    with Pcsx2(fake.address) as emulator:
        emulator.watch(0x00345000, 0x00345010, READ)
        emulator.watch(0x1000, 0x1004, cpu=IOP)
        emulator.trace(0x00123450)
        assert fake.watches == [(MAIN, READ, 0x00345000, 0x00345010), (IOP, READ | WRITE, 0x1000, 0x1004)]
        assert fake.traces == [(MAIN, 0x00123450)]
        registers = tuple(range(0x100, 0x120))
        fake.waiting = [
            (1, MAIN, 0, 0x00200000, 0x00345000, 500, registers),
            (1, MAIN, 1, 0x00200010, 0x00345000, 501, registers),
            (0, MAIN, 0, 0x00123450, 0x00123450, 502, registers),
        ]
        hits = emulator.hits()
        assert hits[0] == Hit(True, MAIN, 0x00200000, 0x00345000, False, 500, registers)
        # In a delay slot the instruction is the one after the branch the processor was at.
        assert (hits[1].pc, hits[1].instruction) == (0x00200010, 0x00200014)
        assert (hits[2].memory, hits[2].instruction, hits[2].registers[4]) == (False, 0x00123450, 0x104)
        # Each says in which of the emulator's frames it happened, whenever it is collected.
        assert [hit.frame for hit in hits] == [500, 501, 502]
        # Collected once: the batches are asked for until none is left, and then there are none.
        assert len(hits) == 3 and emulator.hits() == []
        emulator.clear_watches()
        assert fake.watches == [] and fake.traces == []


def test_speed_and_states_by_file():
    fake = FakeEmulator()
    with Pcsx2(fake.address) as emulator:
        emulator.set_speed(fast=True)
        assert fake.fast
        emulator.set_speed(fast=False)
        assert not fake.fast
        emulator.save_state_file(r"C:\states\scene.p2s")
        emulator.load_state_file(r"C:\states\scene.p2s")
        assert fake.states == [("save", r"C:\states\scene.p2s"), ("load", r"C:\states\scene.p2s")]
        with pytest.raises(Pcsx2Error):
            emulator.load_state_file(r"C:\states\missing.p2s")
