//! Ganbare Goemon: Tōkai Dōchū Ōedo Tengurigaeshi no Maki (Nintendo DS, Konami, 2005).
//!
//! What is here was found by taking the game apart; nothing of the game itself is in the repository.
//!
//! # Where the text is
//!
//! The cartridge's file system (`platforms::nds`) is one folder of 1,628 files. All the text the
//! game draws from its font, the scenes, what the townspeople and the shopkeepers say, the help, the
//! save and wireless messages, is in the 172 files named `ss_*.dat`: scripts, one per scene or place,
//! uncompressed. The code (the ARM9 binary and its 15 overlays) is not compressed either, and holds no
//! text. Text drawn into pictures, such as the names of places on the map and the labels of the save
//! slots, is part of those pictures and out of reach of a translation.
//!
//! # A script
//!
//! A script file is code for a small machine, in instructions of one form: a byte that says how many
//! arguments follow, a byte that says what to do, then the arguments, 16 bits each, low byte first.
//! The file is cut into named pieces of code, which the game runs by name. Its first word is the
//! offset of a trailer, marked by the word 0x12345678; the next word of the trailer is the offset of a
//! list of pairs ending with a zero word, each pair the offset of a record and of the piece's name. A
//! record gives the name's offset, the code's offset and its length in 16-bit units. Offsets in a
//! file have their top bit set. The pieces cover every instruction of the file, and nothing else is
//! code.
//!
//! The instruction 0x03 draws characters: each argument is the number of a glyph of the font. Others
//! that come with text:
//!
//! | Instruction | Does | Written in the script as |
//! | --- | --- | --- |
//! | 0x03 | draws its glyphs | the characters |
//! | 0x06 | starts a new line | a line break |
//! | 0x05 | waits, its argument a number of frames | `{WAIT:001E}` |
//! | 0x60 | starts the reading of the last N characters drawn, their furigana | `{RUBY:0002}` |
//! | 0x61 | ends that reading | `{RUBY_END}` |
//!
//! A reading is drawn small, above the characters it belongs to, from a font of its own
//! (`rubyfont.dat`); between `{RUBY:…}` and `{RUBY_END}` are the kana of the reading. A translation
//! leaves both out. A string of the script is a run of text and of the instructions that come with
//! it: 0x03, 0x05, 0x06, 0x60, 0x61, and 0x07, 0x0A, 0x0D, 0x5B and 0xAA, which are seen between the
//! characters of one sentence and whose meaning is still to be learned on screen (0x07 changes how
//! the speaker looks, 0x5B the colour). Such an instruction is written as its number in two
//! hexadecimal digits, with its arguments as four digits each: `{07:0005}`. A string starts at its
//! first character and ends at its last character or reading: what comes before and after it,
//! opening and closing the box, waiting for a button, stays in the game's bytes. Any other instruction
//! ends a string, so that each answer of a choice (`はい`, `いいえ`, placed by the instruction 0x20)
//! is a string of its own.
//!
//! # The font
//!
//! `ssfont.dat` holds the glyphs: 32 bytes of palette, 16 greys from black to white, then pixels at
//! four bits each, in tiles of 8 by 8, the low half of a byte being the pixel on the left. Its records
//! are in `ssssf.dat`: at offset 0 the offset of the records (0xD8), at 4 their number, 1,500, and at
//! 0x18 a table from the ASCII codes 0x20 to 0x7F to glyphs, 0xFFFF where there is none. A record is
//! three 16-bit numbers: where the glyph's pixels start, in units of 32 bytes after the palette, and
//! its width and height. A glyph is 16 by 16 dots, or 8 by 16 for the 38 narrow ones the ASCII table
//! points at (some capital letters, digits and signs). The glyphs come in the order they were first
//! used when the game was made: the digits, four signs and the hiragana first, at 0 to 0x5E, then
//! katakana and kanji mixed. `characters.tbl` gives the character of each, by glyph number.
//!
//! A glyph with no entry in the table is written `{K:0123}`, its number in four hexadecimal digits.
//!
//! # Sections
//!
//! A section is a script file, named as the file is without `ss_` and `.dat`: `demo_a1_n0` is
//! `ss_demo_a1_n0.dat`, a scene of the first area. `ss_create_font.dat`, which holds the messages of
//! the save, the wireless link and the like, is the section `system`. The three test scripts
//! (`ss_test.dat`, `ss_test2.dat`, `ss_test_shop.dat`), left over from the game's making, with
//! sample lines and a test shop, are left out. A string is numbered by where its first instruction is in its file, so the
//! numbers have gaps.
//!
//! # Boxes and lines
//!
//! The script sets up each text window with the instruction 0x67, its corners in dots: the dialogue's
//! box is 160 dots wide, beside the speaker's face (88 to 248) or without one (48 to 208), and the
//! system's messages 224 (16 to 240). A box shows three lines. The game breaks a line that runs past
//! the right edge of its box wherever it is, in the middle of a word too, and when a fourth line
//! comes it waits for a button and goes on in a cleared box; `{0D}{0A}` does the same where the
//! writers wanted a new box. So a build breaks lines at spaces, at 160 and 224 dots (`DIALOGUE_LINE`,
//! `SYSTEM_LINE`), and a translation needs no breaks of its own.
//!
//! # Building
//!
//! A changed script file is laid out again (`rebuild_script`): its pieces of code with the new strings,
//! the records, names and trailer between and after them moved whole, on the same place in a word as
//! before, and every offset outside the code moved with what it points at. The scripts jump to
//! numbered labels (the instruction 0x18 sets one, 0x19 and the conditions name it), never to an
//! offset, so a longer string moves nothing that the code depends on. The file goes back in the
//! cartridge where it fits and after the last file otherwise (`platforms::nds`).
//!
//! A character the game's font lacks is drawn from Unifont and added after the game's 1,500 glyphs:
//! its pixels at the end of `ssfont.dat`, its record at the end of those in `ssssf.dat`, whose count
//! and size of pixels (in units of 64 bytes, at offset 0x0C) are brought up to date. The game draws a
//! glyph in whole tiles of 8 dots and moves on by its width, and a glyph less wide than a tile is not
//! drawn at all, so a Latin letter is 8 dots wide, as Unifont designs it. Every Latin letter, digit
//! and sign a translation uses is drawn this way, also those the font has: the game's own are bold
//! pieces of its emoticons, and do not match. The game reads glyphs from the cartridge as it draws
//! them, through where the font was when the file was opened: a state saved on another build of the
//! game draws from the wrong place once the font has moved, so a build is looked at from power-on.
//!
//! The storyteller's screens before each stage (`stage_demo`) are drawn in columns, top to bottom and
//! right to left: the instruction 0x84 with the argument 1 makes the window being set up write so.
//! The build option `horizontal-storyteller` sets it to 0 in that script, and the game then writes
//! those pages across, 12 letters to a line of the book's page (`STORYTELLER_LINE`), with the lines
//! as far apart as the columns were.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;

use crate::adapter::{Adapter, BuildOptions, Built, CatalogueEntry, Release, TextKind};
use crate::error::{Error, Result, text_error};
use crate::fonts;
use crate::layout;
use crate::platform::{Image, Platform};
use crate::platforms::nds::{self, FileSystem};
use crate::script::{self, Pack, Script};
use crate::text::{Token, from_hex, read_table, tokens};

pub const NAME: &str = "Ganbare Goemon: Tōkai Dōchū Ōedo Tengurigaeshi no Maki (Nintendo DS, Japan)";
pub const ROM_SHA1: &str = "a90e1c3491a3120567385245fdb7302c9866fcd4";
pub const ROM_SIZE: u64 = 0x400_0000;
pub const SOURCE_LANGUAGE: &str = "ja";

/// The files of the font, the glyphs and their records, by name and by number. A glyph is read by the
/// file's number, which a rebuild keeps, so that drawing the font does not read the name table for each.
pub const FONT_FILE: &str = "ssfont.dat";
pub const FONT_RECORDS_FILE: &str = "ssssf.dat";
pub const FONT_FILE_NUMBER: u16 = 1629;
pub const FONT_RECORDS_FILE_NUMBER: u16 = 1630;
pub const GLYPH_SIZE: usize = 16;
pub const GLYPH_COUNT: usize = 1500;
/// The greys of the font's palette, from black to white.
pub static GLYPH_SHADES: [u8; 16] = [0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255];

/// What marks the trailer of a script file.
const TRAILER: u32 = 0x1234_5678;
/// The bit an offset in a script file has set.
const OFFSET_BIT: u32 = 0x8000_0000;

/// The instructions a string is made of.
pub const TEXT: u8 = 0x03;
pub const WAIT: u8 = 0x05;
pub const LINE: u8 = 0x06;
pub const RUBY: u8 = 0x60;
pub const RUBY_END: u8 = 0x61;
/// Instructions seen between the characters of a sentence, which a string carries along.
const INLINE: [u8; 10] = [TEXT, WAIT, LINE, RUBY, RUBY_END, 0x07, 0x0A, 0x0D, 0x5B, 0xAA];
/// The instructions that have a name in the script.
const NAMED: [(u8, &str); 3] = [(WAIT, "WAIT"), (RUBY, "RUBY"), (RUBY_END, "RUBY_END")];

/// The section of the script file that is named for what it holds and not for its file.
const SYSTEM_FILE: &str = "ss_create_font.dat";
const SYSTEM_SECTION: &str = "system";
/// Test scripts left over from the game's making.
const TEST_FILES: [&str; 3] = ["ss_test.dat", "ss_test2.dat", "ss_test_shop.dat"];

/// The section a script file is, or `None` for a file that is not one of the script's.
pub fn section_of(file: &str) -> Option<String> {
    if file == SYSTEM_FILE {
        return Some(SYSTEM_SECTION.into());
    }
    if TEST_FILES.contains(&file) {
        return None;
    }
    file.strip_prefix("ss_")?.strip_suffix(".dat").map(str::to_string)
}

/// The script file a section is.
pub fn file_of(section: &str) -> String {
    if section == SYSTEM_SECTION { SYSTEM_FILE.into() } else { format!("ss_{section}.dat") }
}

// --- the table -------------------------------------------------------------

/// The glyph of the half-width space, which the table cannot give: a line that ends in a space does not
/// survive every editor.
const HALF_SPACE: usize = 0x2FC;

struct Table {
    chars: Vec<Option<char>>,
    glyphs: HashMap<char, u16>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut chars = vec![None; GLYPH_COUNT];
        let mut glyphs = HashMap::new();
        for (key, value) in read_table(include_str!("characters.tbl")) {
            let (Ok(glyph), Some(char)) = (usize::from_str_radix(key, 16), value.chars().next()) else { continue };
            if glyph < GLYPH_COUNT {
                chars[glyph] = Some(char);
                glyphs.entry(char).or_insert(glyph as u16);
            }
        }
        chars[HALF_SPACE] = Some(' ');
        glyphs.insert(' ', HALF_SPACE as u16);
        Table { chars, glyphs }
    })
}

// --- reading a script file -------------------------------------------------

/// One instruction of a script: where it is in its file, what it does, its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub offset: usize,
    pub op: u8,
    pub args: Vec<u16>,
}

impl Instruction {
    pub fn size(&self) -> usize {
        2 + 2 * self.args.len()
    }
}

fn damaged<T>(file: &str, what: &str) -> Result<T> {
    Err(Error::Image(format!("{file} is damaged: {what}")))
}

fn word(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

fn offset_in(data: &[u8], at: usize) -> Option<usize> {
    let value = word(data, at)?;
    (value & OFFSET_BIT != 0).then_some((value & !OFFSET_BIT) as usize)
}

/// The pieces of code of a script file, as (start, end) in the file, in the order the file has them.
pub fn pieces(name: &str, data: &[u8]) -> Result<Vec<(usize, usize)>> {
    let mut found: Vec<(usize, usize)> = records(name, data)?.into_iter().map(|(_, start, end)| (start, end)).collect();
    found.sort_unstable();
    found.dedup();
    Ok(found)
}

/// The records of the pieces of code: where each record is, and where its piece starts and ends.
fn records(name: &str, data: &[u8]) -> Result<Vec<(usize, usize, usize)>> {
    let Some(trailer) = offset_in(data, 0) else { return damaged(name, "it does not start with its trailer's offset") };
    if word(data, trailer) != Some(TRAILER) {
        return damaged(name, "its trailer is not where its first word says");
    }
    let Some(mut entry) = offset_in(data, trailer + 4) else {
        return damaged(name, "its trailer has no list of pieces");
    };
    let mut found = Vec::new();
    while let Some(record) = word(data, entry) {
        if record == 0 {
            break;
        }
        let record = (record & !OFFSET_BIT) as usize;
        let (Some(code), Some(length)) = (offset_in(data, record + 4), word(data, record + 8)) else {
            return damaged(name, "a record of a piece of code is cut short");
        };
        let end = code + 2 * length as usize;
        if end > trailer {
            return damaged(name, "a piece of code runs past the code");
        }
        found.push((record, code, end));
        entry += 8;
    }
    Ok(found)
}

/// The instructions of one piece of code.
pub fn instructions(name: &str, data: &[u8], (start, end): (usize, usize)) -> Result<Vec<Instruction>> {
    let mut found = Vec::new();
    let mut at = start;
    while at < end {
        let count = usize::from(data[at]);
        let next = at + 2 + 2 * count;
        if next > end {
            return damaged(name, &format!("the instruction at {at:#x} runs past the end of its piece"));
        }
        let args = data[at + 2..next].as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
        found.push(Instruction { offset: at, op: data[at + 1], args });
        at = next;
    }
    Ok(found)
}

/// The strings of a piece of code, as ranges of its instructions.
pub fn strings_of(code: &[Instruction]) -> Vec<std::ops::Range<usize>> {
    let mut found = Vec::new();
    let mut index = 0;
    while index < code.len() {
        if code[index].op != TEXT {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index + 1;
        let mut scan = index + 1;
        while scan < code.len() && INLINE.contains(&code[scan].op) {
            if matches!(code[scan].op, TEXT | RUBY_END) {
                end = scan + 1;
            }
            scan += 1;
        }
        found.push(start..end);
        index = scan;
    }
    found
}

/// Each string of a script file: its number, which is its offset in the file, and its instructions.
pub fn file_strings(name: &str, data: &[u8]) -> Result<Vec<(u32, Vec<Instruction>)>> {
    let mut found = Vec::new();
    for piece in pieces(name, data)? {
        let code = instructions(name, data, piece)?;
        for range in strings_of(&code) {
            found.push((code[range.start].offset as u32, code[range].to_vec()));
        }
    }
    Ok(found)
}

// --- text and back ---------------------------------------------------------

fn op_name(op: u8) -> String {
    NAMED.iter().find(|(code, _)| *code == op).map_or_else(|| format!("{op:02X}"), |(_, name)| (*name).to_string())
}

/// The instructions of a string, as text.
pub fn decode(code: &[Instruction]) -> String {
    let table = table();
    let mut out = String::new();
    for instruction in code {
        match instruction.op {
            TEXT => {
                for glyph in &instruction.args {
                    match table.chars.get(usize::from(*glyph)).copied().flatten() {
                        Some(char) => out.push(char),
                        None => out.push_str(&format!("{{K:{glyph:04X}}}")),
                    }
                }
            }
            LINE if instruction.args.is_empty() => out.push('\n'),
            op => {
                out.push('{');
                out.push_str(&op_name(op));
                if !instruction.args.is_empty() {
                    let words: Vec<String> = instruction.args.iter().map(|arg| format!("{arg:04X}")).collect();
                    out.push(':');
                    out.push_str(&words.join(" "));
                }
                out.push('}');
            }
        }
    }
    out
}

fn push_instruction(out: &mut Vec<u8>, op: u8, args: &[u16]) {
    out.push(args.len() as u8);
    out.push(op);
    for arg in args {
        out.extend_from_slice(&arg.to_le_bytes());
    }
}

/// The bytes of a string's instructions, from its text.
pub fn encode(text: &str) -> Result<Vec<u8>> {
    encode_with(text, &HashMap::new())
}

/// The same, with glyphs added to the font for characters the game's own lacks.
pub fn encode_with(text: &str, added: &HashMap<char, u16>) -> Result<Vec<u8>> {
    let table = table();
    let mut out = Vec::new();
    let mut glyphs: Vec<u16> = Vec::new();
    let flush = |out: &mut Vec<u8>, glyphs: &mut Vec<u16>| {
        for chunk in glyphs.chunks(255) {
            push_instruction(out, TEXT, chunk);
        }
        glyphs.clear();
    };
    for token in tokens(text) {
        match token {
            Token::Char('\n') => {
                flush(&mut out, &mut glyphs);
                push_instruction(&mut out, LINE, &[]);
            }
            Token::Char(char) => match added.get(&char).or_else(|| table.glyphs.get(&char)) {
                Some(glyph) => glyphs.push(*glyph),
                None => {
                    return text_error(format!("the game's font has no {}", crate::text::quoted(&char.to_string())));
                }
            },
            Token::Tag { raw, name, value } => {
                let words = match value.map(from_hex) {
                    None => Some(Vec::new()),
                    Some(Some(bytes)) if bytes.len() % 2 == 0 => {
                        Some(bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_be_bytes(*pair)).collect())
                    }
                    Some(_) => None,
                };
                let Some(words) = words else {
                    return text_error(format!("{raw}: an argument is four hexadecimal digits"));
                };
                if name == "K" {
                    match words.as_slice() {
                        [glyph] if usize::from(*glyph) < GLYPH_COUNT + added.len() => glyphs.push(*glyph),
                        _ => return text_error(format!("{raw} is not a glyph of the font")),
                    }
                    continue;
                }
                let op = NAMED
                    .iter()
                    .find(|(_, known)| *known == name)
                    .map(|(code, _)| *code)
                    .or_else(|| (name.len() == 2).then(|| u8::from_str_radix(name, 16).ok()).flatten());
                let Some(op) = op else { return text_error(format!("unknown tag {raw}")) };
                if op == TEXT || words.len() > 255 {
                    return text_error(format!("unknown tag {raw}"));
                }
                flush(&mut out, &mut glyphs);
                push_instruction(&mut out, op, &words);
            }
        }
    }
    flush(&mut out, &mut glyphs);
    Ok(out)
}

fn bytes_of(code: &[Instruction]) -> Vec<u8> {
    let mut out = Vec::new();
    for instruction in code {
        push_instruction(&mut out, instruction.op, &instruction.args);
    }
    out
}

/// The script files of the game: their sections, names and contents.
fn script_files<'a>(rom: &'a [u8], files: &'a FileSystem) -> Result<Vec<(String, &'a str, &'a [u8])>> {
    let mut found = Vec::new();
    for (number, name) in &files.names {
        if let Some(section) = section_of(name) {
            found.push((section, name.as_str(), files.file(rom, *number)?));
        }
    }
    Ok(found)
}

/// The game's strings as they are, by section and number, in the game's bytes.
pub fn original_strings(rom: &[u8]) -> Result<BTreeMap<String, BTreeMap<u32, Vec<u8>>>> {
    let files = FileSystem::read(rom)?;
    let mut found = BTreeMap::new();
    for (section, name, data) in script_files(rom, &files)? {
        let strings = file_strings(name, data)?.into_iter().map(|(number, code)| (number, bytes_of(&code))).collect();
        found.insert(section, strings);
    }
    Ok(found)
}

pub fn extract(rom: &[u8]) -> Result<Pack> {
    let files = FileSystem::read(rom)?;
    let mut texts = Script::new();
    for (section, name, data) in script_files(rom, &files)? {
        let strings = file_strings(name, data)?.into_iter().map(|(number, code)| (number, decode(&code))).collect();
        texts.insert(section, strings);
    }
    Ok(Pack::of(texts))
}

pub fn verify_script(rom: &[u8], texts: &Script) -> Result<()> {
    script::verify_by_place(texts, &original_strings(rom)?, encode)
}

/// The pixels of a glyph, each 0 to 15, in rows of 16; a narrow glyph is in the left half.
pub fn glyph(rom: &[u8], index: usize) -> Result<Vec<Vec<u8>>> {
    let font = nds::file_by_number(rom, FONT_FILE_NUMBER)?;
    let records = nds::file_by_number(rom, FONT_RECORDS_FILE_NUMBER)?;
    let damaged_font = || Error::Image(format!("{FONT_RECORDS_FILE} is damaged"));
    let start = offset_in(records, 0).ok_or_else(damaged_font)?;
    let half = |at: usize| records.get(at..at + 2).map(|pair| usize::from(u16::from_le_bytes([pair[0], pair[1]])));
    let record = start + 6 * index;
    let (Some(unit), Some(width), Some(height)) = (half(record), half(record + 2), half(record + 4)) else {
        return Err(damaged_font());
    };
    let mut rows = vec![vec![0u8; GLYPH_SIZE]; GLYPH_SIZE];
    let (columns, lines) = (width.min(GLYPH_SIZE).div_ceil(8), height.min(GLYPH_SIZE).div_ceil(8));
    let base = 32 + unit * 32;
    for tile_row in 0..lines {
        for tile_column in 0..columns {
            let tile = base + (tile_row * columns + tile_column) * 32;
            for y in 0..8 {
                for x in 0..8 {
                    let byte = font.get(tile + y * 4 + x / 2).copied().unwrap_or(0);
                    rows[tile_row * 8 + y][tile_column * 8 + x] = if x % 2 == 1 { byte >> 4 } else { byte & 0xF };
                }
            }
        }
    }
    Ok(rows)
}

// --- building -------------------------------------------------------------

/// A script file with some of its strings replaced: `replaced` gives, by the string's offset in the
/// file, how many bytes the game's own takes there and the bytes that take its place.
///
/// The file is laid out again region by region, in the order it has them: the pieces of code, with
/// their strings replaced, and what lies between and around them (the header, the records and names of
/// the pieces, the trailer), copied whole, each starting at the same place in a 4-byte word as before,
/// since the game reads words from them. Every offset in those regions is then moved to where what it
/// points at went, and each record is given its piece's new length. No offset points inside a string,
/// and none is kept in the code, whose arguments are 16 bits.
pub fn rebuild_script(name: &str, data: &[u8], replaced: &BTreeMap<usize, (usize, Vec<u8>)>) -> Result<Vec<u8>> {
    let records = records(name, data)?;
    let mut code: Vec<(usize, usize)> = records.iter().map(|(_, start, end)| (*start, *end)).collect();
    code.sort_unstable();
    code.dedup();
    // The regions, in order: (old start, old end, whether it is code).
    let mut regions = Vec::new();
    let mut at = 0;
    for (start, end) in &code {
        if *start < at {
            return damaged(name, "two pieces of code overlap");
        }
        if *start > at {
            regions.push((at, *start, false));
        }
        regions.push((*start, *end, true));
        at = *end;
    }
    regions.push((at, data.len(), false));
    for offset in replaced.keys() {
        if !code.iter().any(|(start, end)| (start..end).contains(&offset)) {
            return damaged(name, &format!("no string starts at {offset:#x}"));
        }
    }

    let mut out = Vec::with_capacity(data.len() + 1024);
    let mut placed = Vec::new(); // (old start, old end, new start, code)
    for (start, end, is_code) in regions {
        if !is_code {
            while out.len() % 4 != start % 4 {
                out.push(0);
            }
        }
        let new_start = out.len();
        if is_code {
            let mut position = start;
            for (offset, (length, bytes)) in replaced.range(start..end) {
                out.extend_from_slice(&data[position..*offset]);
                out.extend_from_slice(bytes);
                position = offset + length;
            }
            out.extend_from_slice(&data[position..end]);
        } else {
            out.extend_from_slice(&data[start..end]);
        }
        placed.push((start, end, new_start, is_code));
    }
    let moved = |target: usize| -> Option<usize> {
        let (start, _, new_start, is_code) =
            placed.iter().find(|(start, end, _, _)| (*start..*end).contains(&target))?;
        // Into code, an offset may only point at the start of a piece.
        (!is_code || target == *start).then(|| new_start + (target - start))
    };
    for (start, end, new_start, is_code) in placed.iter().copied() {
        if is_code {
            continue;
        }
        let mut offset = start.div_ceil(4) * 4;
        while offset + 4 <= end {
            let value = u32::from_le_bytes(data[offset..offset + 4].try_into().expect("four bytes"));
            let target = (value & !OFFSET_BIT) as usize;
            if value & OFFSET_BIT != 0 && target < data.len() {
                let Some(new) = moved(target) else {
                    return damaged(name, &format!("the offset at {offset:#x} points inside a piece of code"));
                };
                let at = new_start + (offset - start);
                out[at..at + 4].copy_from_slice(&(new as u32 | OFFSET_BIT).to_le_bytes());
            }
            offset += 4;
        }
    }
    for (record, start, end) in records {
        let Some(new_record) = moved(record) else {
            return damaged(name, "a record of a piece of code is not where it was");
        };
        let grown: isize =
            replaced.range(start..end).map(|(_, (length, bytes))| bytes.len() as isize - *length as isize).sum();
        let length = (end - start).checked_add_signed(grown).expect("a piece keeps a length");
        out[new_record + 8..new_record + 12].copy_from_slice(&((length / 2) as u32).to_le_bytes());
    }
    Ok(out)
}

/// The glyphs of characters the game's font lacks, as a build adds them: each takes the next number after
/// the game's own, and is drawn from the pack's font in a cell 16 dots tall, 8 or 16 wide.
pub struct AddedGlyphs {
    /// The number each new character is given.
    pub numbers: HashMap<char, u16>,
    glyphs: Vec<fonts::Glyph>,
}

impl AddedGlyphs {
    pub fn new(characters: &[char], sources: &[fonts::Font]) -> Result<AddedGlyphs> {
        if GLYPH_COUNT + characters.len() > usize::from(u16::MAX) {
            return text_error(format!(
                "the pack needs {} new characters and the font has room for {}",
                characters.len(),
                usize::from(u16::MAX) - GLYPH_COUNT
            ));
        }
        let drawn = fonts::new_glyphs(sources, characters)?;
        let mut numbers = HashMap::new();
        let mut glyphs = Vec::new();
        for (index, char) in characters.iter().enumerate() {
            numbers.insert(*char, (GLYPH_COUNT + index) as u16);
            glyphs.push(drawn[char]);
        }
        Ok(AddedGlyphs { numbers, glyphs })
    }

    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// How far the pen moves for a character: the width of the glyph a build gives it, or of the
    /// game's own, which is 16 dots but for the narrow ones, all of them in ASCII.
    pub fn advance(&self, char: char) -> usize {
        match self.numbers.get(&char) {
            Some(number) => self.glyphs[usize::from(*number) - GLYPH_COUNT].width,
            None if char.is_ascii() => 8,
            None => GLYPH_SIZE,
        }
    }

    /// The font's two files with the new glyphs at their ends: the pixels, then the records.
    pub fn extend(&self, font: &[u8], records: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let damaged_font = || Error::Image(format!("{FONT_RECORDS_FILE} is damaged"));
        let start = offset_in(records, 0).ok_or_else(damaged_font)?;
        let count = word(records, 4).ok_or_else(damaged_font)? as usize;
        let end = start + 6 * count;
        if end > records.len() || count != GLYPH_COUNT {
            return Err(damaged_font());
        }
        let mut pixels = font.to_vec();
        // Each glyph starts on a unit of 32 bytes after the palette.
        pixels.resize(32 + (pixels.len() - 32).div_ceil(32) * 32, 0);
        let mut added = Vec::new();
        for glyph in &self.glyphs {
            let unit = (pixels.len() - 32) / 32;
            // The game draws a glyph in whole tiles of 8 dots, as wide as its record says, and moves on by
            // that width: a glyph narrower than a tile is not drawn at all. So a glyph is drawn as its font
            // designed it, in a cell 8 or 16 dots wide.
            let (rows, width) = (glyph.rows, glyph.width);
            let columns = width / 8;
            for tile_row in 0..2 {
                for tile_column in 0..columns {
                    for y in 0..8 {
                        let row = rows[tile_row * 8 + y];
                        for pair in 0..4 {
                            let x = tile_column * 8 + pair * 2;
                            let ink = |x: usize| if row >> (15 - x) & 1 != 0 { INK } else { 0 };
                            pixels.push(ink(x) | ink(x + 1) << 4);
                        }
                    }
                }
            }
            // A record gives where a glyph starts in 16 bits: past that, the font is full.
            let Ok(unit) = u16::try_from(unit) else {
                return text_error("the pack adds more characters than the game's font has room for");
            };
            for value in [unit, width as u16, GLYPH_SIZE as u16] {
                added.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut table = records.to_vec();
        table.splice(end..end, added);
        let total = (count + self.glyphs.len()) as u32;
        table[4..8].copy_from_slice(&total.to_le_bytes());
        // The size of the pixels, in units of 64 bytes, rounded up so that the last glyph is within it.
        let size = (pixels.len() - 32).div_ceil(64) as u32;
        table[12..16].copy_from_slice(&size.to_le_bytes());
        Ok((pixels, table))
    }
}

/// How wide a line of dialogue is, in dots: beside the speaker's face, where most is said, 10 of the
/// game's characters or 20 Latin letters. The game breaks a line that runs past the edge of its box
/// wherever it is, in the middle of a word too, so a build breaks lines at spaces before that.
pub const DIALOGUE_LINE: usize = 160;
/// The same for the system's messages, in the box of the save screen.
pub const SYSTEM_LINE: usize = 224;
/// The same for the storyteller's pages when they are drawn across (`HORIZONTAL`): 96 dots, 12 letters.
pub const STORYTELLER_LINE: usize = 96;
/// The section of the storyteller's screens, and the build option that draws them across.
pub const STORYTELLER: &str = "stage_demo";
pub const HORIZONTAL: &str = "horizontal-storyteller";
/// The instruction that makes the text window being set up write in columns, top to bottom and right
/// to left, with its argument 1; the storyteller's script gives it to both pages of the book.
const VERTICAL: u8 = 0x84;

/// A script file with each instruction `VERTICAL` set to write across: the same bytes, but for its argument.
pub fn written_across(name: &str, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = data.to_vec();
    for piece in pieces(name, data)? {
        for instruction in instructions(name, data, piece)? {
            if instruction.op == VERTICAL && instruction.args == [1] {
                out[instruction.offset + 2..instruction.offset + 4].copy_from_slice(&0u16.to_le_bytes());
            }
        }
    }
    Ok(out)
}

/// The tags after which the game starts a new box, at the left: `{0A}` clears the box.
const NEW_BOX: &[&str] = &["0A"];

/// Stand, while a line is measured, for the second 8 dots of a glyph 16 wide, and for each 8 dots of a
/// glyph written as a `{K:…}` tag.
const WIDE_CELL: char = '\u{E001}';
const TAG_CELL: char = '\u{E000}';

/// A string broken at spaces to a line `limit` dots wide; breaks and tags already in it are kept.
///
/// Every glyph is 8 or 16 dots wide, so a line is measured in cells of 8: a wide character as itself and
/// one `WIDE_CELL`, a `{K:…}` tag as cells of its glyph, put back once the line is broken. The shared
/// `layout::wrap_lines` lets a character start while the line has room; measured in cells, a character
/// that starts in the last cell also ends there, so no glyph runs past the edge of the box.
pub fn wrap(
    text: &str,
    limit: usize,
    advance: &dyn Fn(char) -> usize,
    glyph_width: &dyn Fn(u16) -> usize,
) -> Result<String> {
    let mut measured = String::new();
    let mut tags = Vec::new();
    for token in tokens(text) {
        match token {
            Token::Char(char) if char != '\n' && char != ' ' => {
                measured.push(char);
                measured.extend(std::iter::repeat_n(WIDE_CELL, (advance(char) / 8).saturating_sub(1)));
            }
            Token::Tag { raw, name: "K", value } => {
                let glyph = value.and_then(|value| u16::from_str_radix(value.trim(), 16).ok()).unwrap_or(0);
                let cells = (glyph_width(glyph) / 8).max(1);
                measured.extend(std::iter::repeat_n(TAG_CELL, cells));
                tags.push((raw, cells));
            }
            other => measured.push_str(&other.text()),
        }
    }
    let broken = layout::wrap_lines(&measured, &|_| Ok(1), limit / 8, &|_, _| 0, NEW_BOX)?;
    let mut restored = String::new();
    let (mut tags, mut chars) = (tags.into_iter(), broken.chars());
    while let Some(char) = chars.next() {
        match char {
            WIDE_CELL => {}
            TAG_CELL => {
                let (raw, cells) = tags.next().expect("each run of tag cells stands for a tag");
                restored.push_str(raw);
                for _ in 1..cells {
                    chars.next();
                }
            }
            char => restored.push(char),
        }
    }
    Ok(restored)
}

/// The width of each of the game's glyphs, from the font's records.
fn record_widths(records: &[u8]) -> Result<Vec<usize>> {
    let damaged_font = || Error::Image(format!("{FONT_RECORDS_FILE} is damaged"));
    let start = offset_in(records, 0).ok_or_else(damaged_font)?;
    let count = word(records, 4).ok_or_else(damaged_font)? as usize;
    (0..count)
        .map(|index| {
            let at = start + 6 * index + 2;
            let pair = records.get(at..at + 2).ok_or_else(damaged_font)?;
            Ok(usize::from(u16::from_le_bytes([pair[0], pair[1]])))
        })
        .collect()
}

/// The width of a glyph written as a `{K:…}` tag: one of the game's, or one a build adds.
fn glyph_width(glyph: u16, widths: &[usize], added: &AddedGlyphs) -> usize {
    match widths.get(usize::from(glyph)) {
        Some(width) => *width,
        None => {
            added.glyphs.get(usize::from(glyph).saturating_sub(GLYPH_COUNT)).map_or(GLYPH_SIZE, |glyph| glyph.width)
        }
    }
}

/// The value of a pixel of ink in a glyph a build adds: the brightest of the font's 16.
const INK: u8 = 15;

/// The game with the strings a pack changes, and its result in words.
pub fn build(rom: &[u8], pack: &Pack, options: &BuildOptions) -> Result<(Vec<u8>, Vec<String>)> {
    let files = FileSystem::read(rom)?;
    let originals = extract(rom)?.texts;
    let changes = script::changes_by_place(&pack.texts, |section| originals.get(section).cloned())?;
    // Characters the game's font lacks, in the order they are first met.
    let table = table();
    let mut new_characters: Vec<char> = Vec::new();
    for (_, _, text) in &changes {
        for char in crate::text::plain_characters(text) {
            // The game's own space is kept: it has no ink, so it goes with any font.
            let redrawn = char.is_ascii_graphic();
            if (redrawn || !table.glyphs.contains_key(&char)) && !new_characters.contains(&char) {
                new_characters.push(char);
            }
        }
    }
    new_characters.sort_unstable();
    let sources =
        if options.fonts.is_empty() { fonts::Font::built_ins(ADAPTER.fonts()) } else { options.fonts.clone() };
    let added = AddedGlyphs::new(&new_characters, &sources)?;

    let mut by_section: BTreeMap<&str, BTreeMap<usize, (usize, Vec<u8>)>> = BTreeMap::new();
    let original_bytes = original_strings(rom)?;
    let widths = record_widths(nds::file_by_number(rom, FONT_RECORDS_FILE_NUMBER)?)?;
    for (section, number, text) in &changes {
        let wrapped = options.wrap && ADAPTER.kind_of(section).is_some_and(|kind| kind.wrap);
        let across = options.switches.contains(HORIZONTAL);
        let limit = match section.as_str() {
            _ if is_system(section) => SYSTEM_LINE,
            STORYTELLER if across => STORYTELLER_LINE,
            _ => DIALOGUE_LINE,
        };
        let text = if wrapped {
            wrap(text, limit, &|char| added.advance(char), &|glyph| glyph_width(glyph, &widths, &added))?
        } else {
            text.clone()
        };
        let bytes = encode_with(&text, &added.numbers)
            .map_err(|error| Error::Text(format!("{}: {error}", script::string_name(section, *number))))?;
        let length = original_bytes[section][number].len();
        by_section.entry(section).or_default().insert(*number as usize, (length, bytes));
    }
    let across = options.switches.contains(HORIZONTAL);
    if across {
        by_section.entry(STORYTELLER).or_default();
    }
    let mut replaced = BTreeMap::new();
    for (section, strings) in &by_section {
        let name = file_of(section);
        let Some(number) = files.number(&name) else {
            return Err(Error::Image(format!("the image has no file {name}")));
        };
        let data = files.file(rom, number)?;
        let data = if across && *section == STORYTELLER { written_across(&name, data)? } else { data.to_vec() };
        replaced.insert(number, rebuild_script(&name, &data, strings)?);
    }
    if !added.is_empty() {
        let (pixels, records) = added
            .extend(nds::file_by_number(rom, FONT_FILE_NUMBER)?, nds::file_by_number(rom, FONT_RECORDS_FILE_NUMBER)?)?;
        replaced.insert(FONT_FILE_NUMBER, pixels);
        replaced.insert(FONT_RECORDS_FILE_NUMBER, records);
    }
    let built = files.rebuild(rom, &replaced)?;
    let mut summary = vec![format!("strings:   {} changed in {} script files", changes.len(), by_section.len())];
    if !added.is_empty() {
        let shown: String = new_characters.iter().collect();
        summary.push(format!("font:      {} characters added: {shown}", new_characters.len()));
    }
    Ok((built, summary))
}

// --- what the engine sees --------------------------------------------------

fn is_dialogue(section: &str) -> bool {
    section != SYSTEM_SECTION
}

fn is_system(section: &str) -> bool {
    section == SYSTEM_SECTION
}

/// What limits a string, for a translator; the same for both kinds.
const LIMITS: &str = "a line of dialogue holds 20 Latin letters (10 of the game's own characters), a system message 28; \
                      the build breaks lines at spaces to fit; a box shows three lines, and when a fourth comes the \
                      game waits for a button and goes on in a cleared box; every Latin letter is drawn from Unifont, \
                      8 dots wide";

static KINDS: [TextKind; 2] = [
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        numbered_by_place: true,
        limits: LIMITS,
        trial: Some(("demo_a1_n0.1032", "demo_a1_n0.1060")),
        ..TextKind::new("dialogue", is_dialogue)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        numbered_by_place: true,
        limits: LIMITS,
        trial: Some(("system.2720", "system.2844")),
        ..TextKind::new("system", is_system)
    },
];

/// The note `extract` writes at the head of every file.
const READING_NOTE: &str = "{RUBY:000N}...{RUBY_END} is the reading of the N characters before it, drawn small above them\n\
    (furigana). Leave it out of a translation.";

/// The note `extract` adds at the head of the storyteller's file.
const STORYTELLER_NOTE: &str = "The storyteller's screens before each stage are drawn in vertical columns, read from the right.\n\
    Build with the option horizontal-storyteller to have them written across, 12 Latin letters to a line.";

pub struct Goemon;

pub static ADAPTER: Goemon = Goemon;

impl Adapter for Goemon {
    fn id(&self) -> &'static str {
        "goemon-nds"
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn platform(&self) -> &'static dyn Platform {
        &nds::PLATFORM
    }

    fn images(&self) -> &'static [Release] {
        // No-Intro's name for this SHA-1, from its list of Nintendo DS dumps, which has one dump of the
        // game. Dat-o-Matic shows a record only past a page of terms to accept, so no address is given.
        &[Release {
            sha1: ROM_SHA1,
            name: "Japan",
            size: ROM_SIZE,
            label: "A5EJ",
            version: "0",
            catalogue: Some(CatalogueEntry {
                catalogue: &nds::NO_INTRO,
                name: "Ganbare Goemon - TokaiDouchuu Ooedo Tengurigaeshi no Maki (Japan)",
                page: None,
            }),
        }]
    }

    fn source_language(&self) -> &'static str {
        SOURCE_LANGUAGE
    }

    fn kinds(&self) -> &'static [TextKind] {
        &KINDS
    }

    fn extract(&self, image: &Image) -> Result<Pack> {
        extract(image.cartridge()?)
    }

    fn note(&self, section: &str, number: Option<u32>) -> Option<String> {
        match number {
            Some(_) => None,
            None if section == "stage_demo" => Some(format!("{READING_NOTE}\n{STORYTELLER_NOTE}")),
            None => Some(READING_NOTE.to_string()),
        }
    }

    fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
        verify_script(image.cartridge()?, texts)
    }

    fn write(&self, image: &Image, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
        let (rom, summary) = build(image.cartridge()?, pack, options)?;
        nds::write_rom(output, &rom)?;
        Ok(Built { summary })
    }

    fn fonts(&self) -> &'static [&'static str] {
        &["unifont"]
    }

    fn switches(&self) -> &'static [(&'static str, &'static str)] {
        &[(
            HORIZONTAL,
            "draw the storyteller's screens before each stage across, left to right, where the game draws them in columns",
        )]
    }

    fn glyph_size(&self) -> usize {
        GLYPH_SIZE
    }

    fn glyph_count(&self) -> usize {
        GLYPH_COUNT
    }

    fn glyph_shades(&self) -> &'static [u8] {
        &GLYPH_SHADES
    }

    fn glyph(&self, image: &Image, index: usize) -> Result<Vec<Vec<u8>>> {
        glyph(image.cartridge()?, index)
    }

    fn code_at(&self, index: usize) -> u32 {
        index as u32
    }

    /// A character with no table entry is written `{K:xxxx}`, four hexadecimal digits in capitals.
    fn unmapped(&self, text: &str) -> Vec<String> {
        let digit = |byte: &u8| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte);
        text.match_indices("{K:")
            .filter_map(|(at, _)| {
                let code = text.as_bytes().get(at + 3..at + 8)?;
                (code[..4].iter().all(digit) && code[4] == b'}').then(|| text[at + 3..at + 7].to_string())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
