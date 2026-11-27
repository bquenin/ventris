//! G.O.D pure (PlayStation, Japan).
//!
//! What is known so far covers the dialogue, the text of the game's areas, most of the text the
//! program itself holds (its lists, help pages, battle messages and menus, see `program`), and what
//! the bosses say in battle (see `battle`): where it is, how it is encoded, and how to write it out
//! and read it back without loss. The dialogue can be changed, longer than the game's own and with
//! characters its font lacks (see `insert`), and so can the program's text, its strings sharing the
//! room they had between them, and the menus' labels, each in its slot (see `program`). What the
//! bosses say cannot be changed yet.
//!
//! Where things are. The disc holds one large archive, `GOD.DAT`, with no header of its own. Its
//! table of contents is in the program the console starts, `SLPS_009.44`, at offset $86F64 of the
//! file (the program is loaded so that a file offset is its address less $8000F800): 196 entries of
//! two 32-bit numbers, the sector where a member starts, counted from the archive's first, and its
//! size in bytes. Several entries may name one member: entries 96 and 189 to 195 are all the
//! first area. There are 164 members, and they fill the archive with nothing between them.
//!
//! Entries 96 to 188 are the game's areas, a member each. An area starts with a header of $800
//! bytes, whose first number is the size of what follows it that the game loads to run the area,
//! here called the area's code. That code is, one after another:
//!
//! 1. a count, then that many 16-bit numbers, where the area's events start;
//! 2. a count, then that many 16-bit numbers, the events themselves;
//! 3. a size, then the text block, of that many bytes;
//! 4. what else the area needs, from the next multiple of 4.
//!
//! Each count or size is 32 bits, and what follows a count is padded to a multiple of 4. The
//! header's second number says where the area's other data starts, past the text block, which this
//! module checks.
//!
//! The text block starts with a table of 32-bit offsets, counted from the block's start; the first
//! is where the table ends, which gives how many there are. The game finds an entry by its number,
//! at $8001CA88. Most entries are strings. Some are not text but lists of numbers the events use,
//! such as positions: a 16-bit count, then that many 16-bit numbers or pairs of them, and nothing
//! else up to the next entry (see `numbers_at`). In every area entry 1 is at the same place as
//! entry 0 (in nine areas that entry is a list of numbers). A string ends in a zero byte; the next one starts on an even offset, and the byte
//! between, where there is one, is left over from whatever was there before. The text block ends
//! with four zero bytes. Area 100 has no text at all, and areas 121 and 122, two members, hold the
//! same text block.
//!
//! How a string is encoded, as the routine at $80024CE8 reads it. A byte from $10 up is a
//! character: the program has a table of 256 16-bit codes at offset $81B28 of its file, and the
//! byte's code is the one at its place there. A code of 0 draws nothing. The table is read from
//! the game, not copied here. Bytes $01 to $0A are the high half of a code whose low half is the
//! next byte, which reaches every glyph; the game uses that form only for codes the table of bytes
//! does not have. The bytes under $10 are these:
//!
//! - $00 ends the string. Where a string was put in another (see $0C), the game goes back to it.
//! - $0B waits for the player's button, then goes on at the start of a new line: `{WAIT}`. The text
//!   after it nearly always starts the new line at once, so the line break after the tag in a
//!   script is only layout, as `{WAIT}` gives one of its own.
//! - $0C and one more byte puts in something the game fills in, by putting another string inside
//!   this one. $00 to $06 and $16 are eight names the game keeps (`{NAME:01}`), $17 and $19 to $1C
//!   the name of whoever is in a place in the party, which is also `{NAME:19}`; $0D is a number
//!   (`{NUM}`), $12 a word from a list of the game's (`{WORD}`), $10 and $18 other numbers, and
//!   other values do nothing; those are written with their value, `{0C:10}`.
//! - $0D is a new line, a line break in a script.
//! - $0E starts a spoken line: the game draws 「 and starts the lines that follow under the line's
//!   first character, until the next $0E, which ends the spoken line and draws nothing. The first is
//!   written `{SAY}` and the second `{/SAY}`; both are the one code.
//! - $0F and one more byte: $00 and a third byte holds the text that many frames (`{PAUSE:14}`),
//!   $01 waits for the player's button as $0B does, but goes on along the same line (`{HOLD}`),
//!   $02 clears the window and goes back to its first line (`{CLEAR}`), $03 plays a sound, and
//!   other values do nothing; those are written with their value, `{0F:03}`.
//!
//! The game's font is two pictures in the TIM format, members 0 and 1 of the archive, each 256 by
//! 256 pixels of 4 bits. Each bit of a pixel is a font of its own, so each picture holds four fonts
//! of 256 glyphs of 16 by 16 pixels, and the two hold the 2048 glyphs of codes $100 to $8FF: code
//! `c` is glyph `c - $100`, in the first picture below $500, in the bit given by the glyph's number
//! divided by 256, at the column and row given by its last two hexadecimal digits.
//!
//! The table of characters, `characters.tbl`, gives the character each code draws. Where two codes
//! draw one character, the character is written with the one the game has a byte for, or else
//! with the lower.
//!
//! A section is an area, named by its entry in the table of contents as three digits: `096` to
//! `188`, but `100`. A string is numbered by its entry in the area's table, which is a number no
//! later finding changes. Entries that are lists of numbers have no string, and entry 1, which is
//! entry 0 again, is left out, so a section's numbers have gaps.
//!
//! The program's text is in two more sections, `program` and `menu`, each of them a kind of its own
//! since the two are written and drawn differently; their strings are numbered by their offset in the
//! program's file (see `program`). What the bosses say in battle, also in the program's file, is a
//! third, `battle`, numbered the same way (see `battle`).

use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;
use std::sync::{Arc, LazyLock};

use std::path::Path;

use crate::adapter::{Adapter, BuildOptions, Built, CatalogueEntry, Release, TextKind};
use crate::error::{Error, Result, text_error};
use crate::platform::{Image, Platform};
use crate::platforms::ps1::{self, Disc, tim};
use crate::script::{Pack, Script, verify_by_place};
use crate::text::{Token, from_hex, quoted, read_table, tag_at};

const NAME: &str = "G.O.D pure (PlayStation, Japan)";
const SOURCE_LANGUAGE: &str = "ja";
pub const ROM_SHA1: &str = "a771b5f362751845086e02f53b2c0dd7f302361a";

/// The program the console starts, and the archive.
pub const PROGRAM: &str = "SLPS_009.44";
pub const ARCHIVE: &str = "GOD.DAT";
/// Where the archive's table of contents is in the program's file, and how many entries it has.
pub const CONTENTS: u64 = 0x86F64;
pub const CONTENTS_ENTRIES: usize = 196;
/// Where the program's file has the code of each byte from $00 to $FF.
pub const BYTE_CODES: u64 = 0x81B28;
/// The entries of the table of contents that are areas.
pub const AREAS: RangeInclusive<usize> = 96..=188;
/// The size of an area's header, before its code.
pub const AREA_HEADER: usize = 0x800;
/// The two members that are the font.
pub const FONT_PAGES: [usize; 2] = [0, 1];
pub const FIRST_CODE: u32 = 0x100;
pub const GLYPH_SIZE: usize = 16;
pub const GLYPH_COUNT: usize = 0x800;
/// The highest code a pair of bytes can give: high half $0A.
const LAST_PAIR_CODE: u32 = 0x0AFF;
/// The highest value of $0C that the game looks up; what follows it in memory is not a list of
/// its routines.
const LAST_INSERT: u8 = 0x1E;

/// Areas whose text block is another's, byte for byte: the area, and the one it repeats.
pub const REPEATED_AREAS: [(usize, usize); 1] = [(122, 121)];
/// In every area entry 1 of the table is at the place entry 0 is.
pub const SHARED_ENTRY: (u32, u32) = (1, 0);
/// The areas where that entry is a list of numbers, not a string.
pub const LIST_FIRST: [usize; 9] = [99, 128, 146, 149, 164, 177, 184, 185, 188];

/// The values of $0C that put in a name.
const NAME_INSERTS: [u8; 13] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x16, 0x17, 0x19, 0x1A, 0x1B, 0x1C];
const NUMBER_INSERT: u8 = 0x0D;
const WORD_INSERT: u8 = 0x12;

/// The characters of the game's font, from `characters.tbl`: one `CODE=character` line each, the
/// code in four hexadecimal digits.
struct Table {
    characters: HashMap<u32, char>,
    /// The codes that draw each character, lowest first: a few characters are in the font twice.
    codes: HashMap<char, Vec<u32>>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut characters = HashMap::new();
    let mut codes = HashMap::new();
    for (hex_code, text) in read_table(include_str!("characters.tbl")) {
        let code = match u32::from_str_radix(hex_code, 16) {
            Ok(code) if hex_code.len() == 4 && (FIRST_CODE..FIRST_CODE + GLYPH_COUNT as u32).contains(&code) => code,
            _ => panic!("god_ps1/characters.tbl: {hex_code} is not a code from 0100 to 08FF"),
        };
        let mut letters = text.chars();
        let (Some(char), None) = (letters.next(), letters.next()) else {
            panic!("god_ps1/characters.tbl: code {hex_code} is not one character");
        };
        assert!(characters.insert(code, char).is_none(), "god_ps1/characters.tbl: code {hex_code} appears twice");
        codes.entry(char).or_insert_with(Vec::new).push(code);
    }
    codes.values_mut().for_each(|codes: &mut Vec<u32>| codes.sort());
    Table { characters, codes }
});

/// What is read once from the program: the archive's table of contents, and the code of each byte.
pub struct Program {
    /// Each entry's first sector, counted from the archive's, and size in bytes.
    pub contents: Vec<(u32, u32)>,
    pub codes: ByteCodes,
}

/// The code each byte stands for, as the program's table gives it, and the byte for each code.
pub struct ByteCodes {
    codes: [u16; 256],
    bytes: HashMap<u32, u8>,
}

impl ByteCodes {
    pub fn new(codes: [u16; 256]) -> Self {
        let mut bytes = HashMap::new();
        for byte in (0x10..=0xFF).rev() {
            if is_glyph_code(u32::from(codes[byte])) {
                bytes.insert(u32::from(codes[byte]), byte as u8);
            }
        }
        ByteCodes { codes, bytes }
    }

    /// The code a byte from $10 up stands for: 0 for none.
    pub fn code(&self, byte: u8) -> u32 {
        u32::from(self.codes[usize::from(byte)])
    }

    /// The byte a code is written with alone, if it has one: the lowest, if it has two.
    pub fn byte(&self, code: u32) -> Option<u8> {
        self.bytes.get(&code).copied()
    }
}

fn is_glyph_code(code: u32) -> bool {
    (FIRST_CODE..FIRST_CODE + GLYPH_COUNT as u32).contains(&code)
}

fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

/// A little-endian 32-bit number at `at`, as a size or an offset.
fn word(data: &[u8], at: usize) -> Option<usize> {
    let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes) as usize)
}

fn half(data: &[u8], at: usize) -> Option<usize> {
    let bytes: [u8; 2] = data.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(usize::from(u16::from_le_bytes(bytes)))
}

/// The table of contents and the codes of the bytes, read once for an open disc.
pub fn program(disc: &Disc) -> Result<Arc<Program>> {
    disc.keep("god-ps1 program", || {
        let contents = disc.read(PROGRAM, CONTENTS, CONTENTS_ENTRIES as u64 * 8)?;
        let contents = contents.chunks(8).map(|entry| (word(entry, 0).unwrap() as u32, word(entry, 4).unwrap() as u32));
        let table = disc.read(PROGRAM, BYTE_CODES, 512)?;
        let codes = std::array::from_fn(|byte| half(&table, byte * 2).unwrap() as u16);
        Ok(Program { contents: contents.collect(), codes: ByteCodes::new(codes) })
    })
}

/// The member of the archive an entry of its table of contents names. It is read from the sector
/// the entry gives, counted from the archive's first, as the game reads it: a member a build moved
/// is outside the archive (see `insert`).
pub fn member(disc: &Disc, entry: usize) -> Result<Vec<u8>> {
    let Some(&(sector, size)) = program(disc)?.contents.get(entry) else {
        return image_error(format!("the archive has no entry {entry}"));
    };
    let data = disc.data(disc.first_sector(ARCHIVE)? + sector, 0, u64::from(size))?;
    if data.len() != size as usize {
        return image_error(format!("entry {entry} of the archive runs past the end of the disc"));
    }
    Ok(data)
}

/// The text block of an area's member: where it starts in the member, and its size.
pub fn text_block(member: &[u8]) -> Option<(usize, usize)> {
    let code_size = word(member, 0)?;
    let code_end = AREA_HEADER.checked_add(code_size)?;
    if code_end > member.len() {
        return None;
    }
    let padded = |count: usize| count.div_ceil(2) * 4;
    let events = AREA_HEADER + 4 + padded(word(member, AREA_HEADER)?);
    let size_at = events + 4 + padded(word(member, events)?);
    let (start, size) = (size_at + 4, word(member, size_at)?);
    // The header says where the data after the text block starts, from the next multiple of 4.
    let after = word(member, 4)?;
    (start + size <= code_end && after + AREA_HEADER == start + size.next_multiple_of(4)).then_some((start, size))
}

/// What an entry of an area's table of strings is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A string: where it starts in the text block, and where its zero byte is.
    Text { start: usize, end: usize },
    /// A list of numbers, not text.
    Numbers,
    /// At the place of an earlier entry, with this number.
    Same(u32),
}

/// Whether the bytes from an entry up to the next are a list of numbers: a 16-bit count, then that
/// many 16-bit numbers or pairs of them, then nothing but zero bytes, at most four (the text block's
/// last four, for the last entry).
pub fn numbers_at(span: &[u8]) -> bool {
    let Some(count) = half(span, 0) else { return false };
    [2 + 2 * count, 2 + 4 * count]
        .into_iter()
        .any(|size| size <= span.len() && span.len() - size <= 4 && span[size..].iter().all(|byte| *byte == 0))
}

/// Where the zero byte that ends the text at the start of `data` is, if every byte up to it is one
/// the game reads as text, and how many characters the text draws: a code of two bytes, or a byte
/// whose code is a glyph.
pub fn read_text(data: &[u8], codes: &ByteCodes) -> Option<(usize, usize)> {
    let (mut at, mut characters) = (0, 0);
    loop {
        let byte = *data.get(at)?;
        let length = match byte {
            0x00 => return Some((at, characters)),
            0x01..=0x0A => 2,
            0x0C if *data.get(at + 1)? <= LAST_INSERT => 2,
            0x0C => return None,
            0x0F if *data.get(at + 1)? == 0 => 3,
            0x0F => 2,
            0x0B | 0x0D | 0x0E => 1,
            // A byte whose table entry is not a glyph is no character: the table of bytes from $10
            // to $1F holds other numbers of the program.
            _ if codes.code(byte) == 0 || is_glyph_code(codes.code(byte)) => 1,
            _ => return None,
        };
        if at + length > data.len() || (byte <= 0x0A && (u32::from(byte) << 8 | u32::from(data[at + 1])) > 0x8FF) {
            return None;
        }
        characters += usize::from(byte <= 0x0A || (byte >= 0x10 && codes.code(byte) != 0));
        at += length;
    }
}

/// Whether every character a string, without its zero byte, draws is one the table of characters
/// has. A byte that draws nothing, or a code with no character, is not.
pub fn every_character_known(data: &[u8], codes: &ByteCodes) -> bool {
    let mut at = 0;
    while at < data.len() {
        let (code, length) = match data[at] {
            byte @ 0x01..=0x0A => (data.get(at + 1).map(|low| u32::from(byte) << 8 | u32::from(*low)), 2),
            0x0C => (None, 2),
            0x0F if data.get(at + 1) == Some(&0) => (None, 3),
            0x0F => (None, 2),
            byte @ 0x10..=0xFF => (Some(codes.code(byte)), 1),
            _ => (None, 1),
        };
        if code.is_some_and(|code| !TABLE.characters.contains_key(&code)) {
            return false;
        }
        at += length;
    }
    true
}

/// Where the zero byte that ends a string starting at `span[0]` is, if the bytes up to it are all
/// ones the game reads as text, and only a left-over byte (to reach an even offset) and zero bytes
/// follow it up to the end of the span. `start` is where the span is in the text block.
pub fn string_end(span: &[u8], start: usize, codes: &ByteCodes) -> Option<usize> {
    let (at, _) = read_text(span, codes)?;
    let mut rest = at + 1;
    if (start + rest) % 2 == 1 {
        rest += 1;
    }
    span.get(rest..).unwrap_or_default().iter().all(|byte| *byte == 0).then_some(at)
}

/// What each entry of a text block's table is. It fails where one is neither text nor numbers, so
/// that a string is never lost unseen.
pub fn entries(text: &[u8], codes: &ByteCodes) -> std::result::Result<Vec<Entry>, String> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let Some(first) = word(text, 0).filter(|first| *first >= 4 && first % 4 == 0 && *first <= text.len()) else {
        return Err("its text block does not start with a table of offsets".into());
    };
    let offsets: Vec<usize> = (0..first / 4).map(|number| word(text, number * 4).unwrap()).collect();
    if offsets.windows(2).any(|pair| pair[0] > pair[1]) || offsets.last().is_some_and(|last| *last >= text.len()) {
        return Err("the offsets of its table of strings do not rise within its text block".into());
    }
    let mut found = Vec::new();
    for (number, &start) in offsets.iter().enumerate() {
        if let Some(earlier) = offsets[..number].iter().position(|offset| *offset == start) {
            found.push(Entry::Same(earlier as u32));
            continue;
        }
        let end = offsets[number..].iter().copied().find(|offset| *offset > start).unwrap_or(text.len());
        let span = &text[start..end];
        // A list of numbers can read as a string as well; no string of the game has that shape.
        if numbers_at(span) {
            found.push(Entry::Numbers);
        } else if let Some(length) = string_end(span, start, codes) {
            found.push(Entry::Text { start, end: start + length });
        } else {
            return Err(format!("entry {number} of its table of strings is neither text nor a list of numbers"));
        }
    }
    Ok(found)
}

/// One area: the entry it is in the table of contents, its text block, and what each entry of its
/// table of strings is.
pub struct Area {
    pub entry: usize,
    pub text: Vec<u8>,
    pub entries: Vec<Entry>,
}

impl Area {
    /// The section it is in a script.
    pub fn section(&self) -> String {
        format!("{:03}", self.entry)
    }

    /// Its strings, by number, each without its zero byte.
    pub fn strings(&self) -> BTreeMap<u32, &[u8]> {
        let strings = self.entries.iter().enumerate().filter_map(|(number, entry)| match *entry {
            Entry::Text { start, end } => Some((number as u32, &self.text[start..end])),
            _ => None,
        });
        strings.collect()
    }
}

/// Every area, in the order of the table of contents, each member once, read once for an open disc.
pub fn areas(disc: &Disc) -> Result<Arc<Vec<Area>>> {
    disc.keep("god-ps1 areas", || {
        let program = program(disc)?;
        let mut areas: Vec<Area> = Vec::new();
        let mut seen = Vec::new();
        for entry in AREAS {
            if seen.contains(&program.contents[entry]) {
                continue;
            }
            seen.push(program.contents[entry]);
            let member = member(disc, entry)?;
            let Some((start, size)) = text_block(&member) else {
                return image_error(format!("area {entry:03} is not laid out as an area is"));
            };
            let text = member[start..start + size].to_vec();
            let entries = match entries(&text, &program.codes) {
                Ok(entries) => entries,
                Err(problem) => return image_error(format!("area {entry:03}: {problem}")),
            };
            areas.push(Area { entry, text, entries });
        }
        Ok(areas)
    })
}

// --- text ------------------------------------------------------------------

/// A string of the game, without its zero byte, as script text.
pub fn decode(data: &[u8], codes: &ByteCodes) -> String {
    let mut out = String::new();
    let mut speaking = false;
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        let next = data.get(at + 1).copied();
        match (byte, next) {
            (0x01..=0x0A, Some(low)) => {
                let code = u32::from(byte) << 8 | u32::from(low);
                match character(code, codes) {
                    Some(char) if codes.byte(code).is_none() => out.push(char),
                    _ if codes.byte(code).is_some() => out.push_str(&format!("{{W:{code:04X}}}")),
                    _ => out.push_str(&format!("{{K:{code:04X}}}")),
                }
                at += 2;
                continue;
            }
            (0x0B, _) => out.push_str("{WAIT}\n"),
            (0x0C, Some(value)) => {
                out.push_str(&match value {
                    _ if NAME_INSERTS.contains(&value) => format!("{{NAME:{value:02X}}}"),
                    NUMBER_INSERT => "{NUM}".to_string(),
                    WORD_INSERT => "{WORD}".to_string(),
                    _ => format!("{{0C:{value:02X}}}"),
                });
                at += 2;
                continue;
            }
            (0x0D, _) => out.push('\n'),
            (0x0E, _) => {
                out.push_str(if speaking { "{/SAY}" } else { "{SAY}" });
                speaking = !speaking;
            }
            (0x0F, Some(0)) if at + 2 < data.len() => {
                out.push_str(&format!("{{PAUSE:{:02X}}}", data[at + 2]));
                at += 3;
                continue;
            }
            (0x0F, Some(value)) => {
                out.push_str(&match value {
                    1 => "{HOLD}".to_string(),
                    2 => "{CLEAR}".to_string(),
                    _ => format!("{{0F:{value:02X}}}"),
                });
                at += 2;
                continue;
            }
            (0x10..=0xFF, _) => {
                let code = codes.code(byte);
                match character(code, codes) {
                    Some(char) if codes.byte(code) == Some(byte) => out.push(char),
                    _ if codes.byte(code) == Some(byte) => out.push_str(&format!("{{K:{code:04X}}}")),
                    _ => out.push_str(&format!("{{B:{byte:02X}}}")),
                }
            }
            // What is left does not occur in a string the game reads: a zero byte, or a code cut
            // short by the end.
            _ => out.push_str(&format!("{{B:{byte:02X}}}")),
        }
        at += 1;
    }
    out
}

/// The character a code draws, where the table has one and that character is written with this code.
fn character(code: u32, codes: &ByteCodes) -> Option<char> {
    let char = *TABLE.characters.get(&code)?;
    (code_of(char, codes) == Some(code)).then_some(char)
}

/// The code a character is written with: of those that draw it, one the game writes in a single
/// byte if there is one, otherwise the lowest.
fn code_of(char: char, codes: &ByteCodes) -> Option<u32> {
    let all = TABLE.codes.get(&char)?;
    all.iter().copied().find(|code| codes.byte(*code).is_some()).or_else(|| all.first().copied())
}

/// A code as the game writes it: in one byte where its table of bytes has one, otherwise in two.
fn code_bytes(code: u32, codes: &ByteCodes) -> Vec<u8> {
    match codes.byte(code) {
        Some(byte) => vec![byte],
        None => vec![(code >> 8) as u8, code as u8],
    }
}

/// Script text as the bytes of a string, without its zero byte.
pub fn encode(text: &str, codes: &ByteCodes) -> Result<Vec<u8>> {
    encode_with(text, codes, &|char| code_of(char, codes))
}

/// The same, with `code` giving the code each character is written with, or none where the game
/// cannot draw it: a pack's text may use characters the game's own does not (see `insert`).
pub fn encode_with(text: &str, codes: &ByteCodes, code: &dyn Fn(char) -> Option<u32>) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        if char == '{' {
            let Some(Token::Tag { raw, name, value }) = tag_at(rest) else {
                let shown: String = rest.chars().take(16).collect();
                return text_error(format!("malformed tag: {}", quoted(&shown)));
            };
            out.extend(encode_tag(raw, name, value.unwrap_or(""), codes)?);
            rest = &rest[raw.len()..];
            // A wait starts a new line of its own; the line break after it in a script is layout.
            if name == "WAIT" {
                rest = rest.strip_prefix('\n').unwrap_or(rest);
            }
            continue;
        }
        if char == '\n' {
            out.push(0x0D);
        } else if let Some(code) = code(char) {
            out.extend(code_bytes(code, codes));
        } else {
            return text_error(format!("character {} is not in the game's character table", quoted(&char.to_string())));
        }
        rest = &rest[char.len_utf8()..];
    }
    Ok(out)
}

/// The bytes a tag stands for.
fn encode_tag(raw: &str, name: &str, value: &str, codes: &ByteCodes) -> Result<Vec<u8>> {
    let Some(data) = from_hex(value) else {
        return text_error(format!("tag {raw} does not hold hexadecimal bytes"));
    };
    let pair = |data: &[u8]| match data {
        [high, low] => Some(u32::from(*high) << 8 | u32::from(*low)),
        _ => None,
    };
    let bytes = match (name, data.as_slice()) {
        ("WAIT", []) => vec![0x0B],
        ("SAY" | "/SAY", []) => vec![0x0E],
        ("NAME", [value]) if NAME_INSERTS.contains(value) => vec![0x0C, *value],
        ("NUM", []) => vec![0x0C, NUMBER_INSERT],
        ("WORD", []) => vec![0x0C, WORD_INSERT],
        ("0C", [value]) if *value <= LAST_INSERT && !NAME_INSERTS.contains(value) => vec![0x0C, *value],
        ("PAUSE", [frames]) => vec![0x0F, 0x00, *frames],
        ("HOLD", []) => vec![0x0F, 0x01],
        ("CLEAR", []) => vec![0x0F, 0x02],
        ("0F", [value]) if *value > 2 => vec![0x0F, *value],
        ("K", _) => match pair(&data) {
            Some(code) if (FIRST_CODE..=LAST_PAIR_CODE).contains(&code) => code_bytes(code, codes),
            _ => return text_error(format!("{raw} is not a code from 0100 to 0AFF")),
        },
        ("W", _) => match pair(&data) {
            Some(code) if (FIRST_CODE..=LAST_PAIR_CODE).contains(&code) => data,
            _ => return text_error(format!("{raw} is not a code from 0100 to 0AFF")),
        },
        ("B", [byte]) if *byte >= 0x10 => vec![*byte],
        _ => return text_error(format!("unknown tag {raw}")),
    };
    Ok(bytes)
}

// --- the font --------------------------------------------------------------

/// The two pictures of the font, read once for an open disc.
fn font(disc: &Disc) -> Result<Arc<Vec<tim::Tim>>> {
    disc.keep("god-ps1 font", || FONT_PAGES.iter().map(|entry| tim::read(&member(disc, *entry)?)).collect())
}

/// A glyph of the font: 16 rows of 16 pixels, each 0 or 1.
pub fn glyph(disc: &Disc, index: usize) -> Result<Vec<Vec<u8>>> {
    if index >= GLYPH_COUNT {
        return text_error(format!("the game's font has no glyph {index}"));
    }
    let font = font(disc)?;
    let page = &font[index / 0x400];
    let (bit, column, row) = ((index >> 8) & 3, index & 0xF, (index >> 4) & 0xF);
    let rows = (0..GLYPH_SIZE).map(|y| {
        let pixel = |x: usize| page.pixel(column * GLYPH_SIZE + x, row * GLYPH_SIZE + y).map(|value| value >> bit & 1);
        (0..GLYPH_SIZE).map(pixel).collect::<Option<Vec<u8>>>()
    });
    match rows.collect::<Option<Vec<_>>>() {
        Some(rows) => Ok(rows),
        None => image_error("the game's font is smaller than 256 by 256 pixels of 4 bits"),
    }
}

pub fn code_at(index: usize) -> u32 {
    FIRST_CODE + index as u32
}

// --- what the engine sees --------------------------------------------------

pub fn extract(disc: &Disc) -> Result<Script> {
    let codes = &program(disc)?.codes;
    let mut script = Script::new();
    for area in areas(disc)?.iter() {
        let strings = area.strings();
        if !strings.is_empty() {
            script.insert(
                area.section(),
                strings.into_iter().map(|(number, data)| (number, decode(data, codes))).collect(),
            );
        }
    }
    let text = program::program_text(disc)?;
    let strings = &text[program::PROGRAM_SECTION];
    script.insert(
        program::PROGRAM_SECTION.to_string(),
        strings.iter().map(|(number, data)| (*number, decode(data, codes))).collect(),
    );
    let labels = &text[program::MENU_SECTION];
    script.insert(
        program::MENU_SECTION.to_string(),
        labels.iter().map(|(number, data)| (*number, program::decode_small(data))).collect(),
    );
    let lines = battle::battle_text(disc)?;
    script.insert(
        battle::BATTLE_SECTION.to_string(),
        lines.iter().map(|(number, line)| (*number, decode(&line.data, codes))).collect(),
    );
    Ok(script)
}

/// Check that an extracted script encodes back to exactly the game's bytes. The menus' labels are
/// written another way than the rest, and are checked on their own.
pub fn verify_script(disc: &Disc, script: &Script) -> Result<()> {
    let codes = &program(disc)?.codes;
    let areas = areas(disc)?;
    let text = program::program_text(disc)?;
    let mut original: BTreeMap<String, BTreeMap<u32, Vec<u8>>> = areas
        .iter()
        .map(|area| {
            (area.section(), area.strings().into_iter().map(|(number, data)| (number, data.to_vec())).collect())
        })
        .collect();
    original.insert(program::PROGRAM_SECTION.to_string(), text[program::PROGRAM_SECTION].clone());
    let lines = battle::battle_text(disc)?;
    original.insert(
        battle::BATTLE_SECTION.to_string(),
        lines.iter().map(|(number, line)| (*number, line.data.clone())).collect(),
    );
    let labels = BTreeMap::from([(program::MENU_SECTION.to_string(), text[program::MENU_SECTION].clone())]);
    let (menu, rest): (Script, Script) =
        script.clone().into_iter().partition(|(section, _)| section == program::MENU_SECTION);
    verify_by_place(&rest, &original, |text| encode(text, codes))?;
    verify_by_place(&menu, &labels, program::encode_small)
}

/// An area is named by its entry in the table of contents, in three digits. The program's text is
/// in three sections named in words.
fn is_dialogue(section: &str) -> bool {
    let digits = section.len() == 3 && section.bytes().all(|byte| byte.is_ascii_digit());
    digits && section.parse().is_ok_and(|entry: usize| AREAS.contains(&entry))
}

fn is_program(section: &str) -> bool {
    section == program::PROGRAM_SECTION
}

fn is_menu(section: &str) -> bool {
    section == program::MENU_SECTION
}

fn is_battle(section: &str) -> bool {
    section == battle::BATTLE_SECTION
}

static KINDS: [TextKind; 4] = [
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        numbered_by_place: true,
        limits: insert::LIMITS,
        // The first line of the game, and the one after it, which is longer and as wide as a spoken
        // line can be.
        trial: Some(("096.109", "096.110")),
        ..TextKind::new("dialogue", is_dialogue)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        numbered_by_place: true,
        limits: program::LIMITS,
        // A shopkeeper's line, and a longer one of another shop that fits the room of the first.
        trial: Some(("program.657758", "program.657988")),
        ..TextKind::new("program", is_program)
    },
    TextKind {
        insert: true,
        numbered_by_place: true,
        limits: program::MENU_LIMITS,
        // The main menu's first label, and its sixth.
        trial: Some(("menu.687388", "menu.687438")),
        ..TextKind::new("menu", is_menu)
    },
    TextKind { numbered_by_place: true, ..TextKind::new("battle", is_battle) },
];

/// The adapter itself: what the rest of this module knows, on the contract every game follows.
pub struct God;

pub static ADAPTER: God = God;

impl Adapter for God {
    fn id(&self) -> &'static str {
        "god-ps1"
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn platform(&self) -> &'static dyn Platform {
        &ps1::PLATFORM
    }

    fn images(&self) -> &'static [Release] {
        // Redump's "G.O.D pure (Japan)", disc 3613, whose record gives this SHA-1 for its one track.
        // Its SYSTEM.CNF gives no version.
        &[Release {
            sha1: ROM_SHA1,
            name: "Japan",
            size: 489_919_248,
            label: "SLPS-00944",
            version: "",
            // redump.org does not answer over https, so its address is http.
            catalogue: Some(CatalogueEntry {
                catalogue: &ps1::REDUMP,
                name: "G.O.D pure (Japan)",
                page: Some("http://redump.org/disc/3613/"),
            }),
        }]
    }

    fn source_language(&self) -> &'static str {
        SOURCE_LANGUAGE
    }

    fn kinds(&self) -> &'static [TextKind] {
        &KINDS
    }

    fn fonts(&self) -> &'static [&'static str] {
        insert::FONTS
    }

    fn extract(&self, image: &Image) -> Result<Pack> {
        Ok(Pack::of(extract(image.ps1_disc()?)?))
    }

    fn note(&self, section: &str, number: Option<u32>) -> Option<String> {
        if is_battle(section) {
            return battle::note(number);
        }
        if !is_dialogue(section) {
            return program::note(section, number);
        }
        let entry: usize = section.parse().ok()?;
        match number {
            None => REPEATED_AREAS.iter().find(|(area, _)| *area == entry).map(|(_, original)| {
                format!(
                    "This area's text is the same as area {original:03}'s, string for string: the game keeps it \
                     twice. Translate it the same way."
                )
            }),
            Some(number) if number == SHARED_ENTRY.1 && !LIST_FIRST.contains(&entry) => Some(format!(
                "The area's table of strings also gives this string as {:03}, which is left out.",
                SHARED_ENTRY.0
            )),
            Some(_) => None,
        }
    }

    fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
        verify_script(image.ps1_disc()?, texts)
    }

    fn write(&self, image: &Image, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
        insert::build(image.ps1_disc()?, pack, output, options)
    }

    fn glyph_size(&self) -> usize {
        GLYPH_SIZE
    }

    fn glyph_count(&self) -> usize {
        GLYPH_COUNT
    }

    fn glyph(&self, image: &Image, index: usize) -> Result<Vec<Vec<u8>>> {
        glyph(image.ps1_disc()?, index)
    }

    fn code_at(&self, index: usize) -> u32 {
        code_at(index)
    }

    /// A code the character table does not cover is written `{K:xxxx}`, or `{W:xxxx}` in its
    /// two-byte form, four hexadecimal digits in capitals.
    fn unmapped(&self, text: &str) -> Vec<String> {
        let digit = |byte: &u8| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte);
        let mut found: Vec<(usize, String)> = Vec::new();
        for tag in ["{K:", "{W:"] {
            for (at, _) in text.match_indices(tag) {
                let Some(code) = text.as_bytes().get(at + 3..at + 8) else { continue };
                if code[..4].iter().all(digit) && code[4] == b'}' {
                    let hex = &text[at + 3..at + 7];
                    let known = u32::from_str_radix(hex, 16).is_ok_and(|code| TABLE.characters.contains_key(&code));
                    if !known {
                        found.push((at, hex.to_string()));
                    }
                }
            }
        }
        found.sort();
        found.into_iter().map(|(_, code)| code).collect()
    }
}

pub mod battle;
pub mod heap;
pub mod insert;
pub mod program;

#[cfg(test)]
mod tests;
