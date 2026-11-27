//! What the code of a MIPS program shows about where its data is, for the consoles whose processor
//! is one: the PlayStation and the PlayStation 2.
//!
//! A program's text is often found by what points at it. Some of that is 32-bit words in its
//! tables, which are easy to read. The rest is the code itself, which builds an address with two
//! instructions: `lui` gives a register the high half, and a later `addiu` or `ori` on that register
//! adds the low half. This module follows the code to find those.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::Range;

/// The instruction a function returns with, `jr $ra`.
pub const RETURN: u32 = 0x03E0_0008;
/// The instructions that build an address between them: `lui` gives a register its high half, and
/// an `addiu` or an `ori` on that register adds the low half.
pub const LUI: u32 = 0x0F;
pub const ADDIU: u32 = 0x09;
pub const ORI: u32 = 0x0D;
/// Instructions that load a value from memory into a register.
const LOADS: [u32; 11] = [0x1A, 0x1B, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x37];
/// In an instruction that works on registers alone, the two that jump to an address in a register:
/// they put nothing into the register the others put their result in.
const JUMPS: [u32; 2] = [0x08, 0x09];
/// Instructions that put into a register the result of arithmetic on a register and a number.
const NUMBERS: [u32; 9] = [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x18, 0x19];
/// The instruction that calls a function at a fixed address, and, among those that work on
/// registers alone, the one that calls a function whose address is in a register.
pub const CALL: u32 = 0x03;
const CALL_BY_REGISTER: u32 = 0x09;
/// The registers a function may change, by the convention the compilers of both consoles keep: after
/// a call they no longer hold what they held.
const CHANGED_BY_A_CALL: [usize; 18] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 31];
/// The PlayStation 2's own arithmetic, which puts its result where the instructions on registers
/// alone do. The PlayStation's processor has no such instruction.
const MULTIMEDIA: u32 = 0x1C;

/// The 32-bit number at `at` in `data`, little-endian, if `data` reaches that far.
pub fn number(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Whether an instruction reads or writes memory at a number added to a register.
fn is_memory(operation: u32) -> bool {
    matches!(operation, 0x1A | 0x1B | 0x1E | 0x1F | 0x20..=0x2E | 0x31 | 0x36 | 0x37 | 0x39 | 0x3E | 0x3F)
}

/// Whether an instruction moves a value from the registers of another unit into one of the 32.
fn is_move_in(word: u32) -> bool {
    matches!(word >> 26, 0x10..=0x12) && (word >> 21 & 31) <= 2
}

/// The places in `data` that something in it may point at, as offsets, `data` being loaded at
/// `base`: a 32-bit word that is an address inside it, or an `addiu` or `ori` that adds a low half to
/// a register some `lui` in it gives a high half.
///
/// A function sets the high half once, at its start, and adds low halves to it wherever it needs
/// an address, so the two instructions are not looked for side by side: every low half is tried
/// with every high half its register is ever given. That finds addresses nobody builds, too. What
/// is found here is therefore only where to look; `code_use` follows the code and finds less.
pub fn pointed_at(data: &[u8], base: u32) -> BTreeSet<usize> {
    let inside =
        |address: u32| address.checked_sub(base).map(|offset| offset as usize).filter(|offset| *offset < data.len());
    let words: Vec<u32> = (0..data.len() / 4).filter_map(|word| number(data, 4 * word)).collect();
    let mut found: BTreeSet<usize> = words.iter().filter_map(|word| inside(*word)).collect();
    let mut highs: [BTreeSet<u32>; 32] = Default::default();
    for word in words.iter().filter(|word| *word >> 26 == LUI) {
        highs[(word >> 16 & 31) as usize].insert((word & 0xFFFF) << 16);
    }
    for word in &words {
        let (operation, low) = (word >> 26, word & 0xFFFF);
        for high in &highs[(word >> 21 & 31) as usize] {
            let address = match operation {
                ADDIU => high.wrapping_add(low as u16 as i16 as u32), // this one adds its half as a signed number
                ORI => high | low,
                _ => break,
            };
            found.extend(inside(address));
        }
    }
    found
}

/// What a program's code does with the addresses of its data.
#[derive(Debug, Default, PartialEq)]
pub struct CodeUse {
    /// The addresses the code builds with two instructions, as its compiler has it: one gives a
    /// register the high half of an address, and a later one adds the low half. Each address is
    /// with the offsets of the instructions that add its low half.
    pub built: BTreeMap<u32, Vec<usize>>,
    /// The addresses the code reads or writes without building them: a load or a store through a
    /// register that holds a high half, the low half being in the load or the store itself.
    pub straight: BTreeSet<u32>,
}

/// What an instruction does to the order the code runs in.
#[derive(Clone, Copy, PartialEq)]
enum Way {
    /// Nothing: the next instruction follows.
    On,
    /// A branch that may be taken: the next instruction runs, then the target or what follows.
    Branch,
    /// A branch that runs the next instruction only when it is taken.
    Likely,
    /// A branch that is always taken, or a jump to a fixed address.
    Jump,
    /// A call: the next instruction runs, the function does, and the code goes on from there.
    Call,
    /// A return, or a jump to an address in a register: where the code goes on is not known.
    Away,
}

fn way_of(word: u32) -> Way {
    let (operation, source, target) = (word >> 26, word >> 21 & 31, word >> 16 & 31);
    match operation {
        0 if word & 0x3F == CALL_BY_REGISTER => Way::Call,
        0 if word & 0x3F == 0x08 => Way::Away,
        CALL => Way::Call,
        0x02 => Way::Jump,
        0x04 if source == target => Way::Jump, // equal to itself, which the compiler writes for "always"
        0x04..=0x07 => Way::Branch,
        0x14..=0x17 => Way::Likely,
        0x01 if matches!(target, 0x00 | 0x01) => Way::Branch,
        0x01 if matches!(target, 0x02 | 0x03) => Way::Likely,
        0x01 if matches!(target, 0x10..=0x13) => Way::Call,
        0x10..=0x12 if source == 0x08 => match target & 2 {
            0 => Way::Branch,
            _ => Way::Likely,
        },
        _ => Way::On,
    }
}

/// Follows the code at `code` of `data`, which runs at its offset plus `base`, along the ways it
/// can run, and keeps for every instruction the high halves each register may hold when the
/// instruction runs: those a `lui` gave it, on any way that leads there, that nothing has replaced
/// since. An `addiu` or an `ori` on such a register builds an address, and a load or a store
/// through it reads or writes one.
///
/// The ways: an instruction is followed by the next one. After a branch the next instruction still
/// runs, and then the target or the instruction after; a branch of the kind that runs the next
/// instruction only when taken leads to it and on to the target, or past it. A jump leads to its
/// target alone. After a call, and the instruction that follows it, the registers a function may
/// change hold nothing known. A return leads nowhere, and neither does a jump through a register,
/// whose targets are not followed: taking them to be every place a table names made addresses out
/// of numbers.
///
/// Reading the code in the order of the file instead, as was done first, loses a register at every
/// branch: of the references to text that a fan translation of Tales of Destiny 2 changed, it missed
/// nine.
pub fn code_use(data: &[u8], code: Range<usize>, base: u32) -> CodeUse {
    let words: Vec<u32> = code.clone().step_by(4).map_while(|at| number(data, at)).collect();
    let ways: Vec<Way> = words.iter().map(|word| way_of(*word)).collect();
    let count = words.len();
    // Where the branch or the jump at instruction `at` leads. A jump gives the low 28 bits of its
    // target; the top 4 are those of the instruction after it, which matters where code runs high in
    // memory, as on the PlayStation (0x80000000 up).
    let target = |at: usize| {
        let to = match words[at] >> 26 {
            0x02 => {
                let next = i64::from(base) + code.start as i64 + 4 * at as i64 + 4;
                let destination = (next & 0xF000_0000) | i64::from((words[at] & 0x03FF_FFFF) << 2);
                (destination - i64::from(base) - code.start as i64) / 4
            }
            _ => at as i64 + 1 + i64::from(words[at] as u16 as i16),
        };
        (0..count as i64).contains(&to).then_some(to as usize)
    };
    // The instructions that may run after instruction `at`.
    let after = |at: usize| -> [Option<usize>; 2] {
        let before = if at == 0 { Way::On } else { ways[at - 1] };
        match before {
            Way::Away => [None, None],
            Way::Jump | Way::Likely => [target(at - 1), None],
            Way::Branch => [target(at - 1), Some(at + 1)],
            Way::On | Way::Call => [Some(at + 1), (ways[at] == Way::Likely).then_some(at + 2)],
        }
    };
    // What the registers may hold after instruction `at`, given what they may hold before it: a
    // sorted list of a register and a high half.
    let apply = |at: usize, held: &[(u8, u16)]| -> Vec<(u8, u16)> {
        let word = words[at];
        let (operation, target, result) = (word >> 26, (word >> 16 & 31) as u8, (word >> 11 & 31) as u8);
        let written = match operation {
            LUI => Some(target),
            _ if NUMBERS.contains(&operation) || LOADS.contains(&operation) || is_move_in(word) => Some(target),
            0 if !JUMPS.contains(&(word & 0x3F)) => Some(result),
            MULTIMEDIA => Some(result),
            _ => None,
        };
        let called = at > 0 && ways[at - 1] == Way::Call;
        let lost = |register: u8| Some(register) == written || (called && CHANGED_BY_A_CALL.contains(&register.into()));
        let mut out: Vec<(u8, u16)> = held.iter().copied().filter(|(register, _)| !lost(*register)).collect();
        if operation == LUI && target != 0 {
            out.push((target, word as u16));
            out.sort_unstable();
        }
        out
    };
    let mut held: Vec<Vec<(u8, u16)>> = vec![Vec::new(); count];
    let mut waiting: VecDeque<usize> = (0..count).collect();
    let mut queued = vec![true; count];
    while let Some(at) = waiting.pop_front() {
        queued[at] = false;
        let out = apply(at, &held[at]);
        for next in after(at).into_iter().flatten().filter(|next| *next < count) {
            if out.iter().all(|pair| held[next].binary_search(pair).is_ok()) {
                continue;
            }
            held[next].extend(&out);
            held[next].sort_unstable();
            held[next].dedup();
            if !queued[next] {
                queued[next] = true;
                waiting.push_back(next);
            }
        }
    }
    let mut found = CodeUse::default();
    for (at, word) in words.iter().enumerate() {
        let (operation, source, low) = (word >> 26, (word >> 21 & 31) as u8, word & 0xFFFF);
        let signed = low as u16 as i16 as u32; // `addiu`, a load and a store add their half as a signed number
        for (_, half) in held[at].iter().filter(|(register, _)| *register == source) {
            let half = u32::from(*half) << 16;
            match operation {
                ADDIU => found.built.entry(half.wrapping_add(signed)).or_default().push(code.start + 4 * at),
                ORI => found.built.entry(half | low).or_default().push(code.start + 4 * at),
                _ if is_memory(operation) => {
                    found.straight.insert(half.wrapping_add(signed));
                }
                _ => {}
            }
        }
    }
    found
}

/// The offsets in `data`, which is loaded at `base`, that the string at `offset` can be put at with
/// the instructions at `code` still building its address. Each of them adds a low half to a high
/// half that another instruction gives; that one may serve other addresses too and is left as it
/// is, so the string stays within the 65,536 bytes its high half reaches.
pub fn within_reach(data: &[u8], base: u32, offset: usize, code: &[usize]) -> Range<usize> {
    let address = offset as i64 + i64::from(base);
    let mut reach = 0..usize::MAX;
    for at in code {
        let word = number(data, *at).unwrap_or(0);
        // `addiu` adds its half as a signed number, and so reaches 32,768 bytes to either side.
        let first = match word >> 26 == ADDIU {
            true => address - i64::from(word as u16 as i16) - 0x8000,
            false => address & !0xFFFF,
        };
        // The window's end is counted before its start is kept from going below the data's: one
        // that starts below the base still ends where its high half's reach does.
        let first = first - i64::from(base);
        let (start, end) = (first.max(0) as usize, (first + 0x10000).max(0) as usize);
        reach = reach.start.max(start)..reach.end.min(end);
    }
    reach
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Code that runs high in memory, as a PlayStation program does from 0x80010000: a `lui` gives
    /// `a0` the high half of an address, a `j` jumps over an instruction that would replace it, and
    /// the `addiu` at the jump's target adds the low half. The jump's target keeps the top bits of
    /// its own address, so the address is found.
    #[test]
    fn an_address_built_across_a_jump_is_found_in_code_that_runs_high_in_memory() {
        let base = 0x8000_F800;
        let start = 0x800;
        let target = base + start as u32 + 4 * 4;
        let words = [
            0x3C04_800B,                                 // lui a0, 0x800B
            0x0800_0000 | ((target & 0x0FFF_FFFF) >> 2), // j target
            0x0000_0000,                                 // nop, in the jump's delay slot
            0x3C04_1111,                                 // lui a0, 0x1111, jumped over
            0x2484_1234,                                 // addiu a0, a0, 0x1234
        ];
        let mut data = vec![0; start];
        data.extend(words.iter().flat_map(|word: &u32| word.to_le_bytes()));
        let found = code_use(&data, start..data.len(), base);
        assert_eq!(found.built.get(&0x800B_1234), Some(&vec![start + 16]));
    }

    /// A string near the start of what is loaded high in memory: the reach of its high half starts
    /// below the data, and ends where that half's reach ends, not 65,536 bytes past the data's start.
    #[test]
    fn a_reach_that_starts_below_the_data_keeps_its_end() {
        let base = 0x8000_F800;
        let mut data = vec![0; 0x1000];
        // `ori a0, a0, 0xFC00` builds the address of offset $400 on the high half $8000.
        data[0x900..0x904].copy_from_slice(&(ORI << 26 | 4 << 21 | 4 << 16 | 0xFC00).to_le_bytes());
        assert_eq!(within_reach(&data, base, 0x400, &[0x900]), 0..0x800);
        // `addiu a0, a0, 0x0400` builds that of offset $C00 on $8001, which reaches $8000 bytes down.
        data[0x904..0x908].copy_from_slice(&(ADDIU << 26 | 4 << 21 | 4 << 16 | 0x0400).to_le_bytes());
        assert_eq!(within_reach(&data, base, 0xC00, &[0x904]), 0..0x8800);
    }
}
