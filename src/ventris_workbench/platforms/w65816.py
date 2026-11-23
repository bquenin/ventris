"""Disassembler for the WDC 65C816, the CPU of the SNES.

The size of an immediate operand depends on two processor flags that only the running program knows:
M selects an 8-bit or 16-bit accumulator, X the same for the index registers. A caller either passes the
flags it observed (from an emulator trace) or lets ``disassemble`` follow ``REP`` and ``SEP`` as it goes.
"""

from collections.abc import Iterator
from dataclasses import dataclass

FLAG_M = 0x20
FLAG_X = 0x10

_TABLE = """
BRK:imm8 ORA:idpx COP:imm8 ORA:sr   TSB:dp  ORA:dp  ASL:dp  ORA:ildp  PHP:imp ORA:immm ASL:acc PHD:imp TSB:abs   ORA:abs  ASL:abs  ORA:long
BPL:rel  ORA:idpy ORA:idp  ORA:isry TRB:dp  ORA:dpx ASL:dpx ORA:ildpy CLC:imp ORA:absy INC:acc TCS:imp TRB:abs   ORA:absx ASL:absx ORA:longx
JSR:abs  AND:idpx JSL:long AND:sr   BIT:dp  AND:dp  ROL:dp  AND:ildp  PLP:imp AND:immm ROL:acc PLD:imp BIT:abs   AND:abs  ROL:abs  AND:long
BMI:rel  AND:idpy AND:idp  AND:isry BIT:dpx AND:dpx ROL:dpx AND:ildpy SEC:imp AND:absy DEC:acc TSC:imp BIT:absx  AND:absx ROL:absx AND:longx
RTI:imp  EOR:idpx WDM:imm8 EOR:sr   MVP:mv  EOR:dp  LSR:dp  EOR:ildp  PHA:imp EOR:immm LSR:acc PHK:imp JMP:abs   EOR:abs  LSR:abs  EOR:long
BVC:rel  EOR:idpy EOR:idp  EOR:isry MVN:mv  EOR:dpx LSR:dpx EOR:ildpy CLI:imp EOR:absy PHY:imp TCD:imp JML:long  EOR:absx LSR:absx EOR:longx
RTS:imp  ADC:idpx PER:rell ADC:sr   STZ:dp  ADC:dp  ROR:dp  ADC:ildp  PLA:imp ADC:immm ROR:acc RTL:imp JMP:iabs  ADC:abs  ROR:abs  ADC:long
BVS:rel  ADC:idpy ADC:idp  ADC:isry STZ:dpx ADC:dpx ROR:dpx ADC:ildpy SEI:imp ADC:absy PLY:imp TDC:imp JMP:iabsx ADC:absx ROR:absx ADC:longx
BRA:rel  STA:idpx BRL:rell STA:sr   STY:dp  STA:dp  STX:dp  STA:ildp  DEY:imp BIT:immm TXA:imp PHB:imp STY:abs   STA:abs  STX:abs  STA:long
BCC:rel  STA:idpy STA:idp  STA:isry STY:dpx STA:dpx STX:dpy STA:ildpy TYA:imp STA:absy TXS:imp TXY:imp STZ:abs   STA:absx STZ:absx STA:longx
LDY:immx LDA:idpx LDX:immx LDA:sr   LDY:dp  LDA:dp  LDX:dp  LDA:ildp  TAY:imp LDA:immm TAX:imp PLB:imp LDY:abs   LDA:abs  LDX:abs  LDA:long
BCS:rel  LDA:idpy LDA:idp  LDA:isry LDY:dpx LDA:dpx LDX:dpy LDA:ildpy CLV:imp LDA:absy TSX:imp TYX:imp LDY:absx  LDA:absx LDX:absy LDA:longx
CPY:immx CMP:idpx REP:imm8 CMP:sr   CPY:dp  CMP:dp  DEC:dp  CMP:ildp  INY:imp CMP:immm DEX:imp WAI:imp CPY:abs   CMP:abs  DEC:abs  CMP:long
BNE:rel  CMP:idpy CMP:idp  CMP:isry PEI:idp CMP:dpx DEC:dpx CMP:ildpy CLD:imp CMP:absy PHX:imp STP:imp JML:ilabs CMP:absx DEC:absx CMP:longx
CPX:immx SBC:idpx SEP:imm8 SBC:sr   CPX:dp  SBC:dp  INC:dp  SBC:ildp  INX:imp SBC:immm NOP:imp XBA:imp CPX:abs   SBC:abs  INC:abs  SBC:long
BEQ:rel  SBC:idpy SBC:idp  SBC:isry PEA:abs SBC:dpx INC:dpx SBC:ildpy SED:imp SBC:absy PLX:imp XCE:imp JSR:iabsx SBC:absx INC:absx SBC:longx
"""
OPCODES: list[tuple[str, str]] = [tuple(entry.split(":")) for entry in _TABLE.split()]  # type: ignore[misc]
assert len(OPCODES) == 256

# Operand size in bytes and display format for each addressing mode.
_MODES: dict[str, tuple[int, str]] = {
    "imp": (0, ""),
    "acc": (0, "A"),
    "imm8": (1, "#${:02X}"),
    "dp": (1, "${:02X}"),
    "dpx": (1, "${:02X},X"),
    "dpy": (1, "${:02X},Y"),
    "idp": (1, "(${:02X})"),
    "idpx": (1, "(${:02X},X)"),
    "idpy": (1, "(${:02X}),Y"),
    "ildp": (1, "[${:02X}]"),
    "ildpy": (1, "[${:02X}],Y"),
    "sr": (1, "${:02X},S"),
    "isry": (1, "(${:02X},S),Y"),
    "abs": (2, "${:04X}"),
    "absx": (2, "${:04X},X"),
    "absy": (2, "${:04X},Y"),
    "iabs": (2, "(${:04X})"),
    "iabsx": (2, "(${:04X},X)"),
    "ilabs": (2, "[${:04X}]"),
    "long": (3, "${:06X}"),
    "longx": (3, "${:06X},X"),
    "rel": (1, "${:04X}"),
    "rell": (2, "${:04X}"),
    "mv": (2, "${:02X},${:02X}"),
}


class DisassemblyError(ValueError):
    """The bytes end in the middle of an instruction."""


@dataclass(frozen=True)
class Instruction:
    address: int  # 24-bit CPU address
    data: bytes
    mnemonic: str
    mode: str
    operand: str
    target: int | None  # where a branch, jump or call goes, when the instruction says so

    @property
    def size(self) -> int:
        return len(self.data)

    def __str__(self) -> str:
        return f"{self.mnemonic} {self.operand}".rstrip()


def decode(data: bytes, offset: int, address: int, m8: bool = True, x8: bool = True) -> Instruction:
    """Decode the instruction at ``data[offset]``, which the CPU sees at ``address``."""
    if offset >= len(data):
        raise DisassemblyError("no instruction at this offset")
    mnemonic, mode = OPCODES[data[offset]]
    if mode in ("immm", "immx"):
        size = 1 if (m8 if mode == "immm" else x8) else 2
        text = "#${:02X}" if size == 1 else "#${:04X}"
    else:
        size, text = _MODES[mode]
    raw = data[offset : offset + 1 + size]
    if len(raw) != 1 + size:
        raise DisassemblyError("instruction is cut off by the end of the data")
    value = int.from_bytes(raw[1:], "little")
    bank = address & 0xFF0000
    target: int | None = None

    if mode == "rel":
        value = (address + 2 + (value - 0x100 if value & 0x80 else value)) & 0xFFFF
        target = bank | value
    elif mode == "rell":
        value = (address + 3 + (value - 0x10000 if value & 0x8000 else value)) & 0xFFFF
        target = bank | value
    elif mnemonic in ("JSR", "JMP") and mode == "abs":
        target = bank | value
    elif mnemonic in ("JSL", "JML") and mode == "long":
        target = value

    if mode == "mv":
        operand = text.format(raw[2], raw[1])  # written source first, stored destination first
    else:
        operand = text.format(value)
    return Instruction(address, bytes(raw), mnemonic, mode, operand, target)


def disassemble(
    data: bytes, offset: int, address: int, count: int, m8: bool = True, x8: bool = True
) -> Iterator[Instruction]:
    """Decode ``count`` instructions in a straight line, following ``REP`` and ``SEP`` for the flag state.

    Flags changed any other way (``PLP``, ``XCE``, a jump into code that assumes other flags) are not
    tracked, so a listing can drift after such an instruction.
    """
    for _ in range(count):
        instruction = decode(data, offset, address, m8, x8)
        yield instruction
        if instruction.mnemonic in ("REP", "SEP"):
            mask = instruction.data[1]
            if mask & FLAG_M:
                m8 = instruction.mnemonic == "SEP"
            if mask & FLAG_X:
                x8 = instruction.mnemonic == "SEP"
        offset += instruction.size
        address = (address & 0xFF0000) | ((address + instruction.size) & 0xFFFF)
