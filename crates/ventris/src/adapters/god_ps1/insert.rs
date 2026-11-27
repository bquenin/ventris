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
//! high, and every code takes one cell. A spoken line starts with 「 in the first cell, and the
//! lines that follow it start one cell in, so they hold 15 cells. The game draws a character
//! and, when that fills the line, starts the next line at once; when the window is full, a new line
//! scrolls the others up, without waiting. So a line that fills the window followed by a line break
//! would leave an empty line, and the line break is left out; a `{WAIT}` there is written as
//! `{HOLD}`, which waits without starting a line. With wrapping on, the text is broken at spaces to
//! fit, and a `{WAIT}` is put where the window would scroll before the player has read it all: in
//! place of the last line break before that point, or else before the character that would fill the
//! window. With wrapping off a line longer than the window is refused. Strings the pack does not
//! change keep the game's bytes, and are not fitted again.
//!
//! Two characters to a cell (`Pairs`). A cell is 16 pixels wide, of which the game draws 14, and a
//! Latin letter needs half of that. So in the changed strings of the dialogue and of the program's
//! text (but the party's names, which the menus draw too, from records of their own), the narrow
//! characters, those the game's font lacks and the pack's font draws 8 pixels wide (letters, digits,
//! a space, punctuation, accented letters), go two to a cell: every pair of them that the pack writes
//! side by side is a glyph of its own, drawn from Unifont, each character in 7 of the cell's 14
//! columns with its last one empty (`fonts::narrowed`), the narrowest marks set close to the letter
//! before them (`CLOSE_UP`). A run of them is paired from its start, and ends at anything else: a
//! tag, a line break, a character of the game's own font; but a pause or a sound inside a word does
//! not end it, and is written after the cell of the character before it (`IN_RUN_TAGS`). An odd one
//! at the end of a run has a cell to itself, drawn in its left half, and two spaces are the game's
//! own blank. A pair's cell is written as a kanji is, in two bytes, so two letters take the bytes two
//! full-width letters took, on half the screen.
//!
//! The font is drawn twice: in glyphs of 16 by 16 pixels, members 0 and 1, which the dialogue's
//! window draws, and in glyphs of 12 by 11, member 2, by the same codes, which the battle's window and
//! the screens of the main menu draw the program's text with (see `FONT_MEMBERS`). A new glyph is drawn
//! into both. In the small font each character of a pair has 6 columns and 11 rows, Unifont's 16 rows
//! being brought down to them (`eleven_rows`).
//!
//! The cells pairs are drawn in are those of kanji that no string the pack leaves as it is uses
//! (`free_cells`): not those of the game's other characters, kana, digits, Latin letters,
//! punctuation, which the program also draws itself, the 「 of a spoken line, a number, a name. A
//! full translation frees most of the font's 1,516 kanji, and the game's own text leaves 275 of them
//! unused. If a pack needs more pairs than there are such cells, those it writes most get cells and
//! the others are written a character to a cell, as without pairs; the build says how many. Since
//! where a line breaks decides which characters are paired, lines are broken counting a narrow
//! character as half a cell, then measured as they will be drawn, and broken narrower where a line
//! comes out too wide. The build switch `full-width` writes every character a cell, as the game's own.
//!
//! Characters. Without pairs, a pack's letters, digits and punctuation in ASCII are written with the
//! font's full-width forms of them, which are what the game uses (Ａ, ａ, ０, ！, ？, ．…), and a space
//! with the blank the game puts between words. A character the font does not have, and that is not
//! paired, is drawn into one of the font's 26 blank cells (`BLANK_CODES`), from Unifont unless the
//! build is given another font: each cell is one bit of the 4-bit pictures that are the font (see the
//! module above), so only that bit of its pixels is written, and the other three fonts are left as
//! they were. A pair is drawn the same way, over the kanji that was in its cell. The font is loaded
//! when the game starts.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use super::battle::{BATTLE_SECTION, BattleText, battle_text};
use super::heap::{self, Footprint, SPARE, Shortfall};
use super::program::{self as program_text, Lines, MENU_SECTION, PROGRAM_SECTION, ProgramText};
use super::{
    ARCHIVE, AREA_HEADER, AREAS, Area, ByteCodes, CONTENTS, Entry, FIRST_CODE, FONT_PAGES, GLYPH_COUNT, GLYPH_SIZE,
    NAME_INSERTS, PROGRAM, TABLE, WORD_INSERT, areas, code_bytes, code_of, decode, encode_with, member, program,
    text_block, word,
};
use crate::adapter::{BuildOptions, Built};
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, CELL, Font, Glyph};
use crate::layout;
use crate::platforms::ps1::{self, Disc, tim};
use crate::script::{Pack, changes_by_place, string_name};
use crate::text::{Token, from_hex, plain_characters, quoted, tokens};

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
/// The columns of a cell each of two narrow characters takes: half of the 14 the game draws.
const HALF: usize = 7;
/// The font is drawn twice. Members 0 and 1 are its glyphs of 16 by 16 pixels, which the dialogue's
/// window draws; member 2 holds the same glyphs, by the same codes, in 12 by 11 pixels, for the
/// battle's window and the screens of the main menu, which draw the program's text with it. So a
/// new glyph is drawn into both. Member 2 is a picture of 256 by 256 pixels of 4 bits like the
/// others, each bit a font of 21 cells across and 23 down, 1,932 glyphs in all.
const FONT_MEMBERS: [usize; 3] = [FONT_PAGES[0], FONT_PAGES[1], 2];
const SMALL_WIDTH: usize = 12;
const SMALL_HEIGHT: usize = 11;
const SMALL_COLUMNS: usize = 21;
const SMALL_PER_PLANE: usize = SMALL_COLUMNS * 23;
/// The codes the small font has glyphs for end before this one.
const SMALL_END: u32 = FIRST_CODE + 4 * SMALL_PER_PLANE as u32;
/// A glyph 8 pixels wide is put this far in, in the middle of the small font's 12 columns.
const SMALL_NARROW_LEFT: usize = 2;
/// The build switch that writes every character a cell, narrow ones with the font's full-width glyphs.
pub const FULL_WIDTH: &str = "full-width";
pub const SWITCHES: [(&str, &str); 1] = [(
    FULL_WIDTH,
    "write Latin letters, digits and punctuation a cell each, with the font's full-width glyphs as the game's own \
     text does, instead of two to a cell",
)];
/// What a pair is written with while the pairs a pack needs are counted: a code of two bytes, as
/// the pair's own will be.
const COUNTING_CODE: u32 = 0x08FF;

/// What a translator is told about the dialogue.
pub const LIMITS: &str = "the window shows 4 lines of 16 cells, 15 in a spoken line ({SAY}); Latin letters, digits, \
                          spaces and punctuation go two to a cell, so a line holds about 30 of them (a cell each with \
                          the build option full-width), and a kana, a kanji or a mark of the game's own font takes a \
                          cell; lines are broken at spaces, and a {WAIT} is put in where the window would fill, \
                          counting a name the game fills in as 6 cells, a number as 6 and a word as 8 (with wrapping \
                          off, a line longer than the window is refused); an area's code, its text included, can \
                          grow only as far as the game's heap has room for when the player goes through a door into \
                          or out of the area, loads one of its maps or fights in it: not at all for areas 105 and \
                          107, a few thousand bytes for areas 131, 154, 157 and 179, and over 150,000 for half of the \
                          areas (docs/packs.md lists the tightest)";

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

// --- drawing into the fonts ------------------------------------------------

/// Where a code's glyph is in one of the font's pictures: which picture of `pages` (see
/// `FONT_MEMBERS`), the bit of each pixel that is its font, and its cell's top left corner and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    page: usize,
    plane: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// A code's cell in the font of 16 by 16 pixels, members 0 and 1 (see the module above).
fn large_cell(code: u32) -> Cell {
    let index = (code - FIRST_CODE) as usize;
    let (plane, column, row) = ((index >> 8) & 3, index & 0xF, (index >> 4) & 0xF);
    Cell { page: index / 0x400, plane, x: column * GLYPH_SIZE, y: row * GLYPH_SIZE, width: CELL, height: CELL }
}

/// A code's cell in the font of 12 by 11 pixels, member 2, if it has one: glyph `n` of it, `n` being
/// the code less $100, is in the bit `n / 483`, and at place `r = n % 483` of that bit's picture, 21
/// cells to a row (the routine at $80058828 works it out so).
fn small_cell(code: u32) -> Option<Cell> {
    let index = (code - FIRST_CODE) as usize;
    let (plane, place) = (index / SMALL_PER_PLANE, index % SMALL_PER_PLANE);
    let (x, y) = (place % SMALL_COLUMNS * SMALL_WIDTH, place / SMALL_COLUMNS * SMALL_HEIGHT);
    (plane < 4).then_some(Cell { page: 2, plane, x, y, width: SMALL_WIDTH, height: SMALL_HEIGHT })
}

/// Draw rows of pixels, the leftmost in the top bit, into a cell of the font's pictures (`pages`, as
/// the archive has them), in the bit of each pixel that is the cell's font: the other three fonts
/// in the same pixels are left as they were. With `over`, what the cell held is cleared first;
/// without, it fails if the cell is not blank, so that no glyph of the game is drawn over unasked.
fn draw_cell(pages: &mut [Vec<u8>], cell: Cell, rows: &[u16], over: bool, code: u32) -> Result<()> {
    let page = &mut pages[cell.page];
    let picture = tim::read(page)?;
    if picture.depth != 4 || picture.width < 16 * GLYPH_SIZE || picture.height < 16 * GLYPH_SIZE {
        return text_error("the game's font is not three pictures of 256 by 256 pixels of 4 bits");
    }
    let row_bytes = picture.width / 2;
    let place = |x: usize, y: usize| {
        let (x, y) = (cell.x + x, cell.y + y);
        (picture.pixels_at + y * row_bytes + x / 2, 1u8 << ((x & 1) * 4 + cell.plane))
    };
    let pixels = (0..cell.height).flat_map(|y| (0..cell.width).map(move |x| (x, y)));
    if !over && pixels.clone().any(|(x, y)| page[place(x, y).0] & place(x, y).1 != 0) {
        return text_error(format!("the cell of the game's font for code {code:04X} is not blank"));
    }
    for (x, y) in pixels {
        let (at, bit) = place(x, y);
        page[at] &= !bit;
        if rows.get(y).is_some_and(|row| row >> (CELL - 1 - x) & 1 == 1) {
            page[at] |= bit;
        }
    }
    Ok(())
}

/// Draw a new character into the cells of a code in both fonts. In the large one it fails if the
/// cell is not blank, so that no glyph of the game is drawn over; the small one's cell is the same
/// character's, and is drawn over.
fn draw_glyph(pages: &mut [Vec<u8>], code: u32, glyph: &Glyph) -> Result<()> {
    // Unifont's top row is blank in most glyphs, and its two bottom rows hold descenders; the game
    // leaves a cell's bottom row out, so a glyph goes one row up where it can.
    let up = usize::from(glyph.rows[0] == 0);
    let left = if glyph.width == CELL { 0 } else { NARROW_LEFT };
    let rows: [u16; CELL] = std::array::from_fn(|y| glyph.rows.get(y + up).copied().unwrap_or(0) >> left);
    draw_cell(pages, large_cell(code), &rows, false, code)?;
    if let Some(cell) = small_cell(code) {
        let mut small = fonts::narrowed(glyph, SMALL_WIDTH);
        if glyph.width < CELL {
            small = small.map(|row| row >> SMALL_NARROW_LEFT);
        }
        draw_cell(pages, cell, &eleven_rows(&small), true, code)?;
    }
    Ok(())
}

/// Draw a pair of narrow characters into the cells of a code in both fonts, over the kanji there.
fn draw_pair(pages: &mut [Vec<u8>], code: u32, left: char, right: char, shapes: &BTreeMap<char, Glyph>) -> Result<()> {
    draw_cell(pages, large_cell(code), &pair_rows(left, right, shapes), true, code)?;
    if let Some(cell) = small_cell(code) {
        draw_cell(pages, cell, &small_pair_rows(left, right, shapes), true, code)?;
    }
    Ok(())
}

/// A narrow character fitted to `width` columns (`fonts::narrowed`), its narrowest marks moved to
/// the left (see `CLOSE_UP`).
fn fitted(char: char, glyph: &Glyph, width: usize) -> [u16; CELL] {
    let rows = fonts::narrowed(glyph, width);
    let ink = rows.iter().fold(0, |ink, row| ink | row);
    match CLOSE_UP.contains(char) && ink.leading_zeros() > 1 {
        true => rows.map(|row| row << (ink.leading_zeros() - 1)),
        false => rows,
    }
}

/// The rows of a narrow character in half a cell of the large font, `HALF` columns wide at the left
/// (`fitted`), and one row up, since the game leaves a cell's bottom row out and Unifont's two bottom
/// rows hold descenders. The first empty row goes, which is the top one for most letters and, for a
/// capital with an accent in the top row (É), the one between the accent and the letter, so that
/// every letter stands on the same line.
fn half_rows(char: char, glyph: &Glyph) -> [u16; CELL] {
    let rows = fitted(char, glyph, HALF);
    let Some(empty) = rows.iter().position(|row| *row == 0) else { return rows };
    std::array::from_fn(|y| if y < empty { rows[y] } else { rows.get(y + 1).copied().unwrap_or(0) })
}

/// Unifont puts its narrowest marks in the middle of their cell, which at this size reads as a space
/// before them ("time ."); these start one column in, close to the letter before.
const CLOSE_UP: &str = ".,:;!";

/// The glyph of a pair in the large font: the first character in the left half of the 14 columns
/// the game draws, the second in the right half. `shapes` are the narrow characters' glyphs.
fn pair_rows(left: char, right: char, shapes: &BTreeMap<char, Glyph>) -> [u16; CELL] {
    let half = |char: char| shapes.get(&char).map_or([0; CELL], |glyph| half_rows(char, glyph));
    let (left, right) = (half(left), half(right));
    std::array::from_fn(|y| left[y] | right[y] >> HALF)
}

/// The glyph of a pair in the small font: each character in 6 of the 12 columns, and in 11 rows.
fn small_pair_rows(left: char, right: char, shapes: &BTreeMap<char, Glyph>) -> [u16; SMALL_HEIGHT] {
    let half = |char: char| {
        shapes.get(&char).map_or([0; SMALL_HEIGHT], |glyph| eleven_rows(&fitted(char, glyph, SMALL_WIDTH / 2)))
    };
    let (left, right) = (half(left), half(right));
    std::array::from_fn(|y| left[y] | right[y] >> (SMALL_WIDTH / 2))
}

/// A Unifont glyph's 16 rows brought down to the small font's 11, keeping where letters stand and
/// how tall the small letters are the same in every glyph: of the rows above the small letters
/// (0 to 5) four go, and of theirs (6 to 13) one; the two rows of descenders stay. In each part the
/// rows that go are the empty ones first, from the top, then those the same as the row below, the
/// middlemost first, so that what a letter is made of stays: its crossbar, its bowl, its dots.
fn eleven_rows(rows: &[u16; CELL]) -> [u16; SMALL_HEIGHT] {
    let gone = |range: std::ops::Range<usize>, count: usize| -> Vec<usize> {
        let middle = (range.start + range.end - 1) as f64 / 2.0;
        let mut by_middle: Vec<usize> = range.clone().collect();
        by_middle.sort_by_key(|y| ((*y as f64 - middle).abs() * 2.0) as usize);
        let empty = range.clone().filter(|y| rows[*y] == 0);
        let repeated = by_middle.iter().copied().filter(|y| rows[*y] == rows[*y + 1]);
        let mut found: Vec<usize> = Vec::new();
        for y in empty.chain(repeated).chain(by_middle.iter().copied()) {
            if found.len() < count && !found.contains(&y) {
                found.push(y);
            }
        }
        found
    };
    let mut dropped = gone(0..6, 4);
    dropped.extend(gone(6..14, 1));
    let mut kept = (0..CELL).filter(|y| !dropped.contains(y)).map(|y| rows[y]);
    std::array::from_fn(|_| kept.next().unwrap_or(0))
}

// --- two characters to a cell ----------------------------------------------

/// How the characters of a changed string are written.
#[derive(Clone, Copy)]
pub(super) struct Writing<'a> {
    pub codes: &'a ByteCodes,
    /// The characters drawn into the font's blank cells, a character to a cell, by code.
    pub drawn: &'a HashMap<char, u32>,
    /// The narrow characters two to a cell, or none where every character takes a cell.
    pub pairs: Option<&'a Pairs>,
}

impl Writing<'_> {
    /// The code a character is written with alone.
    fn code(&self, char: char) -> Option<u32> {
        pack_code(char, self.codes, self.drawn)
    }

    fn is_narrow(&self, char: char) -> bool {
        self.pairs.is_some_and(|pairs| pairs.narrow.contains(&char))
    }
}

/// The pairs of narrow characters a build draws (see the module's comment).
#[derive(Default)]
pub(super) struct Pairs {
    /// The characters that go two to a cell.
    pub narrow: BTreeSet<char>,
    /// The code of each pair's cell, by its two characters, the second a space for one alone. None
    /// while the pairs a pack needs are counted: every pair is then taken to have a cell.
    pub cells: Option<HashMap<(char, char), u32>>,
    /// How many times each pair has been written.
    pub counts: RefCell<BTreeMap<(char, char), usize>>,
}

impl Pairs {
    /// The glyph two characters are drawn as, the second a space where the first is alone, or none
    /// for two spaces, which are the game's own blank.
    fn key(left: char, right: Option<char>) -> Option<(char, char)> {
        let key = (left, right.unwrap_or(' '));
        (key != (' ', ' ')).then_some(key)
    }

    fn has_cell(&self, key: (char, char)) -> bool {
        self.cells.as_ref().is_none_or(|cells| cells.contains_key(&key))
    }

    /// How many cells a pair takes: one, or a cell for each character where it has no cell of its own.
    fn cells_of(&self, left: char, right: Option<char>) -> usize {
        match Self::key(left, right) {
            Some(key) if !self.has_cell(key) => 1 + usize::from(right.is_some()),
            _ => 1,
        }
    }

    /// One more time a pair has been written.
    fn count(&self, key: (char, char)) {
        *self.counts.borrow_mut().entry(key).or_default() += 1;
    }

    /// The bytes a pair is written with: its cell's code, or the characters one by one where it has
    /// no cell. Each pair written is counted.
    fn bytes(&self, left: char, right: Option<char>, writing: &Writing) -> Result<Vec<u8>> {
        let Some(key) = Self::key(left, right) else {
            return encode_with(" ", writing.codes, &|char| writing.code(char));
        };
        self.count(key);
        match &self.cells {
            None => Ok(code_bytes(COUNTING_CODE, writing.codes)),
            Some(cells) => match cells.get(&key) {
                Some(code) => Ok(code_bytes(*code, writing.codes)),
                None => {
                    let alone: String = [Some(left), right].into_iter().flatten().collect();
                    encode_with(&alone, writing.codes, &|char| writing.code(char))
                }
            },
        }
    }

    /// The bytes of a pair that has no cell and tags between its two characters: each character in
    /// a cell of its own and the tags between them, where the pack has them. The pair is counted.
    fn bytes_apart(&self, left: char, between: &[&str], right: char, writing: &Writing) -> Result<Vec<u8>> {
        self.count((left, right));
        let text = format!("{left}{}{right}", between.concat());
        encode_with(&text, writing.codes, &|char| writing.code(char))
    }

    /// The characters a pair is written with one to a cell, as `counts` has them: those of each
    /// pair written that has no cell. The space a character alone at the end of its run is paired
    /// with is not written, but a space has a code of the game's own anyway.
    fn written_alone(&self) -> BTreeSet<char> {
        let counts = self.counts.borrow();
        let apart = counts.keys().filter(|key| !self.has_cell(**key));
        apart.flat_map(|(left, right)| [*left, *right]).collect()
    }
}

/// A piece of a changed string as pairs are written: text written as it is; two narrow characters in
/// a cell, the second absent where the first is alone at the end of its run; or two narrow characters
/// that have no cell, with tags between them that stay there (see `IN_RUN_TAGS`).
#[derive(Debug, PartialEq, Eq)]
enum Unit<'a> {
    Text(&'a str),
    Pair(char, Option<char>),
    Apart(char, Vec<&'a str>, char),
}

/// The tags that take no room and leave the reading as it is: a pause and a sound. One of them inside
/// a run of narrow characters does not end it, and is written after the cell of the character
/// before it, so that a word with a pause in it ("Sum{PAUSE:14}mer") is not cut by a half-empty cell.
/// Where two characters that would share a cell find none and are written one to a cell, a tag
/// between them stays there.
const IN_RUN_TAGS: [&str; 2] = ["PAUSE", "0F"];

/// A string cut into units: each run of narrow characters paired from its start, and what is between
/// the runs as text. Anything that is not a narrow character ends a run, but the tags of `IN_RUN_TAGS`.
fn units<'a>(text: &'a str, writing: &Writing) -> Vec<Unit<'a>> {
    let mut out = Vec::new();
    // The run's characters, and the tags within it, each after the character of that number, in order.
    let mut run: Vec<char> = Vec::new();
    let mut within: Vec<(usize, &'a str)> = Vec::new();
    // Where the text not yet in a unit starts, and where the token being read does.
    let (mut start, mut at) = (0, 0);
    let has_cell = |key: (char, char)| writing.pairs.is_none_or(|pairs| pairs.has_cell(key));
    let pair_up = |run: &mut Vec<char>, within: &mut Vec<(usize, &'a str)>, out: &mut Vec<Unit<'a>>| {
        let mut tags = within.iter().copied().peekable();
        for (index, (left, right)) in fonts::pair_up(run).into_iter().enumerate() {
            // The tags after the pair's first character, then those after its second.
            let mut through = |last: usize| -> Vec<&'a str> {
                std::iter::from_fn(|| tags.next_if(|(after, _)| *after <= last).map(|(_, raw)| raw)).collect()
            };
            // Two spaces are the game's own blank whatever is between them, and take one cell.
            match (right, Pairs::key(left, right)) {
                (Some(right), Some(key)) if !has_cell(key) => match through(2 * index) {
                    between if between.is_empty() => out.push(Unit::Pair(left, Some(right))),
                    between => out.push(Unit::Apart(left, between, right)),
                },
                _ => out.push(Unit::Pair(left, right)),
            }
            out.extend(through(2 * index + 1).into_iter().map(Unit::Text));
        }
        run.clear();
        within.clear();
    };
    for token in tokens(text) {
        let length = match token {
            Token::Char(char) => char.len_utf8(),
            Token::Tag { raw, .. } => raw.len(),
        };
        match token {
            Token::Char(char) if writing.is_narrow(char) => {
                if start < at {
                    out.push(Unit::Text(&text[start..at]));
                }
                run.push(char);
                start = at + length;
            }
            Token::Tag { raw, name, .. } if !run.is_empty() && IN_RUN_TAGS.contains(&name) => {
                within.push((run.len() - 1, raw));
                start = at + length;
            }
            _ if !run.is_empty() => pair_up(&mut run, &mut within, &mut out),
            _ => {}
        }
        at += length;
    }
    pair_up(&mut run, &mut within, &mut out);
    if start < at {
        out.push(Unit::Text(&text[start..]));
    }
    out
}

/// Script text as the bytes of a string, without its zero byte, with the narrow characters two to a
/// cell where there are pairs.
fn encode_text(text: &str, writing: &Writing) -> Result<Vec<u8>> {
    let Some(pairs) = writing.pairs else {
        return encode_with(text, writing.codes, &|char| writing.code(char));
    };
    let mut out = Vec::new();
    for unit in units(text, writing) {
        match unit {
            Unit::Text(text) => out.extend(encode_with(text, writing.codes, &|char| writing.code(char))?),
            Unit::Pair(left, right) => out.extend(pairs.bytes(left, right, writing)?),
            Unit::Apart(left, between, right) => out.extend(pairs.bytes_apart(left, &between, right, writing)?),
        }
    }
    Ok(out)
}

/// How many cells a piece of a line takes, as `wrap_to` measures it: each character a cell, but
/// narrow characters, which go two to a cell where there are pairs; tags take none, since those that
/// take room stand there as characters.
fn measured_cells(text: &str, writing: &Writing) -> usize {
    let cells = |text: &str| tokens(text).iter().filter(|token| matches!(token, Token::Char(_))).count();
    let Some(pairs) = writing.pairs else { return cells(text) };
    let unit_cells = |unit: &Unit| match unit {
        Unit::Text(text) => cells(text),
        Unit::Pair(left, right) => pairs.cells_of(*left, *right),
        Unit::Apart(..) => 2,
    };
    units(text, writing).iter().map(unit_cells).sum()
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
                    "its line {line} is longer than the window, which holds {COLUMNS} cells a line, {} in a spoken \
                     line: break it sooner, or build with wrapping on",
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

/// A changed string's text broken at spaces to the window's width: 15 cells in a spoken line, 16
/// otherwise. Line breaks and waits already in it are kept.
fn wrap(text: &str, writing: &Writing) -> Result<String> {
    wrap_to(text, COLUMNS, writing)
}

/// The tags after which the game goes on at the start of a new line, as a line break does.
const NEW_LINE_TAGS: [&str; 2] = ["WAIT", "CLEAR"];

/// The pieces of a broken text that the game draws on lines of their own: its lines, each cut where
/// a wait or a clearing of the window starts a new one.
fn drawn_lines(text: &str) -> Vec<String> {
    let mut found = vec![String::new()];
    for token in tokens(text) {
        match token {
            Token::Char('\n') => found.push(String::new()),
            Token::Tag { name, .. } if NEW_LINE_TAGS.contains(&name) => found.push(String::new()),
            other => found.last_mut().unwrap().push_str(&other.text()),
        }
    }
    found
}

/// The same, to lines of `columns` cells, one fewer in a spoken line.
///
/// Lines are measured in half cells: a narrow character that goes in a pair takes one, everything
/// else two. A line can still come out wider than that measure, as a run of narrow characters with
/// an odd count leaves half a cell empty; so each line, once broken, is measured as it will be drawn
/// (`measured_cells`), and where one that could be broken further is too wide, the whole of the
/// script's line is broken again half a cell narrower.
fn wrap_to(text: &str, columns: usize, writing: &Writing) -> Result<String> {
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
            // An insert written by its byte takes the room the same insert takes under its name.
            "0C" => match value.and_then(|value| u8::from_str_radix(value, 16).ok()) {
                Some(value) if NAME_INSERTS.contains(&value) => NAME_CELLS,
                Some(value) if NUMBER_INSERTS.contains(&value) => NUMBER_CELLS,
                Some(WORD_INSERT) => WORD_CELLS,
                _ => 0,
            },
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
        let halves = |char: char| Ok(if writing.is_narrow(char) { 1 } else { 2 });
        let too_wide = |line: &String| line.contains(' ') && measured_cells(line, writing) > limit;
        let mut room = 2 * limit;
        let broken = loop {
            let broken = layout::wrap_lines(&measured, &halves, room, &|_, _| 0, &NEW_LINE_TAGS)?;
            if writing.pairs.is_none() || room <= 2 || !drawn_lines(&broken).iter().any(too_wide) {
                break broken;
            }
            room -= 1;
        };
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

// --- the cells pairs may take ----------------------------------------------

/// Add to `found` the codes some bytes of the game draw, read as the game reads a string: a code of
/// two bytes, or a byte from $10 up. It reads on past a zero byte, so it can be given what lies
/// between strings, and takes everything that could be a code for one, so that it finds too many
/// rather than too few.
fn codes_drawn(data: &[u8], codes: &ByteCodes, found: &mut BTreeSet<u32>) {
    let mut at = 0;
    while at < data.len() {
        let next = data.get(at + 1).copied();
        at += match (data[at], next) {
            (high @ 0x01..=0x0A, Some(low)) => {
                found.insert(u32::from(high) << 8 | u32::from(low));
                2
            }
            (0x0C, _) => 2,
            (0x0F, Some(0)) => 3,
            (0x0F, _) => 2,
            (byte @ 0x10..=0xFF, _) => {
                found.insert(codes.code(byte));
                1
            }
            _ => 1,
        };
    }
}

/// The text of the game a build leaves as it is, and the strings it changes.
struct Kept<'a> {
    areas: &'a [Area],
    program: &'a ProgramText,
    battle: &'a BattleText,
    /// The areas' strings a pack changes, by section and number, and the program's, by offset.
    dialogue: BTreeSet<(&'a str, u32)>,
    program_strings: BTreeSet<u32>,
}

/// The cells of the font a pair of characters may be drawn in: those of the kanji that no string
/// the pack leaves as it is draws, in any kind of text drawn from this font (the dialogue, the
/// program's text, the bosses' speeches), nor any of the bytes between the program's strings,
/// which hold pieces of text nothing points at, nor the pack's own text. The game's other
/// characters (kana, digits, letters, punctuation, the blank cells) are never taken, since the
/// program draws some of them itself: the 「 of a spoken line, a number's digits, the names it keeps.
/// Nor are the codes the small font has no glyph for. The kanji the pack's changes free come first,
/// then those no string of the game uses.
fn free_cells(file: &[u8], kept: &Kept, texts: &[&str], codes: &ByteCodes) -> Vec<u32> {
    let (mut kept_codes, mut freed) = (BTreeSet::new(), BTreeSet::new());
    for area in kept.areas {
        let section = area.section();
        for (number, data) in area.strings() {
            let found = if kept.dialogue.contains(&(section.as_str(), number)) { &mut freed } else { &mut kept_codes };
            codes_drawn(data, codes, found);
        }
    }
    let strings = &kept.program[PROGRAM_SECTION];
    for (offset, data) in strings {
        let found = if kept.program_strings.contains(offset) { &mut freed } else { &mut kept_codes };
        codes_drawn(data, codes, found);
    }
    for stretch in program_text::TEXT {
        let mut at = stretch.start;
        let starts = strings.range(stretch.start as u32..stretch.end as u32);
        for (offset, data) in starts {
            codes_drawn(&file[at..(*offset as usize).max(at)], codes, &mut kept_codes);
            at = at.max(*offset as usize + data.len() + 1);
        }
        codes_drawn(&file[at.min(stretch.end)..stretch.end], codes, &mut kept_codes);
    }
    for line in kept.battle.values() {
        codes_drawn(&line.data, codes, &mut kept_codes);
    }
    for text in texts {
        for token in tokens(text) {
            let code = match token {
                Token::Char(char) => pack_code(char, codes, &HashMap::new()),
                Token::Tag { name: "K" | "W", value: Some(value), .. } => from_hex(value)
                    .filter(|bytes| bytes.len() == 2)
                    .map(|bytes| u32::from(bytes[0]) << 8 | u32::from(bytes[1])),
                Token::Tag { .. } => None,
            };
            kept_codes.extend(code);
        }
    }
    let kanji = (FIRST_CODE..FIRST_CODE + GLYPH_COUNT as u32)
        .filter(|code| TABLE.characters.get(code).is_some_and(|char| fonts::is_kanji(*char)))
        .filter(|code| !kept_codes.contains(code) && *code < SMALL_END);
    let (first, then): (Vec<u32>, Vec<u32>) = kanji.partition(|code| freed.contains(code));
    first.into_iter().chain(then).collect()
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
    /// The pairs of characters drawn into the font, the cells free for them, and the pairs left to be
    /// written a character to a cell for want of cells.
    pairs: Option<(usize, usize, usize)>,
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
        if let Some((drawn, free, short)) = self.pairs {
            let short = match short {
                0 => String::new(),
                short => format!("; {short} more, those written least, are written a character to a cell"),
            };
            lines.push(format!(
                "pairs:     {drawn} pairs of characters drawn two to a cell, in the cells of kanji no unchanged string \
                 uses, of {free}{short}"
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

/// Whether a string of the program's text is one of the party's names, which a new game copies into
/// records the menus draw too, in their own font: their characters are never paired.
fn is_name(offset: u32) -> bool {
    (program_text::NAMES..program_text::NAMES_END).contains(&offset)
}

/// A changed string of the program's text as it is to be written, fitted to the room its part has
/// on screen (see `program::PARTS`): with `wrap`, broken at spaces to the part's width, and without
/// it, refused where a line is wider. A string with more lines than its part allows is refused, and
/// so is a name of the party too long for its record. `own` is the game's own string.
fn program_string(text: &str, offset: u32, own: &[u8], writing: &Writing, wrap: bool) -> Result<Vec<u8>> {
    let (codes, drawn) = (writing.codes, writing.drawn);
    let names = is_name(offset);
    let writing = &Writing { pairs: if names { None } else { writing.pairs }, ..*writing };
    let part = program_text::part_of(offset);
    let own_cells = line_cells(own, codes);
    let (width, whose) = match program_text::OWN_WIDTH.contains(&offset) {
        true => (own_cells.iter().copied().max().unwrap_or(0), "its box, as wide as the game's own text,"),
        false => (part.width, "a line of this part"),
    };
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
    let text = if wrap { wrap_to(text, width, writing)? } else { text.to_string() };
    let data = encode_text(&text, writing)?;
    let lines = line_cells(&data, codes);
    if let Some((line, cells)) = lines.iter().enumerate().find(|(_, cells)| **cells > width) {
        return text_error(match wrap {
            true => format!(
                "broken at spaces, it still has a line of {cells} cells, and {whose} has room for {width}: \
                 shorten its longest word"
            ),
            false => format!(
                "its line {} takes {cells} cells, and {whose} has room for {width}: break it sooner, or build \
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

    // The fonts new glyphs are drawn from.
    let own_fonts;
    let sources = if options.fonts.is_empty() {
        own_fonts = Font::built_ins(FONTS);
        &own_fonts[..]
    } else {
        &options.fonts[..]
    };
    let none = HashMap::new();
    let all_texts: Vec<&str> = dialogue.iter().chain(&program_changes).map(|(_, _, text)| text.as_str()).collect();
    let write_dialogue = |text: &str, writing: &Writing| {
        let text = if options.wrap { wrap(text, writing)? } else { text.to_string() };
        fit(&encode_text(&text, writing)?, codes, options.wrap)
    };
    let write_program = |text: &str, number: u32, writing: &Writing| match own_text[PROGRAM_SECTION].get(&number) {
        Some(own) => program_string(text, number, own, writing, options.wrap),
        None => text_error("the game has no such string"),
    };

    // The narrow characters, two to a cell (see the module's comment): those the game's font lacks
    // and the pack's font draws narrow, in the dialogue and the program's text but the party's names.
    // Every changed string is written once to count the pairs it needs, as if each had a cell, and
    // each character the font lacks as if it had been drawn, so that one a pack's font may not have
    // does not stop the counting: the strings are refused for it when they are written.
    let used = plain_characters(&all_texts.concat());
    let lacking: Vec<char> = used.iter().copied().filter(|char| pack_code(*char, codes, &none).is_none()).collect();
    let as_if_drawn: HashMap<char, u32> = lacking.iter().map(|char| (*char, COUNTING_CODE)).collect();
    let mut pairs = Pairs::default();
    let mut shapes = BTreeMap::new();
    if !options.switches.contains(FULL_WIDTH) {
        let pairable = dialogue.iter().chain(program_changes.iter().filter(|(_, number, _)| !is_name(*number)));
        let pairable: String = pairable.map(|(_, _, text)| text.as_str()).collect();
        let lacking: Vec<char> = plain_characters(&pairable)
            .into_iter()
            .filter(|char| !matches!(char, '{' | '}' | '\n') && code_of(*char, codes).is_none())
            .collect();
        shapes = fonts::narrow_glyphs(sources, &lacking)?;
        pairs.narrow = shapes.keys().copied().collect();
    }
    let mut pair_cells = BTreeMap::new();
    let mut free_count = None;
    let write_all = |pairs: &Pairs| {
        let writing = Writing { codes, drawn: &as_if_drawn, pairs: Some(pairs) };
        for (_, _, text) in &dialogue {
            let _ = write_dialogue(text, &writing);
        }
        for (_, number, text) in &program_changes {
            let _ = write_program(text, *number, &writing);
        }
    };
    // The narrow characters written one to a cell, in the pairs that find no cell.
    let mut alone = BTreeSet::new();
    if !pairs.narrow.is_empty() {
        write_all(&pairs);
        let counts = pairs.counts.take();
        if !counts.is_empty() {
            let file = program_text::program_file(disc)?;
            let kept = Kept {
                areas: &areas,
                program: &own_text,
                battle: &battle_lines,
                dialogue: dialogue.iter().map(|(section, number, _)| (section.as_str(), *number)).collect(),
                program_strings: program_changes.iter().map(|(_, number, _)| *number).collect(),
            };
            let free: Vec<usize> =
                free_cells(&file, &kept, &all_texts, codes).into_iter().map(|code| code as usize).collect();
            pair_cells = fonts::assign_most_used(&counts, &free);
            free_count = Some(free.len());
        }
        pairs.cells = Some(pair_cells.iter().map(|(key, code)| (*key, *code as u32)).collect());
        // Where some pairs find no cell, the strings are written again with the cells there are, as
        // they will be: a pair with no cell takes two, so lines may break elsewhere and the pairs be
        // others. Where every pair has a cell, the strings break as they did.
        if counts.len() > pair_cells.len() {
            write_all(&pairs);
            alone = pairs.written_alone();
            pairs.counts.take();
        }
    }

    // The characters the font lacks and that are not paired, drawn into its blank cells. The
    // program's text is drawn with the same font; the menus' labels are not. A narrow character is
    // drawn where a pair it is in has no cell.
    let new: Vec<char> =
        lacking.into_iter().filter(|char| !pairs.narrow.contains(char) || alone.contains(char)).collect();
    let mut drawn = HashMap::new();
    let mut pages: Vec<Vec<u8>> = Vec::new();
    if !new.is_empty() || !pair_cells.is_empty() {
        pages = FONT_MEMBERS.iter().map(|entry| member(disc, *entry)).collect::<Result<_>>()?;
    }
    if !new.is_empty() {
        let glyphs = fonts::new_glyphs(sources, &new)?;
        let free: Vec<usize> = BLANK_CODES.iter().map(|code| *code as usize).collect();
        let cells = fonts::assign_slots(&new, &free, "the game's font")?;
        for (char, code) in &cells {
            draw_glyph(&mut pages, *code as u32, &glyphs[char])?;
            drawn.insert(*char, *code as u32);
        }
        done.new_characters = new;
    }
    for ((left, right), code) in &pair_cells {
        draw_pair(&mut pages, *code as u32, *left, *right, &shapes)?;
    }
    for (entry, page) in FONT_MEMBERS.iter().zip(pages) {
        patches.push((ARCHIVE, u64::from(program.contents[*entry].0) * ps1::SECTOR, page));
    }
    let writing = Writing { codes, drawn: &drawn, pairs: (!pairs.narrow.is_empty()).then_some(&pairs) };

    // The changed strings, each fitted to the window, by area.
    let mut refusals = Vec::new();
    let mut replaced: BTreeMap<&str, BTreeMap<u32, Vec<u8>>> = BTreeMap::new();
    for (section, number, text) in &dialogue {
        match write_dialogue(text, &writing) {
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
            match write_program(text, *number, &writing) {
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

    // How many pairs found a cell, and how many of those the strings were written with did not.
    if let Some(free) = free_count {
        let short = pairs.counts.borrow().keys().filter(|key| !pair_cells.contains_key(*key)).count();
        done.pairs = Some((pair_cells.len(), free, short));
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
