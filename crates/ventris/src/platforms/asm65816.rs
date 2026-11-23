//! A small assembler for the WDC 65C816, for the code that a patch injects into a game.
//!
//! Syntax, one statement per line, `;` starts a comment:
//!
//! ```text
//! org $60A000             ; where the following code lives, as a 24-bit CPU address
//! width = $2E             ; a constant; it remembers the size it was written with
//! draw:                   ; a label
//! .loop:                  ; a local label, private to the last plain label
//!     m8                  ; the accumulator is 8 bits wide from here (m16, x8, x16 likewise)
//!     lda #$12
//!     sta width           ; direct page, because the constant was written with two digits
//!     sta $2180           ; absolute, four digits
//!     sta $002180         ; long, six digits
//!     lda.l table,x       ; .b .w .l force the operand size
//!     bne .loop
//!     jsl draw            ; jumps and calls take whichever size the instruction has
//!     mvn $7E,$C8         ; source bank, destination bank
//!     db $01,$02          ; bytes; dw and dl emit 16-bit and 24-bit values
//!     dw <draw, >draw     ; < low byte, > high byte, ^ bank byte
//! ```
//!
//! A label used as an operand is 16 bits wide unless forced, so every size is known before any
//! address is. `rep` and `sep` with a literal operand update the register widths, as the CPU would.
//!
//! Numbers are held in 64 bits, and `org` takes a 24-bit address; anything larger is refused.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::error::{Error, Result};
use crate::platforms::w65816::{FLAG_M, FLAG_X, Mode, OPCODES};
use crate::text::quoted;

/// An instruction in one of its forms -> its opcode.
static OPCODE: LazyLock<HashMap<(&'static str, Mode), u8>> = LazyLock::new(|| {
    OPCODES.iter().enumerate().map(|(opcode, (mnemonic, mode))| ((*mnemonic, *mode), opcode as u8)).collect()
});
static MNEMONICS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| OPCODES.iter().map(|(mnemonic, _)| *mnemonic).collect());

/// How an operand is written, apart from its size.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    /// `$12`
    Direct,
    /// `$12,X`
    X,
    /// `$12,Y`
    Y,
    /// `($12)`
    Indirect,
    /// `($12,X)`
    IndirectX,
    /// `($12),Y`
    IndirectY,
    /// `[$12]`
    Long,
    /// `[$12],Y`
    LongY,
    /// `$12,S`
    Stack,
    /// `($12,S),Y`
    StackY,
}

impl Syntax {
    /// The addressing mode for an operand written this way, of `size` bytes.
    fn mode(self, size: usize) -> Option<Mode> {
        Some(match (self, size) {
            (Syntax::Direct, 1) => Mode::Dp,
            (Syntax::Direct, 2) => Mode::Abs,
            (Syntax::Direct, 3) => Mode::Long,
            (Syntax::X, 1) => Mode::Dpx,
            (Syntax::X, 2) => Mode::Absx,
            (Syntax::X, 3) => Mode::Longx,
            (Syntax::Y, 1) => Mode::Dpy,
            (Syntax::Y, 2) => Mode::Absy,
            (Syntax::Indirect, 1) => Mode::Idp,
            (Syntax::Indirect, 2) => Mode::Iabs,
            (Syntax::IndirectX, 1) => Mode::Idpx,
            (Syntax::IndirectX, 2) => Mode::Iabsx,
            (Syntax::IndirectY, 1) => Mode::Idpy,
            (Syntax::Long, 1) => Mode::Ildp,
            (Syntax::Long, 2) => Mode::Ilabs,
            (Syntax::LongY, 1) => Mode::Ildpy,
            (Syntax::Stack, 1) => Mode::Sr,
            (Syntax::StackY, 1) => Mode::Isry,
            _ => return None,
        })
    }
}

/// What stands before and after the expression in each way of writing an operand, in the order they
/// are tried. The register letters may be written in either case.
const SYNTAX_PATTERNS: [(Syntax, &str, &str); 9] = [
    (Syntax::StackY, "(", ",S),Y"),
    (Syntax::IndirectX, "(", ",X)"),
    (Syntax::IndirectY, "(", "),Y"),
    (Syntax::Indirect, "(", ")"),
    (Syntax::LongY, "[", "],Y"),
    (Syntax::Long, "[", "]"),
    (Syntax::Stack, "", ",S"),
    (Syntax::X, "", ",X"),
    (Syntax::Y, "", ",Y"),
];

fn assembly_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Assembly(message.into()))
}

/// What was assembled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assembly {
    /// A CPU address and the code that goes there, for each `org`.
    pub segments: Vec<(u32, Vec<u8>)>,
    /// Every name and its value: labels, constants, and the symbols that were given.
    pub labels: HashMap<String, i64>,
}

/// Where the assembler is in the source.
struct State {
    address: Option<i64>,
    m8: bool,
    x8: bool,
    /// The last plain label, which local labels belong to.
    scope: String,
}

/// Whether a character is blank to the Python reference: the spaces of Unicode, and four separators.
fn is_space(char: char) -> bool {
    char.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&char)
}

fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// The lines of a source, ended wherever the Python reference ends a line: at a line feed, a
/// carriage return or both, and at the rarer separators (form feed, U+2028 and the like).
fn lines(source: &str) -> Vec<&str> {
    let ends_line = |char: char| {
        matches!(char, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}')
    };
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = source.char_indices().peekable();
    while let Some((at, char)) = chars.next() {
        if !ends_line(char) {
            continue;
        }
        lines.push(&source[start..at]);
        start = at + char.len_utf8();
        if char == '\r' && chars.next_if(|(_, next)| *next == '\n').is_some() {
            start += 1;
        }
    }
    if start < source.len() {
        lines.push(&source[start..]);
    }
    lines
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

/// The number or name at the start of `text`: `$` and hexadecimal digits, `%` and binary digits,
/// decimal digits, or a name.
fn atom_at(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let run = |from: usize, wanted: fn(&u8) -> bool| bytes[from..].iter().take_while(|byte| wanted(byte)).count();
    let digits = match *bytes.first()? {
        b'$' => 1 + Some(run(1, u8::is_ascii_hexdigit)).filter(|count| *count > 0)?,
        b'%' => 1 + Some(run(1, |byte| matches!(byte, b'0' | b'1'))).filter(|count| *count > 0)?,
        b'0'..=b'9' => run(0, u8::is_ascii_digit),
        _ => return name_at(text),
    };
    Some(&text[..digits])
}

/// The expression inside an operand written as `prefix`, expression, `suffix`. The letters of the
/// suffix are matched in either case.
fn enclosed<'a>(operand: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let rest = operand.strip_prefix(prefix)?;
    let mut end = rest.len();
    let mut found = rest.char_indices().rev();
    for wanted in suffix.chars().rev() {
        let (at, char) = found.next()?;
        // The long s counts as an S, as it does for the Python reference.
        let same = char == wanted
            || (wanted.is_ascii_uppercase() && (char == wanted.to_ascii_lowercase() || (wanted == 'S' && char == 'ſ')));
        if !same {
            return None;
        }
        end = at;
    }
    (end > 0).then(|| &rest[..end])
}

/// A name as it is known everywhere: a local label has the last plain label before it.
fn qualified(atom: &str, state: &State) -> String {
    if atom.starts_with('.') { format!("{}{atom}", state.scope) } else { atom.to_string() }
}

/// The full name of a label or constant being defined. A plain one starts a new scope.
fn qualify(name: &str, state: &mut State) -> String {
    if name.starts_with('.') {
        return format!("{}{name}", state.scope);
    }
    state.scope = name.to_string();
    name.to_string()
}

/// The lowest `size` bytes of a value, lowest first.
fn low_bytes(value: i64, size: usize) -> Vec<u8> {
    value.to_le_bytes()[..size].to_vec()
}

/// Operand size in bytes implied by how an expression is written.
fn width(expression: &str, state: &State, widths: &HashMap<String, usize>) -> Result<usize> {
    if expression.is_empty() || expression.starts_with(['<', '>', '^']) {
        return Ok(1);
    }
    let mut largest = None;
    let mut rest = expression;
    while let Some(char) = rest.chars().next() {
        let Some(atom) = atom_at(rest) else {
            rest = &rest[char.len_utf8()..];
            continue;
        };
        rest = &rest[atom.len()..];
        let size = if atom.starts_with('$') {
            (atom.len() - 1).div_ceil(2) // two digits to a byte
        } else if atom.starts_with('%') {
            (atom.len() - 1).div_ceil(8) // eight digits to a byte
        } else if char.is_ascii_digit() {
            match atom.parse::<u64>() {
                Ok(..0x100) => 1,
                Ok(..0x10000) => 2,
                _ => 3,
            }
        } else {
            widths.get(&qualified(atom, state)).copied().unwrap_or(2)
        };
        largest = largest.max(Some(size));
    }
    match largest {
        Some(size) => Ok(size.min(3)),
        None => assembly_error(format!("cannot read {}", quoted(expression))),
    }
}

/// Evaluate sums and differences of numbers and names, with an optional `<` `>` `^` byte selector.
fn evaluate(expression: &str, state: &State, labels: &HashMap<String, i64>, strict: bool) -> Result<i64> {
    let unreadable = || Error::Assembly(format!("cannot read {}", quoted(expression)));
    let selector = expression.chars().next().filter(|char| matches!(char, '<' | '>' | '^'));
    let mut rest = if selector.is_some() { &expression[1..] } else { expression };
    let (mut total, mut sign) = (0i64, 1i64);
    let mut expect_atom = true;
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
        let atom = atom_at(rest).ok_or_else(unreadable)?;
        let too_large = || Error::Assembly(format!("{atom} is too large a number"));
        let value = if let Some(digits) = atom.strip_prefix('$') {
            i64::from_str_radix(digits, 16).map_err(|_| too_large())?
        } else if let Some(digits) = atom.strip_prefix('%') {
            i64::from_str_radix(digits, 2).map_err(|_| too_large())?
        } else if char.is_ascii_digit() {
            atom.parse().map_err(|_| too_large())?
        } else {
            let known = labels.get(&qualified(atom, state)).copied();
            if known.is_none() && strict {
                return assembly_error(format!("{atom} is not defined"));
            }
            known.unwrap_or(0)
        };
        total = sign
            .checked_mul(value)
            .and_then(|term| total.checked_add(term))
            .ok_or_else(|| Error::Assembly(format!("{expression} is too large a number")))?;
        sign = 1;
        rest = &rest[atom.len()..];
        expect_atom = false;
    }
    if expect_atom {
        return Err(unreadable());
    }
    Ok(match selector {
        Some('<') => total & 0xFF,
        Some('>') => total >> 8 & 0xFF,
        Some('^') => total >> 16 & 0xFF,
        _ => total,
    })
}

/// The bytes of one instruction. `strict` is off on the first pass, when labels further down are
/// not known yet and stand for 0.
fn instruction(
    mnemonic: &'static str,
    suffix: &str,
    operand: &str,
    state: &mut State,
    labels: &HashMap<String, i64>,
    widths: &HashMap<String, usize>,
    strict: bool,
) -> Result<Vec<u8>> {
    let has = |mode: Mode| OPCODE.contains_key(&(mnemonic, mode));
    let emit = |mode: Mode, payload: &[u8]| -> Vec<u8> { [&[OPCODE[&(mnemonic, mode)]], payload].concat() };

    if operand.is_empty() || operand.eq_ignore_ascii_case("A") {
        if has(Mode::Acc) {
            return Ok(emit(Mode::Acc, &[]));
        }
        if has(Mode::Imp) && operand.is_empty() {
            return Ok(emit(Mode::Imp, &[]));
        }
        return assembly_error(format!("{mnemonic} needs an operand"));
    }

    if mnemonic == "MVN" || mnemonic == "MVP" {
        let banks: Vec<&str> = operand.split(',').collect();
        if banks.len() != 2 {
            return assembly_error(format!("{mnemonic} takes a source bank and a destination bank"));
        }
        // Written source first, stored destination first.
        let destination = evaluate(banks[1], state, labels, strict)?;
        let source = evaluate(banks[0], state, labels, strict)?;
        return Ok(emit(Mode::Mv, &[destination as u8, source as u8]));
    }

    if let Some(expression) = operand.strip_prefix('#') {
        let Some(mode) = [Mode::Imm8, Mode::Immm, Mode::Immx].into_iter().find(|mode| has(*mode)) else {
            return assembly_error(format!("{mnemonic} has no immediate form"));
        };
        let narrow = match mode {
            Mode::Imm8 => true,
            Mode::Immm => state.m8,
            _ => state.x8,
        };
        let size = match suffix {
            "B" => 1,
            "W" => 2,
            _ if narrow => 1,
            _ => 2,
        };
        let value = evaluate(expression, state, labels, strict)?;
        if !(-(1i64 << (8 * size - 1))..1i64 << (8 * size)).contains(&value) {
            return assembly_error(format!("{operand} does not fit in {} bits", size * 8));
        }
        let literal = atom_at(expression).is_some_and(|atom| atom.len() == expression.len());
        if (mnemonic == "REP" || mnemonic == "SEP") && literal {
            if value & i64::from(FLAG_M) != 0 {
                state.m8 = mnemonic == "SEP";
            }
            if value & i64::from(FLAG_X) != 0 {
                state.x8 = mnemonic == "SEP";
            }
        }
        return Ok(emit(mode, &low_bytes(value, size)));
    }

    for (mode, size) in [(Mode::Rel, 1), (Mode::Rell, 2)] {
        if !has(mode) {
            continue;
        }
        let address = state.address.unwrap_or(0);
        let target = evaluate(operand, state, labels, strict)?;
        if target > 0xFFFF && strict && target >> 16 != address >> 16 {
            return assembly_error(format!("{operand} is in another bank"));
        }
        let offset = ((target & 0xFFFF) - (address + 1 + size as i64)) & 0xFFFF;
        let signed = if offset & 0x8000 != 0 { offset - 0x10000 } else { offset };
        if size == 1 && strict && !(-128..=127).contains(&signed) {
            return assembly_error(format!("{operand} is too far for a short branch ({signed} bytes)"));
        }
        return Ok(emit(mode, &low_bytes(offset, size)));
    }

    let (syntax, expression) = SYNTAX_PATTERNS
        .iter()
        .find_map(|(syntax, prefix, suffix)| Some((*syntax, enclosed(operand, prefix, suffix)?)))
        .unwrap_or((Syntax::Direct, operand));
    let sizes = match suffix {
        "" => {
            // Take the size the operand was written with, or the next larger one the instruction has.
            let written = width(expression, state, widths)?;
            written..=3
        }
        "B" => 1..=1,
        "W" => 2..=2,
        "L" => 3..=3,
        _ => return assembly_error(format!("unknown size suffix .{}", suffix.to_lowercase())),
    };
    for size in sizes {
        if let Some(mode) = syntax.mode(size).filter(|mode| has(*mode)) {
            let value = evaluate(expression, state, labels, strict)?;
            return Ok(emit(mode, &low_bytes(value, size)));
        }
    }
    assembly_error(format!("{mnemonic} has no form that takes {operand}"))
}

/// Everything a pass over the source builds up.
struct Pass<'a> {
    /// The second pass, which emits the code; the first only needs sizes, to place the labels.
    last: bool,
    state: State,
    labels: &'a mut HashMap<String, i64>,
    widths: &'a mut HashMap<String, usize>,
    segments: &'a mut Vec<(u32, Vec<u8>)>,
}

impl Pass<'_> {
    /// Give a name its value, once.
    fn define(&mut self, name: String, value: i64) -> Result<()> {
        if self.labels.contains_key(&name) {
            return assembly_error(format!("{name} is defined twice"));
        }
        self.labels.insert(name, value);
        Ok(())
    }

    /// Assemble one line, already without its comment and the blanks around it.
    fn statement(&mut self, mut text: &str) -> Result<()> {
        while let Some(name) = name_at(text).filter(|name| text[name.len()..].starts_with(':')) {
            let full = qualify(name, &mut self.state);
            let Some(address) = self.state.address else {
                return assembly_error("a label needs an org before it");
            };
            if !self.last {
                self.define(full, address)?;
            }
            text = strip(&text[name.len() + 1..]);
        }
        if text.is_empty() {
            return Ok(());
        }

        // A constant: a name, an equals sign, and an expression.
        let constant = name_at(text).and_then(|name| {
            let value = text[name.len()..].trim_start_matches(is_space).strip_prefix('=')?;
            Some((name, value.trim_start_matches(is_space))).filter(|(_, value)| !value.is_empty())
        });
        if let Some((name, value)) = constant {
            let full = qualify(name, &mut self.state);
            if !self.last {
                if self.labels.contains_key(&full) {
                    return assembly_error(format!("{full} is defined twice"));
                }
                self.widths.insert(full.clone(), width(value, &self.state, self.widths)?);
                let value = evaluate(value, &self.state, self.labels, true)?;
                self.define(full, value)?;
            }
            return Ok(());
        }

        let (word, operand) = text.split_once(' ').unwrap_or((text, ""));
        let upper = word.to_uppercase();
        let (mnemonic, suffix) = upper.split_once('.').unwrap_or((&upper, ""));
        let operand = operand.replace([' ', '\t'], "");
        if mnemonic == "ORG" {
            let address = evaluate(&operand, &self.state, self.labels, true)?;
            let Some(start) = u32::try_from(address).ok().filter(|start| *start <= 0xFF_FFFF) else {
                return assembly_error(format!("{operand} is not a 24-bit address"));
            };
            self.state.address = Some(address);
            if self.last {
                self.segments.push((start, Vec::new()));
            }
            return Ok(());
        }
        match mnemonic {
            "M8" => self.state.m8 = true,
            "M16" => self.state.m8 = false,
            "X8" => self.state.x8 = true,
            "X16" => self.state.x8 = false,
            _ => return self.code(word, mnemonic, suffix, &operand),
        }
        Ok(())
    }

    /// Assemble a statement that takes room: data, or an instruction.
    fn code(&mut self, word: &str, mnemonic: &str, suffix: &str, operand: &str) -> Result<()> {
        let Some(address) = self.state.address else {
            return assembly_error("code needs an org before it");
        };
        let data_size = match mnemonic {
            "DB" => Some(1),
            "DW" => Some(2),
            "DL" => Some(3),
            _ => None,
        };
        let code = if let Some(size) = data_size {
            let mut code = Vec::new();
            for item in operand.split(',') {
                code.extend(low_bytes(evaluate(item, &self.state, self.labels, self.last)?, size));
            }
            code
        } else if let Some(mnemonic) = MNEMONICS.get(mnemonic).copied() {
            instruction(mnemonic, suffix, operand, &mut self.state, self.labels, self.widths, self.last)?
        } else {
            return assembly_error(format!("unknown instruction {word}"));
        };
        if (address & 0xFFFF) as usize + code.len() > 0x10000 {
            return assembly_error("code runs past the end of its bank");
        }
        self.state.address = Some(address + code.len() as i64);
        if self.last
            && let Some((_, segment)) = self.segments.last_mut()
        {
            segment.extend(code);
        }
        Ok(())
    }
}

/// Assemble `source`. `symbols` are addresses defined elsewhere, such as routines in the game.
pub fn assemble(source: &str, symbols: &[(&str, i64)]) -> Result<Assembly> {
    let statements: Vec<&str> =
        lines(source).into_iter().map(|line| strip(line.split(';').next().unwrap_or_default())).collect();
    let mut labels: HashMap<String, i64> = symbols.iter().map(|(name, value)| ((*name).to_string(), *value)).collect();
    let mut widths = HashMap::new();
    let mut segments = Vec::new();
    for last in [false, true] {
        let state = State { address: None, m8: true, x8: true, scope: String::new() };
        let mut pass = Pass { last, state, labels: &mut labels, widths: &mut widths, segments: &mut segments };
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
    use crate::text::from_hex;

    use super::*;

    fn hex(bytes: &str) -> Vec<u8> {
        from_hex(bytes).unwrap()
    }

    /// The code of a source that has one origin, which is put before it.
    fn code_with(source: &str, symbols: &[(&str, i64)]) -> Result<Vec<u8>> {
        let assembly = assemble(&format!("org $C08000\n{source}"), symbols)?;
        assert_eq!(assembly.segments.len(), 1);
        Ok(assembly.segments[0].1.clone())
    }

    fn code(source: &str) -> Vec<u8> {
        code_with(source, &[]).unwrap()
    }

    /// Why a source was refused.
    fn fault<T: std::fmt::Debug>(result: Result<T>) -> String {
        match result {
            Err(error @ Error::Assembly(_)) => error.to_string(),
            other => panic!("it was not refused as source that cannot be assembled: {other:?}"),
        }
    }

    /// An instruction with operand bytes $12 $34 $56, as a disassembler writes it for the CPU at
    /// `address`, and the bytes it takes.
    fn written(opcode: u8, address: i64, m8: bool, x8: bool) -> (String, Vec<u8>) {
        let (mnemonic, mode) = OPCODES[usize::from(opcode)];
        let near = (address + 2 + 0x12) & 0xFFFF;
        let far = (address + 3 + 0x3412) & 0xFFFF;
        let (size, operand) = match mode {
            Mode::Imp => (0, String::new()),
            Mode::Acc => (0, "A".to_string()),
            Mode::Imm8 => (1, "#$12".to_string()),
            Mode::Immm if m8 => (1, "#$12".to_string()),
            Mode::Immx if x8 => (1, "#$12".to_string()),
            Mode::Immm | Mode::Immx => (2, "#$3412".to_string()),
            Mode::Dp => (1, "$12".to_string()),
            Mode::Dpx => (1, "$12,X".to_string()),
            Mode::Dpy => (1, "$12,Y".to_string()),
            Mode::Idp => (1, "($12)".to_string()),
            Mode::Idpx => (1, "($12,X)".to_string()),
            Mode::Idpy => (1, "($12),Y".to_string()),
            Mode::Ildp => (1, "[$12]".to_string()),
            Mode::Ildpy => (1, "[$12],Y".to_string()),
            Mode::Sr => (1, "$12,S".to_string()),
            Mode::Isry => (1, "($12,S),Y".to_string()),
            Mode::Abs => (2, "$3412".to_string()),
            Mode::Absx => (2, "$3412,X".to_string()),
            Mode::Absy => (2, "$3412,Y".to_string()),
            Mode::Iabs => (2, "($3412)".to_string()),
            Mode::Iabsx => (2, "($3412,X)".to_string()),
            Mode::Ilabs => (2, "[$3412]".to_string()),
            Mode::Long => (3, "$563412".to_string()),
            Mode::Longx => (3, "$563412,X".to_string()),
            Mode::Rel => (1, format!("${near:04X}")),
            Mode::Rell => (2, format!("${far:04X}")),
            Mode::Mv => (2, "$34,$12".to_string()), // written source first, stored destination first
        };
        (format!("{mnemonic} {operand}").trim_end().to_string(), [opcode, 0x12, 0x34, 0x56][..1 + size].to_vec())
    }

    #[test]
    fn every_opcode_is_assembled_from_the_way_it_is_written() {
        let address = 0xC08000;
        for (m8, x8) in [(true, true), (false, false), (true, false), (false, true)] {
            for opcode in 0..=255 {
                let (text, data) = written(opcode, address, m8, x8);
                let widths = format!("{}\n{}", if m8 { "m8" } else { "m16" }, if x8 { "x8" } else { "x16" });
                let source = format!("org ${address:06X}\n{widths}\n{text}");
                assert_eq!(assemble(&source, &[]).unwrap().segments[0].1, data, "opcode {opcode:02X}: {text}");
                // And in small letters, as the injected code is written.
                let source = format!("org ${address:06X}\n{widths}\n{}", text.to_lowercase());
                assert_eq!(assemble(&source, &[]).unwrap().segments[0].1, data, "opcode {opcode:02X}: {text}");
            }
        }
    }

    #[test]
    fn operand_size_follows_how_it_is_written() {
        assert_eq!(code("lda $12"), hex("A5 12"));
        assert_eq!(code("lda $0012"), hex("AD 12 00"));
        assert_eq!(code("lda $000012"), hex("AF 12 00 00"));
        assert_eq!(code("lda 18"), hex("A5 12"));
        assert_eq!(code("lda 4660,x"), hex("BD 34 12"));
        assert_eq!(
            code("lda 256\nlda 65536\nlda %101\nlda %100000001\nlda $1\nlda $123"),
            hex("AD 00 01 AF 00 00 01 A5 05 AD 01 01 A5 01 AD 23 01")
        );
        // A size the instruction lacks is widened; one that is forced is not.
        assert_eq!(code("jsr $12"), hex("20 12 00"));
        assert_eq!(code("lda.l $12"), hex("AF 12 00 00"));
        assert_eq!(code("lda.w $12,y"), hex("B9 12 00"));
        assert_eq!(fault(code_with("stz $002180", &[])), "line 2: STZ has no form that takes $002180");
        assert_eq!(fault(code_with("ldx.b $1234,x", &[])), "line 2: LDX has no form that takes $1234,x");
        assert_eq!(fault(code_with("lda.q $12", &[])), "line 2: unknown size suffix .q");
    }

    #[test]
    fn labels_constants_and_local_labels() {
        let source = "
            width = $2E
            port = $002180
            first:
                lda width
                sta port
            .loop:
                dex
                bne .loop
                jsr second
                jsl second
                jml first
            second:
            .loop:
                bra .loop
                rts
            ";
        // LDA $2E, STA $002180, DEX, BNE $8006, JSR $8014, JSL $C08014, JML $C08000, BRA $8014, RTS
        assert_eq!(code(source), hex("A5 2E 8F 80 21 00 CA D0 FD 20 14 80 22 14 80 C0 5C 00 80 C0 80 FE 60"));
        let assembly = assemble(&format!("org $C08000\n{source}"), &[]).unwrap();
        let value = |name: &str| assembly.labels[name];
        assert_eq!((value("width"), value("first"), value("first.loop")), (0x2E, 0xC08000, 0xC08006));
        assert_eq!((value("second"), value("second.loop")), (0xC08014, 0xC08014));
        // Several labels on a line, and a local label of the last of them.
        assert_eq!(code("foo: bar: lda #<foo\n  .l: bra .l"), hex("A9 00 80 FE"));
        // A constant starts a scope as a label does, and a label after the last byte of a bank has an address.
        let assembly = assemble("org $C0FFFF\nwidth =$2E\n.x: nop\nend:\ndw end", &[("far", 7)]).unwrap();
        assert_eq!(assembly.segments, [(0xC0FFFF, hex("EA 00 00"))]);
        let expected = [("far", 7), ("width", 0x2E), ("width.x", 0xC0FFFF), ("end", 0xC10000)];
        assert_eq!(assembly.labels, expected.into_iter().map(|(name, value)| (name.to_string(), value)).collect());
    }

    #[test]
    fn symbols_from_outside_and_byte_selectors() {
        let source = "jsl draw\nlda #<draw\nldx #>draw\nldy #^draw\ndb <draw,>draw,^draw\ndw draw\ndl draw+2";
        let with_symbol = code_with(source, &[("draw", 0xC8E5A9)]).unwrap();
        assert_eq!(with_symbol, hex("22 A9 E5 C8  A9 A9  A2 E5  A0 C8  A9 E5 C8  A9 E5  AB E5 C8"));
        assert_eq!(
            code_with("lda $12 + $1\nlda <$1234\nlda >$1234,x\nlda ^$123456", &[]).unwrap(),
            hex("A5 13 A5 34 B5 12 A5 12")
        );
        assert_eq!(code_with("x = 5\nlda #x+-3\nlda #-x\nlda #x - 3", &[]).unwrap(), hex("A9 02 A9 FB A9 02"));
        // Data keeps the low bytes of a value, whatever its sign or size.
        assert_eq!(code("dl $123456789, -1\ndw -2\ndb 300"), hex("89 67 45 FF FF FF FE FF 2C"));
    }

    #[test]
    fn register_widths() {
        assert_eq!(code("m16\nlda #$1234\nm8\nlda #$12"), hex("A9 34 12 A9 12"));
        assert_eq!(code("x16\nldx #$1234\nlda #$12"), hex("A2 34 12 A9 12"));
        // rep and sep change the widths like the CPU does.
        assert_eq!(
            code("rep #$30\nlda #$1234\nldx #1\nsep #$20\nlda #1\nldx #1"),
            hex("C2 30 A9 34 12 A2 01 00 E2 20 A9 01 A2 01 00")
        );
        // With a constant too, but not with a sum, whose value the CPU would follow all the same.
        assert_eq!(
            code("flags = $30\nrep #flags\nlda #$1234\nsep #$10+$20\nlda #$12"),
            hex("C2 30 A9 34 12 E2 30 A9 12 00")
        );
        assert_eq!(code("m16\nlda.b #$12"), hex("A9 12"));
        assert_eq!(code("lda #-128\nlda #255\nm16\nlda #-32768\nlda #65535"), hex("A9 80 A9 FF A9 00 80 A9 FF FF"));
        assert_eq!(fault(code_with("lda #$1234", &[])), "line 2: #$1234 does not fit in 8 bits");
        assert_eq!(fault(code_with("lda #-129", &[])), "line 2: #-129 does not fit in 8 bits");
    }

    #[test]
    fn several_origins_and_block_move() {
        let assembly = assemble("org $C8E5A9\njsl new\nrtl\norg $60A000\nnew:\nmvn $7E,$C8\nrtl", &[]).unwrap();
        assert_eq!(assembly.segments, [(0xC8E5A9, hex("22 00 A0 60 6B")), (0x60A000, hex("54 C8 7E 6B"))]);
        assert_eq!(assembly.labels["new"], 0x60A000);
    }

    #[test]
    fn branches_and_every_way_of_writing_an_operand() {
        assert_eq!(code("per next\nbrl next\nnext:\nbra next"), hex("62 03 00 82 00 00 80 FE"));
        assert_eq!(
            code(
                "lda ($12,s),y\nlda [$12],Y\nlda ($12),y\nlda ($12,x)\njmp ($1234)\njml [$1234]\njmp ($1234,x)\nlda $12,s"
            ),
            hex("B3 12 B7 12 B1 12 A1 12 6C 34 12 DC 34 12 7C 34 12 A3 12")
        );
        assert_eq!(code("inc\ninc a\ndec A\nnop\nasl"), hex("1A 1A 3A EA 0A"));
        assert_eq!(code("m8.x\ndb.w 1"), hex("01"));
    }

    #[test]
    fn a_line_ends_where_the_python_reference_ends_it() {
        assert_eq!(
            code("nop\r\nnop\u{b}nop\u{c}nop\u{1c}nop\u{85}nop\u{2028}nop\rnop\n"),
            hex("EA EA EA EA EA EA EA EA")
        );
        assert_eq!(code("\u{1f} nop \u{a0}; a comment"), hex("EA"));
        assert_eq!(lines("a\n\nb\r\n"), ["a", "", "b"]);
        assert!(lines("").is_empty());
    }

    #[test]
    fn errors_name_the_line() {
        for (source, message) in [
            ("lda #1", "line 1: code needs an org before it"),
            ("lda: nop", "line 1: a label needs an org before it"),
            ("org $C08000\nfoo #1", "line 2: unknown instruction foo"),
            ("org $C08000\nlda\t#1", "line 2: unknown instruction lda\t#1"),
            ("org $C08000\nlda missing", "line 2: missing is not defined"),
            ("org $C08000\na:\na:", "line 3: a is defined twice"),
            ("org $C08000\nmoved = $1\nmoved:", "line 3: moved is defined twice"),
            ("org $C08000\nbne far\norg $C08200\nfar:", "line 2: far is too far for a short branch (510 bytes)"),
            ("org $C08000\nbne far\norg $C18000\nfar:", "line 2: far is in another bank"),
            ("org $C08000\nrts #1", "line 2: RTS has no immediate form"),
            ("org $C08000\nlda", "line 2: LDA needs an operand"),
            ("org $C08000\nlda a", "line 2: LDA needs an operand"),
            ("org $C0FFFF\nlda #1", "line 2: code runs past the end of its bank"),
            ("org $C08000\nlda $12+", "line 2: cannot read '$12+'"),
            ("org $C08000\nlda +$12", "line 2: cannot read '+$12'"),
            ("org $C08000\nlda ()", "line 2: cannot read '()'"),
            ("org $C08000\nlda #", "line 2: cannot read ''"),
            ("org $C08000\ndb 1,,2", "line 2: cannot read ''"),
            ("org", "line 1: cannot read ''"),
            ("org $C08000\nwidth = $2E + 1", "line 2: cannot read '$2E + 1'"),
            ("org $C08000\nmvn 1", "line 2: MVN takes a source bank and a destination bank"),
            ("org $C08000\nmvn a,b", "line 2: b is not defined"),
            ("org $C08000\njmp [$1234]", "line 2: JMP has no form that takes [$1234]"),
        ] {
            assert_eq!(fault(assemble(source, &[])), message, "{source:?}");
        }
        assert_eq!(fault(assemble("org $C08000\nmoved:", &[("moved", 1)])), "line 2: moved is defined twice");
    }

    #[test]
    fn what_no_cpu_address_or_number_can_be_is_refused() {
        // The Python reference takes any whole number for these; a patch never has one.
        assert_eq!(fault(assemble("org -1\nnop", &[])), "line 1: -1 is not a 24-bit address");
        assert_eq!(fault(assemble("org $1000000\nnop", &[])), "line 1: $1000000 is not a 24-bit address");
        assert_eq!(
            fault(code_with("lda $99999999999999999999", &[])),
            "line 2: $99999999999999999999 is too large a number"
        );
    }
}
