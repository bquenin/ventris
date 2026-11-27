//! A small assembler for the MIPS I processor of the PlayStation (the R3000A), for the code that a
//! patch adds to a game.
//!
//! Syntax, one statement per line, `;` starts a comment:
//!
//! ```text
//! org $801F8000           ; where the following code lives
//! limit = 224             ; a constant
//! advance:                ; a label
//! .loop:                  ; a local label, private to the last plain label
//!     lhu   v1, 0(t2)     ; registers by name, with or without `$`
//!     lui   at, %hi(widths)
//!     addu  at, at, v1
//!     lbu   v1, %lo(widths)(at)
//!     nop                 ; nothing is reordered: delay slots are written out
//!     bnez  v1, .loop
//!     addiu a3, a3, 14
//!     jr    ra
//!     nop
//! widths:
//!     .byte 14, 14, 9     ; data: .byte, .half, .word; .align 4; .space 16
//! ```
//!
//! Besides the processor's instructions there are `nop`, `move`, `li` (a number of 16 bits), `la`
//! (an address, two instructions), `b`, `beqz`, `bnez` and `negu`. `%hi` and `%lo` give the halves of an
//! address as `lui` and a signed offset add them up. Expressions are sums and differences of numbers
//! (`$1F`, `0x1F`, `31`) and names.
//!
//! The processor reads a register loaded from memory only from the second instruction after the
//! load; one that reads it in the next gets what the register held before. The assembler refuses
//! that, and a jump or a branch in the delay slot of another, so the code means what it says.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::text::quoted;

fn assembly_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Assembly(message.into()))
}

/// What was assembled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assembly {
    /// An address and the code that goes there, for each `org`.
    pub segments: Vec<(u32, Vec<u8>)>,
    /// Every name and its value: labels, constants, and the symbols that were given.
    pub labels: HashMap<String, i64>,
}

impl Assembly {
    /// The value of a label or constant, which the caller knows the source defines.
    pub fn label(&self, name: &str) -> Result<u32> {
        match self.labels.get(name) {
            Some(value) => Ok(*value as u32),
            None => assembly_error(format!("{name} is not defined")),
        }
    }
}

const REGISTERS: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "s0", "s1", "s2",
    "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp", "ra",
];

/// A register by its name (`a0`, `$a0`, `$4`, `r4`).
fn register(text: &str) -> Result<u32> {
    let name = text.strip_prefix('$').unwrap_or(text).to_ascii_lowercase();
    if let Some(number) = REGISTERS.iter().position(|known| *known == name) {
        return Ok(number as u32);
    }
    if name == "s8" {
        return Ok(30);
    }
    match name.strip_prefix('r').unwrap_or(&name).parse::<u32>() {
        Ok(number) if number < 32 && text.starts_with(['$', 'r']) => Ok(number),
        _ => assembly_error(format!("{} is not a register", quoted(text))),
    }
}

/// How an instruction's operands are written, and how it is encoded.
#[derive(Clone, Copy)]
enum Form {
    /// `rd, rs, rt`, with this function code.
    Three(u32),
    /// `rd, rt, shift`.
    Shift(u32),
    /// `rd, rt, rs`.
    ShiftBy(u32),
    /// `rs, rt`: multiply and divide.
    Pair(u32),
    /// `rd`: move from HI or LO.
    From(u32),
    /// `rs`: move to HI or LO.
    To(u32),
    /// `rt, rs, number`: arithmetic with a number, signed or not.
    Number(u32, bool),
    /// `rt, number`.
    Upper,
    /// `rt, offset(rs)`: a load, or a store.
    Memory(u32),
    /// `rs, rt, label`.
    Compare(u32),
    /// `rs, label`: a branch on one register, by opcode and the code put in `rt`.
    Test(u32, u32),
}

/// The instructions, by mnemonic.
const INSTRUCTIONS: [(&str, Form); 46] = [
    ("sll", Form::Shift(0x00)),
    ("srl", Form::Shift(0x02)),
    ("sra", Form::Shift(0x03)),
    ("sllv", Form::ShiftBy(0x04)),
    ("srlv", Form::ShiftBy(0x06)),
    ("srav", Form::ShiftBy(0x07)),
    ("mfhi", Form::From(0x10)),
    ("mthi", Form::To(0x11)),
    ("mflo", Form::From(0x12)),
    ("mtlo", Form::To(0x13)),
    ("mult", Form::Pair(0x18)),
    ("multu", Form::Pair(0x19)),
    ("div", Form::Pair(0x1A)),
    ("divu", Form::Pair(0x1B)),
    ("add", Form::Three(0x20)),
    ("addu", Form::Three(0x21)),
    ("sub", Form::Three(0x22)),
    ("subu", Form::Three(0x23)),
    ("and", Form::Three(0x24)),
    ("or", Form::Three(0x25)),
    ("xor", Form::Three(0x26)),
    ("nor", Form::Three(0x27)),
    ("slt", Form::Three(0x2A)),
    ("sltu", Form::Three(0x2B)),
    ("addi", Form::Number(0x08, true)),
    ("addiu", Form::Number(0x09, true)),
    ("slti", Form::Number(0x0A, true)),
    ("sltiu", Form::Number(0x0B, true)),
    ("andi", Form::Number(0x0C, false)),
    ("ori", Form::Number(0x0D, false)),
    ("xori", Form::Number(0x0E, false)),
    ("lui", Form::Upper),
    ("lb", Form::Memory(0x20)),
    ("lh", Form::Memory(0x21)),
    ("lw", Form::Memory(0x23)),
    ("lbu", Form::Memory(0x24)),
    ("lhu", Form::Memory(0x25)),
    ("sb", Form::Memory(0x28)),
    ("sh", Form::Memory(0x29)),
    ("sw", Form::Memory(0x2B)),
    ("beq", Form::Compare(0x04)),
    ("bne", Form::Compare(0x05)),
    ("blez", Form::Test(0x06, 0)),
    ("bgtz", Form::Test(0x07, 0)),
    ("bltz", Form::Test(0x01, 0)),
    ("bgez", Form::Test(0x01, 1)),
];

/// Where the assembler is in the source.
struct State {
    address: Option<i64>,
    /// The last plain label, which local labels belong to.
    scope: String,
}

/// The name at the start of `text`, if there is one: letters, digits and underscores, not starting
/// with a digit, with a dot in front for a local label.
fn name_at(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let dot = usize::from(bytes.first() == Some(&b'.'));
    let first = *bytes.get(dot)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    let rest = bytes[dot..].iter().take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_').count();
    Some(&text[..dot + rest])
}

/// The full name of a label or constant: a local one has the last plain label before it.
fn qualified(name: &str, state: &State) -> String {
    if name.starts_with('.') { format!("{}{name}", state.scope) } else { name.to_string() }
}

/// Evaluate an expression: `%hi(...)` or `%lo(...)` around a sum, or a sum of numbers and names.
/// `strict` is off on the first pass, when labels further down are not known yet and stand for 0.
fn evaluate(expression: &str, state: &State, labels: &HashMap<String, i64>, strict: bool) -> Result<i64> {
    let lower = expression.to_ascii_lowercase();
    for (prefix, half) in [("%hi(", true), ("%lo(", false)] {
        if lower.starts_with(prefix) && expression.ends_with(')') {
            let value = evaluate(&expression[prefix.len()..expression.len() - 1], state, labels, strict)?;
            return Ok(match half {
                true => (value + 0x8000) >> 16 & 0xFFFF,
                false => i64::from((value & 0xFFFF) as u16 as i16),
            });
        }
    }
    let unreadable = || Error::Assembly(format!("cannot read {}", quoted(expression)));
    let (mut total, mut sign, mut expect_atom) = (0i64, 1i64, true);
    let mut rest = expression;
    while let Some(char) = rest.chars().next() {
        if !expect_atom {
            sign = match char {
                '+' => 1,
                '-' => -1,
                _ => return Err(unreadable()),
            };
            rest = &rest[1..];
            expect_atom = true;
            continue;
        }
        if char == '-' {
            sign = -sign;
            rest = &rest[1..];
            continue;
        }
        let (value, length) = if let Some(digits) = rest.strip_prefix('$').or_else(|| rest.strip_prefix("0x")) {
            let count = digits.bytes().take_while(u8::is_ascii_hexdigit).count();
            let value = i64::from_str_radix(&digits[..count], 16).map_err(|_| unreadable())?;
            (value, rest.len() - digits.len() + count)
        } else if char.is_ascii_digit() {
            let count = rest.bytes().take_while(u8::is_ascii_digit).count();
            (rest[..count].parse().map_err(|_| unreadable())?, count)
        } else {
            let name = name_at(rest).ok_or_else(unreadable)?;
            let known = labels.get(&qualified(name, state)).copied();
            if known.is_none() && strict {
                return assembly_error(format!("{name} is not defined"));
            }
            (known.unwrap_or(0), name.len())
        };
        total = sign
            .checked_mul(value)
            .and_then(|term| total.checked_add(term))
            .ok_or_else(|| Error::Assembly(format!("{expression} is too large a number")))?;
        sign = 1;
        rest = &rest[length..];
        expect_atom = false;
    }
    if expect_atom {
        return Err(unreadable());
    }
    Ok(total)
}

/// What an instruction does with registers, for the checks of delay slots.
#[derive(Clone, Copy, Default)]
struct Effect {
    /// The register a load puts its value in, which the next instruction may not read.
    loads: Option<u32>,
    /// The registers it reads.
    reads: [Option<u32>; 2],
    /// Whether it jumps or branches, and so has a delay slot.
    jumps: bool,
}

fn effect(word: u32) -> Effect {
    let (operation, rs, rt) = (word >> 26, word >> 21 & 31, word >> 16 & 31);
    let reads = |registers: &[u32]| {
        let mut found = [None; 2];
        for (slot, register) in found.iter_mut().zip(registers) {
            *slot = Some(*register).filter(|register| *register != 0);
        }
        found
    };
    match operation {
        0 => {
            let function = word & 0x3F;
            match function {
                0x00 | 0x02 | 0x03 => Effect { reads: reads(&[rt]), ..Effect::default() },
                0x08 | 0x09 => Effect { reads: reads(&[rs]), jumps: true, ..Effect::default() },
                0x10 | 0x12 => Effect::default(),
                0x11 | 0x13 => Effect { reads: reads(&[rs]), ..Effect::default() },
                _ => Effect { reads: reads(&[rs, rt]), ..Effect::default() },
            }
        }
        0x01 | 0x06 | 0x07 => Effect { reads: reads(&[rs]), jumps: true, ..Effect::default() },
        0x02 | 0x03 => Effect { jumps: true, ..Effect::default() },
        0x04 | 0x05 => Effect { reads: reads(&[rs, rt]), jumps: true, ..Effect::default() },
        0x0F => Effect::default(),
        0x20..=0x26 => Effect { loads: Some(rt).filter(|rt| *rt != 0), reads: reads(&[rs]), ..Effect::default() },
        0x28..=0x2E => Effect { reads: reads(&[rs, rt]), ..Effect::default() },
        _ => Effect { reads: reads(&[rs]), ..Effect::default() },
    }
}

/// The operands of an instruction, split at commas outside parentheses.
fn operands(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (at, char) in text.char_indices() {
        match char {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                found.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    if !text.trim().is_empty() {
        found.push(text[start..].trim());
    }
    found
}

struct Pass<'a> {
    /// The second pass, which emits the code; the first only places the labels.
    last: bool,
    state: State,
    labels: &'a mut HashMap<String, i64>,
    segments: &'a mut Vec<(u32, Vec<u8>)>,
    /// What the instruction before did, in the same run of code.
    before: Option<Effect>,
}

impl Pass<'_> {
    fn define(&mut self, name: String, value: i64) -> Result<()> {
        if self.labels.contains_key(&name) {
            return assembly_error(format!("{name} is defined twice"));
        }
        self.labels.insert(name, value);
        Ok(())
    }

    fn value(&self, expression: &str) -> Result<i64> {
        evaluate(expression, &self.state, self.labels, self.last)
    }

    /// A number that must fit 16 bits, signed or not.
    fn number(&self, expression: &str, signed: bool) -> Result<u32> {
        let value = self.value(expression)?;
        let fits = match signed {
            true => (-0x8000..0x8000).contains(&value),
            false => (0..0x10000).contains(&value),
        };
        if !fits && self.last {
            let kind = if signed { "a signed" } else { "an unsigned" };
            return assembly_error(format!("{expression} ({value}) is not {kind} number of 16 bits"));
        }
        Ok(value as u32 & 0xFFFF)
    }

    /// The offset of a branch at `address` to a label.
    fn branch(&self, target: &str, address: i64) -> Result<u32> {
        let distance = self.value(target)? - (address + 4);
        if self.last && (distance % 4 != 0 || !(-0x20000..0x20000).contains(&distance)) {
            return assembly_error(format!("{target} is out of a branch's reach"));
        }
        Ok((distance >> 2) as u32 & 0xFFFF)
    }

    /// Assemble one line, already without its comment and the blanks around it.
    fn statement(&mut self, mut text: &str) -> Result<()> {
        while let Some(name) = name_at(text).filter(|name| text[name.len()..].starts_with(':')) {
            if !name.starts_with('.') {
                self.state.scope = name.to_string();
            }
            let full = qualified(name, &self.state);
            let Some(address) = self.state.address else {
                return assembly_error("a label needs an org before it");
            };
            if !self.last {
                self.define(full, address)?;
            }
            text = text[name.len() + 1..].trim();
        }
        if text.is_empty() {
            return Ok(());
        }
        // A constant: a name, an equals sign, and an expression.
        let constant = name_at(text).and_then(|name| {
            let value = text[name.len()..].trim_start().strip_prefix('=')?;
            Some((name, value.trim())).filter(|(_, value)| !value.is_empty())
        });
        if let Some((name, value)) = constant {
            if !name.starts_with('.') {
                self.state.scope = name.to_string();
            }
            let full = qualified(name, &self.state);
            if !self.last {
                let value = evaluate(&value.replace(' ', ""), &self.state, self.labels, true)?;
                self.define(full, value)?;
            }
            return Ok(());
        }
        let (word, operand) = text.split_once([' ', '\t']).unwrap_or((text, ""));
        let mnemonic = word.to_ascii_lowercase();
        let operand = operand.replace([' ', '\t'], "");
        if mnemonic == "org" {
            let address = self.value(&operand)?;
            if !(0..=0xFFFF_FFFF).contains(&address) || address % 4 != 0 {
                return assembly_error(format!("{operand} is not an address of a word"));
            }
            self.state.address = Some(address);
            self.before = None;
            if self.last {
                self.segments.push((address as u32, Vec::new()));
            }
            return Ok(());
        }
        let Some(address) = self.state.address else {
            return assembly_error("code needs an org before it");
        };
        let items = operands(&operand);
        let code = match mnemonic.as_str() {
            ".byte" | ".half" | ".word" => {
                let size = match mnemonic.as_str() {
                    ".byte" => 1,
                    ".half" => 2,
                    _ => 4,
                };
                let mut code = Vec::new();
                for item in &items {
                    code.extend(&self.value(item)?.to_le_bytes()[..size]);
                }
                self.before = None;
                code
            }
            ".align" => {
                let size = self.value(&operand)?;
                if size <= 0 {
                    return assembly_error(format!("cannot align to {operand}"));
                }
                self.before = None;
                vec![0; ((size - address.rem_euclid(size)) % size) as usize]
            }
            ".space" => {
                self.before = None;
                vec![0; usize::try_from(self.value(&operand)?).unwrap_or(0)]
            }
            _ => {
                let words = self.instruction(&mnemonic, &items, address)?;
                if self.last {
                    for word in &words {
                        self.check(*word)?;
                    }
                }
                words.iter().flat_map(|word| word.to_le_bytes()).collect()
            }
        };
        self.state.address = Some(address + code.len() as i64);
        if self.last
            && let Some((_, segment)) = self.segments.last_mut()
        {
            segment.extend(code);
        }
        Ok(())
    }

    /// Refuse an instruction that reads the register the one before loads, or that jumps in the
    /// delay slot of a jump.
    fn check(&mut self, word: u32) -> Result<()> {
        let now = effect(word);
        if let Some(before) = self.before {
            if let Some(loaded) = before.loads
                && now.reads.contains(&Some(loaded))
            {
                return assembly_error(format!(
                    "reads ${} in the delay slot of the load before it, which gives it the register's old value",
                    REGISTERS[loaded as usize]
                ));
            }
            if before.jumps && now.jumps {
                return assembly_error("a jump or a branch in the delay slot of another");
            }
        }
        self.before = Some(now);
        Ok(())
    }

    /// The words of one instruction.
    fn instruction(&self, mnemonic: &str, items: &[&str], address: i64) -> Result<Vec<u32>> {
        let count = |wanted: usize| match items.len() == wanted {
            true => Ok(()),
            false => assembly_error(format!("{mnemonic} takes {wanted} operands")),
        };
        let r = |at: usize| register(items[at]);
        let special = |rs: u32, rt: u32, rd: u32, shift: u32, function: u32| {
            rs << 21 | rt << 16 | rd << 11 | shift << 6 | function
        };
        let immediate = |operation: u32, rs: u32, rt: u32, value: u32| operation << 26 | rs << 21 | rt << 16 | value;
        let jump = |operation: u32, target: &str| -> Result<u32> {
            let target = self.value(target)?;
            if self.last && (target % 4 != 0 || (target ^ (address + 4)) & 0xF000_0000 != 0) {
                return assembly_error(format!("{target:#x} is out of a jump's reach"));
            }
            Ok(operation << 26 | (target as u32 >> 2 & 0x03FF_FFFF))
        };
        let word = match mnemonic {
            "nop" => {
                count(0)?;
                0
            }
            "move" => {
                count(2)?;
                special(r(1)?, 0, r(0)?, 0, 0x21)
            }
            "negu" => {
                count(2)?;
                special(0, r(1)?, r(0)?, 0, 0x23)
            }
            "li" => {
                count(2)?;
                let value = self.value(items[1])?;
                match value {
                    -0x8000..0x8000 => immediate(0x09, 0, r(0)?, value as u32 & 0xFFFF),
                    0x8000..0x10000 => immediate(0x0D, 0, r(0)?, value as u32),
                    _ if !self.last => 0,
                    _ => return assembly_error(format!("li takes a number of 16 bits: use la for {}", items[1])),
                }
            }
            "la" => {
                count(2)?;
                let (rt, value) = (r(0)?, self.value(items[1])?);
                let high = immediate(0x0F, 0, rt, ((value + 0x8000) >> 16 & 0xFFFF) as u32);
                return Ok(vec![high, immediate(0x09, rt, rt, value as u32 & 0xFFFF)]);
            }
            "b" => {
                count(1)?;
                immediate(0x04, 0, 0, self.branch(items[0], address)?)
            }
            "beqz" | "bnez" => {
                count(2)?;
                let operation = if mnemonic == "beqz" { 0x04 } else { 0x05 };
                immediate(operation, r(0)?, 0, self.branch(items[1], address)?)
            }
            "j" => {
                count(1)?;
                jump(0x02, items[0])?
            }
            "jal" => {
                count(1)?;
                jump(0x03, items[0])?
            }
            "jr" => {
                count(1)?;
                special(r(0)?, 0, 0, 0, 0x08)
            }
            "jalr" => match items.len() {
                1 => special(r(0)?, 0, 31, 0, 0x09),
                _ => {
                    count(2)?;
                    special(r(1)?, 0, r(0)?, 0, 0x09)
                }
            },
            "syscall" | "break" => {
                count(0)?;
                if mnemonic == "syscall" { 0x0C } else { 0x0D }
            }
            _ => {
                let Some((_, form)) = INSTRUCTIONS.iter().find(|(name, _)| *name == mnemonic) else {
                    return assembly_error(format!("unknown instruction {mnemonic}"));
                };
                match *form {
                    Form::Three(function) => {
                        count(3)?;
                        special(r(1)?, r(2)?, r(0)?, 0, function)
                    }
                    Form::Shift(function) => {
                        count(3)?;
                        let shift = self.value(items[2])?;
                        if !(0..32).contains(&shift) {
                            return assembly_error(format!("{} is not a shift of 0 to 31", items[2]));
                        }
                        special(0, r(1)?, r(0)?, shift as u32, function)
                    }
                    Form::ShiftBy(function) => {
                        count(3)?;
                        special(r(2)?, r(1)?, r(0)?, 0, function)
                    }
                    Form::Pair(function) => {
                        count(2)?;
                        special(r(0)?, r(1)?, 0, 0, function)
                    }
                    Form::From(function) => {
                        count(1)?;
                        special(0, 0, r(0)?, 0, function)
                    }
                    Form::To(function) => {
                        count(1)?;
                        special(r(0)?, 0, 0, 0, function)
                    }
                    Form::Number(operation, signed) => {
                        count(3)?;
                        immediate(operation, r(1)?, r(0)?, self.number(items[2], signed)?)
                    }
                    Form::Upper => {
                        count(2)?;
                        immediate(0x0F, 0, r(0)?, self.number(items[1], false)?)
                    }
                    Form::Memory(operation) => {
                        count(2)?;
                        let place = items[1];
                        let Some(open) = place.rfind('(').filter(|_| place.ends_with(')')) else {
                            return assembly_error(format!("{} is not an offset and a register", quoted(place)));
                        };
                        let base = register(&place[open + 1..place.len() - 1])?;
                        let offset = if open == 0 { 0 } else { self.number(&place[..open], true)? };
                        immediate(operation, base, r(0)?, offset)
                    }
                    Form::Compare(operation) => {
                        count(3)?;
                        immediate(operation, r(0)?, r(1)?, self.branch(items[2], address)?)
                    }
                    Form::Test(operation, code) => {
                        count(2)?;
                        immediate(operation, r(0)?, code, self.branch(items[1], address)?)
                    }
                }
            }
        };
        Ok(vec![word])
    }
}

/// Assemble `source`. `symbols` are addresses defined elsewhere, such as routines in the game.
pub fn assemble(source: &str, symbols: &[(&str, i64)]) -> Result<Assembly> {
    let statements: Vec<&str> = source.lines().map(|line| line.split(';').next().unwrap_or_default().trim()).collect();
    let mut labels: HashMap<String, i64> = symbols.iter().map(|(name, value)| ((*name).to_string(), *value)).collect();
    let mut segments = Vec::new();
    for last in [false, true] {
        let state = State { address: None, scope: String::new() };
        let mut pass = Pass { last, state, labels: &mut labels, segments: &mut segments, before: None };
        for (number, text) in statements.iter().enumerate() {
            pass.statement(text).map_err(|error| match error {
                Error::Assembly(message) => Error::Assembly(format!("line {}: {message}", number + 1)),
                other => other,
            })?;
        }
    }
    Ok(Assembly { segments, labels })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(source: &str) -> Vec<u32> {
        let assembly = assemble(source, &[]).unwrap();
        assembly.segments[0].1.chunks(4).map(|word| u32::from_le_bytes(word.try_into().unwrap())).collect()
    }

    fn refused(source: &str) -> String {
        assemble(source, &[]).unwrap_err().to_string()
    }

    #[test]
    fn instructions_encode_as_the_game_has_them() {
        // Each word is from G.O.D pure's program, at the address in the comment.
        let source = "org $80024F2C
            bnez  v0, $80024F44       ; 80024F2C
            nop
            jal   $80024858           ; 80024F34
            move  a0, s0
            addiu a3, a3, $E
            lhu   v0, 4(s0)
            sll   s1, s1, 1
            mult  v1, v0
            mflo  a1
            addu  v0, a1, v0
            sh    a0, (v0)
            sltu  v0, v0, v1
            lui   at, $800E
            lw    v0, $315C(at)
            nop
            andi  v0, v0, $F
            jr    ra
            addiu sp, sp, -$18
            sra   v0, a2, 7
            slt   v0, t0, v0
            lbu   v0, -$11B0(at)
            nop
            sw    zero, ($v0)";
        assert_eq!(
            words(source),
            [
                0x1440_0005,
                0,
                0x0C00_9216,
                0x0200_2021,
                0x24E7_000E,
                0x9602_0004,
                0x0011_8840,
                0x0062_0018,
                0x0000_2812,
                0x00A2_1021,
                0xA444_0000,
                0x0043_102B,
                0x3C01_800E,
                0x8C22_315C,
                0,
                0x3042_000F,
                0x03E0_0008,
                0x27BD_FFE8,
                0x0006_11C3,
                0x0102_102A,
                0x9022_EE50,
                0,
                0xAC40_0000,
            ]
        );
    }

    #[test]
    fn halves_of_an_address_add_up_as_lui_and_a_signed_offset_do() {
        let assembly = assemble(
            "org $801F8000
             start:
                lui  at, %hi(table)
                lbu  v0, %lo(table)(at)
                la   t0, $8008EFC0
                li   t1, $FFFF
                li   t2, -1
             table = $80018000",
            &[],
        )
        .unwrap();
        let code = &assembly.segments[0].1;
        let word = |at: usize| u32::from_le_bytes(code[at * 4..at * 4 + 4].try_into().unwrap());
        assert_eq!(word(0), 0x3C01_8002); // $80018000 is $80020000 less $8000
        assert_eq!(word(1), 0x9022_8000);
        assert_eq!([word(2), word(3)], [0x3C08_8009, 0x2508_EFC0]);
        assert_eq!(word(4), 0x3409_FFFF);
        assert_eq!(word(5), 0x240A_FFFF);
        assert_eq!(assembly.label("start").unwrap(), 0x801F_8000);
    }

    #[test]
    fn branches_reach_labels_before_and_after_and_local_labels_are_private() {
        let source = "org $1000
            one:
            .loop: bne a0, zero, .loop
                nop
                beqz a0, two
                nop
            two:
            .loop: b .loop
                nop
                .word one, two
                .half $1234
                .byte 1, 2
                .align 4
                .space 4";
        let words = words(source);
        assert_eq!(&words[..6], [0x1480_FFFF, 0, 0x1080_0001, 0, 0x1000_FFFF, 0]);
        assert_eq!(&words[6..], [0x1000, 0x1010, 0x0201_1234, 0]);
    }

    #[test]
    fn a_register_read_in_a_load_delay_slot_is_refused() {
        assert_eq!(
            refused("org 0\n lhu v1, 0(t2)\n addu a3, a3, v1"),
            "line 3: reads $v1 in the delay slot of the load before it, which gives it the register's old value"
        );
        assert_eq!(refused("org 0\n lw v0, 0(a0)\n sw v0, 4(a0)").split(':').next(), Some("line 3"));
        // Reading it one instruction later, or loading into zero, is fine.
        assert_eq!(words("org 0\n lhu v1, 0(t2)\n nop\n addu a3, a3, v1").len(), 3);
        assert_eq!(words("org 0\n lw zero, 0(a0)\n addu a0, a0, zero").len(), 2);
    }

    #[test]
    fn mistakes_are_refused_with_the_line() {
        assert_eq!(refused("org 0\n jr ra\n b 0"), "line 3: a jump or a branch in the delay slot of another");
        assert_eq!(refused("org 0\n addiu a0, a0, $8000"), "line 2: $8000 (32768) is not a signed number of 16 bits");
        assert_eq!(refused("org 0\n frob a0"), "line 2: unknown instruction frob");
        assert_eq!(refused("org 0\n addu a0, a1, q9"), "line 2: 'q9' is not a register");
        assert_eq!(refused("org 0\n j nowhere"), "line 2: nowhere is not defined");
        assert_eq!(refused("nop"), "line 1: code needs an org before it");
        assert_eq!(refused("org 0\n li a0, $12345"), "line 2: li takes a number of 16 bits: use la for $12345");
        assert_eq!(refused("org $0FFFFFF8\n j $10000000"), "line 2: 0x10000000 is out of a jump's reach");
    }
}
