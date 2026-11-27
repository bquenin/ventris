//! The text the program holds itself, in its file `SLPS_009.44`.
//!
//! The file is a PlayStation program: a header of $800 bytes, then what is loaded at $80010000, so
//! the address of a byte is its offset in the file plus $8000F800. Just past the header is some of
//! its read-only data, up to $31AC, where its first function, `main`, starts: jump tables, messages
//! for its programmers, the names of its movies, and a few labels of the menus (see `READ_ONLY`).
//! Its code runs from there to $7F4C8 of the file, just past its last return; the rest of its data
//! follows.
//!
//! Two routines draw the program's text, and each reads its own encoding.
//!
//! Most of it is written as the dialogue is (see `decode`), and drawn with the dialogue's font. It
//! is section `program`. Nothing lists these strings, but something points at each one: a 32-bit
//! word in a table, which is an address, or the code itself, which builds the address with two
//! instructions (see `mips::code_use`). Many 32-bit words of the program hold such a value and are
//! no pointer at text: a table of shops points at each shop's record, a table of monsters at each
//! monster's, and the first bytes of a record may read as a character or two. So a string is taken
//! to be what is pointed at, in one of the stretches of the data where the text is (see `TEXT`),
//! that reads as text up to a zero byte, draws a character and only characters the table of
//! characters has, and does not start inside the string before it. In those stretches the strings follow one another, each after the zero byte
//! of the one before or after the table that points at them. A string is named by its offset in the
//! file, which no later finding changes.
//!
//! The labels of the main menu and of the screens it opens (items, equipment, chakras, the
//! configuration) are drawn another way, by the routine at $800583C0, from the small font of 8 by 8
//! pixels (member 3 of the archive). They are written in the single bytes of Shift-JIS: ASCII, and
//! the half-width katakana, where a voiced sound is the letter and then ﾞ or ﾟ. An `@` switches
//! between katakana and hiragana, which the small font also has; a label starts in katakana. The
//! small font draws a lowercase letter as its capital. These labels are section `menu`, written
//! in a script as kana of the width the game's own text has, the hiragana as hiragana (see
//! `decode_small`). Most of them sit in arrays of a fixed size per label, which the code indexes, so
//! nothing points at those but the first: a label is what the code builds the address of, outside
//! the stretches of the dialogue's text, that reads as such text with a kana letter in it, and the
//! labels that follow it with nothing but zero bytes between, with a kana letter or an `@` in each.
//! Labels written in ASCII alone are not taken: the same part of the data holds the names of files
//! and the formats of numbers, which look the same. Most labels are in the data past the code; a
//! few are in the read-only data before it, each built by the code on its own: the shop's commands
//! (buy, sell, leave), its yes and no, its count of what the party has, the monster book's count of
//! a monster beaten, and a count of turns left. A label that holds a format, such as `%8dコ`, is
//! filled in by the code before it is drawn, and a pack's must keep its conversions (see
//! `conversions`).
//!
//! Putting changed text back. The program's file cannot grow: it fills its place on the disc, up
//! to the sector where the archive starts, and the memory after what it loads holds the game's
//! variables and then its heap. Nor has it free room: its runs of zero bytes are buffers its code
//! uses. So the strings of section `program` that a pack changes share the room they had between
//! them (see `rooms` and `write_strings`): they are written back one after another, and what refers
//! to one that moved is given its new address, a 32-bit word the address and an instruction the new
//! low half. Strings given the same text are written once, where one copy is within reach of all
//! of them, which matters here, since the game keeps many copies of the same text (the monster
//! book's descriptions above all): the 3,303 strings have 70,402 bytes of room between them, of
//! which a third are such copies. A pack that changes a few
//! scattered strings has only their room, and a string that finds no place is refused, with what
//! each needs. The game was seen to show strings moved this way: the title screen's menu, a word of
//! the status screen and the hero's name, each built by code, read from a table, or copied at a new
//! game.
//!
//! A changed string must also fit the screen. Each part of the section has a width in cells, the
//! widest line of the game's own there, and a number of lines (see `PARTS`); lines are broken at
//! spaces to the width, and what still does not fit is refused. A line is measured in the pixels of
//! the small font, 12 a cell, which some parts are drawn in: Latin letters are drawn at their own
//! widths here as in the dialogue (see `insert`), as wide in both fonts, and a glyph of the game's is
//! narrower in the small font, so a line that fits there fits in the dialogue's font too. A few
//! strings are in a box the game made as wide as its own text, and keep to that (see `OWN_WIDTH`).
//! The party's names are copied into records of 16 bytes when a new game starts, and the menus
//! draw them in a small font that has no lowercase letters, so a name has at most 15 bytes and is written in capitals, a character to a cell; the name entry
//! gives the player 6 characters, which the screens that show a name have room for, and which is
//! what the dialogue counts for a name it fills in.
//!
//! A label of the menus stays in its slot, since the code finds it a set number of bytes from the
//! first of its screen (see `label_room`), and draws no more characters than the widest of the
//! game's own labels on its screen (see `label_width`). That is a guess at the room its box or
//! column has, not a measure of it, so a label within it is not sure to show whole, and a box may
//! have more room, as the main menu's was seen to have for 8 characters. The small font draws a
//! lowercase letter as its capital, so a pack's are written as capitals.
//!
//! The battles' scripts, from $98128 of the file to $9AC23, which a table at $98048 lists, hold what
//! the bosses say in battle, written as the dialogue is among commands of their own. They are read
//! in `battle`, as section `battle`, and are in none of the stretches above. Not covered yet: the
//! pieces of text that follow some strings, which nothing points at (see `rooms`), are not strings
//! of the section, so a pack cannot change them yet; they move with the string before them.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use super::{ByteCodes, PROGRAM, every_character_known, read_text};
use crate::error::{Result, text_error};
use crate::placement::{Shared, pack_room, pack_room_fewest_places_first, share_room};
use crate::platforms::mips::{CodeUse, code_use, number, within_reach};
use crate::platforms::ps1::Disc;
use crate::script::string_name;
use crate::text::{Token, quoted, tag_at, tokens};

/// What is added to an offset in the program's file to get the address it is loaded at.
pub const PROGRAM_BASE: u32 = 0x8000_F800;
/// Where the program's code is in its file: from the end of the header to just past the last
/// instruction that returns from a function.
pub const PROGRAM_CODE: Range<usize> = 0x800..0x7_F4C8;
/// The read-only data the program's file has before its code, from the end of its header to `main`.
/// The code builds the address of a few labels of the menus here (see `menu_strings`).
pub const READ_ONLY: Range<usize> = 0x800..0x31AC;
/// The stretches of the program's file where the text written as dialogue is: the kinds of shop;
/// the shopkeepers' lines and the monster book; the souvenirs; and the rest, from the professor's
/// notebook to the rumours. Between them are tables and records, and the menus' labels.
pub const TEXT: [Range<usize>; 4] = [0x9_F2B8..0x9_F2EC, 0xA_0954..0xA_79AD, 0xA_8A60..0xA_8EA2, 0xA_9088..0xB_90D0];
/// The most zero bytes between two labels of one array of the menus. The configuration's labels
/// are 35 bytes apart, the longest 13 of them.
const MENU_GAP: usize = 32;
/// The byte that switches the small font between katakana and hiragana.
const SWITCH: u8 = b'@';

/// What a translator is told about the program's text. The room is what all the section's strings
/// have between them (see `rooms`), counted by a test.
pub const LIMITS: &str = "a line is no wider than the note on the first string of its part gives, the widest the \
                          game's own lines there take, in cells of 12 pixels; Latin letters, digits, spaces and \
                          punctuation are each drawn at their own width, from 2 to 9 pixels and 4 for a space (12 \
                          each with the build option full-width), and a kana, a kanji or a mark of the game's own \
                          font takes 12, counting a name the game fills in as 6 cells, a number as 6 and a word as \
                          8; lines are broken at spaces to fit (with wrapping off, a wider one is refused); a string \
                          has no more lines than the game's own, but a shopkeeper's line may have 2, a description \
                          in the monster book 5, and the help pages, the chakra lessons and the diary may run on; a choice, such as yes and no, is no wider than the game's own, its box \
                          being made for that; a name of the party takes at most 6 characters, a cell each, and 15 \
                          bytes, in capitals; a monster's name takes at most 17 bytes, the battle putting a space \
                          and a letter after it; a place's name takes at most 31 bytes, the caption that names an area \
                          copying it into 32; the strings a pack changes share the room they had between them, 70,402 \
                          bytes if every one changes, where a letter drawn at its own width takes one byte and each \
                          run of them one more, the pairs of letters a pack writes most taking a byte for two, a \
                          kanji two, and a kana, or a letter written a cell each, one, and \
                          strings given the same text are written once, unless the code that finds them cannot reach \
                          one place from all of them";

/// What a translator is told about the menus' labels.
pub const MENU_LIMITS: &str = "a label stays in its slot, and takes no more bytes than the game left for it there, the \
                               game's own and the unused ones after it: a letter, a digit, a space, a kana or a mark \
                               takes one, so a kana with ﾞ or ﾟ takes two; nor does it draw more characters than the \
                               widest of the game's own labels on its screen, ﾞ and ﾟ being drawn over the letter \
                               before, but the main menu's may draw 8; the small font has capitals, digits and \
                               !\"#$%&'()*+,-./:;<=>?[] besides the kana, and lowercase letters are written as capitals; \
                               a label that holds a format the game fills in, such as %8d, keeps the game's own, in \
                               the same order";

/// The two sections of the program's text.
pub const PROGRAM_SECTION: &str = "program";
pub const MENU_SECTION: &str = "menu";

/// How many lines a changed string of a part may have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lines {
    /// No more than the game's own string has: a field of one line, or a list of choices whose
    /// cursor goes from line to line.
    Own,
    /// Up to this many, whatever the game's own has: running text in a box of that many lines.
    Most(usize),
    /// A string of one line stays one line, being a title or an entry of a list; one of several
    /// lines may have up to this many, the most any of the game's own has.
    Running(usize),
}

/// A part of section `program`: where its first string is, what a translator is told there, and
/// the room a line has on screen, in cells, and how many lines a string may have.
pub struct Part {
    pub start: u32,
    pub about: &'static str,
    pub width: usize,
    pub lines: Lines,
}

const fn part(start: u32, about: &'static str, width: usize, lines: Lines) -> Part {
    Part { start, about, width, lines }
}

/// The parts of section `program`, in the order of the file. Each runs to the next. The width of
/// each is that of the widest line of the game's own strings there, counted as the dialogue's lines
/// are (a name the game fills in as 6 cells, a number as 6, a word as 8), and the lines are what
/// its strings show: so a changed string takes no more room on screen than the game's own were
/// seen to have. The names of the party are the exception: the name entry lets the player give a
/// name of 6 characters, so the screens that show one have room for that.
pub const PARTS: [Part; 25] = [
    part(0x9_F2B8, "The kinds of shop, as the game names them.", 5, Lines::Own),
    part(
        0xA_0954,
        "The shopkeepers' lines: each of the 68 shops has a set of nine, one of each kind, and many are in a          dialect. A note on each says which kind it is.",
        13,
        Lines::Most(2),
    ),
    part(
        0xA_0F4C,
        "The labels of the monster book, one per line, each padded with dots to six characters: type, height,          weight, experience, cosmo stones, money, weakness, where it is met.",
        6,
        Lines::Own,
    ),
    part(
        0xA_2348,
        "The monster book's entries: each monster's type, its height, its weight, its weakness and the rest.          ？？？？？？ is what the book shows where it does not say.",
        6,
        Lines::Own,
    ),
    part(
        0xA_3930,
        "The monster book's descriptions. Most monsters have the same description, each its own copy; copies a          pack gives the same text are written once.",
        12,
        Lines::Most(5),
    ),
    part(0xA_8A60, "What the souvenirs are.", 19, Lines::Own),
    part(
        0xA_9088,
        "The professor's notebook and the help pages it opens (psychic powers, the chakras, the game itself).",
        17,
        Lines::Running(109),
    ),
    part(0xA_AA34, "The chakra screen's lessons.", 16, Lines::Running(18)),
    part(0xA_B350, "The names of the party, as a new game gives them.", 6, Lines::Own),
    part(
        0xA_B371,
        "Choices, and messages of the field: something found, money found, a monster appears.",
        24,
        Lines::Own,
    ),
    part(
        0xA_B4AB,
        "The status screen's words: strength, defence, spirit, speed, love, experience, and the figures worked out          from them.",
        7,
        Lines::Own,
    ),
    part(0xA_B567, "The important things: the name of each, then what it is.", 19, Lines::Own),
    part(
        0xA_BA50,
        "The name entry's prompt, the menus of the notebook and of the diary, and the memory card's messages.",
        19,
        Lines::Own,
    ),
    part(0xA_DD04, "The names of items and equipment.", 6, Lines::Own),
    part(0xA_ED28, "What each item and piece of equipment does.", 19, Lines::Own),
    part(0xB_0B2C, "The seven chakras, one character each.", 1, Lines::Own),
    part(0xB_0D2E, "The psychic powers: the names, then what each does.", 19, Lines::Own),
    part(0xB_1D50, "The names of the monsters.", 9, Lines::Own),
    part(0xB_2AB0, "The names of places.", 8, Lines::Own),
    part(0xB_30C4, "The diary's entries, as the hero writes them.", 17, Lines::Running(20)),
    part(
        0xB_64C4,
        "The titles of the diary's entries, between 『 and 』. The game's own put a wide space after the          seventh character, where the title's line breaks.",
        15,
        Lines::Own,
    ),
    part(
        0xB_6A9C,
        "The battle's commands and messages. {NAME:19} is the name of whoever acts, {0C:0A} a monster's,          {WORD} a thing's or a power's, {NUM} a number.",
        28,
        Lines::Own,
    ),
    part(0xB_775C, "What monsters do in battle, as the battle's messages say it.", 17, Lines::Own),
    part(0xB_8894, "How a monster looks at the party, which the game puts before what it does.", 14, Lines::Own),
    part(0xB_8B68, "The rumours an informant tells.", 22, Lines::Own),
];

/// Where the names of the monsters are: a battle copies each it meets into a record of 20 bytes.
pub const MONSTER_NAMES: Range<u32> = 0xB_1D50..0xB_2AB0;

/// Where the names of places are: the caption that names an area the first time it is entered copies
/// one into 32 bytes on the stack ($80031EE8).
pub const PLACE_NAMES: Range<u32> = 0xB_2AB0..0xB_30C4;

/// Where the names of the party are: a new game copies each into a record of its own.
pub const NAMES: u32 = 0xA_B350;
pub const NAMES_END: u32 = 0xA_B371;

/// The part a string of section `program` is in.
pub fn part_of(offset: u32) -> &'static Part {
    PARTS.iter().rev().find(|part| part.start <= offset).unwrap_or(&PARTS[0])
}

impl Part {
    /// What a translator is told of the room a string of this part has.
    pub fn room(&self) -> String {
        let lines = match self.lines {
            Lines::Own => "a string has no more lines than the game's own".to_string(),
            Lines::Most(most) => format!("a string has at most {most} lines"),
            Lines::Running(most) => {
                format!("a string of one line stays one line, and one of several lines has at most {most}")
            }
        };
        let width = match self.width {
            1 => "1 cell".to_string(),
            width => format!("{width} cells"),
        };
        let drawn = match self.start {
            NAMES => ", each character taking a cell",
            _ => " of 12 pixels, Latin letters, digits, spaces and marks each at its own width",
        };
        format!("Lines are at most {width}{drawn}, and {lines}.")
    }
}

/// The kinds of a shopkeeper's line, in the order of a shop's record, each with the offset of the
/// first line of that kind: the compiler put the lines of one kind together.
pub const SHOP_LINES: [(u32, &str); 9] = [
    (0xA_0954, "the greeting"),
    (0xA_0A38, "asking what the player will buy"),
    (0xA_0AC2, "asking whether the player will buy more"),
    (0xA_0B53, "the player has not got the money"),
    (0xA_0C0A, "the player cannot carry more"),
    (0xA_0CD5, "asking what the player will sell"),
    (0xA_0D5A, "asking whether the player will sell more"),
    (0xA_0DE7, "there is nothing the player can buy"),
    (0xA_0EAC, "the price, which {0C:10} fills in, and asking whether that is all right"),
];
/// Where the shopkeepers' lines end: the monster book's labels follow.
const SHOP_LINES_END: u32 = 0xA_0F4C;
/// The table of the shops: a pointer at each shop's record, 68 of them. A record is a number, a
/// pointer at the list of what the shop sells, and a pointer at each of its nine lines.
pub const SHOPS: usize = 0x9_F2E4;
pub const SHOP_COUNT: usize = 68;

/// The program's file, read once for an open disc.
pub fn program_file(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("god-ps1 program file", || disc.read(PROGRAM, 0, disc.size(PROGRAM)?))
}

/// The strings of each of the program's sections, each under its offset in the file, without the
/// zero byte that ends it.
pub type ProgramText = BTreeMap<&'static str, BTreeMap<u32, Vec<u8>>>;

/// The program's text, read once for an open disc.
pub fn program_text(disc: &Disc) -> Result<Arc<ProgramText>> {
    disc.keep("god-ps1 program text", || {
        let file = program_file(disc)?;
        let codes = &super::program(disc)?.codes;
        let built: BTreeSet<usize> = built_targets(&file);
        let strings = program_strings(&file, &built, codes);
        let labels = menu_strings(&file, &built);
        Ok(BTreeMap::from([(PROGRAM_SECTION, strings), (MENU_SECTION, labels)]))
    })
}

/// The offset in the file of an address, if the file holds it.
fn offset_of(file: &[u8], address: u32) -> Option<usize> {
    let offset = address.checked_sub(PROGRAM_BASE)? as usize;
    (offset < file.len()).then_some(offset)
}

/// The places in the program's data whose address its code builds, as offsets in its file: in the
/// data past its code, or in the read-only data before it.
pub fn built_targets(file: &[u8]) -> BTreeSet<usize> {
    let built = code_use(file, PROGRAM_CODE, PROGRAM_BASE).built;
    let data = |at: &usize| *at >= PROGRAM_CODE.end || READ_ONLY.contains(at);
    built.into_keys().filter_map(|address| offset_of(file, address)).filter(data).collect()
}

/// The places in the program's data that one of its 32-bit words points at, as offsets in its file.
pub fn word_targets(file: &[u8]) -> BTreeSet<usize> {
    let words = (0..file.len() / 4).filter_map(|word| number(file, 4 * word));
    words.filter_map(|word| offset_of(file, word)).filter(|at| *at >= PROGRAM_CODE.end).collect()
}

fn in_text(at: usize) -> bool {
    TEXT.iter().any(|stretch| stretch.contains(&at))
}

/// The strings of section `program`: what one of the program's words points at, or what its code
/// builds the address of (`built`), in the stretches of its text, that reads as text, draws a
/// character and only characters the table has, and does not start inside the string before it.
/// The records among the text (a monster's, a shop's) start with numbers, which can read as a code
/// or as a byte that draws nothing; no string of the program has either.
pub fn program_strings(file: &[u8], built: &BTreeSet<usize>, codes: &ByteCodes) -> BTreeMap<u32, Vec<u8>> {
    let mut strings = BTreeMap::new();
    let mut free = 0; // just past the zero byte of the string found last
    let targets: BTreeSet<usize> = word_targets(file).into_iter().chain(built.iter().copied()).collect();
    for at in targets.into_iter().filter(|at| in_text(*at)) {
        if at < free {
            continue;
        }
        let Some((length, characters)) = read_text(&file[at..], codes) else { continue };
        if characters == 0 || !every_character_known(&file[at..at + length], codes) {
            continue;
        }
        strings.insert(at as u32, file[at..at + length].to_vec());
        free = at + length + 1;
    }
    strings
}

/// A byte the small font's routine reads as a character: ASCII but for the braces, which a script
/// keeps for its tags, and the half-width katakana with their punctuation.
fn is_small(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7E if byte != b'{' && byte != b'}') || (0xA1..=0xDF).contains(&byte)
}

/// A half-width katakana letter: one the switch to hiragana changes. ｰ is the same in both.
fn is_kana_letter(byte: u8) -> bool {
    matches!(byte, 0xA6..=0xAF | 0xB1..=0xDD)
}

/// How long the label at the start of `data` is, up to its zero byte, if it is one: bytes the small
/// font's routine reads as characters, with a kana letter among them, or with `@` if `switch` is
/// allowed in place of one, and a character drawn. An `@` alone draws nothing: the shop's commands
/// are followed by a table of buttons whose first number is $0040, which reads as one.
fn small_length(data: &[u8], switch: bool) -> Option<usize> {
    let length = data.iter().position(|byte| *byte == 0)?;
    let label = &data[..length];
    let letters = label.iter().any(|byte| is_kana_letter(*byte)) || (switch && label.contains(&SWITCH));
    let drawn = label_cells(label) > 0;
    (letters && drawn && label.iter().all(|byte| is_small(*byte))).then_some(length)
}

/// The labels of section `menu`: what the program's code builds the address of (`built`), in its
/// data outside the stretches of the dialogue's text, that reads as a label with a kana letter in
/// it, and the labels that follow such a one with nothing but zero bytes between, each with a kana
/// letter or an `@`. The labels of one screen are an array, each in a slot of the same size, and
/// the code builds the address of the array's first alone.
pub fn menu_strings(file: &[u8], built: &BTreeSet<usize>) -> BTreeMap<u32, Vec<u8>> {
    let mut labels = BTreeMap::new();
    let mut free = 0;
    for start in built.iter().copied().filter(|at| !in_text(*at)) {
        if start < free {
            continue;
        }
        let Some(length) = small_length(&file[start..], false) else { continue };
        labels.insert(start as u32, file[start..start + length].to_vec());
        let mut end = start + length + 1;
        while let Some(gap) = file[end..].iter().position(|byte| *byte != 0) {
            let next = end + gap;
            if gap > MENU_GAP || in_text(next) {
                break;
            }
            let Some(length) = small_length(&file[next..], true) else { break };
            labels.insert(next as u32, file[next..next + length].to_vec());
            end = next + length + 1;
        }
        free = end;
    }
    labels
}

// --- the small font's labels ---------------------------------------------------

/// The characters of the half-width bytes from $A1 to $DF, as the game's own text writes them.
const HALF_WIDTH: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン゛゜";
const FIRST_HALF_WIDTH: u8 = 0xA1;
const VOICED: u8 = 0xDE;
const SEMI_VOICED: u8 = 0xDF;
/// The tag for an `@` that a script cannot show as hiragana: one that comes before something other
/// than a kana letter.
const SWITCH_TAG: &str = "KANA";

fn half_width(byte: u8) -> Option<char> {
    HALF_WIDTH.chars().nth(usize::from(byte.checked_sub(FIRST_HALF_WIDTH)?))
}

fn half_width_byte(char: char) -> Option<u8> {
    HALF_WIDTH.chars().position(|known| known == char).map(|index| FIRST_HALF_WIDTH + index as u8)
}

/// The katakana a letter and ﾞ make, if they make one.
fn voiced(char: char) -> Option<char> {
    match char {
        'カ'..='ト' if "カキクケコサシスセソタチツテト".contains(char) => {
            char::from_u32(char as u32 + 1)
        }
        'ハ'..='ホ' if "ハヒフヘホ".contains(char) => char::from_u32(char as u32 + 1),
        'ウ' => Some('ヴ'),
        _ => None,
    }
}

/// The katakana a letter and ﾟ make, if they make one.
fn semi_voiced(char: char) -> Option<char> {
    "ハヒフヘホ".contains(char).then(|| char::from_u32(char as u32 + 2)).flatten()
}

/// A katakana letter and the mark it was made with, if it was: ガ is カ and ﾞ.
fn split_voiced(char: char) -> (char, Option<u8>) {
    let before = |offset: u32| (char as u32).checked_sub(offset).and_then(char::from_u32).unwrap_or(char);
    match char {
        'ヴ' => ('ウ', Some(VOICED)),
        _ if voiced(before(1)) == Some(char) => (before(1), Some(VOICED)),
        _ if semi_voiced(before(2)) == Some(char) => (before(2), Some(SEMI_VOICED)),
        _ => (char, None),
    }
}

const KATAKANA: std::ops::RangeInclusive<char> = 'ァ'..='ヴ';
const HIRAGANA: std::ops::RangeInclusive<char> = 'ぁ'..='ゔ';
/// How far apart a katakana and its hiragana are in Unicode.
const TO_HIRAGANA: u32 = 0x60;

/// A label of the small font, without its zero byte, as script text: half-width katakana as the
/// katakana of the game's own text, a letter and its ﾞ or ﾟ as one character, and what follows an
/// `@` as hiragana until the next. An `@` before anything but a kana letter is `{KANA}`.
pub fn decode_small(data: &[u8]) -> String {
    let mut out = String::new();
    let mut hiragana = false;
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        let next = data.get(at + 1).copied();
        at += 1;
        if byte == SWITCH {
            if !next.is_some_and(is_kana_letter) {
                out.push_str(&format!("{{{SWITCH_TAG}}}"));
            }
            hiragana = !hiragana;
        } else if is_kana_letter(byte) {
            let letter = half_width(byte).unwrap_or('?');
            let made = match next {
                Some(VOICED) => voiced(letter),
                Some(SEMI_VOICED) => semi_voiced(letter),
                _ => None,
            };
            at += usize::from(made.is_some());
            let letter = made.unwrap_or(letter);
            let shown = if hiragana { char::from_u32(letter as u32 - TO_HIRAGANA) } else { Some(letter) };
            out.push(shown.unwrap_or(letter));
        } else if let Some(char) = half_width(byte) {
            out.push(char);
        } else if is_small(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("{{B:{byte:02X}}}"));
        }
    }
    out
}

/// Script text as the bytes of a label of the small font, without its zero byte.
pub fn encode_small(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut hiragana = false;
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        if char == '{' {
            let Some(Token::Tag { raw, name, value }) = tag_at(rest) else {
                let shown: String = rest.chars().take(16).collect();
                return text_error(format!("malformed tag: {}", quoted(&shown)));
            };
            match (name, value.and_then(crate::text::from_hex).as_deref()) {
                (SWITCH_TAG, None) if value.is_none() => {
                    out.push(SWITCH);
                    hiragana = !hiragana;
                }
                ("B", Some([byte])) if !is_small(*byte) && *byte != 0 => out.push(*byte),
                _ => return text_error(format!("unknown tag {raw} in a label of the small font")),
            }
            rest = &rest[raw.len()..];
            continue;
        }
        let (katakana, wanted) = match char {
            _ if HIRAGANA.contains(&char) => (char::from_u32(char as u32 + TO_HIRAGANA).unwrap_or(char), Some(true)),
            _ if KATAKANA.contains(&char) => (char, Some(false)),
            _ => (char, None),
        };
        let (letter, mark) = split_voiced(katakana);
        let byte = match half_width_byte(letter) {
            Some(byte) => byte,
            None if char.is_ascii() && is_small(char as u8) && char as u8 != SWITCH => char as u8,
            None if char as u32 == u32::from(SWITCH) => {
                return text_error(format!(
                    "{} is not drawn in a label of the small font: {{{SWITCH_TAG}}} switches between katakana and \
                     hiragana",
                    quoted("@")
                ));
            }
            None => {
                return text_error(format!(
                    "character {} is not in the small font, which has ASCII and the kana",
                    quoted(&char.to_string())
                ));
            }
        };
        if let Some(wanted) = wanted.filter(|wanted| is_kana_letter(byte) && *wanted != hiragana) {
            out.push(SWITCH);
            hiragana = wanted;
        }
        out.push(byte);
        out.extend(mark);
        rest = &rest[char.len_utf8()..];
    }
    Ok(out)
}

// --- putting the program's text back ---------------------------------------------

/// What in the program refers to each place of its file.
pub struct References {
    /// For each place a 32-bit word of the file points at, the offsets of those words.
    pub words: BTreeMap<usize, Vec<usize>>,
    /// What the code does with addresses (see `code_use`), as addresses.
    pub code: CodeUse,
    /// Every place of the file that one of those refers to.
    pub referred: BTreeSet<usize>,
}

/// What refers to the places of the program's file, read once for an open disc.
pub fn references(disc: &Disc) -> Result<Arc<References>> {
    disc.keep("god-ps1 references", || Ok(references_in(&program_file(disc)?)))
}

pub fn references_in(file: &[u8]) -> References {
    let mut words: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for at in (0..file.len() / 4).map(|word| 4 * word) {
        if let Some(target) = number(file, at).and_then(|word| offset_of(file, word)) {
            words.entry(target).or_default().push(at);
        }
    }
    let code = code_use(file, PROGRAM_CODE, PROGRAM_BASE);
    let built = code.built.keys().chain(&code.straight).filter_map(|address| offset_of(file, *address));
    let referred = words.keys().copied().chain(built).collect();
    References { words, code, referred }
}

/// What refers to a string of section `program`, all of which is changed when it moves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reached {
    /// The offsets of the 32-bit words that hold its address.
    pub words: Vec<usize>,
    /// The offsets of the instructions that add the low half of its address.
    pub code: Vec<usize>,
}

/// Where a string of section `program` is, as a build sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    /// The bytes it has there, the zero byte that ends it included (see `rooms`).
    pub size: usize,
    /// Text right after its zero byte that nothing refers to, which the game reads on into from the
    /// end of the string: it goes wherever the string goes, just after its zero byte.
    pub tail: Vec<u8>,
    /// What refers to it, when that is known well enough for it to move.
    pub reached: Option<Reached>,
    /// Where something refers to a place inside it, the first such place, as a count of bytes from
    /// its start, and its bytes from there to its zero byte: whatever reads it there must still read
    /// the same, so a change keeps those bytes, and keeps them where they are.
    pub inside: Option<(usize, Vec<u8>)>,
}

/// Where each string of section `program` is, as a build sees it: `strings` are the section's.
///
/// A string has its own bytes, and also the zero bytes after it, where nothing but zero bytes is
/// between it and the next string and nothing refers to them. Nothing points at some pieces of text
/// that follow strings: the effect of what a monster does in battle, which the game reads on into
/// past the zero byte of what the monster does ("poisoned!" after "a sting!"), and a few copies of
/// strings the game does not seem to use. Such a piece, right after a string's zero byte, is its
/// tail, and its bytes are part of the string's room.
///
/// A string can move when everything found to refer to it is a 32-bit word that holds its address
/// or an instruction that builds that address and no other, and nothing refers to a place inside it
/// or reads it straight from memory. Every string of the game's has words or code that refer to it, and none is
/// referred to otherwise. Nothing in the archive was found to point at the program's text: the 33
/// words of its members that hold the address of a string are what chance gives among its pictures,
/// as many as words in the same span hold any address there.
pub fn rooms(
    file: &[u8],
    strings: &BTreeMap<u32, Vec<u8>>,
    references: &References,
    codes: &ByteCodes,
) -> BTreeMap<u32, Room> {
    let referred = |range: Range<usize>| references.referred.range(range).next().is_some();
    // The instructions that add a low half to more than one high half, their register having been
    // given different ones on different ways to them: a new low half for one address would change
    // the others too.
    let mut addresses_of: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
    for (address, sites) in &references.code.built {
        for at in sites {
            addresses_of.entry(*at).or_default().insert(*address);
        }
    }
    let shared_code = |at: &usize| addresses_of.get(at).is_some_and(|addresses| addresses.len() > 1);
    let starts: Vec<usize> = strings.keys().map(|offset| *offset as usize).collect();
    let mut rooms = BTreeMap::new();
    for (index, (offset, data)) in strings.iter().enumerate() {
        let (start, address) = (*offset as usize, offset + PROGRAM_BASE);
        let end = start + data.len(); // its zero byte
        let reached = Reached {
            words: references.words.get(&start).cloned().unwrap_or_default(),
            code: references.code.built.get(&address).cloned().unwrap_or_default(),
        };
        let followed = !references.code.straight.contains(&address)
            && !referred(start + 1..end + 1)
            && !reached.code.iter().any(shared_code)
            && (!reached.words.is_empty() || !reached.code.is_empty());
        let stretch = TEXT.iter().find(|stretch| stretch.contains(&start));
        let next = starts.get(index + 1).copied().filter(|next| stretch.is_some_and(|stretch| stretch.contains(next)));
        let own = (data.len() + 1, Vec::new());
        let (size, tail) = match next {
            Some(next) if !referred(end + 1..next) => {
                let gap = &file[end + 1..next];
                if gap.iter().all(|byte| *byte == 0) {
                    (next - start, Vec::new())
                } else {
                    match text_tail(gap, codes) {
                        Some(length) if gap[0] != 0 => (next - start, gap[..length].to_vec()),
                        _ => own,
                    }
                }
            }
            _ => own,
        };
        let inside = references.referred.range(start + 1..end + 1).next().map(|at| {
            let from = at - start;
            (from, data[from..].to_vec())
        });
        rooms.insert(*offset, Room { size, tail, reached: followed.then_some(reached), inside });
    }
    rooms
}

/// How long the text that follows a string is, read from the start of the gap after it: pieces of
/// text each ending in its zero byte, with zero bytes between them, up to the last piece's zero byte;
/// none if something in the gap is not text. Read forwards, so that a piece ending in a two-byte
/// character whose second byte is zero is read whole.
pub(super) fn text_tail(gap: &[u8], codes: &ByteCodes) -> Option<usize> {
    let (mut at, mut end) = (0, None);
    while at < gap.len() {
        if gap[at] == 0 {
            at += 1;
            continue;
        }
        match read_text(&gap[at..], codes) {
            Some((length, characters)) if characters > 0 => {
                at += length + 1;
                end = Some(at);
            }
            _ => return None,
        }
    }
    end
}

/// What writing the changed strings of section `program` back gave.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Written {
    /// The bytes to write in the program's file, each with its offset.
    pub patches: Vec<(usize, Vec<u8>)>,
    /// How many of the strings moved.
    pub moved: usize,
    /// How many were not written, being the same text as another that was.
    pub once: usize,
    /// The bytes the strings that moved take, and the room they had between them.
    pub used: usize,
    pub room: usize,
}

/// A string waiting for its place: its offset, its new bytes, and the offsets it may be put at.
type Moving<'a> = (u32, &'a [u8], Range<usize>);

/// Write the changed strings of section `program` back into the program's file. `changed` gives
/// each by its offset, as its new bytes without the zero byte that ends it.
///
/// The program's file cannot grow: it fills its place on the disc, the archive starting in the
/// sector after it, and the memory after what it loads is in use. So the strings a pack changes
/// share the room they had between them (see `share_room`): each is written with its zero byte and
/// its tail, one after another, and what refers to one that moved is given its new address. A word
/// is given the address; an instruction that adds the low half of it is given the new low half, and
/// the instruction that gives the high half, which may serve other addresses, is left alone, so such
/// a string stays within the 65,536 bytes that half reaches (see `within_reach`). Strings the pack
/// gives the same text, without a tail, are written once, all of them referring to the one copy,
/// where one copy can be within reach of them all. A string that cannot move stays where it is,
/// with the room it has there, and keeps the bytes that something reads from inside it.
///
/// What it cannot place, it says, in words a translator can act on.
pub fn write_strings(
    file: &[u8],
    rooms: &BTreeMap<u32, Room>,
    changed: &BTreeMap<u32, Vec<u8>>,
) -> std::result::Result<Written, String> {
    let mut written = Written::default();
    let name = |offset: u32| string_name(PROGRAM_SECTION, offset);
    let need = |offset: u32, data: &[u8]| data.len() + 1 + rooms[&offset].tail.len();
    let mut too_long = Vec::new();
    let mut kept_inside = Vec::new();
    let mut moving: Vec<Moving> = Vec::new();
    for (offset, data) in changed {
        let room = &rooms[offset];
        if let Some((from, kept)) = &room.inside
            && data.get(*from..) != Some(kept.as_slice())
        {
            kept_inside.push(format!(
                "string {}: the program also reads it from {from} bytes in, so its first {from} bytes may change, \
                 to as many bytes, and the rest must stay as it is",
                name(*offset)
            ));
            continue;
        }
        if let Some(reached) = &room.reached {
            moving.push((*offset, data, within_reach(file, PROGRAM_BASE, *offset as usize, &reached.code)));
            continue;
        }
        if need(*offset, data) > room.size {
            // Its room holds its zero byte and its tail too, which a translator does not write.
            let has = room.size - 1 - room.tail.len();
            too_long.push(format!("string {} takes {} bytes, and has {has}", name(*offset), data.len()));
            continue;
        }
        let mut bytes = [data.as_slice(), &[0], &room.tail].concat();
        bytes.resize(room.size, 0);
        written.patches.push((*offset as usize, bytes));
    }
    if !too_long.is_empty() {
        kept_inside.push(format!(
            "{}: such a string stays where it is, since not all that refers to it is known, so shorten it",
            too_long.join("; ")
        ));
    }
    if !kept_inside.is_empty() {
        return Err(kept_inside.join("\n"));
    }

    // The strings given the same text are written once, at a place within reach of all of them.
    let mut groups: Vec<(Vec<usize>, Range<usize>)> = Vec::new(); // the strings of each, and where it may go
    for (index, (offset, data, reach)) in moving.iter().enumerate() {
        let alone = !rooms[offset].tail.is_empty();
        let joins = groups.iter().position(|(members, common)| {
            let (first, first_data, _) = moving[members[0]];
            let overlap = common.start.max(reach.start) < common.end.min(reach.end);
            !alone && rooms[&first].tail.is_empty() && first_data == *data && overlap
        });
        match joins {
            Some(group) => {
                let common = &mut groups[group].1;
                *common = common.start.max(reach.start)..common.end.min(reach.end);
                groups[group].0.push(index);
            }
            None => groups.push((vec![index], reach.clone())),
        }
    }
    // Each string brings its room; the first of each group needs room for them all. With
    // `fewest_places_first`, they are packed that way (see `pack_room_fewest_places_first`).
    let place = |groups: &[(Vec<usize>, Range<usize>)], fewest_places_first: bool| {
        let mut needs: Vec<(usize, usize, usize, Range<usize>)> =
            moving.iter().map(|(offset, ..)| (*offset as usize, rooms[offset].size, 0, 0..usize::MAX)).collect();
        for (members, common) in groups {
            let (offset, data, _) = moving[members[0]];
            needs[members[0]].2 = need(offset, data);
            needs[members[0]].3 = common.clone();
        }
        if fewest_places_first {
            let shared = pack_room_fewest_places_first(&needs, None);
            return (needs, shared);
        }
        let mut shared = share_room(&needs, None);
        if groups.iter().any(|(members, _)| shared.1[members[0]].is_none()) {
            shared = pack_room(&needs, None);
        }
        (needs, shared)
    };
    // A group may go only where its strings' reaches overlap, which may have no room free, and the
    // copy a group shares may take the only room another string had. So the strings are placed four
    // ways, and the first that gives every string a place is kept: those given the same text sharing
    // one copy; the groups that found no place written as one copy per string, each within its own
    // reach; every group written so; and every group written so with the strings that have the
    // fewest places put first, for when a string put first in a room it fits takes the only room
    // another can reach. Only a pack the first way cannot place goes on to the others, so a pack
    // that is placed the first way is written the same whatever the others do. If none places every
    // string, what is refused is the first.
    let placed = |groups: &[(Vec<usize>, Range<usize>)], shared: &Shared| {
        groups.iter().all(|(members, _)| shared.1[members[0]].is_some())
    };
    let apart = |groups: &[(Vec<usize>, Range<usize>)], split: &dyn Fn(&[usize]) -> bool| {
        let each = |(members, common): &(Vec<usize>, Range<usize>)| match members.len() > 1 && split(members) {
            true => members.iter().map(|member| (vec![*member], moving[*member].2.clone())).collect(),
            false => vec![(members.clone(), common.clone())],
        };
        groups.iter().flat_map(each).collect::<Vec<_>>()
    };
    let (mut needs, mut shared) = place(&groups, false);
    if !placed(&groups, &shared) {
        let tries = [
            (apart(&groups, &|members| shared.1[members[0]].is_none()), false),
            (apart(&groups, &|_| true), false),
            (apart(&groups, &|_| true), true),
        ];
        // A way that would only place the same groups as the first, the same way, is not tried again.
        for (other, fewest_places_first) in tries.into_iter().filter(|(other, fewest)| *fewest || *other != groups) {
            let (other_needs, other_shared) = place(&other, fewest_places_first);
            if placed(&other, &other_shared) {
                (groups, needs, shared) = (other, other_needs, other_shared);
                break;
            }
        }
    }
    let (stretches, places, largest) = shared;
    let left: Vec<(u32, usize)> = (groups.iter())
        .filter(|(members, _)| places[members[0]].is_none())
        .map(|(members, _)| (moving[members[0]].0, needs[members[0]].2))
        .collect();
    if !left.is_empty() {
        let room: usize = needs.iter().map(|need| need.1).sum();
        let wanted: usize = needs.iter().map(|need| need.2).sum();
        let which = match left.as_slice() {
            [(offset, _)] => format!("string {} found no place", name(*offset)),
            several => format!("{} strings of the program found no place", several.len()),
        };
        let bytes = |count: usize| if count == 1 { "1 byte".to_string() } else { format!("{count} bytes") };
        let lines: String =
            left.iter().map(|(offset, need)| format!("\n  {} needs {}", name(*offset), bytes(*need))).collect();
        return Err(format!(
            "{which}: the strings of the program that a pack changes share the room they had between them, {room} \
             bytes here with the zero byte that ends each, and this pack's need {wanted}; the largest piece left has \
             {largest}, and a string the program's code builds the address of stays within about 32 KB of where it \
             was. Shorter text in any of them makes room, and so does the same text where the game's own repeats\
             {lines}"
        ));
    }
    written.room = needs.iter().map(|need| need.1).sum();
    written.used = needs.iter().map(|need| need.2).sum();
    let mut areas: Vec<(usize, Vec<u8>)> =
        stretches.iter().map(|stretch| (stretch.start, vec![0; stretch.len()])).collect();
    for (members, _) in &groups {
        let (offset, data, _) = moving[members[0]];
        let place = places[members[0]].expect("every group has a place");
        let (start, area) = areas.iter_mut().rev().find(|(start, _)| *start <= place).expect("a place is in a stretch");
        let bytes = [data, &[0], &rooms[&offset].tail].concat();
        area[place - *start..place - *start + bytes.len()].copy_from_slice(&bytes);
        written.once += members.len() - 1;
        for member in members {
            let offset = moving[*member].0;
            if place == offset as usize {
                continue;
            }
            written.moved += 1;
            let address = place as u32 + PROGRAM_BASE;
            let reached = rooms[&offset].reached.as_ref().expect("a string that moves is reached");
            for at in &reached.words {
                written.patches.push((*at, address.to_le_bytes().to_vec()));
            }
            for at in &reached.code {
                let instruction = number(file, *at).unwrap_or(0) & 0xFFFF_0000 | address & 0xFFFF;
                written.patches.push((*at, instruction.to_le_bytes().to_vec()));
            }
        }
    }
    written.patches.extend(areas);
    written.patches.sort();
    Ok(written)
}

/// How many bytes the label of the small font at `start` may take, its zero byte left out: its own,
/// and the zero bytes after it, up to the next label of its run (see `menu_strings`), which the game
/// finds a set number of bytes on, or, after the last of a run, up to the next multiple of 4, where
/// the compiler starts what follows. `labels` are the section's, and only bytes nothing refers to
/// are counted.
pub fn label_room(file: &[u8], start: usize, labels: &BTreeMap<u32, Vec<u8>>, referred: &BTreeSet<usize>) -> usize {
    let length = labels.get(&(start as u32)).map_or(0, Vec::len);
    let end = start + length; // its zero byte
    let next = labels.range(start as u32 + 1..).next().map(|(next, _)| *next as usize);
    let stop = match next {
        Some(next) if next - end - 1 <= MENU_GAP && file[end..next].iter().all(|byte| *byte == 0) => next,
        _ => (end + 1).next_multiple_of(4),
    };
    let free = (end + 1..stop.min(file.len())).take_while(|at| file[*at] == 0 && !referred.contains(at)).count();
    length + free
}

/// The labels of the main menu, whose box was seen to show 8 characters of a label.
const MAIN_MENU: Range<u32> = 0xA_7D1C..0xA_7D64;
const MAIN_MENU_WIDTH: usize = 8;

/// How many characters a label of the small font draws: each byte one, but `@`, which switches
/// between katakana and hiragana, and ﾞ and ﾟ, which the font draws over the letter before.
pub fn label_cells(label: &[u8]) -> usize {
    label.iter().filter(|byte| ![SWITCH, VOICED, SEMI_VOICED].contains(byte)).count()
}

/// How many characters the label at `start` may draw: as many as the widest of the game's own
/// labels in its run (see `menu_strings`), which are the labels of one screen. That is taken as the
/// room its box or column has, without having been measured, so a label within it is not sure to
/// show whole; the main menu's box was seen to show 8 characters. `labels` are the section's, read
/// from `file`, whose bytes between two labels tell whether the second goes on the run: only zero
/// bytes do, as `menu_strings` has it.
pub fn label_width(file: &[u8], start: u32, labels: &BTreeMap<u32, Vec<u8>>) -> usize {
    if MAIN_MENU.contains(&start) {
        return MAIN_MENU_WIDTH;
    }
    let listed: Vec<(usize, &Vec<u8>)> = labels.iter().map(|(offset, label)| (*offset as usize, label)).collect();
    // Whether the label after `index` is in the same run.
    let runs_on = |index: usize| {
        let (end, next) = (listed[index].0 + listed[index].1.len(), listed[index + 1].0);
        next - end - 1 <= MENU_GAP && file[end..next].iter().all(|byte| *byte == 0)
    };
    let Some(at) = listed.iter().position(|(offset, _)| *offset == start as usize) else { return 0 };
    let mut first = at;
    while first > 0 && runs_on(first - 1) {
        first -= 1;
    }
    let mut last = at;
    while last + 1 < listed.len() && runs_on(last) {
        last += 1;
    }
    listed[first..=last].iter().map(|(_, label)| label_cells(label)).max().unwrap_or(0)
}

/// The conversions of the format a label holds, which the code fills in before the label is drawn,
/// in their order: `d` for `%8d`. A `%%` draws a `%` and is none. A pack's label must have the same
/// as the game's own: the code passes what it fills in for each, and a conversion it does not pass
/// for reads whatever is there.
pub fn conversions(label: &[u8]) -> Vec<u8> {
    let mut found = Vec::new();
    let mut bytes = label.iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            continue;
        }
        let conversion = bytes.find(|byte| !in_conversion(*byte));
        match conversion {
            Some(b'%') => {}
            Some(conversion) => found.push(conversion),
            None => found.push(0),
        }
    }
    found
}

/// What a translator is told about some of the labels of section `menu`.
const LABEL_NOTES: [(u32, &str); 6] = [
    (
        0xD00,
        "A count of turns left, which the code drawn in battle fills in: %d is the number. Not yet seen on screen.",
    ),
    (
        0x15C8,
        "The shop's commands: buy, then sell, then leave the shop, each in a slot of its own. Buy and sell have 3 \
         bytes each, leave 7.",
    ),
    (0x15D8, "In a shop, the label over how many of the chosen thing the party has. Its slot has 11 bytes."),
    (
        0x15F4,
        "A shop's yes and no, in one label, with a space between: the hand points at the label's first character \
         for yes and at its fourth for no, so no starts 3 characters in.",
    ),
    (0x16D0, "How many of a monster the party has beaten. Not yet seen on screen."),
    (
        0xD_2E8C,
        "In a shop, the count of the chosen thing to buy, and of how many the party has: %8d is the number, put \
         right in 8 characters; コ is the word for things counted.",
    ),
];

/// Strings of section `program` shown in a box the game sized for its own text, so that no line may
/// be wider than the game's own widest: the choices of yes and no, the title screen's menu, the
/// inn's menu, and the memory card's question whether to write over a save. The title screen's
/// box was seen to cut a longer line short.
pub const OWN_WIDTH: [u32; 7] = [0xA_B371, 0xA_B378, 0xA_B380, 0xA_B38E, 0xA_BB0E, 0xA_BB11, 0xA_BBC2];

/// The characters of ASCII the small font draws as something else: a block or an arrow. A lowercase
/// letter it draws as its capital.
const NOT_SMALL: [char; 6] = ['\\', '^', '_', '`', '|', '~'];

/// A pack's label as the bytes of a label of the small font: lowercase letters written as the
/// capitals the font draws for them, and what the font does not have refused, a byte written as
/// `{B:..}` among it, which the font has no character for.
pub fn encode_label(text: &str) -> Result<Vec<u8>> {
    let raw = tokens(text).into_iter().find_map(|token| match token {
        Token::Tag { raw, name: "B", .. } => Some(raw),
        _ => None,
    });
    if let Some(raw) = raw {
        return text_error(format!(
            "{raw} writes a byte the small font of the menus has no character for: write characters the font has"
        ));
    }
    if let Some(char) = text.chars().find(|char| NOT_SMALL.contains(char)) {
        return text_error(format!(
            "character {} is not in the small font of the menus, which has capitals, digits, the kana and \
             !\"#$%&'()*+,-./:;<=>?[]",
            quoted(&char.to_string())
        ));
    }
    encode_small(&capitals(text))
}

/// Whether a byte can be between the `%` of a conversion and its letter: a flag, a width, a
/// precision or a size.
fn in_conversion(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'-' | b'+' | b' ' | b'#' | b'.' | b'l' | b'h')
}

/// A label's lowercase letters as capitals, but those of a conversion of its format, which the code
/// reads as they are: `%8d` stays `%8d`.
fn capitals(text: &str) -> String {
    let mut out = String::new();
    let mut converting = false;
    for char in text.chars() {
        if converting {
            out.push(char);
            converting = char.is_ascii() && in_conversion(char as u8);
        } else {
            out.push(char.to_ascii_uppercase());
            converting = char == '%';
        }
    }
    out
}

// --- what a translator is told ---------------------------------------------------

/// What a translator is told about the program's text: with no number, about a section's file, and
/// with one, about a string.
pub fn note(section: &str, number: Option<u32>) -> Option<String> {
    match (section, number) {
        (PROGRAM_SECTION, None) => Some(
            "The program's own text: what its lists and screens show, written as the dialogue is. A string is \
             named by where it is in the program's file, so the numbers have gaps. A note under the first string \
             of each part says what the part is and how wide the game's own lines are, in cells."
                .to_string(),
        ),
        (PROGRAM_SECTION, Some(number)) => {
            let part =
                PARTS.iter().find(|part| part.start == number).map(|part| format!("{} {}", part.about, part.room()));
            let boxed = OWN_WIDTH.contains(&number).then(|| {
                "Its box is as wide as the game's own text, so no line may be wider than the game's own widest."
                    .to_string()
            });
            let part = match (part, boxed) {
                (Some(part), Some(boxed)) => Some(format!("{part} {boxed}")),
                (part, boxed) => part.or(boxed),
            };
            let shop = (SHOP_LINES[0].0..SHOP_LINES_END).contains(&number).then(|| {
                let (_, kind) = SHOP_LINES.iter().rev().find(|(start, _)| *start <= number).unwrap_or(&SHOP_LINES[0]);
                format!("A shopkeeper's line: {kind}.")
            });
            match (part, shop) {
                (Some(part), Some(shop)) => Some(format!("{part} {shop}")),
                (part, shop) => part.or(shop),
            }
        }
        (MENU_SECTION, None) => Some(
            "The labels of the main menu and of its screens, drawn from the small font of 8 by 8 pixels. It has \
             capitals, digits, some ASCII punctuation and the kana, and draws a lowercase letter as its capital; \
             none of the dialogue's tags work here. Hiragana and katakana are written as they show; {KANA} is a \
             switch between the two with no kana after it. Most labels sit in a slot of a fixed size, with the \
             next label of the screen at the next slot: the game's own leave the bytes up to the next label \
             unused, and a label cannot take more. Nor can it draw more characters than the widest label of its \
             screen does, ﾞ and ﾟ being drawn over the letter before; the main menu's have room for 8. The dots \
             after a configuration label lead to its setting, 11 characters in. A label with a % in it is a \
             format the game fills in before it draws it: %8d is a number put right in 8 characters, and a \
             label keeps the game's own, in the same order."
                .to_string(),
        ),
        (MENU_SECTION, Some(number)) => {
            LABEL_NOTES.iter().find(|(start, _)| *start == number).map(|(_, note)| note.to_string())
        }
        _ => None,
    }
}
