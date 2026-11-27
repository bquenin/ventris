//! A renderer that draws a translation's letters each at its own width, put into the game by a
//! build (`vwf.asm`).
//!
//! How the game draws text. A window's text is a grid of 16-bit codes in a block of the heap, a code
//! to a cell, as many cells to a line as the window is wide. Two writers fill it: $80024CE8 a
//! character a frame, $80024F70 a whole string at once. Each starts a new line when the pen reaches
//! the window's width, in cells, and $80024858 starts it, scrolling the lines up when the window is
//! full. Every frame a draw routine goes through the grid and draws each code as a textured quad
//! (a `POLY_FT4`) from the font's pictures in video memory, 14 by 15 pixels, moving the pen 14 pixels
//! a cell whether the cell holds a code or not ($80025BFC; $80025E30 while the window opens). The
//! battle's window draws its codes from the small font, 12 by 11 pixels, as sprites, 12 pixels a cell
//! ($800261A4). The four windows the game opens are records of $28 bytes at $80110830. The menus'
//! screens draw a string straight from its bytes in the small font, 12 pixels a character
//! ($80058828). A glyph's pixels where its font has no ink take the colour table's first entry,
//! which is transparent, so glyphs may overlap without hiding one another.
//!
//! What the renderer changes. Each code has a width, in a table of the renderer's (`widths` in
//! `vwf.asm`): the draw routines move the pen by the code's width, a line of a window's grid has four
//! times as many cells as the window is wide, and the writers end a line when the pen, the widths of
//! the line's cells added up, reaches the window's width in pixels (14 a cell, 12 in the small font)
//! or the line's cells run out. The game's own glyphs, and an empty cell, keep their width, so the
//! game's own text is drawn exactly as before. Each character a pack writes in Latin letters, digits,
//! spaces and punctuation, whatever its font draws in an 8-pixel cell, is drawn once into a cell of
//! both fonts, left-aligned, and given its own width: Unifont's ink and one pixel more
//! (`fonts::proportional`), 4 pixels for a space. The cells are those of kanji that no string the pack
//! leaves as it is draws (see `insert::free_cells`): about a hundred are needed, and the game's text
//! alone leaves 275 kanji unused. A pack that needs more cells than there are is refused.
//!
//! A byte a character. The renderer's characters would take two bytes each as codes of the font, and
//! an area's text can grow little or not at all (see `heap`). So they are written a byte each, in
//! runs: the byte `RUN` ($0A, the high half of codes past the font's, which the game's own text never
//! writes) starts one, and in it each byte from $10 up is one of the renderer's characters, by its own
//! table (`run_codes` in `vwf.asm`, 240 characters at most). `RUN` again, or any byte under $10, a
//! line break, a wait, a tag, ends it. The writers keep, for each window, whether it is in a run, and
//! a window given a new string ($8002492C) is not; the menus' screens keep one for the string they
//! draw. A letter takes a byte, and a run one more.
//!
//! Two characters a byte. A run's bytes the renderer's characters leave over, from the one after
//! the last character's up to $FF, each stand for two characters that often follow one another
//! (`Pairs`), chosen at each build from the runs of the text it writes (`byte_pairs::choose`), so
//! the same pack always gets the same pairs. A reader that comes to such a byte takes its first
//! character, notes that the second is to come, and reads the same byte again as the next
//! character, taking the second (`run_code` in `vwf.asm`). So the two characters are drawn, and a
//! line ends between them, exactly as if each had a byte of its own, and a build lays out and
//! measures every string a byte a character before it puts the pairs in (see `insert`): only the
//! number of bytes changes.
//!
//! Where the code is. The renderer's code and its tables, under 6 KB, go at $801F8000, the bottom
//! of the 32 KB the game leaves for its stack above its heap (the heap ends at $801F8000, the stack
//! starts at $801FFFF0). Measured in PCSX-Redux from power-on through the opening movie and the
//! first dialogue, nothing the game runs wrote to $801F8000-$801FDFFF; only the BIOS's start-up,
//! before the game is loaded, uses it. The game's file has no free room, and cannot grow, so the code
//! is kept until then in the zero bytes at $8008EFC0-$80091000, variables of the game's that start
//! at zero, and loaded by `ps1::exe::install`: main's call of the heap's set-up ($800129C8) goes to
//! a loader there first, which copies the code to $801F8000 and sets the block back to zero.
//!
//! The grids of the four windows are four times as large, on the heap: the largest the game opens
//! is 18 cells by 4, so the grids take at most `GRIDS` bytes more than the game's own while four
//! windows are open, which the build counts at every moment of an area it changes (see `heap`).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::insert::{Cell, draw_cell, eleven_rows, large_cell, small_cell};
use crate::byte_pairs::{self, Unit};
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, CELL, Glyph};
use crate::platforms::asm_mips::assemble;
use crate::platforms::ps1::exe::{self, Placement};

/// Where the renderer's code is kept and goes (see the module's comment).
pub const PLACEMENT: Placement =
    Placement { hook: 0x8001_29C8, room: 0x8008_EFC0..0x8009_1000, destination: 0x801F_8000 };
/// The width of a glyph of the game's own, in the dialogue's font and in the small one.
pub const LARGE: usize = 14;
pub const SMALL: usize = 12;
/// How many times the window's width a line of its grid has.
pub const CELLS_PER_COLUMN: usize = 4;
/// The most bytes the four windows' grids take more than the game's own: four windows of 18 cells
/// by 4, the largest the game opens, each cell two bytes, three times more.
pub const GRIDS: usize = 4 * 18 * 4 * 2 * (CELLS_PER_COLUMN - 1);
/// The codes the table of widths has: every code a string can write, two bytes up to $0AFF.
const TABLE_CODES: usize = 0xB00;
/// The table's entry for a code of the game's own: its width in the small font in the high four
/// bits, in the dialogue's in the low.
const OWN: u8 = (SMALL << 4 | LARGE) as u8;
/// The byte that starts or ends a run of the renderer's characters, a byte each (see `vwf.asm`),
/// and the first byte a character of a run is written with: the bytes under it end a run.
pub use super::RUN;
const FIRST_BYTE: u8 = 0x10;
/// The bytes of `run_codes` in `vwf.asm` for each byte of a run: two codes of two bytes.
const RUN_ENTRY: usize = 4;

/// The characters a build draws at their own widths: each one's code, its cell in both fonts, and
/// its width in pixels, the same in both.
#[derive(Default)]
pub struct Renderer {
    cells: BTreeMap<char, (u32, usize)>,
    widths: HashMap<u32, usize>,
    /// The byte each is written with in a run, and the code of each such byte.
    bytes: HashMap<char, u8>,
    byte_codes: HashMap<u8, u32>,
    /// The glyph of each, as it is drawn: its rows in the dialogue's font and in the small one.
    rows: BTreeMap<char, ([u16; CELL], [u16; 11])>,
}

impl Renderer {
    /// Give each of `glyphs`, designed 8 pixels wide, one of the `free` cells and a byte. Refused
    /// when there are too few cells, or more characters than bytes.
    pub fn new(glyphs: &BTreeMap<char, Glyph>, free: &[u32]) -> Result<Renderer> {
        let bytes = usize::from(u8::MAX - FIRST_BYTE) + 1;
        if glyphs.len() > bytes {
            return text_error(format!(
                "the pack writes {} characters that are drawn at their own widths, and a run of them has a byte \
                 for {bytes}: use fewer characters",
                glyphs.len()
            ));
        }
        if glyphs.len() > free.len() {
            return text_error(format!(
                "the pack writes {} characters that are drawn at their own widths, each in a cell of the font of \
                 its own, and the font has {} cells free, those of the kanji no string left as it is uses: \
                 translate more of the game's strings, or use fewer characters",
                glyphs.len(),
                free.len()
            ));
        }
        let mut renderer = Renderer::default();
        for (((char, glyph), code), byte) in glyphs.iter().zip(free).zip(FIRST_BYTE..=u8::MAX) {
            let (rows, width) = fonts::proportional(glyph);
            assert!(width <= 15, "a narrow glyph is at most 9 pixels wide");
            renderer.cells.insert(*char, (*code, width));
            renderer.widths.insert(*code, width);
            renderer.bytes.insert(*char, byte);
            renderer.byte_codes.insert(byte, *code);
            renderer.rows.insert(*char, (one_row_up(&rows), eleven_rows(&rows)));
        }
        Ok(renderer)
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// The code a character is drawn with, if it is one of the renderer's.
    pub fn code(&self, char: char) -> Option<u32> {
        self.cells.get(&char).map(|(code, _)| *code)
    }

    /// The byte a character of the renderer's is written with in a run.
    pub fn byte(&self, char: char) -> Option<u8> {
        self.bytes.get(&char).copied()
    }

    /// The code of a byte in a run.
    pub fn byte_code(&self, byte: u8) -> Option<u32> {
        self.byte_codes.get(&byte).copied()
    }

    /// The width of a character of the renderer's, in pixels.
    pub fn width(&self, char: char) -> Option<usize> {
        self.cells.get(&char).map(|(_, width)| *width)
    }

    /// The width of a code of the renderer's, in pixels.
    pub fn code_width(&self, code: u32) -> Option<usize> {
        self.widths.get(&code).copied()
    }

    /// Draw every character into its cells of both fonts, over the kanji there. `pages` are the
    /// font's three pictures (see `insert::FONT_MEMBERS`).
    pub fn draw(&self, pages: &mut [Vec<u8>]) -> Result<()> {
        for (char, (code, _)) in &self.cells {
            let (large, small) = &self.rows[char];
            draw_cell(pages, large_cell(*code), large, true, *code)?;
            let Some(cell): Option<Cell> = small_cell(*code) else {
                return Err(Error::Image(format!("code {code:04X} has no glyph in the small font")));
            };
            draw_cell(pages, cell, small, true, *code)?;
        }
        Ok(())
    }

    /// The bytes of a run that its characters leave over, for pairs.
    fn spare_bytes(&self) -> Vec<u8> {
        (usize::from(FIRST_BYTE) + self.len()..=usize::from(u8::MAX)).map(|byte| byte as u8).collect()
    }

    /// The table of widths, by code (see `vwf.asm`).
    fn table(&self) -> Vec<u8> {
        let mut table = vec![OWN; TABLE_CODES];
        for (code, width) in &self.widths {
            table[*code as usize] = (width << 4 | width) as u8;
        }
        table
    }

    /// The changes to the program's file that put the renderer in, with `pairs`: its code and
    /// tables, kept in the file and loaded at start-up, and the hooks in the game's code, as offsets
    /// in the file.
    pub fn install(&self, exe: &[u8], pairs: &Pairs) -> Result<Vec<(usize, Vec<u8>)>> {
        let code = PLACEMENT.code_address();
        let assembly = assemble(include_str!("vwf.asm"), &[("CODE", i64::from(code))])?;
        let widths = (assembly.label("widths")? - code) as usize;
        let run_codes = (assembly.label("run_codes")? - code) as usize;
        let mut patches = Vec::new();
        for (address, bytes) in &assembly.segments {
            if *address == code {
                let mut bytes = bytes.clone();
                bytes[widths..widths + TABLE_CODES].copy_from_slice(&self.table());
                let singles = self.byte_codes.iter().map(|(byte, code)| (*byte, *code, 0));
                let doubles = pairs
                    .bytes
                    .iter()
                    .map(|((first, second), byte)| (*byte, self.byte_codes[first], self.byte_codes[second]));
                for (byte, first, second) in singles.chain(doubles) {
                    let at = run_codes + RUN_ENTRY * usize::from(byte);
                    bytes[at..at + 2].copy_from_slice(&(first as u16).to_le_bytes());
                    bytes[at + 2..at + 4].copy_from_slice(&(second as u16).to_le_bytes());
                }
                patches.extend(exe::install(exe, &PLACEMENT, &bytes)?);
            } else {
                patches.extend(exe::code_patches(exe, &[(*address, bytes.clone())])?);
            }
        }
        Ok(patches)
    }
}

/// The pairs of the renderer's characters written a byte each in a run (see the module's comment).
#[derive(Default)]
pub struct Pairs {
    /// The byte of each pair, by the bytes its two characters have alone.
    bytes: BTreeMap<(u8, u8), u8>,
    set: BTreeSet<(u8, u8)>,
}

impl Pairs {
    /// The pairs that save the most bytes in `runs`, each run given as the bytes its characters have
    /// alone, as many as the renderer leaves bytes for.
    pub fn choose(renderer: &Renderer, runs: &[Vec<u8>]) -> Pairs {
        let spare = renderer.spare_bytes();
        let chosen = byte_pairs::choose(runs, spare.len());
        let bytes: BTreeMap<(u8, u8), u8> = chosen.into_iter().zip(spare).collect();
        let set = bytes.keys().copied().collect();
        Pairs { bytes, set }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// A run, given as the bytes its characters have alone, written with as few bytes as the pairs
    /// allow.
    pub fn write(&self, run: &[u8]) -> Vec<u8> {
        let units = byte_pairs::write(run, &self.set);
        let byte = |unit: Unit<u8>| match unit {
            Unit::One(byte) => byte,
            Unit::Two(first, second) => self.bytes[&(first, second)],
        };
        units.into_iter().map(byte).collect()
    }

    /// The two bytes a byte of a run stands for alone, if it is a pair's.
    pub fn split(&self, byte: u8) -> Option<(u8, u8)> {
        self.bytes.iter().find(|(_, pair)| **pair == byte).map(|(characters, _)| *characters)
    }
}

/// A glyph's rows one row up, where it can: the game leaves a cell's bottom row out, and Unifont's
/// two bottom rows hold descenders. The first empty row goes, which is the top one for most letters
/// and, for a capital with an accent in the top row (É), the one between the accent and the letter,
/// so that every letter stands on the same line.
fn one_row_up(rows: &[u16; CELL]) -> [u16; CELL] {
    let Some(empty) = rows.iter().position(|row| *row == 0) else { return *rows };
    std::array::from_fn(|y| if y < empty { rows[y] } else { rows.get(y + 1).copied().unwrap_or(0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(rows: [u16; CELL]) -> Glyph {
        Glyph { rows, width: 8 }
    }

    #[test]
    fn each_character_takes_a_cell_and_its_own_width() {
        // An "i" of one column of ink at column 3, and a space.
        let mut i = [0u16; CELL];
        (4..14).for_each(|y| i[y] = 0x1000);
        let glyphs = BTreeMap::from([('i', glyph(i)), (' ', glyph([0; CELL]))]);
        let renderer = Renderer::new(&glyphs, &[0x2F0, 0x2F1, 0x2F2]).unwrap();
        assert_eq!(renderer.len(), 2);
        assert_eq!((renderer.code(' '), renderer.width(' ')), (Some(0x2F0), Some(fonts::SPACE_ADVANCE)));
        assert_eq!((renderer.byte(' '), renderer.byte('i')), (Some(0x10), Some(0x11)));
        assert_eq!((renderer.byte_code(0x11), renderer.byte_code(0x12)), (Some(0x2F1), None));
        assert_eq!((renderer.code('i'), renderer.width('i')), (Some(0x2F1), Some(2)));
        assert_eq!(renderer.code_width(0x2F1), Some(2));
        assert_eq!(renderer.code('x'), None);
        // The ink starts a column in, and a row up.
        assert_eq!(renderer.rows[&'i'].0[3..13], [0x4000; 10]);
        let table = renderer.table();
        assert_eq!((table[0], table[0x2F1], table[0x2F0], table[0xAFF]), (0xCE, 0x22, 0x44, 0xCE));
    }

    #[test]
    fn pairs_take_the_bytes_the_characters_leave_and_write_a_run_in_fewer() {
        let glyphs = BTreeMap::from([('a', glyph([0; CELL])), ('b', glyph([0; CELL])), ('c', glyph([0; CELL]))]);
        let renderer = Renderer::new(&glyphs, &[0x2F0, 0x2F1, 0x2F2]).unwrap();
        let (a, b, c) = (0x10, 0x11, 0x12);
        let pairs = Pairs::choose(&renderer, &[vec![a, b, a, b, c], vec![c, a]]);
        // "ab" twice, then "ca"; "bc" lost its b to "ab".
        assert_eq!(pairs.len(), 2);
        assert_eq!((pairs.split(0x13), pairs.split(0x14), pairs.split(0x15)), (Some((a, b)), Some((c, a)), None));
        assert_eq!(pairs.write(&[a, b, a, b, c]), [0x13, 0x13, c]);
        assert_eq!(pairs.write(&[b, c, a]), [b, 0x14]);
        // A renderer with as many characters as a run has bytes leaves none for pairs.
        let all: BTreeMap<char, Glyph> = ('\u{100}'..).take(240).map(|char| (char, glyph([0; CELL]))).collect();
        let full = Renderer::new(&all, &(0x100..0x1F0).collect::<Vec<u32>>()).unwrap();
        assert!(Pairs::choose(&full, &[vec![a, b, a, b]]).is_empty());
    }

    #[test]
    fn a_pack_needing_more_cells_than_are_free_is_refused_with_both_counts() {
        let glyphs = BTreeMap::from([('a', glyph([0; CELL])), ('b', glyph([0; CELL]))]);
        let refused = Renderer::new(&glyphs, &[0x2F0]).err().unwrap().to_string();
        assert!(refused.contains("writes 2 characters"), "{refused}");
        assert!(refused.contains("has 1 cells free"), "{refused}");
    }

    #[test]
    fn the_renderer_assembles_into_its_room_and_hooks_the_game_s_routines() {
        // A program as large as the game's, with the call the loader takes the place of.
        let mut exe = vec![0; 0x800 + 0xD3000];
        exe[..8].copy_from_slice(b"PS-X EXE");
        exe[0x18..0x1C].copy_from_slice(&0x8001_0000u32.to_le_bytes());
        exe[0x1C..0x20].copy_from_slice(&0xD3000u32.to_le_bytes());
        let hook = exe::offset(&exe, PLACEMENT.hook, 4).unwrap();
        exe[hook..hook + 4].copy_from_slice(&0x0C00_AC13u32.to_le_bytes()); // jal $8002B04C
        let glyphs = BTreeMap::from([('a', glyph([0; CELL])), ('b', glyph([0; CELL]))]);
        let renderer = Renderer::new(&glyphs, &[0x2F0, 0x2F1]).unwrap();
        let pairs = Pairs::choose(&renderer, &[vec![0x10, 0x11, 0x11, 0x10]]);
        let patches = renderer.install(&exe, &pairs).unwrap();
        let room = exe::offset(&exe, PLACEMENT.room.start, 4).unwrap();
        let (_, kept) = patches.iter().find(|(at, _)| *at == room).unwrap();
        assert!(kept.len() as u32 <= PLACEMENT.room.end - PLACEMENT.room.start);
        // Each byte of a run stands for a character's code and 0, or for a pair's two codes.
        let code = PLACEMENT.code_address();
        let assembly = assemble(include_str!("vwf.asm"), &[("CODE", i64::from(code))]).unwrap();
        let run_codes = (assembly.label("run_codes").unwrap() - code) as usize;
        let at = |byte: usize| (exe::LOADER_SIZE + exe::FINISH_SIZE) as usize + run_codes + RUN_ENTRY * byte;
        let entry = |byte: usize| kept[at(byte)..at(byte) + 4].to_vec();
        assert_eq!(entry(0x10), [0xF0, 0x02, 0, 0]);
        assert_eq!(entry(0x11), [0xF1, 0x02, 0, 0]);
        let paired = pairs.write(&[0x10, 0x11]);
        assert_eq!(paired.len(), 1);
        assert_eq!(entry(usize::from(paired[0])), [0xF0, 0x02, 0xF1, 0x02]);
        assert_eq!(entry(0x0F), [0; 4]);
        // The draw routine's move of the pen by 14 pixels is now a call.
        let draw = exe::offset(&exe, 0x8002_5DD0, 4).unwrap();
        let (_, call) = patches.iter().find(|(at, _)| *at == draw).unwrap();
        assert_eq!(u32::from_le_bytes(call[..4].try_into().unwrap()) >> 26, 3);
    }
}
