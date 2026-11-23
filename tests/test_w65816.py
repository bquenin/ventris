import pytest

from ventris_workbench.platforms import w65816
from ventris_workbench.platforms.w65816 import DisassemblyError, decode, disassemble


def _one(hex_bytes: str, address: int = 0xC08000, m8: bool = True, x8: bool = True) -> tuple[str, int, int | None]:
    instruction = decode(bytes.fromhex(hex_bytes), 0, address, m8, x8)
    return str(instruction), instruction.size, instruction.target


@pytest.mark.parametrize(
    ("hex_bytes", "text", "size"),
    [
        ("EA", "NOP", 1),
        ("0A", "ASL A", 1),
        ("C2 30", "REP #$30", 2),
        ("A5 12", "LDA $12", 2),
        ("B5 12", "LDA $12,X", 2),
        ("B6 12", "LDX $12,Y", 2),
        ("B2 12", "LDA ($12)", 2),
        ("A1 12", "LDA ($12,X)", 2),
        ("B1 12", "LDA ($12),Y", 2),
        ("A7 12", "LDA [$12]", 2),
        ("B7 12", "LDA [$12],Y", 2),
        ("A3 04", "LDA $04,S", 2),
        ("B3 04", "LDA ($04,S),Y", 2),
        ("AD 34 12", "LDA $1234", 3),
        ("BD 34 12", "LDA $1234,X", 3),
        ("B9 34 12", "LDA $1234,Y", 3),
        ("6C 34 12", "JMP ($1234)", 3),
        ("7C 34 12", "JMP ($1234,X)", 3),
        ("DC 34 12", "JML [$1234]", 3),
        ("AF 56 34 12", "LDA $123456", 4),
        ("BF 56 34 12", "LDA $123456,X", 4),
        ("8F 80 21 00", "STA $002180", 4),
        ("54 7E C8", "MVN $C8,$7E", 3),
        ("F4 34 12", "PEA $1234", 3),
    ],
)
def test_addressing_modes(hex_bytes, text, size):
    assert _one(hex_bytes)[:2] == (text, size)


def test_immediate_size_follows_the_flags():
    assert _one("A9 34 12", m8=True)[:2] == ("LDA #$34", 2)
    assert _one("A9 34 12", m8=False)[:2] == ("LDA #$1234", 3)
    assert _one("A2 34 12", x8=True)[:2] == ("LDX #$34", 2)
    assert _one("A2 34 12", x8=False)[:2] == ("LDX #$1234", 3)
    # The accumulator flag does not affect index instructions, and the reverse.
    assert _one("A0 34 12", m8=False, x8=True)[:2] == ("LDY #$34", 2)
    assert _one("C9 34 12", m8=True, x8=False)[:2] == ("CMP #$34", 2)


def test_branch_and_jump_targets():
    assert _one("D0 FE", address=0xC8E700) == ("BNE $E700", 2, 0xC8E700)
    assert _one("80 10", address=0xC8E700) == ("BRA $E712", 2, 0xC8E712)
    assert _one("F0 80", address=0xC80010) == ("BEQ $FF92", 2, 0xC8FF92)  # wraps inside the bank
    assert _one("82 00 10", address=0xC8E700) == ("BRL $F703", 3, 0xC8F703)
    assert _one("20 34 12", address=0xC8E700) == ("JSR $1234", 3, 0xC81234)
    assert _one("4C 34 12", address=0xC8E700) == ("JMP $1234", 3, 0xC81234)
    assert _one("22 56 34 C7", address=0xC8E700) == ("JSL $C73456", 4, 0xC73456)
    assert _one("5C 56 34 C7", address=0xC8E700) == ("JML $C73456", 4, 0xC73456)
    assert _one("AD 34 12")[2] is None


def test_every_opcode_decodes():
    for opcode in range(256):
        instruction = decode(bytes([opcode, 0x11, 0x22, 0x33]), 0, 0x808000, m8=False, x8=False)
        assert 1 <= instruction.size <= 4
        assert instruction.mnemonic.isupper() and len(instruction.mnemonic) == 3
    sizes = [decode(bytes([op, 0, 0, 0]), 0, 0).size for op in range(256)]
    assert sizes.count(4) == 18  # the long and long,X forms of eight operations, plus JSL and JML


def test_disassemble_follows_rep_and_sep():
    code = bytes.fromhex("C2 30 A9 34 12 A2 78 56 E2 20 A9 12 A2 78 56 E2 10 A2 01 60")
    listing = [str(i) for i in disassemble(code, 0, 0xC08000, 8)]
    assert listing == [
        "REP #$30",
        "LDA #$1234",
        "LDX #$5678",
        "SEP #$20",
        "LDA #$12",
        "LDX #$5678",
        "SEP #$10",
        "LDX #$01",
    ]
    last = list(disassemble(code, 0, 0xC08000, 9))[-1]
    assert (str(last), last.address) == ("RTS", 0xC08013)


def test_cut_off_instruction():
    with pytest.raises(DisassemblyError):
        decode(bytes.fromhex("AD 34"), 0, 0)
    with pytest.raises(DisassemblyError):
        decode(b"", 0, 0)
    assert w65816.FLAG_M == 0x20 and w65816.FLAG_X == 0x10
