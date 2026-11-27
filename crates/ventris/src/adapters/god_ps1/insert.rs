//! Putting a changed script back into the game: the dialogue of the areas, and the program's own
//! text and the menus' labels, whose room and placing are described in `program`.
//!
//! A new text block. An area's strings are all in its text block, and the game finds a string by
//! its number in the block's table of offsets. So an area with a changed string gets a new text
//! block: the table, each entry giving where its text now is, then the entries one after another in
//! the order of the table. A changed string is written with its zero byte, and one more zero byte
//! where that is needed to reach an even offset. Everything else is copied as the game has it,
//! the unchanged strings with whatever follows them up to the next entry, and the lists of numbers,
//! each of which keeps its place counted in 4 bytes, in case the game reads it in pieces of that
//! size. Entry 1 stays at entry 0's place. When the last entry is a changed string, four zero bytes
//! end the block, as they end the game's.
//!
//! What moves with it. The text block is in the area's code (block A), at the end of its first
//! part, and the member's header says where everything after it is, in 32-bit words:
//!
//! - word 0 is the size of the code, and is the sum of words 1, 2, 4 and 6: word 1 is where the
//!   code's second part starts, just past the text block at the next multiple of 4, and words 2, 4
//!   and 6 are the sizes of the three parts that follow. The game finds those parts from word 1
//!   and from one another, and turns what they hold into addresses when it loads them, counting
//!   from the part, so they can move as a whole;
//! - word 9 is where the rest of the area's data starts in the member: at the first multiple of $800
//!   past the code;
//! - from $29C, 33 entries of 16 bytes; one that is not all zero gives two places in the member, its
//!   second and fourth numbers, both in the rest of the data.
//!
//! So the code's parts after the text block move with it, and words 0 and 1 change by as much; word 9
//! is worked out again; the rest of the data moves by as much as word 9 did, and the two places of
//! each entry with it; the room between the code and word 9 is zero bytes, as the game has it. Both
//! sums are checked before a member is rebuilt and after (`layout_holds`). The game was seen to run
//! area 096 rebuilt this way with a text block 130 KB larger, from a new place on the disc.
//!
//! Where a member goes. The archive's members follow one another, each from the start of a sector,
//! with nothing between. A member that takes no more sectors than it did stays where it is, and zero
//! bytes fill the rest of its last sector, as in the game. One that grew past them goes to the free
//! room of the disc: `DATA/OUT3MIN.DAT`, 32 MB of zero bytes that only fill the disc (the program has
//! no such name in it, and never opens it). Members that grew are put there one after another from
//! its first sector, and the table of contents in the program says where: it counts a member's first
//! sector from the archive's, and the program, which looks the archive up by name when it starts and
//! adds the archive's first sector to entries 0 to 190, reads a member from wherever that is on the
//! disc. Every entry that names the member is changed with it: entries 189 to 195 name the first area
//! as 096 does, and are kept the same as 096. (The program does not add the archive's sector to 191
//! to 195, so they could not be read wherever the member was; nothing seems to read them.)
//!
//! How large an area may grow. The game loads an area's code onto its heap, where it must find room
//! for it among everything else, and most of all when the player goes through a door, as the old
//! area and the new are both there for a while. Every moment of a changed area must leave at least as
//! much of the heap free as the game itself does at its tightest (see `heap`).
//!
//! How a string is fitted to the window (`fit`). The dialogue's window is 16 cells wide and 4 lines
//! high, and every character takes one cell. A spoken line starts with 「 in the first cell, and the
//! lines that follow it start one cell in, so they hold 15 characters. The game draws a character
//! and, when that fills the line, starts the next line at once; when the window is full, a new line
//! scrolls the others up, without waiting. So a line that fills the window followed by a line break
//! would leave an empty line, and the line break is left out; a `{WAIT}` there is written as
//! `{HOLD}`, which waits without starting a line. With wrapping on, the text is broken at spaces to
//! fit, and a `{WAIT}` is put where the window would scroll before the player has read it all: in
//! place of the last line break before that point, or else before the character that would fill the
//! window. With wrapping off a line longer than the window is refused. Strings the pack does not
//! change keep the game's bytes, and are not fitted again.
//!
//! Characters. A pack's letters, digits and punctuation in ASCII are written with the font's
//! full-width forms of them, which are what the game uses (Ａ, ａ, ０, ！, ？, ．…), and a space with
//! the blank the game puts between words. A character the font does not have is drawn into one of
//! the font's 26 blank cells (`BLANK_CODES`), from Unifont unless the build is given another font:
//! each cell is one bit of the 4-bit pictures that are the font (see the module above), so only that
//! bit of its pixels is written, and the other three fonts are left as they were. The font is loaded
//! when the game starts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use super::battle::{BATTLE_SECTION, battle_text};
use super::heap::{self, Footprint, SPARE, Shortfall};
use super::program::{self as program_text, Lines, MENU_SECTION, PROGRAM_SECTION};
use super::{
    ARCHIVE, AREA_HEADER, AREAS, Area, ByteCodes, CONTENTS, Entry, FIRST_CODE, FONT_PAGES, GLYPH_SIZE, NAME_INSERTS,
    PROGRAM, WORD_INSERT, areas, code_of, decode, encode_with, member, program, text_block, word,
};
use crate::adapter::{BuildOptions, Built};
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, CELL, Font, Glyph};
use crate::layout;
use crate::platforms::ps1::{self, Disc, tim};
use crate::script::{Pack, changes_by_place, string_name};
use crate::text::{Token, plain_characters, quoted, tokens};

/// The file of the disc whose room takes the members that grew.
pub const SPARE_FILE: &str = "DATA/OUT3MIN.DAT";
/// Where the header's entries that give places in the member are, and how many there are.
const HEADER_ENTRIES: usize = 0x29C;
const HEADER_ENTRY_COUNT: usize = 33;
/// The header's word 9 is at the first multiple of this past the code.
const REST_ALIGN: usize = 0x800;
const SECTOR: usize = ps1::SECTOR as usize;

/// The dialogue's window: cells across, lines down.
pub const COLUMNS: usize = 16;
pub const ROWS: usize = 4;
/// The cells counted, when fitting a line, for what the game fills in: a name, a number, a word from
/// its list. The longest name the game lets the player give was not measured; those it gives are at
/// most 5 characters long.
pub const NAME_CELLS: usize = 6;
pub const NUMBER_CELLS: usize = 6;
pub const WORD_CELLS: usize = 8;
/// The values of $0C that put in a number.
const NUMBER_INSERTS: [u8; 3] = [0x0D, 0x10, 0x18];

/// The blank cells of the font, by code, that new characters are drawn into: those the character
/// table lists as blank but $04D7, the blank the game puts between words. No string of the game uses
/// them, nor does the table of bytes.
pub const BLANK_CODES: [u32; 26] = [
    0x134, 0x4D3, 0x4D4, 0x4D5, 0x4D6, 0x4FB, 0x4FC, 0x4FD, 0x4FE, 0x4FF, 0x51E, 0x51F, 0x57E, 0x57F, 0x58E, 0x58F,
    0x59F, 0x62F, 0x69F, 0x6FF, 0x73F, 0x7AF, 0x7B2, 0x7C4, 0x7C7, 0x7EF,
];
/// The built-in fonts new characters are drawn from when the build is given none.
pub const FONTS: &[&str] = &["unifont"];
/// The game draws 14 columns and 15 rows of a glyph's cell. A glyph 8 pixels wide is put this far
/// in, so that it is in the middle of what is drawn, as the game's own letters are.
const NARROW_LEFT: usize = 3;

/// What a translator is told about the dialogue.
pub const LIMITS: &str = "the window shows 4 lines of 16 characters, 15 in a spoken line ({SAY}), every character \
                          and space taking the same room; lines are broken at spaces, and a {WAIT} is put in where \
                          the window would fill, counting a name the game fills in as 6 characters, a number as 6 \
                          and a word as 8 (with wrapping off, a line longer than the window is refused); an area's \
                          code, its text included, can grow only as far as the game's heap has room for when the \
                          player goes through a door into or out of the area, loads one of its maps or fights in it: \
                          not at all for areas 105 and 107, a few thousand bytes for areas 131, 154, 157 and 179, and \
                          over 150,000 for half of the areas (docs/packs.md lists the tightest)";

// --- characters ------------------------------------------------------------

/// The character of the font an ASCII character of a pack is written with: its full-width form,
/// the blank between words for a space, and 〜 for a tilde.
fn stand_in(char: char) -> Option<char> {
    match char {
        ' ' => Some('\u{3000}'),
        '~' => Some('〜'),
        '!'..='}' => char::from_u32(char as u32 + 0xFEE0),
        _ => None,
    }
}

/// The code a character of a pack is written with: the font's own, its full-width form's for
/// ASCII, or the cell it was drawn into (`drawn`).
fn pack_code(char: char, codes: &ByteCodes, drawn: &HashMap<char, u32>) -> Option<u32> {
    code_of(char, codes)
        .or_else(|| stand_in(char).and_then(|other| code_of(other, codes)))
        .or_else(|| drawn.get(&char).copied())
}

/// Draw a glyph into the font's cell of a code: `pages` are the two pictures of the font, as the
/// archive has them. Only the bit of each pixel that is the cell's font is written. It fails if the
/// cell is not blank, so that no glyph of the game is drawn over.
fn draw_glyph(pages: &mut [Vec<u8>], code: u32, glyph: &Glyph) -> Result<()> {
    let index = (code - FIRST_CODE) as usize;
    let page = &mut pages[index / 0x400];
    let picture = tim::read(page)?;
    if picture.depth != 4 || picture.width < 16 * GLYPH_SIZE || picture.height < 16 * GLYPH_SIZE {
        return text_error("the game's font is not two pictures of 256 by 256 pixels of 4 bits");
    }
    let (plane, column, row) = ((index >> 8) & 3, index & 0xF, (index >> 4) & 0xF);
    let row_bytes = picture.width / 2;
    let place = |x: usize, y: usize| {
        let (x, y) = (column * GLYPH_SIZE + x, row * GLYPH_SIZE + y);
        (picture.pixels_at + y * row_bytes + x / 2, 1u8 << ((x & 1) * 4 + plane))
    };
    let used = (0..GLYPH_SIZE).any(|y| {
        (0..GLYPH_SIZE).any(|x| {
            let (at, bit) = place(x, y);
            page[at] & bit != 0
        })
    });
    if used {
        return text_error(format!("the cell of the game's font for code {code:04X} is not blank"));
    }
    // Unifont's top row is blank in most glyphs, and its two bottom rows hold descenders; the game
    // leaves a cell's bottom row out, so a glyph goes one row up where it can.
    let up = usize::from(glyph.rows[0] == 0);
    let left = if glyph.width == CELL { 0 } else { NARROW_LEFT };
    for y in 0..GLYPH_SIZE {
        let bits = glyph.rows.get(y + up).copied().unwrap_or(0);
        for x in left..GLYPH_SIZE {
            if bits >> (CELL - 1 - (x - left)) & 1 == 1 {
                let (at, bit) = place(x, y);
                page[at] |= bit;
            }
        }
    }
    Ok(())
}

// --- fitting a string to the window ----------------------------------------

/// What a piece of a string does to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    /// Takes this many cells: a character, or what the game fills in.
    Cells(usize),
    /// $0D, a new line.
    Break,
    /// $0B, a wait, then a new line.
    Wait,
    /// $0F $01, a wait along the same line.
    Hold,
    /// $0F $02, the window cleared.
    Clear,
    /// $0E, which starts a spoken line, drawing 「, or ends one.
    Say,
    /// Takes no room: a pause, a sound, a byte that draws nothing.
    Silent,
}

/// A string's bytes, piece by piece.
fn pieces(data: &[u8], codes: &ByteCodes) -> Vec<(Piece, Vec<u8>)> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let next = data.get(at + 1).copied();
        let (piece, length) = match data[at] {
            0x01..=0x0A => (Piece::Cells(1), 2),
            0x0B => (Piece::Wait, 1),
            0x0C => {
                let cells = match next {
                    Some(value) if NAME_INSERTS.contains(&value) => NAME_CELLS,
                    Some(value) if NUMBER_INSERTS.contains(&value) => NUMBER_CELLS,
                    Some(WORD_INSERT) => WORD_CELLS,
                    _ => 0,
                };
                (if cells == 0 { Piece::Silent } else { Piece::Cells(cells) }, 2)
            }
            0x0D => (Piece::Break, 1),
            0x0E => (Piece::Say, 1),
            0x0F => match next {
                Some(0) => (Piece::Silent, 3),
                Some(1) => (Piece::Hold, 2),
                Some(2) => (Piece::Clear, 2),
                _ => (Piece::Silent, 2),
            },
            byte if byte >= 0x10 && codes.code(byte) != 0 => (Piece::Cells(1), 1),
            _ => (Piece::Silent, 1),
        };
        let end = (at + length).min(data.len());
        found.push((piece, data[at..end].to_vec()));
        at = end;
    }
    found
}

/// How many cells each line of a string takes, counted as `pieces` counts them: a line ends at a
/// line break, a wait that starts a new line, or the window cleared. The program's text is measured
/// this way against the room its parts have (see `program`).
pub fn line_cells(data: &[u8], codes: &ByteCodes) -> Vec<usize> {
    let mut lines = vec![0];
    let mut speaking = false;
    for (piece, _) in pieces(data, codes) {
        match piece {
            Piece::Cells(count) => *lines.last_mut().unwrap() += count,
            Piece::Break | Piece::Wait | Piece::Clear => lines.push(0),
            Piece::Say => {
                *lines.last_mut().unwrap() += usize::from(!speaking);
                speaking = !speaking;
            }
            Piece::Hold | Piece::Silent => {}
        }
    }
    lines
}

/// What laying a string out in the window gave.
#[derive(Debug, PartialEq, Eq)]
enum Laid {
    /// It fits: the bytes to write.
    Fits(Vec<u8>),
    /// The window would scroll at piece `at` before the player is asked to read on; the page, what
    /// has been shown since the last wait, starts at piece `page`.
    Overfull { page: usize, at: usize },
    /// Line `line`, counted from 1, is longer than the window.
    TooLong { line: usize },
}

/// Where the game's pen is in the window, as it draws a string.
#[derive(Default)]
struct Pen {
    column: usize,
    /// Where a new line starts: one cell in, in a spoken line.
    indent: usize,
    /// Lines shown since the last wait, this one included.
    rows: usize,
    /// The last character filled its line, and the game started the next.
    started: bool,
}

impl Pen {
    /// A new line. It returns whether the window has now scrolled past what was shown at the last
    /// wait.
    fn new_line(&mut self) -> bool {
        self.column = self.indent;
        self.rows += 1;
        self.rows > ROWS
    }

    /// A character, which starts a new line when it fills one: as `new_line`.
    fn character(&mut self) -> bool {
        self.started = false;
        self.column += 1;
        if self.column < COLUMNS {
            return false;
        }
        self.started = true;
        self.new_line()
    }

    /// A wait was answered, at the start of line `rows` of the window.
    fn waited(&mut self) {
        self.rows = 1;
    }
}

/// Follow a string as the game draws it (`fit`). With `wrap`, a page that would scroll is reported;
/// without it, a line longer than the window is.
fn lay(pieces: &[(Piece, Vec<u8>)], wrap: bool) -> Laid {
    let mut out = Vec::new();
    let mut pen = Pen { rows: 1, ..Pen::default() };
    let mut page = 0;
    let mut line = 1;
    for (at, (piece, bytes)) in pieces.iter().enumerate() {
        let overfull = Laid::Overfull { page, at };
        let mut cells = 0;
        match piece {
            Piece::Cells(count) => {
                out.extend(bytes);
                cells = *count;
            }
            Piece::Say => {
                out.extend(bytes);
                if pen.indent == 0 {
                    pen.indent = pen.column + 1;
                    cells = 1;
                } else {
                    pen.indent = 0;
                }
            }
            Piece::Break => {
                line += 1;
                if pen.started {
                    pen.started = false;
                } else {
                    out.extend(bytes);
                    if pen.new_line() && wrap {
                        return overfull;
                    }
                }
            }
            Piece::Wait => {
                line += 1;
                out.extend(if pen.started { &[0x0F, 0x01][..] } else { bytes });
                if !pen.started {
                    pen.new_line();
                }
                pen.started = false;
                pen.waited();
                page = at + 1;
            }
            Piece::Hold => {
                out.extend(bytes);
                pen.waited();
                page = at + 1;
            }
            Piece::Clear => {
                line += 1;
                out.extend(bytes);
                (pen.column, pen.started) = (0, false);
                pen.waited();
                page = at + 1;
            }
            Piece::Silent => out.extend(bytes),
        }
        for _ in 0..cells {
            if pen.started && !wrap {
                return Laid::TooLong { line };
            }
            if pen.character() && wrap {
                return overfull;
            }
        }
    }
    Laid::Fits(out)
}

/// A changed string's bytes as they are to be written, fitted to the window (see the module's
/// comment): with `wrap`, waits are put in where the window would scroll unread; without it, a line
/// longer than the window is refused.
fn fit(data: &[u8], codes: &ByteCodes, wrap: bool) -> Result<Vec<u8>> {
    let mut pieces = pieces(data, codes);
    // Each round moves a page's end to an earlier piece or puts one in, so this many is never reached.
    for _ in 0..=2 * pieces.len() + 1 {
        match lay(&pieces, wrap) {
            Laid::Fits(out) => return Ok(out),
            Laid::TooLong { line } => {
                return text_error(format!(
                    "its line {line} is longer than the window, which holds {COLUMNS} characters a line, {} in a \
                     spoken line: break it sooner, or build with wrapping on",
                    COLUMNS - 1
                ));
            }
            Laid::Overfull { page, at } => match (page..=at).rev().find(|index| pieces[*index].0 == Piece::Break) {
                Some(index) => pieces[index] = (Piece::Wait, vec![0x0B]),
                None => pieces.insert(at, (Piece::Wait, vec![0x0B])),
            },
        }
    }
    text_error("it cannot be laid out in the window")
}

/// Stands for one cell of a tag that takes room on screen while a line is broken: the shared
/// `layout::wrap_lines` lets a tag start while the line has room and run past its end, but here a
/// name, a number or a word that would not fit must go to the next line whole, as a word does.
const TAG_CELL: char = '\u{E000}';

/// A changed string's text broken at spaces to the window's width: 15 characters in a spoken line,
/// 16 otherwise. Line breaks and waits already in it are kept.
fn wrap(text: &str) -> Result<String> {
    wrap_to(text, COLUMNS)
}

/// The same, to lines of `columns` cells, one fewer in a spoken line.
fn wrap_to(text: &str, columns: usize) -> Result<String> {
    // The line breaking stands for a tag's cells with this character, so a string cannot have it.
    if text.contains(TAG_CELL) {
        return text_error(format!(
            "it has the character U+{:04X}, which the breaking of lines uses for itself: remove it",
            TAG_CELL as u32
        ));
    }
    let mut speaking = false;
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let spoken = speaking || line.starts_with("{SAY}");
        let limit = if spoken { columns - 1 } else { columns };
        let cells = |name: &str, value: Option<&str>| match name {
            "NAME" => NAME_CELLS,
            "NUM" => NUMBER_CELLS,
            "WORD" => WORD_CELLS,
            "0C" if value.is_some_and(|value| ["10", "18"].contains(&value)) => NUMBER_CELLS,
            "SAY" if !spoken => 1,
            "K" | "W" | "B" => 1,
            _ => 0,
        };
        // Each tag that takes room is measured as that many cells of a word with no space in it,
        // and put back once the line is broken.
        let mut sized = Vec::new();
        let mut measured = String::new();
        for token in tokens(line) {
            match token {
                Token::Tag { raw, name, value } if cells(name, value) > 0 => {
                    let width = cells(name, value);
                    measured.extend(std::iter::repeat_n(TAG_CELL, width));
                    sized.push((raw, width));
                }
                other => measured.push_str(&other.text()),
            }
        }
        let broken = layout::wrap_lines(&measured, &|_| Ok(1), limit, &|_, _| 0, &["WAIT", "CLEAR"])?;
        let mut restored = String::new();
        let (mut sized, mut chars) = (sized.into_iter(), broken.chars());
        while let Some(char) = chars.next() {
            if char != TAG_CELL {
                restored.push(char);
                continue;
            }
            let (raw, width) = sized.next().expect("each run of tag cells stands for a tag");
            restored.push_str(raw);
            for _ in 1..width {
                chars.next();
            }
        }
        lines.push(restored);
        for token in tokens(line) {
            match token {
                Token::Tag { name: "SAY", .. } => speaking = true,
                Token::Tag { name: "/SAY", .. } => speaking = false,
                _ => {}
            }
        }
    }
    Ok(lines.join("\n"))
}

// --- rebuilding an area's member -------------------------------------------

/// A 32-bit word of the header, by number.
fn header_word(member: &[u8], number: usize) -> Option<usize> {
    word(member, number * 4)
}

fn put_word(member: &mut [u8], at: usize, value: usize) {
    member[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

/// Whether an area's member is laid out as its header says, as far as rebuilding it needs: the
/// code's size is the sum of its parts' (words 0, 1, 2, 4 and 6), the rest starts at the first
/// multiple of $800 past it (word 9), the text block is where it should be, and each entry at $29C
/// gives places in the rest.
pub fn layout_holds(member: &[u8]) -> bool {
    let words: Option<Vec<usize>> = (0..10).map(|number| header_word(member, number)).collect();
    let Some(words) = words else { return false };
    let rest = words[9];
    let entries = (0..HEADER_ENTRY_COUNT).all(|index| {
        let at = HEADER_ENTRIES + index * 16;
        let places = [word(member, at + 4), word(member, at + 12)];
        member[at..at + 16].iter().all(|byte| *byte == 0)
            || places.iter().all(|place| place.is_some_and(|place| (rest..=member.len()).contains(&place)))
    });
    words[0] == words[1] + words[2] + words[4] + words[6]
        && rest == (AREA_HEADER + words[0]).next_multiple_of(REST_ALIGN)
        && rest <= member.len()
        && text_block(member).is_some()
        && entries
}

/// An area's member with a new text block, everything after the block moved to make room for it
/// (see the module's comment), or what is wrong with the member.
pub fn rebuild_member(member: &[u8], text: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let laid_out = || "its header does not lay it out as an area's header does".to_string();
    if !layout_holds(member) {
        return Err(laid_out());
    }
    let (start, _) = text_block(member).ok_or_else(laid_out)?;
    let (code, after, rest) = (member_word(member, 0), member_word(member, 1), member_word(member, 9));
    let mut out = member[..start - 4].to_vec();
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(text);
    out.resize(out.len().next_multiple_of(4), 0);
    let new_after = out.len() - AREA_HEADER;
    out.extend(&member[AREA_HEADER + after..AREA_HEADER + code]);
    let new_code = out.len() - AREA_HEADER;
    let new_rest = out.len().next_multiple_of(REST_ALIGN);
    out.resize(new_rest, 0);
    out.extend(&member[rest..]);
    put_word(&mut out, 0, new_code);
    put_word(&mut out, 4, new_after);
    put_word(&mut out, 36, new_rest);
    for index in 0..HEADER_ENTRY_COUNT {
        let at = HEADER_ENTRIES + index * 16;
        if out[at..at + 16].iter().any(|byte| *byte != 0) {
            for place in [at + 4, at + 12] {
                let moved = word(&out, place).unwrap_or_default() as i64 + new_rest as i64 - rest as i64;
                put_word(&mut out, place, moved as usize);
            }
        }
    }
    if !layout_holds(&out) || text_block(&out) != Some((start, text.len())) {
        return Err("rebuilt, it is not laid out as an area's member is".into());
    }
    Ok(out)
}

/// What each area puts on the heap, by entry, as the game has it, read once for an open disc.
pub(super) fn footprints(disc: &Disc) -> Result<Arc<BTreeMap<usize, Footprint>>> {
    disc.keep("god-ps1 footprints", || {
        let program = program(disc)?;
        let first = disc.first_sector(ARCHIVE)?;
        let mut found = BTreeMap::new();
        for entry in AREAS {
            let sector = first + program.contents[entry].0;
            let code = word(&disc.data(sector, 0, AREA_HEADER as u64)?, 0).unwrap_or_default();
            let start = disc.data(sector, 0, (AREA_HEADER + code) as u64)?;
            let Some(footprint) = Footprint::of(&start) else {
                return Err(Error::Image(format!("area {entry:03} is not laid out as an area is")));
            };
            found.insert(entry, footprint);
        }
        Ok(found)
    })
}

/// A word of a member that `layout_holds` has checked is there.
fn member_word(member: &[u8], number: usize) -> usize {
    header_word(member, number).unwrap_or_default()
}

/// An area's text block with some of its strings replaced: `strings` are the new ones, by number,
/// each without its zero byte (see the module's comment).
pub fn text_block_with(area: &Area, strings: &BTreeMap<u32, Vec<u8>>) -> Vec<u8> {
    let old = &area.text;
    let count = area.entries.len();
    let offsets: Vec<usize> = (0..count).map(|number| word(old, number * 4).unwrap_or_default()).collect();
    let mut out = vec![0; count * 4];
    let mut places = vec![0; count];
    let mut ends_changed = false;
    for (number, entry) in area.entries.iter().enumerate() {
        let start = offsets[number];
        let end = offsets[number..].iter().copied().find(|offset| *offset > start).unwrap_or(old.len());
        match (*entry, strings.get(&(number as u32))) {
            (Entry::Same(earlier), _) => {
                places[number] = places[earlier as usize];
                continue;
            }
            (Entry::Text { .. }, Some(bytes)) => {
                places[number] = out.len();
                out.extend(bytes);
                out.push(0);
                out.resize(out.len().next_multiple_of(2), 0);
                ends_changed = true;
                continue;
            }
            (Entry::Numbers, _) => {
                while out.len() % 4 != start % 4 {
                    out.push(0);
                }
            }
            (Entry::Text { .. }, None) => {}
        }
        places[number] = out.len();
        out.extend(&old[start..end]);
        ends_changed = false;
    }
    if ends_changed {
        out.extend([0; 4]);
    }
    for (number, place) in places.iter().enumerate() {
        put_word(&mut out, number * 4, *place);
    }
    out
}

// --- the build -------------------------------------------------------------

/// What a build did, for its summary.
#[derive(Default)]
struct Done {
    strings: usize,
    areas: usize,
    /// The areas that were moved to the free room of the disc.
    moved: Vec<String>,
    /// The least of the heap that a moment of the changed areas leaves free, and which moment it is.
    least: Option<(usize, String)>,
    new_characters: Vec<char>,
    /// The strings of the program's text that changed, how many of them moved, and how many were
    /// written once for several.
    program_strings: usize,
    program_moved: usize,
    program_once: usize,
    labels: usize,
}

impl Done {
    fn summary(&self) -> Vec<String> {
        let mut lines = vec![format!("dialogue:  {} strings changed in {} areas", self.strings, self.areas)];
        if self.program_strings > 0 {
            let once = match self.program_once {
                0 => String::new(),
                once => format!(", {once} of them the same text as another and written once"),
            };
            lines.push(format!(
                "program:   {} strings changed, {} of them moved in the room they share{once}",
                self.program_strings, self.program_moved
            ));
        }
        if self.labels > 0 {
            lines.push(format!("menu:      {} labels changed, each in its slot", self.labels));
        }
        if !self.moved.is_empty() {
            lines.push(format!(
                "moved:     area {} grew past its sectors and is now in the disc's free room ({SPARE_FILE})",
                self.moved.join(", ")
            ));
        }
        if let Some((free, moment)) = &self.least {
            lines.push(format!(
                "room:      {free} bytes of the heap free at the tightest moment, {moment} (the game leaves {SPARE} at its own)"
            ));
        }
        if !self.new_characters.is_empty() {
            let shown: String = self.new_characters.iter().collect();
            lines.push(format!(
                "font:      {} new characters drawn into its blank cells, of {}: {shown}",
                self.new_characters.len(),
                BLANK_CODES.len()
            ));
        }
        lines
    }
}

/// The most bytes a name of the party may take: a new game copies each into a record of 16 bytes,
/// with the zero byte that ends it.
pub const NAME_BYTES: usize = 15;

/// Whether the menus' small font draws a character of a party's name: a capital, a digit, a space,
/// the punctuation it has (see `program::encode_label`), or a kana, also in the full width the
/// game's own text writes them in.
fn in_menu_font(char: char) -> bool {
    let narrow = match char {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(char as u32 - 0xFEE0).unwrap_or(char),
        '\u{3000}' => ' ',
        _ => char,
    };
    narrow.is_ascii_uppercase()
        || narrow.is_ascii_digit()
        || " !\"#$%&'()*+,-./:;<=>?[]".contains(narrow)
        || matches!(narrow, '\u{3041}'..='\u{3096}' | '\u{309B}' | '\u{309C}' | '\u{30A1}'..='\u{30FC}')
        || "。「」、".contains(narrow)
}

/// A changed string of the program's text as it is to be written, fitted to the room its part has
/// on screen (see `program::PARTS`): with `wrap`, broken at spaces to the part's width, and without
/// it, refused where a line is wider. A string with more lines than its part allows is refused, and
/// so is a name of the party too long for its record. `own` is the game's own string.
fn program_string(
    text: &str,
    offset: u32,
    own: &[u8],
    codes: &ByteCodes,
    drawn: &HashMap<char, u32>,
    wrap: bool,
) -> Result<Vec<u8>> {
    let part = program_text::part_of(offset);
    let own_cells = line_cells(own, codes);
    let (width, whose) = match program_text::OWN_WIDTH.contains(&offset) {
        true => (own_cells.iter().copied().max().unwrap_or(0), "its box, as wide as the game's own text,"),
        false => (part.width, "a line of this part"),
    };
    let names = (program_text::NAMES..program_text::NAMES_END).contains(&offset);
    // The record holds the name's characters alone: the menus' font draws no code of the dialogue.
    if names
        && let Some(Token::Tag { raw, .. }) = tokens(text).into_iter().find(|token| matches!(token, Token::Tag { .. }))
    {
        return text_error(format!(
            "it has the tag {raw}, and a name of the party is copied into a record that the menus draw as \
             characters, in a font that has no codes: write the name in characters alone"
        ));
    }
    if names && let Some(char) = text.chars().find(|char| drawn.contains_key(char) || !in_menu_font(*char)) {
        return text_error(format!(
            "it has {}, and a name of the party is also shown in the menus, in a font of capitals, digits, kana \
             and some punctuation that has no lowercase letters, no kanji and none of the characters drawn into \
             the game's font: write it in capitals, digits and kana",
            quoted(&char.to_string())
        ));
    }
    let text = if wrap { wrap_to(text, width)? } else { text.to_string() };
    let data = encode_with(&text, codes, &|char| pack_code(char, codes, drawn))?;
    let lines = line_cells(&data, codes);
    if let Some((line, cells)) = lines.iter().enumerate().find(|(_, cells)| **cells > width) {
        return text_error(match wrap {
            true => format!(
                "broken at spaces, it still has a line of {cells} characters, and {whose} has room for {width}: \
                 shorten its longest word"
            ),
            false => format!(
                "its line {} takes {cells} characters, and {whose} has room for {width}: break it sooner, or build \
                 with wrapping on",
                line + 1
            ),
        });
    }
    let own_lines = own_cells.len();
    let (most, why) = match part.lines {
        Lines::Own => (own_lines, format!("the game's own has {own_lines}, and a string of this part takes no more")),
        Lines::Running(_) if own_lines == 1 => {
            (1, "the game's own has one, a title or an entry of a list, which stays on one line".to_string())
        }
        Lines::Most(most) | Lines::Running(most) => (most, format!("a string of this part has room for {most}")),
    };
    if lines.len() > most {
        let what = match wrap {
            true => format!("shorten it until, broken at spaces, it has no more than {most}"),
            false => format!("join its lines, or shorten it, until it has no more than {most}"),
        };
        return text_error(format!("it has {} lines, and {why}: {what}", lines.len()));
    }
    if names && data.len() > NAME_BYTES {
        return text_error(format!(
            "it takes {} bytes, and a new game copies a name of the party into a record that has room for \
             {NAME_BYTES}: a letter, a digit or a kana takes one byte, and a kanji two",
            data.len()
        ));
    }
    Ok(data)
}

/// Write to `output` the game with the pack's strings in it: the areas' dialogue, the program's
/// own text and the menus' labels. A pack that changes what the bosses say in battle is refused.
pub fn build(disc: &Disc, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
    let program = program(disc)?;
    let codes = &program.codes;
    let areas = areas(disc)?;
    let by_section: HashMap<String, &Area> = areas.iter().map(|area| (area.section(), area)).collect();
    let own_text = program_text::program_text(disc)?;
    let battle_lines = battle_text(disc)?;
    let changes = changes_by_place(&pack.texts, |section| match by_section.get(section) {
        Some(area) => Some(area.strings().into_iter().map(|(number, data)| (number, decode(data, codes))).collect()),
        None if section == PROGRAM_SECTION => {
            Some(own_text[section].iter().map(|(number, data)| (*number, decode(data, codes))).collect())
        }
        None if section == MENU_SECTION => {
            Some(own_text[section].iter().map(|(number, data)| (*number, program_text::decode_small(data))).collect())
        }
        None if section == BATTLE_SECTION => {
            Some(battle_lines.iter().map(|(number, line)| (*number, decode(&line.data, codes))).collect())
        }
        None => None,
    })?;
    let (dialogue, others): (Vec<_>, Vec<_>) =
        changes.into_iter().partition(|(section, ..)| by_section.contains_key(section));
    let (speeches, others): (Vec<_>, Vec<_>) = others.into_iter().partition(|(section, ..)| section == BATTLE_SECTION);
    if !speeches.is_empty() {
        let names: Vec<String> =
            speeches.iter().take(5).map(|(section, number, _)| string_name(section, *number)).collect();
        let more = match speeches.len() - names.len() {
            0 => String::new(),
            more => format!(" and {more} more"),
        };
        return text_error(format!(
            "the pack changes what the bosses say in battle ({}{more}), which cannot be changed yet: the dialogue \
             of the areas, the program's own text and the menus' labels can",
            names.join(", ")
        ));
    }
    let (program_changes, labels): (Vec<_>, Vec<_>) =
        others.into_iter().partition(|(section, ..)| section == PROGRAM_SECTION);
    let mut done = Done {
        strings: dialogue.len(),
        program_strings: program_changes.len(),
        labels: labels.len(),
        ..Done::default()
    };
    let mut patches: Vec<(&str, u64, Vec<u8>)> = Vec::new();

    // The characters the font lacks, drawn into its blank cells. The program's text is drawn with
    // the same font; the menus' labels are not.
    let texts = dialogue.iter().chain(&program_changes).map(|(_, _, text)| text.as_str());
    let used = plain_characters(&texts.collect::<String>());
    let none = HashMap::new();
    let new: Vec<char> = used.into_iter().filter(|char| pack_code(*char, codes, &none).is_none()).collect();
    let mut drawn = HashMap::new();
    if !new.is_empty() {
        let own_fonts;
        let sources = if options.fonts.is_empty() {
            own_fonts = Font::built_ins(FONTS);
            &own_fonts[..]
        } else {
            &options.fonts[..]
        };
        let glyphs = fonts::new_glyphs(sources, &new)?;
        let free: Vec<usize> = BLANK_CODES.iter().map(|code| *code as usize).collect();
        let cells = fonts::assign_slots(&new, &free, "the game's font")?;
        let mut pages: Vec<Vec<u8>> = FONT_PAGES.iter().map(|entry| member(disc, *entry)).collect::<Result<_>>()?;
        for (char, code) in &cells {
            draw_glyph(&mut pages, *code as u32, &glyphs[char])?;
            drawn.insert(*char, *code as u32);
        }
        for (entry, page) in FONT_PAGES.iter().zip(pages) {
            patches.push((ARCHIVE, u64::from(program.contents[*entry].0) * ps1::SECTOR, page));
        }
        done.new_characters = new;
    }

    // The changed strings, each fitted to the window, by area.
    let mut refusals = Vec::new();
    let mut replaced: BTreeMap<&str, BTreeMap<u32, Vec<u8>>> = BTreeMap::new();
    for (section, number, text) in &dialogue {
        let written = (|| {
            let text = if options.wrap { wrap(text)? } else { text.clone() };
            let data = encode_with(&text, codes, &|char| pack_code(char, codes, &drawn))?;
            fit(&data, codes, options.wrap)
        })();
        match written {
            Ok(data) => {
                replaced.entry(section.as_str()).or_default().insert(*number, data);
            }
            Err(Error::Text(problem)) => refusals.push(format!("string {}: {problem}", string_name(section, *number))),
            Err(other) => return Err(other),
        }
    }

    // The program's own text, each string fitted to the room its part has on screen, then all of
    // them written back into the room they share; and the menus' labels, each in its slot.
    if !program_changes.is_empty() || !labels.is_empty() {
        let file = program_text::program_file(disc)?;
        let references = program_text::references(disc)?;
        let mut changed = BTreeMap::new();
        for (section, number, text) in &program_changes {
            let own = &own_text[PROGRAM_SECTION][number];
            match program_string(text, *number, own, codes, &drawn, options.wrap) {
                Ok(data) => {
                    changed.insert(*number, data);
                }
                Err(Error::Text(problem)) => {
                    refusals.push(format!("string {}: {problem}", string_name(section, *number)))
                }
                Err(other) => return Err(other),
            }
        }
        if !changed.is_empty() {
            let rooms = program_text::rooms(&file, &own_text[PROGRAM_SECTION], &references, codes);
            match program_text::write_strings(&file, &rooms, &changed) {
                Ok(written) => {
                    (done.program_moved, done.program_once) = (written.moved, written.once);
                    let to_program = written.patches.into_iter().map(|(at, data)| (PROGRAM, at as u64, data));
                    patches.extend(to_program);
                }
                Err(problem) => refusals.push(problem),
            }
        }
        let own_labels = &own_text[MENU_SECTION];
        for (section, number, text) in &labels {
            let room = program_text::label_room(&file, *number as usize, own_labels, &references.referred);
            let width = program_text::label_width(&file, *number, own_labels);
            match program_text::encode_label(text) {
                Ok(data) if program_text::label_cells(&data) > width => refusals.push(format!(
                    "string {}: it draws {} characters, and the labels of its screen have room for {width}, the most \
                     the game's own take there",
                    string_name(section, *number),
                    program_text::label_cells(&data)
                )),
                Ok(data) if data.len() > room => refusals.push(format!(
                    "string {}: it takes {} bytes, and its slot has room for {room}: each letter, digit, space, kana \
                     and mark takes one, and a kana with ﾞ or ﾟ takes two",
                    string_name(section, *number),
                    data.len()
                )),
                Ok(mut data) => {
                    data.resize(room + 1, 0);
                    patches.push((PROGRAM, u64::from(*number), data));
                }
                Err(Error::Text(problem)) => {
                    refusals.push(format!("string {}: {problem}", string_name(section, *number)))
                }
                Err(other) => return Err(other),
            }
        }
    }

    // Each changed area rebuilt, and put where it fits.
    let mut contents = program.contents.clone();
    let archive_first = disc.first_sector(ARCHIVE)?;
    let spare_first = disc.first_sector(SPARE_FILE)?;
    let spare_sectors = (disc.size(SPARE_FILE)? / ps1::SECTOR) as usize;
    let mut spare_used = 0;
    let mut spare_wanted = 0;
    let mut new_codes = BTreeMap::new();
    for (section, strings) in &replaced {
        let area = by_section[*section];
        let old = member(disc, area.entry)?;
        let rebuilt = match rebuild_member(&old, &text_block_with(area, strings)) {
            Ok(rebuilt) => rebuilt,
            Err(problem) => return Err(Error::Image(format!("area {section}: {problem}"))),
        };
        new_codes.insert(area.entry, member_word(&rebuilt, 0));
        let (sector, size) = program.contents[area.entry];
        let (old_sectors, new_sectors) = ((size as usize).div_ceil(SECTOR), rebuilt.len().div_ceil(SECTOR));
        let mut data = rebuilt.clone();
        let new_sector = if new_sectors <= old_sectors {
            data.resize(old_sectors * SECTOR, 0);
            patches.push((ARCHIVE, u64::from(sector) * ps1::SECTOR, data));
            sector
        } else {
            data.resize(new_sectors * SECTOR, 0);
            spare_wanted += new_sectors;
            if spare_used + new_sectors > spare_sectors {
                continue;
            }
            patches.push((SPARE_FILE, (spare_used * SECTOR) as u64, data));
            let placed = spare_first + spare_used as u32 - archive_first;
            spare_used += new_sectors;
            done.moved.push(section.to_string());
            placed
        };
        for (entry, named) in program.contents.iter().enumerate() {
            if *named == (sector, size) {
                contents[entry] = (new_sector, rebuilt.len() as u32);
            }
        }
        done.areas += 1;
    }
    // Room on the heap for every moment of the areas that changed. A pack that changes no area's
    // code has nothing to check, and the areas are not read for it.
    let changed: BTreeSet<usize> = new_codes.keys().copied().collect();
    let (shortfalls, least) = if changed.is_empty() {
        (Vec::new(), None)
    } else {
        let mut footprints = (*footprints(disc)?).clone();
        for (entry, code) in &new_codes {
            footprints.get_mut(entry).expect("every area has a footprint").code = *code;
        }
        heap::check(&footprints, &changed)
    };
    done.least = least;
    for Shortfall { area, other, moment, over } in shortfalls {
        let also = match other {
            Some(other) if changed.contains(&other) => format!(" (or those of area {other:03})"),
            _ => String::new(),
        };
        refusals.push(format!(
            "area {area:03}: {moment}, the game would need {over} bytes more of its heap than it has room for: \
             shorten some of its changed lines{also}"
        ));
    }
    if spare_wanted > spare_sectors {
        refusals.push(format!(
            "the areas that grew past their sectors need {spare_wanted} sectors of the disc's free room, which has \
             {spare_sectors}: shorten some of the changed lines"
        ));
    }
    if !refusals.is_empty() {
        return text_error(refusals.join("\n"));
    }
    if contents != program.contents {
        let table: Vec<u8> = contents
            .iter()
            .flat_map(|(sector, size)| sector.to_le_bytes().into_iter().chain(size.to_le_bytes()))
            .collect();
        patches.push((PROGRAM, CONTENTS, table));
    }
    ps1::write_image(disc, output, &patches)?;
    Ok(Built { summary: done.summary() })
}

#[cfg(test)]
mod tests;
