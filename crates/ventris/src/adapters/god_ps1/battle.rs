//! What the bosses say in battle, from the scripts of the battles the story sets, in the program's
//! file `SLPS_009.44`.
//!
//! A battle has a number, which the game keeps at $800E3114. Battles 200 and up have a script: the
//! table at $98048 of the file holds the address of each, for battles 200 to 255, and the routine at
//! $80034A6C reads the entry of battle `n` at `n - 200`. The scripts follow the table, one after
//! another, from $98128 to $9AC23. A script is commands of one byte, some with one or two bytes after
//! them, and strings written as the dialogue is (see `decode`), each up to its zero byte. Nothing
//! points at a string: where one starts is known only by reading the script from its start, command
//! by command, as the game does. The lengths below are those the game's own two readers give.
//!
//! As the battle starts, the routine at $80034A6C reads the script's first commands, through its
//! table at $80010510, up to a command it has no entry for, or a zero byte:
//!
//! - $01 and a string: shown as the battle starts, in place of the game's 「モンスターがあらわれた！！」.
//! - $02 and a string: shown when the party wins, in place of the game's own message. The routine
//!   at $80042B18 copies it, with the routine at $80033388, into the list of the battle's last
//!   messages at $800FDB28, which takes 48 bytes for each; the experience, the money and an item the
//!   party wins are written after it, 48 bytes on each time.
//! - $14 and a string: shown when the party loses, in place of 「…はまけてしまった・・・」. Only
//!   battles 238 to 240 have one.
//! - $05 and one byte, and $04, $08, $0D and $12 alone: settings of the battle.
//!
//! The routine that skips a string there, $800349E8, reads it as the dialogue's own routine does:
//! $01 to $0A and $0C take the next byte with them, $0F the next, and the one after if the next is a
//! zero byte.
//!
//! As the battle goes on, the routine at $80034C58 reads on from where the first left off, through
//! its table at $80010560, until a zero byte or a command it has no entry for, where it stays:
//!
//! - $03 waits until the first round is over, $07 and one byte until the count of rounds, which
//!   starts at 0 as the battle starts ($800E3098), is that number, and $0C does not wait. A string
//!   follows each, which the battle's window shows (the task at $800434C8).
//! - After such a string, $06 and two strings, as many times as there are: the game asks はい or
//!   いいえ, with its own string, and shows the first string for はい, the second for いいえ.
//! - $0B and one byte waits for that round, $13 and one byte until the first monster's hit points are
//!   below that many hundredths of its most; $05, $09, $0A and $10 with one byte, and $11 with two,
//!   make something happen (a sound, an attack); $0D, $0E, $0F and $12 are alone. No string follows
//!   any of them.
//!
//! Read that way, each script stops at a zero byte just before the next script, but six, where the
//! game stops earlier: the bytes left up to the next script are the end of an older version of the
//! text, which no routine reads. In battle 210 they hold a whole line, ダークソルジャー's last words.
//! They are not taken.
//!
//! The battle's window, which the routine at $80024480 opens for every string here, is 18 cells wide
//! and 4 lines high, each cell 12 pixels square, smaller than the field's 14. When a line reaches the
//! window's width, the game goes on at the start of the next line by itself; when the window is
//! full, it scrolls.
//!
//! The strings are section `battle`, each numbered by its offset in the program's file, which no later
//! finding changes. They are in no stretch of the text of section `program` (see `program::TEXT`).

use std::collections::BTreeMap;
use std::sync::Arc;

use super::program::{PROGRAM_BASE, program_file};
use super::{ByteCodes, image_error, read_text};
use crate::error::Result;
use crate::platforms::mips::number;
use crate::platforms::ps1::Disc;

pub const BATTLE_SECTION: &str = "battle";
/// Where the table of the scripts is in the program's file, and how many entries it has.
pub const SCRIPTS: usize = 0x9_8048;
pub const SCRIPT_COUNT: usize = 56;
/// The number of the battle the table's first entry is for.
pub const FIRST_BATTLE: u32 = 200;

/// The strings that answer the game's question, はい or いいえ: the one shown for はい, then the one
/// for いいえ, each named by its offset in the program's file. The text itself does not show it.
pub const ANSWERS: [(u32, u32); 5] =
    [(0x9_824F, 0x9_827E), (0x9_86A1, 0x9_86EA), (0x9_8A23, 0x9_8A3A), (0x9_8AA9, 0x9_8ADA), (0x9_8B5B, 0x9_8B72)];

/// The program's string the game asks its question with, はい and いいえ, as section `program` names it:
/// its offset in the file, $B6CA5.
pub const CHOICE: u32 = 0xB_6CA5;

/// When the battle shows a string of its script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// As the battle starts ($01).
    Start,
    /// When the party wins ($02).
    Won,
    /// When the party loses ($14).
    Lost,
    /// Once the first round is over ($03).
    AfterFirstRound,
    /// When the count of rounds is this number ($07).
    Round(u8),
    /// Without waiting, after what comes before ($0C).
    Next,
    /// After the game asks はい or いいえ, for はい ($06, the first of its two strings).
    Yes,
    /// The same, for いいえ.
    No,
}

/// One string of a battle's script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The battle's number, from 200.
    pub battle: u32,
    pub when: When,
    /// The string, without its zero byte.
    pub data: Vec<u8>,
}

/// What follows a command: how many bytes, and whether a string comes after them.
struct Follows {
    bytes: usize,
    string: Option<When>,
}

const fn bytes(bytes: usize) -> Option<Follows> {
    Some(Follows { bytes, string: None })
}

/// A command the battle reads as it starts (the routine at $80034A6C), and what follows it; none for
/// a byte where it stops.
fn first_command(command: u8) -> Option<Follows> {
    match command {
        0x01 => Some(Follows { bytes: 0, string: Some(When::Start) }),
        0x02 => Some(Follows { bytes: 0, string: Some(When::Won) }),
        0x14 => Some(Follows { bytes: 0, string: Some(When::Lost) }),
        0x05 => bytes(1),
        0x04 | 0x08 | 0x0D | 0x12 => bytes(0),
        _ => None,
    }
}

/// A command the battle reads as it goes on (the routine at $80034C58), with the byte after it, and
/// what follows it; none for a byte where it stops.
fn later_command(command: u8, after: Option<u8>) -> Option<Follows> {
    match command {
        0x03 => Some(Follows { bytes: 0, string: Some(When::AfterFirstRound) }),
        0x07 => Some(Follows { bytes: 1, string: Some(When::Round(after?)) }),
        0x0C => Some(Follows { bytes: 0, string: Some(When::Next) }),
        0x05 | 0x09 | 0x0A | 0x0B | 0x10 | 0x13 => bytes(1),
        0x11 => bytes(2),
        0x0D | 0x0E | 0x0F | 0x12 => bytes(0),
        _ => None,
    }
}

/// The command that puts a question after a string, with the two strings that answer it.
const QUESTION: u8 = 0x06;

/// A battle's script, read as the game reads it from `start` in `file`: each string, with where it
/// starts and when it is shown, and the offset of the byte where the game stops, a zero byte or a
/// command it does not know. It fails where a string or a command runs past the file, or a string
/// holds a byte that is no text, so that a string is never taken wrong unseen.
pub fn read_script(
    file: &[u8],
    start: usize,
    codes: &ByteCodes,
) -> std::result::Result<(Vec<(usize, When)>, usize), String> {
    let mut strings = Vec::new();
    let mut at = start;
    let byte = |at: usize| file.get(at).copied().ok_or_else(|| format!("its script runs past the file at ${at:X}"));
    // The string at `at`, up to its zero byte: where the next thing starts.
    let string = |at: usize, when: When, strings: &mut Vec<(usize, When)>| match read_text(&file[at..], codes) {
        Some((length, _)) => {
            strings.push((at, when));
            Ok(at + length + 1)
        }
        None => Err(format!("its string at ${at:X} is not text up to a zero byte")),
    };
    let mut first = true;
    loop {
        let command = byte(at)?;
        if command == 0 {
            return Ok((strings, at));
        }
        let follows = match first.then(|| first_command(command)).flatten() {
            Some(follows) => follows,
            None => {
                first = false;
                // A round's number is part of its command, so a script that ends before it is cut
                // short, not stopped.
                let after = if command == 0x07 { Some(byte(at + 1)?) } else { file.get(at + 1).copied() };
                match later_command(command, after) {
                    Some(follows) => follows,
                    None => return Ok((strings, at)),
                }
            }
        };
        at += 1 + follows.bytes;
        byte(at - 1)?;
        let Some(when) = follows.string else { continue };
        at = string(at, when, &mut strings)?;
        if !first {
            while byte(at)? == QUESTION {
                at = string(at + 1, When::Yes, &mut strings)?;
                at = string(at, When::No, &mut strings)?;
            }
        }
    }
}

/// Every battle's strings, each under its offset in the program's file.
pub type BattleText = BTreeMap<u32, Line>;

/// Every battle's strings, read once for an open disc.
pub fn battle_text(disc: &Disc) -> Result<Arc<BattleText>> {
    disc.keep("god-ps1 battle text", || {
        let file = program_file(disc)?;
        let codes = &super::program(disc)?.codes;
        read_battles(&file, codes)
    })
}

/// The strings of every battle's script, in a program's file.
pub fn read_battles(file: &[u8], codes: &ByteCodes) -> Result<BattleText> {
    let mut lines = BTreeMap::new();
    for entry in 0..SCRIPT_COUNT {
        let battle = FIRST_BATTLE + entry as u32;
        let start = number(file, SCRIPTS + 4 * entry)
            .and_then(|address| address.checked_sub(PROGRAM_BASE))
            .filter(|start| (*start as usize) < file.len());
        let Some(start) = start else {
            return image_error(format!("the program has no script for battle {battle} where its table says"));
        };
        let (strings, _) = match read_script(file, start as usize, codes) {
            Ok(read) => read,
            Err(problem) => return image_error(format!("battle {battle}: {problem}")),
        };
        for (at, when) in strings {
            let (length, _) = read_text(&file[at..], codes).unwrap_or_default();
            lines.insert(at as u32, Line { battle, when, data: file[at..at + length].to_vec() });
        }
    }
    Ok(lines)
}

/// What a translator is told about section `battle`: with no number, about the section's file, and
/// with one, about a string.
pub fn note(number: Option<u32>) -> Option<String> {
    let Some(number) = number else {
        return Some(
            "What the bosses say in battle, from the scripts of the battles the story sets (battles 200 to 255, in \
             the order of the program's file), and the messages that take the place of the battle's own when one \
             of them starts, is won or is lost. A string is named by where it is in the program's file, so the \
             numbers have gaps. It is written as the dialogue is, and shown in the battle's window: 4 lines of 18 \
             cells, each a little smaller than the field's. A line longer than the window goes on at the start of \
             the next by itself, and a full window scrolls. A speech starts with who speaks, on a line of its own, \
             then {SAY}; a {CLEAR} before the name starts it in an empty window. Several bosses speak in more than \
             one battle."
                .to_string(),
        );
    };
    ANSWERS.iter().find_map(|(yes, no)| {
        let (word, question) = match number {
            _ if number == *yes => ("はい", "just before"),
            _ if number == *no => ("いいえ", "two before"),
            _ => return None,
        };
        Some(format!(
            "What the boss says if the player answers {word} to the question the game asks after the string \
             {question}. The game's own はい and いいえ are program.{CHOICE}."
        ))
    })
}
