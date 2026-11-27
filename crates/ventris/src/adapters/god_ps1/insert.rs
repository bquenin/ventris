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
//! bytes fill the rest of its last sector, as in the game. One that grew past them moves, and the
//! table of contents in the program says where: it counts a member's first sector from the
//! archive's, and the program, which looks the archive up by name when it starts and adds the
//! archive's first sector to entries 0 to 190, reads a member from wherever that is on the disc.
//! Every entry that names the member is changed with it: entries 189 to 195 name the first area as
//! 096 does, and are kept the same as 096. (The program does not add the archive's sector to 191 to
//! 195, so they could not be read wherever the member was; nothing seems to read them.)
//!
//! Where a member that grew is put (`place_grown`). A full translation grows nearly every area, by
//! a few sectors each, and an area is large (145 to 2,989 sectors, its maps and pictures with its
//! text): the 93 areas are the last 64,384 of the archive's 72,403 sectors, one after another. So
//! the sectors of every area that moves are free for the others, and those of areas side by side
//! make one piece: the area that needs the most goes first, each into the piece it leaves the least
//! of. The areas no piece is left large enough for go to the free room of the disc, one after
//! another from its first sector: `DATA/OUT3MIN.DAT`, 16,384 sectors (32 MB) of zero bytes that
//! only fill the disc (the program has no such name in it, and never opens it). It is the disc's
//! last file, at sector 191,765. What a pack can grow is bounded by the heap long before it is by
//! the disc: every area grown as far as the heap lets it grows them by about 2,650 sectors in all,
//! and puts 3,033 sectors in the free room. The image is never made larger.
//!
//! How large an area may grow. The game loads an area's code onto its heap, where it must find room
//! for it among everything else, and most of all when the player goes through a door, as the old
//! area and the new are both there for a while. Every moment of a changed area, and of every area
//! where the build puts the renderer in, must leave at least as much of the heap free as the game
//! itself does at its tightest; and the build makes the game read a new area's pictures and other
//! data only once the old area is gone, which leaves its doors far more room (see `heap`).
//!
//! How a string is fitted to the window (`fit`). The dialogue's window is 16 cells wide and 4 lines
//! high, 224 pixels across, a glyph of the game's being 14. A spoken line starts with 「, and the lines
//! that follow it start under its first character, 14 pixels in. The game draws a character and,
//! when that fills the line, starts the next line at once; when the window is full, a new line
//! scrolls the others up, without waiting. So a line that fills the window followed by a line break
//! would leave an empty line, and the line break is left out; a `{WAIT}` there is written as
//! `{HOLD}`, which waits without starting a line. A line is measured in pixels, each character at the
//! width it is drawn (see below), and a character that would run past the window's edge is refused,
//! since the game would draw it there. With wrapping on, the text is broken at spaces to fit
//! (`wrap_to`), and a `{WAIT}` is put where the window would scroll before the player has read it
//! all: in place of the last line break before that point, or else before the character that would
//! fill the window. With wrapping off a line wider than the window is refused. Strings the pack does
//! not change keep the game's bytes, and are not fitted again.
//!
//! Characters at their own widths (see `vwf`). In the changed strings of the dialogue and of the
//! program's text (but the party's names, which the menus draw too, from records of their own), the
//! characters the game's font lacks and the pack's font draws 8 pixels wide (letters, digits, a space,
//! punctuation, accented letters) are drawn each at its own width by a renderer the build puts into
//! the game, a cell of the font each, from Unifont unless the build is given another font. They are
//! written a byte each, in runs (`encode_text`): the byte $0A starts a run, and ends it where a
//! character of the game's own font follows; a line break or a tag ends it anyway. So a letter takes
//! one byte, and each run one more. Every changed string is laid out and measured so, and then the
//! pairs of characters that save the most bytes in the runs of all of them are written a byte each
//! (`paired`, see `vwf::Pairs`), which the game draws as it draws the two characters: so the bytes
//! the build counts, for the program's room and the heap, are those written, and the widths and
//! lines it measured hold. The build switch `full-width` writes every character a cell, as the
//! game's own text does, and puts no renderer in.
//!
//! The font is drawn twice: in glyphs of 16 by 16 pixels, members 0 and 1, which the dialogue's
//! window draws, and in glyphs of 12 by 11, member 2, by the same codes, which the battle's window and
//! the screens of the main menu draw the program's text with (see `FONT_MEMBERS`). A new glyph is drawn
//! into both, Unifont's 16 rows brought down to 11 for the small one (`eleven_rows`).
//!
//! The cells the renderer's characters are drawn in are those of kanji that no string the pack leaves
//! as it is uses (`free_cells`): not those of the game's other characters, kana, digits, Latin
//! letters, punctuation, which the program also draws itself, the 「 of a spoken line, a number, a
//! name. A full translation frees most of the font's 1,516 kanji, and the game's own text leaves 275
//! of them unused; a pack needs about a hundred. One that needs more than there are is refused.
//!
//! Characters. Without the renderer, a pack's letters, digits and punctuation in ASCII are written
//! with the font's full-width forms of them, which are what the game uses (Ａ, ａ, ０, ！, ？, ．…), and
//! a space with the blank the game puts between words. A character the font does not have, and that
//! the renderer does not draw (one 16 pixels wide in the pack's font), is drawn into one of the font's
//! 26 blank cells (`BLANK_CODES`): each cell is one bit of the 4-bit pictures that are the font (see
//! the module above), so only that bit of its pixels is written, and the other three fonts are left
//! as they were. The renderer's characters are drawn the same way, over the kanji that was in their
//! cells. The font is loaded when the game starts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use super::battle::{BATTLE_SECTION, BattleText, battle_text};
use super::heap::{self, Footprint, SPARE, Shortfall};
use super::program::{self as program_text, Lines, MENU_SECTION, PROGRAM_SECTION, ProgramText};
use super::vwf::{self, Pairs, Renderer};
use super::{
    ARCHIVE, AREA_HEADER, AREAS, Area, ByteCodes, CONTENTS, Entry, FIRST_CODE, FONT_PAGES, GLYPH_COUNT, GLYPH_SIZE,
    NAME_INSERTS, PROGRAM, TABLE, WORD_INSERT, areas, code_of, decode, encode_with, member, program, text_block, word,
};
use crate::adapter::{BuildOptions, Built};
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, CELL, Font, Glyph};
use crate::placement;
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
/// Its width in pixels, a glyph of the game's being 14 wide, and the cells a line of its grid has
/// with the renderer (see `vwf`); without it, a line is full at 16 glyphs, before that many.
pub const WINDOW_WIDTH: usize = COLUMNS * vwf::LARGE;
const LINE_CELLS: usize = COLUMNS * vwf::CELLS_PER_COLUMN;
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
     text does, instead of each at its own width",
)];

/// What a translator is told about the dialogue.
pub const LIMITS: &str = "the window shows 4 lines of 224 pixels, 210 in a spoken line ({SAY}); Latin letters, \
                          digits, spaces and punctuation are each drawn at their own width, from 2 to 9 pixels and 4 \
                          for a space, so a line holds about 35 of them (14 pixels each with the build option \
                          full-width), and a kana, a kanji or a mark of the game's own font takes 14; lines are \
                          broken at spaces, and a {WAIT} is put in where the window would fill, counting a name the \
                          game fills in as 84 pixels, a number as 84 and a word as 112 (with wrapping off, a line \
                          wider than the window is refused); an area's code, its text included, can grow only as far \
                          as the game's heap has room for when the player goes through a door into or out of the \
                          area, loads one of its maps or fights in it: by more than 52,000 bytes for every area, and \
                          by more than 180,000 for half of them (docs/packs.md lists the tightest); a letter drawn at \
                          its own width takes one byte, and each run of them one more, a kana one, and the pairs of \
                          letters a pack writes most are written a byte each, which takes about two fifths of the \
                          bytes off English";

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
pub(super) struct Cell {
    page: usize,
    plane: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// A code's cell in the font of 16 by 16 pixels, members 0 and 1 (see the module above).
pub(super) fn large_cell(code: u32) -> Cell {
    let index = (code - FIRST_CODE) as usize;
    let (plane, column, row) = ((index >> 8) & 3, index & 0xF, (index >> 4) & 0xF);
    Cell { page: index / 0x400, plane, x: column * GLYPH_SIZE, y: row * GLYPH_SIZE, width: CELL, height: CELL }
}

/// A code's cell in the font of 12 by 11 pixels, member 2, if it has one: glyph `n` of it, `n` being
/// the code less $100, is in the bit `n / 483`, and at place `r = n % 483` of that bit's picture, 21
/// cells to a row (the routine at $80058828 works it out so).
pub(super) fn small_cell(code: u32) -> Option<Cell> {
    let index = (code - FIRST_CODE) as usize;
    let (plane, place) = (index / SMALL_PER_PLANE, index % SMALL_PER_PLANE);
    let (x, y) = (place % SMALL_COLUMNS * SMALL_WIDTH, place / SMALL_COLUMNS * SMALL_HEIGHT);
    (plane < 4).then_some(Cell { page: 2, plane, x, y, width: SMALL_WIDTH, height: SMALL_HEIGHT })
}

/// Draw rows of pixels, the leftmost in the top bit, into a cell of the font's pictures (`pages`, as
/// the archive has them), in the bit of each pixel that is the cell's font: the other three fonts
/// in the same pixels are left as they were. With `over`, what the cell held is cleared first;
/// without, it fails if the cell is not blank, so that no glyph of the game is drawn over unasked.
pub(super) fn draw_cell(pages: &mut [Vec<u8>], cell: Cell, rows: &[u16], over: bool, code: u32) -> Result<()> {
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

/// A Unifont glyph's 16 rows brought down to the small font's 11, keeping where letters stand and
/// how tall the small letters are the same in every glyph: of the rows above the small letters
/// (0 to 5) four go, and of theirs (6 to 13) one; the two rows of descenders stay. In each part the
/// rows that go are the empty ones first, from the top, then those the same as the row below, the
/// middlemost first, so that what a letter is made of stays: its crossbar, its bowl, its dots.
pub(super) fn eleven_rows(rows: &[u16; CELL]) -> [u16; SMALL_HEIGHT] {
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

// --- how characters are written and measured --------------------------------

/// Which of the game's two fonts a piece of text is measured for: the dialogue's, whose glyphs are
/// 14 pixels wide, or the small one, whose glyphs are 12.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Face {
    Large,
    Small,
}

impl Face {
    /// The width of a glyph of the game's own, and of a cell the game fills in.
    pub(super) fn cell(self) -> usize {
        match self {
            Face::Large => vwf::LARGE,
            Face::Small => vwf::SMALL,
        }
    }
}

/// How the characters of a changed string are written.
#[derive(Clone, Copy)]
pub(super) struct Writing<'a> {
    pub codes: &'a ByteCodes,
    /// The characters drawn into the font's blank cells, a character to a cell, by code.
    pub drawn: &'a HashMap<char, u32>,
    /// The characters drawn at their own widths, or none where every character takes a cell.
    pub renderer: Option<&'a Renderer>,
}

impl Writing<'_> {
    /// The code a character is written with.
    fn code(&self, char: char) -> Option<u32> {
        self.renderer.and_then(|renderer| renderer.code(char)).or_else(|| pack_code(char, self.codes, self.drawn))
    }

    /// How wide a character is drawn, in pixels: its own width, or a glyph's of the game.
    fn width(&self, char: char, font: Face) -> usize {
        self.renderer.and_then(|renderer| renderer.width(char)).unwrap_or(font.cell())
    }

    /// How wide a code is drawn, in pixels.
    fn code_width(&self, code: u32, font: Face) -> usize {
        self.renderer.and_then(|renderer| renderer.code_width(code)).unwrap_or(font.cell())
    }
}

/// Script text as the bytes of a string, without its zero byte. The renderer's characters are
/// written a byte each, in runs (see `vwf`): `vwf::RUN` starts one, and ends it where a character of
/// the game's font follows; a byte under $10 (a line break, a tag) ends it anyway.
///
/// A `{RUN}` the pack writes itself is refused where the build puts no renderer in, as the game
/// would read it as the high half of a code its font does not have, and where what follows it is
/// not a character written a byte from $10 up: the menus' screens read the byte after a `RUN` as a
/// character's, even a zero byte that ends the string.
fn encode_text(text: &str, writing: &Writing) -> Result<Vec<u8>> {
    let run_tag = tokens(text).into_iter().find(|token| matches!(token, Token::Tag { name: "RUN", .. }));
    if let Some(Token::Tag { raw, .. }) = run_tag {
        if writing.renderer.is_none() {
            return text_error(format!(
                "it has {raw}, which starts or ends a run of the characters drawn at their own widths, and this \
                 build draws none: leave it out"
            ));
        }
        let out = encode_runs(text, writing)?;
        let found = pieces(&out, writing);
        let marker = |(piece, bytes): &(Piece, Vec<u8>)| *piece == Piece::Silent && bytes[..] == [vwf::RUN];
        let unfollowed = (0..found.len())
            .filter(|index| marker(&found[*index]))
            .any(|index| !found.get(index + 1).is_some_and(|(_, next)| next.first().is_some_and(|byte| *byte >= 0x10)));
        if unfollowed {
            return text_error(format!(
                "it has a {raw} that is not followed by a character written a byte from $10 up: the end of the \
                 string, a line break, a tag or another {raw} after it would be read as a character's bytes"
            ));
        }
        // In a run, the game reads each byte from $10 up by the renderer's table, whatever the font has.
        let unassigned = found
            .iter()
            .zip(run_states(&found))
            .find(|((piece, bytes), run)| *run && *piece == Piece::Silent && bytes[0] >= 0x10);
        if let Some(((_, bytes), _)) = unassigned {
            return text_error(format!(
                "it has the byte ${:02X} in a run of the characters drawn at their own widths, after a {raw}, and \
                 none of them is written with it",
                bytes[0]
            ));
        }
        return Ok(out);
    }
    encode_runs(text, writing)
}

/// The same, without looking at the pack's own `{RUN}`s.
fn encode_runs(text: &str, writing: &Writing) -> Result<Vec<u8>> {
    let encode = |text: &str| encode_with(text, writing.codes, &|char| writing.code(char));
    let Some(renderer) = writing.renderer else { return encode(text) };
    let (mut out, mut other, mut in_run) = (Vec::new(), String::new(), false);
    let flush = |other: &mut String, out: &mut Vec<u8>, in_run: &mut bool| -> Result<()> {
        if other.is_empty() {
            return Ok(());
        }
        let bytes = encode(other)?;
        if *in_run && bytes.first().is_some_and(|byte| *byte >= 0x10) {
            out.push(vwf::RUN);
        }
        out.extend(bytes);
        // What was written may start or end a run itself, with a `{RUN}` of the pack's.
        *in_run = run_states(&pieces(out, writing)).last().copied().unwrap_or_default();
        other.clear();
        Ok(())
    };
    for token in tokens(text) {
        match token {
            Token::Char(char) if renderer.byte(char).is_some() => {
                flush(&mut other, &mut out, &mut in_run)?;
                if !in_run {
                    out.push(vwf::RUN);
                    in_run = true;
                }
                out.extend(renderer.byte(char));
            }
            other_token => other.push_str(&other_token.text()),
        }
    }
    flush(&mut other, &mut out, &mut in_run)?;
    Ok(out)
}

// --- fitting a string to the window ----------------------------------------

/// What a piece of a string does to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    /// A character, by its code.
    Glyph(u32),
    /// One of the renderer's characters, read in a run, by its code.
    Run(u32),
    /// What the game fills in, taking this many cells: a name, a number, a word.
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

/// A string's bytes, piece by piece, with the runs of the renderer's characters read as they are
/// written (see `encode_text`).
fn pieces(data: &[u8], writing: &Writing) -> Vec<(Piece, Vec<u8>)> {
    let codes = writing.codes;
    let mut found = Vec::new();
    let (mut at, mut run) = (0, false);
    while at < data.len() {
        let next = data.get(at + 1).copied();
        let read_in_run = run && data[at] >= 0x10;
        run = match data[at] {
            vwf::RUN => !run,
            0x00..=0x0F => false,
            _ => run,
        };
        let (piece, length) = match data[at] {
            // A byte the renderer has no character for is read as nothing (see `encode_text`).
            byte if read_in_run && writing.renderer.is_some() => {
                (writing.renderer.and_then(|renderer| renderer.byte_code(byte)).map_or(Piece::Silent, Piece::Run), 1)
            }
            vwf::RUN if writing.renderer.is_some() => (Piece::Silent, 1),
            high @ 0x01..=0x0A => (Piece::Glyph(u32::from(high) << 8 | u32::from(next.unwrap_or(0))), 2),
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
            byte if byte >= 0x10 && codes.code(byte) != 0 => (Piece::Glyph(codes.code(byte)), 1),
            _ => (Piece::Silent, 1),
        };
        let end = (at + length).min(data.len());
        found.push((piece, data[at..end].to_vec()));
        at = end;
    }
    found
}

/// How wide each line of a string is, in pixels, as the game draws it in `font`: a line ends at a
/// line break, a wait that starts a new line, or the window cleared. The program's text is measured
/// this way against the room its parts have (see `program`).
fn line_widths(data: &[u8], writing: &Writing, font: Face) -> Vec<usize> {
    let mut lines = vec![0];
    let mut speaking = false;
    for (piece, _) in pieces(data, writing) {
        let line = lines.last_mut().unwrap();
        match piece {
            Piece::Glyph(code) | Piece::Run(code) => *line += writing.code_width(code, font),
            Piece::Cells(count) => *line += count * font.cell(),
            Piece::Break | Piece::Wait | Piece::Clear => lines.push(0),
            Piece::Say => {
                *line += if speaking { 0 } else { font.cell() };
                speaking = !speaking;
            }
            Piece::Hold | Piece::Silent => {}
        }
    }
    lines
}

/// How many cells each line of a string of the game's takes, every character a cell: what the game's
/// own lines take, which is the room a part of the program's text has (see `program::PARTS`).
pub fn line_cells(data: &[u8], codes: &ByteCodes) -> Vec<usize> {
    let none = HashMap::new();
    let writing = Writing { codes, drawn: &none, renderer: None };
    line_widths(data, &writing, Face::Large).into_iter().map(|width| width / vwf::LARGE).collect()
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

/// Where the game's pen is in the window, as it draws a string: in pixels from the line's start, and
/// in cells of the window's grid.
#[derive(Default)]
struct Pen {
    x: usize,
    cells: usize,
    /// Where a new line starts: past the 「 of a spoken line.
    indent: (usize, usize),
    /// Lines shown since the last wait, this one included.
    rows: usize,
    /// The last character filled its line, and the game started the next.
    started: bool,
}

impl Pen {
    /// A new line. It returns whether the window has now scrolled past what was shown at the last
    /// wait.
    fn new_line(&mut self) -> bool {
        (self.x, self.cells) = self.indent;
        self.rows += 1;
        self.rows > ROWS
    }

    /// A character `width` pixels wide taking `cells` cells, which starts a new line when it fills
    /// one, as the game's writers do: when the pen reaches the window's width, or the line's cells
    /// run out. It returns what `new_line` does.
    fn character(&mut self, width: usize, cells: usize) -> bool {
        self.started = false;
        self.x += width;
        self.cells += cells;
        if self.x < WINDOW_WIDTH && self.cells < LINE_CELLS {
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
/// without it, a line longer than the window is. A character that would run past the window's edge
/// is a line too long either way, since the game draws it there before it starts a new line.
///
/// A line break left out also no longer ends a run of the renderer's characters (see `encode_text`):
/// the run goes on into the next line where another starts there, and is ended by `vwf::RUN` where a
/// character of the game's font follows.
fn lay(pieces: &[(Piece, Vec<u8>)], wrap: bool, writing: &Writing) -> Laid {
    let mut out = Vec::new();
    let mut pen = Pen { rows: 1, ..Pen::default() };
    let mut page = 0;
    let mut line = 1;
    // Whether what is written so far ends in a run, and whether a line break left out ended one.
    let (mut run, mut unended) = (false, false);
    for (at, (piece, bytes)) in pieces.iter().enumerate() {
        let overfull = Laid::Overfull { page, at };
        let first = bytes.first().copied();
        let marker = writing.renderer.is_some() && *piece == Piece::Silent && first == Some(vwf::RUN);
        if std::mem::take(&mut unended) {
            if marker {
                // The next line starts a run: the one left open goes on instead.
                continue;
            }
            if first.is_some_and(|byte| byte >= 0x10) {
                out.push(vwf::RUN);
                run = false;
            }
        }
        match first {
            Some(_) if marker => run = !run,
            Some(byte) if byte < 0x10 && !(*piece == Piece::Break && pen.started) => run = false,
            _ => {}
        }
        let (width, cells) = match piece {
            Piece::Glyph(code) | Piece::Run(code) => {
                out.extend(bytes);
                (writing.code_width(*code, Face::Large), 1)
            }
            Piece::Cells(count) => {
                out.extend(bytes);
                (count * vwf::LARGE, *count)
            }
            Piece::Say => {
                out.extend(bytes);
                if pen.indent == (0, 0) {
                    pen.indent = (pen.x + vwf::LARGE, pen.cells + 1);
                    (vwf::LARGE, 1)
                } else {
                    pen.indent = (0, 0);
                    (0, 0)
                }
            }
            Piece::Break => {
                line += 1;
                if pen.started {
                    pen.started = false;
                    unended = run;
                } else {
                    out.extend(bytes);
                    if pen.new_line() && wrap {
                        return overfull;
                    }
                }
                (0, 0)
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
                (0, 0)
            }
            Piece::Hold => {
                out.extend(bytes);
                pen.waited();
                page = at + 1;
                (0, 0)
            }
            Piece::Clear => {
                line += 1;
                out.extend(bytes);
                (pen.x, pen.cells, pen.started) = (0, 0, false);
                pen.waited();
                page = at + 1;
                (0, 0)
            }
            Piece::Silent => {
                out.extend(bytes);
                (0, 0)
            }
        };
        if cells == 0 {
            continue;
        }
        if (pen.started && !wrap) || pen.x + width > WINDOW_WIDTH {
            return Laid::TooLong { line };
        }
        if pen.character(width, cells) && wrap {
            return overfull;
        }
    }
    Laid::Fits(out)
}

/// A changed string's bytes as they are to be written, fitted to the window (see the module's
/// comment): with `wrap`, waits are put in where the window would scroll unread; without it, a line
/// longer than the window is refused.
fn fit(data: &[u8], writing: &Writing, wrap: bool) -> Result<Vec<u8>> {
    let mut pieces = pieces(data, writing);
    // Each round moves a page's end to an earlier piece or puts one in, so this many is never reached.
    for _ in 0..=2 * pieces.len() + 1 {
        match lay(&pieces, wrap, writing) {
            Laid::Fits(out) => return Ok(out),
            Laid::TooLong { line } if wrap => {
                return text_error(format!(
                    "broken at spaces, its line {line} is still wider than the window, {WINDOW_WIDTH} pixels, {} in \
                     a spoken line: shorten its longest word",
                    WINDOW_WIDTH - vwf::LARGE
                ));
            }
            Laid::TooLong { line } => {
                return text_error(format!(
                    "its line {line} is wider than the window, {WINDOW_WIDTH} pixels, {} in a spoken line: break it \
                     sooner, or build with wrapping on",
                    WINDOW_WIDTH - vwf::LARGE
                ));
            }
            Laid::Overfull { page, at } => match (page..=at).rev().find(|index| pieces[*index].0 == Piece::Break) {
                Some(index) => pieces[index] = (Piece::Wait, vec![0x0B]),
                None => {
                    // A wait ends a run of the renderer's characters: one put before a character of a
                    // run starts the run again after it. A break turned into a wait needs nothing more,
                    // as a break ends a run too and the run is started again after it.
                    let in_run = matches!(pieces[at].0, Piece::Run(_));
                    pieces.insert(at, (Piece::Wait, vec![0x0B]));
                    if in_run {
                        pieces.insert(at + 1, (Piece::Silent, vec![vwf::RUN]));
                    }
                }
            },
        }
    }
    text_error("it cannot be laid out in the window")
}

/// Whether each piece of a string is read in a run of the renderer's characters, and, last, whether
/// the string ends in one: `vwf::RUN` starts or ends a run, and any other byte under $10 ends it.
fn run_states(pieces: &[(Piece, Vec<u8>)]) -> Vec<bool> {
    let mut states = vec![false];
    for (piece, bytes) in pieces {
        let run = states[states.len() - 1];
        states.push(match bytes.first() {
            Some(&vwf::RUN) if *piece == Piece::Silent => !run,
            Some(byte) if *byte < 0x10 => false,
            _ => run,
        });
    }
    states
}

/// Where the runs of the renderer's characters are in a string's bytes: the stretches of bytes each
/// read as one of its characters, one after another (see `pieces`).
fn run_stretches(data: &[u8], writing: &Writing) -> Vec<Range<usize>> {
    let mut found: Vec<Range<usize>> = Vec::new();
    let mut at = 0;
    for (piece, bytes) in pieces(data, writing) {
        if matches!(piece, Piece::Run(_)) {
            match found.last_mut() {
                Some(last) if last.end == at => last.end += bytes.len(),
                _ => found.push(at..at + bytes.len()),
            }
        }
        at += bytes.len();
    }
    found
}

/// A string's bytes, laid out a byte a character, with its runs written in pairs (see
/// `vwf::Pairs`). The game draws a pair as it draws its two characters, a byte each, and ends a
/// line between them where it would between those: so what was measured of the string holds, and
/// only its bytes are fewer.
fn paired(data: &[u8], writing: &Writing, pairs: &Pairs) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    for run in run_stretches(data, writing) {
        out.extend(&data[at..run.start]);
        out.extend(pairs.write(&data[run.clone()]));
        at = run.end;
    }
    out.extend(&data[at..]);
    out
}

/// The pairs that save the most in the runs of the areas' strings that changed, `replaced`, and of
/// the program's, `changed`, put into them (see `vwf::Pairs` and `paired`).
///
/// A program string that the program also reads from a place inside (see
/// `program_text::Room::inside`, `rooms` giving each string's) is left a byte a character, and its
/// runs are not counted: its bytes from that place on must stay where they are, and a pair before
/// it would move them.
fn put_pairs(
    replaced: &mut BTreeMap<&str, BTreeMap<u32, Vec<u8>>>,
    changed: &mut BTreeMap<u32, Vec<u8>>,
    rooms: &BTreeMap<u32, program_text::Room>,
    writing: &Writing,
    renderer: &Renderer,
) -> Pairs {
    let read_inside = |offset: &u32| rooms.get(offset).is_some_and(|room| room.inside.is_some());
    let pairable: Vec<u32> = changed.keys().filter(|offset| !read_inside(offset)).copied().collect();
    let strings = replaced.values().flat_map(BTreeMap::values).chain(pairable.iter().map(|offset| &changed[offset]));
    let runs: Vec<Vec<u8>> =
        strings.flat_map(|data| run_stretches(data, writing).into_iter().map(|run| data[run].to_vec())).collect();
    let pairs = Pairs::choose(renderer, &runs);
    for data in replaced.values_mut().flat_map(BTreeMap::values_mut) {
        *data = paired(data, writing, &pairs);
    }
    for offset in &pairable {
        let data = changed.get_mut(offset).expect("a changed string");
        *data = paired(data, writing, &pairs);
    }
    pairs
}

/// The tags after which the game goes on at the start of a new line, as a line break does.
const NEW_LINE_TAGS: [&str; 2] = ["WAIT", "CLEAR"];

/// A changed string's text broken at spaces to the window's width. Line breaks and waits already in
/// it are kept.
fn wrap(text: &str, writing: &Writing) -> Result<String> {
    wrap_to(text, WINDOW_WIDTH, LINE_CELLS, writing, Face::Large)
}

/// The same, to lines `width` pixels wide and `line_cells` cells long in `font`, each character
/// measured at the width it is drawn and taking a cell, and each cell of what the game fills in at a
/// glyph's. A line starts where the game starts it: at the window's left edge, or, in a spoken line,
/// under its first character, past the 「 that `{SAY}` draws wherever it is; a `{WAIT}` starts a new
/// line there too, and a `{CLEAR}` at the left edge. A name, a number or a word the game fills in is
/// part of the word it is in. A word wider than a line is left as it is, for `fit` to refuse.
fn wrap_to(text: &str, width: usize, line_cells: usize, writing: &Writing, font: Face) -> Result<String> {
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
        "K" | "W" | "B" => 1,
        _ => 0,
    };
    // What a character or a tag that is neither a space nor `{SAY}` takes of a line: pixels, cells.
    let size = |token: &Token| match token {
        Token::Char(char) => (writing.width(*char, font), 1),
        Token::Tag { name, value, .. } => (cells(name, *value) * font.cell(), cells(name, *value)),
    };
    // Where a line starts, in pixels and cells: past the 「 of a spoken line, at the edge otherwise.
    let start = |speaking: bool, indent: (usize, usize)| if speaking { indent } else { (0, 0) };
    let mut out = String::new();
    let (mut speaking, mut indent) = (false, (0, 0));
    for (number, line) in text.split('\n').enumerate() {
        if number > 0 {
            out.push('\n');
        }
        let (mut x, mut used) = start(speaking, indent);
        // Where the line's last space is in `out`, and whether a spoken line was open there, with its
        // indent.
        let mut space = None;
        for token in tokens(line) {
            match token {
                Token::Char(' ') => {
                    let step = writing.width(' ', font);
                    if x + step > width || used + 1 > line_cells {
                        // The space falls off the end of the line: the next one starts here.
                        out.push('\n');
                        (x, used) = start(speaking, indent);
                        space = None;
                    } else {
                        space = Some((out.len(), speaking, indent));
                        out.push(' ');
                        (x, used) = (x + step, used + 1);
                    }
                    continue;
                }
                Token::Tag { name, raw, .. } if NEW_LINE_TAGS.contains(&name) => {
                    out.push_str(raw);
                    (x, used) = if name == "CLEAR" { (0, 0) } else { start(speaking, indent) };
                    space = None;
                    continue;
                }
                _ => {}
            }
            // `{SAY}` draws a 「 that takes a glyph's room, and its line goes on past it; `{/SAY}` takes none.
            let say = matches!(token, Token::Tag { name: "SAY" | "/SAY", .. });
            let (step, step_cells) = match say {
                true if speaking => (0, 0),
                true => (font.cell(), 1),
                false => size(&token),
            };
            if (x + step > width || used + step_cells > line_cells)
                && let Some((at, was_speaking, was_indent)) = space.take()
            {
                // What follows the space starts the next line, and is measured again from its start,
                // where a `{SAY}` in it draws its 「 and sets the indent anew.
                out.replace_range(at..at + 1, "\n");
                (speaking, indent) = (was_speaking, was_indent);
                (x, used) = start(speaking, indent);
                for moved in tokens(&out[at + 1..]) {
                    if let Token::Tag { name: "SAY" | "/SAY", .. } = moved {
                        speaking = !speaking;
                        if speaking {
                            (x, used) = (x + font.cell(), used + 1);
                            indent = (x, used);
                        }
                    } else {
                        let (step, step_cells) = size(&moved);
                        (x, used) = (x + step, used + step_cells);
                    }
                }
            }
            out.push_str(&token.text());
            (x, used) = (x + step, used + step_cells);
            if say {
                speaking = !speaking;
                if speaking {
                    indent = (x, used);
                }
            }
        }
    }
    Ok(out)
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

// --- the cells the renderer's characters take ------------------------------

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

/// The cells of the font a character drawn at its own width may take: those of the kanji that no string
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

/// Where the areas that grew past their sectors go. `grown` gives each one's first sector, the
/// sectors it had and the sectors it needs now, and `spare` the disc's free room, all in sectors
/// counted from the archive's first. Once an area moves, the sectors it had are free, and those side
/// by side make one piece. The area that needs the most goes first, each into the piece it leaves
/// the least of (`placement::pack_room`), so that the large pieces are kept for the large areas;
/// those no piece is left large enough for go to the disc's free room, one after another. So the
/// areas stay in the archive, near the rest of the game, as far as they can. Gives each area's new
/// first sector, or None where the free room too was full, and how many sectors the pieces have.
fn place_grown(grown: &[(u32, usize, usize)], spare: Range<usize>) -> (Vec<Option<u32>>, usize) {
    let areas: Vec<(usize, usize, usize, Range<usize>)> =
        grown.iter().map(|(sector, had, needs)| (*sector as usize, *had, *needs, 0..usize::MAX)).collect();
    let (pieces, mut places, _) = placement::pack_room(&areas, None);
    let mut next = spare.start;
    for (place, (.., needs)) in places.iter_mut().zip(grown) {
        if place.is_none() && next + needs <= spare.end {
            *place = Some(next);
            next += needs;
        }
    }
    let room = pieces.iter().map(|piece| piece.len()).sum();
    (places.into_iter().map(|place| place.map(|place| place as u32)).collect(), room)
}

// --- the build -------------------------------------------------------------

/// What a build did, for its summary.
#[derive(Default)]
struct Done {
    strings: usize,
    areas: usize,
    /// The areas that grew past their sectors and were moved, and the sectors of the disc's free room
    /// they used, of how many it has.
    moved: Vec<String>,
    spare: (usize, usize),
    /// The least of the heap that a moment of the areas checked leaves free, and which moment it is.
    least: Option<(usize, String)>,
    new_characters: Vec<char>,
    /// The characters drawn at their own widths, and the cells free for them, and the pairs of them
    /// written a byte each.
    renderer: Option<(usize, usize)>,
    pairs: usize,
    /// The strings of the program's text that changed, how many of them moved, and how many were
    /// written once for several.
    program_strings: usize,
    program_moved: usize,
    program_once: usize,
    /// The bytes the program's strings that moved take of the room they share, and that room.
    program_room: (usize, usize),
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
            let room = match self.program_room {
                (_, 0) => String::new(),
                (used, room) => format!(", taking {used} bytes of its {room}"),
            };
            lines.push(format!(
                "program:   {} strings changed, {} of them moved in the room they share{room}{once}",
                self.program_strings, self.program_moved
            ));
        }
        if self.labels > 0 {
            lines.push(format!("menu:      {} labels changed, each in its slot", self.labels));
        }
        if !self.moved.is_empty() {
            let (which, its, is) =
                if self.moved.len() == 1 { ("area", "its", "is") } else { ("areas", "their", "are") };
            lines.push(format!(
                "moved:     {which} {} grew past {its} sectors, and {is} now in the sectors such areas left in \
                 {ARCHIVE} or in the disc's free room ({SPARE_FILE}), of which {} sectors of {} are used",
                self.moved.join(", "),
                self.spare.0,
                self.spare.1
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
        if let Some((drawn, free)) = self.renderer {
            let pairs = match self.pairs {
                0 => String::new(),
                pairs => format!(", and {pairs} pairs of them written a byte each"),
            };
            lines.push(format!(
                "renderer:  {drawn} characters drawn at their own widths, in cells of kanji no unchanged string \
                 uses, of {free}{pairs}; its code at ${:08X}, loaded at start-up from the program's file",
                vwf::PLACEMENT.destination
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
    let writing = &Writing { renderer: if names { None } else { writing.renderer }, ..*writing };
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
    // Measured in the small font's pixels, which the part may be drawn in: a line that fits there
    // fits in the dialogue's font too, whose glyphs of the game's are wider and the others as wide.
    let pixels = width * vwf::SMALL;
    let text = if wrap { wrap_to(text, pixels, usize::MAX, writing, Face::Small)? } else { text.to_string() };
    let data = encode_text(&text, writing)?;
    let lines = line_widths(&data, writing, Face::Small);
    if let Some((line, wide)) = lines.iter().enumerate().find(|(_, wide)| **wide > pixels) {
        return text_error(match wrap {
            true => format!(
                "broken at spaces, it still has a line {wide} pixels wide, and {whose} has room for {pixels} ({width} \
                 cells of 12 pixels): shorten its longest word"
            ),
            false => format!(
                "its line {} is {wide} pixels wide, and {whose} has room for {pixels} ({width} cells of 12 pixels): \
                 break it sooner, or build with wrapping on",
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
        fit(&encode_text(&text, writing)?, writing, options.wrap)
    };
    let write_program = |text: &str, number: u32, writing: &Writing| match own_text[PROGRAM_SECTION].get(&number) {
        Some(own) => program_string(text, number, own, writing, options.wrap),
        None => text_error("the game has no such string"),
    };

    // The characters drawn at their own widths (see `vwf`): those the game's font lacks and the
    // pack's font draws narrow, in the dialogue and the program's text but the party's names, each in
    // a cell of a kanji no string left as it is uses.
    let used = plain_characters(&all_texts.concat());
    let lacking: Vec<char> = used.iter().copied().filter(|char| pack_code(*char, codes, &none).is_none()).collect();
    let mut renderer = Renderer::default();
    if !options.switches.contains(FULL_WIDTH) {
        let drawn_narrow = dialogue.iter().chain(program_changes.iter().filter(|(_, number, _)| !is_name(*number)));
        let drawn_narrow: String = drawn_narrow.map(|(_, _, text)| text.as_str()).collect();
        let narrow: Vec<char> = plain_characters(&drawn_narrow)
            .into_iter()
            .filter(|char| !matches!(char, '{' | '}') && code_of(*char, codes).is_none())
            .collect();
        let glyphs = fonts::narrow_glyphs(sources, &narrow)?;
        if !glyphs.is_empty() {
            let file = program_text::program_file(disc)?;
            let kept = Kept {
                areas: &areas,
                program: &own_text,
                battle: &battle_lines,
                dialogue: dialogue.iter().map(|(section, number, _)| (section.as_str(), *number)).collect(),
                program_strings: program_changes.iter().map(|(_, number, _)| *number).collect(),
            };
            let free = free_cells(&file, &kept, &all_texts, codes);
            renderer = Renderer::new(&glyphs, &free)?;
            done.renderer = Some((renderer.len(), free.len()));
        }
    }

    // The characters the font lacks that are not drawn at their own widths, drawn into its blank
    // cells. The program's text is drawn with the same font; the menus' labels are not.
    let new: Vec<char> = lacking.into_iter().filter(|char| renderer.code(*char).is_none()).collect();
    let mut drawn = HashMap::new();
    let mut pages: Vec<Vec<u8>> = Vec::new();
    if !new.is_empty() || !renderer.is_empty() {
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
    renderer.draw(&mut pages)?;
    for (entry, page) in FONT_MEMBERS.iter().zip(pages) {
        patches.push((ARCHIVE, u64::from(program.contents[*entry].0) * ps1::SECTOR, page));
    }
    let writing = Writing { codes, drawn: &drawn, renderer: (!renderer.is_empty()).then_some(&renderer) };

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

    // The program's own text, each string fitted to the room its part has on screen; and the menus'
    // labels, each in its slot.
    let mut changed = BTreeMap::new();
    for (section, number, text) in &program_changes {
        match write_program(text, *number, &writing) {
            Ok(data) => {
                changed.insert(*number, data);
            }
            Err(Error::Text(problem)) => refusals.push(format!("string {}: {problem}", string_name(section, *number))),
            Err(other) => return Err(other),
        }
    }
    if !labels.is_empty() {
        let file = program_text::program_file(disc)?;
        let references = program_text::references(disc)?;
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

    // Where the program's strings are, which the pairs below must know.
    let rooms = if changed.is_empty() {
        BTreeMap::new()
    } else {
        let file = program_text::program_file(disc)?;
        let references = program_text::references(disc)?;
        program_text::rooms(&file, &own_text[PROGRAM_SECTION], &references, codes)
    };

    // Two characters a byte, in the runs of every string laid out above, now that each has been
    // measured a byte a character.
    if !renderer.is_empty() {
        let pairs = put_pairs(&mut replaced, &mut changed, &rooms, &writing, &renderer);
        let file = program_text::program_file(disc)?;
        patches.extend(renderer.install(&file, &pairs)?.into_iter().map(|(at, data)| (PROGRAM, at as u64, data)));
        done.pairs = pairs.len();
    }

    // The program's strings written back into the room they share.
    if !changed.is_empty() {
        let file = program_text::program_file(disc)?;
        match program_text::write_strings(&file, &rooms, &changed) {
            Ok(written) => {
                (done.program_moved, done.program_once) = (written.moved, written.once);
                done.program_room = (written.used, written.room);
                let to_program = written.patches.into_iter().map(|(at, data)| (PROGRAM, at as u64, data));
                patches.extend(to_program);
            }
            Err(problem) => refusals.push(problem),
        }
    }

    // Each changed area rebuilt, and put where it fits: in its own sectors if it still fits there,
    // and if it grew past them, in the sectors such areas leave or in the disc's free room.
    let mut contents = program.contents.clone();
    let mut point = |old: (u32, u32), sector: u32, size: usize| {
        for (entry, named) in program.contents.iter().enumerate() {
            if *named == old {
                contents[entry] = (sector, size as u32);
            }
        }
    };
    let mut new_codes = BTreeMap::new();
    let mut grown = Vec::new();
    for (section, strings) in &replaced {
        let area = by_section[*section];
        let old = member(disc, area.entry)?;
        let mut rebuilt = match rebuild_member(&old, &text_block_with(area, strings)) {
            Ok(rebuilt) => rebuilt,
            Err(problem) => return Err(Error::Image(format!("area {section}: {problem}"))),
        };
        new_codes.insert(area.entry, member_word(&rebuilt, 0));
        let (sector, size) = program.contents[area.entry];
        let (old_sectors, size_now) = ((size as usize).div_ceil(SECTOR), rebuilt.len());
        done.areas += 1;
        if size_now.div_ceil(SECTOR) <= old_sectors {
            rebuilt.resize(old_sectors * SECTOR, 0);
            patches.push((ARCHIVE, u64::from(sector) * ps1::SECTOR, rebuilt));
            point((sector, size), sector, size_now);
        } else {
            rebuilt.resize(size_now.div_ceil(SECTOR) * SECTOR, 0);
            grown.push((*section, (sector, size), size_now, rebuilt));
        }
    }
    let spare_start = (disc.first_sector(SPARE_FILE)? - disc.first_sector(ARCHIVE)?) as usize;
    let spare = spare_start..spare_start + (disc.size(SPARE_FILE)? / ps1::SECTOR) as usize;
    let needs: Vec<(u32, usize, usize)> = grown
        .iter()
        .map(|(_, (sector, size), _, data)| (*sector, (*size as usize).div_ceil(SECTOR), data.len() / SECTOR))
        .collect();
    let (places, room) = place_grown(&needs, spare.clone());
    let mut homeless = Vec::new();
    let mut spare_used = 0;
    for ((section, old, size_now, data), place) in grown.into_iter().zip(places) {
        let Some(place) = place else {
            homeless.push(format!("{section} ({} sectors)", data.len() / SECTOR));
            continue;
        };
        if spare.contains(&(place as usize)) {
            spare_used += data.len() / SECTOR;
            patches.push((SPARE_FILE, (place as usize - spare.start) as u64 * ps1::SECTOR, data));
        } else {
            patches.push((ARCHIVE, u64::from(place) * ps1::SECTOR, data));
        }
        point(old, place, size_now);
        done.moved.push(section.to_string());
    }
    done.spare = (spare_used, spare.len());
    // Room on the heap for every moment of the areas that changed, and of every area where the
    // renderer is put in, since its grids take more of the heap while windows are open (see `vwf`).
    // The game is made to read a new area's rest only once it has left the old one, which leaves its
    // doors much more room, and the check counts on it (see `heap`). A pack that changes neither
    // leaves the heap as the game has it: nothing is checked or changed, and the areas are not read.
    let changed: BTreeSet<usize> = new_codes.keys().copied().collect();
    let (shortfalls, least) = if changed.is_empty() && renderer.is_empty() {
        (Vec::new(), None)
    } else {
        let (at, data) = heap::door_patch(&program_text::program_file(disc)?)?;
        patches.push((PROGRAM, at as u64, data));
        let mut footprints = (*footprints(disc)?).clone();
        for (entry, code) in &new_codes {
            footprints.get_mut(entry).expect("every area has a footprint").code = *code;
        }
        let (checked, reserve) =
            if renderer.is_empty() { (changed.clone(), 0) } else { (footprints.keys().copied().collect(), vwf::GRIDS) };
        heap::check(&footprints, &checked, reserve)
    };
    done.least = least;
    refusals.extend(heap_refusals(shortfalls, &changed));
    if !homeless.is_empty() {
        let wanted: usize = needs.iter().map(|(.., sectors)| sectors).sum();
        refusals.push(format!(
            "the areas that grew past their sectors need {wanted} sectors, and the room for them has {}: the {room} \
             sectors they leave in {ARCHIVE} and the {} of the disc's free room ({SPARE_FILE}); none of it was left \
             for area {}: shorten some of the changed lines",
            room + spare.len(),
            spare.len(),
            homeless.join(", ")
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

/// What a build says of each moment on the heap it would leave short, and how to fix it. `changed` is
/// the areas whose code the pack changes. A door between a changed area and one that is not is said
/// once, from the changed side, where the other side's moment is the changed area's own too
/// (`Shortfall::mutual`); a door assumed into an unchanged area is said from that area, since no
/// moment of the changed one counts it.
fn heap_refusals(shortfalls: Vec<Shortfall>, changed: &BTreeSet<usize>) -> Vec<String> {
    let mut refusals = Vec::new();
    for Shortfall { area, other, mutual, moment, over } in shortfalls {
        let fix = match other {
            Some(other) if changed.contains(&other) && !changed.contains(&area) => {
                if mutual {
                    continue;
                }
                format!("shorten some of the changed lines of area {other:03}")
            }
            Some(other) if changed.contains(&other) => {
                format!("shorten some of its changed lines (or those of area {other:03})")
            }
            _ if changed.contains(&area) => "shorten some of its changed lines".to_string(),
            _ => format!(
                "the renderer's larger grids take {} bytes of it: build with the option {FULL_WIDTH}",
                vwf::GRIDS
            ),
        };
        refusals.push(format!(
            "area {area:03}: {moment}, the game would need {over} bytes more of its heap than it has room for: {fix}"
        ));
    }
    refusals
}

#[cfg(test)]
mod tests;
