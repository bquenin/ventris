//! Super Robot Wars Z (PlayStation 2, 2008), released in Japan as Super Robot Taisen Z.
//!
//! What is known so far covers the game's text as far as it can be read without running the game:
//! where each body of it is, how it is compressed and encoded, how to write it out and read it back
//! without loss, and the font. Nothing can be put back yet: every kind of text here is
//! extract-only.
//!
//! The disc holds no single archive. The program the console starts, `SLPS_258.87`, finds its files
//! through `VMAP.DAT` (a list of names, first sectors and sector counts that matches the ISO 9660
//! directory), and most files are runs of compressed blocks, one after another, each starting on a
//! multiple of 16. A file's blocks have no names, only numbers, and the table of where each starts
//! is kept somewhere else: the program holds those of the library, the prologue, the help screens
//! and the summaries (see `PROGRAM_TABLES`), and `HEDBDY/HB.BIN` holds that of the stages. Every
//! block is compressed the one way (`codecs::lz_varint`). Inside a block there is often a small
//! pack: how many entries, the size of the header, then an offset and a size for each entry, the
//! offsets counted from the end of the header.
//!
//! The text is Shift-JIS as Windows has it (code page 932): two-byte characters, with the row of
//! NEC symbols (circled numbers, Roman numerals) that plain Shift-JIS lacks. A string ends with a
//! zero byte. One body of text, the library, is also masked: every byte but 0x00 and 0x5E is XORed
//! with 0x5E.
//!
//! All of the game's text that was seen on screen is drawn by one loop in the program (at 0x13A290
//! in memory), which always reads two bytes for a character and has no single-byte characters at
//! all: ASCII is never drawn. Before that, it turns 0x0A into 0x2E and a zero byte into 0x2F, and
//! sends bytes 0x2E to 0x3D through a table of sixteen codes (at 0x433E50), in every string,
//! dialogue included. So a digit, `.`, `/`, `:`, `;`, `<` or `=` written as one byte is never a
//! character. What the codes do, from the loop's code:
//!
//! - 0x2E starts a new line, as 0x0A does, and 0x2F ends the string.
//! - 0x30 calls a routine (0x139930) and skips the byte after it; 0x31 and 0x36 to 0x39 each store
//!   the byte after them in a field of the text's style whose use was not followed (0x31 is used
//!   before words of the menus, and is probably a colour).
//! - 0x32 to 0x35 store the byte after them as the glyph's drawn width, its drawn height, the pen's
//!   advance after each character and the step between lines. These four were seen at work on
//!   screen.
//! - 0x3A to 0x3D draw a name kept in memory: the hero's given name, family name, short name and
//!   the squad's name (in a new game as Rand: ランド, トラビス, ランド, ＺＥＵＴＨ).
//!
//! In a script these are tags: `{2E}` `{2F}`, `{30:xx}`, `{31:xx}`, `{WIDTH:xx}`, `{HEIGHT:xx}`,
//! `{ADVANCE:xx}`, `{LINE_STEP:xx}`, `{36:xx}` to `{39:xx}`, and `{GIVEN_NAME}` `{FAMILY_NAME}`
//! `{SHORT_NAME}` `{SQUAD_NAME}`, so that no bare digit in a script is a code. The stages' own text
//! writes the names differently, as `$n`, `$F`, `$f`, `$l` and `$c`, which something before the
//! loop fills in: the hero's name, full name with rank, short name, family name and the squad's
//! name. They are `{HERO}`, `{HERO_FULL}`, `{HERO_SHORT}`, `{HERO_FAMILY}` and `{SQUAD}`. The help
//! of the data lists draws icons with `<14>` and buttons with `<-5>` (as the text around them
//! says), and has a code `[10]` whose meaning was not found; each is kept as written, inside braces
//! (`{<14>}`, `{<-5>}`, `{[10]}`), so that it moves as a whole. Any other byte that is not part of
//! a character is written as its number, `{1F}`. The glossary links of the dialogue, `《term》`, are
//! plain characters, as the game stores them.
//!
//! The font is the third of the fourteen segments of `DATA/VT1.BIN` (whose table is in the
//! program), one compressed block that expands to 4,480 glyphs of 24 by 24 pixels at 4 bits a
//! pixel, the left pixel in the low half of each byte. The glyph of a code below 0x989F with a
//! first byte from 0x81 to 0x98 is (first − 0x81) × 192 + (second − 0x40). From 0x989F up, where
//! the second level of kanji is, the program looks the code up in a table of its own (at 0x3F7D70:
//! a 16-bit code, a signed byte and a byte, ending with code 0), and the glyph is 224 × the first
//! byte + the second; six codes are there twice, and the first is taken; a code that is not in the
//! table is drawn blank. A code the font cannot draw, or one that is not a character of code page
//! 932 written the way the game writes it, is kept in a script as `{G:xxxx}`, and these are what
//! `unmapped` reports. The game's own text has two, kanji of IBM's extension in two names in the
//! credits, which the game draws blank.
//!
//! Where the text is, by body:
//!
//! The stages. `DATA/STAGE.BIN` is 205 blocks (offsets in segment 7 of `HEDBDY/HB.BIN`, whose
//! segment bounds are in `HEDBDY/HBBDY.BIN`). Each is a pack of one entry, a program overlay
//! (Metrowerks `MWo3`) that the game loads at 0x756700: a header of 0x80 bytes with the overlay's
//! name (`stg_010.bin`; block 0 is `flow.bin`, the intermission), its code, then its data. A
//! stage's lines are in lists of 32-byte records in the data, each record a kind, three numbers
//! and, at +0x10, the address of its text: kinds 0, 1, 2, 5, 6 and 7 are a line (the numbers are an
//! expression, the window and the portrait), 10 to 12 a choice (with the number of choices at
//! +0x14), and 16 to 18 a place's caption, three records for one caption. A line whose text has
//! glossary links has, at +0x18, the address of a list of the glossary entries it links to, each
//! entry four addresses: the term, its series, and two descriptions. The stage's code stores the
//! addresses of its conditions of victory, of defeat and of the SR point, and of what follows them,
//! at 0x5952B0, 0x5952B8, 0x5952C0 and 0x5952C8: each is a list of addresses of strings, ending
//! with a zero or where the next starts. A string is taken to be one of these only where the record
//! holds together: the record starts on a multiple of 16, its numbers are in the ranges the game
//! uses, the words after the address are what that kind has there, and the address is of text that
//! starts after a zero byte. Many strings are pointed at by more than one record. A stage's lines
//! and choices are section `stg_010`, its captions, conditions and glossary entries `stg_010-info`,
//! each string numbered by its offset in the overlay (its address less 0x756700). The
//! intermission's lines are `flow`; its summaries of each stage (a table of addresses at 0x10DC0 of
//! the overlay, ending with a zero) and three button hints (at 0x11E98) are `flow-info`.
//!
//! The battle lines are `BTL/SRVC.BIN`, not compressed, one segment per character (offsets in
//! `BTL/SRVC.SEG`). A segment starts with 0x4F00 and three counts, then a table of 79 entries of
//! four bytes, a table of 8-byte entries (one per situation), one of 8-byte and one of 4-byte
//! entries (the low and high bytes of the first count say how many), and the records, eight bytes
//! each: a line's number and the offset of its text from the end of the records, where the text
//! starts. A battle line breaks its lines with the two characters `\n`, not with 0x0A, and in a
//! script that is a line break. Section `battle_267` is segment 267, numbered by offset in the
//! segment; segments with no records are left out, and so are strings of a segment's text that no
//! record points at.
//!
//! The data lists are `DATA/COMPDATA.BN`, one block, an overlay loaded at 0x6D6800 whose data is
//! tables of records and, from 0x61680 to its end, the strings those tables point at: the names of
//! pilots, units, weapons, skills and parts with their help, the lines pilots say as they retreat,
//! the titles of the music and the help of the options. Section `compdata`, by offset in the
//! overlay.
//!
//! The program, `SLPS_258.87`, holds the menus, system messages, the help of skills and spirit
//! commands and the parts' descriptions, as zero-ended strings on multiples of 8 among its data
//! (from 0x3F5600 in memory; its file's offset 0x1A80 is loaded at 0x100000). A string is what a
//! 32-bit word of the program points at, or what its code builds the address of with a `lui` and an
//! `addiu` or `ori`, that starts after a zero byte on a multiple of 8 and reads as text: characters
//! the font has, line breaks, and the codes 0x31 to 0x35 with their bytes. Section `program`, by
//! offset in the file. One such string is left out: the grid of letters the name-entry screen
//! offers.
//!
//! The library (robots, characters, keywords) is `DATA/MTVZKNRT.BIN`, `MTVZKNPT.BIN` and
//! `MTVZKNKW.BIN`, one block an entry, masked. An entry is `ZKAN`, its kind and two numbers, then
//! chunks: a tag of four letters, a size and a body (`DSIZ` and `DATA` have no body). The text
//! chunks are a robot's name (`RBTN`), series (`PRDC`), pilot (`PLTN`), height (`HEIT`), weight
//! (`WEIT`), a reading (`KANA`, once); a character's full name (`CHFN`), short name (`CHNN`),
//! series and voice actor (`ACTR`); a keyword (`WORD`) and its source (`SRCE`); and for all, two
//! descriptions (`DSCR`, `DSC2`). Sections `library-robots`, `library-characters` and
//! `library-keywords`, a string numbered by its entry times ten plus its field (see
//! `LIBRARY_FIELDS`).
//!
//! `DATA/NISVDATA.BIN` holds the squads' names (block 4: a pack whose entry is 104 records of 286
//! bytes, the name in a field of 28 bytes at +2), the help on SR points (block 5) and the
//! tutorial's pages (block 6, all entries but the first, which is an index). A help page is a
//! 16-bit length, then until it is used up, items of an 8-byte header (where and how to draw) and a
//! zero-ended string. Sections `squads` (by record), `srpoint` and `tutorial` (by offset in the
//! block).
//!
//! `DATA/HSFC.BIN` holds the summaries of the stages shown when a game is loaded (block 0: 209 of
//! three lines, each line in a field of 50 bytes), section `summary`, by summary, its three lines
//! one string with line breaks; and the staff credits (block 2: entry 4 is a table of offsets into
//! the text of entry 6), section `credits`, by offset in the block.
//!
//! The prologue's narration is in `DATA/MTV_PROS.BIN`, 14 blocks of scenes made of tagged chunks;
//! the text is in the chunks tagged `rawt`. Section `prologue`, numbered by block times ten plus
//! the chunk's place among the block's `rawt` chunks. The names of the maps are `MAP/MAPNAME.BIN`,
//! 195 fields of 256 bytes, section `places`, by field.
//!
//! Not covered yet: the names of the squads and units on a stage's map (records the stage's code
//! points at, with a name's address at +0x1C); a few dozen messages of the stages that other
//! records point at (a weapon added, points gained); the fixed-width name fields of the data lists'
//! pilot records; the names in the attract demos (`BTL/OP0-2.BIN`); the kanji the name-entry screen
//! offers (block 3 of `NISVDATA.BIN`); the strings of the battle segments that no record points at;
//! and text drawn as pictures. Found later, each will be added where it is without renumbering what
//! is here.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, LazyLock};

use crate::adapter::{Adapter, CatalogueEntry, Refused, Release, TextKind};
use crate::codecs::lz_varint;
use crate::error::{Error, Result, text_error};
use crate::platform::{Image, Platform};
use crate::platforms::ps2::{self, Disc};
use crate::script::{self, Pack, Script};
use crate::shift_jis;
use crate::text::{Token, from_hex, quoted, tokens};

pub const NAME: &str = "Super Robot Wars Z (PlayStation 2, Japan)";
/// The whole disc image, version 1.04.
pub const ROM_SHA1: &str = "e8dbe37e88afe8f82d48889b0775274ccde3cf99";
pub const SOURCE_LANGUAGE: &str = "ja";

// --- the program and its tables

pub const PROGRAM: &str = "SLPS_258.87";
/// Where the program's one loaded segment starts in its file, and where it is loaded.
const PROGRAM_FILE_START: usize = 0x1A80;
const PROGRAM_BASE: u32 = 0x10_0000;
/// The end of the program's code in memory, and of what is loaded.
const PROGRAM_CODE_END: u32 = 0x3F_5600;
const PROGRAM_END: u32 = 0x44_BC80;
/// The compiler puts every string on a multiple of this.
const PROGRAM_ALIGNMENT: u32 = 8;
/// The codes the program's own strings use, each with one byte after it.
const PROGRAM_CODES: [u8; 5] = [0x31, 0x32, 0x33, 0x34, 0x35];
/// The letters of the name-entry screen, one string of the program, which is left out: a translation
/// does not change how a name is entered.
const NAME_ENTRY_LETTERS: usize = 0x32_8730;

/// A file that is a run of compressed blocks, and the table of where they start, in the program.
struct Blocks {
    file: &'static str,
    /// The table's address in memory: one 32-bit offset a block, and the file's size after them.
    table: u32,
    count: usize,
}

const FONT_FILE: Blocks = Blocks { file: "DATA/VT1.BIN", table: 0x3F_8680, count: 14 };
const ROBOTS: Blocks = Blocks { file: "DATA/MTVZKNRT.BIN", table: 0x42_9910, count: 321 };
const CHARACTERS: Blocks = Blocks { file: "DATA/MTVZKNPT.BIN", table: 0x42_8D90, count: 411 };
const KEYWORDS: Blocks = Blocks { file: "DATA/MTVZKNKW.BIN", table: 0x42_9F00, count: 52 };
const PROLOGUE: Blocks = Blocks { file: "DATA/MTV_PROS.BIN", table: 0x42_A020, count: 14 };
const NISVDATA: Blocks = Blocks { file: "DATA/NISVDATA.BIN", table: 0x42_7410, count: 7 };
const HSFC: Blocks = Blocks { file: "DATA/HSFC.BIN", table: 0x43_A3A0, count: 4 };
/// The tables the program holds, for the record.
pub const PROGRAM_TABLES: [(&str, u32); 7] = [
    (FONT_FILE.file, FONT_FILE.table),
    (ROBOTS.file, ROBOTS.table),
    (CHARACTERS.file, CHARACTERS.table),
    (KEYWORDS.file, KEYWORDS.table),
    (PROLOGUE.file, PROLOGUE.table),
    (NISVDATA.file, NISVDATA.table),
    (HSFC.file, HSFC.table),
];

// --- the stages

const STAGES: &str = "DATA/STAGE.BIN";
const STAGE_BLOCKS: usize = 205;
const STAGE_TABLE_FILE: &str = "HEDBDY/HB.BIN";
const STAGE_TABLE_BOUNDS: &str = "HEDBDY/HBBDY.BIN";
/// Which segment of `HB.BIN` is the table of the stages' blocks.
const STAGE_TABLE_SEGMENT: usize = 7;
/// Where the game loads a stage's overlay.
const STAGE_BASE: u32 = 0x75_6700;
/// An overlay's header: where its name is, how long the name may be, and where the size of its code
/// (with the header) is.
const OVERLAY_NAME: usize = 0x20;
const OVERLAY_NAME_SIZE: usize = 0x20;
const OVERLAY_CODE_SIZE: usize = 0x0C;
const OVERLAY_HEADER: usize = 0x80;
/// A record of a stage's lists.
const RECORD_SIZE: usize = 0x20;
const RECORD_ALIGNMENT: usize = 0x10;
const RECORD_TEXT: usize = 0x10;
const LINE_KINDS: [u32; 6] = [0, 1, 2, 5, 6, 7];
const CHOICE_KINDS: [u32; 3] = [10, 11, 12];
const CAPTION_KINDS: [u32; 3] = [16, 17, 18];
/// The ranges a line's numbers keep to: its expression, its window and its portrait.
const LINE_EXPRESSIONS: u32 = 16;
const LINE_WINDOWS: u32 = 0x400;
const LINE_PORTRAITS: u32 = 0x400;
/// The most choices a choice offers, and glossary entries a line links to.
const MOST_CHOICES: u32 = 9;
const MOST_LINKS: u32 = 16;
/// A glossary entry: the term, its series, and two descriptions.
const GLOSSARY_FIELDS: usize = 4;
/// Where a stage's code stores the addresses of its conditions of victory, defeat and SR point, and
/// of what follows them.
const CONDITION_SLOTS: [u32; 4] = [0x59_52B0, 0x59_52B8, 0x59_52C0, 0x59_52C8];
/// The intermission's overlay: its summaries of the stages and its button hints, two lists of
/// addresses ending with a zero, whose addresses its code builds (at 0x1AE0 and 0x1574).
const FLOW: &str = "flow";
const FLOW_SUMMARIES: usize = 0x1_0DC0;
const FLOW_HINTS: usize = 0x1_1E98;
const INFO: &str = "-info";

// --- the battle lines

const BATTLE_LINES: &str = "BTL/SRVC.BIN";
const BATTLE_SEGMENTS: &str = "BTL/SRVC.SEG";
const BATTLE_MAGIC: u16 = 0x4F00;
/// The first table of a segment: 79 entries of four bytes, after the 8-byte header.
const BATTLE_FIRST_TABLE: usize = 8 + 79 * 4;
const BATTLE_PREFIX: &str = "battle_";

// --- the data lists

const DATA_LISTS: &str = "DATA/COMPDATA.BN";
const DATA_LISTS_BASE: u32 = 0x6D_6800;
/// Where the strings start, after the tables.
const DATA_LISTS_STRINGS: usize = 0x6_1680;

// --- the library

const LIBRARY_MASK: u8 = 0x5E;
const LIBRARY_MAGIC: &[u8] = b"ZKAN";
const LIBRARY_HEAD: usize = 16;
/// Chunks that are a number, not a body.
const LIBRARY_VALUES: [&[u8]; 2] = [b"DSIZ", b"DATA"];
/// A library string is numbered by its entry times this, plus its field.
const LIBRARY_STRIDE: u32 = 10;
/// A text chunk of the library: its tag, its field number, and what it is.
pub type LibraryField = (&'static [u8; 4], u32, &'static str);
/// The text chunks of the library, by section.
pub const LIBRARY_FIELDS: [(&str, &[LibraryField]); 3] = [
    (
        "library-robots",
        &[
            (b"RBTN", 0, "the robot's name"),
            (b"PRDC", 1, "the series it is from"),
            (b"PLTN", 2, "its pilot"),
            (b"HEIT", 3, "its height"),
            (b"WEIT", 4, "its weight"),
            (b"DSCR", 5, "its description"),
            (b"DSC2", 6, "its second description"),
            (b"KANA", 7, "a reading of its name"),
        ],
    ),
    (
        "library-characters",
        &[
            (b"CHFN", 0, "the character's full name"),
            (b"CHNN", 1, "the character's short name"),
            (b"PRDC", 2, "the series the character is from"),
            (b"ACTR", 3, "the voice actor"),
            (b"DSCR", 5, "the character's description"),
            (b"DSC2", 6, "the character's second description"),
        ],
    ),
    (
        "library-keywords",
        &[
            (b"WORD", 0, "the keyword"),
            (b"SRCE", 1, "the series it is from"),
            (b"DSCR", 5, "its description"),
            (b"DSC2", 6, "its second description"),
        ],
    ),
];

// --- help, summaries, credits, prologue, places

const SQUAD_BLOCK: usize = 4;
const SR_POINT_BLOCK: usize = 5;
const TUTORIAL_BLOCK: usize = 6;
const SQUAD_RECORD: usize = 0x11E;
const SQUAD_NAME: std::ops::Range<usize> = 2..0x1E;
/// A help item's header, before its string.
const HELP_ITEM_HEADER: usize = 8;
const SUMMARY_BLOCK: usize = 0;
const SUMMARY_LINE: usize = 50;
const SUMMARY_LINES: usize = 3;
const CREDITS_BLOCK: usize = 2;
const CREDITS_TABLE: usize = 4;
const CREDITS_TEXT: usize = 6;
const PROLOGUE_TAG: &[u8] = b"rawt";
/// A prologue string is numbered by its block times this, plus its place among the block's.
const PROLOGUE_STRIDE: u32 = 10;
const PLACES: &str = "MAP/MAPNAME.BIN";
const PLACE_FIELD: usize = 0x100;

// --- the font

const FONT_SEGMENT: usize = 2;
pub const GLYPH_SIZE: usize = 24;
pub const GLYPH_BYTES: usize = 288;
pub const GLYPH_COUNT: usize = 4480;
pub const GLYPH_SHADES: [u8; 16] = [0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255];
const GLYPHS_PER_LEAD: usize = 192;
const FIRST_LEAD: u8 = 0x81;
const LAST_LINEAR_LEAD: u8 = 0x98;
const FIRST_TRAIL: u8 = 0x40;
/// From this code up, glyphs are looked up in the program's table.
const FIRST_LOOKED_UP: u16 = 0x989F;
const LOOKUP_TABLE: u32 = 0x3F_7D70;
const LOOKUP_ROW: usize = 224;

// --- the section names

const SUMMARY_SECTION: &str = "summary";
const CREDITS_SECTION: &str = "credits";
const PROLOGUE_SECTION: &str = "prologue";
const PLACES_SECTION: &str = "places";
const SQUADS_SECTION: &str = "squads";
const SR_POINT_SECTION: &str = "srpoint";
const TUTORIAL_SECTION: &str = "tutorial";
const DATA_LISTS_SECTION: &str = "compdata";
const PROGRAM_SECTION: &str = "program";

fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn half(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

/// The bytes from `at` up to the next zero byte, if there is one.
fn zero_ended(data: &[u8], at: usize) -> Option<&[u8]> {
    let rest = data.get(at..)?;
    Some(&rest[..rest.iter().position(|byte| *byte == 0)?])
}

/// What precedes a zero byte in `field`, or the whole field when it has none.
fn field_text(field: &[u8]) -> &[u8] {
    &field[..field.iter().position(|byte| *byte == 0).unwrap_or(field.len())]
}

/// The entries of a pack: how many, the size of its header, then for each an offset from the end of
/// the header and a size.
pub fn pack_entries(pack: &[u8]) -> Option<Vec<std::ops::Range<usize>>> {
    let count = word(pack, 0)? as usize;
    let header = word(pack, 4)? as usize;
    (0..count)
        .map(|entry| {
            let start = header.checked_add(word(pack, 8 + 8 * entry)? as usize)?;
            let end = start.checked_add(word(pack, 12 + 8 * entry)? as usize)?;
            (end <= pack.len()).then_some(start..end)
        })
        .collect()
}

fn pack_entry(pack: &[u8], entry: usize, what: &str) -> Result<std::ops::Range<usize>> {
    match pack_entries(pack).and_then(|entries| entries.get(entry).cloned()) {
        Some(range) => Ok(range),
        None => image_error(format!("{what} has no entry {entry}")),
    }
}

// --- reading the disc

/// The program the console starts, whole.
pub fn program_file(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("srwz-ps2: the program", || disc.read_from(PROGRAM, 0))
}

/// Where an address of the program's memory is in its file.
fn program_offset(address: u32) -> usize {
    (address - PROGRAM_BASE) as usize + PROGRAM_FILE_START
}

/// Where each of a file's blocks starts, and the file's end, from the program's table.
fn block_offsets(disc: &Disc, blocks: &Blocks) -> Result<Vec<u32>> {
    let program = program_file(disc)?;
    let start = program_offset(blocks.table);
    let offsets: Option<Vec<u32>> = (0..=blocks.count).map(|index| word(&program, start + 4 * index)).collect();
    let size = disc.size(blocks.file)?;
    match offsets {
        Some(offsets) if u64::from(offsets[blocks.count]) == size => Ok(offsets),
        _ => image_error(format!("the program's table of {} does not end with the file's size", blocks.file)),
    }
}

/// Block `index` of a file, expanded.
fn read_block(disc: &Disc, blocks: &Blocks, index: usize) -> Result<Vec<u8>> {
    let offsets = block_offsets(disc, blocks)?;
    let (start, end) = (u64::from(offsets[index]), u64::from(offsets[index + 1]));
    let data = disc.read(blocks.file, start, end - start)?;
    Ok(lz_varint::decompress(&data, 0)?.0)
}

/// Every block of a file, expanded, read once for an image.
fn all_blocks(disc: &Disc, blocks: &Blocks, name: &'static str) -> Result<Arc<Vec<Vec<u8>>>> {
    disc.keep(name, || {
        let offsets = block_offsets(disc, blocks)?;
        let data = disc.read_from(blocks.file, 0)?;
        (0..blocks.count)
            .map(|index| Ok(lz_varint::decompress(&data[offsets[index] as usize..offsets[index + 1] as usize], 0)?.0))
            .collect()
    })
}

/// The program's table of glyphs for codes from 0x989F up: code, glyph. The program goes through it
/// from the start and takes the first entry with the code.
pub fn looked_up(disc: &Disc) -> Result<Arc<HashMap<u16, usize>>> {
    disc.keep("srwz-ps2: the glyphs looked up", || {
        let program = program_file(disc)?;
        let mut table = HashMap::new();
        let mut at = program_offset(LOOKUP_TABLE);
        loop {
            let (Some(code), Some(row), Some(column)) = (half(&program, at), program.get(at + 2), program.get(at + 3))
            else {
                return image_error("the program's table of glyphs runs past its end");
            };
            if code == 0 {
                return Ok(table);
            }
            let glyph = LOOKUP_ROW as i64 * i64::from(*row as i8) + i64::from(*column);
            // Six codes are there twice; the program takes the first.
            table.entry(code).or_insert(usize::try_from(glyph).unwrap_or(usize::MAX));
            at += 4;
        }
    })
}

// --- the characters

/// The row of NEC symbols that code page 932 adds at first byte 0x87: each second byte that has a
/// character, and the character.
const NEC_ROW: [(u8, char); 83] = {
    const CHARS: [char; 83] = [
        '①', '②', '③', '④', '⑤', '⑥', '⑦', '⑧', '⑨', '⑩', '⑪', '⑫', '⑬', '⑭', '⑮', '⑯', '⑰', '⑱', '⑲', '⑳', 'Ⅰ', 'Ⅱ',
        'Ⅲ', 'Ⅳ', 'Ⅴ', 'Ⅵ', 'Ⅶ', 'Ⅷ', 'Ⅸ', 'Ⅹ', '㍉', '㌔', '㌢', '㍍', '㌘', '㌧', '㌃', '㌶', '㍑', '㍗', '㌍', '㌦',
        '㌣', '㌫', '㍊', '㌻', '㎜', '㎝', '㎞', '㎎', '㎏', '㏄', '㎡', '㍻', '〝', '〟', '№', '㏍', '℡', '㊤', '㊥',
        '㊦', '㊧', '㊨', '㈱', '㈲', '㈹', '㍾', '㍽', '㍼', '≒', '≡', '∫', '∮', '∑', '√', '⊥', '∠', '∟', '⊿', '∵',
        '∩', '∪',
    ];
    let mut row = [(0u8, ' '); 83];
    let mut index = 0;
    let mut trail = 0x40u8;
    while index < 83 {
        // 0x5E and 0x76 to 0x7D and 0x7F have no character.
        if trail == 0x5E || (trail >= 0x76 && trail <= 0x7D) || trail == 0x7F {
            trail += 1;
            continue;
        }
        row[index] = (trail, CHARS[index]);
        index += 1;
        trail += 1;
    }
    row
};
const NEC_LEAD: u8 = 0x87;

/// The characters code page 932 reads differently from the JIS standard in the first row, and the
/// code each is: they are taken when a pack writes them, though a script shows the standard's.
const WINDOWS_READINGS: [(char, u16); 6] =
    [('～', 0x8160), ('∥', 0x8161), ('－', 0x817C), ('￠', 0x8191), ('￡', 0x8192), ('￢', 0x81CA)];

fn is_lead(byte: u8) -> bool {
    (0x81..=0x9F).contains(&byte) || (0xE0..=0xFC).contains(&byte)
}

fn is_trail(byte: u8) -> bool {
    (0x40..=0xFC).contains(&byte) && byte != 0x7F
}

/// The character a two-byte code stands for: JIS X 0208 as the rest of ventris reads it, and the
/// NEC row.
fn character(code: u16) -> Option<char> {
    let [lead, trail] = code.to_be_bytes();
    if lead == NEC_LEAD {
        return NEC_ROW.iter().find(|(known, _)| *known == trail).map(|(_, char)| *char);
    }
    shift_jis::character(lead, trail)
}

/// The code for each character: the first code that stands for it, as code page 932 writes it.
static CODES: LazyLock<HashMap<char, u16>> = LazyLock::new(|| {
    let mut codes = HashMap::new();
    for lead in (0x81..=0x9Fu8).chain(0xE0..=0xFC) {
        for trail in (0x40..=0xFCu8).filter(|trail| is_trail(*trail)) {
            let code = u16::from_be_bytes([lead, trail]);
            if let Some(char) = character(code) {
                codes.entry(char).or_insert(code);
            }
        }
    }
    for (char, code) in WINDOWS_READINGS {
        codes.entry(char).or_insert(code);
    }
    codes
});

/// The glyph a two-byte code draws, given the program's table for the codes from 0x989F up.
pub fn glyph_of(code: u16, looked_up: &HashMap<u16, usize>) -> Option<usize> {
    let [lead, trail] = code.to_be_bytes();
    if code >= FIRST_LOOKED_UP {
        return looked_up.get(&code).copied().filter(|glyph| *glyph < GLYPH_COUNT);
    }
    if !(FIRST_LEAD..=LAST_LINEAR_LEAD).contains(&lead) || trail < FIRST_TRAIL {
        return None;
    }
    let glyph = usize::from(lead - FIRST_LEAD) * GLYPHS_PER_LEAD + usize::from(trail - FIRST_TRAIL);
    (glyph < GLYPH_COUNT).then_some(glyph)
}

/// The code that selects a glyph by the game's formula.
pub fn code_at(index: usize) -> u32 {
    let lead = u32::from(FIRST_LEAD) + (index / GLYPHS_PER_LEAD) as u32;
    let trail = u32::from(FIRST_TRAIL) + (index % GLYPHS_PER_LEAD) as u32;
    lead << 8 | trail
}

// --- the codes

/// How a body of text writes a line break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breaks {
    /// The byte 0x0A.
    Byte,
    /// The two characters `\n`, as the battle lines do.
    Escaped,
}

const LINE: u8 = 0x0A;
const ESCAPED_LINE: &[u8] = b"\\n";
/// The names the stages' text fills in, written `$` and a letter.
const FILLS: [(u8, &str); 5] =
    [(b'n', "HERO"), (b'F', "HERO_FULL"), (b'f', "HERO_SHORT"), (b'l', "HERO_FAMILY"), (b'c', "SQUAD")];
const FILL_MARK: u8 = b'$';
/// The drawing loop's codes, 0x2E to 0x3D: the bytes after each, and the names of those known.
const FIRST_CODE: u8 = 0x2E;
const LAST_CODE: u8 = 0x3D;
const NAMED_CODES: [(u8, &str); 8] = [
    (0x32, "WIDTH"),
    (0x33, "HEIGHT"),
    (0x34, "ADVANCE"),
    (0x35, "LINE_STEP"),
    (0x3A, "GIVEN_NAME"),
    (0x3B, "FAMILY_NAME"),
    (0x3C, "SHORT_NAME"),
    (0x3D, "SQUAD_NAME"),
];

/// How many bytes follow a code of the drawing loop.
fn code_bytes(code: u8) -> usize {
    usize::from((0x30..=0x39).contains(&code))
}

fn code_name(code: u8) -> String {
    NAMED_CODES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| format!("{code:02X}"), |(_, name)| name.to_string())
}

/// The length of an icon `<14>`, `<-5>` or a code `[10]` at the start of `data`, if one is there.
fn bracketed(data: &[u8]) -> Option<usize> {
    let (close, rest) = match data.first()? {
        b'<' => (b'>', data[1..].strip_prefix(b"-").unwrap_or(&data[1..])),
        b'[' => (b']', &data[1..]),
        _ => return None,
    };
    let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
    (digits > 0 && rest.get(digits) == Some(&close)).then(|| data.len() - rest.len() + digits + 1)
}

/// One piece of a string as the game stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece<'a> {
    /// A two-byte code.
    Code(u16),
    Line,
    Fill(u8),
    /// A code of the drawing loop with its bytes, or a byte that is not part of anything.
    Byte(u8, &'a [u8]),
    Bracketed(&'a [u8]),
}

/// A string split into its pieces.
fn pieces(data: &[u8], breaks: Breaks) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        let rest = &data[at..];
        let (piece, length) = if is_lead(byte) && data.get(at + 1).is_some_and(|trail| is_trail(*trail)) {
            (Piece::Code(u16::from_be_bytes([byte, data[at + 1]])), 2)
        } else if breaks == Breaks::Byte && byte == LINE {
            (Piece::Line, 1)
        } else if breaks == Breaks::Escaped && rest.starts_with(ESCAPED_LINE) {
            (Piece::Line, 2)
        } else if byte == FILL_MARK && rest.get(1).is_some_and(|letter| FILLS.iter().any(|(fill, _)| fill == letter)) {
            (Piece::Fill(rest[1]), 2)
        } else if let Some(length) = bracketed(rest) {
            (Piece::Bracketed(&rest[..length]), length)
        } else if (FIRST_CODE..=LAST_CODE).contains(&byte) && rest.len() > code_bytes(byte) {
            (Piece::Byte(byte, &rest[1..=code_bytes(byte)]), 1 + code_bytes(byte))
        } else {
            (Piece::Byte(byte, &[]), 1)
        };
        out.push(piece);
        at += length;
    }
    out
}

/// Whether the bytes are text the drawing loop can draw: characters the font has, line breaks, the
/// names filled in, and the codes given, with at least one character.
fn reads_as_text(data: &[u8], codes: &[u8], looked_up: &HashMap<u16, usize>) -> bool {
    let pieces = pieces(data, Breaks::Byte);
    let fits = |piece: &Piece| match piece {
        Piece::Code(code) => glyph_of(*code, looked_up).is_some(),
        Piece::Line | Piece::Fill(_) => true,
        Piece::Byte(byte, bytes) => codes.contains(byte) && bytes.len() == code_bytes(*byte),
        Piece::Bracketed(_) => false,
    };
    pieces.iter().all(fits) && pieces.iter().any(|piece| matches!(piece, Piece::Code(_)))
}

/// One string as it is written in a script file.
pub fn decode(data: &[u8], breaks: Breaks, looked_up: &HashMap<u16, usize>) -> String {
    let mut out = String::new();
    for piece in pieces(data, breaks) {
        match piece {
            Piece::Code(code) => match character(code) {
                Some(char) if CODES.get(&char) == Some(&code) && glyph_of(code, looked_up).is_some() => out.push(char),
                _ => out.push_str(&format!("{{G:{code:04X}}}")),
            },
            Piece::Line => out.push('\n'),
            Piece::Fill(letter) => {
                let name = FILLS.iter().find(|(fill, _)| *fill == letter).map_or("", |(_, name)| name);
                out.push_str(&format!("{{{name}}}"));
            }
            // A code that takes no byte, by its name; one whose byte is missing, as the byte alone.
            Piece::Byte(byte, bytes)
                if bytes.is_empty() && (FIRST_CODE..=LAST_CODE).contains(&byte) && code_bytes(byte) == 0 =>
            {
                out.push_str(&format!("{{{}}}", code_name(byte)));
            }
            Piece::Byte(byte, []) => out.push_str(&format!("{{{byte:02X}}}")),
            Piece::Byte(byte, bytes) => out.push_str(&format!("{{{}:{}}}", code_name(byte), crate::text::hex(bytes))),
            Piece::Bracketed(bytes) => out.push_str(&format!("{{{}}}", String::from_utf8_lossy(bytes))),
        }
    }
    out
}

/// The bytes a tag stands for: `raw` is the tag as written, `value` what follows its colon.
fn encode_tag(raw: &str, name: &str, value: Option<&str>) -> Result<Vec<u8>> {
    let unknown = || text_error(format!("unknown tag {raw}"));
    if name.starts_with(['<', '[']) {
        let bytes = name.as_bytes();
        return if value.is_none() && bracketed(bytes) == Some(bytes.len()) { Ok(bytes.to_vec()) } else { unknown() };
    }
    if let Some((letter, _)) = FILLS.iter().find(|(_, known)| *known == name) {
        return if value.is_none() { Ok(vec![FILL_MARK, *letter]) } else { unknown() };
    }
    let Some(parameters) = from_hex(value.unwrap_or("")) else { return unknown() };
    if name == "G" {
        return match parameters[..] {
            [lead, trail] if is_lead(lead) && is_trail(trail) => Ok(parameters),
            _ => unknown(),
        };
    }
    let named = NAMED_CODES.iter().find(|(_, known)| *known == name).map(|(code, _)| *code);
    let numbered = (name.len() == 2).then(|| u8::from_str_radix(name, 16).ok()).flatten();
    let byte = match named.or(numbered) {
        Some(byte) if byte != 0 => byte,
        _ => return unknown(),
    };
    if value.is_none() && named.is_none() {
        return Ok(vec![byte]); // a byte on its own
    }
    if !(FIRST_CODE..=LAST_CODE).contains(&byte) {
        return unknown();
    }
    let count = code_bytes(byte);
    if parameters.len() != count || (count == 0 && value.is_some()) {
        return text_error(format!("{raw} needs {count} parameter bytes"));
    }
    Ok([&[byte], parameters.as_slice()].concat())
}

/// A string of a script file as the game stores it, without the zero byte that ends it.
pub fn encode(text: &str, breaks: Breaks, looked_up: &HashMap<u16, usize>) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for token in tokens(text) {
        match token {
            Token::Tag { raw, name, value } => out.extend(encode_tag(raw, name, value)?),
            Token::Char('\n') => match breaks {
                Breaks::Byte => out.push(LINE),
                Breaks::Escaped => out.extend_from_slice(ESCAPED_LINE),
            },
            Token::Char(char) => match CODES.get(&char).filter(|code| glyph_of(**code, looked_up).is_some()) {
                Some(code) => out.extend(code.to_be_bytes()),
                None => {
                    return text_error(format!(
                        "the game's font has no {}; a Latin letter or a digit is written full-width",
                        quoted(char.encode_utf8(&mut [0; 4]))
                    ));
                }
            },
        }
    }
    Ok(out)
}

// --- the stages

/// A stage's overlay: its name and its image as the game loads it.
pub struct Overlay {
    pub name: String,
    pub image: Vec<u8>,
}

/// Where each block of `STAGE.BIN` starts, and the file's end, from `HB.BIN`.
fn stage_offsets(disc: &Disc) -> Result<Vec<u32>> {
    let bounds = disc.read_from(STAGE_TABLE_BOUNDS, 0)?;
    let table = disc.read_from(STAGE_TABLE_FILE, 0)?;
    let start = word(&bounds, 4 * STAGE_TABLE_SEGMENT).map(|start| start as usize);
    let offsets: Option<Vec<u32>> =
        start.and_then(|start| (0..=STAGE_BLOCKS).map(|index| word(&table, start + 4 * index)).collect());
    match offsets {
        Some(offsets) if u64::from(offsets[STAGE_BLOCKS]) == disc.size(STAGES)? => Ok(offsets),
        _ => image_error("HB.BIN's table of the stages does not end with STAGE.BIN's size"),
    }
}

/// Every stage's overlay, read once for an image.
pub fn overlays(disc: &Disc) -> Result<Arc<Vec<Overlay>>> {
    disc.keep("srwz-ps2: the stages", || {
        let offsets = stage_offsets(disc)?;
        let data = disc.read_from(STAGES, 0)?;
        let mut found = Vec::new();
        for index in 0..STAGE_BLOCKS {
            let block = lz_varint::decompress(&data[offsets[index] as usize..offsets[index + 1] as usize], 0)?.0;
            let entry = pack_entry(&block, 0, &format!("stage block {index}"))?;
            let image = block[entry].to_vec();
            let name = image.get(OVERLAY_NAME..OVERLAY_NAME + OVERLAY_NAME_SIZE).map(field_text).unwrap_or_default();
            let name = String::from_utf8_lossy(name);
            let Some(name) =
                name.strip_suffix(".bin").filter(|name| crate::script::parse_name(&format!("{name}.0")).is_ok())
            else {
                return image_error(format!("stage block {index} is not an overlay with a name"));
            };
            found.push(Overlay { name: name.to_string(), image });
        }
        Ok(found)
    })
}

/// The strings of a stage's overlay: its lines and choices, and its captions, conditions and
/// glossary entries, each under its offset in the overlay.
pub fn stage_strings(overlay: &Overlay, looked_up: &HashMap<u16, usize>) -> (Numbered, Numbered) {
    let image = &overlay.image;
    let inside = |address: u32| {
        address.checked_sub(STAGE_BASE).map(|offset| offset as usize).filter(|offset| *offset < image.len())
    };
    let text_at = |address: u32| -> Option<(u32, Vec<u8>)> {
        let offset = inside(address)?;
        if offset < OVERLAY_HEADER || image[offset - 1] != 0 {
            return None;
        }
        let text = zero_ended(image, offset)?;
        reads_as_text(text, &[0x3A, 0x3B, 0x3C, 0x3D], looked_up).then(|| (offset as u32, text.to_vec()))
    };
    let at = |offset: usize| word(image, offset).unwrap_or(0);
    let (mut lines, mut info) = (Numbered::new(), Numbered::new());
    let mut links = BTreeSet::new();
    let code_end = (at(OVERLAY_CODE_SIZE) as usize).next_multiple_of(RECORD_ALIGNMENT).max(OVERLAY_HEADER);
    for record in (code_end..image.len().saturating_sub(RECORD_SIZE - 1)).step_by(RECORD_ALIGNMENT) {
        let kind = at(record);
        let [first, second, third] = [at(record + 4), at(record + 8), at(record + 12)];
        let [after, link, last] = [at(record + 0x14), at(record + 0x18), at(record + 0x1C)];
        if last != 0 {
            continue;
        }
        let a_line = || first < LINE_EXPRESSIONS && (1..LINE_WINDOWS).contains(&second) && third < LINE_PORTRAITS;
        let a_link = |link: u32| {
            let list = inside(link).map(|offset| (at(offset), at(offset + 4)));
            list.is_some_and(|(list, count)| inside(list).is_some() && (1..=MOST_LINKS).contains(&count))
        };
        let Some((offset, text)) = text_at(at(record + RECORD_TEXT)) else { continue };
        if LINE_KINDS.contains(&kind) && after == 0 && a_line() && (link == 0 || a_link(link)) {
            lines.insert(offset, text);
            if link != 0 {
                links.insert(link);
            }
        } else if CHOICE_KINDS.contains(&kind) && (1..=MOST_CHOICES).contains(&after) && link == 0 && a_line() {
            lines.insert(offset, text);
        } else if CAPTION_KINDS.contains(&kind) && after == 0 && link == 0 && second == 0 && third == 0 {
            info.insert(offset, text);
        }
    }
    // The glossary entries the lines link to.
    for link in links {
        let Some(offset) = inside(link) else { continue };
        let (list, count) = (at(offset), at(offset + 4));
        for index in 0..count as usize {
            let Some(entry) = inside(list).and_then(|list| inside(at(list + 4 * index))) else { continue };
            let fields: Option<Vec<(u32, Vec<u8>)>> =
                (0..GLOSSARY_FIELDS).map(|field| text_at(at(entry + 4 * field))).collect();
            info.extend(fields.unwrap_or_default());
        }
    }
    // The conditions, from what the stage's code stores.
    let stored = stores(image, code_end);
    let slots: Option<Vec<usize>> =
        CONDITION_SLOTS.iter().map(|slot| stored.get(slot).copied().and_then(inside)).collect();
    if let Some(slots) = slots {
        for pair in slots.windows(2) {
            let mut place = pair[0];
            while place < pair[1] && at(place) != 0 {
                info.extend(text_at(at(place)));
                place += 4;
            }
        }
    }
    // What is a line is not also something else.
    info.retain(|offset, _| !lines.contains_key(offset));
    (lines, info)
}

/// What the code of an overlay stores, where its address is built with `lui` and `addiu`: the
/// address written to, and the value, where each is known.
fn stores(image: &[u8], code_end: usize) -> HashMap<u32, u32> {
    const SW: u32 = 0x2B;
    let mut known: [Option<u32>; 32] = [None; 32];
    let mut found = HashMap::new();
    for at in (OVERLAY_HEADER..code_end.min(image.len())).step_by(4) {
        let Some(instruction) = word(image, at) else { break };
        let (operation, source, target) =
            (instruction >> 26, (instruction >> 21 & 31) as usize, (instruction >> 16 & 31) as usize);
        let low = instruction & 0xFFFF;
        let signed = low as u16 as i16 as i32 as u32;
        match operation {
            LUI => known[target] = Some(low << 16),
            ADDIU if source == 0 => known[target] = Some(signed),
            ADDIU => known[target] = known[source].map(|high| high.wrapping_add(signed)),
            SW => {
                if let (Some(base), value) = (known[source], if target == 0 { Some(0) } else { known[target] }) {
                    found.extend(value.map(|value| (base.wrapping_add(signed), value)));
                }
            }
            _ if instruction == RETURN => known = [None; 32],
            _ => {}
        }
        known[0] = Some(0);
    }
    found
}

const LUI: u32 = 0x0F;
const ADDIU: u32 = 0x09;
const ORI: u32 = 0x0D;
const RETURN: u32 = 0x03E0_0008;

/// The intermission's summaries and button hints: two lists of addresses ending with a zero.
fn flow_info(overlay: &Overlay, looked_up: &HashMap<u16, usize>) -> Numbered {
    let image = &overlay.image;
    let mut found = Numbered::new();
    for start in [FLOW_SUMMARIES, FLOW_HINTS] {
        let mut place = start;
        while let Some(address) = word(image, place).filter(|address| *address != 0) {
            let offset =
                address.checked_sub(STAGE_BASE).map(|offset| offset as usize).filter(|offset| *offset < image.len());
            if let Some(offset) = offset.filter(|offset| *offset > OVERLAY_HEADER && image[offset - 1] == 0)
                && let Some(text) = zero_ended(image, offset).filter(|text| reads_as_text(text, &[], looked_up))
            {
                found.insert(offset as u32, text.to_vec());
            }
            place += 4;
        }
    }
    found
}

// --- the battle lines

/// The strings of one segment of the battle lines, each under its offset in the segment.
pub fn battle_strings(segment: &[u8]) -> Option<Numbered> {
    let (magic, small, situations, records) =
        (half(segment, 0)?, half(segment, 2)?, half(segment, 4)?, half(segment, 6)?);
    if magic != BATTLE_MAGIC {
        return None;
    }
    let (small, situations, records) = (usize::from(small), usize::from(situations), usize::from(records));
    let start = BATTLE_FIRST_TABLE + 8 * situations + 8 * (small & 0xFF) + 4 * (small >> 8);
    let text = start + 8 * records;
    let mut found = Numbered::new();
    for record in 0..records {
        let offset = text.checked_add(word(segment, start + 8 * record + 4)? as usize)?;
        if offset > text && segment.get(offset - 1) != Some(&0) {
            return None;
        }
        found.insert(offset as u32, zero_ended(segment, offset)?.to_vec());
    }
    Some(found)
}

/// Every segment of the battle lines that has records, by number.
fn battle_segments(disc: &Disc) -> Result<BTreeMap<usize, Numbered>> {
    let data = disc.read_from(BATTLE_LINES, 0)?;
    let table = disc.read_from(BATTLE_SEGMENTS, 0)?;
    let mut offsets: Vec<usize> =
        table.as_chunks::<4>().0.iter().map(|bytes| u32::from_le_bytes(*bytes) as usize).collect();
    offsets.push(data.len());
    let mut found = BTreeMap::new();
    for (index, pair) in offsets.windows(2).enumerate() {
        let Some(segment) = data.get(pair[0]..pair[1]).filter(|segment| segment.len() >= 8) else { continue };
        match battle_strings(segment) {
            Some(strings) if !strings.is_empty() => {
                found.insert(index, strings);
            }
            Some(_) => {}
            None => {
                return image_error(format!(
                    "segment {index} of SRVC.BIN is not laid out as a segment of battle lines"
                ));
            }
        }
    }
    Ok(found)
}

// --- the data lists and the program

/// The strings of the data lists, under their offsets: what follows the tables, each string pointed
/// at by a 32-bit word of the tables.
pub fn data_list_strings(lists: &[u8]) -> Numbered {
    let tables = &lists[..DATA_LISTS_STRINGS.min(lists.len())];
    let pointed: BTreeSet<u32> =
        (0..tables.len() / 4).filter_map(|index| word(tables, 4 * index)?.checked_sub(DATA_LISTS_BASE)).collect();
    let mut found = Numbered::new();
    let mut at = DATA_LISTS_STRINGS;
    while at < lists.len() {
        if lists[at] == 0 {
            at += 1;
            continue;
        }
        let text = zero_ended(lists, at).unwrap_or(&lists[at..]);
        if pointed.contains(&(at as u32)) {
            found.insert(at as u32, text.to_vec());
        }
        at += text.len();
    }
    found
}

/// The addresses the program refers to: what its 32-bit words hold, and what its code builds with a
/// `lui` and an `addiu` or `ori`.
fn program_targets(program: &[u8]) -> BTreeSet<u32> {
    let end = program_offset(PROGRAM_END).min(program.len());
    let image = &program[PROGRAM_FILE_START..end];
    let mut found: BTreeSet<u32> = (0..image.len() / 4).filter_map(|index| word(image, 4 * index)).collect();
    let code_end = program_offset(PROGRAM_CODE_END) - PROGRAM_FILE_START;
    let mut highs: [Option<u32>; 32] = [None; 32];
    for at in (0..code_end.min(image.len())).step_by(4) {
        let Some(instruction) = word(image, at) else { break };
        let (operation, source, target) =
            (instruction >> 26, (instruction >> 21 & 31) as usize, (instruction >> 16 & 31) as usize);
        let low = instruction & 0xFFFF;
        match operation {
            LUI => highs[target] = Some(low << 16),
            ADDIU => found.extend(highs[source].map(|high| high.wrapping_add(low as u16 as i16 as i32 as u32))),
            ORI => found.extend(highs[source].map(|high| high | low)),
            _ if instruction == RETURN => highs = [None; 32],
            _ => {}
        }
    }
    found
}

/// The program's own strings, under their offsets in its file.
pub fn program_strings(program: &[u8], looked_up: &HashMap<u16, usize>) -> Numbered {
    let mut found = Numbered::new();
    for address in program_targets(program) {
        if !(PROGRAM_CODE_END..PROGRAM_END).contains(&address) || address % PROGRAM_ALIGNMENT != 0 {
            continue;
        }
        let offset = program_offset(address);
        if program.get(offset - 1) != Some(&0) || offset == NAME_ENTRY_LETTERS {
            continue;
        }
        if let Some(text) = zero_ended(program, offset).filter(|text| reads_as_text(text, &PROGRAM_CODES, looked_up)) {
            found.insert(offset as u32, text.to_vec());
        }
    }
    found
}

// --- the library

/// A library block's entry with the mask taken off.
fn unmask(data: &[u8]) -> Vec<u8> {
    data.iter().map(|byte| if *byte == 0 || *byte == LIBRARY_MASK { *byte } else { byte ^ LIBRARY_MASK }).collect()
}

/// The text chunks of one library entry: tag, and the text before its first zero byte.
pub fn library_chunks(block: &[u8]) -> Option<Vec<([u8; 4], Vec<u8>)>> {
    let entry = pack_entries(block)?.first()?.clone();
    let data = unmask(&block[entry]);
    if !data.starts_with(LIBRARY_MAGIC) {
        return None;
    }
    let mut found = Vec::new();
    let mut at = LIBRARY_HEAD;
    while at + 8 <= data.len() {
        let tag: [u8; 4] = data[at..at + 4].try_into().ok()?;
        if !tag.iter().all(u8::is_ascii_alphanumeric) {
            break;
        }
        let size = word(&data, at + 4)? as usize;
        if LIBRARY_VALUES.contains(&&tag[..]) {
            at += 8;
            continue;
        }
        let body = data.get(at + 8..at + 8 + size)?;
        found.push((tag, field_text(body).to_vec()));
        at += 8 + size;
    }
    Some(found)
}

/// The strings of a library file, numbered by entry and field.
fn library_strings(disc: &Disc, blocks: &Blocks, fields: &[LibraryField], name: &'static str) -> Result<Numbered> {
    let mut found = Numbered::new();
    for (index, block) in all_blocks(disc, blocks, name)?.iter().enumerate() {
        let Some(chunks) = library_chunks(block) else {
            return image_error(format!("block {index} of {} is not a library entry", blocks.file));
        };
        for (tag, text) in chunks {
            if let Some((_, field, _)) = fields.iter().find(|(known, _, _)| **known == tag) {
                found.insert(index as u32 * LIBRARY_STRIDE + field, text);
            }
        }
    }
    Ok(found)
}

// --- help, summaries, credits, prologue, places, squads

/// The strings of a help page (an entry of a pack), under their offsets in `block`.
pub fn help_strings(block: &[u8], entry: std::ops::Range<usize>) -> Option<Numbered> {
    let page = block.get(entry.clone())?;
    let end = 2 + usize::from(half(page, 0)?);
    let mut found = Numbered::new();
    let mut at = 2;
    while at < end {
        let start = at + HELP_ITEM_HEADER;
        let text = zero_ended(page, start)?;
        found.insert((entry.start + start) as u32, text.to_vec());
        at = start + text.len() + 1;
    }
    (at == end).then_some(found)
}

/// The summaries: each of three lines, as the game stores them, joined by zero bytes.
fn summaries(block: &[u8]) -> Result<Vec<Vec<u8>>> {
    let entry = pack_entry(block, 0, "block 0 of HSFC.BIN")?;
    let summary = SUMMARY_LINE * SUMMARY_LINES;
    let joined = |summary: &[u8]| {
        let lines: Vec<&[u8]> = summary.as_chunks::<SUMMARY_LINE>().0.iter().map(|line| field_text(line)).collect();
        lines.join(&0)
    };
    Ok(block[entry].chunks_exact(summary).map(joined).collect())
}

/// A summary as a script writes it: its lines, each decoded, with line breaks between them, and the
/// empty lines at its end left out.
fn decode_summary(data: &[u8], looked_up: &HashMap<u16, usize>) -> String {
    let lines: Vec<String> = data.split(|byte| *byte == 0).map(|line| decode(line, Breaks::Byte, looked_up)).collect();
    lines.join("\n").trim_end_matches('\n').to_string()
}

fn encode_summary(text: &str, looked_up: &HashMap<u16, usize>) -> Result<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> =
        text.split('\n').map(|line| encode(line, Breaks::Byte, looked_up)).collect::<Result<_>>()?;
    if lines.len() > SUMMARY_LINES {
        return text_error(format!("a summary has {SUMMARY_LINES} lines at most, and this one has {}", lines.len()));
    }
    lines.resize(SUMMARY_LINES, Vec::new());
    Ok(lines.join(&0))
}

/// The staff credits, under their offsets in the block.
fn credits(block: &[u8]) -> Result<Numbered> {
    let table = pack_entry(block, CREDITS_TABLE, "block 2 of HSFC.BIN")?;
    let text = pack_entry(block, CREDITS_TEXT, "block 2 of HSFC.BIN")?;
    let mut found = Numbered::new();
    for place in block[table].as_chunks::<4>().0 {
        let offset = text.start + u32::from_le_bytes(*place) as usize;
        if offset >= text.end || (offset > text.start && block[offset - 1] != 0) {
            return image_error("a credit of HSFC.BIN does not start a string");
        }
        found
            .insert(offset as u32, zero_ended(&block[..text.end], offset).unwrap_or(&block[offset..text.end]).to_vec());
    }
    Ok(found)
}

/// The prologue's `rawt` chunks of one block, in order: the tag, a size, then that many bytes of
/// text, with no zero byte after them, and the next chunk's tag.
pub fn prologue_strings(block: &[u8], looked_up: &HashMap<u16, usize>) -> Vec<Vec<u8>> {
    let entry = pack_entries(block).and_then(|entries| entries.first().cloned()).unwrap_or(0..0);
    let data = &block[entry];
    let is_tag = |at: usize| data.get(at..at + 4).is_some_and(|tag| tag.iter().all(u8::is_ascii_lowercase));
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(place) = data.get(at..).and_then(|rest| rest.windows(4).position(|tag| tag == PROLOGUE_TAG)) {
        let start = at + place;
        at = start + 1;
        let Some(size) = word(data, start + 4).map(|size| size as usize) else { break };
        let Some(body) = data.get(start + 8..start + 8 + size) else { continue };
        let end = start + 8 + size;
        if reads_as_text(body, &[], looked_up) && (end == data.len() || is_tag(end)) {
            found.push(body.to_vec());
            at = end;
        }
    }
    found
}

/// The squads' names, by record.
fn squads(block: &[u8]) -> Result<Vec<Vec<u8>>> {
    let entry = pack_entry(block, 0, "block 4 of NISVDATA.BIN")?;
    let records = block[entry].as_chunks::<SQUAD_RECORD>().0;
    Ok(records.iter().map(|record| field_text(&record[SQUAD_NAME]).to_vec()).collect())
}

// --- the whole script

type Numbered = BTreeMap<u32, Vec<u8>>;

/// How a section's strings are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Text(Breaks),
    Summary,
}

/// Every string of the game by section, as the game stores it, and how each section's are written.
pub struct Strings {
    sections: BTreeMap<String, (Form, Numbered)>,
}

fn strings(disc: &Disc) -> Result<Arc<Strings>> {
    disc.keep("srwz-ps2: the strings", || {
        let looked_up = looked_up(disc)?;
        let mut sections = BTreeMap::new();
        let mut add = |name: String, form: Form, strings: Numbered| {
            if !strings.is_empty() {
                sections.insert(name, (form, strings));
            }
        };
        let text = Form::Text(Breaks::Byte);
        for overlay in overlays(disc)?.iter() {
            let (lines, mut info) = stage_strings(overlay, &looked_up);
            if overlay.name == FLOW {
                info.extend(flow_info(overlay, &looked_up));
            }
            add(overlay.name.clone(), text, lines);
            add(format!("{}{INFO}", overlay.name), text, info);
        }
        for (index, strings) in battle_segments(disc)? {
            add(format!("{BATTLE_PREFIX}{index:03}"), Form::Text(Breaks::Escaped), strings);
        }
        let lists = disc.read_from(DATA_LISTS, 0)?;
        add(DATA_LISTS_SECTION.to_string(), text, data_list_strings(&lz_varint::decompress(&lists, 0)?.0));
        add(PROGRAM_SECTION.to_string(), text, program_strings(&program_file(disc)?, &looked_up));
        let library =
            [(&ROBOTS, "srwz-ps2: robots"), (&CHARACTERS, "srwz-ps2: characters"), (&KEYWORDS, "srwz-ps2: keywords")];
        for ((section, fields), (blocks, name)) in LIBRARY_FIELDS.iter().zip(library) {
            add(section.to_string(), text, library_strings(disc, blocks, fields, name)?);
        }
        let nisvdata = all_blocks(disc, &NISVDATA, "srwz-ps2: NISVDATA")?;
        add(SQUADS_SECTION.to_string(), text, numbered(squads(&nisvdata[SQUAD_BLOCK])?));
        for (section, block, first) in [(SR_POINT_SECTION, SR_POINT_BLOCK, 0), (TUTORIAL_SECTION, TUTORIAL_BLOCK, 1)] {
            let block = &nisvdata[block];
            let mut found = Numbered::new();
            for (index, entry) in pack_entries(block).unwrap_or_default().into_iter().enumerate().skip(first) {
                match help_strings(block, entry) {
                    Some(strings) => found.extend(strings),
                    None => return image_error(format!("page {index} of {section} is not laid out as a help page")),
                }
            }
            add(section.to_string(), text, found);
        }
        let hsfc = all_blocks(disc, &HSFC, "srwz-ps2: HSFC")?;
        add(SUMMARY_SECTION.to_string(), Form::Summary, numbered(summaries(&hsfc[SUMMARY_BLOCK])?));
        add(CREDITS_SECTION.to_string(), text, credits(&hsfc[CREDITS_BLOCK])?);
        let mut prologue = Numbered::new();
        for (index, block) in all_blocks(disc, &PROLOGUE, "srwz-ps2: prologue")?.iter().enumerate() {
            for (place, text) in prologue_strings(block, &looked_up).into_iter().enumerate() {
                prologue.insert(index as u32 * PROLOGUE_STRIDE + place as u32, text);
            }
        }
        add(PROLOGUE_SECTION.to_string(), text, prologue);
        let places = disc.read_from(PLACES, 0)?;
        add(
            PLACES_SECTION.to_string(),
            text,
            numbered(places.as_chunks::<PLACE_FIELD>().0.iter().map(|field| field_text(field).to_vec()).collect()),
        );
        Ok(Strings { sections })
    })
}

/// Strings numbered from 0 up.
fn numbered(strings: Vec<Vec<u8>>) -> Numbered {
    strings.into_iter().enumerate().map(|(number, data)| (number as u32, data)).collect()
}

fn decode_as(data: &[u8], form: Form, looked_up: &HashMap<u16, usize>) -> String {
    match form {
        Form::Text(breaks) => decode(data, breaks, looked_up),
        Form::Summary => decode_summary(data, looked_up),
    }
}

fn encode_as(text: &str, form: Form, looked_up: &HashMap<u16, usize>) -> Result<Vec<u8>> {
    match form {
        Form::Text(breaks) => encode(text, breaks, looked_up),
        Form::Summary => encode_summary(text, looked_up),
    }
}

/// The game's whole script.
pub fn extract(disc: &Disc) -> Result<Script> {
    let looked_up = looked_up(disc)?;
    Ok(strings(disc)?
        .sections
        .iter()
        .map(|(section, (form, strings))| {
            let texts = strings.iter().map(|(number, data)| (*number, decode_as(data, *form, &looked_up))).collect();
            (section.clone(), texts)
        })
        .collect())
}

/// Check that an extracted script encodes back to exactly the game's bytes.
pub fn verify_script(disc: &Disc, texts: &Script) -> Result<()> {
    let looked_up = looked_up(disc)?;
    let strings = strings(disc)?;
    let forms = [Form::Text(Breaks::Byte), Form::Text(Breaks::Escaped), Form::Summary];
    for section in texts.keys() {
        if !strings.sections.contains_key(section) {
            return text_error(format!("{section} is not a part of this game's script"));
        }
    }
    for form in forms {
        let of_form = |section: &String| strings.sections.get(section).is_some_and(|(known, _)| *known == form);
        let script: Script =
            texts.iter().filter(|(section, _)| of_form(section)).map(|(s, t)| (s.clone(), t.clone())).collect();
        let original: BTreeMap<String, Numbered> = strings
            .sections
            .iter()
            .filter(|(_, (known, _))| *known == form)
            .map(|(s, (_, n))| (s.clone(), n.clone()))
            .collect();
        script::verify_by_place(&script, &original, |text| encode_as(text, form, &looked_up))?;
    }
    Ok(())
}

// --- the font

fn font(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("srwz-ps2: the font", || read_block(disc, &FONT_FILE, FONT_SEGMENT))
}

/// A glyph as 24 rows of 24 pixels, each 0 to 15.
pub fn glyph(disc: &Disc, index: usize) -> Result<Vec<Vec<u8>>> {
    let font = font(disc)?;
    let Some(data) = font.get(index * GLYPH_BYTES..(index + 1) * GLYPH_BYTES) else {
        return text_error(format!("the game's font has no glyph {index}"));
    };
    let rows = data.as_chunks::<{ GLYPH_SIZE / 2 }>().0;
    Ok(rows.iter().map(|row| row.iter().flat_map(|byte| [byte & 0x0F, byte >> 4]).collect()).collect())
}

// --- what the engine sees

/// The name of a stage's overlay: `stg_` and its number, or the intermission's.
fn is_overlay(name: &str) -> bool {
    name == FLOW
        || name
            .strip_prefix("stg_")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn is_stage_lines(section: &str) -> bool {
    is_overlay(section)
}

fn is_stage_info(section: &str) -> bool {
    section.strip_suffix(INFO).is_some_and(is_overlay)
}

fn is_battle(section: &str) -> bool {
    section
        .strip_prefix(BATTLE_PREFIX)
        .is_some_and(|number| number.len() == 3 && number.bytes().all(|b| b.is_ascii_digit()))
}

fn is_data_lists(section: &str) -> bool {
    section == DATA_LISTS_SECTION
}

fn is_program(section: &str) -> bool {
    section == PROGRAM_SECTION
}

fn is_library(section: &str) -> bool {
    LIBRARY_FIELDS.iter().any(|(name, _)| *name == section)
}

fn is_help(section: &str) -> bool {
    section == SR_POINT_SECTION || section == TUTORIAL_SECTION
}

fn is_summary(section: &str) -> bool {
    section == SUMMARY_SECTION
}

fn is_credits(section: &str) -> bool {
    section == CREDITS_SECTION
}

fn is_prologue(section: &str) -> bool {
    section == PROLOGUE_SECTION
}

fn is_places(section: &str) -> bool {
    section == PLACES_SECTION
}

fn is_squads(section: &str) -> bool {
    section == SQUADS_SECTION
}

static KINDS: [TextKind; 12] = [
    TextKind {
        numbered_by_place: true,
        limits: "a line of the dialogue box holds about 21 wide characters, and the box three lines under the \
                 speaker's name; the game neither wraps a longer line nor cuts it, and it runs over the box's edge",
        ..TextKind::new("dialogue", is_stage_lines)
    },
    TextKind { numbered_by_place: true, ..TextKind::new("captions", is_stage_info) },
    TextKind {
        numbered_by_place: true,
        limits: "a battle line is copied into a field of 96 bytes, so it takes at most 95: 47 wide characters, a line \
                 break taking two bytes; the band shows two lines of about 21 wide characters",
        ..TextKind::new("battle", is_battle)
    },
    TextKind { numbered_by_place: true, ..TextKind::new("lists", is_data_lists) },
    TextKind { numbered_by_place: true, ..TextKind::new("menus", is_program) },
    TextKind { numbered_by_place: true, ..TextKind::new("library", is_library) },
    TextKind {
        numbered_by_place: true,
        limits: "a string is one piece of a page, drawn where the game places it, so a sentence often runs over \
                 several strings, each a line of the page",
        ..TextKind::new("help", is_help)
    },
    TextKind {
        limits: "a summary has three lines, each in a field of 50 bytes: 24 wide characters",
        ..TextKind::new("summaries", is_summary)
    },
    TextKind { numbered_by_place: true, ..TextKind::new("credits", is_credits) },
    TextKind { numbered_by_place: true, ..TextKind::new("prologue", is_prologue) },
    TextKind { limits: "a map's name is in a field of 256 bytes", ..TextKind::new("places", is_places) },
    TextKind {
        limits: "a squad's name is in a field of 28 bytes: 13 wide characters",
        ..TextKind::new("squads", is_squads)
    },
];

/// The adapter itself: what the rest of this module knows, on the contract every game follows.
pub struct Srwz;

pub static ADAPTER: Srwz = Srwz;

impl Adapter for Srwz {
    fn id(&self) -> &'static str {
        "srwz-ps2"
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn platform(&self) -> &'static dyn Platform {
        &ps2::PLATFORM
    }

    fn images(&self) -> &'static [Release] {
        // Redump's "Super Robot Taisen Z (Japan, Korea)", disc 4932, whose record gives this SHA-1:
        // the print sold in Korea, SCKA-20131, is the same image.
        &[Release {
            sha1: ROM_SHA1,
            name: "Japan, version 1.04",
            size: 3_758_358_528,
            label: "SLPS-25887",
            version: "1.04",
            // redump.org does not answer over https, so its address is http.
            catalogue: Some(CatalogueEntry {
                catalogue: &ps2::REDUMP,
                name: "Super Robot Taisen Z (Japan, Korea)",
                page: Some("http://redump.org/disc/4932/"),
            }),
        }]
    }

    fn refuses(&self) -> &'static [Refused] {
        // The reissue is a revised build: its program and files are not this one's, so none of the
        // offsets here hold. It is not in Redump.
        &[Refused {
            label: "SLPS-73270",
            version: None,
            about: "the PlayStation 2 the Best reissue, a revised build",
            patched: false,
        }]
    }

    fn source_language(&self) -> &'static str {
        SOURCE_LANGUAGE
    }

    fn kinds(&self) -> &'static [TextKind] {
        &KINDS
    }

    fn extract(&self, image: &Image) -> Result<Pack> {
        Ok(Pack::of(extract(image.disc()?)?))
    }

    fn note(&self, section: &str, number: Option<u32>) -> Option<String> {
        if let Some((_, fields)) = LIBRARY_FIELDS.iter().find(|(name, _)| *name == section) {
            return match number {
                None => Some(format!(
                    "Each entry's strings are numbered by the entry times {LIBRARY_STRIDE}, plus the field: {}.",
                    fields.iter().map(|(_, field, about)| format!("{field} {about}")).collect::<Vec<_>>().join(", ")
                )),
                Some(number) => fields
                    .iter()
                    .find(|(_, field, _)| *field == number % LIBRARY_STRIDE)
                    .map(|(_, _, about)| format!("Entry {}: {about}.", number / LIBRARY_STRIDE)),
            };
        }
        if number.is_some() {
            return None;
        }
        let note = if is_stage_info(section) && section.starts_with(FLOW) {
            "The intermission's summary of each stage, and its three button hints."
        } else if is_stage_info(section) {
            "The stage's captions of places, its conditions of victory, defeat and SR point, and the glossary \
             entries its lines link to: for each, the term, its series and two descriptions."
        } else if is_summary(section) {
            "The summary of each stage shown when a game is loaded: three lines, each of 24 wide characters at most."
        } else {
            return None;
        };
        Some(note.to_string())
    }

    fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
        verify_script(image.disc()?, texts)
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
        glyph(image.disc()?, index)
    }

    fn code_at(&self, index: usize) -> u32 {
        code_at(index)
    }

    /// A two-byte code the font cannot draw, or that is no character of code page 932 written the
    /// game's way, is written `{G:xxxx}`, four hexadecimal digits in capitals.
    fn unmapped(&self, text: &str) -> Vec<String> {
        let digit = |byte: &u8| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte);
        text.match_indices("{G:")
            .filter_map(|(at, _)| {
                let code = text.as_bytes().get(at + 3..at + 8)?;
                (code[..4].iter().all(digit) && code[4] == b'}').then(|| text[at + 3..at + 7].to_string())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
