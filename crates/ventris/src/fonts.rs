//! Bitmap glyphs for the characters a pack uses that the game's own font lacks.
//!
//! The main source is GNU Unifont's `.hex` format: one line per character, `CODEPOINT:BITMAP`, where
//! the bitmap is 16 rows of either 8 pixels (alphabetic scripts) or 16 pixels (CJK), in hexadecimal.
//! A `.hex` file may be compressed with gzip. Fonts in BDF, the plain-text format bitmap fonts such
//! as Terminus are published in, are read too, for the games that draw glyphs of other sizes.
//!
//! A font is a file, or one of the few built into the program (`BUILT_IN`). Each game's adapter
//! names the built-in fonts that were tried in that game and look right, so a pack need bring none.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;

use crate::error::{Error, Result, at, text_error};

/// A Unifont glyph is designed in a square this many pixels wide, or in its left half.
pub const CELL: usize = 16;
/// How far the pen moves for a glyph with no ink: a space.
pub const SPACE_ADVANCE: usize = 4;
/// Rows of a Unifont glyph that are above the baseline.
pub const UNIFONT_ASCENT: i64 = 14;

/// A glyph from a Unifont `.hex` file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// 16 rows; each a 16-bit value whose top bit is the leftmost pixel.
    pub rows: [u16; CELL],
    /// Width of the cell the glyph was designed in: 8 or 16.
    pub width: usize,
}

fn font_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Font(message.into()))
}

/// A font built into the program, with the terms it is passed on under.
pub struct BuiltIn {
    /// What a pack, a user or an adapter calls it: "unifont".
    pub name: &'static str,
    /// What it is, in a line.
    pub about: &'static str,
    /// Its terms in a word or two, as an SPDX name where there is one.
    pub license: &'static str,
    /// Its terms in full, with the notices they ask to be kept with every copy.
    pub terms: &'static str,
    /// Whether the file is in BDF; otherwise it is a `.hex` file.
    bdf: bool,
    /// The font file, compressed with gzip.
    data: &'static [u8],
}

/// The fonts built into the program. Each is here because its terms let it be passed on inside
/// another program, and because it has been looked at in a game.
pub static BUILT_IN: [BuiltIn; 3] = [
    BuiltIn {
        name: "unifont",
        about: "GNU Unifont 18.0.01, 16 pixels tall, with a glyph for nearly every character of Unicode",
        license: "OFL-1.1",
        terms: include_str!("../fonts/unifont-LICENSE.txt"),
        bdf: false,
        data: include_bytes!("../fonts/unifont_all-18.0.01.hex.gz"),
    },
    BuiltIn {
        name: "dejavu-condensed-bold",
        about: "DejaVu Sans Condensed Bold 2.37, drawn large for a cell of 12 by 16 pixels: Latin, Greek, Cyrillic",
        license: "Bitstream Vera Fonts licence, with the DejaVu changes in the public domain",
        terms: include_str!("../fonts/dejavu-LICENSE.txt"),
        bdf: true,
        data: include_bytes!("../fonts/dejavu-condensed-bold.bdf.gz"),
    },
    BuiltIn {
        name: "unscii-8",
        about: "Unscii-8, 8 by 8 pixels, for text drawn in small tiles",
        license: "public domain",
        terms: include_str!("../fonts/unscii-8-LICENSE.txt"),
        bdf: false,
        data: include_bytes!("../fonts/unscii-8.hex.gz"),
    },
];

/// The built-in font of a name.
pub fn built_in(name: &str) -> Option<&'static BuiltIn> {
    BUILT_IN.iter().find(|font| font.name == name)
}

impl fmt::Debug for BuiltIn {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "BuiltIn({})", self.name)
    }
}

/// Where glyphs are read from: a font file, or a font built into the program.
#[derive(Clone, Debug)]
pub enum Font {
    File(PathBuf),
    BuiltIn(&'static BuiltIn),
}

impl Font {
    /// The font a user means by what they typed: the built-in font of that name, or else the file
    /// at that path.
    pub fn named(text: &str) -> Font {
        built_in(text).map_or_else(|| Font::File(PathBuf::from(text)), Font::BuiltIn)
    }

    /// The built-in fonts of these names, in this order: how an adapter's own list is turned into
    /// fonts. A name that is no built-in font's is left out; `declaration_faults` reports it.
    pub fn built_ins(names: &[&str]) -> Vec<Font> {
        names.iter().filter_map(|name| built_in(name)).map(Font::BuiltIn).collect()
    }

    /// What to call the font in a message: a built-in font's name, a file's name without its folder.
    pub fn name(&self) -> String {
        match self {
            Font::File(path) => path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            Font::BuiltIn(font) => font.name.to_string(),
        }
    }

    fn is_bdf(&self) -> bool {
        match self {
            Font::File(path) => path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("bdf")),
            Font::BuiltIn(font) => font.bdf,
        }
    }
}

impl PartialEq for Font {
    fn eq(&self, other: &Font) -> bool {
        match (self, other) {
            (Font::File(one), Font::File(two)) => one == two,
            (Font::BuiltIn(one), Font::BuiltIn(two)) => one.name == two.name,
            _ => false,
        }
    }
}

impl From<PathBuf> for Font {
    fn from(path: PathBuf) -> Font {
        Font::File(path)
    }
}

impl From<&PathBuf> for Font {
    fn from(path: &PathBuf) -> Font {
        Font::File(path.clone())
    }
}

impl From<&Path> for Font {
    fn from(path: &Path) -> Font {
        Font::File(path.to_path_buf())
    }
}

/// The lines of a font file, read as they are needed, so that a search can stop at what it wants.
///
/// A file whose name ends in `.gz` is decompressed, and so is a built-in font, which is kept
/// compressed. A line ends at a line feed, a carriage return or both, and is given without them
/// and without the spaces around it.
struct Lines {
    source: Box<dyn BufRead>,
    waiting: VecDeque<Vec<u8>>,
}

impl Lines {
    fn open(font: &Font) -> Result<Lines> {
        let source: Box<dyn BufRead> = match font {
            Font::BuiltIn(font) => Box::new(BufReader::new(MultiGzDecoder::new(font.data))),
            Font::File(path) => {
                let file = File::open(path).map_err(at(path))?;
                if path.extension().is_some_and(|extension| extension == "gz") {
                    Box::new(BufReader::new(MultiGzDecoder::new(file)))
                } else {
                    Box::new(BufReader::new(file))
                }
            }
        };
        Ok(Lines { source, waiting: VecDeque::new() })
    }
}

impl Iterator for Lines {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(line) = self.waiting.pop_front() {
                return Some(Ok(line));
            }
            let mut chunk = Vec::new();
            match self.source.read_until(b'\n', &mut chunk) {
                Ok(0) => return None,
                Ok(_) => {}
                Err(error) => return Some(Err(error.into())),
            }
            if chunk.ends_with(b"\n") {
                chunk.pop();
            }
            if chunk.ends_with(b"\r") {
                chunk.pop();
            }
            self.waiting.extend(chunk.split(|byte| *byte == b'\r').map(|line| line.trim_ascii().to_vec()));
        }
    }
}

/// One line of a `.hex` font: the code point it is for, as written, and its bitmap in hexadecimal.
/// None for a line that holds no glyph.
fn hex_line(line: &[u8]) -> Result<Option<(&str, &str)>> {
    let Some(line) = std::str::from_utf8(line).ok().filter(|line| line.is_ascii()) else {
        return font_error("the font is not a .hex file: it holds more than plain ASCII text");
    };
    Ok(line.split_once(':').filter(|(_, data)| !data.is_empty()))
}

/// The code point a line of a `.hex` font is for.
fn code_point(key: &str) -> Result<u32> {
    match u32::from_str_radix(key.trim(), 16) {
        Ok(code) => Ok(code),
        Err(_) => font_error(format!("{key:?} is not a code point in hexadecimal")),
    }
}

/// The characters of a list as a font file names them: by code point.
fn by_code(characters: &[char]) -> BTreeMap<u32, char> {
    characters.iter().map(|char| (*char as u32, *char)).collect()
}

/// The first few of the characters a font lacks, each with its code point, for a message.
fn shown_with_codes(characters: impl Iterator<Item = char>) -> String {
    characters.take(8).map(|char| format!("{char} (U+{:04X})", char as u32)).collect::<Vec<_>>().join(" ")
}

/// Read the glyphs for `characters`. Fails with `Error::Font` when the font lacks one, unless told not to.
pub fn load_unifont(font: &Font, characters: &[char], allow_missing: bool) -> Result<BTreeMap<char, Glyph>> {
    let wanted = by_code(characters);
    let mut found = BTreeMap::new();
    let lines = Lines::open(font)?;
    if !wanted.is_empty() {
        for line in lines {
            let line = line?;
            let Some((key, data)) = hex_line(&line)? else { continue };
            let Some(char) = wanted.get(&code_point(key)?) else { continue };
            let width = data.len() * 4 / CELL;
            if !(width == 8 || width == 16) || data.len() != width * CELL / 4 {
                return font_error(format!("unexpected glyph size for U+{key}"));
            }
            let digits = width / 4;
            let mut rows = [0u16; CELL];
            for (index, row) in rows.iter_mut().enumerate() {
                let Ok(bits) = u16::from_str_radix(&data[index * digits..(index + 1) * digits], 16) else {
                    return font_error(format!("the glyph for U+{key} is not written in hexadecimal"));
                };
                *row = bits << (CELL - width);
            }
            found.insert(*char, Glyph { rows, width });
            if found.len() == wanted.len() {
                break;
            }
        }
    }
    let missing: BTreeSet<char> = wanted.values().copied().filter(|char| !found.contains_key(char)).collect();
    if !missing.is_empty() && !allow_missing {
        let shown = shown_with_codes(missing.iter().copied());
        return font_error(format!("the font has no glyph for {} characters: {shown}", missing.len()));
    }
    Ok(found)
}

/// Read 8x8 glyphs for `characters` from a `.hex` font such as Unscii-8.
///
/// Each glyph is 8 rows of 8 pixels, the leftmost in the top bit. Fails with `Error::Font` when one
/// is missing.
pub fn load_tiles(font: &Font, characters: &[char]) -> Result<BTreeMap<char, [u8; 8]>> {
    let wanted = by_code(characters);
    let mut found = BTreeMap::new();
    for line in Lines::open(font)? {
        let line = line?;
        // A glyph of any other size, in the same file, is not an 8x8 tile.
        let Some((key, data)) = hex_line(&line)?.filter(|(_, data)| data.len() == 16) else { continue };
        let Some(char) = wanted.get(&code_point(key)?) else { continue };
        let mut rows = [0u8; 8];
        for (index, row) in rows.iter_mut().enumerate() {
            let Ok(bits) = u8::from_str_radix(&data[index * 2..index * 2 + 2], 16) else {
                return font_error(format!("the glyph for U+{key} is not written in hexadecimal"));
            };
            *row = bits;
        }
        found.insert(*char, rows);
    }
    let missing: BTreeSet<char> = wanted.values().copied().filter(|char| !found.contains_key(char)).collect();
    if !missing.is_empty() {
        let shown = shown_with_codes(missing.iter().copied());
        return font_error(format!("the menu font has no 8x8 glyph for {} characters: {shown}", missing.len()));
    }
    Ok(found)
}

/// Turn a one-bit 8x8 glyph into a two-bitplane SNES tile in three tones.
///
/// The glyph's upper `light_rows` rows take colour 1 and the rest colour 2 (three light rows is the
/// usual choice); empty pixels directly right of or below a glyph pixel take colour 3, a shadow.
/// Shadow that would fall outside the tile is dropped.
pub fn shaded_tile(rows: &[u8], light_rows: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(rows.len() * 2);
    for (y, row) in rows.iter().copied().enumerate() {
        let above = if y > 0 { rows[y - 1] } else { 0 };
        let shadow = ((row >> 1) | above) & !row;
        let (body_low, body_high) = if y < light_rows { (row, 0) } else { (0, row) };
        out.push(body_low | shadow); // first bitplane: colours 1 and 3
        out.push(body_high | shadow); // second bitplane: colours 2 and 3
    }
    out
}

/// Lay a glyph out for variable-width text: its rows, and how far the pen moves after it.
///
/// A glyph from an 8-pixel cell is trimmed to its ink and placed one pixel in, which leaves room for
/// an outline on its left; the pen then advances by the ink's width plus one. A glyph from a
/// 16-pixel cell is kept as designed and advances a full cell.
pub fn proportional(glyph: &Glyph) -> ([u16; CELL], usize) {
    if glyph.width == CELL {
        return (glyph.rows, CELL);
    }
    let ink = glyph.rows.iter().fold(0u16, |ink, row| ink | row);
    if ink == 0 {
        return (glyph.rows, SPACE_ADVANCE);
    }
    let left = ink.leading_zeros(); // blank columns before the ink
    let right = ink.trailing_zeros(); // blank columns after it
    let width = CELL - left as usize - right as usize;
    (glyph.rows.map(|row| (row << left) >> 1), width + 1)
}

/// For each row, the pixels directly left, right, above or below a glyph pixel, clipped to the cell.
pub fn outline(rows: &[u16]) -> Vec<u16> {
    (0..rows.len())
        .map(|index| {
            let row = rows[index];
            let above = if index > 0 { rows[index - 1] } else { 0 };
            let below = rows.get(index + 1).copied().unwrap_or(0);
            (row << 1) | (row >> 1) | above | below
        })
        .collect()
}

/// What to say when a pack needs characters drawn and brings no font.
fn no_font(characters: &[char]) -> String {
    let shown = characters.iter().take(12).map(char::to_string).collect::<Vec<_>>().join(" ");
    format!(
        "the pack uses {} characters the game's font lacks ({shown}); a font is needed to draw them",
        characters.len()
    )
}

/// Glyphs from a pack's fonts for the characters the game's font lacks.
///
/// `sources` are fonts in Unifont's `.hex` format: a character is taken from the first that has it. Fails with
/// `Error::Text` when characters are needed and there is no font, or none of them has a character.
pub fn new_glyphs(sources: &[Font], characters: &[char]) -> Result<BTreeMap<char, Glyph>> {
    let mut found = BTreeMap::new();
    if characters.is_empty() {
        return Ok(found);
    }
    if sources.is_empty() {
        return text_error(no_font(characters));
    }
    for (index, source) in sources.iter().enumerate() {
        let last = index + 1 == sources.len(); // the last font is the one that reports what is still missing
        let missing: Vec<char> = characters.iter().copied().filter(|char| !found.contains_key(char)).collect();
        match load_unifont(source, &missing, !last) {
            Ok(glyphs) => found.extend(glyphs),
            Err(Error::Font(message)) => return text_error(message),
            Err(other) => return Err(other),
        }
    }
    Ok(found)
}

/// Glyphs designed 8 pixels wide for those of `characters` that a font has so, from the first of
/// `sources` that has each; a character no font draws narrow is left out.
pub fn narrow_glyphs(sources: &[Font], characters: &[char]) -> Result<BTreeMap<char, Glyph>> {
    let mut found = BTreeMap::new();
    for source in sources {
        let missing: Vec<char> = characters.iter().copied().filter(|char| !found.contains_key(char)).collect();
        if missing.is_empty() {
            break;
        }
        match load_unifont(source, &missing, true) {
            Ok(glyphs) => found.extend(glyphs.into_iter().filter(|(_, glyph)| glyph.width == 8)),
            Err(Error::Font(message)) => return text_error(format!("{}: {message}", source.name())),
            Err(other) => return Err(other),
        }
    }
    Ok(found)
}

/// Give each new character one of the free places in a font, in order. `what` names the font in the
/// message when there are too few: "the font".
pub fn assign_slots(characters: &[char], free: &[usize], what: &str) -> Result<BTreeMap<char, usize>> {
    if characters.len() > free.len() {
        return text_error(format!(
            "the pack needs {} new characters and {what} has room for {}",
            characters.len(),
            free.len()
        ));
    }
    Ok(characters.iter().copied().zip(free.iter().copied()).collect())
}

// --- two narrow characters to a cell ---------------------------------------
//
// A game whose font has cells as wide as a kanji can draw two narrow letters in each, and so fit
// twice the text on a line: every pair of characters a translation uses becomes a glyph of its own.

/// Narrow characters in the order they are written, two to a cell. An odd one at the end is alone.
pub fn pair_up(run: &[char]) -> Vec<(char, Option<char>)> {
    run.chunks(2).map(|pair| (pair[0], pair.get(1).copied())).collect()
}

/// Whether a character is a kanji: one of the unified ideographs, or of their first extension.
/// Fonts that hold a game's kanji in cells of their own are where pairs find room once the
/// strings that used those kanji are translated.
pub fn is_kanji(char: char) -> bool {
    matches!(char, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}')
}

/// A narrow glyph (one designed 8 pixels wide) fitted to `width` columns, the last of which is left
/// empty so that letters side by side do not touch: its rows, the leftmost pixel in the top bit.
///
/// Empty columns go first, from the right edge, then from the left, which is where Unifont leaves
/// its own room. A glyph whose ink is still too wide (Unifont's `m`, `w`, `M`, `W`) loses the
/// columns inside it that have the least ink, the middlemost first, so that its strokes stay.
pub fn narrowed(glyph: &Glyph, width: usize) -> [u16; CELL] {
    let column = |x: usize| -> u16 { (0..CELL).fold(0, |bits, y| bits | (glyph.rows[y] >> (CELL - 1 - x) & 1) << y) };
    let mut columns: Vec<u16> = (0..glyph.width.min(CELL)).map(column).collect();
    let room = width.saturating_sub(1);
    while columns.len() > room {
        if columns.last() == Some(&0) {
            columns.pop();
        } else if columns.first() == Some(&0) {
            columns.remove(0);
        } else if columns.len() > 2 {
            let middle = (columns.len() - 1) as f64 / 2.0;
            let weight = |x: usize| (columns[x].count_ones(), ((x as f64 - middle).abs() * 2.0) as u32);
            let thinnest = (1..columns.len() - 1).min_by_key(|x| weight(*x)).unwrap_or(1);
            columns.remove(thinnest);
        } else {
            columns.pop();
        }
    }
    std::array::from_fn(|y| {
        columns.iter().enumerate().fold(0, |row, (x, bits)| row | (bits >> y & 1) << (CELL - 1 - x))
    })
}

/// A glyph of any size, as its font designed it: nothing scaled, nothing trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    /// Top to bottom. A row is `width` pixels packed eight to a byte, the leftmost in the top bit of
    /// the first byte; the bits after the last pixel are zero.
    pub rows: Vec<Vec<u8>>,
    pub width: usize,
    /// How many of the rows are above the baseline.
    pub ascent: i64,
}

impl Bitmap {
    /// Whether the glyph has ink at column `x` of row `y`; nothing outside it.
    pub fn pixel(&self, x: i64, y: i64) -> bool {
        let inside = 0 <= y && (y as usize) < self.rows.len() && 0 <= x && (x as usize) < self.width;
        inside && self.rows[y as usize][x as usize / 8] >> (7 - x as usize % 8) & 1 == 1
    }
}

/// The whole numbers that follow a BDF keyword.
fn bdf_numbers(keyword: &str, rest: &str) -> Result<Vec<i64>> {
    rest.split_whitespace()
        .map(|number| {
            number.parse().map_err(|_| Error::Font(format!("{keyword} is followed by {number:?}, not a number")))
        })
        .collect()
}

/// Read glyphs for `characters` from a BDF font, each laid out in the font's own cell.
///
/// Characters the font lacks are left out of the result.
pub fn load_bdf(font: &Font, characters: &[char]) -> Result<BTreeMap<char, Bitmap>> {
    let wanted: BTreeMap<i64, char> = characters.iter().map(|char| (*char as i64, *char)).collect();
    let mut found = BTreeMap::new();
    let mut cell: Option<Vec<i64>> = None;
    let mut code = -1;
    let mut bounds = vec![0; 4];
    let mut rows: Option<Vec<String>> = None;
    for line in Lines::open(font)? {
        // The format is ASCII; a name or a comment in another encoding is read byte for byte.
        let line: String = line?.iter().map(|byte| *byte as char).collect();
        let (keyword, rest) = line.split_once(' ').unwrap_or((&line, ""));
        match keyword {
            "FONTBOUNDINGBOX" => cell = Some(bdf_numbers(keyword, rest)?),
            "ENCODING" => match rest.split_whitespace().next() {
                Some(number) => code = bdf_numbers(keyword, number)?[0],
                None => return font_error("ENCODING is followed by no number"),
            },
            "BBX" => bounds = bdf_numbers(keyword, rest)?,
            "BITMAP" => rows = Some(Vec::new()),
            "ENDCHAR" => {
                if let (Some(char), Some(rows)) = (wanted.get(&code), &rows) {
                    let Some(cell) = &cell else {
                        return font_error(format!("{} gives no size for its glyphs", font.name()));
                    };
                    found.insert(*char, in_cell(rows, &bounds, cell)?);
                }
                rows = None;
            }
            _ => {
                if let Some(rows) = rows.as_mut()
                    && !keyword.is_empty()
                {
                    rows.push(keyword.to_string());
                }
            }
        }
    }
    Ok(found)
}

/// The leftmost `width` pixels of a row of a BDF glyph, which is written in hexadecimal and padded
/// on the right to whole bytes.
fn leading_pixels(row: &str, width: usize) -> Result<Vec<bool>> {
    let mut digits = Vec::with_capacity(row.len());
    for char in row.chars() {
        match char.to_digit(16) {
            Some(digit) => digits.push(digit),
            None => return font_error(format!("a row of a glyph, {row:?}, is not written in hexadecimal")),
        }
    }
    if digits.len() * 4 < width {
        return font_error(format!("a row of a glyph, {row:?}, is shorter than the glyph is wide"));
    }
    Ok((0..width).map(|index| digits[index / 4] >> (3 - index % 4) & 1 == 1).collect())
}

/// Place a BDF glyph, given as rows of hexadecimal and its bounding box, in the font's cell.
fn in_cell(rows: &[String], bounds: &[i64], cell: &[i64]) -> Result<Bitmap> {
    let (&[width, height, left, bottom], &[cell_width, cell_height, cell_left, cell_bottom]) = (bounds, cell) else {
        return font_error("a bounding box is not four numbers");
    };
    if width < 0 || height < 0 || cell_width < 0 || cell_height < 0 {
        return font_error("a bounding box has a negative size");
    }
    let ascent = cell_height + cell_bottom;
    let top = ascent - (height + bottom); // rows of the cell above the glyph's first row
    let indent = left - cell_left; // columns of the cell left of the glyph
    let mut placed = vec![vec![0u8; (cell_width as usize).div_ceil(8)]; cell_height as usize];
    for (index, row) in rows.iter().take(height as usize).enumerate() {
        let pixels = leading_pixels(row, width as usize)?;
        let y = top + index as i64;
        if !(0..cell_height).contains(&y) {
            continue;
        }
        // What would fall outside the cell, on either side, is dropped.
        for (x, _) in pixels.iter().enumerate().filter(|(_, ink)| **ink) {
            let column = indent + x as i64;
            if (0..cell_width).contains(&column) {
                placed[y as usize][column as usize / 8] |= 0x80 >> (column % 8);
            }
        }
    }
    Ok(Bitmap { rows: placed, width: cell_width as usize, ascent })
}

/// Glyphs for the characters the game's font lacks, from the pack's fonts, tried in the order given.
///
/// A font is a BDF file or a Unifont `.hex` file. Fails with `Error::Text` when characters are
/// needed and there is no font, or no font has one of them; with `allow_missing`, those are left
/// out instead.
pub fn new_bitmaps(sources: &[Font], characters: &[char], allow_missing: bool) -> Result<BTreeMap<char, Bitmap>> {
    let mut found = BTreeMap::new();
    if characters.is_empty() || (allow_missing && sources.is_empty()) {
        return Ok(found);
    }
    if sources.is_empty() {
        return text_error(no_font(characters));
    }
    let lacking = |found: &BTreeMap<char, Bitmap>| -> Vec<char> {
        characters.iter().copied().filter(|char| !found.contains_key(char)).collect()
    };
    for source in sources {
        let missing = lacking(&found);
        if missing.is_empty() {
            break;
        }
        let loaded = if source.is_bdf() {
            load_bdf(source, &missing)
        } else {
            load_unifont(source, &missing, true).map(|glyphs| {
                let bitmap = |glyph: Glyph| Bitmap {
                    rows: glyph.rows.iter().map(|row| row.to_be_bytes()[..glyph.width / 8].to_vec()).collect(),
                    width: glyph.width,
                    ascent: UNIFONT_ASCENT,
                };
                glyphs.into_iter().map(|(char, glyph)| (char, bitmap(glyph))).collect()
            })
        };
        match loaded {
            Ok(glyphs) => found.extend(glyphs),
            Err(Error::Font(message)) => {
                return text_error(format!("{}: {message}", source.name()));
            }
            Err(other) => return Err(other),
        }
    }
    let missing = lacking(&found);
    if !missing.is_empty() && !allow_missing {
        let shown = shown_with_codes(missing.iter().copied());
        return text_error(format!("no font given has a glyph for {} characters: {shown}", missing.len()));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    /// An "i" and a space in 8-pixel cells, and a full 16-pixel cell.
    fn hex_font() -> String {
        [
            format!("0020:{}", "00".repeat(16)),
            "0069:000000080800180808080808083E0000".to_string(),
            format!("4E00:{}FFFE{}", "0000".repeat(7), "0000".repeat(8)),
            String::new(),
        ]
        .join("\n")
    }

    fn characters(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn a_unifont_file_is_read_plain_or_compressed() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("font.hex");
        fs::write(&path, hex_font()).unwrap();
        let glyphs = load_unifont(&Font::from(&path), &characters("i一"), false).unwrap();
        assert_eq!(glyphs.keys().copied().collect::<Vec<_>>(), ['i', '一']);
        assert_eq!((glyphs[&'i'].width, glyphs[&'一'].width), (8, 16));
        // An 8-pixel cell sits in the left half of the 16-bit row.
        assert_eq!((glyphs[&'i'].rows[3], glyphs[&'i'].rows[13]), (0x0800, 0x3E00));
        assert_eq!(glyphs[&'一'].rows[7], 0xFFFE);
        assert!(load_unifont(&Font::from(&path), &[], false).unwrap().is_empty());

        let packed = folder.path().join("font.hex.gz");
        let mut out = GzEncoder::new(File::create(&packed).unwrap(), Compression::default());
        out.write_all(hex_font().as_bytes()).unwrap();
        out.finish().unwrap();
        assert_eq!(load_unifont(&Font::from(&packed), &['i'], false).unwrap(), BTreeMap::from([('i', glyphs[&'i'])]));

        // The same file with the line ends of another system.
        fs::write(&path, hex_font().replace('\n', "\r\n")).unwrap();
        assert_eq!(load_unifont(&Font::from(&path), &characters("i一"), false).unwrap(), glyphs);

        let error = load_unifont(&Font::from(&path), &characters("iZж"), false).unwrap_err();
        assert!(matches!(error, Error::Font(_)));
        assert_eq!(error.to_string(), "the font has no glyph for 2 characters: Z (U+005A) ж (U+0436)");
        assert_eq!(load_unifont(&Font::from(&path), &characters("iZж"), true).unwrap().len(), 1);

        fs::write(&path, "0041:00FF\n").unwrap();
        assert_eq!(
            load_unifont(&Font::from(&path), &['A'], false).unwrap_err().to_string(),
            "unexpected glyph size for U+0041"
        );
    }

    #[test]
    fn proportional_trims_narrow_cells_only() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("font.hex");
        fs::write(&path, hex_font()).unwrap();
        let glyphs = load_unifont(&Font::from(&path), &characters(" i一"), false).unwrap();
        let (rows, advance) = proportional(&glyphs[&'i']);
        // The ink is five pixels wide (the serif row $3E); it moves to start one pixel in.
        assert_eq!(advance, 6);
        assert_eq!((rows[13], rows[3]), (0x7C00, 0x1000));
        assert_eq!(proportional(&glyphs[&' ']), (glyphs[&' '].rows, SPACE_ADVANCE));
        assert_eq!(proportional(&glyphs[&'一']), (glyphs[&'一'].rows, 16));
    }

    #[test]
    fn outline_is_the_four_neighbours_clipped_to_the_cell() {
        let mut rows = [0u16; 16];
        rows[5] = 0x0100;
        let edge = outline(&rows);
        assert_eq!((edge[4], edge[6]), (0x0100, 0x0100));
        assert_eq!(edge[5], 0x0280); // left and right of the pixel, not the pixel itself
        assert_eq!(edge.iter().filter(|row| **row != 0).count(), 3);
        let mut rows = [0u16; 16];
        rows[0] = 0x8001;
        let corner = outline(&rows);
        assert_eq!((corner[0], corner[1]), (0x4002, 0x8001)); // nothing spills past either edge
        assert_eq!(outline(&[0; 16]), [0; 16]);
    }

    #[test]
    fn tiles_are_read_from_an_8x8_font() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("font8.hex");
        fs::write(&path, format!("0041:183C66667E666600\n0042:{}\n0061:00003C063E663E00\n", "00".repeat(16))).unwrap();
        let tiles = load_tiles(&Font::from(&path), &characters("Aa")).unwrap();
        assert_eq!(tiles[&'A'], [0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00]);
        assert_eq!(tiles[&'a'][2], 0x3C);
        // A 16-row glyph in the same file is not an 8x8 tile.
        assert_eq!(
            load_tiles(&Font::from(&path), &characters("AB")).unwrap_err().to_string(),
            "the menu font has no 8x8 glyph for 1 characters: B (U+0042)"
        );
    }

    #[test]
    fn a_tile_is_shaded_in_three_tones() {
        // One pixel on the fourth row: colour 2 for it, colour 3 to its right and below.
        let tile = shaded_tile(&[0, 0, 0, 0b0010_0000, 0, 0, 0, 0], 3);
        let planes: Vec<(u8, u8)> = tile.chunks(2).map(|pair| (pair[0], pair[1])).collect();
        assert_eq!(planes[3], (0b0001_0000, 0b0011_0000));
        assert_eq!(planes[4], (0b0010_0000, 0b0010_0000));
        assert!(planes.iter().enumerate().all(|(y, pair)| y == 3 || y == 4 || *pair == (0, 0)));
        // In the upper rows the glyph itself is colour 1.
        let top = shaded_tile(&[0b0100_0000, 0, 0, 0, 0, 0, 0, 0], 3);
        assert_eq!((top[0], top[1]), (0b0110_0000, 0b0010_0000));
        // Shadow never leaves the tile: nothing right of the last column or below the last row.
        let edge = shaded_tile(&[0, 0, 0, 0, 0, 0, 0, 0b0000_0001], 3);
        assert_eq!(edge, [[0u8; 14].as_slice(), &[0, 1]].concat());
    }

    #[test]
    fn new_glyphs_come_from_the_first_font_that_has_them() {
        let folder = tempfile::tempdir().unwrap();
        let first = folder.path().join("first.hex");
        let second = folder.path().join("second.hex");
        fs::write(&first, format!("0069:{}\n", "FF".repeat(16))).unwrap();
        fs::write(&second, hex_font()).unwrap();
        let both = [Font::from(&first), Font::from(&second)];
        let glyphs = new_glyphs(&both, &characters("i一")).unwrap();
        assert_eq!((glyphs[&'i'].rows[0], glyphs[&'一'].rows[7]), (0xFF00, 0xFFFE));
        assert!(new_glyphs(&[], &[]).unwrap().is_empty());

        let error = new_glyphs(&[], &characters("ab")).unwrap_err();
        assert!(matches!(error, Error::Text(_)));
        assert_eq!(
            error.to_string(),
            "the pack uses 2 characters the game's font lacks (a b); a font is needed to draw them"
        );
        // Only the last font says what is still missing, and a translator is told as for any other fault.
        let error = new_glyphs(&both, &characters("iZ")).unwrap_err();
        assert!(matches!(error, Error::Text(_)));
        assert_eq!(error.to_string(), "the font has no glyph for 1 characters: Z (U+005A)");
    }

    #[test]
    fn slots_are_given_in_order_while_there_is_room() {
        assert_eq!(assign_slots(&['a', 'b'], &[7, 9, 11], "the font").unwrap(), BTreeMap::from([('a', 7), ('b', 9)]));
        assert_eq!(
            assign_slots(&['a', 'b'], &[7], "the menu font").unwrap_err().to_string(),
            "the pack needs 2 new characters and the menu font has room for 1"
        );
    }

    #[test]
    fn narrow_characters_go_two_to_a_cell() {
        assert_eq!(pair_up(&characters("abcde")), [('a', Some('b')), ('c', Some('d')), ('e', None)]);
        assert!(pair_up(&[]).is_empty());
        assert!(is_kanji('目') && is_kanji('㐀') && !is_kanji('あ') && !is_kanji('Ａ'));
    }

    /// A narrow glyph from rows of 8 pixels written as text, `#` for ink.
    fn drawn(rows: &[&str]) -> Glyph {
        let mut glyph = Glyph { rows: [0; CELL], width: 8 };
        for (y, row) in rows.iter().enumerate() {
            glyph.rows[y] = row.chars().enumerate().filter(|(_, c)| *c == '#').map(|(x, _)| 0x8000 >> x).sum();
        }
        glyph
    }

    #[test]
    fn a_narrow_glyph_is_fitted_to_fewer_columns_with_its_last_one_empty() {
        // Ink in columns 1 to 6, as most of Unifont's letters: the empty columns at the edges go.
        let o = drawn(&[".######.", ".#....#.", ".######."]);
        assert_eq!(narrowed(&o, 7)[..3], [0xFC00, 0x8400, 0xFC00]);
        // Ink in seven columns, as Unifont's m: the inner column with least ink goes, the middlemost first.
        let m = drawn(&[".###.##.", ".#..#..#", ".#..#..#"]);
        let fitted = narrowed(&m, 7);
        assert_eq!(fitted[..3], [0xD800, 0xA400, 0xA400]);
        assert!(fitted.iter().all(|row| row & 0x0300 == 0));
        // Narrower still: a column of ink at most, and never past the room.
        assert!(narrowed(&m, 6).iter().all(|row| row & 0x07FF == 0));
        assert_eq!(narrowed(&drawn(&["...##..."]), 7)[0], 0x1800);
    }

    /// A font with a cell 6 wide and 8 high, 2 rows of it below the baseline, and three glyphs:
    /// one that fills its box, one small and raised, one that hangs below the baseline.
    const BDF: &str = "STARTFONT 2.1\n\
        FONT -made-up\n\
        FONTBOUNDINGBOX 6 8 0 -2\n\
        STARTCHAR A\nENCODING 65\nBBX 6 8 0 -2\nBITMAP\nFC\n84\n84\n84\n84\n84\n84\nFC\nENDCHAR\n\
        STARTCHAR dot\nENCODING 46\nBBX 2 2 1 0\nBITMAP\nC0\nC0\nENDCHAR\n\
        STARTCHAR comma\nENCODING 44\nBBX 1 3 4 -2\nBITMAP\n80\n80\n80\nENDCHAR\n\
        ENDFONT\n";

    #[test]
    fn a_bdf_glyph_is_placed_in_the_font_s_cell() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("small.bdf");
        fs::write(&path, BDF).unwrap();
        let glyphs = load_bdf(&Font::from(&path), &characters("A.,Z")).unwrap();
        assert_eq!(glyphs.keys().copied().collect::<Vec<_>>(), [',', '.', 'A']);
        let letter = &glyphs[&'A'];
        assert_eq!((letter.width, letter.ascent), (6, 6));
        // Six pixels a row, from the top bit of a byte.
        assert_eq!(letter.rows, [[0xFC], [0x84], [0x84], [0x84], [0x84], [0x84], [0x84], [0xFC]]);
        assert!(letter.pixel(0, 0) && letter.pixel(5, 7) && !letter.pixel(1, 1));
        assert!(!letter.pixel(-1, 0) && !letter.pixel(6, 0) && !letter.pixel(0, 8));
        // Two rows on the baseline, one column in from the left.
        assert_eq!(glyphs[&'.'].rows, [[0], [0], [0], [0], [0b0110_0000], [0b0110_0000], [0], [0]]);
        // The lowest three rows of the cell, in its fifth column.
        assert_eq!(glyphs[&','].rows, [[0], [0], [0], [0], [0], [0b0000_1000], [0b0000_1000], [0b0000_1000]]);

        // A glyph may be as wide as its font likes, and one that reaches past its cell is cut at the edge.
        let wide = "FONTBOUNDINGBOX 72 2 0 0\n\
            ENCODING 65\nBBX 72 2 0 0\nBITMAP\n800000000000000001\n0000000000000000FF\nENDCHAR\n\
            ENCODING 66\nBBX 8 1 68 1\nBITMAP\nFF\nENDCHAR\n\
            ENCODING 67\nBBX 8 1 -4 3\nBITMAP\nFF\nENDCHAR\n";
        fs::write(&path, wide).unwrap();
        let glyphs = load_bdf(&Font::from(&path), &characters("ABC")).unwrap();
        let ink = |glyph: &Bitmap, y: i64| (0..72).filter(|x| glyph.pixel(*x, y)).collect::<Vec<_>>();
        assert_eq!((glyphs[&'A'].width, glyphs[&'A'].rows.len(), glyphs[&'A'].ascent), (72, 2, 2));
        assert_eq!(ink(&glyphs[&'A'], 0), [0, 71]);
        assert_eq!(ink(&glyphs[&'A'], 1), [64, 65, 66, 67, 68, 69, 70, 71]);
        assert_eq!((ink(&glyphs[&'B'], 0), ink(&glyphs[&'B'], 1)), (vec![68, 69, 70, 71], vec![]));
        // Above the cell altogether: nothing of it is kept.
        assert_eq!(glyphs[&'C'].rows, [[0; 9], [0; 9]]);

        fs::write(&path, BDF.replace("FONTBOUNDINGBOX 6 8 0 -2\n", "")).unwrap();
        assert_eq!(
            load_bdf(&Font::from(&path), &['A']).unwrap_err().to_string(),
            "small.bdf gives no size for its glyphs"
        );
    }

    #[test]
    fn the_built_in_fonts_are_read_like_files() {
        // Each is found by its name, has its terms with it, and gives glyphs.
        let names: Vec<&str> = BUILT_IN.iter().map(|font| font.name).collect();
        assert_eq!(names, ["unifont", "dejavu-condensed-bold", "unscii-8"]);
        assert!(
            BUILT_IN.iter().all(|font| !font.about.is_empty() && !font.license.is_empty() && font.terms.len() > 100)
        );
        assert!(built_in("no-such-font").is_none());

        let unifont = Font::named("unifont");
        assert!(matches!(unifont, Font::BuiltIn(_)) && unifont.name() == "unifont" && !unifont.is_bdf());
        let glyphs = load_unifont(&unifont, &characters("i一é"), false).unwrap();
        assert_eq!((glyphs[&'i'].width, glyphs[&'一'].width, glyphs[&'é'].width), (8, 16, 8));

        let dejavu = Font::named("dejavu-condensed-bold");
        assert!(dejavu.is_bdf());
        let bitmaps = load_bdf(&dejavu, &characters("Aéж一")).unwrap();
        assert_eq!(bitmaps.keys().copied().collect::<Vec<char>>(), ['A', 'é', 'ж']);
        assert_eq!((bitmaps[&'A'].width, bitmaps[&'A'].rows.len()), (96, 128));
        // A capital with an accent is drawn shorter than the others, so that the accent fits in the cell
        // above it: its inked rows are the accent, a gap, then the letter.
        let inked_runs = |bitmap: &Bitmap| {
            let inked: Vec<bool> = bitmap.rows.iter().map(|row| row.iter().any(|byte| *byte != 0)).collect();
            inked.iter().zip(std::iter::once(&false).chain(&inked)).filter(|(now, before)| **now && !**before).count()
        };
        let capitals = load_bdf(&dejavu, &characters("EÉÀÔ")).unwrap();
        assert_eq!(capitals.values().map(inked_runs).collect::<Vec<usize>>(), [1, 2, 2, 2]);
        // What the first lacks comes from the next, as with files.
        let both = Font::built_ins(&["dejavu-condensed-bold", "no-such-font", "unifont"]);
        assert_eq!(both, [dejavu.clone(), unifont.clone()]);
        let found = new_bitmaps(&both, &characters("A一"), false).unwrap();
        assert_eq!((found[&'A'].width, found[&'一'].width), (96, 16));

        let tiles = load_tiles(&Font::named("unscii-8"), &characters("Aa")).unwrap();
        assert!(tiles[&'A'] != [0; 8] && tiles[&'a'] != [0; 8]);

        // A name that is no built-in font's is a file's, and the two are told apart.
        let file = Font::named("work/fonts/unifont.hex");
        assert_eq!(file, Font::from(PathBuf::from("work/fonts/unifont.hex")));
        assert!(file != unifont && file.name() == "unifont.hex" && Font::named("mine.BDF").is_bdf());
        assert_eq!(format!("{unifont:?}"), "BuiltIn(BuiltIn(unifont))");
    }

    #[test]
    fn bitmaps_come_from_bdf_and_unifont_alike() {
        let folder = tempfile::tempdir().unwrap();
        let bdf = folder.path().join("small.BDF");
        let hex = folder.path().join("font.hex");
        fs::write(&bdf, BDF).unwrap();
        fs::write(&hex, hex_font()).unwrap();
        let sources = [Font::from(&bdf), Font::from(&hex)];
        let found = new_bitmaps(&sources, &characters("Ai一"), false).unwrap();
        assert_eq!(found[&'A'].width, 6);
        // A Unifont glyph keeps the width of its cell, with its rows moved down to that many bits.
        assert_eq!((found[&'i'].width, found[&'i'].ascent, &found[&'i'].rows[13]), (8, UNIFONT_ASCENT, &vec![0x3E]));
        assert_eq!((found[&'一'].width, &found[&'一'].rows[7]), (16, &vec![0xFF, 0xFE]));

        assert_eq!(
            new_bitmaps(&sources, &characters("AZж"), false).unwrap_err().to_string(),
            "no font given has a glyph for 2 characters: Z (U+005A) ж (U+0436)"
        );
        assert_eq!(new_bitmaps(&sources, &characters("AZ"), true).unwrap().len(), 1);
        assert!(new_bitmaps(&[], &['A'], true).unwrap().is_empty());
        assert!(new_bitmaps(&[], &['A'], false).unwrap_err().to_string().ends_with("a font is needed to draw them"));

        // What is wrong with a font is reported under its name.
        fs::write(&hex, "0069:00FF\n").unwrap();
        let error = new_bitmaps(&[Font::from(&hex)], &['i'], false).unwrap_err();
        assert!(matches!(error, Error::Text(_)));
        assert_eq!(error.to_string(), "font.hex: unexpected glyph size for U+0069");
    }
}
