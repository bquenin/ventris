//! Tales of Phantasia (Super Famicom, Japan, 1995).
//!
//! Script layout
//!
//! ```text
//! 0x280000   table of 100 three-byte CPU addresses, one per block
//! blocks     compressed with `codecs::lzss_rle`; they come in pairs, one pair per game area:
//!            even index = event data, odd index = the dialogue strings of that area
//! ```
//!
//! A dialogue block is a table of 16-bit offsets followed by the strings it points to, each ending
//! in FF FF.
//!
//! Text encoding
//!
//! ```text
//! 00              line break
//! 01              page break (wait for a button press)
//! 02 xx           name of party member xx
//! 03 xx           text speed
//! 04 xx           text colour
//! 05              space
//! 06-7F           one glyph: digits, hiragana, katakana, punctuation
//! low page        one glyph, low in 80-FF and page from 01 up: kanji and the remaining kana
//! 81 80 xx xx     number read from memory
//! 86 80 xx xx     string read from memory
//! ```
//!
//! Font
//!
//! ```text
//! 0x300000   16x16 glyphs, one bit per pixel, 32 bytes each: 16 rows of the left half, then 16 of
//!            the right. Glyph 0 belongs to code 05; a two-byte code continues the sequence after 7F.
//! ```
//!
//! Captions
//!
//! The game has a second routine that draws text in the same font, at $C8:EE01, and it reads a
//! pool of its own: what a battle says ("Not enough TP"), the names of skills as they are written
//! in kanji, the descriptions of items, and messages of the menu screens.
//!
//! ```text
//! 0x2F997C   622 strings back to back, each a run of 16-bit values ending in $FFFF
//! $000D           line break
//! $000A xxxx      a phrase that many strings share, by its number ($EA60 to $EA8E)
//! other           a glyph of the font, plus $10
//! ```
//!
//! A caption is named by a number, and two lists that follow the pool turn the number into a place:
//!
//! ```text
//! 0x2FE5DA   650 caption numbers, 16 bits each, ending in zero
//! 0x2FEAF0   for each of them, the offset of its caption from the start of the pool
//! ```
//!
//! Every offset is the start of a string, and 620 of the 622 strings are named. Four routines search
//! the lists: $C2:B54B gives the offset, $C9:D837 gives the address, and $C9:D045 and the one around
//! $C9:D415 go on to read the caption. A caller of $C2:B54B at $C2:B25B counts a caption's values
//! and places it in a field of ten, so the screens measure captions in glyphs.
//!
//! The pool's own address is written into the code in five places: $C2:B261 as a 24-bit address,
//! $C8:EE03 as a pointer's low word with its bank at $C8:EE0A, and $C9:D06C, $C9:D429 and $C9:D858
//! as a 16-bit address with the data bank set to $EF. The one caller of $C9:D837, at $C1:D31F, makes
//! a pointer of the address it is given and bank $EF. (The same bytes at $C4:572F, $C4:57B4 and
//! $C4:99A2 are something else.)
//!
//! A pack that changes a caption gets a new pool, in a bank of its own in the expanded area, with a
//! copy of the two lists at the addresses they have in bank $EF, since the routines read all three
//! through one data bank. No code is added: the numbers above are changed, and the data bank the
//! four routines set. Bank $EF keeps the game's pool and lists, which nothing reads any more.
//!
//! The captions' cells are 16 dots wide, and a line of the largest box holds about 15. A letter to
//! a cell would leave a translation a third of the room the game's own text has, so narrow
//! characters go two to a cell: every pair of letters that the captions use is a glyph of its own,
//! drawn from Unifont, whose narrow characters are 8 dots wide. The glyphs are drawn by two
//! routines ($C8:E6C3 and $C9:D4CB), which find a glyph at its number times 32 in bank $F0, so a
//! font holds 2,048 and the game uses 1,601. A changed pool gets a copy of the font in the
//! expanded area, and the routines are given its bank (at $C8:E6D7 and $C8:E784 for a glyph's two
//! halves, and $C9:D4D2). A third routine reads the font the same way, at $C8:E999, but it draws
//! dialogue shown as sprites and not captions, and has a font of its own (see `SPRITE_FONT`): a
//! caption's glyph numbers and dialogue's are not the same glyphs. The new glyphs take the 447 numbers after the
//! game's, and then the numbers of kanji that no caption of the built game uses; glyphs that are
//! not kanji keep theirs, since names and numbers are drawn by the same routines. This rests on
//! one thing that has not been proved: that whatever these routines draw in kanji comes from the
//! caption pool.
//!
//! The same font draws menu strings and names: the name of the item being described, the name
//! being typed on the naming screen. Two places, at $C9:D77D and $C9:D7C6, turn each byte of the
//! menu encoding into a caption value, through the table of 256 bytes at $C9:D85F that dialogue
//! uses too. A pack that adds characters to the menu font gives them the bytes of kana, and the
//! game's table still says kana. So such a pack gets a glyph in the captions' font for each new
//! character, as wide as a cell since these are drawn a character to a cell, and a table of 256
//! words in the captions' bank, since the new glyphs' numbers do not fit a byte. The two places
//! are rewritten within their own 14 bytes to read it. That is the one change to the game's
//! instructions here that is more than a number.
//!
//! The game does not draw the last two columns of a cell, so a new glyph sits one dot to the left
//! of where its font has it. Nor does it draw the top row of a glyph: both routines put nothing of
//! the letter there and use the font's top row only for the outline of the row below. The game's own
//! glyphs leave that row empty, but Unifont puts a capital's accent there (É, Â), so a character
//! with dots in its top row has the rows above its first empty one moved down a row.
//!
//! And a screen gives its captions only the room the game's own took: a caption with more cells
//! than the game's has been seen to overwrite the caption beside it. How much room each screen has
//! is not known, so the build names the captions that grew and leaves them to be looked at.
//!
//! Full names
//!
//! The first line of the status screen is the character's full name: the given name, the sign ＝,
//! and the family name, the way Japanese writes a foreign name (クレス ＝ アルベイン). The given
//! name is the one the player can change, read from the character's record in work RAM ($7E:07D1,
//! eight bytes a character). The sign is not a tile of the screen's layout: it is menu string 759,
//! which holds ＝ alone, drawn by the routine that draws labels ($C2:B5A4, which names a menu string
//! by a number of its own, here $75A9). The family name is menu string 816 for the first party
//! member and the five after it for the others (number $7917 plus the member's).
//!
//! The code at $C2:8EBB counts the given name's characters and places all three from that count:
//! the name at column 2 of row 6, the sign one column after the name's end, and the family name one
//! column after the sign. The sign's place and the family name's are numbers the code loads, $2186
//! at $C2:8F05 and $218A at $C2:8F19, to which it adds twice the count. The count stops at six
//! characters, the most a name of the game has (the numbers at $C2:8ECB and $C2:8EDF).
//!
//! The naming screen draws string 759 too, at a fixed place (in the list of labels at $C2:AADE),
//! right before the family name ($C2:A954). Nothing else in the code draws string 759 or a family
//! name. The family name of the sixth member, string 821 (Ｆ　＝　レスター), holds a ＝ of its own,
//! which is part of that string.
//!
//! So a pack changes the sign by writing string 759. As a space, it leaves a gap of three columns on
//! the status screen; for one space, the pack also moves the family name two columns to the left,
//! as "status.family" under `[layout]`, which draws it where the sign was, over it. And since the
//! menus of a pack can hold a name of seven characters, a build whose menus have one makes the count
//! stop at seven, or the sign and the family name would come one column early after such a name.
//!
//! Character table
//!
//! `characters.tbl` maps codes to characters. The kana and symbols were read from the font. Most kanji
//! come from aligning the dialogue with a 1999 script dump by Disnesquick, Trimethius and Xeur; the
//! codes that dump does not cover were read from the font and checked against the words they appear
//! in.
//!
//! At run time (found by tracing the game in an emulator)
//!
//! ```text
//! $7E:BFA0   an area's event block is decompressed here, with its dialogue block directly after it.
//!            The two must fit before the end of the bank, 16,480 bytes; the largest original pair
//!            uses 15,957.
//! $C6:79C1   loads an area: its number is in $13CD, and the table at $E8:0000 gives the two blocks.
//!            It leaves the size of the dialogue block in $1434, and in $D7-$D9 a 24-bit pointer to
//!            where the dialogue block starts.
//! $C4:0D02, $C4:0EBB
//!            the two places where an event asks for a string by its number: the offset table at
//!            the pointer in $D7 gives where the string starts, and that offset goes to $13C4.
//! $13C4      the place the text engine is reading at, as an offset from the pointer in $D7. Zero
//!            means that no string is being shown, and events wait for that: one string is shown
//!            at a time.
//! $D7 + $1434 + 2
//!            where the game writes text of its own to show: a character's name, an item's name,
//!            a message. Four places work this address out ($C6:8F09, $C6:9486, $C8:DF93, $C8:E064).
//! ```
//!
//! Dialogue too large for memory
//!
//! A translation is longer than the game's text, and an area's events and dialogue then no longer
//! fit in those 16,480 bytes. Since the game shows one string at a time and finds it in two places
//! only, the dialogue of such an area is left on the cartridge and the string an event asks for is
//! copied to where the dialogue would have been (`demand.asm`). Memory then has to hold the
//! area's events and its longest string, and no longer all of its dialogue. An area that fits is
//! unpacked as the game has always done.
//!
//! ```text
//! $C7:F68D   decompressor, back-reference copy loop (X = source, Y = destination, A = count)
//! $C4:0D26   reads the dialogue block's offset table to find a string by number
//! $C8:DBA9, $C8:E555, $C8:E58F, $C8:E59E
//!            text engine: these read the characters and control-code parameters of the string on
//!            screen
//! ```
//!
//! Text engine routines (bank $C8)
//!
//! ```text
//! $E550      fetch character: reads [$D7],Y and returns a 16-bit code in A, or branches away on
//!            FF FF. A two-byte code becomes page * 128 + (low & $7F). Called from $E506 and $E51C
//!            only.
//! $E5A9      draw glyph (long call, returns with RTL): code in $00, window state at $7E:0000,X.
//!            Font offset = (code - 5) * 32. Called from $DDFB only, in the whole ROM.
//!            The window state holds the tile-data offset (+9), the column (+B) and a phase counter
//!            (+E): three glyphs advance two tiles and every fourth advances one, a fixed 14-pixel
//!            pitch.
//! $E6C3      compose glyph: reads the font through data bank $F0 and writes two bitplanes to work
//!            RAM through $2180, the second being the glyph plus a shadow built from neighbouring
//!            pixels. Called from $E5E2 (dialogue) and $EEA2, in the routine at $EE01 that draws
//!            captions (see Captions above).
//! $E66D, $E691
//!            place the glyph's tiles in the tilemap copy at $7E:5980, two columns or one.
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use crate::adapter::{Adapter, BuildOptions, Built, Release, TextKind};
use crate::codecs::lzss_rle;
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, Font, Glyph};
use crate::layout;
use crate::placement::{Expansion, place};
use crate::platform::{Image, Platform};
use crate::platforms::asm65816::assemble;
use crate::platforms::snes::{self, BankAllocator, Mapping};
use crate::script::{Pack, Script};
use crate::text::{Token, from_hex, plain_characters, quoted, read_table, tag_at, tokens};

pub const NAME: &str = "Tales of Phantasia (Super Famicom, Japan)";
pub const ROM_SHA1: &str = "fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0";
pub const SOURCE_LANGUAGE: &str = "ja";

pub const BLOCK_TABLE: usize = 0x280000;
pub const BLOCK_COUNT: usize = 100;
pub const MAPPING: Mapping = Mapping::ExHiRom;
/// The original image is 6 MiB and its script region is full. Padding it to 8 MiB adds banks
/// $60-$7D, which the game reads without any change: a block that outgrows its slot is moved there.
pub const EXPANDED_SIZE: usize = 0x800000;
pub const SCRIPT_EXPANSION: (usize, usize) = (0x600000, 0x700000);
// The variable-width renderer and its font, also in the new space (see vwf.asm).
pub const RENDERER_CODE: usize = 0x700000;
pub const RENDERER_WIDTHS: usize = 0x710000;
/// 64 bytes per glyph, so each bank holds 1024 glyphs. The bank after it is the sprite text's font,
/// and the two after that are the captions'.
pub const RENDERER_FONT: usize = 0x720000;
pub const RENDERER_FONT_END: usize = 0x7B0000;
/// The game has a second way to show dialogue: over the scene with no window, each character a
/// sprite, 14 dots apart. Its routine at $C8:E8E4 draws a character by its number from the game's
/// font, through the copy routine at $C8:E999, which names the font's bank at $C8:E9AB. That is
/// not the routine the renderer replaces, so the characters a pack adds get a font here that the
/// routine can read: the game's glyphs at their numbers, then the new ones, a character to a cell.
/// The routine finds a glyph at its number times 32 within one bank, so it reaches 2,048.
pub const SPRITE_FONT: usize = 0x7B0000;
pub const SPRITE_FONT_BANK_AT: u32 = 0xC8E9AB;
/// The pitch at which the game spaces its own glyphs.
pub const ORIGINAL_ADVANCE: usize = 14;
// Menu text: labels, item and skill names. 929 strings back to back, each ending in $FF, in the
// naming screen's one-byte encoding. The game addresses a string by its offset from $EF:7000, and
// about nine places in the code read the pool directly, so a string cannot be moved without finding
// every reference.
pub const MENU_SECTION: &str = "menu";
pub const MENU_POOL: (usize, usize) = (0x2F7000, 0x2F8A8E);
/// The text of the game's second text routine: battle messages, the names of skills in kanji, the
/// descriptions of items. Its strings are back to back between these two file offsets.
pub const CAPTION_SECTION: &str = "captions";
pub const CAPTION_POOL: (usize, usize) = (0x2F997C, 0x2FE5DA);
/// The value that ends a caption, and the two that are not glyphs.
pub const CAPTION_END: u16 = 0xFFFF;
pub const CAPTION_LINE: u16 = 0x000D;
/// Followed by one more value, the number of a phrase that many captions share.
pub const CAPTION_PHRASE: u16 = 0x000A;
/// A caption holds glyph numbers plus this.
pub const CAPTION_GLYPH_BIAS: u16 = 0x10;
/// The two lists that follow the pool: the captions' numbers, ending in zero, and for each number
/// the offset of its caption in the pool.
pub const CAPTION_NUMBERS: usize = 0x2FE5DA;
pub const CAPTION_OFFSETS: usize = 0x2FEAF0;
/// A pool that a pack changed goes to a bank of its own in the expanded area. The game reads the
/// pool and the two lists through one data bank, so the lists are copied there too, at the
/// addresses they have in bank $EF, and the pool may fill what is below them. It starts a little
/// into the bank, since an address of zero could be taken for no caption at all.
pub const CAPTION_BANK: usize = 0x7D0000;
pub const CAPTION_BASE: usize = 0x0100;
/// The font the captions are drawn from once a pack changes them: the game's glyphs at their own
/// numbers, and the new ones. The game finds a glyph at its number times 32 within one bank, so
/// the font holds 2,048.
pub const CAPTION_FONT: usize = 0x7C0000;
pub const CAPTION_GLYPHS: usize = 0x10000 / GLYPH_BYTES;
/// The built-in font the captions' new glyphs are drawn from. Its narrow characters are 8 dots
/// wide and a caption's cell is 16, so two of them share a cell.
const CAPTION_FONT_NAME: &str = "unifont";
/// Where the game draws a menu string or a name in the captions' font, it turns each byte into a
/// caption value through its table of 256 bytes (`NAME_GLYPH_TABLE`). A new glyph's number does
/// not fit a byte, so a pack that adds characters to the menu font gets a table of 256 words here,
/// in the captions' bank above the two lists.
pub const CAPTION_MENU_VALUES: usize = CAPTION_BANK + 0xF800;
/// CPU addresses of the two places that read the game's table for this, and what is there: push X,
/// X = the byte, read the table, keep the low byte, add $10, pull X.
const CAPTION_MENU_READERS: [u32; 2] = [0xC9D77D, 0xC9D7C6];
const CAPTION_MENU_READER: [u8; 14] =
    [0xDA, 0xAA, 0xBF, 0x5F, 0xD8, 0xC9, 0x29, 0xFF, 0x00, 0x18, 0x69, 0x10, 0x00, 0xFA];
/// Line break.
pub const MENU_LINE: u8 = 0xFC;
/// An empty tile.
pub const MENU_BLANK: u8 = 0xBF;
/// The part of the pool that holds the default character names.
pub const NAME_STRINGS: (usize, usize) = (0x2F7B92, 0x2F7BC0);
/// The menu font: 192 two-bitplane 8x8 tiles, compressed, in two copies that differ in a few tiles.
/// A byte below $C0 draws the tile of that number. $C0-$E8 and $E9-$FB are accented kana: the plain
/// kana's tile, looked up in a table, with a mark on the row above.
pub const MENU_FONT_BLOCKS: [usize; 2] = [0x1EC23C, 0x2E9FC4];
pub const MENU_ACCENTED: u8 = 0xC0;
pub const MENU_ACCENTED_2: u8 = 0xE9;
/// CPU addresses.
pub const MENU_BASE_TABLE: u32 = 0xC9D135;
pub const MENU_BASE_TABLE_2: u32 = 0xC9D15E;
pub const MENU_MARKS: [u8; 2] = [0xB2, 0xB3];
/// A menu string that outgrows its slot moves to `MENU_AREA`; its slot then holds `MENU_ESCAPE` and
/// the new address, which the hooked readers follow (see menu.asm). `MENU_AREA` must be the
/// upper half of one bank, because the readers tell a moved string by an index of $8000 or more.
pub const MENU_ESCAPE: u8 = 0xFD;
pub const MENU_CODE: usize = 0x706000;
pub const MENU_AREA: (usize, usize) = (0x718000, 0x720000);
/// The longest original string; some readers copy a string into a fixed buffer.
pub const MENU_MAX_LENGTH: usize = 26;
// Compressed blocks that outgrow their slot move here, and a hook on the decompressor finds them
// (see moved.asm).
pub const MOVED_CODE: usize = 0x705000;
pub const MOVED_TABLE: usize = 0x708400;
pub const MOVED_AREA: (usize, usize) = (0x709000, 0x710000);
/// Character names (see names.asm). The game keeps each as an 8-byte record in work RAM, in
/// the naming screen's encoding. These are the defaults, read from a running game, by party member
/// number.
pub const DEFAULT_NAME_RECORDS: [(u8, &[u8]); 5] = [
    (0x01, &[0x55, 0x77, 0x5A, 0xFF]),
    (0x02, &[0x6D, 0x7A, 0x61, 0xFF]),
    (0x03, &[0x4E, 0x4D, 0x5E, 0x47, 0xFF]),
    (0x04, &[0x5E, 0x47, 0x5A, 0x5D, 0x4D, 0xFF]),
    (0x06, &[0x55, 0x74, 0x4D, 0x5A, 0xFF]),
];
/// CPU address: naming-screen byte -> glyph number.
pub const NAME_GLYPH_TABLE: u32 = 0xC9D85F;
/// A converted name holds glyph numbers plus this.
pub const NAME_GLYPH_BIAS: usize = 0x15;
pub const NAME_CODE: usize = 0x704000;
pub const NAME_DEFAULTS: usize = 0x708000;
pub const NAME_TRANSLATED: usize = 0x708100;
/// 256 words: menu byte -> glyph number plus `NAME_GLYPH_BIAS`, for the pack's menu font.
pub const NAME_GLYPHS: usize = 0x708600;
/// Bytes per translated name: up to 15 characters and the end marker.
pub const NAME_SLOT: usize = 32;
/// Characters of a name in the game's 8-byte record, which the menus show.
pub const NAME_RECORD_LENGTH: usize = 7;
pub const MAX_NAME_LENGTH: usize = NAME_SLOT / 2 - 1;
/// What a build holds a name to, as `Adapter::name_limits` gives it: `MAX_NAME_LENGTH` characters for
/// dialogue, and `NAME_RECORD_LENGTH` for the menus, which keep the game's names when one is longer
/// (see `build`). A test holds the numbers here to the constants.
const NAME_LIMITS: &str = "a name is 1 to 15 characters, without tags; the menus hold names of up to 7, and keep the \
                           game's own names when one is longer";
/// A converted name is written two bytes past the dialogue block, so that much RAM must stay free.
pub const NAME_ROOM: usize = 2 + NAME_SLOT;
/// The window is 224 pixels wide. On its last line the game stops drawing once the pen reaches
/// column 26, so a character may start anywhere left of that on every line.
pub const LINE_WIDTH: usize = 26 * 8;
/// The dialogue the game shows as sprites over the scene and not in a box, by block and string
/// number. An area's events say how each string is shown: command $3C, then the string's number.
/// These are the eight strings that command names, found by reading every area's events command
/// by command. The game does not draw such a string through the routine the renderer replaces
/// (see `SPRITE_FONT`), so its letters are 14 dots apart whatever they are, and three things
/// limit it, each seen in the game:
///
/// - a row holds 16 characters, and the game starts a new row after the 16th wherever that falls.
///   A row that is full and then has a line break leaves an empty row, so the build fills a row
///   with 15 at most;
/// - a page holds 32 drawn characters: the 33rd and those after it come out as scrambled dots;
/// - the game's own space (code 05) is not drawn and does not count toward the 32, where a space
///   a pack's font adds is a character like any other. So the build writes every space as the
///   game's own.
pub const SPRITE_LINES: [(usize, u32); 8] = [(1, 4), (7, 0), (49, 79), (59, 8), (79, 53), (99, 0), (99, 1), (99, 2)];
pub const SPRITE_ROW: usize = 15;
pub const SPRITE_ROW_FULL: usize = 16;
pub const SPRITE_PAGE: usize = 32;
/// The names of places and shops, by block and string number, which the game shows in a box of one
/// line at the top of the screen on entering. An area's events show such a name with command $0D,
/// then the string's number; these are the 122 strings that command names, found by reading every
/// area's events command by command, and the game shows none of them any other way.
///
/// The command's code ($C4:0E96) sets bit 0 of $137D, which has the text engine centre the line
/// ($C8:DB51 to $C8:DB9C). It counts the string's characters, draws half of what 16 leaves of the
/// count as the game's spaces in front, 14 dots each, then the string. Two things break the box:
///
/// - more than 16 characters. The engine takes the count from 16 in one byte without a check, so 17
///   gives about 127 spaces;
/// - a character that would start at dot 208 or after (`LINE_WIDTH`), where the engine breaks the line
///   ($C8:DC31). The box has one line, so the page never fills: the name is not drawn at all and the
///   box shows what was left in it from an earlier message.
///
/// What the engine counts is set out at `place_name_count`.
#[rustfmt::skip]
pub const PLACE_NAMES: [(usize, u32); 122] = [
    (3, 0), (5, 0), (5, 1), (5, 2), (5, 3), (5, 4), (5, 5), (13, 1), (17, 0), (17, 1), (17, 2), (17, 3), (17, 4),
    (17, 5), (17, 6), (17, 7), (17, 8), (17, 9), (17, 10), (19, 0), (21, 0), (21, 1), (23, 0), (25, 0), (25, 1),
    (25, 2), (25, 3), (25, 4), (31, 0), (35, 0), (35, 1), (35, 3), (37, 0), (37, 1), (37, 2), (37, 3), (37, 4),
    (37, 5), (37, 6), (39, 1), (41, 0), (41, 1), (41, 2), (41, 3), (41, 4), (43, 2), (43, 3), (49, 0), (53, 17),
    (53, 18), (53, 19), (55, 0), (55, 1), (55, 2), (55, 3), (55, 4), (55, 5), (57, 1), (57, 2), (57, 3), (57, 4),
    (57, 5), (57, 6), (57, 7), (57, 8), (59, 0), (61, 0), (61, 1), (61, 2), (61, 3), (61, 4), (61, 5), (61, 6),
    (61, 7), (61, 8), (61, 9), (63, 0), (63, 1), (63, 2), (63, 3), (63, 4), (65, 0), (65, 1), (65, 2), (65, 3),
    (65, 4), (65, 5), (65, 92), (65, 96), (67, 2), (69, 0), (69, 1), (69, 2), (71, 0), (71, 1), (73, 1), (73, 2),
    (73, 3), (73, 4), (73, 5), (75, 0), (75, 1), (77, 0), (77, 1), (77, 2), (77, 3), (77, 4), (77, 5), (79, 6),
    (85, 0), (85, 2), (87, 0), (87, 1), (87, 2), (89, 1), (89, 2), (89, 3), (91, 0), (93, 0), (93, 1), (95, 0),
    (97, 0),
];
/// The most characters a place name may have, as the game counts them.
pub const PLACE_NAME_LENGTH: usize = 16;
/// The menus draw most of their fixed labels at places that are plain numbers in the game: in
/// lists of labels handed to the routine at $C2:748B (a byte for the colour, the string's number
/// and its place, five bytes a label), or as the number an instruction loads before a call to
/// $C2:B5A4. A place is an offset into the menus' tile memory: two bytes a tile, 32 tiles a row,
/// $800 a layer. `labels.tbl` lists every such label, 164 of them, found by reading the
/// image for the two forms: its name, where in the image its place is written, the place the game
/// has there, and the screen it is on where that was seen. A label's name is its string's address,
/// a hash, and which of the string's places it is, in the order they are in the image.
///
/// A pack moves a label by columns and rows (`BuildOptions::layout`), which changes that one
/// number. What is beside a label is not moved with it: a value, a name from a list and a picture
/// are placed by the screen's own code. The build cannot see whether a moved label lands on
/// something else; only the screen shows that.
pub struct Label {
    pub name: &'static str,
    /// The menu string it shows.
    pub string: u32,
    /// Where in the image its place is written, as a 16-bit number.
    offset: usize,
    /// The place the game has there.
    place: u16,
    /// The screen it is on, or nothing where that is not known.
    pub screen: &'static str,
}

impl Label {
    /// Where the game draws it: its row and its column, counted in tiles within its layer.
    pub fn row_and_column(&self) -> (u16, u16) {
        (self.place % LABEL_LAYER / LABEL_ROW, self.place % LABEL_ROW / 2)
    }
}

/// A row of the menus' tile memory is 32 tiles of two bytes, and a layer is 32 rows.
const LABEL_ROW: u16 = 0x40;
const LABEL_LAYER: u16 = 0x800;

pub static LABELS: LazyLock<Vec<Label>> = LazyLock::new(|| {
    let mut labels = Vec::new();
    for line in include_str!("labels.tbl").lines() {
        let mut fields = line.splitn(4, ' ');
        let (Some(name), Some(offset), Some(place)) = (fields.next(), fields.next(), fields.next()) else { continue };
        let string = name.strip_prefix("menu.").and_then(|rest| rest.split('#').next()).and_then(|n| n.parse().ok());
        let numbers = (usize::from_str_radix(offset, 16), u16::from_str_radix(place, 16));
        if let (Some(string), (Ok(offset), Ok(place))) = (string, numbers) {
            labels.push(Label { name, string, offset, place, screen: fields.next().unwrap_or_default() });
        }
    }
    labels
});

/// A picture that a screen draws beside a label, which has to move when the label does. It is a
/// sprite, placed by the screen's code in dots from the left and from the top, each a number an
/// instruction loads. There is one so far: the control pad after the label of the skills row on
/// the status screen, set at $C2:D1DB, which the game puts four dots after a label of three
/// characters.
struct Picture {
    name: &'static str,
    /// Where in the image its distance from the left and from the top are written, and what the
    /// game has there.
    left: (usize, u16),
    top: (usize, u16),
}

const PICTURES: [Picture; 1] = [Picture { name: "picture.skills", left: (0x02D1DC, 164), top: (0x02D1E1, 164) }];

/// The game with a picture drawn where a pack asks, a column being eight dots and a row eight.
fn move_picture(rom: &mut [u8], picture: &Picture, columns: i32, rows: i32) -> Result<()> {
    let name = picture.name;
    let to = (i32::from(picture.left.1) + 8 * columns, i32::from(picture.top.1) + 8 * rows);
    if !(0..=240).contains(&to.0) || !(0..=208).contains(&to.1) {
        return text_error(format!(
            "{name} is {} dots from the left and {} from the top, and moving it by {columns} columns and {rows} rows \
             would take it off the screen",
            picture.left.1, picture.top.1
        ));
    }
    for ((at, game), new) in [(picture.left, to.0), (picture.top, to.1)] {
        if rom.get(at..at + 2) != Some(&game.to_le_bytes()[..]) {
            return Err(Error::Image(format!("the place of {name} is not where it is known to be")));
        }
        rom[at..at + 2].copy_from_slice(&(new as u16).to_le_bytes());
    }
    Ok(())
}

/// The menu string that the status screen draws between a character's given name and family name,
/// and the naming screen before the family name: ＝ alone (see "Full names" at the top).
pub const NAME_SIGN: u32 = 759;
/// The menu strings that hold the family names, one per party member, in the order of their numbers.
pub const FAMILY_NAMES: (u32, u32) = (816, 821);

/// The family name on the status screen, which a pack moves as it moves a label. Its place is not
/// a plain number like a label's: the code loads this one and adds twice the length of the given
/// name before it, so the column it names is where the family name goes after a name of no letters.
/// Two columns to the left draws it where the sign was, one column after the given name.
static FAMILY_NAME: Label =
    Label { name: "status.family", string: FAMILY_NAMES.0, offset: 0x028F1A, place: 0x218A, screen: "status screen" };

/// Where the status screen's code says how many characters of a given name it counts to place what
/// follows the name: a 16-bit number at the first offset and an 8-bit one at the second. The game
/// counts six, its longest name; a pack's menus can hold seven (`NAME_RECORD_LENGTH`).
const NAME_COUNT: [(usize, usize); 2] = [(0x028ECC, 2), (0x028EE0, 1)];
const NAME_COUNTED: usize = 6;

/// The game with the status screen counting every character a name in the menus can have, so that
/// the sign and the family name after a name of seven characters keep their distance from it.
fn install_name_count(mut rom: Vec<u8>) -> Result<Vec<u8>> {
    for (at, size) in NAME_COUNT {
        let game = (NAME_COUNTED as u16).to_le_bytes();
        if rom.get(at..at + size) != Some(&game[..size]) {
            return Err(Error::Image("the status screen's count of a name is not where it is known to be".into()));
        }
        rom[at..at + size].copy_from_slice(&(NAME_RECORD_LENGTH as u16).to_le_bytes()[..size]);
    }
    Ok(rom)
}

/// The game with labels drawn where a pack asks: each name with how far to move it, in columns
/// to the right and rows down.
pub fn install_layout(mut rom: Vec<u8>, layout: &BTreeMap<String, (i32, i32)>) -> Result<Vec<u8>> {
    for (name, (columns, rows)) in layout {
        if let Some(picture) = PICTURES.iter().find(|picture| picture.name == name) {
            move_picture(&mut rom, picture, *columns, *rows)?;
            continue;
        }
        let Some(label) = LABELS.iter().chain([&FAMILY_NAME]).find(|label| label.name == name) else {
            return text_error(format!(
                "{} is not a label a pack can move: the extracted menu.txt names them, under each string",
                quoted(name)
            ));
        };
        let (row, column) = label.row_and_column();
        let (to_row, to_column) = (i32::from(row) + rows, i32::from(column) + columns);
        if !(0..32).contains(&to_row) || !(0..32).contains(&to_column) {
            return text_error(format!(
                "{name} is at column {column}, row {row}, and moving it by {columns} columns and {rows} rows would \
                 take it off the 32 columns and 32 rows of the screen"
            ));
        }
        let at = label.offset;
        if rom.get(at..at + 2) != Some(&label.place.to_le_bytes()[..]) {
            return Err(Error::Image(format!("the place of the label {name} is not where it is known to be")));
        }
        let place = i32::from(label.place) + 2 * columns + i32::from(LABEL_ROW) * rows;
        rom[at..at + 2].copy_from_slice(&(place as u16).to_le_bytes());
    }
    Ok(rom)
}

/// The game's own space, which a string shown as sprites uses between its words.
const GAME_SPACE: char = '\u{3000}';
/// An area's event block and dialogue block are decompressed back to back at $7E:BFA0.
pub const AREA_RAM_SIZE: usize = 0x10000 - 0xBFA0;
// Dialogue that does not fit there stays on the cartridge, and the string being shown is copied
// into memory when an event asks for it (see demand.asm).
pub const DEMAND_CODE: usize = 0x707000;
/// Eight bytes per area: where its dialogue is on the cartridge, and the room kept for one string.
pub const DEMAND_TABLE: usize = 0x708800;
/// Where in that room a string is copied. Zero would tell the game that no string is being shown.
pub const DEMAND_STRING_AT: usize = 2;
/// Memory left free past that room. The game writes text of its own there, such as a name or an
/// item's name in a sentence, and nothing says how long that text can be: the game's own largest
/// area leaves 523 bytes.
pub const DEMAND_SPARE: usize = 512;

pub const FONT: usize = 0x300000;
pub const GLYPH_SIZE: usize = 16;
pub const GLYPH_BYTES: usize = 32;
pub const FIRST_GLYPH_CODE: usize = 0x05;
pub const LAST_CODE: u32 = 0x0CC5;
pub const GLYPH_COUNT: usize = glyph_index(LAST_CODE) + 1;
/// The built-in fonts that new characters of the dialogue are drawn from when a build is given none.
const FONTS: &[&str] = &["unifont"];
/// The same for the menus, whose characters are tiles of 8 by 8 pixels.
const MENU_FONT: &str = "unscii-8";

const END: [u8; 2] = [0xFF, 0xFF];

/// The characters of the game's font, from `characters.tbl`: one `HEX=character` line per code, the
/// hex being the bytes as stored in the ROM.
struct Table {
    /// Code -> character. Single-byte codes are stored as is, two-byte codes as page << 8 | low.
    /// The table covers every code the dialogue uses; font slots it never uses have no entry yet.
    characters: HashMap<u32, char>,
    codes: HashMap<char, u32>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut characters = HashMap::new();
    for (hex_code, text) in read_table(include_str!("characters.tbl")) {
        let code = match from_hex(hex_code).as_deref() {
            Some([code]) => u32::from(*code),
            Some([low, page]) => u32::from(*page) << 8 | u32::from(*low),
            _ => panic!("top_sfc/characters.tbl: {hex_code} is not a code of one or two bytes"),
        };
        let mut letters = text.chars();
        let (Some(char), None) = (letters.next(), letters.next()) else {
            panic!("top_sfc/characters.tbl: code {hex_code} is not one character");
        };
        characters.insert(code, char);
    }
    let codes: HashMap<char, u32> = characters.iter().map(|(code, char)| (*char, *code)).collect();
    assert_eq!(codes.len(), characters.len(), "the character table must be one-to-one");
    Table { characters, codes }
});

const PARAM_TAGS: [(u8, &str); 3] = [(0x02, "NAME"), (0x03, "SPEED"), (0x04, "COLOR")];
const LONG_TAGS: [(u8, &str); 2] = [(0x81, "NUM"), (0x86, "STR")];
/// Code 02 prints a party member's name, which the player may have changed. The script shows who
/// it is.
pub const SPEAKERS: [(u8, &str); 5] =
    [(0x01, "クレス"), (0x02, "ミント"), (0x03, "アーチェ"), (0x04, "チェスター"), (0x06, "クラース")];

/// The tag a code of one of the lists above is written as.
fn tag_of(tags: &[(u8, &'static str)], code: u8) -> Option<&'static str> {
    tags.iter().find(|(known, _)| *known == code).map(|(_, tag)| *tag)
}

/// The code a tag of one of the lists above stands for.
fn code_of(tags: &[(u8, &'static str)], tag: &str) -> Option<u8> {
    tags.iter().find(|(_, known)| *known == tag).map(|(code, _)| *code)
}

/// The tag of the party member a default name record belongs to.
fn speaker(member: u8) -> &'static str {
    tag_of(&SPEAKERS, member).expect("every default name record is a speaker's")
}

/// The codes of the characters a pack adds to the font: character -> glyph number + 5.
pub type Extra = BTreeMap<char, usize>;

/// The strings a pack gives for one section of the script, by number.
type Strings = BTreeMap<u32, String>;

// --- blocks ----------------------------------------------------------------

/// File offset of each script block, in table order.
pub fn block_offsets(rom: &[u8]) -> Result<Vec<usize>> {
    (0..BLOCK_COUNT)
        .map(|index| {
            let at = BLOCK_TABLE + 3 * index;
            let address = u32::from(rom[at]) | u32::from(rom[at + 1]) << 8 | u32::from(rom[at + 2]) << 16;
            snes::snes_to_pc(address, MAPPING)
        })
        .collect()
}

pub fn read_blocks(rom: &[u8]) -> Result<Vec<Vec<u8>>> {
    block_offsets(rom)?.into_iter().map(|offset| lzss_rle::decompress_block(rom, offset)).collect()
}

pub fn is_text_block(index: usize) -> bool {
    index % 2 == 1
}

/// Split a dialogue block into its strings, each still carrying its end marker.
pub fn split_strings(block: &[u8]) -> Result<Vec<&[u8]>> {
    let word = |at: usize| usize::from(block[at]) | usize::from(block[at + 1]) << 8;
    let table_size = match block {
        [] => 0,
        [only] => usize::from(*only),
        _ => word(0),
    };
    if table_size < 2 || table_size % 2 != 0 || table_size > block.len() {
        return text_error("block does not start with a string offset table");
    }
    let mut bounds: Vec<usize> = (0..table_size).step_by(2).map(word).collect();
    bounds.push(block.len());
    if bounds.windows(2).any(|pair| pair[0] >= pair[1]) {
        return text_error("string offsets are not strictly ascending");
    }
    Ok(bounds.windows(2).map(|pair| &block[pair[0]..pair[1]]).collect())
}

/// Build a dialogue block from its strings.
pub fn join_strings<S: AsRef<[u8]>>(strings: &[S]) -> Result<Vec<u8>> {
    let mut position = 2 * strings.len();
    let mut block = Vec::new();
    for string in strings {
        let Ok(offset) = u16::try_from(position) else {
            return text_error("dialogue block is larger than 64 KiB");
        };
        block.extend(offset.to_le_bytes());
        position += string.as_ref().len();
    }
    for string in strings {
        block.extend(string.as_ref());
    }
    Ok(block)
}

/// What a build made, and what it did to make it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildResult {
    pub rom: Vec<u8>,
    /// Dialogue blocks whose content changed.
    pub changed: Vec<usize>,
    /// Those that outgrew their slot and went to the expanded area.
    pub moved: Vec<usize>,
    /// Those too large for the memory the game unpacks an area into, whose strings are read from
    /// the cartridge one at a time.
    pub on_cartridge: Vec<usize>,
    /// Characters the game's font lacks, drawn from the pack's font.
    pub new_characters: Vec<char>,
    /// Whether the variable-width renderer was installed.
    pub renderer: bool,
    /// Characters whose default name the pack replaces, by tag.
    pub renamed: Vec<String>,
    /// Menu strings whose content changed.
    pub menu_changed: Vec<usize>,
    /// Characters added to the menu font.
    pub new_menu_characters: Vec<char>,
    /// Why the menus kept the game's names although the pack renames characters, if so.
    pub menu_names_skipped: String,
    /// Captions whose content changed, by number.
    pub captions_changed: Vec<usize>,
    /// Glyphs added to the captions' font, and how many of them took the number of a kanji that no
    /// caption uses any more.
    pub caption_glyphs: usize,
    pub caption_glyphs_replaced: usize,
    /// How many of those glyphs are characters the menu font gained, which the captions' font draws
    /// where the game shows a menu string or a name in it.
    pub caption_menu_glyphs: usize,
    /// Changed captions that take more cells than the game's own.
    pub captions_wider: Vec<usize>,
}

impl BuildResult {
    /// What the build did, as lines to show.
    pub fn summary(&self) -> Vec<String> {
        let numbered =
            |blocks: &[usize]| blocks.iter().map(|index| format!("{index:02}")).collect::<Vec<_>>().join(", ");
        let mut lines = Vec::new();
        if self.changed.is_empty() {
            lines.push("changed:   nothing, the script matches the game".to_string());
        } else {
            lines.push(format!("changed:   {} dialogue blocks ({})", self.changed.len(), numbered(&self.changed)));
        }
        if !self.moved.is_empty() {
            lines.push(format!(
                "moved:     {} outgrew their space and went to the expanded area ({})",
                self.moved.len(),
                numbered(&self.moved)
            ));
        }
        if !self.on_cartridge.is_empty() {
            lines.push(format!(
                "cartridge: {} are too large for the game's memory and are read from the cartridge a string at a \
                 time ({})",
                self.on_cartridge.len(),
                numbered(&self.on_cartridge)
            ));
        }
        if !self.new_characters.is_empty() {
            let shown: String = self.new_characters.iter().take(60).collect();
            lines.push(format!("font:      {} characters the game's font lacks: {shown}", self.new_characters.len()));
        }
        if !self.menu_changed.is_empty() {
            lines.push(format!("menu:      {} menu strings changed", self.menu_changed.len()));
        }
        if !self.new_menu_characters.is_empty() {
            let shown: String = self.new_menu_characters.iter().take(60).collect();
            lines.push(format!("menu font: {} characters added: {shown}", self.new_menu_characters.len()));
        }
        if !self.captions_changed.is_empty() || self.caption_glyphs > 0 {
            let mut line = format!("captions:  {} captions changed", self.captions_changed.len());
            if self.caption_glyphs > 0 {
                line.push_str(&format!(", with {} glyphs added to their font", self.caption_glyphs));
            }
            if self.caption_glyphs_replaced > 0 {
                let replaced = self.caption_glyphs_replaced;
                line.push_str(&format!(" ({replaced} in place of kanji that no caption uses any more)"));
            }
            lines.push(line);
        }
        if self.caption_menu_glyphs > 0 {
            lines.push(format!(
                "           {} of the glyphs are the menu font's new characters, for a menu string or a name shown \
                 in the captions' font",
                self.caption_menu_glyphs
            ));
        }
        if !self.captions_wider.is_empty() {
            let shown: Vec<String> = self.captions_wider.iter().take(12).map(|number| format!("{number:03}")).collect();
            let more = if self.captions_wider.len() > shown.len() { ", ..." } else { "" };
            let count = self.captions_wider.len();
            let listed = shown.join(", ");
            lines.push(format!(
                "           {count} take more cells than the game's own and may run into what is beside them on \
                 screen ({listed}{more})"
            ));
        }
        if !self.renamed.is_empty() {
            lines.push(format!("names:     {} characters renamed ({})", self.renamed.len(), self.renamed.join(", ")));
        }
        if !self.menu_names_skipped.is_empty() {
            lines.push(format!("           the menus keep the game's names: {}", self.menu_names_skipped));
        }
        if self.renderer {
            lines.push("renderer:  variable-width renderer installed".to_string());
        }
        lines
    }
}

/// The menu encoding: byte -> character, read from the game's own conversion table.
///
/// A few characters can be written with two different bytes. Each goes to the byte the game's text
/// uses, so that the table is one-to-one; the spare byte has no entry and shows up as a raw code.
pub fn menu_characters(rom: &[u8]) -> Result<BTreeMap<u8, char>> {
    let table = snes::snes_to_pc(NAME_GLYPH_TABLE, MAPPING)?;
    let mut uses = [0usize; 256];
    for (_, data) in menu_strings(rom) {
        for byte in data {
            uses[usize::from(*byte)] += 1;
        }
    }
    let mut chars = BTreeMap::from([(MENU_BLANK, '　')]);
    let mut owner = HashMap::from([('　', MENU_BLANK)]);
    for byte in 0..MENU_LINE {
        let glyph = rom[table + usize::from(byte)];
        let Some(char) = TABLE.characters.get(&code_at(usize::from(glyph))).copied() else { continue };
        if byte == MENU_BLANK || glyph == 0 || char == '　' {
            continue;
        }
        if let Some(rival) = owner.get(&char).copied() {
            if uses[usize::from(rival)] >= uses[usize::from(byte)] {
                continue;
            }
            chars.remove(&rival);
        }
        chars.insert(byte, char);
        owner.insert(char, byte);
    }
    Ok(chars)
}

/// The menu encoding the other way round: character -> byte.
fn menu_codes(chars: &BTreeMap<u8, char>) -> HashMap<char, u8> {
    chars.iter().map(|(byte, char)| (*char, *byte)).collect()
}

/// Every menu string as (file offset, bytes without the terminator), in the order they are stored.
pub fn menu_strings(rom: &[u8]) -> Vec<(usize, &[u8])> {
    let mut strings = Vec::new();
    let (start, end) = MENU_POOL;
    let mut position = start;
    while position < end {
        // The pool ends with a terminator; in an image where it does not, what is left is no string.
        let Some(length) = rom[position..end].iter().position(|byte| *byte == 0xFF) else { break };
        strings.push((position, &rom[position..position + length]));
        position += length + 1;
    }
    strings
}

/// Convert a menu string to editable text.
pub fn decode_menu(data: &[u8], chars: &BTreeMap<u8, char>) -> String {
    let mut text = String::new();
    for byte in data {
        match chars.get(byte) {
            _ if *byte == MENU_LINE => text.push('\n'),
            Some(char) => text.push(*char),
            None => text.push_str(&format!("{{RAW:{byte:02X}}}")),
        }
    }
    text
}

/// A character in quotes, as a message shows it.
fn quoted_char(char: char) -> String {
    quoted(char.encode_utf8(&mut [0; 4]))
}

/// Convert editable text to a menu string, without its terminator.
pub fn encode_menu(text: &str, codes: &HashMap<char, u8>) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        if char == '\n' {
            out.push(MENU_LINE);
        } else if char == '{' {
            let Some(Token::Tag { raw, name: "RAW", value }) = tag_at(rest) else {
                let shown: String = rest.chars().take(12).collect();
                return text_error(format!("menu text has no tags; found {}", quoted(&shown)));
            };
            // The Python reference stops with an internal error here; a translator is told instead.
            let Some(bytes) = from_hex(value.unwrap_or("")) else {
                return text_error(format!("tag {raw} does not hold hexadecimal bytes"));
            };
            out.extend(bytes);
            rest = &rest[raw.len()..];
            continue;
        } else if let Some(byte) = codes.get(&char) {
            out.push(*byte);
        } else {
            return text_error(format!("character {} cannot be used in menu text yet", quoted_char(char)));
        }
        rest = &rest[char.len_utf8()..];
    }
    Ok(out)
}

/// The game's own name for each party member, by tag.
pub fn default_names(rom: &[u8]) -> Result<Vec<(&'static str, String)>> {
    let table = snes::snes_to_pc(NAME_GLYPH_TABLE, MAPPING)?;
    let mut names = Vec::new();
    for (member, record) in DEFAULT_NAME_RECORDS {
        let mut name = String::new();
        for byte in &record[..record.len() - 1] {
            let code = code_at(usize::from(rom[table + usize::from(*byte)]));
            let Some(char) = TABLE.characters.get(&code) else {
                return Err(Error::Image(format!(
                    "the game's table of name characters has no character for ${byte:02x}"
                )));
            };
            name.push(*char);
        }
        names.push((speaker(member), name));
    }
    Ok(names)
}

/// The image as it is once expanded: itself if it already is, padded to 8 MiB if not.
fn expanded(rom: Vec<u8>) -> Result<Vec<u8>> {
    if rom.len() >= EXPANDED_SIZE { Ok(rom) } else { snes::expand(&rom, EXPANDED_SIZE) }
}

/// A file offset as the CPU address the assembler is given for it.
fn cpu_address(offset: usize) -> Result<i64> {
    Ok(i64::from(snes::pc_to_snes(offset, MAPPING)?))
}

/// An area's dialogue as it is kept on the cartridge when it is not unpacked into memory: a table
/// of 16-bit offsets, one per string and one more for where the last string ends, and the strings.
pub fn demand_block<S: AsRef<[u8]>>(strings: &[S]) -> Result<Vec<u8>> {
    let mut position = 2 * (strings.len() + 1);
    let mut block = Vec::new();
    let ends = strings.iter().map(|string| string.as_ref().len()).chain([0]);
    for length in ends {
        let Ok(offset) = u16::try_from(position) else {
            return text_error("dialogue block is larger than 64 KiB");
        };
        block.extend(offset.to_le_bytes());
        position += length;
    }
    for string in strings {
        block.extend(string.as_ref());
    }
    Ok(block)
}

/// Make the game read the dialogue of some areas from the cartridge, a string at a time.
///
/// `areas` gives, for each such dialogue block, its index, the file offset its `demand_block` was
/// written at, and the room in memory that one of its strings needs.
pub fn install_demand(rom: Vec<u8>, areas: &[(usize, usize, usize)]) -> Result<Vec<u8>> {
    let mut out = expanded(rom)?;
    out[DEMAND_TABLE..DEMAND_TABLE + 8 * (BLOCK_COUNT / 2)].fill(0);
    for (index, target, room) in areas {
        // A dialogue block is the second of its area's pair.
        let entry = DEMAND_TABLE + 8 * (index / 2);
        let address = snes::pc_to_snes(*target, MAPPING)?.to_le_bytes();
        out[entry..entry + 3].copy_from_slice(&address[..3]);
        let Ok(room) = u16::try_from(*room) else {
            return text_error("a string is too long to be copied into memory");
        };
        out[entry + 6..entry + 8].copy_from_slice(&room.to_le_bytes());
    }
    let symbols = [("DEMAND_CODE", cpu_address(DEMAND_CODE)?), ("DEMAND", cpu_address(DEMAND_TABLE)?)];
    snes::write_code(&out, &assemble(include_str!("demand.asm"), &symbols)?.segments, MAPPING)
}

/// The captions, each as its 16-bit values without the one that ends it.
pub fn caption_strings(rom: &[u8]) -> Result<Vec<Vec<u16>>> {
    let Some(pool) = rom.get(CAPTION_POOL.0..CAPTION_POOL.1) else {
        return Err(Error::Image("the image is too short to hold the captions".into()));
    };
    let mut values = pool.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair));
    let mut strings = Vec::new();
    let mut string = Vec::new();
    while let Some(value) = values.next() {
        if value == CAPTION_END {
            strings.push(std::mem::take(&mut string));
            continue;
        }
        string.push(value);
        // A phrase's number is a value like any other, and is not to be taken for the end.
        if value == CAPTION_PHRASE {
            string.extend(values.next());
        }
    }
    if !string.is_empty() {
        return Err(Error::Image("the captions do not end where they are known to".into()));
    }
    Ok(strings)
}

/// A glyph number as the code the dialogue has for the same glyph, in the form the character table
/// is keyed by: a single byte as it is, two bytes as page << 8 | low.
fn code_of_glyph(glyph: u32) -> u32 {
    let number = glyph + FIRST_GLYPH_CODE as u32;
    if number < 0x80 { number } else { (number >> 7) << 8 | 0x80 | (number & 0x7F) }
}

/// One caption as editable text. A glyph the character table has no entry for is written as the
/// dialogue writes it, `{K:0C45}`; a shared phrase as `{PHRASE:EA61}`; any other value as `{RAW:0001}`.
pub fn decode_caption(values: &[u16]) -> String {
    let mut out = String::new();
    let mut values = values.iter().copied();
    while let Some(value) = values.next() {
        let glyph = u32::from(value).wrapping_sub(u32::from(CAPTION_GLYPH_BIAS));
        if value == CAPTION_LINE {
            out.push('\n');
        } else if value == CAPTION_PHRASE {
            match values.next() {
                Some(phrase) => out.push_str(&format!("{{PHRASE:{phrase:04X}}}")),
                None => out.push_str(&format!("{{RAW:{value:04X}}}")),
            }
        } else if value >= CAPTION_GLYPH_BIAS && (glyph as usize) < GLYPH_COUNT {
            let code = code_of_glyph(glyph);
            match TABLE.characters.get(&code) {
                // A character that is also how a line break or a tag is written would not read back.
                Some(char) if !matches!(char, '\n' | '{' | '}') => out.push(*char),
                _ => out.push_str(&format!("{{K:{code:04X}}}")),
            }
        } else {
            out.push_str(&format!("{{RAW:{value:04X}}}"));
        }
    }
    out
}

/// A glyph the captions gain: one character as wide as a cell, two narrow ones side by side, or a
/// narrow one alone in the middle of a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaptionGlyph {
    Wide(char),
    Pair(char, char),
    Alone(char),
}

/// The form of a character that is as wide as a cell, where Unicode has one: `ａ` for `a`.
fn full_width(char: char) -> Option<char> {
    matches!(char, '!'..='~').then(|| char::from_u32(char as u32 + 0xFEE0)).flatten()
}

/// What a caption is made of, before its new glyphs have numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionCell {
    /// A value as the game holds it: a glyph of its font, a line break, a phrase or its number.
    Value(u16),
    New(CaptionGlyph),
}

/// The value a caption holds for a character of the game's font, by its code in the dialogue.
fn caption_value_of_code(code: u32) -> Option<u16> {
    let number = if code < 0x80 { code } else { (code >> 8) * 128 + (code & 0x7F) };
    let glyph = number.checked_sub(FIRST_GLYPH_CODE as u32)?;
    ((glyph as usize) < GLYPH_COUNT).then_some(glyph as u16 + CAPTION_GLYPH_BIAS)
}

/// One caption from its text, as cells.
///
/// A character of the game's font takes a cell. With `widths`, the width in dots of each character
/// the caption font has, a character the game's font lacks is taken from that font: a wide one
/// takes a cell, and narrow ones go two to a cell, in the order they are written. A run of narrow
/// characters with an odd number of them ends in a space; so does a run before a line break, a
/// phrase or a character of the game's font, since a cell cannot be shared across those.
///
/// The game's font has a few of the characters a narrow run is written with, the percent sign for
/// one. Next to a narrow character such a character is narrow too; among the game's own it is the
/// game's, so that the game's captions read back as they are.
pub fn caption_cells(text: &str, widths: Option<&BTreeMap<char, usize>>) -> Result<Vec<CaptionCell>> {
    enum Item {
        Value(u16),
        Char(char),
    }
    let mut items = Vec::new();
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        if char == '\n' {
            items.push(Item::Value(CAPTION_LINE));
            rest = &rest[1..];
        } else if char == '{' {
            let Some(Token::Tag { raw, name, value }) = tag_at(rest) else {
                return text_error(format!("a caption has a tag that is not closed: {}", quoted(rest)));
            };
            let digits = value.unwrap_or_default();
            let number = u16::from_str_radix(digits, 16).ok().filter(|_| digits.len() == 4);
            match (name, number) {
                ("PHRASE", Some(phrase)) => items.extend([Item::Value(CAPTION_PHRASE), Item::Value(phrase)]),
                ("RAW", Some(value)) => items.push(Item::Value(value)),
                ("K", Some(code)) => match caption_value_of_code(u32::from(code)) {
                    Some(value) => items.push(Item::Value(value)),
                    None => return text_error(format!("{raw} is not a character of the game's font")),
                },
                _ => return text_error(format!("a caption cannot hold the tag {raw}")),
            }
            rest = &rest[raw.len()..];
        } else {
            items.push(Item::Char(char));
            rest = &rest[char.len_utf8()..];
        }
    }

    let width = |char: char| widths.and_then(|widths| widths.get(&char)).copied();
    let own = |char: char| TABLE.codes.get(&char).copied().and_then(caption_value_of_code);
    let only_narrow =
        |item: Option<&Item>| matches!(item, Some(Item::Char(char)) if own(*char).is_none() && width(*char) == Some(8));
    // Narrow characters waiting for a partner.
    let mut run: Vec<char> = Vec::new();
    let mut out = Vec::new();
    let flush = |run: &mut Vec<char>, out: &mut Vec<CaptionCell>| {
        if run.len() % 2 == 1 {
            run.push(' ');
        }
        for pair in run.chunks(2) {
            // Two spaces are the game's own blank glyph.
            out.push(match (pair[0], pair[1]) {
                (' ', ' ') => CaptionCell::Value(CAPTION_GLYPH_BIAS),
                (left, right) => CaptionCell::New(CaptionGlyph::Pair(left, right)),
            });
        }
        run.clear();
    };
    for (index, item) in items.iter().enumerate() {
        let char = match item {
            Item::Value(value) => {
                flush(&mut run, &mut out);
                out.push(CaptionCell::Value(*value));
                continue;
            }
            Item::Char(char) => *char,
        };
        let joins = char.is_ascii() && width(char) == Some(8) && (!run.is_empty() || only_narrow(items.get(index + 1)));
        match (own(char), width(char)) {
            (Some(value), _) if !joins => {
                flush(&mut run, &mut out);
                out.push(CaptionCell::Value(value));
            }
            (_, Some(8)) => run.push(char),
            (_, Some(_)) => {
                flush(&mut run, &mut out);
                out.push(CaptionCell::New(CaptionGlyph::Wide(char)));
            }
            (_, None) => {
                let lacks = if widths.is_some() {
                    "neither the game's font nor the captions' has"
                } else {
                    "the game's font lacks"
                };
                return text_error(format!("a caption cannot hold {}, which {lacks}", quoted(&char.to_string())));
            }
        }
    }
    flush(&mut run, &mut out);
    Ok(out)
}

/// One caption from its text, without the value that ends it, for text that keeps to the game's font.
pub fn encode_caption(text: &str) -> Result<Vec<u16>> {
    let cells = caption_cells(text, None)?;
    // Without a font to take new glyphs from, every cell is a value.
    Ok(cells
        .into_iter()
        .filter_map(|cell| match cell {
            CaptionCell::Value(value) => Some(value),
            CaptionCell::New(_) => None,
        })
        .collect())
}

/// The glyph cells in a caption's values: everything but line breaks and phrases.
fn caption_glyph_values(values: &[u16]) -> Vec<u16> {
    let mut found = Vec::new();
    let mut values = values.iter().copied();
    while let Some(value) = values.next() {
        if value == CAPTION_PHRASE {
            values.next();
        } else if value != CAPTION_LINE {
            found.push(value);
        }
    }
    found
}

/// A caption pool that a pack changed, ready to go into the image.
pub struct CaptionBuild {
    /// Every caption, each ending in $FFFF.
    pub pool: Vec<u8>,
    /// The list of offsets, one for each number in the game's list of numbers.
    pub offsets: Vec<u8>,
    /// The font: 2,048 glyphs of 32 bytes.
    pub font: Vec<u8>,
    /// The captions whose content changed, by number.
    pub changed: Vec<usize>,
    /// The new glyphs, each with its number.
    pub glyphs: Vec<(usize, CaptionGlyph)>,
    /// How many of them took the number of a kanji that no caption uses any more.
    pub replaced: usize,
    /// The changed captions that take more cells than the game's own.
    pub wider: Vec<usize>,
    /// For each byte of the menu encoding, the caption value that draws it, as 256 words. Empty
    /// when the menu font is the game's own, whose table the game then keeps reading.
    pub menu_values: Vec<u8>,
    /// How many of the new glyphs are characters the menu font gained.
    pub menu_glyphs: usize,
}

/// A glyph of the game's font that a new one may take the number of, once no caption uses it: a
/// kanji, or a glyph the character table does not know, which only kanji are.
fn is_spare_when_unused(glyph: usize) -> bool {
    match TABLE.characters.get(&code_of_glyph(glyph as u32)) {
        Some(char) => matches!(*char, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}'),
        None => true,
    }
}

/// A character's rows with nothing in the top one, which the game does not draw: the two routines
/// that draw a caption's glyph ($C8:E6C3 and $C9:D4CB) put nothing of the letter in its top row and
/// use the font's top row only for the outline of the row below it. None of the game's own glyphs has
/// a dot there, but Unifont puts the accent of a capital (É, Â, Ô) in its two top rows, and a
/// circumflex drawn without its top row reads as a diaeresis. So when the top row has dots, the
/// rows above the first empty one move down a row into it, and the accent keeps all of its rows.
/// A character with nothing in its top row is left as it is, and so is one with no empty row.
fn below_top_row(rows: [u16; fonts::CELL]) -> [u16; fonts::CELL] {
    let Some(empty) = rows.iter().position(|row| *row == 0).filter(|empty| *empty > 0) else {
        return rows;
    };
    let mut moved = rows;
    moved.copy_within(..empty, 1);
    moved[0] = 0;
    moved
}

/// A new glyph as the game's font holds one: 16 rows of the left half, then 16 of the right.
///
/// The game does not draw the last two columns of a cell, so everything sits one dot to the left
/// of where the font has it: a narrow character's last column is empty, and so is a wide one's.
/// Nor does it draw the top row, so a character with dots there is moved down (`below_top_row`).
fn caption_picture(glyph: CaptionGlyph, shapes: &BTreeMap<char, Glyph>) -> [u8; GLYPH_BYTES] {
    let rows_of = |char: char| shapes.get(&char).map_or([0; fonts::CELL], |shape| below_top_row(shape.rows));
    let rows: [u16; GLYPH_SIZE] = match glyph {
        CaptionGlyph::Wide(char) => rows_of(char).map(|row| row << 1),
        CaptionGlyph::Pair(left, right) => {
            let (left, right) = (rows_of(left), rows_of(right));
            // A narrow glyph is in the top byte of its rows.
            let half = |row: u16| (row >> 8 << 1) & 0xFF;
            std::array::from_fn(|row| half(left[row]) << 8 | half(right[row]))
        }
        // Four dots in from the left to be in the middle, then one back as everything else.
        CaptionGlyph::Alone(char) => rows_of(char).map(|row| (row & 0xFF00) >> 3),
    };
    let mut picture = [0; GLYPH_BYTES];
    for (row, bits) in rows.iter().enumerate() {
        picture[row] = (bits >> 8) as u8;
        picture[GLYPH_SIZE + row] = *bits as u8;
    }
    picture
}

/// The captions with a pack's changes: the pool, the list of offsets and the font. `None` when the
/// pack's captions are the game's own and so is the menu font, so that nothing of the game is touched.
///
/// `menu` is each character the pack added to the menu font, with its byte in the menu encoding.
/// The game also draws menu strings and names in the captions' font (the name of the item being
/// described, a name a battle caption starts with), one character to a cell, so each of those
/// characters gets a glyph here too.
pub fn build_captions(rom: &[u8], items: &Strings, menu: &[(char, u8)]) -> Result<Option<CaptionBuild>> {
    let own = caption_strings(rom)?;
    // The characters to look for in the captions' font: those the game's lacks, and those that
    // could be either.
    let mut wanted: BTreeSet<char> = BTreeSet::from([' ']);
    for text in items.values() {
        wanted.extend(
            plain_characters(text).into_iter().filter(|char| char.is_ascii() || !TABLE.codes.contains_key(char)),
        );
    }
    for (char, _) in menu {
        wanted.insert(*char);
        wanted.extend(full_width(*char));
    }
    let wanted: Vec<char> = wanted.into_iter().collect();
    let font = Font::built_ins(&[CAPTION_FONT_NAME]);
    let shapes = match font.first() {
        Some(font) => fonts::load_unifont(font, &wanted, true)?,
        None => BTreeMap::new(),
    };
    let widths: BTreeMap<char, usize> = shapes.iter().map(|(char, shape)| (*char, shape.width)).collect();

    let mut changed: BTreeMap<usize, Vec<CaptionCell>> = BTreeMap::new();
    for (number, text) in items {
        let Some(original) = own.get(*number as usize) else {
            let count = own.len();
            return text_error(format!("string captions.{number:03} does not exist: the game has {count} captions"));
        };
        let cells = match caption_cells(text, Some(&widths)) {
            Ok(cells) => cells,
            Err(Error::Text(message)) => return text_error(format!("string captions.{number:03}: {message}")),
            Err(other) => return Err(other),
        };
        let same =
            cells.iter().map(|cell| Some(*cell)).eq(original.iter().map(|value| Some(CaptionCell::Value(*value))));
        if !same {
            changed.insert(*number as usize, cells);
        }
    }
    if changed.is_empty() && menu.is_empty() {
        return Ok(None);
    }

    // A character the menu font gained has a cell to itself here. The game's own glyph is used
    // when it has one; then the character's full-width form, which is what the game's own Latin
    // capitals are; then the character as its font has it, in the middle of the cell if narrow.
    let menu_cells: Vec<(u8, CaptionCell)> = menu
        .iter()
        .map(|(char, byte)| {
            let own = TABLE.codes.get(char).copied().and_then(caption_value_of_code);
            let wide = full_width(*char).filter(|wide| shapes.contains_key(wide));
            let cell = match (own, wide, widths.get(char)) {
                (Some(value), _, _) => CaptionCell::Value(value),
                (None, Some(wide), _) => CaptionCell::New(CaptionGlyph::Wide(wide)),
                (None, None, Some(8)) => CaptionCell::New(CaptionGlyph::Alone(*char)),
                (None, None, Some(_)) => CaptionCell::New(CaptionGlyph::Wide(*char)),
                // A character the captions' font lacks leaves its cell empty.
                (None, None, None) => CaptionCell::Value(CAPTION_GLYPH_BIAS),
            };
            (*byte, cell)
        })
        .collect();
    let menu_new: BTreeSet<CaptionGlyph> = menu_cells
        .iter()
        .filter_map(|(_, cell)| match cell {
            CaptionCell::New(glyph) => Some(*glyph),
            CaptionCell::Value(_) => None,
        })
        .collect();

    // The game's glyphs that a caption still uses keep their numbers. So do all that are not kanji:
    // the game draws names and numbers through the same routines.
    let glyph_of =
        |value: u16| value.checked_sub(CAPTION_GLYPH_BIAS).map(usize::from).filter(|glyph| *glyph < GLYPH_COUNT);
    let mut used: BTreeSet<usize> = BTreeSet::new();
    for (number, original) in own.iter().enumerate() {
        match changed.get(&number) {
            None => used.extend(caption_glyph_values(original).into_iter().filter_map(glyph_of)),
            Some(cells) => {
                let values: Vec<u16> = cells
                    .iter()
                    .filter_map(|cell| match cell {
                        CaptionCell::Value(value) => Some(*value),
                        CaptionCell::New(_) => None,
                    })
                    .collect();
                used.extend(caption_glyph_values(&values).into_iter().filter_map(glyph_of));
            }
        }
    }
    let new: BTreeSet<CaptionGlyph> = changed
        .values()
        .flatten()
        .filter_map(|cell| match cell {
            CaptionCell::New(glyph) => Some(*glyph),
            CaptionCell::Value(_) => None,
        })
        .chain(menu_new.iter().copied())
        .collect();
    // A glyph the menu's bytes are drawn with is in use too, whatever the captions hold.
    used.extend(menu_cells.iter().filter_map(|(_, cell)| match cell {
        CaptionCell::Value(value) => glyph_of(*value),
        CaptionCell::New(_) => None,
    }));
    let spare = (0..GLYPH_COUNT).filter(|glyph| is_spare_when_unused(*glyph) && !used.contains(glyph));
    let free: Vec<usize> = (GLYPH_COUNT..CAPTION_GLYPHS).chain(spare).collect();
    if new.len() > free.len() {
        return text_error(format!(
            "the captions need {} new glyphs, and their font has room for {}: the {} numbers after the game's own \
             glyphs, and those of the kanji that no caption uses any more",
            new.len(),
            free.len(),
            CAPTION_GLYPHS - GLYPH_COUNT
        ));
    }
    let glyphs: Vec<(usize, CaptionGlyph)> = free.iter().copied().zip(new.iter().copied()).collect();
    let number_of: BTreeMap<CaptionGlyph, usize> = glyphs.iter().map(|(number, glyph)| (*glyph, *number)).collect();

    let mut strings = own.clone();
    let mut wider = Vec::new();
    for (number, cells) in &changed {
        let values: Vec<u16> = cells
            .iter()
            .map(|cell| match cell {
                CaptionCell::Value(value) => *value,
                // Every new glyph was given a number above.
                CaptionCell::New(glyph) => number_of.get(glyph).copied().unwrap_or(0) as u16 + CAPTION_GLYPH_BIAS,
            })
            .collect();
        if caption_glyph_values(&values).len() > caption_glyph_values(&own[*number]).len() {
            wider.push(*number);
        }
        strings[*number] = values;
    }

    let mut pool = Vec::new();
    let mut starts = Vec::new();
    for string in &strings {
        starts.push(pool.len());
        pool.extend(string.iter().chain([&CAPTION_END]).flat_map(|value| value.to_le_bytes()));
    }
    let room = (CAPTION_NUMBERS & 0xFFFF) - CAPTION_BASE;
    if pool.len() > room {
        return text_error(format!(
            "the captions are {} bytes too long for the bank they are kept in, which holds {room}",
            pool.len() - room
        ));
    }

    // Each of the game's offsets is the start of one of its strings, and becomes that string's new start.
    let mut old_starts: HashMap<usize, usize> = HashMap::new();
    let mut at = 0;
    for (number, string) in own.iter().enumerate() {
        old_starts.insert(at, number);
        at += 2 * (string.len() + 1);
    }
    let word =
        |offset: usize| rom.get(offset..offset + 2).map(|pair| usize::from(u16::from_le_bytes([pair[0], pair[1]])));
    let mut offsets = Vec::new();
    for index in 0..(CAPTION_OFFSETS - CAPTION_NUMBERS) / 2 {
        if word(CAPTION_NUMBERS + 2 * index).is_none_or(|number| number == 0) {
            break;
        }
        let Some(number) = word(CAPTION_OFFSETS + 2 * index).and_then(|old| old_starts.get(&old)) else {
            return Err(Error::Image("the list of the captions' places does not match the captions".into()));
        };
        offsets.extend((starts[*number] as u16).to_le_bytes());
    }

    let mut font = vec![0; CAPTION_GLYPHS * GLYPH_BYTES];
    let Some(game_font) = rom.get(FONT..FONT + GLYPH_COUNT * GLYPH_BYTES) else {
        return Err(Error::Image("the image is too short to hold the font".into()));
    };
    font[..game_font.len()].copy_from_slice(game_font);
    for (number, glyph) in &glyphs {
        font[number * GLYPH_BYTES..(number + 1) * GLYPH_BYTES].copy_from_slice(&caption_picture(*glyph, &shapes));
    }
    let replaced = glyphs.iter().filter(|(number, _)| *number < GLYPH_COUNT).count();

    // The game's table with the menu font's new characters put in: a value is a glyph's number plus
    // the bias, as in a caption.
    let mut menu_values = Vec::new();
    if !menu_cells.is_empty() {
        let table = snes::snes_to_pc(NAME_GLYPH_TABLE, MAPPING)?;
        let Some(game_table) = rom.get(table..table + 256) else {
            return Err(Error::Image("the image is too short to hold the table of the menu's characters".into()));
        };
        let mut values: Vec<u16> = game_table.iter().map(|glyph| u16::from(*glyph) + CAPTION_GLYPH_BIAS).collect();
        for (byte, cell) in &menu_cells {
            values[usize::from(*byte)] = match cell {
                CaptionCell::Value(value) => *value,
                CaptionCell::New(glyph) => number_of.get(glyph).copied().unwrap_or(0) as u16 + CAPTION_GLYPH_BIAS,
            };
        }
        menu_values = values.into_iter().flat_map(u16::to_le_bytes).collect();
    }
    Ok(Some(CaptionBuild {
        pool,
        offsets,
        font,
        changed: changed.into_keys().collect(),
        glyphs,
        replaced,
        wider,
        menu_values,
        menu_glyphs: menu_new.len(),
    }))
}

/// Put a changed caption pool, its lists and its font in the expanded area, and make the game read
/// them there.
///
/// No code is added. The game names the pool, the lists' bank and the font's bank as plain numbers
/// in its own instructions, and those numbers are changed: see the notes at the top. Where the menu
/// font gained characters, the two places that turn a menu byte into a caption value are rewritten
/// within their own 14 bytes to read a table of words instead of the game's table of bytes.
pub fn install_captions(rom: Vec<u8>, captions: &CaptionBuild) -> Result<Vec<u8>> {
    let mut out = expanded(rom)?;
    let pool_bank = (cpu_address(CAPTION_BANK)? >> 16) as u8;
    let font_bank = (cpu_address(CAPTION_FONT)? >> 16) as u8;
    let base = (CAPTION_BASE as u16).to_le_bytes();
    // The CPU address of each number, what the game has there, and what it is given.
    let patches: [(u32, &[u8], &[u8]); 14] = [
        // The four routines that look a caption up set the data bank for the lists.
        (0xC2B554, &[0xEF], &[pool_bank]),
        (0xC9D04F, &[0xEF], &[pool_bank]),
        (0xC9D40D, &[0xEF], &[pool_bank]),
        (0xC9D83C, &[0xEF], &[pool_bank]),
        // The pool's address: in full, as a pointer's low word and its bank, as a 16-bit address in
        // the data bank (three times), and the bank of the pointer one caller makes of that.
        (0xC2B262, &[0x7C, 0x99, 0xEF], &[base[0], base[1], pool_bank]),
        (0xC8EE04, &[0x7C, 0x99], &base),
        (0xC8EE0B, &[0xEF], &[pool_bank]),
        (0xC9D06D, &[0x7C, 0x99], &base),
        (0xC9D42A, &[0x7C, 0x99], &base),
        (0xC9D859, &[0x7C, 0x99], &base),
        (0xC1D32B, &[0xEF], &[pool_bank]),
        // The font's bank, where the two routines that draw a caption's glyph set it: one of them
        // twice, for a glyph's left half and its right.
        (0xC8E6D8, &[0xF0], &[font_bank]),
        (0xC8E785, &[0xF0], &[font_bank]),
        (0xC9D4D3, &[0xF0], &[font_bank]),
    ];
    for (address, game, new) in patches {
        let at = snes::snes_to_pc(address, MAPPING)?;
        if out.get(at..at + game.len()) != Some(game) {
            return Err(Error::Image(format!("the game's code at ${address:06X} is not what reads the captions")));
        }
        out[at..at + new.len()].copy_from_slice(new);
    }
    let lists = out[CAPTION_NUMBERS..CAPTION_OFFSETS].to_vec();
    let put = |out: &mut Vec<u8>, at: usize, data: &[u8]| out[at..at + data.len()].copy_from_slice(data);
    put(&mut out, CAPTION_BANK + CAPTION_BASE, &captions.pool);
    put(&mut out, CAPTION_BANK + (CAPTION_NUMBERS & 0xFFFF), &lists);
    put(&mut out, CAPTION_BANK + (CAPTION_OFFSETS & 0xFFFF), &captions.offsets);
    put(&mut out, CAPTION_FONT, &captions.font);

    if !captions.menu_values.is_empty() {
        let table = cpu_address(CAPTION_MENU_VALUES)?;
        // Push X, double the byte since the table holds words, X = that, read the value, and fill
        // what is left before X is pulled with instructions that do nothing.
        let mut reader = vec![0xDA, 0x0A, 0xAA, 0xBF, table as u8, (table >> 8) as u8, (table >> 16) as u8];
        reader.resize(CAPTION_MENU_READER.len() - 1, 0xEA);
        reader.push(0xFA);
        for address in CAPTION_MENU_READERS {
            let at = snes::snes_to_pc(address, MAPPING)?;
            if out.get(at..at + CAPTION_MENU_READER.len()) != Some(&CAPTION_MENU_READER) {
                let message = format!("the game's code at ${address:06X} is not what reads the menu's characters");
                return Err(Error::Image(message));
            }
            put(&mut out, at, &reader);
        }
        put(&mut out, CAPTION_MENU_VALUES, &captions.menu_values);
    }
    Ok(out)
}

/// Put compressed blocks in the expanded area and make the decompressor look for them there.
///
/// `blocks` maps the file offset of an original block to its replacement.
pub fn install_moved_blocks(rom: Vec<u8>, blocks: &BTreeMap<usize, Vec<u8>>) -> Result<Vec<u8>> {
    let mut out = expanded(rom)?;
    let mut space = BankAllocator::new(MOVED_AREA.0, MOVED_AREA.1);
    let mut table = Vec::new();
    for (offset, block) in blocks {
        let Ok(target) = space.allocate(block.len()) else {
            return text_error("no room left in the ROM for a block that had to move");
        };
        out[target..target + block.len()].copy_from_slice(block);
        for address in [snes::pc_to_snes(*offset, MAPPING)?, snes::pc_to_snes(target, MAPPING)?] {
            let [low, high, bank, _] = address.to_le_bytes();
            table.extend([bank, low, high]);
        }
    }
    table.push(0);
    out[MOVED_TABLE..MOVED_TABLE + table.len()].copy_from_slice(&table);
    let symbols = [("MOVED_CODE", cpu_address(MOVED_CODE)?), ("MOVED", cpu_address(MOVED_TABLE)?)];
    snes::write_code(&out, &assemble(include_str!("moved.asm"), &symbols)?.segments, MAPPING)
}

/// Write moved menu strings and hook the game's menu readers so that they follow the pointers to
/// them. `moved` holds each string with the file offset it goes to.
pub fn install_menu_strings(rom: Vec<u8>, moved: &[(usize, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut out = expanded(rom)?;
    for (target, data) in moved {
        out[*target..target + data.len()].copy_from_slice(data);
    }
    let symbols = [("MENU_CODE", cpu_address(MENU_CODE)?), ("MENU_BANK", cpu_address(MENU_AREA.0)? & 0xFF0000)];
    snes::write_code(&out, &assemble(include_str!("menu.asm"), &symbols)?.segments, MAPPING)
}

/// Which menu string holds each party member's default name, by tag.
pub fn name_string_numbers(rom: &[u8]) -> Result<Vec<(&'static str, usize)>> {
    let strings = menu_strings(rom);
    let mut numbers = Vec::new();
    for (member, record) in DEFAULT_NAME_RECORDS {
        let name = &record[..record.len() - 1];
        let found = strings
            .iter()
            .position(|(offset, data)| *data == name && (NAME_STRINGS.0..NAME_STRINGS.1).contains(offset));
        let Some(number) = found else {
            let message = format!("the menu text does not hold the game's own name for {}", speaker(member));
            return Err(Error::Image(message));
        };
        numbers.push((speaker(member), number));
    }
    Ok(numbers)
}

/// Write the name hook and its tables.
///
/// `translated` maps a tag to the glyph numbers of the name to show in dialogue, `records` a tag to
/// the default name record a new game now starts with (where the pack changed it), and
/// `glyph_numbers` a menu byte to the dialogue glyph that now stands for it (where the pack changed
/// the menu font).
pub fn install_names(
    rom: Vec<u8>,
    translated: &HashMap<&str, Vec<usize>>,
    records: &HashMap<&str, Vec<u8>>,
    glyph_numbers: &BTreeMap<u8, usize>,
) -> Result<Vec<u8>> {
    let table = snes::snes_to_pc(NAME_GLYPH_TABLE, MAPPING)?;
    let original: Vec<u8> = rom[table..table + 256].to_vec();
    let mut out = expanded(rom)?;
    let mut defaults = Vec::new();
    let mut names = Vec::new();
    // What is shorter than its slot is filled out with zeros. Nothing longer reaches here: a name is
    // refused before, and a record is at most the seven characters the menus hold and its end.
    let padded = |mut data: Vec<u8>, size: usize| {
        data.resize(data.len().max(size), 0);
        data
    };
    for (member, record) in DEFAULT_NAME_RECORDS {
        let tag = speaker(member);
        defaults.extend(padded(records.get(tag).cloned().unwrap_or_else(|| record.to_vec()), 8));
        let mut words = Vec::new();
        if let Some(glyphs) = translated.get(tag).filter(|glyphs| !glyphs.is_empty()) {
            for glyph in glyphs {
                words.extend(((glyph + NAME_GLYPH_BIAS) as u16).to_le_bytes());
            }
            words.extend([0xFF, 0xFF]);
        }
        names.extend(padded(words, NAME_SLOT));
    }
    out[NAME_DEFAULTS..NAME_DEFAULTS + defaults.len()].copy_from_slice(&defaults);
    out[NAME_TRANSLATED..NAME_TRANSLATED + names.len()].copy_from_slice(&names);
    for (byte, own) in (0..=255u8).zip(&original) {
        let glyph = glyph_numbers.get(&byte).copied().unwrap_or(usize::from(*own));
        let at = NAME_GLYPHS + 2 * usize::from(byte);
        out[at..at + 2].copy_from_slice(&((glyph + NAME_GLYPH_BIAS) as u16).to_le_bytes());
    }
    let symbols = [
        ("NAME_CODE", cpu_address(NAME_CODE)?),
        ("NAME_DEFAULTS", cpu_address(NAME_DEFAULTS)?),
        ("NAME_DEFAULTS_SIZE", defaults.len() as i64),
        ("NAME_TRANSLATED", cpu_address(NAME_TRANSLATED)?),
        ("NAME_GLYPHS", cpu_address(NAME_GLYPHS)?),
    ];
    snes::write_code(&out, &assemble(include_str!("names.asm"), &symbols)?.segments, MAPPING)
}

/// Build the renderer's font: the game's own glyphs, then one glyph per extra character.
///
/// Returns the advance widths, the glyph data, and the code assigned to each extra character.
pub fn build_font(rom: &[u8], extra: &BTreeMap<char, Glyph>) -> Result<(Vec<u8>, Vec<u8>, Extra)> {
    let mut widths = Vec::new();
    let mut data = Vec::new();
    // Add a glyph, and return how many there are now.
    let mut add = |rows: &[u16; GLYPH_SIZE], advance: usize| {
        widths.push(advance as u8);
        for (pixels, edge) in rows.iter().zip(fonts::outline(rows)) {
            data.extend(pixels.to_le_bytes());
            data.extend(edge.to_le_bytes());
        }
        widths.len()
    };
    let mut count = 0;
    for index in 0..GLYPH_COUNT {
        count = add(&glyph(rom, index), ORIGINAL_ADVANCE);
    }
    let mut codes = Extra::new();
    for (char, shape) in extra {
        codes.insert(*char, count + FIRST_GLYPH_CODE);
        let (rows, advance) = fonts::proportional(shape);
        count = add(&rows, advance);
    }
    if data.len() > RENDERER_FONT_END - RENDERER_FONT {
        return text_error("too many different characters for the space reserved for the font");
    }
    Ok((widths, data, codes))
}

/// The font for dialogue the game shows as sprites: the game's own glyphs, then one for each extra
/// character, at the numbers the renderer's font has them.
///
/// The sprites are 14 dots apart whatever the character, so a new glyph sits in the middle of
/// those 14. A character whose number is past what the routine reaches is left out.
pub fn build_sprite_font(rom: &[u8], extra: &BTreeMap<char, Glyph>) -> Result<Vec<u8>> {
    let Some(game_font) = rom.get(FONT..FONT + GLYPH_COUNT * GLYPH_BYTES) else {
        return Err(Error::Image("the image is too short to hold the font".into()));
    };
    let mut font = vec![0; CAPTION_GLYPHS * GLYPH_BYTES];
    font[..game_font.len()].copy_from_slice(game_font);
    for (number, shape) in (GLYPH_COUNT..CAPTION_GLYPHS).zip(extra.values()) {
        let (rows, advance) = fonts::proportional(shape);
        let shift = ORIGINAL_ADVANCE.saturating_sub(advance) / 2;
        for (row, bits) in rows.iter().enumerate() {
            let bits = bits >> shift;
            font[number * GLYPH_BYTES + row] = (bits >> 8) as u8;
            font[number * GLYPH_BYTES + GLYPH_SIZE + row] = bits as u8;
        }
    }
    Ok(font)
}

/// Write the variable-width renderer and its font into the expanded area and hook it in, and give
/// the routine that draws dialogue as sprites its font.
pub fn install_renderer(rom: Vec<u8>, widths: &[u8], glyph_data: &[u8], sprite_font: &[u8]) -> Result<Vec<u8>> {
    let mut out = expanded(rom)?;
    out[RENDERER_WIDTHS..RENDERER_WIDTHS + widths.len()].copy_from_slice(widths);
    out[RENDERER_FONT..RENDERER_FONT + glyph_data.len()].copy_from_slice(glyph_data);
    out[SPRITE_FONT..SPRITE_FONT + sprite_font.len()].copy_from_slice(sprite_font);
    let bank_at = snes::snes_to_pc(SPRITE_FONT_BANK_AT, MAPPING)?;
    if out.get(bank_at) != Some(&0xF0) {
        return Err(Error::Image("the game's code at $C8E9AB is not what draws dialogue as sprites".into()));
    }
    out[bank_at] = (cpu_address(SPRITE_FONT)? >> 16) as u8;
    let symbols = [
        ("CODE", cpu_address(RENDERER_CODE)?),
        ("WIDTHS", cpu_address(RENDERER_WIDTHS)?),
        ("FONT_BANK", cpu_address(RENDERER_FONT)? >> 16),
    ];
    snes::write_code(&out, &assemble(include_str!("vwf.asm"), &symbols)?.segments, MAPPING)
}

/// Break lines that are too wide for the dialogue window, at spaces (see `layout::wrap_lines`).
///
/// `advance` gives the width of a character in pixels and `names` the width of each character's
/// name, by tag.
pub fn wrap_lines(
    text: &str,
    advance: &dyn Fn(char) -> Result<usize>,
    limit: usize,
    names: &[(&str, usize)],
) -> Result<String> {
    layout::wrap_lines(text, advance, limit, &|tag, _| tag_width(tag, names), layout::PAGE_TAGS)
}

/// A string the game shows as sprites (see `SPRITE_LINES`), laid out for it: rows of 15 characters
/// at most, broken at spaces unless `wrap` is off, and every space the game's own.
///
/// `names` gives the length of each character's name, by tag. Fails with `Error::Text` when a
/// row is longer than the game holds, which a word of more than 16 letters makes it, or when a
/// page has more drawn characters than the game can show.
///
/// A string with no space in it is written in a script that has none, as the game's own are, and
/// its rows are left to the game, which starts a new one after every 16th character. That is how
/// the game's own strings rebuild to what they were.
pub fn sprite_line(text: &str, wrap: bool, names: &[(&str, usize)]) -> Result<String> {
    let in_words = text.contains(' ');
    // How many characters a tag's text takes: a name is known, a number or a string is a guess,
    // and anything else is not drawn.
    let cells = |tag: &str| match names.iter().find(|(name, _)| *name == tag) {
        Some((_, length)) => *length,
        None => tag_width(tag, &[]) / ORIGINAL_ADVANCE,
    };
    let laid_out = if wrap {
        layout::wrap_lines(text, &|_| Ok(1), SPRITE_ROW, &|tag, _| cells(tag), layout::PAGE_TAGS)?
    } else {
        text.to_string()
    };
    let (mut row, mut drawn, mut page) = (0, 0, 1);
    let mut rest = laid_out.as_str();
    while let Some(char) = rest.chars().next() {
        let mut step = char.len_utf8();
        if char == '\n' {
            row = 0;
        } else if char == '{'
            && let Some(Token::Tag { raw, name, .. }) = tag_at(rest)
        {
            step = raw.len();
            if layout::PAGE_TAGS.contains(&name) {
                (row, drawn, page) = (0, 0, page + 1);
            } else {
                row += cells(name);
                drawn += cells(name);
            }
        } else {
            row += 1;
            drawn += usize::from(char != ' ' && char != GAME_SPACE);
        }
        if in_words && row > SPRITE_ROW_FULL {
            return text_error(format!(
                "the game shows this string as sprites, {SPRITE_ROW_FULL} to a row, and a row of page {page} is longer: \
                 the game would cut it wherever the row ends"
            ));
        }
        if drawn > SPRITE_PAGE {
            return text_error(format!(
                "the game shows this string as sprites, and page {page} has more than the {SPRITE_PAGE} characters a \
                 page can show, spaces not counted: put a {{PAGE}} before that, or say it in fewer letters"
            ));
        }
        rest = &rest[step..];
    }
    Ok(laid_out.replace(' ', &GAME_SPACE.to_string()))
}

/// A party member's name as a place name meets it (see `PLACE_NAMES`).
pub struct PlaceNameName<'a> {
    pub tag: &'a str,
    /// Characters of the name in the character's record in work RAM, the name the menus show,
    /// which is what the game counts.
    pub record: usize,
    /// The name dialogue draws, which is what takes room on the line.
    pub drawn: &'a str,
}

/// How many characters the game counts in a place name, from its bytes (`data`, end marker
/// included), as the code at $C8:DB51 counts them. `record` gives the length of the name in the
/// record of the party member of a number, where that is known.
///
/// Every character or code counts one, whatever its width, and a character of two bytes counts one.
/// A name, code $02 and the member's number, counts the characters of that member's record; the
/// engine then reads on from the member's number, not after it, and counts that byte as a character
/// too. So a name counts as its letters and one more. Mint's number is $02 itself, so after her name
/// the engine takes her number for another name code and the byte after it for a member's number:
/// it counts whatever is in that record, or in the memory where it would be. That is not known, so it
/// is counted as nothing, the least it can be, and so is a name of any number this game has no
/// record for.
///
/// A number, `{NUM:..}`, is four bytes, two characters of two bytes for this count: it counts two,
/// whatever number it shows.
pub fn place_name_count(data: &[u8], record: &dyn Fn(u8) -> Option<usize>) -> usize {
    const NAME: u8 = 0x02;
    let body = data.strip_suffix(&END).unwrap_or(data);
    let (mut count, mut at) = (0, 0);
    while at < body.len() {
        let byte = body[at];
        if byte == NAME {
            count += body.get(at + 1).and_then(|member| record(*member)).unwrap_or(0);
            at += 1;
        } else {
            count += 1;
            at += if byte >= 0x80 { 2 } else { 1 };
        }
    }
    count
}

/// Hold a place name (see `PLACE_NAMES`) to what its box shows. Fails with `Error::Text` when the
/// game would count more than 16 characters in it, or when, centred, a character of it would start
/// past the end of the line.
///
/// `extra` gives the codes of the characters the pack adds, `advance` the width of a character in
/// dots, and `names` each party member's name as the game counts it and as it draws it.
///
/// A number, `{NUM:..}`, takes the room of one of the game's digits, the least it can: what it shows
/// is not known until the game runs. A character of the game's font with no entry in the table yet,
/// `{K:..}`, takes the 14 dots of the game's own characters. Another tag takes no room, `{RAW:..}`
/// included, which is for bytes the game's own text has and nothing a translator writes. A line break
/// is counted as the game counts it, and takes no room; what the box does with one has not been seen.
pub fn place_name(
    text: &str,
    extra: &Extra,
    advance: &dyn Fn(char) -> Result<usize>,
    names: &[PlaceNameName],
) -> Result<()> {
    let record = |member: u8| {
        let tag = tag_of(&SPEAKERS, member)?;
        names.iter().find(|name| name.tag == tag).map(|name| name.record)
    };
    let count = place_name_count(&encode(text, extra)?, &record);
    if count > PLACE_NAME_LENGTH {
        return text_error(format!(
            "the game shows this string as the name of a place, on one line of at most {PLACE_NAME_LENGTH} \
             characters, and it has {count}: a party member's name counts as the letters the menus show for it, \
             and one more"
        ));
    }
    // The width of each character drawn, in order.
    let mut widths = Vec::new();
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        let mut step = char.len_utf8();
        if char == '{'
            && let Some(Token::Tag { raw, name, value }) = tag_at(rest)
        {
            step = raw.len();
            // A name may also be written by the member's number, as {NAME:01}.
            let member = value.and_then(|value| u8::from_str_radix(&value.replace(' ', ""), 16).ok());
            let tag = member.and_then(|member| tag_of(&SPEAKERS, member)).filter(|_| name == "NAME").unwrap_or(name);
            if let Some(shown) = names.iter().find(|shown| shown.tag == tag) {
                for char in shown.drawn.chars() {
                    widths.push(advance(char)?);
                }
            } else if name == "NUM" || name == "K" {
                widths.push(ORIGINAL_ADVANCE);
            }
        } else if char != '\n' {
            widths.push(advance(char)?);
        }
        rest = &rest[step..];
    }
    let spaces = (PLACE_NAME_LENGTH - count) / 2;
    let before_last: usize = widths.iter().rev().skip(1).sum();
    let last = ORIGINAL_ADVANCE * spaces + before_last;
    if !widths.is_empty() && last >= LINE_WIDTH {
        return text_error(format!(
            "the game shows this string as the name of a place, centred on one line, and its last character would \
             start at dot {last}, where the line ends at dot {LINE_WIDTH}: use fewer or narrower letters"
        ));
    }
    Ok(())
}

/// The room a tag's text takes. Values are only known when the game runs, so they are estimates;
/// a name is measured, though a player who renames a character may make it longer.
fn tag_width(tag: &str, names: &[(&str, usize)]) -> usize {
    if let Some((_, width)) = names.iter().find(|(name, _)| *name == tag) {
        return *width;
    }
    match tag {
        "NAME" => 5 * ORIGINAL_ADVANCE,
        "NUM" => 4 * ORIGINAL_ADVANCE,
        "STR" => 8 * ORIGINAL_ADVANCE,
        _ => 0,
    }
}

/// The font tiles a menu string draws: its own bytes, or for an accented kana the plain kana and
/// the mark.
fn menu_tiles(rom: &[u8], data: &[u8]) -> Result<BTreeSet<u8>> {
    let mut tiles = BTreeSet::new();
    for byte in data.iter().copied() {
        if byte < MENU_ACCENTED {
            tiles.insert(byte);
        } else if byte < MENU_LINE {
            tiles.insert(base_tile(rom, byte)?);
            tiles.insert(MENU_MARKS[usize::from(byte >= MENU_ACCENTED_2)]);
        }
    }
    Ok(tiles)
}

/// The tile drawn on the text row for a menu byte.
fn base_tile(rom: &[u8], byte: u8) -> Result<u8> {
    if !(MENU_ACCENTED..MENU_LINE).contains(&byte) {
        return Ok(byte);
    }
    if byte < MENU_ACCENTED_2 {
        return Ok(rom[snes::snes_to_pc(MENU_BASE_TABLE, MAPPING)? + usize::from(byte - MENU_ACCENTED)]);
    }
    Ok(rom[snes::snes_to_pc(MENU_BASE_TABLE_2, MAPPING)? + usize::from(byte - MENU_ACCENTED_2)])
}

/// Map plain capitals, digits, punctuation and spaces to the menu font's own, which are full-width
/// here.
fn normalise_menu(text: &str, codes: &HashMap<char, u8>) -> String {
    let mut out = String::new();
    for token in tokens(text) {
        match token {
            Token::Char(' ') => out.push('\u{3000}'),
            Token::Char(char @ '!'..='~') => {
                let wide = char::from_u32(char as u32 + 0xFEE0).filter(|wide| codes.contains_key(wide));
                out.push(wide.unwrap_or(char));
            }
            Token::Char(char) => out.push(char),
            Token::Tag { raw, .. } => out.push_str(raw),
        }
    }
    out
}

/// What was done to the menu text.
#[derive(Clone, Debug, Default)]
struct MenuBuild {
    /// Numbers of the strings changed.
    changed: Vec<usize>,
    /// Characters added to the menu font.
    new_characters: Vec<char>,
    /// Rebuilt font copies, by original block offset.
    font_blocks: BTreeMap<usize, Vec<u8>>,
    /// Strings that moved, with the offset they go to.
    moved: Vec<(usize, Vec<u8>)>,
    /// New character and the menu byte it was given, in the order of the characters.
    taken: Vec<(char, u8)>,
    /// Changed string -> its bytes, without terminator.
    encoded: BTreeMap<usize, Vec<u8>>,
}

/// A menu string's address as it is written in a script file.
fn menu_name(number: impl std::fmt::Display) -> String {
    format!("{MENU_SECTION}.{number:03}")
}

/// Rewrite the menu strings a pack changes, adding to the menu font the characters it lacks.
///
/// A string that fits its slot is rewritten in place. A longer one moves to the expanded area and
/// its slot is turned into a pointer to it. The menu font has 192 tiles, so a new character needs
/// the tile of a kana that no remaining string uses.
fn build_menu(rom: &[u8], out: &mut [u8], items: &Strings, menu_font: Option<&Font>) -> Result<MenuBuild> {
    let mut result = MenuBuild::default();
    if items.is_empty() {
        return Ok(result);
    }
    let chars = menu_characters(rom)?;
    let mut codes = menu_codes(&chars);
    let originals = menu_strings(rom);
    let mut replaced: BTreeMap<usize, String> = BTreeMap::new();
    for (number, text) in items {
        let Some((_, original)) = originals.get(*number as usize) else {
            return text_error(format!("string {} does not exist: there are {}", menu_name(number), originals.len()));
        };
        let text = normalise_menu(text, &codes);
        if text != decode_menu(original, &chars) {
            replaced.insert(*number as usize, text);
        }
    }
    if replaced.is_empty() {
        return Ok(result);
    }

    let wanted: BTreeSet<char> = replaced.values().flat_map(|text| plain_characters(text)).collect();
    let needed: Vec<char> = wanted.iter().copied().filter(|char| !codes.contains_key(char)).collect();
    if !needed.is_empty() {
        // With no font given, the game's own for its menus.
        let own_font = fonts::built_in(MENU_FONT).map(Font::BuiltIn);
        let Some(menu_font) = menu_font.or(own_font.as_ref()) else {
            let shown = needed.iter().take(12).map(char::to_string).collect::<Vec<_>>().join(" ");
            return text_error(format!(
                "the pack's menu text uses {} characters the menu font lacks ({shown}); an 8x8 font is needed to \
                 draw them",
                needed.len()
            ));
        };
        // Tiles still in use: by every string the pack leaves alone, and by the game's characters it
        // uses itself.
        let mut kept = BTreeSet::from(MENU_MARKS);
        for (number, (_, data)) in originals.iter().enumerate() {
            if !replaced.contains_key(&number) {
                kept.extend(menu_tiles(rom, data)?);
            }
        }
        for byte in wanted.iter().filter_map(|char| codes.get(char)) {
            kept.insert(base_tile(rom, *byte)?);
        }
        let kana = |char: char| ('\u{3040}'..='\u{30FF}').contains(&char);
        let free: Vec<u8> = chars
            .iter()
            .filter(|(byte, char)| **byte < MENU_ACCENTED && kana(**char) && !kept.contains(byte))
            .map(|(byte, _)| *byte)
            .collect();
        if needed.len() > free.len() {
            let shown: String = needed.iter().take(40).collect();
            return text_error(format!(
                "the menu font has room for {} new characters and the pack needs {} ({shown}); a tile only comes \
                 free once every menu string that uses its kana is replaced",
                free.len(),
                needed.len()
            ));
        }
        let tiles = match fonts::load_tiles(menu_font, &needed) {
            Ok(tiles) => tiles,
            Err(Error::Font(message)) => return text_error(message),
            Err(other) => return Err(other),
        };
        let taken: Vec<(char, u8)> = needed.iter().copied().zip(free).collect();
        // A kana that gives up its tile can no longer be written, and neither can its accented forms.
        let gone: BTreeSet<u8> = taken.iter().map(|(_, byte)| *byte).collect();
        let mut still = HashMap::new();
        for (char, byte) in codes {
            if !gone.contains(&base_tile(rom, byte)?) {
                still.insert(char, byte);
            }
        }
        still.extend(taken.iter().copied());
        codes = still;
        for offset in MENU_FONT_BLOCKS {
            let mut font = lzss_rle::decompress_block(rom, offset)?;
            for (char, byte) in &taken {
                let at = usize::from(*byte) * 16;
                let Some(tile) = font.get_mut(at..at + 16) else {
                    return Err(Error::Image("the menu font holds fewer tiles than the menu text can name".into()));
                };
                tile.copy_from_slice(&fonts::shaded_tile(&tiles[char], 3));
            }
            result.font_blocks.insert(offset, lzss_rle::compress_block(&font)?);
        }
        result.taken = taken;
    }

    let mut space = BankAllocator::new(MENU_AREA.0, MENU_AREA.1);
    for (number, text) in &replaced {
        let (offset, original) = originals[*number];
        let slot = original.len() + 1;
        let data = match encode_menu(text, &codes) {
            Ok(data) => data,
            Err(Error::Text(message)) => return text_error(format!("string {}: {message}", menu_name(number))),
            Err(other) => return Err(other),
        };
        // What the slot holds from now on, the rest of it filled with terminators.
        let mut fill = |start: &[u8]| {
            out[offset..offset + slot].fill(0xFF);
            out[offset..offset + start.len()].copy_from_slice(start);
        };
        if data.len() < slot {
            fill(&data);
        } else if data.len() > MENU_MAX_LENGTH {
            return text_error(format!(
                "string {} is {} characters; the longest a menu string can be is {MENU_MAX_LENGTH}",
                menu_name(number),
                data.len()
            ));
        } else if slot < 3 {
            return text_error(format!(
                "string {} cannot be made longer: the original is a single character",
                menu_name(number)
            ));
        } else {
            let Ok(target) = space.allocate(data.len() + 1) else {
                return text_error("no room left in the ROM for longer menu strings");
            };
            let [low, high, ..] = target.to_le_bytes();
            fill(&[MENU_ESCAPE, low, high]);
            result.moved.push((target, [data.as_slice(), &[0xFF]].concat()));
        }
        result.changed.push(*number);
        result.encoded.insert(*number, data);
    }
    result.new_characters = needed;
    Ok(result)
}

/// The game's whole script, by section, and the characters' names.
pub fn extract(rom: &[u8]) -> Result<Pack> {
    let mut texts = Script::new();
    for (index, block) in read_blocks(rom)?.iter().enumerate() {
        if !is_text_block(index) {
            continue;
        }
        let mut strings = BTreeMap::new();
        for (number, string) in split_strings(block)?.into_iter().enumerate() {
            strings.insert(number as u32, decode(string)?);
        }
        texts.insert(format!("{index:02}"), strings);
    }
    let chars = menu_characters(rom)?;
    let menu =
        menu_strings(rom).into_iter().enumerate().map(|(number, (_, data))| (number as u32, decode_menu(data, &chars)));
    texts.insert(MENU_SECTION.to_string(), menu.collect());
    let captions = caption_strings(rom)?;
    let captions = captions.iter().enumerate().map(|(number, values)| (number as u32, decode_caption(values)));
    texts.insert(CAPTION_SECTION.to_string(), captions.collect());
    let names = default_names(rom)?.into_iter().map(|(tag, name)| (tag.to_string(), name)).collect();
    Ok(Pack { texts, names })
}

/// Check that an extracted script encodes back to exactly what is in the ROM.
pub fn verify_script(rom: &[u8], script: &Script) -> Result<()> {
    let blocks = read_blocks(rom)?;
    let codes = menu_codes(&menu_characters(rom)?);
    for (section, items) in script {
        let same = if section == MENU_SECTION {
            let encoded = items.values().map(|text| encode_menu(text, &codes)).collect::<Result<Vec<_>>>()?;
            encoded.iter().map(Vec::as_slice).eq(menu_strings(rom).into_iter().map(|(_, data)| data))
        } else if section == CAPTION_SECTION {
            let encoded = items.values().map(|text| encode_caption(text)).collect::<Result<Vec<_>>>()?;
            encoded == caption_strings(rom)?
        } else {
            let strings = items.values().map(|text| encode(text, &Extra::new())).collect::<Result<Vec<_>>>()?;
            let block = join_strings(&strings)?;
            // A section that is no block number has no block to be the same as.
            let index = block_number(section).and_then(|(_, index)| index);
            index.and_then(|index| blocks.get(index)).is_some_and(|original| *original == block)
        };
        if !same {
            return text_error(format!("section {section} does not survive a round trip through its script file"));
        }
    }
    Ok(())
}

/// The block a section of the script is, if its name is a number: the number as written, without
/// zeros in front, and its value when it is small enough to be the number of anything.
fn block_number(section: &str) -> Option<(&str, Option<usize>)> {
    if section.is_empty() || !section.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits = section.trim_start_matches('0');
    Some((digits, if digits.is_empty() { Some(0) } else { digits.parse().ok() }))
}

/// Put a pack's text into the ROM.
///
/// The pack's texts map a section of the script to the strings the pack provides, by string number.
/// A section is a dialogue block, named by its two-digit number, or `menu`. Strings the pack leaves
/// out keep the game's own bytes.
///
/// Menu text is a separate, more limited system: fixed-width tiles, one byte per character. A menu
/// string of up to 26 characters may be longer than the original. Characters the menu font lacks
/// are drawn from the options' menu font (an 8x8 `.hex` font such as Unscii-8) into the tiles of
/// kana that no remaining string uses.
///
/// A pack may use any character. Those the game's font lacks are drawn from the options' fonts
/// (Unifont `.hex` files, tried in order) by a variable-width renderer that is installed along with
/// them; the switch `renderer` installs it even when the pack needs no new character. The pack's
/// names give new default names for party members, by tag; they are shown wherever the game prints
/// a name, unless the player has typed one of their own. Unless wrapping is off, lines too wide for
/// the window are broken at spaces.
///
/// Nothing is touched unless it changed, so an unedited script rebuilds the original image exactly.
/// A changed block is rewritten in place when it still fits its slot; otherwise the image is padded
/// to 8 MiB and the block moves to the new space. The internal checksum is left for the caller to
/// update.
pub fn build(rom: &[u8], pack: &Pack, options: &BuildOptions) -> Result<BuildResult> {
    // Dialogue blocks by number, in the order of the numbers. A block named twice ("01" and "1")
    // is taken from the name that comes last.
    let mut dialogue: BTreeMap<(usize, &str), (Option<usize>, &Strings)> = BTreeMap::new();
    let no_items = BTreeMap::new();
    let mut menu_items = &no_items;
    let mut caption_items = &no_items;
    let mut strangers: Vec<&str> = Vec::new();
    for (section, items) in &pack.texts {
        if section == MENU_SECTION {
            menu_items = items;
        } else if section == CAPTION_SECTION {
            caption_items = items;
        } else if let Some((digits, index)) = block_number(section) {
            dialogue.insert((digits.len(), digits), (index, items));
        } else {
            strangers.push(section);
        }
    }
    // Of several sections the game does not have, the one whose file comes first is named.
    let file_order = |section: &&str| {
        let name = format!("{section}.txt");
        if cfg!(windows) { name.to_lowercase() } else { name }
    };
    if let Some(section) = strangers.into_iter().min_by_key(file_order) {
        return text_error(format!("{section} is not a part of this game's script"));
    }

    // Names the pack changes. One that matches the game's own is left to the game.
    let defaults = default_names(rom)?;
    let name_numbers = name_string_numbers(rom)?;
    // The number of the menu string that holds a character's name.
    let number_of = |tag: &str| name_numbers.iter().find(|(known, _)| *known == tag).map(|(_, number)| *number as u32);
    // A name is kept in places of fixed size, one character after another: a longer one would run
    // into the next character's, and a tag is not a letter.
    let fits = |name: &str, most: usize| {
        let length = name.chars().count();
        length > 0 && length <= most && !name.contains('{') && !name.contains('\n')
    };
    let mut renames: Vec<(&'static str, &str)> = Vec::new();
    for (tag, name) in &pack.names {
        let Some((tag, default)) = defaults.iter().find(|(known, _)| known == tag) else {
            let known: Vec<&str> = defaults.iter().map(|(tag, _)| *tag).collect();
            return text_error(format!("{tag} is not a character whose name can be set ({})", known.join(", ")));
        };
        if !fits(name, MAX_NAME_LENGTH) {
            return text_error(format!("the name for {tag} must be 1 to {MAX_NAME_LENGTH} characters, without tags"));
        }
        if name != default {
            renames.push((*tag, name));
        }
    }
    // A name the pack sets in the menu text is what a new game starts with as that character's name,
    // in a record of eight bytes, so it is held to what the record takes. Where the pack gives no
    // other, it is the name dialogue shows too.
    for (tag, default) in &defaults {
        let Some(number) = number_of(tag) else { continue };
        let Some(name) = menu_items.get(&number) else { continue };
        if !fits(name, NAME_RECORD_LENGTH) {
            return text_error(format!(
                "the name for {tag}, set in menu.{number:03}, must be 1 to {NAME_RECORD_LENGTH} characters, \
                 without tags: that is what the game keeps of a name. A longer one, for dialogue alone, goes in names.txt"
            ));
        }
        if pack.name(tag).is_none() && name != default {
            renames.push((*tag, name));
        }
    }

    // Menu text first: the dialogue font has to know which characters the menu font gained.
    let mut out = rom.to_vec();
    let menu_font = options.menu_font.as_ref();
    let menu_names: Strings = renames
        .iter()
        .filter_map(|(tag, name)| Some((number_of(tag)?, (*name).to_string())))
        .filter(|(number, _)| !menu_items.contains_key(number))
        .collect();
    let mut menu_names_skipped = String::new();
    let with_names = || -> Result<(Vec<u8>, MenuBuild)> {
        let too_long: Vec<&str> = renames
            .iter()
            .filter(|(tag, name)| {
                number_of(tag).is_some_and(|number| menu_names.contains_key(&number))
                    && name.chars().count() > NAME_RECORD_LENGTH
            })
            .map(|(tag, _)| *tag)
            .collect();
        if !too_long.is_empty() {
            return text_error(format!(
                "the menus hold names of up to {NAME_RECORD_LENGTH} characters, and {} is longer",
                too_long.join(", ")
            ));
        }
        let mut trial = out.clone();
        let mut items = menu_items.clone();
        items.extend(menu_names.clone());
        let menu = build_menu(rom, &mut trial, &items, menu_font)?;
        Ok((trial, menu))
    };
    let menu = match with_names() {
        Ok((trial, menu)) => {
            out = trial;
            menu
        }
        // The menus cannot take the new names, so they keep the game's; dialogue still shows the new ones.
        Err(Error::Text(message)) if !menu_names.is_empty() => {
            menu_names_skipped = message;
            build_menu(rom, &mut out, menu_items, menu_font)?
        }
        Err(other) => return Err(other),
    };
    // After the menu, since the captions' font also draws what the menu font gained.
    let captions = build_captions(rom, caption_items, &menu.taken)?;

    let mut wanted: BTreeSet<char> = BTreeSet::new();
    for (_, items) in dialogue.values() {
        for text in items.values() {
            wanted.extend(plain_characters(text));
        }
    }
    wanted.extend(renames.iter().flat_map(|(_, name)| name.chars()));
    // A build that is given no font keeps to the game's own while that has every character the
    // dialogue uses. Once one is missing, it draws from this game's own fonts, as if they had been given.
    let lacking = |wanted: &BTreeSet<char>| -> Vec<char> {
        wanted.iter().copied().filter(|char| !TABLE.codes.contains_key(char)).collect()
    };
    let own_fonts;
    let sources = if options.fonts.is_empty() && !lacking(&wanted).is_empty() {
        own_fonts = Font::built_ins(FONTS);
        &own_fonts[..]
    } else {
        &options.fonts[..]
    };
    if !sources.is_empty() {
        // so that a name or an item name can be shown in dialogue as well
        wanted.extend(menu.taken.iter().map(|(char, _)| *char));
    }
    let new_characters = lacking(&wanted);
    let (mut widths, mut glyph_data, mut sprite_font) = (Vec::new(), Vec::new(), Vec::new());
    let mut extra = Extra::new();
    let mut advances: HashMap<char, usize> = HashMap::new();
    if !new_characters.is_empty() || options.switches.contains("renderer") || !renames.is_empty() {
        let glyphs = fonts::new_glyphs(sources, &new_characters)?;
        (widths, glyph_data, extra) = build_font(rom, &glyphs)?;
        sprite_font = build_sprite_font(rom, &glyphs)?;
        advances = glyphs.iter().map(|(char, shape)| (*char, fonts::proportional(shape).1)).collect();
    }
    let advance = |char: char| -> Result<usize> { Ok(advances.get(&char).copied().unwrap_or(ORIGINAL_ADVANCE)) };
    // The glyph that draws a character, for those the game's font or the pack's has.
    let glyph_number = |char: char| -> Option<usize> {
        match TABLE.codes.get(&char) {
            Some(code) => Some(glyph_index(*code)),
            None => extra.get(&char).map(|code| code - FIRST_GLYPH_CODE),
        }
    };
    let mut name_widths: Vec<(&str, usize)> = Vec::new();
    // The same names by their number of characters, for the strings shown as sprites.
    let mut name_lengths: Vec<(&str, usize)> = Vec::new();
    for (tag, default) in &defaults {
        let name = renames.iter().find(|(renamed, _)| renamed == tag).map_or(default.as_str(), |(_, name)| name);
        let width = name.chars().map(|char| advances.get(&char).copied().unwrap_or(ORIGINAL_ADVANCE)).sum();
        name_widths.push((*tag, width));
        name_lengths.push((*tag, name.chars().count()));
    }
    // Dialogue shows names and item names by converting menu bytes to its own glyphs. The hook is
    // needed when a name changed, and when the menu font did, since the game's own table no longer
    // matches it. It also gives a new game its name records (see `install_names`).
    let menu_glyphs: BTreeMap<u8, usize> =
        menu.taken.iter().filter_map(|(char, byte)| Some((*byte, glyph_number(*char)?))).collect();
    let names_installed = !renames.is_empty() || !menu_glyphs.is_empty();
    // The same names for the place names. The game counts the name in the character's record, which
    // the menus show: the menu string that holds it, where the build wrote that string and installs
    // the records, and the game's own otherwise. It draws the name dialogue shows.
    let mut place_names = Vec::new();
    for (tag, default) in &defaults {
        let drawn = renames.iter().find(|(renamed, _)| renamed == tag).map_or(default.as_str(), |(_, name)| name);
        let number = name_numbers.iter().find(|(known, _)| known == tag).map(|(_, number)| number);
        let written = number.and_then(|number| menu.encoded.get(number)).filter(|_| names_installed);
        let record = written.map_or(default.chars().count(), Vec::len);
        place_names.push(PlaceNameName { tag, record, drawn });
    }

    let offsets = block_offsets(rom)?;
    let mut packed = Vec::new();
    for offset in &offsets {
        // A block cut short by the end of the image is refused when it is expanded.
        let end = (offset + lzss_rle::block_size(rom, *offset)?).min(rom.len());
        packed.push(rom[*offset..end].to_vec());
    }
    let mut changed = Vec::new();
    // Blocks kept on the cartridge: each with its strings and the room one string needs in memory.
    let mut on_cartridge: Vec<(usize, Vec<Vec<u8>>, usize)> = Vec::new();
    let keep_on_cartridge = options.switches.contains("cartridge");
    for ((_, digits), (index, items)) in &dialogue {
        let shown = format!("{digits:0>2}");
        let Some(index) = (*index).filter(|index| *index < BLOCK_COUNT && is_text_block(*index)) else {
            return text_error(format!("block {shown} is not a dialogue block"));
        };
        let original = lzss_rle::decompress_block(&packed[index], 0)?;
        // Event data refers to strings by number, so a pack can replace strings but not add or remove them.
        let mut strings: Vec<Vec<u8>> = split_strings(&original)?.into_iter().map(<[u8]>::to_vec).collect();
        for (number, text) in *items {
            let count = strings.len();
            let Some(string) = strings.get_mut(*number as usize) else {
                return text_error(format!("string {shown}.{number:03} does not exist: block {shown} has {count}"));
            };
            // A string given as the game has it is held to nothing new.
            let untouched = decode(string).is_ok_and(|original| original == *text);
            let given = text;
            let wrapped;
            let text = if SPRITE_LINES.contains(&(index, *number)) {
                wrapped = match sprite_line(text, options.wrap, &name_lengths) {
                    Ok(laid_out) => laid_out,
                    Err(Error::Text(message)) => return text_error(format!("string {shown}.{number:03}: {message}")),
                    Err(other) => return Err(other),
                };
                &wrapped
            } else if options.wrap {
                wrapped = wrap_lines(text, &advance, LINE_WIDTH, &name_widths)?;
                &wrapped
            } else {
                text
            };
            *string = match encode(text, &extra) {
                Ok(data) => data,
                Err(Error::Text(message)) => return text_error(format!("string {shown}.{number:03}: {message}")),
                Err(other) => return Err(other),
            };
            if PLACE_NAMES.contains(&(index, *number)) && !untouched {
                match place_name(given, &extra, &advance, &place_names) {
                    Ok(()) => {}
                    Err(Error::Text(message)) => return text_error(format!("string {shown}.{number:03}: {message}")),
                    Err(other) => return Err(other),
                }
            }
        }
        let data = join_strings(&strings)?;
        if data != original {
            let events = lzss_rle::decompress_block(&packed[index - 1], 0)?.len();
            if keep_on_cartridge || events + data.len() + NAME_ROOM > AREA_RAM_SIZE {
                // The area's events and all of its dialogue do not fit in memory together. The
                // dialogue stays on the cartridge, and memory need only hold its longest string.
                let longest = strings.iter().map(Vec::len).max().unwrap_or(0);
                let room = (DEMAND_STRING_AT + longest + 1).next_multiple_of(2);
                let needed = events + room + NAME_ROOM + DEMAND_SPARE;
                if needed > AREA_RAM_SIZE {
                    let number = strings.iter().position(|string| string.len() == longest).unwrap_or(0);
                    return text_error(format!(
                        "string {shown}.{number:03} is {} bytes too long for the memory the game has left beside \
                         the events of its area",
                        needed - AREA_RAM_SIZE
                    ));
                }
                on_cartridge.push((index, strings, room));
            } else {
                packed[index] = lzss_rle::compress_block(&data)?;
            }
            changed.push(index);
        }
    }

    let mut moved = Vec::new();
    let mut expansion =
        Expansion::new(SCRIPT_EXPANSION.0, SCRIPT_EXPANSION.1, |image| snes::expand(image, EXPANDED_SIZE));
    // Where each block that stays on the cartridge is, and the room one of its strings needs.
    let mut demands: Vec<(usize, usize, usize)> = Vec::new();
    for (index, strings, room) in &on_cartridge {
        demands.push((*index, expansion.store(&mut out, &demand_block(strings)?)?, *room));
    }
    for index in changed.iter().copied() {
        if on_cartridge.iter().any(|(kept, _, _)| *kept == index) {
            // The game's own block is left where it is, and is no longer unpacked.
            continue;
        }
        let offset = offsets[index];
        let slot = lzss_rle::block_size(rom, offset)?;
        if let Some(target) = place(&mut out, offset, slot, &packed[index], &mut expansion, Some(0xFF))? {
            // The block has a new home: clear the old one and point the block table at the new.
            out[offset..offset + slot].fill(0xFF);
            let pointer = snes::pc_to_snes(target, MAPPING)?.to_le_bytes();
            out[BLOCK_TABLE + 3 * index..BLOCK_TABLE + 3 * index + 3].copy_from_slice(&pointer[..3]);
            moved.push(index);
        }
    }

    let mut built = out;
    if !demands.is_empty() {
        built = install_demand(built, &demands)?;
    }
    if !menu.font_blocks.is_empty() {
        built = install_moved_blocks(built, &menu.font_blocks)?;
    }
    if !menu.moved.is_empty() {
        built = install_menu_strings(built, &menu.moved)?;
    }
    if !widths.is_empty() {
        built = install_renderer(built, &widths, &glyph_data, &sprite_font)?;
    }
    if let Some(captions) = &captions {
        built = install_captions(built, captions)?;
    }
    if !options.layout.is_empty() {
        built = install_layout(built, &options.layout)?;
    }
    if names_installed {
        let mut records = HashMap::new();
        for (tag, number) in &name_numbers {
            if let Some(data) = menu.encoded.get(number) {
                records.insert(*tag, [data.as_slice(), &[0xFF]].concat());
            }
        }
        let mut translated = HashMap::new();
        for (tag, name) in &renames {
            // Every character of a name is in the game's font or was added to the pack's above.
            translated.insert(*tag, name.chars().filter_map(glyph_number).collect());
        }
        built = install_names(built, &translated, &records, &menu_glyphs)?;
    }
    // A name in the menus longer than any of the game's, which the status screen would count short.
    let longest_name =
        name_numbers.iter().filter_map(|(_, number)| menu.encoded.get(number)).map(Vec::len).max().unwrap_or(0);
    if longest_name > NAME_COUNTED {
        built = install_name_count(built)?;
    }
    let mut renamed: Vec<String> = renames.iter().map(|(tag, _)| (*tag).to_string()).collect();
    renamed.sort();
    Ok(BuildResult {
        rom: built,
        changed,
        moved,
        on_cartridge: demands.iter().map(|(index, _, _)| *index).collect(),
        new_characters,
        renderer: !widths.is_empty(),
        renamed,
        menu_changed: menu.changed,
        new_menu_characters: menu.new_characters,
        menu_names_skipped,
        captions_changed: captions.as_ref().map_or_else(Vec::new, |captions| captions.changed.clone()),
        caption_glyphs: captions.as_ref().map_or(0, |captions| captions.glyphs.len()),
        caption_glyphs_replaced: captions.as_ref().map_or(0, |captions| captions.replaced),
        caption_menu_glyphs: captions.as_ref().map_or(0, |captions| captions.menu_glyphs),
        captions_wider: captions.map_or_else(Vec::new, |captions| captions.wider),
    })
}

// --- text ------------------------------------------------------------------

/// Convert one string, end marker included, to editable text.
///
/// Characters without a table entry yet appear as `{K:ppll}` and anything unexpected as
/// `{RAW:..}`, so that `encode(decode(data)) == data` always holds.
pub fn decode(data: &[u8]) -> Result<String> {
    let Some(body) = data.strip_suffix(&END) else {
        return text_error("string does not end with FF FF");
    };
    let mut out = String::new();
    let mut i = 0;
    while i < body.len() {
        let byte = body[i];
        let rest = body.len() - i;
        let next = body.get(i + 1).copied().unwrap_or(0);
        let wide = u32::from(next) << 8 | u32::from(byte);
        if byte == 0x00 {
            out.push('\n');
            i += 1;
        } else if byte == 0x01 {
            out.push_str("{PAGE}\n");
            i += 1;
        } else if let Some(name) = tag_of(&SPEAKERS, next).filter(|_| byte == 0x02 && rest >= 2) {
            out.push_str(&format!("{{{name}}}"));
            i += 2;
        } else if let Some(tag) = tag_of(&PARAM_TAGS, byte).filter(|_| rest >= 2) {
            out.push_str(&format!("{{{tag}:{next:02X}}}"));
            i += 2;
        } else if let Some(tag) = tag_of(&LONG_TAGS, byte).filter(|_| rest >= 4 && next == 0x80) {
            out.push_str(&format!("{{{tag}:{:02X}{:02X}}}", body[i + 2], body[i + 3]));
            i += 4;
        } else if byte >= 0x80 && rest >= 2 && next >= 1 && wide <= LAST_CODE {
            match TABLE.characters.get(&wide) {
                Some(char) => out.push(*char),
                None => out.push_str(&format!("{{K:{wide:04X}}}")),
            }
            i += 2;
        } else if let Some(char) = TABLE.characters.get(&u32::from(byte)) {
            out.push(*char);
            i += 1;
        } else {
            out.push_str(&format!("{{RAW:{byte:02X}}}"));
            i += 1;
        }
    }
    Ok(out)
}

/// Convert editable text back to one string, end marker included.
///
/// `extra` gives the code of each character that the game's own table lacks; `build` assigns them.
pub fn encode(text: &str, extra: &Extra) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut position = 0; // counted in characters, for a message
    while let Some(char) = rest.chars().next() {
        if char == '\n' {
            out.push(0x00);
        } else if char == '{' {
            let Some(Token::Tag { raw, name, value }) = tag_at(rest) else {
                let shown: String = rest.chars().take(16).collect();
                return text_error(format!("malformed tag at position {position}: {}", quoted(&shown)));
            };
            out.extend(encode_tag(name, &value.unwrap_or("").replace(' ', ""))?);
            rest = &rest[raw.len()..];
            position += raw.chars().count();
            // A page break is written on its own line; that line break is layout, not a code.
            if name == "PAGE" && rest.starts_with('\n') {
                rest = &rest[1..];
                position += 1;
            }
            continue;
        } else if let Some(code) = TABLE.codes.get(&char).copied() {
            if code < 0x80 {
                out.push(code as u8);
            } else {
                out.extend([code as u8, (code >> 8) as u8]);
            }
        } else if let Some(linear) = extra.get(&char) {
            out.extend(linear_code_bytes(*linear)?);
        } else {
            return text_error(format!("character {} is not in the game's character table", quoted_char(char)));
        }
        rest = &rest[char.len_utf8()..];
        position += 1;
    }
    out.extend(END);
    Ok(out)
}

/// Bytes for a character given its position in the code sequence (glyph number + 5).
fn linear_code_bytes(linear: usize) -> Result<Vec<u8>> {
    if linear < 0x80 {
        return Ok(vec![linear as u8]);
    }
    // Page $80 and up would collide with the control codes and the end marker.
    if linear >= 0x80 << 7 {
        return text_error("too many different characters for the game's encoding");
    }
    Ok(vec![0x80 | (linear & 0x7F) as u8, (linear >> 7) as u8])
}

/// The bytes a tag stands for, given its name and its value without spaces.
fn encode_tag(tag: &str, value: &str) -> Result<Vec<u8>> {
    let Some(data) = from_hex(value) else {
        return text_error(format!("tag {{{tag}:{value}}} does not hold hexadecimal bytes"));
    };
    if data.is_empty()
        && let Some(member) = code_of(&SPEAKERS, tag)
    {
        return Ok(vec![0x02, member]);
    }
    if let Some(code) = code_of(&PARAM_TAGS, tag)
        && let [parameter] = data[..]
    {
        return Ok(vec![code, parameter]);
    }
    if let Some(code) = code_of(&LONG_TAGS, tag)
        && let [high, low] = data[..]
    {
        return Ok(vec![code, 0x80, high, low]);
    }
    match (tag, data.as_slice()) {
        ("PAGE", []) => Ok(vec![0x01]),
        ("K", [page, low]) => Ok(vec![*low, *page]),
        ("RAW", [_, ..]) => Ok(data),
        _ => text_error(format!("unknown or malformed tag {{{tag}:{value}}}")),
    }
}

// --- font ------------------------------------------------------------------

/// Position of a character code in the font. The first glyph's code is 5.
pub const fn glyph_index(code: u32) -> usize {
    let linear = if code < 0x80 { code } else { ((code >> 8) - 1) * 0x80 + (code & 0xFF) };
    linear as usize - FIRST_GLYPH_CODE
}

/// Character code of the glyph at a font position.
pub fn code_at(index: usize) -> u32 {
    let linear = (index + FIRST_GLYPH_CODE) as u32;
    if linear < 0x80 { linear } else { ((linear >> 7) << 8) | 0x80 | (linear & 0x7F) }
}

/// Rows of one glyph, each a 16-bit integer with the leftmost pixel in the top bit.
pub fn glyph(rom: &[u8], index: usize) -> [u16; GLYPH_SIZE] {
    let base = FONT + index * GLYPH_BYTES;
    std::array::from_fn(|row| u16::from(rom[base + row]) << 8 | u16::from(rom[base + 16 + row]))
}

// --- what the engine sees --------------------------------------------------

/// A dialogue block is named by its number, in two digits.
/// What `extract` writes under the header of a string the game shows as sprites.
const SPRITE_NOTE: &str = "The game shows this string as sprites over the scene, not in a box, with its letters wide apart.\n\
    A row holds 15 characters and a page 32, spaces not counted. The build breaks the rows.\n\
    Where a page would have more than 32, put a {PAGE}, or say it in fewer letters: the build refuses a longer page.";

/// What `extract` writes under the header of a place name (see `PLACE_NAMES`).
const PLACE_NOTE: &str = "The name of a place, shown on one line and centred: 16 characters at most, a party member's name \
    counting as its letters and one more.";

/// What `extract` writes at the head of the captions' file.
const CAPTION_NOTE: &str = "The game draws a caption in cells 16 dots wide, and the build puts two narrow letters in a cell.\n\
    The game does not break a caption's lines. Break them yourself: a line holds 30 letters, and a description two lines.\n\
    A caption shown inside another, as {PHRASE:..}, or followed by one on the same line, needs an even number of letters.\n\
    A caption that takes more cells than the game's own can run into what is beside it on screen. The build lists those.";

/// What `extract` writes at the head of the menu's file.
const MENU_NOTE: &str = "The menus give a string a column of fixed width and do nothing for a longer one: it is cut off, or runs\n\
    into what is beside it. The game's own string shows how wide the column is. Most often it is 8 characters for the name\n\
    of an item, a piece of equipment, a spell, a monster or a title, 12 for a sword skill, and 4 for an equipment slot.\n\
    One string can be on two screens, and the narrower place decides.\n\
    Where a word does not fit, a label can be moved: a string the game draws at a fixed place has a comment under it\n\
    naming each place, and pack.toml moves one under [layout], as \"menu.659#3\" = { columns = -2 }. What is beside a\n\
    label (a number, a name from a list, a picture) stays where it is, so look at the screen afterwards. One picture\n\
    can be moved the same way: the control pad after the skills label on the status screen, as \"picture.skills\";\n\
    and so can the family name after the given name there, as \"status.family\" (see menu.759).\n\
    Strings 670 and 711 to 755 are the grids of letters on the naming screen.\n\
    A character the menu font lacks takes the tile of a kana that no menu string uses any more.";
const NAME_SIGN_NOTE: &str = "The sign between a character's given name and family name, on the status screen and the naming screen.\n\
    The status screen leaves a column on each side of it. To join the two names with one space, write this string as\n\
    a space and move the family name over it in pack.toml: \"status.family\" = { columns = -2 }.";
const FAMILY_NAME_NOTE: &str = "A family name, which the status screen draws after the given name and the sign (menu.759). A pack moves\n\
    it there as \"status.family\", for every character at once.";

/// A dialogue block is named by its number, in two digits. The blocks between the dialogue blocks
/// hold the areas' events and no text, so their numbers name no section.
fn is_dialogue(section: &str) -> bool {
    let digits = section.len() == 2 && section.bytes().all(|byte| byte.is_ascii_digit());
    digits && section.parse().is_ok_and(|index| index < BLOCK_COUNT && is_text_block(index))
}

fn is_menu(section: &str) -> bool {
    section == MENU_SECTION
}

fn is_caption(section: &str) -> bool {
    section == CAPTION_SECTION
}

static KINDS: [TextKind; 3] = [
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        // The number is AREA_RAM_SIZE; a test holds the sentence to it.
        limits: "an area's longest string and its events must fit together in the 16,480 bytes of memory the game \
                 has for them; the name of a place, shown on entering it, is centred on one line of 16 characters at \
                 most, a party member's name counting as its letters and one more",
        trial: Some(("01.001", "01.003")),
        ..TextKind::new("dialogue", is_dialogue)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        max_length: Some(MENU_MAX_LENGTH),
        limits: "a new character takes the tile of a kana that no menu string uses any more",
        trial: Some(("menu.004", "menu.005")),
        ..TextKind::new("menu", is_menu)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        limits: "battle messages, the names of skills in kanji and the descriptions of items, in cells 16 dots \
                 wide, about 15 to a line; a character the game's font lacks is drawn from Unifont, two narrow ones \
                 to a cell, so a piece that the game joins to another needs an even number of them; a caption that \
                 takes more cells than the game's own may run into what is beside it",
        trial: Some(("captions.004", "captions.561")),
        ..TextKind::new("captions", is_caption)
    },
];

/// The adapter itself: what the rest of this module knows, on the contract every game follows.
pub struct Top;

pub static ADAPTER: Top = Top;

impl Adapter for Top {
    fn id(&self) -> &'static str {
        "top-sfc"
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn platform(&self) -> &'static dyn Platform {
        &snes::PLATFORM
    }

    fn images(&self) -> &'static [Release] {
        &[Release { sha1: ROM_SHA1, name: "Japan", size: 0x600000 }]
    }

    fn source_language(&self) -> &'static str {
        SOURCE_LANGUAGE
    }

    fn kinds(&self) -> &'static [TextKind] {
        &KINDS
    }

    fn switches(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("renderer", "install the variable-width renderer even if no new character needs it"),
            (
                "cartridge",
                "read the dialogue of every area the pack changes from the cartridge, even where it would fit in memory",
            ),
        ]
    }

    fn sets_names(&self) -> bool {
        true
    }

    fn name_limits(&self) -> &'static str {
        NAME_LIMITS
    }

    fn takes_menu_font(&self) -> bool {
        true
    }

    fn fonts(&self) -> &'static [&'static str] {
        FONTS
    }

    fn menu_font(&self) -> Option<&'static str> {
        Some(MENU_FONT)
    }

    fn extract(&self, image: &Image) -> Result<Pack> {
        extract(image.cartridge()?)
    }

    fn moves_labels(&self) -> bool {
        true
    }

    fn note(&self, section: &str, number: Option<u32>) -> Option<String> {
        match number {
            None if section == CAPTION_SECTION => Some(CAPTION_NOTE.to_string()),
            None if section == MENU_SECTION => Some(MENU_NOTE.to_string()),
            Some(number) if section == MENU_SECTION => {
                // Each place the game draws this string at that a pack can move.
                let places: Vec<String> = LABELS
                    .iter()
                    .filter(|label| label.string == number)
                    .map(|label| {
                        let (row, column) = label.row_and_column();
                        let screen =
                            if label.screen.is_empty() { String::new() } else { format!(" on the {}", label.screen) };
                        format!("{} at column {column}, row {row}{screen}", label.name)
                    })
                    .collect();
                let mut lines = Vec::new();
                if !places.is_empty() {
                    lines.push(format!("Placed where a pack can move it: {}.", places.join("; ")));
                }
                if number == NAME_SIGN {
                    lines.push(NAME_SIGN_NOTE.to_string());
                }
                if (FAMILY_NAMES.0..=FAMILY_NAMES.1).contains(&number) {
                    lines.push(FAMILY_NAME_NOTE.to_string());
                }
                (!lines.is_empty()).then(|| lines.join("\n"))
            }
            Some(number) if is_dialogue(section) => {
                let string = (block_number(section)?.1?, number);
                if SPRITE_LINES.contains(&string) {
                    Some(SPRITE_NOTE.to_string())
                } else {
                    PLACE_NAMES.contains(&string).then(|| PLACE_NOTE.to_string())
                }
            }
            _ => None,
        }
    }

    fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
        verify_script(image.cartridge()?, texts)
    }

    fn write(&self, image: &Image, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
        let mut result = build(image.cartridge()?, pack, options)?;
        result.rom = snes::write_rom(output, &result.rom)?;
        Ok(Built { summary: result.summary() })
    }

    fn glyph_size(&self) -> usize {
        GLYPH_SIZE
    }

    fn glyph_count(&self) -> usize {
        GLYPH_COUNT
    }

    fn glyph(&self, image: &Image, index: usize) -> Result<Vec<Vec<u8>>> {
        let rows = glyph(image.cartridge()?, index);
        Ok(rows.iter().map(|row| (0..GLYPH_SIZE).map(|x| (row >> (GLYPH_SIZE - 1 - x) & 1) as u8).collect()).collect())
    }

    fn code_at(&self, index: usize) -> u32 {
        code_at(index)
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
