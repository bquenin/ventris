import pytest

from ventris_workbench.cli import main
from ventris_workbench.platforms import snes


def _make_rom(mapping: str, size: int, title: bytes = b"TEST GAME") -> bytes:
    offsets = {"LoROM": 0x7FC0, "HiROM": 0xFFC0, "ExHiROM": 0x40FFC0}
    modes = {"LoROM": 0x20, "HiROM": 0x21, "ExHiROM": 0x35}
    rom = bytearray((i * 7 + (i >> 8)) & 0xFF for i in range(size))
    base = offsets[mapping]
    rom[base : base + 0x40] = bytes(0x40)
    rom[base : base + 0x15] = title.ljust(0x15, b" ")
    rom[base + 0x15] = modes[mapping]
    rom[base + 0x16] = 0x02
    rom[base + 0x17] = 0x0C
    rom[base + 0x18] = 0x03
    rom[base + 0x19] = 0x00
    rom[base + 0x3C : base + 0x3E] = (0x8000).to_bytes(2, "little")
    # The checksum covers its own fields, which always sum to 0x1FE once they are consistent.
    rom[base + 0x1C : base + 0x20] = b"\xff\xff\x00\x00"
    checksum = snes.compute_checksum(bytes(rom))
    rom[base + 0x1C : base + 0x1E] = (checksum ^ 0xFFFF).to_bytes(2, "little")
    rom[base + 0x1E : base + 0x20] = checksum.to_bytes(2, "little")
    return bytes(rom)


@pytest.mark.parametrize(
    ("mapping", "size"),
    [("LoROM", 0x80000), ("HiROM", 0x100000), ("ExHiROM", 0x600000)],
)
def test_detect_header(mapping, size):
    rom = _make_rom(mapping, size)
    header = snes.detect_header(rom)
    assert header is not None
    assert header.mapping == mapping
    assert header.title == "TEST GAME"
    assert header.region == "Japan"
    assert header.rom_size_kb == 4096
    assert header.ram_size_kb == 8
    assert header.checksum == snes.compute_checksum(rom)


def test_detect_header_rejects_noise():
    assert snes.detect_header(bytes(0x100000)) is None
    assert snes.detect_header(b"too small") is None


def test_copier_header():
    rom = _make_rom("LoROM", 0x80000)
    headered = bytes(512) + rom
    assert snes.has_copier_header(len(headered))
    assert not snes.has_copier_header(len(rom))
    assert snes.strip_copier_header(headered) == rom
    assert snes.strip_copier_header(rom) == rom


def test_checksum_mirrors_non_power_of_two():
    # 6 units: the trailing 2 are mirrored to fill 4, so they count twice.
    unit = 0x1000
    rom = b"\x01" * (4 * unit) + b"\x02" * (2 * unit)
    assert snes.compute_checksum(rom) == (4 * unit * 1 + 2 * (2 * unit * 2)) & 0xFFFF
    assert snes.compute_checksum(b"\x03" * 0x8000) == (3 * 0x8000) & 0xFFFF


@pytest.mark.parametrize(
    ("mapping", "offset", "address"),
    [
        ("LoROM", 0x000000, 0x808000),
        ("LoROM", 0x007FC0, 0x80FFC0),
        ("LoROM", 0x018123, 0x838123),
        ("HiROM", 0x00FFC0, 0xC0FFC0),
        ("HiROM", 0x3FFFFF, 0xFFFFFF),
        ("ExHiROM", 0x00FFC0, 0xC0FFC0),
        ("ExHiROM", 0x40FFC0, 0x40FFC0),
        ("ExHiROM", 0x5ABCDE, 0x5ABCDE),
        ("ExHiROM", 0x7E8000, 0x3E8000),
    ],
)
def test_address_round_trip(mapping, offset, address):
    assert snes.pc_to_snes(offset, mapping) == address
    assert snes.snes_to_pc(address, mapping) == offset


def test_address_mirrors_and_errors():
    assert snes.snes_to_pc(0x008000, "LoROM") == 0
    assert snes.snes_to_pc(0x00FFC0, "HiROM") == 0x00FFC0
    assert snes.snes_to_pc(0x00FFC0, "ExHiROM") == 0x40FFC0
    assert snes.snes_to_pc(0x80FFC0, "ExHiROM") == 0x00FFC0
    with pytest.raises(snes.AddressError):
        snes.snes_to_pc(0x7E0000, "HiROM")
    with pytest.raises(snes.AddressError):
        snes.snes_to_pc(0x001234, "LoROM")
    with pytest.raises(snes.AddressError):
        snes.pc_to_snes(0x7E0000, "ExHiROM")
    with pytest.raises(snes.AddressError):
        snes.pc_to_snes(0x400000, "HiROM")


def test_expand_and_update_checksum():
    rom = _make_rom("HiROM", 0x100000)
    grown = snes.expand(rom, 0x200000)
    assert len(grown) == 0x200000 and grown[: 0xFFC0 + 0x17] == rom[: 0xFFC0 + 0x17]
    assert set(grown[0x100000:]) == {0xFF}
    assert snes.detect_header(grown, min_score=0).rom_size_kb == 2048
    fixed = snes.update_checksum(grown)
    header = snes.detect_header(fixed)
    assert header.checksum == snes.compute_checksum(fixed)
    assert header.checksum + header.checksum_complement == 0xFFFF
    with pytest.raises(ValueError):
        snes.expand(rom, 0x80000)


def test_hirom_becomes_exhirom_with_bank_zero_copied():
    rom = bytearray(_make_rom("HiROM", 0x400000))
    rom[0x8000:0x8004] = b"BOOT"
    grown = snes.to_exhirom(bytes(rom))
    assert len(grown) == 0x800000 and grown[:0xFFC0] == bytes(rom[:0xFFC0])
    # Bank $00 now shows the second half of the file: the start-up code, header and vectors are there too.
    assert grown[0x408000:0x408004] == b"BOOT"
    assert grown[0x40FFC0:0x40FFD5] == grown[0xFFC0:0xFFD5]
    header = snes.detect_header(grown, min_score=0)
    assert (header.mapping, header.rom_size_kb, header.map_mode & 0x0F) == ("ExHiROM", 8192, 5)
    # Both copies of the header get the checksum.
    fixed = snes.update_checksum(grown)
    assert snes.detect_header(fixed).checksum == snes.compute_checksum(fixed)
    assert fixed[0x40FFDC:0x40FFE0] == fixed[0xFFDC:0xFFE0]
    with pytest.raises(ValueError):
        snes.to_exhirom(grown)


def test_bank_allocator_never_crosses_a_bank():
    space = snes.BankAllocator(0x60FF00, 0x620000)
    assert space.allocate(0x80) == 0x60FF00
    assert space.allocate(0x80) == 0x60FF80
    assert space.allocate(1) == 0x610000
    space = snes.BankAllocator(0x60FFF0, 0x620000)
    assert space.allocate(0x20) == 0x610000  # skips the 16 bytes left in the first bank
    assert space.allocate(0xFFE0) == 0x610020
    with pytest.raises(snes.AddressError, match="no room"):
        space.allocate(1)
    with pytest.raises(snes.AddressError, match="one bank"):
        snes.BankAllocator(0, 0x100000).allocate(0x10001)


def test_cli_end_to_end(tmp_path, capsys):
    original = _make_rom("HiROM", 0x100000)
    modified = bytearray(original)
    modified[0x1234:0x1240] = b"TRANSLATION!"
    modified += b"\xee" * 0x100
    rom_path, mod_path = tmp_path / "game.rom", tmp_path / "game-en.rom"
    patch_path, out_path = tmp_path / "game.patch", tmp_path / "out.rom"
    rom_path.write_bytes(bytes(512) + original)  # copier header on the original only
    mod_path.write_bytes(bytes(modified))

    assert main(["identify", str(rom_path)]) == 0
    out = capsys.readouterr().out
    assert "HiROM" in out and "copier header:  yes" in out and "(ok)" in out

    assert main(["diff", str(rom_path), str(mod_path)]) == 0
    out = capsys.readouterr().out
    assert "0x00001234" in out and "$c01234" in out

    plain_path = tmp_path / "plain.rom"
    plain_path.write_bytes(original)
    assert main(["make-patch", str(plain_path), str(mod_path), "-o", str(patch_path)]) == 0
    assert main(["apply", str(plain_path), str(patch_path), "-o", str(out_path)]) == 0
    assert out_path.read_bytes() == bytes(modified)

    assert main(["apply", str(rom_path), str(patch_path), "-o", str(out_path)]) == 1
    assert "different file" in capsys.readouterr().err

    # An IPS patch made against a headered ROM has every offset shifted by 512 bytes.
    ips_path = tmp_path / "headered.patch"
    ips_path.write_bytes(b"PATCH" + (0x1234 + 512).to_bytes(3, "big") + b"\x00\x02" + b"OK" + b"EOF")
    assert main(["apply", str(plain_path), str(ips_path), "-o", str(out_path), "--copier-header"]) == 0
    capsys.readouterr()
    patched = out_path.read_bytes()
    assert len(patched) == len(original)
    assert patched[0x1234:0x1236] == b"OK"

    # Disassemble code placed at a known CPU address: REP #$30, LDA #$1234, RTS.
    code = bytearray(original)
    code[0x2000:0x2006] = bytes.fromhex("C2 30 A9 34 12 60")
    code_path = tmp_path / "code.rom"
    code_path.write_bytes(bytes(code))
    assert main(["disasm", str(code_path), "C02000", "--count", "3"]) == 0
    listing = capsys.readouterr().out.splitlines()
    assert [line.split(None, 1)[0] for line in listing] == ["C02000", "C02002", "C02005"]
    assert listing[1].endswith("LDA #$1234") and listing[2].endswith("RTS")
    assert main(["disasm", str(code_path), "7E0000"]) == 1
    assert "work RAM" in capsys.readouterr().err

    assert main(["addr", "c0ffc0", "--mapping", "HiROM", "--to", "pc"]) == 0
    assert capsys.readouterr().out.strip().endswith("0x00ffc0")
