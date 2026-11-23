//! The instructions of the WDC 65C816, the CPU of the SNES: what each of the 256 opcodes is.
//!
//! The size of an immediate operand depends on two processor flags that only the running program
//! knows: M selects an 8-bit or 16-bit accumulator, X the same for the index registers.
//!
//! The assembler in `asm65816` works from this table. The disassembler that reads the same table is
//! part of the Python tools for taking a game apart, under `src/ventris_workbench`, and is not here.

use std::sync::LazyLock;

pub const FLAG_M: u8 = 0x20;
pub const FLAG_X: u8 = 0x10;

/// How an instruction finds its operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// No operand.
    Imp,
    /// The accumulator: `ASL A`.
    Acc,
    /// One byte, whatever the flags: `REP #$30`.
    Imm8,
    /// One byte or two, by the M flag: `LDA #$12`.
    Immm,
    /// One byte or two, by the X flag: `LDX #$12`.
    Immx,
    /// Direct page: `$12`.
    Dp,
    /// `$12,X`
    Dpx,
    /// `$12,Y`
    Dpy,
    /// `($12)`
    Idp,
    /// `($12,X)`
    Idpx,
    /// `($12),Y`
    Idpy,
    /// `[$12]`
    Ildp,
    /// `[$12],Y`
    Ildpy,
    /// Stack relative: `$12,S`.
    Sr,
    /// `($12,S),Y`
    Isry,
    /// Absolute: `$1234`.
    Abs,
    /// `$1234,X`
    Absx,
    /// `$1234,Y`
    Absy,
    /// `($1234)`
    Iabs,
    /// `($1234,X)`
    Iabsx,
    /// `[$1234]`
    Ilabs,
    /// Long: `$123456`.
    Long,
    /// `$123456,X`
    Longx,
    /// A branch of one byte.
    Rel,
    /// A branch of two bytes.
    Rell,
    /// A block move: source bank, destination bank.
    Mv,
}

impl Mode {
    /// The mode a name of the table below stands for.
    fn named(name: &str) -> Option<Mode> {
        Some(match name {
            "imp" => Mode::Imp,
            "acc" => Mode::Acc,
            "imm8" => Mode::Imm8,
            "immm" => Mode::Immm,
            "immx" => Mode::Immx,
            "dp" => Mode::Dp,
            "dpx" => Mode::Dpx,
            "dpy" => Mode::Dpy,
            "idp" => Mode::Idp,
            "idpx" => Mode::Idpx,
            "idpy" => Mode::Idpy,
            "ildp" => Mode::Ildp,
            "ildpy" => Mode::Ildpy,
            "sr" => Mode::Sr,
            "isry" => Mode::Isry,
            "abs" => Mode::Abs,
            "absx" => Mode::Absx,
            "absy" => Mode::Absy,
            "iabs" => Mode::Iabs,
            "iabsx" => Mode::Iabsx,
            "ilabs" => Mode::Ilabs,
            "long" => Mode::Long,
            "longx" => Mode::Longx,
            "rel" => Mode::Rel,
            "rell" => Mode::Rell,
            "mv" => Mode::Mv,
            _ => return None,
        })
    }
}

/// Sixteen opcodes a line, from $00 up.
const TABLE: &str = "
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
";

/// Each opcode's mnemonic and addressing mode, by opcode.
pub static OPCODES: LazyLock<Vec<(&'static str, Mode)>> = LazyLock::new(|| {
    let entry = |entry: &'static str| {
        let named = entry.split_once(':').and_then(|(mnemonic, mode)| Some((mnemonic, Mode::named(mode)?)));
        named.unwrap_or_else(|| panic!("the opcode table has an entry that cannot be read: {entry}"))
    };
    let opcodes: Vec<_> = TABLE.split_whitespace().map(entry).collect();
    assert_eq!(opcodes.len(), 256, "the opcode table must name every opcode");
    opcodes
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_every_opcode() {
        assert_eq!(OPCODES.len(), 256);
        assert_eq!(OPCODES[0x00], ("BRK", Mode::Imm8));
        assert_eq!(OPCODES[0xA9], ("LDA", Mode::Immm));
        assert_eq!(OPCODES[0x22], ("JSL", Mode::Long));
        assert_eq!(OPCODES[0x54], ("MVN", Mode::Mv));
        assert_eq!(OPCODES[0xFF], ("SBC", Mode::Longx));
        // No instruction has the same form twice, so that an instruction and a form give one opcode.
        let forms: std::collections::HashSet<_> = OPCODES.iter().collect();
        assert_eq!(forms.len(), 256);
    }
}
