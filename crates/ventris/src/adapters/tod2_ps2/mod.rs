//! Tales of Destiny 2 (PlayStation 2, 2002).
//!
//! What is known so far covers the story scripts, the skits (the short talks between party members
//! that the player starts with Select), the text of the menus, and the lists the program itself
//! holds (skills, items, equipment, titles, recipes, tactics, each with its description, the
//! names of the party's members, and the main menu's own labels): where they are, how they are
//! compressed, how their text is encoded, how to write it out and read it back without loss, and
//! how to put a changed string back, the names of enemies, and the scripts of the games within the
//! game, such as the quiz.
//!
//! Putting a string of a script back changes nothing in the game's program. A new string that
//! fits goes where the old one was, followed by zero bytes up to the next string. One that needs
//! more bytes moves,
//! to room another moved string left or to the end of the text, and the references to it in the
//! script's code are changed to match; they are 16 bits, so a script's text cannot pass 65,535
//! bytes. A string that nothing refers to cannot move. To tell a reference from a $F8 that is part
//! of something else, the code is read instruction by instruction: what an expression is made of
//! (below), $E0-$EF with one more byte, $F0, $F1 and $FE alone, $F6 with one more byte, and $F2-$F5
//! and $F8 with a 16-bit number, a place in the code for the first four. The script is compressed
//! again, its container gets the new size, and the archive is written out again: a member that
//! changed size moves every member after it, so the table of contents in the program is rewritten,
//! and so is the archive's size on the disc. A pack that changes nothing gives back the image it
//! started from.
//!
//! The game does not fit a line to the screen: the speech bubble is as wide as its longest line,
//! and a line wider than the screen, about 56 columns, is cut off at both ends. So a build breaks
//! the lines of changed dialogue at spaces, to the 40 columns that nearly all of the game's own
//! lines keep to, a wide character taking two and a narrow one taking one, and a party member's
//! name the columns of the name a new game gives that member (see `PARTY_MEMBERS`). A skit's line
//! is one row that scrolls, and is left as it is.
//!
//! A list to choose from is not broken: each of its lines is a choice, or a line of the question
//! before them, and the game lets the player pick among its last lines. What makes a string a list
//! is in the script's code: the statement that shows it is followed by one that calls for a choice
//! (see `choice_lists`); 778 of the game's strings are. Many start with the code $03 and put a wide
//! space either side of each choice, but not all. A changed list is refused if it has more lines
//! than the game's own, a line wider than the window can be (55 columns, where the game's own keep to
//! 40), or more
//! characters than a page shows (below).
//!
//! A bubble or a caption shows its text a page at a time: four lines, or fewer where a $02 ends the
//! page early, an empty line not counting. The game keeps what it draws on a page in a list of 128
//! places (at $2CC in the window's record in memory, eight bytes each, with a byte each at $24C):
//! one for every character, space or button picture, and for every character of a name or a number
//! it fills in, and none for a line break or a code that changes the colour or the position. Once a
//! page is typed out its 128th character is not shown, those after it are lost too, and at about 140
//! the list runs over what follows it and the screen breaks. Forty columns of narrow letters make
//! 156 on four lines, so a long string of English would not fit: where a page of a changed string
//! draws more than the game can, the build breaks the string's lines narrower, a column at a time,
//! until every page fits. A string that cannot be made to fit is refused.
//!
//! What the game can draw without help: every character in its list, and ASCII, which it draws in
//! a narrow font made of the first glyphs of its list at half the width. That font has capitals
//! only, so lowercase letters come out as capitals, and it draws other ASCII as the nearest thing it
//! has: `"`, `'` and `` ` `` as a closing double quote, `_` and `~` as a minus sign, `;` as a colon,
//! `<` and `>` as round brackets, `[` and `]` as corner brackets and `*` as a star, and `# $ @ \ ^
//! |` not at all. Where a pack uses any of these, the build draws it from the pack's fonts in a cell
//! of the narrow font that the game does not use, and points the character's byte there (see
//! `needs_narrow_glyph`).
//!
//! Where things are. The disc holds one large archive, `FILE.FPB`, with no header of its own. Its
//! table of contents is in the program the console starts, `SLPS_251.72`: one 32-bit number per
//! member, the member's offset (a multiple of 64) with the padding that follows the member in the
//! low 6 bits, and a last number that is the archive's size. Members have no names, only numbers.
//!
//! A story script is a member that starts with `SCPK`: a small container whose entries are each a
//! compressed block (see `codecs::lzss_ring`). Its last entry is the script itself, `SCED`: a
//! header giving where the code and the text start, the code, then the text to the end.
//!
//! A skit is a member that is one compressed block holding a pack of six entries: who speaks, how
//! the speakers' lips move (a count, then a nibble a frame), the script, and three of pictures. A
//! pack starts with how many entries it has, then an offset and a size for each, and each entry
//! starts on a multiple of 16. The script is a `SCED` like a story script's, not compressed again
//! inside the pack. All 609 skits are
//! among members 54 to 1298, each followed by a member that is the recording of its voices. When a
//! skit's script grows, the pack is written out again with the entries after it starting later,
//! and the whole member is compressed again. On screen a skit's line is one row, which scrolls
//! sideways while a line too long for it is typed.
//!
//! A skit keeps time by its voice: its clock counts the frames the voice has played, and the skit
//! ends when the lips' frames run out. Each line starts at a time its script gives and is typed a
//! letter at a time, at the frames per letter its script also gives; it is taken away when the line
//! two after it starts, or when the skit ends. English is about twice as long as the Japanese, so at
//! the game's pace a long line is taken away before it is whole. The build option `skit-pace` types
//! each changed line faster where it must, and refuses a line too long even then (see
//! `skit_paces`). It is an option because it changes how fast the game types, not only its text.
//!
//! The grids of artes on the Artes and Enchant screens draw a spell's name in large letters, with
//! room for 9 in its column where a skill's narrow letters have room for 17. The build option
//! `narrow-spell-names` has them draw a spell's name as they draw a skill's (see `SPELL_FLAGS`).
//!
//! The menus are 19 program modules, members 6798 to 6816, one for each screen the main menu opens
//! (skills, items, equipment, saving and so on). Each is in the archive a second time as one
//! compressed block, 19 members later, and a build changes both. The game loads a module at
//! $29BB00, and the module's text is zero-ended strings among its code and data, written as
//! dialogue is. Of dialogue's codes only two occur: $09 followed by a number draws a button or
//! asks a question, and $05 comes before a word in a few titles. Five strings also write a bracket,
//! a colon, a wide space or a wide capital in Shift-JIS itself, which the game draws as well (see
//! `SHIFT_JIS_LEADS`).
//!
//! Nothing lists a module's strings, so a string is taken to be what all of these hold for:
//! something in the module may point at it (a 32-bit word that is an address inside the module, or
//! an `addiu` or `ori` adding a low half to a register that a `lui` anywhere in the module gives a
//! high half, since a function sets the high half once and uses it throughout); it starts on a
//! multiple of 8, where the compiler put every string; it reads as menu text up to its zero byte;
//! and it does not start inside the string before it. A word that is itself an address is left
//! out, though its bytes may read as a character and a bracket. A string found that way is named
//! by its offset in the module, which stays its name whatever is found later.
//!
//! The bytes between the end of a string and the next multiple of 8 are unused, and are part of
//! the string's room. The strings of one menu that a pack changes share the room they had between
//! them, as the program's own do (below): they are written back one after another with nothing
//! between them, and what refers to a string that moved is given its new address. A string that
//! finds no place there goes past the end of the module, which grows by it. The game loads a
//! module as long as the archive says it is; the limit is the size of the largest of the game's
//! own, 63,736 bytes, since the memory further on is in use (see `MENU_MODULE_LIMIT`).
//!
//! What refers to a string is every 32-bit word of the module that holds its address, and every
//! instruction that adds the low half of its address (see `code_use`). A fan translation of the
//! game moved 567 of these strings, and what it changed for each is what is found here, apart from
//! one instruction it left pointing at the Japanese. One string of the 587 stays where it is: the
//! code reads it straight from memory.
//!
//! A module is its code and then its data, so its text is all past the last instruction that
//! returns from a function, and what reads as text before that is left out.
//!
//! Three larger modules hold text in the same form, and are read the same way: the quiz (member
//! 6167), battles (8055: the labels of the results screen, and the messages that explain a newly
//! learned skill) and the field (8996: the names of places). They are loaded at $2AEA80. The
//! battle module's text is written with all that the program's own is, but for the code $0A.
//! Their strings stay where they are, each with its room and no more: nothing shows what follows
//! these modules in memory, and the fan translation, which left them alone, does not show which
//! of their words are pointers.
//!
//! The game loads the quiz from its member, but not the other two. It loads them from member 17,
//! a pack of three compressed blocks that the program keeps whole in memory from the start and
//! expands one of when it needs that module: entry 0 is member 8055 byte for byte, entry 1 a module
//! with no text, and entry 2 member 8996. Members 8055 and 8996 are copies the game does not use.
//! Strings are still read from them, so that their names stay as they were, and a build writes
//! them; where it changes either, it also compresses that module again into its entry of member 17
//! and writes member 17 out again around it (see `rebuild_compressed_modules`). Builds made before
//! this was known wrote members 8055 and 8996 alone, so none of the battle's and the field's text
//! that a pack changed reached the screen; the same packs now build images that show it. Member 17
//! cannot grow much: see `COMPRESSED_MODULES_LIMIT`.
//!
//! Not covered: the name-entry screen (member 6810), whose letters and labels are in a grid that
//! nothing points at.
//!
//! The enemies are in 550 packs among members 8178 to 8727, a pack holding one or more: for each
//! a record, a picture and its movements, every entry a compressed block, each starting on a
//! multiple of 4. A record starts with `EN` and a table of eleven sections, an offset and a count
//! for each; the eighth section starts with the enemy's name, in a field of 23 bytes that a zero
//! byte ends. A section is a pack, and an enemy's name is numbered by its entry in the pack. A new
//! name goes in the same field, so it takes 22 bytes at most; the record is compressed again and
//! the pack written out around it. An enemy that is in several packs has its name in each.
//!
//! The plate at the top of the screen in battle does not show that name, but one from a list of its
//! own: `TEKI`, an entry of member 8063, 256 names each in a field of 24 bytes padded with spaces,
//! where the plate writes how many of the enemy there are over the last two. Another entry, `ALLG`,
//! says for each pack which of those names its enemies are (see `plate_kinds`). So a pack's names
//! of enemies are not written twice: a build puts each renamed enemy's name in the list as well,
//! padded as the game's own, and refuses two strings that give one enemy two names. Every enemy on
//! the list is in some pack.
//!
//! The games within the game (the quiz, with its host's lines, its questions and its answers, a
//! colouring game and others) have scripts of a third kind, members 6171 to 6301, each stored as
//! it is or as one compressed block. Such a script starts with `SFM_` and a header that says where
//! its two parts are: its code, and its data, which runs to the end. The data holds the strings,
//! zero-ended and one right after another, among much else, pictures included. The script refers
//! to a string by its offset counted from the start of the data, as a 32-bit number in its code or
//! in a table of its data, so a string is what starts after a zero byte in the data, reads as
//! text, and has that number written somewhere in the script; or, where a table refers to it, what
//! follows other data with no zero byte between (see `game_strings`). It is written as dialogue is. A
//! string is named by its offset in the script, and a changed one goes where the game's own was;
//! the next string follows at once, so it cannot be longer.
//!
//! The program, `SLPS_251.72`, holds text of its own in the same form: zero-ended strings on
//! multiples of 8 in its data, from offset $FC000 of its file. They are the names and descriptions
//! of what its lists show, and the labels of the main menu. Here the strings are listed: the
//! program's tables hold a 32-bit word for each, its address, and the file's header says that
//! offset $1000 of the file is loaded at $100000. So a string of the program is what one of its
//! words points at, on a multiple of 8, that reads as text. It is written as dialogue is, with the
//! half-width dot dialogue uses for an ellipsis, and five of dialogue's codes. It is one section,
//! `program`, its strings named by their offset in the file. The fifth code, $0A, which fills in a
//! name, was added after the first release: it lets one more string be found, the prompt to leave
//! a town, which fills in the town's name (`program.1109536`). The game writes that name's address
//! at a fixed place in the string, so a build refuses a new one whose $0A is elsewhere (see
//! `EXIT_PROMPT`).
//!
//! Thirty-five more strings no table points at: the code builds the address of each with two
//! instructions, one giving a register the high half and one adding the low half. Most are a word
//! or two that the code puts into a sentence, such as what an item restores; a few are messages of
//! the main menu. They are found by following the code (see `code_use`), and are strings of the
//! same section.
//!
//! The program's file cannot grow: what follows it in memory is in use. So the strings of the
//! program that a pack changes share the room they had between them, each its own bytes and the
//! unused ones up to the next multiple of 8. They are written back one after another with nothing
//! between them, which the game reads as well as it reads its own. For a string that moved, the
//! words that point at it are given its new address, and the instructions that add the low half
//! of its address are given the new low half; the high half is left alone, so such a string stays
//! within the 65,536 bytes that half reaches. What a pack leaves alone is not touched.
//!
//! A string moves only if everything that refers to it is known, and for 18 of the 3,259 that is
//! not sure enough: see `program_reach`. Those stay where they are, on the same terms as a menu's
//! string. One of them the code copies eleven bytes of, so it has the ten of the game's own and no
//! more.
//!
//! The program also holds a story script of its own among its data, at $CFA40 of its file: the
//! messages of treasure chests, the caption over the world map after a battle is lost, and the
//! message that a title was earned. Its code names each string with $F8 as a story script's does,
//! and the program shows message `n` by reading the `n`th statement of the code (see
//! `PROGRAM_SCRIPT`). It is section `program-script`, part of the dialogue, and was found after the
//! first release: until then none of its 16 messages was extracted, so no pack could change them.
//! What follows it in memory is in use, so it cannot grow: a build writes its strings back one after
//! another in the room its text has, the same text once, and refuses what does not fit (see
//! `rebuild_program_script`).
//!
//! The world map has a story script of its own: the names of places it shows in large letters as
//! the party comes near them, the text of its signposts, an inn's and a chest's messages, and the
//! lines of a few scenes played on the map. There are five world maps, members 9028 to 9032, one for
//! each time the story goes to (see `WORLD_MAPS`). Each is a pack that starts with how many entries
//! it has and the size of each, the entries following one another with nothing between them, and
//! its entry 19 is the script, stored as it is. On the world map the program reads the whole member
//! into a block it set aside (see `WORLD_MAP_ROOM`), works out where each entry is from the sizes,
//! and copies the script into a block of the entry's size, from which the game reads it. So the
//! script may grow as a story script does, and the member with it: a build writes the script back
//! as it writes a story script's (see `rebuild_text`), with the entries after it starting later and
//! its size in the pack's table (see `rebuild_world_map`). They are sections `09028` to `09032`,
//! part of the dialogue, and were found after the first release: until then none of their strings
//! was extracted, so the names of places over the map stayed as the game has them. On screen, the
//! name over the map is drawn in the half-width font, enlarged, so it has what dialogue's narrow
//! letters have, the cells a build adds included; it starts at the same place whatever its length.
//! A signpost's text is shown in a bubble whose name tag is a string of the same script, `notice`.
//!
//! The text is strings back to back, each ending in a zero byte. The code refers to a string with
//! the byte $F8 followed by the string's 16-bit offset in the text. Not every string is referred to
//! that way, so strings are found by reading the text from its start, not by following the code.
//!
//! A string is made of:
//!
//! - two-byte characters, lead byte $99-$9F or $E0 and up, then a byte from $40 to $FC that is
//!   neither $5C nor $7F. Glyph number = row * 187 + position of the second byte, the rows being
//!   the lead bytes in order. This is the layout of Shift-JIS, but not its characters: member 19 of
//!   the archive lists, as Shift-JIS, the character each glyph is, and member 18 is the font in the
//!   same order;
//! - single bytes $20-$7E, which are ASCII, and $A1-$DF, which are half-width kana ($A5 is the dot
//!   the script uses for an ellipsis);
//! - $01, a line break;
//! - $03-$0B, each followed by a 32-bit number. $07 prints the name of a party member;
//! - $12, $14, $15, $16 or $18, followed by a small expression that ends in $C0: a value the game
//!   works out and prints. An expression's bytes may be anything, zero included, so it has to be
//!   read token by token: below $80 a variable (one more byte, or two when bit 3 is set), $80-$8F a
//!   small number, $90-$9F a number with one more byte, $A0-$AF with two, $B0-$BF with four,
//!   $C1-$DF an operator.
//!
//! The font is 2,560 glyphs of 24 by 22 pixels, 4 bits per pixel, the left pixel in the low half of
//! each byte, in 272 bytes per glyph of which the last 8 are unused.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::{Range, RangeInclusive};
use std::path::Path;
use std::sync::{Arc, LazyLock};

use crate::adapter::{Adapter, BuildOptions, Built, CatalogueEntry, Refused, Release, TextKind};
use crate::codecs::lzss_ring;
use crate::error::{Error, Result, text_error};
use crate::fonts::{self, Bitmap, Font};
use crate::layout;
use crate::placement::share_room;
use crate::platform::{Image, Platform};
use crate::platforms::mips::{CodeUse, RETURN, code_use, number, pointed_at, within_reach};
use crate::platforms::ps2::{self, Disc, write_image};
use crate::script::{self, Pack, Script, string_name};
use crate::shift_jis;
use crate::text::{ControlCodes, Token, from_hex, hex, plain_characters, quoted, tokens};

pub const NAME: &str = "Tales of Destiny 2 (PlayStation 2, Japan)";
/// The whole disc image, version 1.01.
pub const ROM_SHA1: &str = "3b2be72b751607a7a2a56d62feec71584b4b8c72";
pub const SOURCE_LANGUAGE: &str = "ja";

// --- the archive
pub const EXECUTABLE: &str = "SLPS_251.72";
pub const ARCHIVE: &str = "FILE.FPB";
/// Offset in the executable of the archive's table of contents.
pub const TABLE: u64 = 0xDD320;
pub const MEMBER_COUNT: usize = 9203;
/// The low bits of a table entry: unused bytes between this member and the next.
const PADDING_BITS: u32 = 0x3F;
pub const FONT_MEMBER: usize = 18;
/// The Shift-JIS code of every glyph, in the font's order.
pub const CHARACTER_MEMBER: usize = 19;
/// How much of the archive is copied at a time.
const COPY_CHUNK: u64 = 1 << 24;

// --- a script
const CONTAINER_MAGIC: &[u8] = b"SCPK";
const SCRIPT_MAGIC: &[u8] = b"SCED";
/// The members among which the skits are. The others in this stretch are recordings of voices.
const SKIT_MEMBERS: RangeInclusive<usize> = 54..=1298;
/// A skit is a pack of this many entries.
const SKIT_ENTRIES: usize = 6;
/// Which of them is the script.
const SKIT_SCRIPT: usize = 2;
/// An entry of a pack starts on a multiple of this.
const PACK_ALIGNMENT: usize = 16;
/// How much of an expanded skit is asked for at first: most skits' scripts end before this.
const SKIT_START: usize = 0x1000;

// --- the dialogue box
/// How wide a line of dialogue may be, in columns: a wide character takes two and a narrow one
/// takes one. All but a few of the game's own lines keep to this; the screen holds about 56.
const LINE_COLUMNS: usize = 40;
/// The columns set aside for the name of a party member, which the game fills in, where a build
/// cannot tell whose name it is or what it will be: the longest of the game's own names has five
/// wide characters. Where it can tell, the name takes its own columns (see `party_name_columns`).
const NAME_COLUMNS: usize = 10;
/// How many party members a name tag names by number: `{NAME:01 00 00 00}` to `{NAME:06 00 00 00}`
/// are Kyle, Reala, Loni, Judas, Nanaly and Harold.
///
/// The code that draws $07 (at $116C48 in memory) takes its number modulo 10,000: 1 to 6 print that
/// member's name from the member's record in the save, 0 and 1,000 the name of a member the game
/// chooses as it runs, 7 and up a name from another table, and a number of 10,000 or more prints
/// just the first letter or two of the name. A record is $234 bytes, the first at $1B8278, and its
/// name is a field of 13 bytes at its third byte, so it has room for 12 bytes: 12 narrow letters or
/// six wide ones, at most 12 columns either way.
///
/// A new game fills the field from the program (see `PARTY_NAME_TABLE`), and the player may change
/// it at any time with Rename on the Status screen (`06809.11600`). So a build counts a name as the
/// one a new game starts with, which is the pack's where it gives one. A line of a bubble broken to
/// 40 columns with Kyle as four takes at most 47 with Kyle renamed to twelve, inside the screen's 56;
/// only a line holding three or four names, each renamed to the full twelve, can pass it.
const PARTY_MEMBERS: usize = 6;
/// Where the table of the party's names is in the program's file ($1BF188 in memory): for each
/// member in order, a word pointing at the member's name, such as `program.1064000` (カイル, Kyle),
/// and one pointing at the surname. A new game copies both into the member's record (above).
const PARTY_NAME_TABLE: usize = 0xC0188;
/// The bytes a party member's name may take in the program: a new game copies it, and the zero byte
/// that ends it, into the name's field of 13 bytes in the member's record (see `PARTY_MEMBERS`), so a
/// longer name would run into what follows the field.
const PARTY_NAME_ROOM: usize = 12;
/// How wide a line of a list to choose from may be, in columns. The game's own lists keep to 40,
/// but the window they are shown in is as wide as its longest line: one of 55 narrow letters was
/// seen whole on screen, the window reaching nearly from one edge of the screen to the other, as a
/// bubble does at about 56 (`LINE_COLUMNS`). Counted as dialogue's lines
/// are at their full width: a party member's name takes the columns of the name, other text the game
/// fills in nothing, since one of the game's own lists has a line of 30 columns and two numbers.
const CHOICE_COLUMNS: usize = 55;
/// How many lists to choose from the game's story scripts have, for a test to hold the finding of
/// them to.
#[cfg(test)]
const CHOICE_LISTS: usize = 778;
/// The lines a page of a bubble or a caption holds; the game starts a new page after them.
const PAGE_LINES: usize = 4;
/// The code that ends a page before its four lines, as a pack writes it.
const PAGE_BREAK: &str = "02";
/// How many characters a page shows once it is typed out: the game's list has 128 places, and the
/// last of them is not shown (seen on screen: 127 drawn in full, of 128 the last left out).
const PAGE_CHARACTERS: usize = 127;
/// Places a build leaves free on every page. The count of a page's characters is exact for what the
/// pack writes; this covers a name or a number the game fills in being longer than it is counted
/// as, and it is well short of where the screen breaks, about 15 places past the list's end.
const PAGE_MARGIN: usize = 4;
/// What a party member's name is counted as on a page. The longest of the game's own has five
/// characters; six leaves room for a name such as Harold, which takes six letters.
const NAME_CHARACTERS: usize = 6;
/// The codes that fill in a party member's name: $07, written `NAME`, and the expression $15.
const NAME_CODES: [&str; 2] = ["NAME", "15"];
/// What a number the game fills in is counted as: seven digits.
const NUMBER_CHARACTERS: usize = 7;
/// The codes that fill in a number: $06 with the number itself, and the expressions $14 and $18.
const NUMBER_CODES: [&str; 3] = ["06", "14", "18"];
/// What the name of a thing the game fills in is counted as, an item's or a title's.
const THING_CHARACTERS: usize = 16;
/// The codes that fill in such a name: $08 and $0A with its number, and the expression $16.
const THING_CODES: [&str; 3] = ["08", "0A", "16"];
/// The code that draws a button's picture, which takes one place on a page.
const BUTTON_CODE: &str = "09";
/// The narrowest a build breaks lines so that the pages of a string fit. Lines broken at 31 columns
/// hold at most 30 narrow letters, and four of them fit whatever they hold, so a string goes
/// narrower only where it fills in names or numbers, or has a word too long to break.
const NARROWEST_COLUMNS: usize = 24;

// --- the world maps
/// The members that are the world map, one for each time the story goes to. By the labels their
/// scripts hold for those who made the game: the present (9028, where a new game starts), the future
/// (9029), the present as it was changed (9030), the war of the past (9031) and the present again at
/// the end (9032). The program loads member 9028 + n, n being which of them (the function at
/// $151F58 in memory).
const WORLD_MAPS: RangeInclusive<usize> = 9028..=9032;
/// Which entry of a world map's pack is its script. The field's module asks for a block of memory of
/// the size the pack's table gives this entry, and copies the entry there (the code at $2B5DDC, with
/// the field's module loaded).
const WORLD_MAP_SCRIPT: usize = 19;
/// The most bytes a world map's member may have: the program sets aside a block of this many bytes
/// when it starts (the code at $151E84) and reads the member into it whole. The largest of the
/// game's own has 1,326,615, and the text of a script cannot pass 65,535 bytes (see `rebuild_text`),
/// so a pack's strings do not reach this; it is checked all the same.
const WORLD_MAP_ROOM: usize = 0x18_0000;

// --- the scripts of the games within the game
/// The members among which those scripts are.
const GAME_MEMBERS: RangeInclusive<usize> = 6171..=6301;
/// How such a script starts.
const GAME_MAGIC: &[u8] = b"SFM_";
/// Where its header says how long its data is, and where the data starts. The data is the second
/// of the script's two parts, after its code.
const GAME_DATA_SIZE: usize = 16;
const GAME_DATA_START: usize = 24;
/// The codes of its text, each followed by a 32-bit number: dialogue's own, but for $0A, which
/// here is the line feed that ends a programmer's message.
const GAME_CODES: [u8; 8] = [0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0B];

// --- the enemies
/// The members among which the packs of enemies are.
const ENEMY_MEMBERS: RangeInclusive<usize> = 8178..=8727;
/// How an enemy's record starts, once expanded.
const ENEMY_MAGIC: &[u8] = b"ENd\0";
/// Where the header of an enemy's record lists its eleven sections, an offset and a count for each.
const ENEMY_SECTIONS: usize = 12;
/// Which of the sections starts with the enemy's name.
const ENEMY_NAME_SECTION: usize = 7;
/// The name is in a field of this many bytes, a zero byte ending it.
const ENEMY_NAME_FIELD: usize = 23;
/// An entry of an enemy's pack starts on a multiple of this, and the pack ends on one.
const ENEMY_PACK_ALIGNMENT: usize = 4;
/// The member that holds the list the battle's name plate takes an enemy's name from. It is a pack
/// laid out as an enemy's is, of three compressed entries: a picture, `ALLG` and `TEKI`.
const PLATE_MEMBER: usize = 8063;
/// Its entry that says which enemies each pack of enemies holds, and the magic it starts with.
const PLATE_GROUPS: usize = 1;
const GROUPS_MAGIC: &[u8] = b"ALLG";
/// Its entry that is the list of names, and the magic it starts with.
const PLATE_NAMES: usize = 2;
const PLATES_MAGIC: &[u8] = b"TEKI";
/// What `ALLG` holds for each pack of enemies, in the order of the packs: three numbers of enemies
/// in `TEKI`, how many of each the battle has, and two bytes that are not about names.
const GROUP_SIZE: usize = 8;
const GROUP_KINDS: usize = 3;
/// An entry of `TEKI`: a name in a field padded with spaces, then four bytes, the first of them zero
/// to end the name.
const PLATE_SIZE: usize = 28;
const PLATE_FIELD: usize = 24;
/// The plate writes how many of the enemy there are at this byte of the field, over the name's last
/// two spaces, so a name has this many bytes before it.
const PLATE_ROOM: usize = 22;
const PLATE_PADDING: u8 = b' ';

// --- the menus
/// The members that are the program modules of the menus, one for each screen.
const MENU_MEMBERS: RangeInclusive<usize> = 6798..=6816;
/// Each module is in the archive a second time, compressed, this many members later.
const MENU_TWIN: usize = 19;
/// Where the game loads a menu module in memory: the address its own pointers count from.
const MENU_BASE: u32 = 0x29BB00;
/// Three larger modules that hold text as the menus' do: the quiz, with its prompts; battles,
/// with the labels of the results screen and the messages that explain a newly learned skill; and
/// the field, with the names of places. The game loads the battle's and the field's from their
/// compressed copies in another member (see `COMPRESSED_MODULES`), and the quiz from its own.
const QUIZ_MODULE: usize = 6167;
const BATTLE_MODULE: usize = 8055;
const FIELD_MODULE: usize = 8996;
/// The member the game loads the battle's and the field's modules from: a pack of three compressed
/// blocks, made of how many entries it has, the offset of each, then the entries. The program reads
/// it whole into memory at $43C600 and, when a battle or the field starts, expands the entry it
/// needs to where a large module is loaded (`LARGE_MODULE_BASE`).
const COMPRESSED_MODULES: usize = 17;
/// Which of its entries is which module, by entry and member. Entry 1 is a module that holds no text.
const COMPRESSED_ENTRIES: [(usize, usize); 2] = [(0, BATTLE_MODULE), (2, FIELD_MODULE)];
/// Its entries start on a multiple of this, with zero bytes before them where it takes some, and it
/// ends on one.
const COMPRESSED_ALIGNMENT: usize = 4;
/// The most bytes it may have: the program keeps it in memory from $43C600, and other data starts at
/// $4A8100, which leaves 148 bytes past the game's own. The engine compresses better than the game
/// did, so a module made again with a translation in it can be a little longer and still fit.
const COMPRESSED_MODULES_LIMIT: usize = 0x4A_8100 - 0x43_C600;
/// Where the game loads any of those three.
const LARGE_MODULE_BASE: u32 = 0x2AEA80;

/// The most bytes a menu's module may have: those of the largest of the game's own, member 6806.
/// The game loads a module as long as the archive says it is, and a fan translation made 17 of the
/// 19 longer. But the memory after a module is not free to its end: the name-entry screen works in
/// it from 65,448 bytes after where a module is loaded. No module longer than the largest is known
/// to load, so none is made longer.
const MENU_MODULE_LIMIT: usize = 63_736;

/// What is known of a program module that holds text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module {
    /// Where the game loads it in memory.
    pub base: u32,
    /// How many members after it its compressed copy is, when it has one.
    pub twin: Option<usize>,
    /// Whether its text is written with all that the program's own is: the half-width kana, and
    /// the codes for a colour and for a party member's name. A menu's has neither.
    pub wide: bool,
    /// Whether its text may also write a character in Shift-JIS itself, as a menu's does in a few
    /// strings (see `SHIFT_JIS_LEADS`).
    pub shift_jis: bool,
}

/// A menu's module.
pub const MENU: Module = Module { base: MENU_BASE, twin: Some(MENU_TWIN), wide: false, shift_jis: true };

/// What is known of member `index` of the archive, if it is a module that holds text.
pub fn module_of(index: usize) -> Option<Module> {
    match index {
        index if MENU_MEMBERS.contains(&index) => Some(MENU),
        QUIZ_MODULE | FIELD_MODULE => {
            Some(Module { base: LARGE_MODULE_BASE, twin: None, wide: false, shift_jis: false })
        }
        BATTLE_MODULE => Some(Module { base: LARGE_MODULE_BASE, twin: None, wide: true, shift_jis: false }),
        _ => None,
    }
}

/// The members that are modules holding text, in the archive's order.
fn module_members() -> impl Iterator<Item = usize> {
    [QUIZ_MODULE].into_iter().chain(MENU_MEMBERS).chain([BATTLE_MODULE, FIELD_MODULE])
}
/// The compiler started each string on a multiple of this, and left the bytes before it unused.
const MENU_ALIGNMENT: usize = 8;
/// The codes of menu text, each followed by a 32-bit number as in dialogue: $09 draws a button or
/// asks a question, and $05 comes before a word in a few titles. Dialogue's other codes do not
/// occur, and $0A must not pass for one: it is the line feed that ends a programmer's message.
const MENU_CODES: [u8; 2] = [0x05, 0x09];
/// The lead bytes of Shift-JIS that the game's own two-byte codes do not use. A few strings of the
/// menus write a bracket, a colon, a wide space or a wide capital in Shift-JIS itself among
/// characters written the game's way, and the game draws them: the cooking screen's
/// "オート設定（重複時：　　　　　　）" does, and shows its brackets and colon. Such a character is
/// kept in a script as the game has it, a byte in a tag and the next one as itself.
const SHIFT_JIS_LEADS: RangeInclusive<u8> = 0x81..=0x98;
/// The name the program's own text has in a pack. Its strings are numbered by their offset in the
/// program's file.
const PROGRAM_SECTION: &str = "program";
/// Where the program's strings are looked for in its file: from here on. Before it are the
/// program's code and most of its tables, whose bytes can read as text.
const PROGRAM_DATA: usize = 0xFC000;
/// What is added to an offset in the program's file to get the address it is loaded at: the
/// file's header puts offset $1000 at address $100000.
const PROGRAM_BASE: u32 = 0xFF000;
/// The codes of the program's text, each followed by a 32-bit number: dialogue's own, of which
/// these occur. $07 prints the name of a party member, $09 draws a button, and $0A fills in the name
/// of a thing: one string has it, the prompt to leave a town (see `EXIT_PROMPT`).
const PROGRAM_CODES: [u8; 5] = [0x04, 0x05, 0x07, 0x09, 0x0A];
/// The prompt to leave a town, `program.1109536`: in the game's own, a button, a space, then $0A,
/// whose number the town's name goes in, and "を出る" ("leave"). The game does not look for the code:
/// before it shows the prompt, the code at $10E714 takes the string's address from the program's
/// table (the word at $C7100 of its file) and writes the address of the town's name over the four
/// bytes from the string's eighth, where the game's own has the $0A's number. So a new string must
/// have its $0A at the same place, `EXIT_PROMPT_CODE` bytes from its start, or the address goes over
/// whatever is there (seen on screen: "Quitter" before the code came out as "Qu", a line break and
/// "er", and no name).
const EXIT_PROMPT: u32 = 0x10_EE20;
const EXIT_PROMPT_CODE: usize = 6;
/// The codes of the battle module's text: the program's, but for $0A, which it does not have. Its
/// strings were found with these, and are not to change.
const BATTLE_CODES: [u8; 4] = [0x04, 0x05, 0x07, 0x09];
/// The name in a pack of the script the program holds itself (see `PROGRAM_SCRIPT`). Its strings are
/// numbered as a story script's are, from 0 in the order of its text.
const PROGRAM_SCRIPT_SECTION: &str = "program-script";
/// A story script, `SCED`, that the program holds among its own data, at this offset of its file: the
/// messages of a treasure chest (an item, money or lenses found, a chest that is empty, a bag that is
/// full), the caption over the world map after a battle is lost, and the message that a title was
/// earned. Its code is a list of 16 statements of six bytes, each naming one string; to show message
/// `n`, the program's function at $121400 takes the script's address, reads statement `n` and shows
/// the string it names. Four places in the code give that function this address, and nothing else
/// refers to the script or to a string of it.
const PROGRAM_SCRIPT: usize = 0xCF_A40;
/// Where the script ends in the program's file: the byte after its last string's zero byte is a
/// variable the program writes as it runs (at $1CED09 in memory), so the script cannot grow.
const PROGRAM_SCRIPT_END: usize = 0xCF_D09;
/// The string of that script that is a list to choose from: lenses found that do not all fit, then
/// how many to take and leave, or to put them back in the chest. The program asks for the choice
/// itself, so nothing in the script's code shows it is a list.
const PROGRAM_SCRIPT_LISTS: [u32; 1] = [12];
/// The addresses the console's memory has that a program or its data can be at: a word in this
/// range is taken for an address and not for text, though its bytes may read as a character.
const ADDRESSES: Range<u32> = 0x0010_0000..0x0200_0000;
/// Where the program's code is in its file, by the file's own list of its parts.
const PROGRAM_CODE: Range<usize> = 0x1000..0x9985C;
/// Where the program's tables are in its file: the part of its data it may change as it runs,
/// which is where the compiler put every table of pointers. The strings are in the part after it.
const PROGRAM_TABLES: Range<usize> = 0xB7580..0xFE658;
/// The furthest apart, in bytes, that the pointers of one column of a table are looked for.
const COLUMN_STEP: usize = 256;
/// How far apart the three strings of a column's pointers may be, counted in strings in the order
/// the file has them. A table's strings were put in the file together.
const COLUMN_NEAR: usize = 64;
// --- the encoding
const END: u8 = 0x00;
const LINE: u8 = 0x01;
/// Two-byte codes per lead byte.
const ROW_SIZE: usize = 187;
const FIRST_TRAIL: u8 = 0x40;
const LAST_TRAIL: u8 = 0xFC;
const SKIPPED_TRAILS: [u8; 2] = [0x5C, 0x7F];
/// The lead bytes, as ranges that stop before their second number.
const LEAD_RANGES: [(u8, u8); 2] = [(0x99, 0xA0), (0xE0, 0xF0)];
/// Single bytes: half-width kana, U+FF61 onwards.
const FIRST_KANA: u8 = 0xA1;
const LAST_KANA: u8 = 0xDF;
const HALF_WIDTH: u32 = 0xFF61;
/// Codes followed by a 32-bit number. $07 is the only one whose effect has been seen on screen.
const NUMBER_SIZE: usize = 4;
static CODES: LazyLock<ControlCodes> = LazyLock::new(|| {
    let parameters: Vec<(u32, usize)> = (0x03..0x0C).map(|code| (code, NUMBER_SIZE)).collect();
    ControlCodes::new(&parameters, &[(0x07, "NAME")])
});
/// Bytes that start an expression, and the byte that ends one.
const EXPRESSIONS: [u8; 5] = [0x12, 0x14, 0x15, 0x16, 0x18];
const EXPRESSION_END: u8 = 0xC0;

// --- the font
/// The width; a glyph has 22 rows, and is drawn here with two empty ones below.
pub const GLYPH_SIZE: usize = 24;
pub const GLYPH_ROWS: usize = 22;
pub const GLYPH_BYTES: usize = 0x110;
pub const GLYPH_COUNT: usize = 2560;
/// The built-in fonts that new characters are drawn from when a build is given none: a font drawn
/// for the game's half-width cell, for the letters it has, then one that has every other character.
const FONTS: &[&str] = &["dejavu-condensed-bold", "unifont"];
/// How the font command shows the 16 levels.
pub const GLYPH_SHADES: [u8; 16] = [0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255];
/// In the executable: for each byte from $20 to $7F, the glyph that draws it, at half the width of
/// a two-byte character. The glyphs are the font's first 256; lowercase letters are sent to the
/// capitals.
const ASCII_TABLE: u64 = 0xC9D00;
const ASCII_TABLE_SIZE: u64 = 0x60;
/// In the main font: how many rows of a cell are above the line letters stand on.
const BASELINE: i64 = 19;
// New glyphs, in the 16 levels of the game's fonts. The palettes draw 5 to 15 from dim to bright
// and keep 1 to 4 for the dark line around a letter.
const INK_LEVEL: u8 = 15;
/// The empty pixel directly under ink: half bright.
const UNDER_LEVEL: u8 = 8;
/// Left or right of ink.
const BESIDE_LEVEL: u8 = 4;
const ABOVE_LEVEL: u8 = 3;
/// Touching ink only at a corner.
const CORNER_LEVEL: u8 = 2;
/// Further out than that.
const FAR_LEVEL: u8 = 1;
/// How many pixels from the ink the dark surround reaches.
const SURROUND: i64 = 2;
/// How many rows each pixel of a letter's ink is repeated downwards. The bitmap fonts a pack is
/// likely to bring draw a horizontal stroke one row thick; the game's own letters have them two
/// thick.
const STROKE_DOWN: i64 = 1;
/// Of a stretched glyph: the share of a pixel it must cover for the pixel to count as ink.
const INK_FROM: f64 = 0.2;
/// And the share from which the pixel is as bright as ink gets.
const SOLID_FROM: f64 = 0.6;
/// Of a large drawing reduced to a cell: the share of a pixel that must be ink for it to show at
/// all, and the share from which it is as bright as ink gets.
const EDGE_FROM: f64 = 0.08;
const SOLID_FROM_REDUCED: f64 = 0.92;
/// The half-width font: a compressed texture in the executable, 128 pixels wide, holding the main
/// font's first 217 glyphs at half width, ten to a row, in cells of 12 by 16 pixels. Single bytes
/// are drawn from it.
const HALF_FONT: u64 = 0xCA238;
/// Where the pixels start in the expanded texture.
const HALF_FONT_PIXELS: usize = 0x340;
const HALF_ROW_BYTES: usize = 64;
const HALF_WIDTH_CELL: usize = 12;
const HALF_HEIGHT_CELL: usize = 16;
const HALF_PER_ROW: usize = 10;
/// How many rows of a half-width cell are above the line the new letters stand on. The game's own
/// capitals stand one row lower; this leaves three rows below, which a letter such as g needs.
const HALF_BASELINE: i64 = 13;
/// What is drawn again from the pack's font whenever it adds a letter, in the cells the game has
/// them: its own capitals and digits are of another hand, and a word would otherwise mix the two.
const LATIN_SET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// To bytes $A0-$DF what `ASCII_TABLE` is to $20-$7F.
const KANA_TABLE: u64 = 0xC9D80;
const KANA_TABLE_SIZE: u64 = 0x40;
/// Cells of the half-width font that no byte is drawn from: the hiragana. New characters go there.
const HALF_SPARE_CELLS: Range<usize> = 49..132;
/// Single bytes that can be given to new characters: the half-width kana from $A6 up, which the
/// story scripts do not use. $A1-$A5 are punctuation, $A5 the dot of the script's ellipsis.
const SPARE_BYTES: Range<u8> = 0xA6..0xE0;

// --- the archive

/// The archive's table of contents, read once for an image.
fn table(disc: &Disc) -> Result<Arc<Vec<u32>>> {
    disc.keep("tod2-ps2: the archive's table of contents", || {
        let data = disc.read(EXECUTABLE, TABLE, 4 * (MEMBER_COUNT as u64 + 1))?;
        let table: Vec<u32> = (0..data.len() / 4).filter_map(|entry| number(&data, entry * 4)).collect();
        if table.len() != MEMBER_COUNT + 1 || u64::from(table[MEMBER_COUNT]) != disc.size(ARCHIVE)? {
            return text_error(format!(
                "{EXECUTABLE} does not hold the table of contents of {ARCHIVE} where it is expected"
            ));
        }
        Ok(table)
    })
}

/// Where a member of the archive is: its offset in the archive and its size. An entry of the table
/// that is out of order gives a size below zero.
pub fn member_span(disc: &Disc, index: usize) -> Result<(u64, i64)> {
    if index >= MEMBER_COUNT {
        return text_error(format!("{ARCHIVE} has no member {index}"));
    }
    let table = table(disc)?;
    let offset = table[index] & !PADDING_BITS;
    let size = i64::from(table[index + 1] & !PADDING_BITS) - i64::from(offset) - i64::from(table[index] & PADDING_BITS);
    Ok((u64::from(offset), size))
}

/// A member of the archive, expanded if it is one compressed block.
pub fn read_member(disc: &Disc, index: usize) -> Result<Vec<u8>> {
    let (offset, size) = member_span(disc, index)?;
    let data = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
    if lzss_ring::is_block(&data) { lzss_ring::decompress_block(&data, 0) } else { Ok(data) }
}

/// The numbers of the members that are story scripts.
pub fn script_members(disc: &Disc) -> Result<Vec<usize>> {
    let mut found = Vec::new();
    for index in 0..MEMBER_COUNT {
        let (offset, size) = member_span(disc, index)?;
        if size >= 16 && disc.read(ARCHIVE, offset, 4)? == CONTAINER_MAGIC {
            found.push(index);
        }
    }
    Ok(found)
}

/// The text of a story script: everything from where its header says the text starts.
pub fn read_script_text(disc: &Disc, index: usize) -> Result<Vec<u8>> {
    let mut data = read_script_data(disc, index)?;
    let start = number(&data, 8).map_or(data.len(), |start| data.len().min(start as usize));
    Ok(data.split_off(start))
}

/// A story script, expanded: its header, its code, then its text.
pub fn read_script_data(disc: &Disc, index: usize) -> Result<Vec<u8>> {
    let no_script = || text_error(format!("member {index} of {ARCHIVE} does not end with a script"));
    let (offset, _) = member_span(disc, index)?;
    let Some(count) = number(&disc.read(ARCHIVE, offset + 8, 4)?, 0).map(u64::from) else { return no_script() };
    let sizes = disc.read(ARCHIVE, offset + 16, 4 * count)?;
    let sizes: Vec<u64> = (0..sizes.len() / 4).filter_map(|entry| number(&sizes, entry * 4)).map(u64::from).collect();
    let Some((last, before)) = sizes.split_last() else { return no_script() };
    let packed = disc.read(ARCHIVE, offset + 16 + 4 * count + before.iter().sum::<u64>(), *last)?;
    let data = lzss_ring::decompress_block(&packed, 0)?;
    if !data.starts_with(SCRIPT_MAGIC) {
        return no_script();
    }
    Ok(data)
}

/// The text of a script: everything from where its header says the text starts.
fn text_of(script: &[u8]) -> &[u8] {
    &script[number(script, 8).map_or(script.len(), |start| script.len().min(start as usize))..]
}

/// Where each entry of a pack is, as an offset and a size. A pack starts with how many entries it
/// has, then those two numbers for each. `None` if `pack` stops before the end of that table.
fn pack_entries(pack: &[u8]) -> Option<Vec<(usize, usize)>> {
    let count = number(pack, 0)? as usize;
    let entry = |entry: usize| Some((number(pack, 4 + 8 * entry)? as usize, number(pack, 8 + 8 * entry)? as usize));
    (0..count).map(entry).collect()
}

/// Where each entry of a world map's pack is. The pack starts with how many entries it has, then the
/// size of each; the entries follow one another with nothing between them, to the end of the pack.
/// `None` if `pack` is not laid out so.
fn world_map_entries(pack: &[u8]) -> Option<Vec<Range<usize>>> {
    let count = number(pack, 0)? as usize;
    let mut position = count.checked_mul(4)?.checked_add(4)?;
    let mut entries = Vec::new();
    for entry in 0..count {
        let end = position.checked_add(number(pack, 4 + 4 * entry)? as usize)?;
        entries.push(position..end);
        position = end;
    }
    (position == pack.len()).then_some(entries)
}

/// The script of a world map's pack: its entry `WORLD_MAP_SCRIPT`, if that is a script.
fn world_map_script(pack: &[u8]) -> Option<&[u8]> {
    let script = &pack[world_map_entries(pack)?.get(WORLD_MAP_SCRIPT)?.clone()];
    let text_start = number(script, 8)? as usize;
    (script.starts_with(SCRIPT_MAGIC) && text_start <= script.len()).then_some(script)
}

/// The scripts of the world maps, by the number of their member in the archive, read once for an
/// image: each its header, its code and its text.
pub fn world_map_scripts(disc: &Disc) -> Result<Arc<BTreeMap<usize, Vec<u8>>>> {
    disc.keep("tod2-ps2: the world maps", || {
        let mut found = BTreeMap::new();
        for index in WORLD_MAPS {
            let (offset, size) = member_span(disc, index)?;
            let pack = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
            let script = world_map_script(&pack).ok_or_else(|| {
                Error::Text(format!("member {index} of {ARCHIVE} does not hold a world map's script where expected"))
            })?;
            found.insert(index, script.to_vec());
        }
        Ok(found)
    })
}

/// A pack of the entries given: the table, then each entry starting on a multiple of 16, with zero
/// bytes before it where it takes some to get there.
fn pack_of(entries: &[&[u8]]) -> Vec<u8> {
    pack_on(entries, PACK_ALIGNMENT)
}

/// A pack of the entries given, each starting on a multiple of `alignment`.
fn pack_on(entries: &[&[u8]], alignment: usize) -> Vec<u8> {
    let mut pack = (entries.len() as u32).to_le_bytes().to_vec();
    let mut position = (4 + 8 * entries.len()).next_multiple_of(alignment);
    for entry in entries {
        pack.extend((position as u32).to_le_bytes());
        pack.extend((entry.len() as u32).to_le_bytes());
        position = (position + entry.len()).next_multiple_of(alignment);
    }
    for entry in entries {
        pack.resize(pack.len().next_multiple_of(alignment), 0);
        pack.extend_from_slice(entry);
    }
    pack
}

/// Strings as the game stores them, each under its number in its section.
type Numbered = BTreeMap<u32, Vec<u8>>;

/// Where the data of a script of a game within the game is in it, if `script` is one.
fn game_data(script: &[u8]) -> Option<Range<usize>> {
    if !script.starts_with(GAME_MAGIC) {
        return None;
    }
    let start = number(script, GAME_DATA_START)? as usize;
    let end = start.checked_add(number(script, GAME_DATA_SIZE)? as usize)?;
    (start <= end && end <= script.len()).then_some(start..end)
}

/// How long the text at `offset` of such a script is, if what is there is text.
fn game_text_length(script: &[u8], offset: usize, chars: &Characters) -> Option<usize> {
    text_length(script, offset, chars, &GAME_CODES, true, false)
}

/// The strings of a script of a game within the game, each under its offset in the script, without
/// the zero byte that ends it.
///
/// The script's data holds its strings among much else, and the script refers to a string by its
/// offset counted from the start of the data, as a 32-bit number in its code or in a table. So a
/// string is what starts after a zero byte in the data, reads as text, and has that number
/// written somewhere in the script. Pictures in the data can read as text; nothing refers to them.
///
/// A string may also follow other data with no zero byte between, as the name of the quiz's book
/// does in member 6203, after six 16-bit numbers. Such a string is taken where a table refers to
/// it: a number that sits right after or right before the number of a string found the first
/// way. Taking every number that points at text after other data would also take pictures and
/// the middles of strings, some 700 places in the game's scripts; this takes that one string and
/// nothing else.
pub fn game_strings(script: &[u8], chars: &Characters) -> Numbered {
    let mut strings = BTreeMap::new();
    let Some(data) = game_data(script) else { return strings };
    let at_word = |at: usize| number(script, at);
    let written: HashSet<u32> = (0..script.len().saturating_sub(3)).filter_map(at_word).collect();
    let mut at = data.start;
    while at < data.end {
        let starts = (at == data.start || script[at - 1] == END) && script[at] != END;
        match starts.then(|| game_text_length(&script[..data.end], at, chars)).flatten() {
            Some(length) => {
                if written.contains(&((at - data.start) as u32)) {
                    strings.insert(at as u32, script[at..at + length].to_vec());
                }
                at += length + 1; // text that nothing refers to is passed over whole, not read from its middle
            }
            None => at += 1,
        }
    }
    // Then the strings a table refers to that follow other data. One is not taken where it would
    // start inside a string already found, or run into one.
    let found: Vec<(usize, usize)> = strings.iter().map(|(at, string)| (*at as usize, string.len())).collect();
    let numbers: HashSet<u32> = found.iter().map(|(at, _)| (at - data.start) as u32).collect();
    let mut more = BTreeMap::new();
    for word in 0..script.len().saturating_sub(3) {
        let Some(at) = at_word(word).and_then(|number| data.start.checked_add(number as usize)) else { continue };
        let mut beside = [word.checked_sub(4), Some(word + 4)].into_iter().flatten().filter_map(at_word);
        if at <= data.start || at >= data.end || script[at - 1] == END || !beside.any(|n| numbers.contains(&n)) {
            continue;
        }
        let Some(length) = game_text_length(&script[..data.end], at, chars) else { continue };
        if found.iter().all(|(start, size)| at + length < *start || *start + size < at) {
            more.insert(at as u32, script[at..at + length].to_vec());
        }
    }
    strings.extend(more);
    strings
}

/// The scripts of the games within the game, expanded, by the number of their member in the
/// archive, read once for an image.
pub fn game_scripts(disc: &Disc) -> Result<Arc<BTreeMap<usize, Vec<u8>>>> {
    disc.keep("tod2-ps2: the games within the game", || {
        let mut found = BTreeMap::new();
        for index in GAME_MEMBERS {
            let script = read_member(disc, index)?;
            if game_data(&script).is_some() {
                found.insert(index, script);
            }
        }
        Ok(found)
    })
}

/// Where the field that holds the name is in an enemy's record, if `record` is one.
fn enemy_name_place(record: &[u8]) -> Option<usize> {
    if !record.starts_with(ENEMY_MAGIC) {
        return None;
    }
    let at = number(record, ENEMY_SECTIONS + 8 * ENEMY_NAME_SECTION)? as usize;
    (at.checked_add(ENEMY_NAME_FIELD)? <= record.len()).then_some(at)
}

/// The name in an enemy's record, without the zero byte that ends it.
fn enemy_name(record: &[u8]) -> Option<&[u8]> {
    let at = enemy_name_place(record)?;
    record[at..at + ENEMY_NAME_FIELD].split(|byte| *byte == END).next()
}

/// The records of the enemies in a pack, expanded, each with the number of its entry. An enemy's
/// pack holds, for each of its enemies, a record, a picture and its movements, every entry a
/// compressed block; a member that is no such pack gives none.
fn enemy_records(pack: &[u8]) -> Vec<(usize, Vec<u8>)> {
    let mut records = Vec::new();
    for (entry, (at, size)) in pack_entries(pack).unwrap_or_default().into_iter().enumerate() {
        let Some(block) = at.checked_add(size).and_then(|end| pack.get(at..end)) else { return Vec::new() };
        // Only the start is expanded to see what an entry is: a picture is large.
        let is_record = lzss_ring::is_block(block)
            && lzss_ring::decompress_start(block, ENEMY_MAGIC.len()).is_ok_and(|start| start == ENEMY_MAGIC);
        if is_record
            && let Ok(record) = lzss_ring::decompress_block(block, 0)
            && enemy_name_place(&record).is_some()
        {
            records.push((entry, record));
        }
    }
    records
}

/// The name of every enemy, by the member that is its pack and by the number of its entry in the
/// pack, as the game stores them, read once for an image.
pub fn enemy_names(disc: &Disc) -> Result<Arc<BTreeMap<usize, Numbered>>> {
    disc.keep("tod2-ps2: the enemies", || {
        let mut found = BTreeMap::new();
        for index in ENEMY_MEMBERS {
            let (offset, size) = member_span(disc, index)?;
            let pack = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
            let names: BTreeMap<u32, Vec<u8>> = enemy_records(&pack)
                .iter()
                .filter_map(|(entry, record)| Some((*entry as u32, enemy_name(record)?.to_vec())))
                .collect();
            if !names.is_empty() {
                found.insert(index, names);
            }
        }
        Ok(found)
    })
}

/// An enemy's pack with some of its enemies renamed: `names` gives the new name of an enemy by the
/// number of its entry, in the game's bytes and no longer than the field. The record is compressed
/// again the way it was, and the pack written out again around it.
pub fn rename_enemies(pack: &[u8], names: &BTreeMap<u32, Vec<u8>>, section: &str) -> Result<Vec<u8>> {
    let broken = || Error::Text(format!("{section} is not laid out as a pack of enemies is"));
    let places = pack_entries(pack).ok_or_else(broken)?;
    let part = |(at, size): &(usize, usize)| pack.get(*at..at.checked_add(*size)?);
    let mut entries: Vec<Vec<u8>> =
        places.iter().map(|place| part(place).map(<[u8]>::to_vec)).collect::<Option<_>>().ok_or_else(broken)?;
    for (number, name) in names {
        let entry = entries.get_mut(*number as usize).ok_or_else(broken)?;
        let (mode, _, _) = lzss_ring::read_header(entry, 0)?;
        let mut record = lzss_ring::decompress_block(entry, 0)?;
        let at = enemy_name_place(&record).ok_or_else(broken)?;
        let field = &mut record[at..at + ENEMY_NAME_FIELD];
        field.fill(0);
        field[..name.len()].copy_from_slice(name);
        *entry = lzss_ring::compress_block(&record, mode)?;
    }
    let mut pack = pack_on(&entries.iter().map(Vec::as_slice).collect::<Vec<_>>(), ENEMY_PACK_ALIGNMENT);
    pack.resize(pack.len().next_multiple_of(ENEMY_PACK_ALIGNMENT), 0);
    Ok(pack)
}

/// The entries of the member that holds the name plates' list, `ALLG` and `TEKI`, expanded, if
/// `member` is laid out as that member is.
fn plate_lists(member: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let entries = pack_entries(member)?;
    let expanded = |entry: usize| {
        let (at, size) = *entries.get(entry)?;
        lzss_ring::decompress_block(member.get(at..at.checked_add(size)?)?, 0).ok()
    };
    let groups = expanded(PLATE_GROUPS).filter(|groups| groups.starts_with(GROUPS_MAGIC))?;
    let names = expanded(PLATE_NAMES).filter(|names| names.starts_with(PLATES_MAGIC))?;
    Some((groups, names))
}

/// The name in entry `kind` of `TEKI`, without the spaces that pad it, if the list has that entry.
fn plate_name(names: &[u8], kind: usize) -> Option<&[u8]> {
    let at = PLATES_MAGIC.len() + kind * PLATE_SIZE;
    let field = names.get(at..at + PLATE_FIELD)?;
    let end = field.iter().rposition(|byte| *byte != PLATE_PADDING).map_or(0, |last| last + 1);
    Some(&field[..end])
}

/// Which entry of `TEKI` each enemy is, by the member that is its pack and the number of its entry
/// in the pack.
///
/// The game keeps no such number in an enemy's record. When a battle starts, the battle's module
/// reads what `ALLG` holds for the battle's pack (its first entry for member 8178, the next for
/// 8179, and so on) and copies the name of each of its enemies from `TEKI` to the plate. A pack's
/// records come in the same order as the enemies `ALLG` gives it, those of which the battle has
/// none left out: so it is in all 550 of the game's packs, and the names agree. Where a pack's
/// names do not agree with the list, that is said.
fn plate_kinds(enemies: &BTreeMap<usize, Numbered>, groups: &[u8], names: &[u8]) -> Result<HashMap<(usize, u32), u8>> {
    let mut kinds = HashMap::new();
    for (pack, records) in enemies {
        let at = GROUPS_MAGIC.len() + (pack - ENEMY_MEMBERS.start()) * GROUP_SIZE;
        let group = groups.get(at..at + GROUP_SIZE).unwrap_or_default();
        let present = (0..GROUP_KINDS).filter(|kind| group.get(GROUP_KINDS + kind).is_some_and(|count| *count != 0));
        let group_kinds: Vec<u8> = present.map(|kind| group[kind]).collect();
        let agree = group_kinds.len() == records.len()
            && records
                .values()
                .zip(&group_kinds)
                .all(|(name, kind)| plate_name(names, usize::from(*kind)) == Some(name));
        if !agree {
            return text_error(format!(
                "the enemies of {} do not agree with the list the battle's name plate takes their names from \
                 (member {PLATE_MEMBER} of {ARCHIVE})",
                section_name(*pack)
            ));
        }
        kinds.extend(records.keys().zip(group_kinds).map(|(entry, kind)| ((*pack, *entry), kind)));
    }
    Ok(kinds)
}

/// The member that holds the name plates' list, with some of its names changed: `names` gives the
/// new name of an entry of `TEKI` by its number, in the game's bytes. A name is padded with spaces
/// to the end of its field, as the game's own are, so that how many of the enemy there are stays
/// where the plate writes it; the four bytes after the field are left alone. `TEKI` is compressed
/// again the way it was, and the member written out again around it.
pub fn rename_plates(member: &[u8], names: &BTreeMap<u8, Vec<u8>>) -> Result<Vec<u8>> {
    let broken = || Error::Text(format!("member {PLATE_MEMBER} of {ARCHIVE} is not laid out as the plates' list is"));
    let places = pack_entries(member).ok_or_else(broken)?;
    let part = |(at, size): &(usize, usize)| member.get(*at..at.checked_add(*size)?);
    let mut entries: Vec<Vec<u8>> =
        places.iter().map(|place| part(place).map(<[u8]>::to_vec)).collect::<Option<_>>().ok_or_else(broken)?;
    if names.is_empty() {
        return Ok(member.to_vec());
    }
    let (_, mut list) = plate_lists(member).ok_or_else(broken)?;
    for (kind, name) in names {
        if name.len() > PLATE_ROOM {
            return text_error(format!(
                "enemy {kind} of the name plates takes {} bytes and has room for {PLATE_ROOM}",
                name.len()
            ));
        }
        let at = PLATES_MAGIC.len() + usize::from(*kind) * PLATE_SIZE;
        let field = list.get_mut(at..at + PLATE_FIELD).ok_or_else(broken)?;
        field.fill(PLATE_PADDING);
        field[..name.len()].copy_from_slice(name);
    }
    let entry = entries.get_mut(PLATE_NAMES).ok_or_else(broken)?;
    let (mode, _, _) = lzss_ring::read_header(entry, 0)?;
    *entry = lzss_ring::compress_block(&list, mode)?;
    let mut rebuilt = pack_on(&entries.iter().map(Vec::as_slice).collect::<Vec<_>>(), ENEMY_PACK_ALIGNMENT);
    rebuilt.resize(rebuilt.len().next_multiple_of(ENEMY_PACK_ALIGNMENT), 0);
    Ok(rebuilt)
}

/// The entries of the member that holds the compressed modules (see `COMPRESSED_MODULES`), each a
/// compressed block, without the zero bytes that follow it. The member starts with how many entries
/// it has and then the offset of each; an entry's block says how long it is. `None` if `member` is
/// not laid out so.
fn compressed_entries(member: &[u8]) -> Option<Vec<&[u8]>> {
    let count = number(member, 0)? as usize;
    let entry = |entry: usize| {
        let at = number(member, 4 + 4 * entry)? as usize;
        let (_, payload, _) = lzss_ring::read_header(member, at).ok()?;
        member.get(at..at.checked_add(lzss_ring::HEADER_SIZE + payload)?)
    };
    (0..count).map(entry).collect()
}

/// The member that holds the compressed modules, with some of them changed: `modules` gives the new
/// contents of a module by its entry. Each is compressed again the way its entry was, and the member
/// is written out again around it, each entry starting on a multiple of 4; the entries not given keep
/// their bytes. Refused if the member would pass the memory the game keeps it in.
pub fn rebuild_compressed_modules(member: &[u8], modules: &BTreeMap<usize, Vec<u8>>) -> Result<Vec<u8>> {
    let broken = || {
        Error::Text(format!(
            "member {COMPRESSED_MODULES} of {ARCHIVE} is not laid out as the compressed battle and field modules are"
        ))
    };
    let mut entries: Vec<Vec<u8>> =
        compressed_entries(member).ok_or_else(broken)?.into_iter().map(<[u8]>::to_vec).collect();
    for (entry, module) in modules {
        let block = entries.get_mut(*entry).ok_or_else(broken)?;
        let (mode, _, _) = lzss_ring::read_header(block, 0)?;
        *block = lzss_ring::compress_block(module, mode)?;
    }
    let mut rebuilt = (entries.len() as u32).to_le_bytes().to_vec();
    let mut position = (4 + 4 * entries.len()).next_multiple_of(COMPRESSED_ALIGNMENT);
    for entry in &entries {
        rebuilt.extend((position as u32).to_le_bytes());
        position = (position + entry.len()).next_multiple_of(COMPRESSED_ALIGNMENT);
    }
    for entry in &entries {
        rebuilt.resize(rebuilt.len().next_multiple_of(COMPRESSED_ALIGNMENT), 0);
        rebuilt.extend_from_slice(entry);
    }
    rebuilt.resize(rebuilt.len().next_multiple_of(COMPRESSED_ALIGNMENT), 0);
    if rebuilt.len() > COMPRESSED_MODULES_LIMIT {
        return text_error(format!(
            "the battle's and the field's modules, compressed again with the strings the pack changes in 08055 and \
             08996, take {} bytes, and the game keeps them in memory where they have room for \
             {COMPRESSED_MODULES_LIMIT}, since other data follows them there; shorten some of those strings",
            rebuilt.len()
        ));
    }
    Ok(rebuilt)
}

/// The script of a skit, if member `index` of the archive is one: its header, its code, then its
/// text. A skit is a compressed pack that holds its pictures as well, so only as much of it is
/// read and expanded as it takes to reach the end of the script.
fn read_skit_script(disc: &Disc, index: usize) -> Result<Option<Vec<u8>>> {
    let (offset, size) = member_span(disc, index)?;
    let size = size.max(0) as u64;
    let start_of = |length: usize| disc.read(ARCHIVE, offset, size.min(lzss_ring::enough_for(length) as u64));
    let mut data = start_of(SKIT_START)?;
    let is_block = lzss_ring::read_header(&data, 0).is_ok_and(|(mode, payload, _)| {
        let known = matches!(mode, lzss_ring::MODE_LZSS | lzss_ring::MODE_LZSS_RLE);
        known && (lzss_ring::HEADER_SIZE + payload) as u64 == size
    });
    if !is_block {
        return Ok(None);
    }
    let Ok(table) = lzss_ring::decompress_start(&data, 4 + 8 * SKIT_ENTRIES) else { return Ok(None) };
    if number(&table, 0) != Some(SKIT_ENTRIES as u32) {
        return Ok(None);
    }
    let Some((at, length)) = pack_entries(&table).map(|entries| entries[SKIT_SCRIPT]) else { return Ok(None) };
    let end = at.saturating_add(length);
    if end > SKIT_START {
        data = start_of(end)?;
    }
    let Ok(pack) = lzss_ring::decompress_start(&data, end) else { return Ok(None) };
    Ok(pack.get(at..end).filter(|script| script.starts_with(SCRIPT_MAGIC)).map(<[u8]>::to_vec))
}

/// The script of every skit, by the number of its member in the archive, read once for an image.
pub fn skit_scripts(disc: &Disc) -> Result<Arc<BTreeMap<usize, Vec<u8>>>> {
    disc.keep("tod2-ps2: the skits", || {
        let mut found = BTreeMap::new();
        for index in SKIT_MEMBERS {
            if let Some(script) = read_skit_script(disc, index)? {
                found.insert(index, script);
            }
        }
        Ok(found)
    })
}

/// The modules that hold text as the game has them, by the number of their member in the archive,
/// read once for an image.
pub fn menu_modules(disc: &Disc) -> Result<Arc<BTreeMap<usize, Vec<u8>>>> {
    disc.keep("tod2-ps2: the modules", || {
        module_members()
            .map(|index| {
                let (offset, size) = member_span(disc, index)?;
                Ok((index, disc.read(ARCHIVE, offset, size.max(0) as u64)?))
            })
            .collect()
    })
}

/// How long the text at `offset` of `data` is, if what is there is text: characters of the game's
/// list, ASCII, line breaks and the codes given with their 32-bit numbers, up to a zero byte that
/// is not part of such a number, with at least one character of the list among them. With `kana`,
/// the single bytes that are half-width kana pass too; the program's text has them, for the dot of
/// its ellipsis, and a menu module's has not. With `shift_jis`, so does a character of the font
/// written in Shift-JIS itself, with a lead byte the game's own codes do not use (see
/// `SHIFT_JIS_LEADS`). It does not count as a character of the list: what has no character written
/// the game's way is no text, as a module's formats for `sprintf` and its programmers' notes,
/// written in Shift-JIS, are not.
fn text_length(
    data: &[u8],
    offset: usize,
    chars: &Characters,
    codes: &[u8],
    kana: bool,
    shift_jis: bool,
) -> Option<usize> {
    // A character of the list, written with the glyph the game's own text writes it with.
    let listed = |lead: u8, trail: u8| {
        let glyph = glyph_of(u16::from_be_bytes([lead, trail]));
        let char = glyph.and_then(|glyph| chars.by_glyph.get(glyph).copied().flatten());
        char.is_some_and(|char| chars.glyphs.get(&char).copied() == glyph)
    };
    // A character the font has, written in Shift-JIS.
    let in_font =
        |lead: u8, trail: u8| shift_jis::character(lead, trail).is_some_and(|char| chars.glyphs.contains_key(&char));
    let (mut position, mut characters) = (offset, 0);
    loop {
        let byte = *data.get(position)?;
        let next = data.get(position + 1).copied();
        position += match (byte, next) {
            (END, _) => break,
            (lead, Some(trail)) if is_lead(lead) && is_trail(trail) && listed(lead, trail) => {
                characters += 1;
                2
            }
            (lead, Some(trail)) if shift_jis && SHIFT_JIS_LEADS.contains(&lead) && in_font(lead, trail) => 2,
            (code, _) if codes.contains(&code) => 1 + NUMBER_SIZE,
            (LINE | 0x20..=0x7E, _) => 1,
            (FIRST_KANA..=LAST_KANA, _) if kana => 1,
            _ => return None,
        };
    }
    (characters > 0 && position <= data.len()).then_some(position - offset)
}

/// How long the text at `offset` of a module is, if what is there is text; `about` says how the
/// module's text is written. A module also holds its programmers' notes and formats, in Shift-JIS,
/// and tables that something points at; neither passes for text.
fn menu_text_length(module: &[u8], offset: usize, chars: &Characters, about: &Module) -> Option<usize> {
    if about.wide {
        text_length(module, offset, chars, &BATTLE_CODES, true, about.shift_jis)
    } else {
        text_length(module, offset, chars, &MENU_CODES, false, about.shift_jis)
    }
}

/// Where a module's code ends and its data starts: just past the last instruction that returns
/// from a function.
fn code_end(module: &[u8]) -> usize {
    let returns = (0..module.len() / 4).rev().find(|word| number(module, 4 * word) == Some(RETURN));
    returns.map_or(0, |word| 4 * word + 4)
}

/// The strings among the places of `data` that something may point at, each under its offset,
/// without the zero byte that ends it: `length` says how long the text at a place is, if it is
/// text. A string starts on a multiple of 8, as the compiler put every string, and does not start
/// inside the string before it. A word that is an address is none, though it may read as a
/// character and a bracket.
fn strings_among(
    data: &[u8],
    targets: &BTreeSet<usize>,
    length: impl Fn(usize) -> Option<usize>,
) -> BTreeMap<u32, Vec<u8>> {
    let mut strings = BTreeMap::new();
    let mut free = 0; // where the string found last stops being its own: just past the zero byte that ends it
    for offset in targets.iter().copied() {
        if offset < free || offset % MENU_ALIGNMENT != 0 {
            continue;
        }
        let Some(length) = length(offset) else { continue };
        if length < 4 && number(data, offset).is_some_and(|word| ADDRESSES.contains(&word)) {
            continue;
        }
        strings.insert(offset as u32, data[offset..offset + length].to_vec());
        free = offset + length + 1;
    }
    strings
}

/// The strings of a module, each under its offset in the module, without the zero byte that ends
/// it: what something in the module may point at, in its data, that reads as text. `about` is what
/// is known of the module.
pub fn menu_strings(module: &[u8], about: &Module, chars: &Characters) -> BTreeMap<u32, Vec<u8>> {
    let data = code_end(module);
    let targets: BTreeSet<usize> = pointed_at(module, about.base).into_iter().filter(|at| *at >= data).collect();
    strings_among(module, &targets, |offset| menu_text_length(module, offset, chars, about))
}

/// The program's file, read once for an image.
fn program_file(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("tod2-ps2: the program", || disc.read(EXECUTABLE, 0, disc.size(EXECUTABLE)?))
}

/// The places in the program's data that the program refers to, as offsets in its file: what one
/// of its 32-bit words points at, and what its code builds the address of. The lists the program
/// holds (items, titles, recipes and so on) are tables of such words. Some thirty short strings
/// are known to the code alone, most of them a word or two that it puts into a sentence.
///
/// What the code reads straight from memory is left out: nearly all of it is numbers, and one, a
/// tenth kept twice over, reads as text.
pub fn program_targets(program: &[u8]) -> BTreeSet<usize> {
    let words = (0..program.len() / 4).filter_map(|word| number(program, 4 * word));
    let addresses = words.chain(program_code_use(program).built.into_keys());
    let offsets = addresses.filter_map(|word| word.checked_sub(PROGRAM_BASE).map(|offset| offset as usize));
    offsets.filter(|offset| (PROGRAM_DATA..program.len()).contains(offset)).collect()
}

/// How long the text at `offset` of the program's file is, if what is there is text.
fn program_text_length(program: &[u8], offset: usize, chars: &Characters) -> Option<usize> {
    text_length(program, offset, chars, &PROGRAM_CODES, true, false)
}

/// The strings of the program's own text, each under its offset in the program's file, without the
/// zero byte that ends it: what a word of the program points at and reads as text.
pub fn program_strings(program: &[u8], chars: &Characters) -> BTreeMap<u32, Vec<u8>> {
    strings_among(program, &program_targets(program), |offset| program_text_length(program, offset, chars))
}

/// The script the program holds itself (see `PROGRAM_SCRIPT`): its header, its code and its text, up
/// to where it ends. Refused if the program has no script there.
fn program_script(program: &[u8]) -> Result<&[u8]> {
    let script = program.get(PROGRAM_SCRIPT..PROGRAM_SCRIPT_END).filter(|script| {
        script.starts_with(SCRIPT_MAGIC) && number(script, 8).is_some_and(|start| start as usize <= script.len())
    });
    script.ok_or_else(|| Error::Text(format!("{EXECUTABLE} does not hold a script at {PROGRAM_SCRIPT:#X}")))
}

/// The script the program holds itself, with some of its strings replaced, and how many of those are
/// at another offset now. `replacements` gives the new bytes of each string by its number.
///
/// The script cannot grow, so its strings share the room of its text: they are written back one
/// after another in their order, each with its zero byte, and each statement of the code that names
/// one is given its new offset. A string that something names and that has the same bytes as one
/// written before it is not written again: what names it is given the first one's offset. The game
/// has the same message for six kinds of empty chest, which leaves the room of five of them to the
/// rest. Zero bytes fill what is left up to the end. Refused, with what to do, if the strings do not
/// fit.
pub fn rebuild_program_script(script: &[u8], replacements: &BTreeMap<u32, Vec<u8>>) -> Result<(Vec<u8>, usize)> {
    let broken =
        || Error::Text(format!("the script at {PROGRAM_SCRIPT:#X} of {EXECUTABLE} is not laid out as a script is"));
    let code_start = number(script, 4).ok_or_else(broken)? as usize;
    let text_start = number(script, 8).ok_or_else(broken)? as usize;
    if code_start > text_start || text_start > script.len() {
        return Err(broken());
    }
    let references = string_references(&script[code_start..text_start])?;
    let mut text = Vec::new();
    let mut offsets: HashMap<usize, usize> = HashMap::new(); // a string's offset in the game's text, and its new one
    let mut written: HashMap<&[u8], usize> = HashMap::new(); // the bytes of each string written, and where
    let (mut old_offset, mut moved) = (0, 0);
    for (number, old) in split_strings(&script[text_start..]).into_iter().enumerate() {
        let new = replacements.get(&(number as u32));
        let data = new.map_or(old, Vec::as_slice);
        let offset = match written.get(data) {
            Some(offset) if references.contains_key(&old_offset) => *offset,
            _ => {
                written.entry(data).or_insert(text.len());
                text.extend_from_slice(data);
                text.push(END);
                text.len() - data.len() - 1
            }
        };
        moved += usize::from(new.is_some() && offset != old_offset);
        offsets.insert(old_offset, offset);
        old_offset += old.len() + 1;
    }
    let room = script.len() - text_start;
    if text.len() > room {
        return text_error(format!(
            "the strings of {PROGRAM_SCRIPT_SECTION} (the messages of treasure chests, the caption after a lost \
             battle and the message of a title earned, which the program holds itself) share the {room} bytes of \
             its text, since what follows it in the program is in use, and with the pack's they take {}: shorten \
             some of them (a wide character takes two bytes and a narrow one takes one, and strings that are the \
             same are kept once)",
            text.len()
        ));
    }
    let mut rebuilt = script[..text_start].to_vec();
    for (old, places) in &references {
        let offset = offsets.get(old).and_then(|offset| u16::try_from(*offset).ok()).ok_or_else(broken)?;
        for place in places {
            rebuilt[code_start + place..code_start + place + 2].copy_from_slice(&offset.to_le_bytes());
        }
    }
    rebuilt.extend(text);
    rebuilt.resize(script.len(), 0);
    Ok((rebuilt, moved))
}

/// How many bytes the string at `offset` of a module may take, the game's own taking `length`:
/// those, and the unused ones after it up to the next multiple of 8, for as far as they are zero
/// and nothing points at them. `targets` is what `pointed_at` gives for the module.
fn menu_room(module: &[u8], offset: usize, length: usize, targets: &BTreeSet<usize>) -> usize {
    let end = offset + length; // where the zero byte that ends the string is
    let boundary = (end + 1).next_multiple_of(MENU_ALIGNMENT).min(module.len());
    let unused = (end + 1..boundary).take_while(|at| module[*at] == 0 && !targets.contains(at)).count();
    length + unused
}

/// What the program's code does with the addresses of its data.
fn program_code_use(program: &[u8]) -> CodeUse {
    code_use(program, PROGRAM_CODE, PROGRAM_BASE)
}

/// What is changed when a string of the program is put elsewhere in its file.
#[derive(Debug, Default, Clone, PartialEq)]
struct Reached {
    /// The offsets of the 32-bit words that point at it.
    words: Vec<usize>,
    /// The offsets of the instructions that add the low half of its address.
    code: Vec<usize>,
}

/// How the program reaches the strings of its own text, for a build that changes some of them.
#[derive(Debug, Default)]
struct ProgramReach {
    /// The strings that can be put elsewhere in the file, each under its offset.
    movable: BTreeMap<u32, Reached>,
    /// The strings the code reads straight from memory. Such a string is copied a set number of
    /// bytes at a time, so it stays where it is and has the bytes of the game's own and no more.
    read_straight: BTreeSet<u32>,
}

/// Which strings of the program can be put elsewhere in its file, and what to change when one is.
///
/// A word that holds a string's address is not always a pointer at it. A size of 32 by 32, kept as
/// two 16-bit numbers, reads as $00200020, and a string is at that address; some 180 words of the
/// program are such numbers. Changing one would spoil a size or a colour, and missing a real
/// pointer would leave it at bytes that now hold other text. So a word is taken for a pointer only
/// when it is in the program's tables and stands in a column of them: at one step there are two
/// more words, one on each side or both on one side, that hold addresses of strings, the three
/// strings all different and near one another in the file.
///
/// A string can move when every word that holds its address is such a pointer, and when the code
/// reaches it only by building the address of its first byte, which is then changed where it is
/// built. One the code reads straight from memory, or builds an address inside of, stays where it
/// is, and so does one that nothing found refers to.
///
/// Nothing outside the program was found to point at a string of it. Other modules hold some forty
/// words that equal the address of one, all of them such numbers.
fn program_reach(program: &[u8], strings: &Numbered) -> ProgramReach {
    let offsets: Vec<u32> = strings.keys().copied().collect();
    // Each string's place in the order of the file, by its address.
    let ranks: HashMap<u32, usize> =
        offsets.iter().enumerate().map(|(rank, offset)| (offset + PROGRAM_BASE, rank)).collect();
    // Every word that holds a string's address, by its offset, with that string's place in the order.
    let words = (0..program.len() / 4).map(|word| 4 * word);
    let held: BTreeMap<usize, usize> = words.filter_map(|at| Some((at, *ranks.get(&number(program, at)?)?))).collect();
    // Whether the words at `at` and at two more places hold three different strings that are near.
    let three = |at: usize, second: Option<usize>, third: Option<usize>| {
        let (Some(second), Some(third)) = (second.and_then(|at| held.get(&at)), third.and_then(|at| held.get(&at)))
        else {
            return false;
        };
        let (first, least, most) = (held[&at], held[&at].min(*second).min(*third), held[&at].max(*second).max(*third));
        first != *second && first != *third && second != third && most - least <= COLUMN_NEAR
    };
    let in_column = |at: usize| {
        let steps = (4..=COLUMN_STEP).step_by(4);
        let mut columns = steps.map(|step| (at.checked_sub(step), Some(at + step), at.checked_sub(2 * step), step));
        PROGRAM_TABLES.contains(&at)
            && columns.any(|(before, after, two_before, step)| {
                three(at, before, after) || three(at, after, Some(at + 2 * step)) || three(at, before, two_before)
            })
    };
    let mut pointers: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut doubtful: HashSet<u32> = HashSet::new();
    for (at, rank) in &held {
        if in_column(*at) {
            pointers.entry(offsets[*rank]).or_default().push(*at);
        } else {
            doubtful.insert(offsets[*rank]);
        }
    }
    let code = program_code_use(program);
    let mut reach = ProgramReach::default();
    for (offset, data) in strings {
        let address = offset + PROGRAM_BASE;
        let end = address + data.len() as u32; // the zero byte that ends it
        if code.straight.range(address..=end).next().is_some() {
            reach.read_straight.insert(*offset);
            continue;
        }
        let reached = Reached {
            words: pointers.remove(offset).unwrap_or_default(),
            code: code.built.get(&address).cloned().unwrap_or_default(),
        };
        let inside = code.built.range(address + 1..=end).next().is_some();
        if !doubtful.contains(offset) && !inside && (!reached.words.is_empty() || !reached.code.is_empty()) {
            reach.movable.insert(*offset, reached);
        }
    }
    reach
}

/// Which strings of a menu's module can be put elsewhere in it, each under its offset with what to
/// change when it is. `strings` are the module's, and `about` what is known of it.
///
/// Every 32-bit word of the module that holds a string's address is taken for a pointer at it: of
/// the 567 strings a fan translation moved, it changed every such word. A string can move when it
/// is reached by such words in the module's data and by instructions that build the address of its
/// first byte, and by nothing else that was found. One the code reads straight from memory or
/// builds an address inside of stays where it is; so does one with its address in a word among
/// the code, and one that nothing found refers to exactly.
fn module_reach(module: &[u8], about: &Module, strings: &Numbered) -> BTreeMap<u32, Reached> {
    let data = code_end(module);
    // The instruction after the last return still runs, and belongs to the code.
    let code = code_use(module, 0..(data + 4).min(module.len()), about.base);
    let mut words: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for at in (0..module.len() / 4).map(|word| 4 * word) {
        let offset = number(module, at).and_then(|word| word.checked_sub(about.base));
        if let Some(offset) = offset.filter(|offset| strings.contains_key(offset)) {
            words.entry(offset).or_default().push(at);
        }
    }
    let mut movable = BTreeMap::new();
    for (offset, text) in strings {
        let address = offset + about.base;
        let end = address + text.len() as u32; // the zero byte that ends it
        let reached = Reached {
            words: words.remove(offset).unwrap_or_default(),
            code: code.built.get(&address).cloned().unwrap_or_default(),
        };
        let followed = code.straight.range(address..=end).next().is_none()
            && code.built.range(address + 1..=end).next().is_none()
            && reached.words.iter().all(|at| *at >= data);
        if followed && (!reached.words.is_empty() || !reached.code.is_empty()) {
            movable.insert(*offset, reached);
        }
    }
    movable
}

/// A changed string that waits for its place in the room it shares: its name, its offset, the
/// bytes that were its own with the zero byte that ended it, and its new bytes.
type Waiting = (String, usize, usize, Vec<u8>);

/// The name a script has in a pack: its member number in the archive.
pub fn section_name(index: usize) -> String {
    format!("{index:05}")
}

/// The member of the archive a section is named after: five digits. A number past the archive's
/// last member names no section, which a pack is told without the game. Which members below it hold
/// text is only known from the disc, so a build is what refuses one that holds none.
fn member_of(section: &str) -> Option<usize> {
    let digits = section.len() == 5 && section.bytes().all(|byte| byte.is_ascii_digit());
    digits.then(|| section.parse().ok()).flatten().filter(|member| *member < MEMBER_COUNT)
}

// --- the encoding

fn is_lead(byte: u8) -> bool {
    LEAD_RANGES.iter().any(|(first, stop)| (*first..*stop).contains(&byte))
}

fn is_trail(byte: u8) -> bool {
    (FIRST_TRAIL..=LAST_TRAIL).contains(&byte) && !SKIPPED_TRAILS.contains(&byte)
}

/// Whether the program's text `data` has a code `code` that starts at byte `at`, read from its start
/// as the program's text is: a code with its 32-bit number, a two-byte character, or a single byte.
fn has_code_at(data: &[u8], at: usize, code: u8) -> bool {
    let mut position = 0;
    while position < at {
        let Some(&byte) = data.get(position) else { return false };
        position += match data.get(position + 1) {
            _ if PROGRAM_CODES.contains(&byte) => 1 + NUMBER_SIZE,
            Some(trail) if is_lead(byte) && is_trail(*trail) => 2,
            _ => 1,
        };
    }
    position == at && data.get(at) == Some(&code) && data.len() >= at + 1 + NUMBER_SIZE
}

/// The glyph number of a two-byte code, or None for two bytes that are not one.
pub fn glyph_of(code: u16) -> Option<usize> {
    let [lead, trail] = code.to_be_bytes();
    if !is_trail(trail) {
        return None;
    }
    let position = usize::from(trail - FIRST_TRAIL) - SKIPPED_TRAILS.iter().filter(|skipped| trail > **skipped).count();
    let mut row = 0;
    for (first, stop) in LEAD_RANGES {
        if (first..stop).contains(&lead) {
            return Some((row + usize::from(lead - first)) * ROW_SIZE + position);
        }
        row += usize::from(stop - first);
    }
    None
}

/// The two-byte code of a glyph, as one number. The lead bytes allow 4,301 glyphs; a number past
/// them gives a code that belongs to another glyph.
pub fn code_at(index: usize) -> u32 {
    let (mut row, position) = (index / ROW_SIZE, index % ROW_SIZE);
    let mut lead = 0;
    for (first, stop) in LEAD_RANGES {
        lead = usize::from(first);
        let leads = usize::from(stop - first);
        if row < leads {
            break;
        }
        row -= leads;
    }
    let mut trail = usize::from(FIRST_TRAIL) + position;
    for skipped in SKIPPED_TRAILS {
        if trail >= usize::from(skipped) {
            trail += 1;
        }
    }
    ((lead + row) << 8 | trail) as u32
}

/// The half-width kana a single byte from $A1 to $DF stands for.
fn kana(byte: u8) -> char {
    char::from_u32(HALF_WIDTH + u32::from(byte - FIRST_KANA)).unwrap_or(char::REPLACEMENT_CHARACTER)
}

/// The byte that stands for a character on its own, if there is one.
fn single_byte(char: char) -> Option<u8> {
    let number = char as u32;
    if (0x20..=0x7E).contains(&number) && char != '{' && char != '}' {
        return Some(number as u8);
    }
    if (HALF_WIDTH..=HALF_WIDTH + u32::from(LAST_KANA - FIRST_KANA)).contains(&number) {
        return Some(FIRST_KANA + (number - HALF_WIDTH) as u8);
    }
    None
}

/// Which character each glyph is, and the glyph to use for each character.
///
/// The game's list has blanks and a few characters twice. A character is written with the first
/// glyph that has it; a code for any other glyph is kept in a script as `{G:code}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Characters {
    /// The character each glyph is; None where the list's two bytes are not one character.
    pub by_glyph: Vec<Option<char>>,
    pub glyphs: HashMap<char, usize>,
    /// Characters a build has given a single byte to.
    pub singles: HashMap<char, u8>,
}

impl Characters {
    /// Read the game's own list: two bytes of Shift-JIS per glyph.
    pub fn from_list(data: &[u8]) -> Characters {
        let (codes, _) = data.as_chunks::<2>();
        let by_glyph: Vec<Option<char>> =
            codes.iter().map(|[lead, trail]| shift_jis::character(*lead, *trail)).collect();
        let mut glyphs = HashMap::new();
        for (glyph, char) in by_glyph.iter().enumerate() {
            if let Some(char) = *char
                && !matches!(char, '{' | '}' | '\n' | char::REPLACEMENT_CHARACTER)
                && single_byte(char).is_none()
            {
                glyphs.entry(char).or_insert(glyph);
            }
        }
        Characters { by_glyph, glyphs, singles: HashMap::new() }
    }
}

/// The game's list of characters, read once for an image.
pub fn characters(disc: &Disc) -> Result<Arc<Characters>> {
    disc.keep("tod2-ps2: the characters of the font", || {
        Ok(Characters::from_list(&read_member(disc, CHARACTER_MEMBER)?))
    })
}

/// Where the expression starting at `position` ends, after its closing byte; None if it does not.
fn expression_end(data: &[u8], mut position: usize) -> Option<usize> {
    while position < data.len() {
        let byte = data[position];
        if byte == EXPRESSION_END {
            return Some(position + 1);
        }
        if byte >= 0xE0 {
            return None;
        }
        position += value_length(byte);
    }
    None
}

/// How long the token starting with `byte`, below $E0, is: the parts an expression is made of.
fn value_length(byte: u8) -> usize {
    match byte {
        // a variable: one more byte, or two
        ..0x80 => {
            if byte & 0x08 != 0 {
                3
            } else {
                2
            }
        }
        // a number: $8x alone, $9x one more byte, $Ax two
        0x80..0xB0 => 1 + usize::from(byte >> 4) - 8,
        // a 32-bit number
        0xB0..0xC0 => 5,
        // an operator, or the end of an expression
        _ => 1,
    }
}

/// The instruction of a script's code that refers to a string.
const STRING_REFERENCE: u8 = 0xF8;

/// How long each instruction of a script's code is, for first bytes from $E0 up. The ones with a
/// 16-bit number after them are jumps to a place in the code, except $F8, which refers to a string.
fn instruction_length(byte: u8) -> Option<usize> {
    match byte {
        0xE0..=0xEF | 0xF6 => Some(2),
        0xF0 | 0xF1 | 0xFE => Some(1),
        0xF2..=0xF5 | STRING_REFERENCE => Some(3),
        _ => None,
    }
}

/// Where a script's code refers to each string: text offset -> the places in the code that hold it.
///
/// The code is read instruction by instruction, because the byte $F8 also turns up inside other
/// instructions, where what follows it is not a string's offset.
pub fn string_references(code: &[u8]) -> Result<BTreeMap<usize, Vec<usize>>> {
    let mut found: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for token in code_tokens(code)? {
        if let [STRING_REFERENCE, low, high] = token {
            found.entry(usize::from(*low) | usize::from(*high) << 8).or_default().push(token_start(code, token) + 1);
        }
    }
    Ok(found)
}

/// The instructions of a script's code, in order, each as its bytes: the parts of an expression
/// (see `value_length`) and the instructions from $E0 up (see `instruction_length`).
fn code_tokens(code: &[u8]) -> Result<Vec<&[u8]>> {
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < code.len() {
        let byte = code[position];
        let length = if byte < 0xE0 { Some(value_length(byte)) } else { instruction_length(byte) };
        let Some(length) = length else {
            return text_error(format!("the script's code has an instruction that is not known, ${byte:02X}"));
        };
        let Some(token) = code.get(position..position + length) else {
            return text_error("the script's code does not end where its text starts");
        };
        tokens.push(token);
        position += length;
    }
    Ok(tokens)
}

/// Where a token that `code_tokens` gave starts in the code it was read from.
fn token_start(code: &[u8], token: &[u8]) -> usize {
    token.as_ptr() as usize - code.as_ptr() as usize
}

/// The call that shows a message: its statement pushes six numbers, the string of a label the game
/// does not show ("select", "notice", a speaker's name), the string to show, then calls $E0 $00.
const MESSAGE_CALL: [u8; 2] = [0xE0, 0x00];
/// The call that waits for the player to choose among the last lines of the message just shown:
/// its statement pushes where to keep the answer, the window, how many choices there are and which
/// one cancelling picks, then calls $E0 $3F.
const CHOICE_CALL: [u8; 2] = [0xE0, 0x3F];

/// The instruction that jumps, always, to the place in the code its 16-bit number gives.
const JUMP: u8 = 0xF2;

/// The strings of a script that the game shows as a list to choose from, by their offset in the
/// text: each is the string of a message whose statement the next one to run is a choice.
///
/// A statement ends at $C0, the end of an expression, or at an instruction from $F0 up other than
/// $F8, which jump or end a block. The next statement to run is the next in the code, or where a
/// jump right after the message goes: a script that shows one list or another, by a page it is on,
/// jumps past the second to the choice they share. The game's own lists put each choice on a line
/// of its own, after the question on lines of its own if there is one, and many but not all start
/// with the code $03 and mark each choice with a wide space either side; the code is what tells
/// them all apart.
fn choice_lists(code: &[u8]) -> Result<BTreeSet<usize>> {
    let tokens = code_tokens(code)?;
    let ends = |token: &[u8]| token[0] == EXPRESSION_END || (token[0] >= 0xF0 && token[0] != STRING_REFERENCE);
    let at: HashMap<usize, usize> =
        tokens.iter().enumerate().map(|(index, token)| (token_start(code, token), index)).collect();
    // Whether the statement that runs after the token at `index` is a choice.
    let choice_after = |mut index: usize| {
        while let Some(token) = tokens.get(index) {
            match token {
                [EXPRESSION_END] => index += 1,
                [JUMP, low, high] => match at.get(&(usize::from(*low) | usize::from(*high) << 8)) {
                    Some(target) if *target > index => index = *target,
                    _ => return false,
                },
                _ => break,
            }
        }
        tokens[index.min(tokens.len())..].iter().take_while(|token| !ends(token)).any(|token| *token == CHOICE_CALL)
    };
    let mut found = BTreeSet::new();
    for (index, window) in tokens.windows(3).enumerate() {
        if let [[STRING_REFERENCE, ..], [STRING_REFERENCE, low, high], call] = window
            && *call == MESSAGE_CALL
            && choice_after(index + 3)
        {
            found.insert(usize::from(*low) | usize::from(*high) << 8);
        }
    }
    Ok(found)
}

/// The numbers of the strings of a story script (its header, code and text) that are lists to
/// choose from (see `choice_lists`).
fn choice_strings(script: &[u8]) -> Result<BTreeSet<u32>> {
    let broken = || Error::Text("a story script is not laid out as a script is".into());
    let code_start = number(script, 4).ok_or_else(broken)? as usize;
    let text_start = number(script, 8).ok_or_else(broken)? as usize;
    let code = script.get(code_start..text_start).ok_or_else(broken)?;
    let lists = choice_lists(code)?;
    let mut found = BTreeSet::new();
    let mut offset = 0;
    for (number, string) in split_strings(&script[text_start..]).iter().enumerate() {
        if lists.contains(&offset) {
            found.insert(number as u32);
        }
        offset += string.len() + 1;
    }
    Ok(found)
}

/// Where the token starting at `position` ends. The end of a string, a zero byte, is not a token.
fn token_end(data: &[u8], position: usize) -> usize {
    let byte = data[position];
    if is_lead(byte) && data.get(position + 1).is_some_and(|trail| is_trail(*trail)) {
        return position + 2;
    }
    if CODES.count(u32::from(byte)) != 0 && position + 1 + NUMBER_SIZE <= data.len() {
        return position + 1 + NUMBER_SIZE;
    }
    if EXPRESSIONS.contains(&byte)
        && let Some(end) = expression_end(data, position + 1)
    {
        return end;
    }
    position + 1
}

/// The strings of a script's text, in order, each without the zero byte that ends it.
pub fn split_strings(text: &[u8]) -> Vec<&[u8]> {
    let mut strings = Vec::new();
    let (mut start, mut position) = (0, 0);
    while position < text.len() {
        if text[position] == END {
            strings.push(&text[start..position]);
            position += 1;
            start = position;
        } else {
            position = token_end(text, position);
        }
    }
    if start < text.len() {
        strings.push(&text[start..]);
    }
    strings
}

/// One string as it is written in a script file.
pub fn decode(data: &[u8], chars: &Characters) -> String {
    let mut out = String::new();
    let mut position = 0;
    while position < data.len() {
        let end = token_end(data, position);
        let (byte, rest) = (data[position], &data[position + 1..end]);
        position = end;
        if is_lead(byte)
            && let [trail] = rest
        {
            let code = u16::from_be_bytes([byte, *trail]);
            let glyph = glyph_of(code);
            let char = glyph.and_then(|glyph| chars.by_glyph.get(glyph).copied().flatten());
            match char {
                Some(char) if chars.glyphs.get(&char).copied() == glyph => out.push(char),
                _ => out.push_str(&format!("{{G:{code:04X}}}")),
            }
        } else if CODES.count(u32::from(byte)) != 0 && !rest.is_empty() {
            out.push_str(&CODES.tag(u32::from(byte), rest));
        } else if EXPRESSIONS.contains(&byte)
            && let [inside @ .., _] = rest
        {
            out.push_str(&format!("{{{byte:02X}:{}}}", hex(inside)));
        } else if byte == LINE {
            out.push('\n');
        } else if (0x20..=0x7E).contains(&byte) && byte != b'{' && byte != b'}' {
            out.push(char::from(byte));
        } else if (FIRST_KANA..=LAST_KANA).contains(&byte) {
            out.push(kana(byte));
        } else {
            out.push_str(&format!("{{{byte:02X}}}"));
        }
    }
    out
}

/// The bytes a tag stands for: `raw` is the tag as written, `value` what follows its colon.
fn encode_tag(raw: &str, name: &str, value: Option<&str>) -> Result<Vec<u8>> {
    let unknown = || text_error(format!("unknown tag {raw}"));
    let Some(parameters) = from_hex(value.unwrap_or("")) else { return unknown() };
    if name == "G" {
        return match parameters[..] {
            [lead, trail] if is_lead(lead) && is_trail(trail) => Ok(parameters),
            _ => unknown(),
        };
    }
    // A code is written as its name, or as its number in two hexadecimal digits. Nothing else is
    // taken for a number: the Python reference also reads a sign and the digits of other scripts.
    let two_digits = || {
        let digits = name.len() == 2 && name.bytes().all(|byte| byte.is_ascii_hexdigit());
        if digits { u32::from_str_radix(name, 16).ok() } else { None }
    };
    let byte = match CODES.code(name).or_else(two_digits).map(u8::try_from) {
        Some(Ok(byte)) if byte != 0 => byte,
        _ => return unknown(),
    };
    if value.is_none() {
        return Ok(vec![byte]); // a byte on its own
    }
    if EXPRESSIONS.contains(&byte) {
        let data = [&[byte], parameters.as_slice(), &[EXPRESSION_END]].concat();
        if expression_end(&data, 1) != Some(data.len()) {
            return text_error(format!("{raw} is not a whole expression"));
        }
        return Ok(data);
    }
    let count = CODES.count(u32::from(byte));
    if parameters.len() != count || parameters.is_empty() {
        return text_error(format!("{raw} needs {count} parameter bytes"));
    }
    Ok([&[byte], parameters.as_slice()].concat())
}

/// A string of a script file as the game stores it, without the zero byte that ends it.
pub fn encode(text: &str, chars: &Characters) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for token in tokens(text) {
        match token {
            Token::Tag { raw, name, value } => out.extend(encode_tag(raw, name, value)?),
            Token::Char('\n') => out.push(LINE),
            Token::Char(char) => {
                if let Some(byte) = chars.singles.get(&char) {
                    out.push(*byte);
                } else if let Some(glyph) = chars.glyphs.get(&char) {
                    out.extend((code_at(*glyph) as u16).to_be_bytes());
                } else if let Some(byte) = single_byte(char) {
                    out.push(byte);
                } else {
                    return text_error(format!("the game's font has no {}", quoted(char.encode_utf8(&mut [0; 4]))));
                }
            }
        }
    }
    Ok(out)
}

// --- the whole script

/// Every string of every story script and every skit, and of the script the program holds itself,
/// by section, as the game stores them.
fn strings(disc: &Disc) -> Result<BTreeMap<String, Vec<Vec<u8>>>> {
    let split = |text: &[u8]| split_strings(text).into_iter().map(<[u8]>::to_vec).collect();
    let mut found = BTreeMap::new();
    for index in script_members(disc)? {
        found.insert(section_name(index), split(&read_script_text(disc, index)?));
    }
    for (index, script) in skit_scripts(disc)?.iter() {
        found.insert(section_name(*index), split(text_of(script)));
    }
    for (index, script) in world_map_scripts(disc)?.iter() {
        found.insert(section_name(*index), split(text_of(script)));
    }
    found.insert(PROGRAM_SCRIPT_SECTION.to_string(), split(text_of(program_script(&program_file(disc)?)?)));
    Ok(found)
}

/// The strings of the module that is member `index` of the archive.
fn strings_of_module(index: usize, module: &[u8], chars: &Characters) -> BTreeMap<u32, Vec<u8>> {
    module_of(index).map(|about| menu_strings(module, &about, chars)).unwrap_or_default()
}

/// Every string that is numbered by where it is, by section, as the game stores them: the menus,
/// each in its module, and the program's own text, by their offsets, and the names of enemies, by
/// their entries in their packs.
fn placed_text(disc: &Disc) -> Result<BTreeMap<String, BTreeMap<u32, Vec<u8>>>> {
    let chars = characters(disc)?;
    let modules = menu_modules(disc)?;
    let mut found: BTreeMap<String, BTreeMap<u32, Vec<u8>>> = modules
        .iter()
        .map(|(index, module)| (section_name(*index), strings_of_module(*index, module, &chars)))
        .collect();
    found.insert(PROGRAM_SECTION.to_string(), program_strings(&program_file(disc)?, &chars));
    for (index, names) in enemy_names(disc)?.iter() {
        found.insert(section_name(*index), names.clone());
    }
    for (index, script) in game_scripts(disc)?.iter() {
        let strings = game_strings(script, &chars);
        if !strings.is_empty() {
            found.insert(section_name(*index), strings);
        }
    }
    Ok(found)
}

/// The story scripts, the skits, the menus and the program's own text, by section. No names yet.
pub fn extract(disc: &Disc) -> Result<Script> {
    let chars = characters(disc)?;
    let numbered = |strings: &Vec<Vec<u8>>| {
        strings.iter().enumerate().map(|(number, data)| (number as u32, decode(data, &chars))).collect()
    };
    let mut texts: Script =
        strings(disc)?.iter().map(|(section, strings)| (section.clone(), numbered(strings))).collect();
    for (section, strings) in placed_text(disc)? {
        texts.insert(section, strings.iter().map(|(offset, data)| (*offset, decode(data, &chars))).collect());
    }
    Ok(texts)
}

/// Check that an extracted script encodes back to exactly the bytes the game has.
pub fn verify_script(disc: &Disc, texts: &Script) -> Result<()> {
    let chars = characters(disc)?;
    let (menus, scripts): (Script, Script) = texts
        .iter()
        .map(|(section, items)| (section.clone(), items.clone()))
        .partition(|(section, _)| is_placed(section));
    script::verify(&scripts, &strings(disc)?, |text| encode(text, &chars))?;
    script::verify_by_place(&menus, &placed_text(disc)?, |text| encode(text, &chars))
}

// --- putting a script back

/// What a build changed. The image itself is written to a file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildResult {
    pub strings: usize,
    /// Strings that outgrew their place and went elsewhere in their script's text.
    pub relocated: usize,
    pub scripts: usize,
    /// The same three for the skits.
    pub skit_strings: usize,
    pub skit_relocated: usize,
    pub skits: usize,
    /// With the option `skit-pace`: the changed lines of skits that were timed, and how many of them
    /// are typed faster than the game types them.
    pub skit_timed: usize,
    pub skit_faster: usize,
    /// Strings of menu text that changed, in how many menus, and how many of them went to another
    /// place in their menu's module.
    pub menu_strings: usize,
    pub menus: usize,
    pub menu_moved: usize,
    /// With the option `narrow-spell-names`: the grids of artes changed to draw spells narrow.
    pub narrow_spells: usize,
    /// Strings of the program's own text that changed, and how many of them went to another place.
    pub list_strings: usize,
    pub list_moved: usize,
    /// Enemies renamed, in how many packs, and how many names of the battle's name plates changed.
    pub enemy_names: usize,
    pub enemy_packs: usize,
    pub enemy_plates: usize,
    /// Strings of the games within the game that changed, and in how many of their scripts.
    pub game_strings: usize,
    pub games: usize,
    /// Members of the archive that now sit at another offset.
    pub moved: usize,
    /// Bytes the archive gained.
    pub growth: i64,
    /// New characters drawn at half width, through the table for single bytes.
    pub narrow: Vec<char>,
    /// New characters drawn at full width, as two-byte codes.
    pub wide: Vec<char>,
}

impl BuildResult {
    pub fn summary(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "dialogue:  {} strings changed in {} scripts, {} of them moved",
            self.strings, self.scripts, self.relocated
        )];
        if self.skit_strings != 0 {
            lines.push(format!(
                "skits:     {} strings changed in {} skits, {} of them moved",
                self.skit_strings, self.skits, self.skit_relocated
            ));
        }
        if self.skit_timed != 0 {
            lines.push(format!(
                "skit pace: {} changed lines timed, {} of them typed faster than the game's pace",
                self.skit_timed, self.skit_faster
            ));
        }
        if self.menu_strings != 0 {
            lines.push(format!(
                "menus:     {} strings changed in {} menus, {} of them moved",
                self.menu_strings, self.menus, self.menu_moved
            ));
        }
        if self.list_strings != 0 {
            lines.push(format!("lists:     {} strings changed, {} of them moved", self.list_strings, self.list_moved));
        }
        if self.narrow_spells != 0 {
            lines.push(format!("spells:    drawn in the narrow font in {} grids of artes", self.narrow_spells));
        }
        if self.enemy_names != 0 {
            lines.push(format!(
                "enemies:   {} names changed in {} packs, and {} on the battle's name plates",
                self.enemy_names, self.enemy_packs, self.enemy_plates
            ));
        }
        if self.game_strings != 0 {
            lines.push(format!("minigames: {} strings changed in {} scripts", self.game_strings, self.games));
        }
        if !self.narrow.is_empty() || !self.wide.is_empty() {
            lines.push(format!(
                "font:      {} new characters, {} at half width and {} at full width",
                self.narrow.len() + self.wide.len(),
                self.narrow.len(),
                self.wide.len()
            ));
        }
        if self.moved != 0 {
            lines.push(format!("archive:   {:+} bytes, {} members moved", self.growth, self.moved));
        }
        lines
    }
}

/// The glyphs of the main font that are empty, apart from the blank the game uses as a space.
pub fn free_glyphs(font: &[u8]) -> Vec<usize> {
    let empty = |index: &usize| {
        font.get(index * GLYPH_BYTES..(index + 1) * GLYPH_BYTES)
            .is_some_and(|glyph| glyph.iter().all(|byte| *byte == 0))
    };
    (1..GLYPH_COUNT).filter(empty).collect()
}

/// Whether the game draws an ASCII character as something else. `table` is the game's table for
/// $20-$7F, which gives each byte a cell of the half-width font, and a cell of that font is the
/// main font's glyph of the same number at half the width, so `by_glyph` says what it draws. On the
/// game's own disc that is a capital for a lowercase letter, a closing double quote for `"`, `'`
/// and `` ` ``, a minus sign for `_` and `~`, a colon for `;`, round brackets for `<` and `>`,
/// corner brackets for `[` and `]`, a star for `*`, and nothing at all for `# $ @ \ ^ |`. A
/// character the game draws as its own full-width form at half the width, such as `!` or `A`, is
/// drawn right, and so is the hyphen-minus, which it draws as a minus sign.
pub fn needs_narrow_glyph(char: char, table: &[u8], by_glyph: &[Option<char>]) -> bool {
    if char == ' ' || !(' '..='~').contains(&char) || single_byte(char).is_none() {
        return false;
    }
    let drawn = table.get(char as usize - 0x20).and_then(|cell| by_glyph.get(usize::from(*cell))).copied().flatten();
    drawn.map(ascii_form) != Some(char)
}

/// The ASCII character a character of the game's list stands for when it is drawn at half the
/// width: its own full-width form, the wide space, or the minus sign for the hyphen-minus.
fn ascii_form(char: char) -> char {
    match char {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(char as u32 - 0xFEE0).unwrap_or(char),
        '\u{3000}' => ' ',
        '\u{2212}' => '-',
        _ => char,
    }
}

/// Ink as the game's fonts hold it: the letter at the brightest level, surrounded as the game's
/// own letters are. Directly under the ink is a half-bright row, which is what gives a stroke its
/// weight; beside it, above it and at its corners are the dark levels; and one pixel further out,
/// all round, is the darkest.
fn outlined(ink: &[Vec<bool>]) -> Vec<Vec<u8>> {
    let (height, width) = (ink.len() as i64, ink.first().map_or(0, Vec::len) as i64);
    let inked = |x: i64, y: i64| (0..height).contains(&y) && (0..width).contains(&x) && ink[y as usize][x as usize];
    let level = |x: i64, y: i64| {
        if inked(x, y) {
            INK_LEVEL
        } else if inked(x, y - 1) {
            UNDER_LEVEL
        } else if inked(x - 1, y) || inked(x + 1, y) {
            BESIDE_LEVEL
        } else if inked(x, y + 1) {
            ABOVE_LEVEL
        } else if [(-1, -1), (-1, 1), (1, -1), (1, 1)].iter().any(|(dx, dy)| inked(x + dx, y + dy)) {
            CORNER_LEVEL
        } else if (-SURROUND..=SURROUND).any(|dx| (-SURROUND..=SURROUND).any(|dy| inked(x + dx, y + dy))) {
            FAR_LEVEL
        } else {
            0
        }
    };
    (0..height).map(|y| (0..width).map(|x| level(x, y)).collect()).collect()
}

/// A glyph in a cell, pixel for pixel: centred across it, with its baseline `baseline` rows down,
/// and its ink made `STROKE_DOWN` rows heavier.
fn placed(glyph: &Bitmap, width: usize, height: usize, baseline: i64) -> Vec<Vec<u8>> {
    let left = (width as i64 - glyph.width as i64).div_euclid(2);
    let top = baseline - glyph.ascent;
    let ink = |x: i64, y: i64| (0..=STROKE_DOWN).any(|down| glyph.pixel(x - left, y - top - down));
    let rows: Vec<Vec<bool>> = (0..height as i64).map(|y| (0..width as i64).map(|x| ink(x, y)).collect()).collect();
    outlined(&rows)
}

/// A length along one axis of a glyph that is being stretched: how much of a pixel of the cell one
/// pixel of the glyph covers. `whole` says the reference holds it as the whole number 1 and not as
/// a fraction, which matters to how it adds such numbers up.
#[derive(Clone, Copy)]
struct Cover {
    amount: f64,
    whole: bool,
}

/// For each pixel along one axis of a cell, the pixels of the glyph under it and how much of it
/// each covers. `size` is the cell's, `count` the glyph's, and `scale` how many times larger the
/// cell is.
fn shares(size: usize, scale: f64, count: usize) -> Vec<Vec<(usize, Cover)>> {
    let under = |pixel: usize| {
        let (near, far) = (pixel as f64, (pixel + 1) as f64);
        (0..count)
            .filter_map(|source| {
                let (start, end) = (source as f64 * scale, (source + 1) as f64 * scale);
                // The glyph's pixel reaches past both edges of the cell's: it covers all of it.
                let whole = far < end && near > start;
                let amount = if far < end { far } else { end } - if near > start { near } else { start };
                (amount > 0.0).then_some((source, Cover { amount, whole }))
            })
            .collect()
    };
    (0..size).map(under).collect()
}

/// The sum of some numbers, to the last bit as the Python reference's `sum` gives it, so that a
/// share of a pixel that lands on a threshold lands on the same side of it in both.
///
/// Python adds whole numbers exactly for as long as that is all it has met. From the first fraction
/// on, it keeps beside the running total the part each addition of a fraction rounded away
/// (Neumaier's method) and adds those parts back at the end; a whole number met by then is added
/// to the total with no such care.
fn python_sum(numbers: impl Iterator<Item = Cover>) -> f64 {
    let (mut total, mut lost) = (0.0_f64, 0.0_f64);
    let mut fractions = false;
    for Cover { amount, whole } in numbers {
        if whole || !fractions {
            total += amount;
            fractions |= !whole;
        } else {
            let sum = total + amount;
            lost += if total.abs() >= amount.abs() { (total - sum) + amount } else { (amount - sum) + total };
            total = sum;
        }
    }
    if lost != 0.0 && lost.is_finite() { total + lost } else { total }
}

/// A glyph stretched to fill a cell. Each pixel of the cell gets the share of it the glyph covers:
/// from `SOLID_FROM` up it is the brightest level, below that a level in proportion, and under
/// `INK_FROM` nothing, or the dark line if it touches ink.
fn scaled(glyph: &Bitmap, width: usize, height: usize) -> Vec<Vec<u8>> {
    let x_scale = width as f64 / glyph.width as f64;
    let y_scale = height as f64 / glyph.rows.len() as f64;
    let columns = shares(width, x_scale, glyph.width);
    let share = |rows: &Vec<(usize, Cover)>, across: &Vec<(usize, Cover)>| {
        let inked = rows.iter().flat_map(|(row, down)| {
            let covered = across.iter().filter(move |(column, _)| glyph.pixel(*column as i64, *row as i64));
            covered.map(move |(_, side)| Cover { amount: down.amount * side.amount, whole: down.whole && side.whole })
        });
        python_sum(inked)
    };
    let cover: Vec<Vec<f64>> = shares(height, y_scale, glyph.rows.len())
        .iter()
        .map(|rows| columns.iter().map(|across| share(rows, across)).collect())
        .collect();
    let ink: Vec<Vec<bool>> = cover.iter().map(|row| row.iter().map(|share| *share >= INK_FROM).collect()).collect();
    let mut levels = outlined(&ink);
    for (y, row) in cover.iter().enumerate() {
        for (x, share) in row.iter().enumerate() {
            if (INK_FROM..SOLID_FROM).contains(share) {
                let steps = (share - INK_FROM) / (SOLID_FROM - INK_FROM) * f64::from(INK_LEVEL - 5);
                levels[y][x] = 5 + steps.round_ties_even() as u8;
            }
        }
    }
    levels
}

/// A glyph of the pack's font as the main font stores one: 22 rows of 24 pixels.
///
/// A character from a cell of 16 pixels or more, such as a kanji, is stretched to fill the cell. A
/// letter is drawn pixel for pixel in the middle of it, on the baseline of the game's own.
pub fn draw_glyph(glyph: &Bitmap) -> Vec<u8> {
    let levels = if glyph.width >= fonts::CELL {
        scaled(glyph, GLYPH_SIZE, GLYPH_ROWS)
    } else {
        placed(glyph, GLYPH_SIZE, GLYPH_ROWS, BASELINE)
    };
    let mut data = vec![0; GLYPH_BYTES];
    for (y, row) in levels.iter().enumerate() {
        for (x, level) in row.iter().enumerate() {
            data[y * (GLYPH_SIZE / 2) + x / 2] |= if x & 1 == 1 { level << 4 } else { *level };
        }
    }
    data
}

/// Whether a glyph can go in the half-width font: it is no wider than a cell, or it is a drawing a
/// whole number of times the size of a cell, to be reduced to it.
pub fn fits_half_cell(glyph: &Bitmap) -> bool {
    if glyph.width <= HALF_WIDTH_CELL {
        return true;
    }
    let (factor, extra) = (glyph.width / HALF_WIDTH_CELL, glyph.width % HALF_WIDTH_CELL);
    extra == 0 && glyph.rows.len() == factor * HALF_HEIGHT_CELL
}

/// A large drawing of a glyph brought down to a cell, the way the game's own letters were made.
///
/// Each pixel of the cell stands for a square of the drawing, and the share of that square that is
/// ink decides its level: nearly all of it, the brightest; some of it, a level in between, which
/// is what makes an edge smooth; none to speak of, the dark surround if it is near the letter.
fn reduced(glyph: &Bitmap, width: usize, height: usize) -> Vec<Vec<u8>> {
    let factor = (glyph.width / width) as i64;
    let share = |x: i64, y: i64| {
        let inked = (0..factor).flat_map(|dy| (0..factor).map(move |dx| (dx, dy)));
        inked.filter(|(dx, dy)| glyph.pixel(x * factor + dx, y * factor + dy)).count() as f64 / (factor * factor) as f64
    };
    let cover: Vec<Vec<f64>> = (0..height as i64).map(|y| (0..width as i64).map(|x| share(x, y)).collect()).collect();
    let ink: Vec<Vec<bool>> = cover.iter().map(|row| row.iter().map(|share| *share >= 0.5).collect()).collect();
    let mut levels = outlined(&ink);
    for (y, row) in cover.iter().enumerate() {
        for (x, share) in row.iter().enumerate() {
            if *share >= SOLID_FROM_REDUCED {
                levels[y][x] = INK_LEVEL;
            } else if *share >= EDGE_FROM {
                levels[y][x] = 5 + (share / SOLID_FROM_REDUCED * f64::from(INK_LEVEL - 6)).round_ties_even() as u8;
            }
        }
    }
    levels
}

/// Draw a glyph of the pack's font into a cell of the half-width font: pixel for pixel if it is
/// that small, reduced if it is a large drawing.
pub fn draw_half_glyph(texture: &mut [u8], cell: usize, glyph: &Bitmap) -> Result<()> {
    let levels = if glyph.width > HALF_WIDTH_CELL {
        reduced(glyph, HALF_WIDTH_CELL, HALF_HEIGHT_CELL)
    } else {
        placed(glyph, HALF_WIDTH_CELL, HALF_HEIGHT_CELL, HALF_BASELINE)
    };
    let (left, above) = (cell % HALF_PER_ROW * HALF_WIDTH_CELL, cell / HALF_PER_ROW * HALF_HEIGHT_CELL);
    if texture.len() < HALF_FONT_PIXELS + (above + HALF_HEIGHT_CELL) * HALF_ROW_BYTES {
        return text_error(format!("the half-width font has no cell {cell}: it is not as large as expected"));
    }
    for (y, row) in levels.iter().enumerate() {
        for (x, level) in row.iter().enumerate() {
            let at = HALF_FONT_PIXELS + (above + y) * HALF_ROW_BYTES + (left + x) / 2;
            texture[at] =
                if (left + x) & 1 == 1 { texture[at] & 0x0F | level << 4 } else { texture[at] & 0xF0 | level };
        }
    }
    Ok(())
}

/// A story script member with some of its strings replaced, and how many of them had to move.
/// `replacements` gives the new bytes of each string by its number, and `name` is the script's.
///
/// A string that fits goes where the old one was. One that does not moves, to room another moved
/// string left behind or to the end of the text, and every reference to it in the code follows.
pub fn rebuild_script(member: &[u8], replacements: &BTreeMap<u32, Vec<u8>>, name: &str) -> Result<(Vec<u8>, usize)> {
    let broken = || Error::Text(format!("script {name} is not laid out as a story script is"));
    let count = number(member, 8).ok_or_else(broken)? as usize;
    let entries_start = 16 + 4 * count;
    let mut sizes: Vec<u32> =
        (0..count).map(|entry| number(member, 16 + 4 * entry)).collect::<Option<_>>().ok_or_else(broken)?;
    let Some((last_size, before)) = sizes.split_last() else { return Err(broken()) };
    let last_start = entries_start + before.iter().map(|size| *size as usize).sum::<usize>();
    let within = |at: usize| at.min(member.len());
    let packed = &member[within(last_start)..within(last_start + *last_size as usize)];
    let (data, moved) = rebuild_text(lzss_ring::decompress_block(packed, 0)?, replacements, name)?;
    let last = lzss_ring::compress_block(&data, packed[0])?;
    sizes[count - 1] = u32::try_from(last.len()).map_err(|_| broken())?;
    let sizes: Vec<u8> = sizes.iter().flat_map(|size| size.to_le_bytes()).collect();
    Ok(([&member[..16], &sizes, &member[entries_start..last_start], &last].concat(), moved))
}

/// A world map's member with some of its script's strings replaced, and how many of them had to
/// move.
///
/// The script is written back as a story script is (see `rebuild_text`), so it may grow. The entries
/// after it start later, and the pack's table gives the script its new size; the game works out where
/// each entry is from the sizes. Refused if the member would pass `WORLD_MAP_ROOM`.
pub fn rebuild_world_map(member: &[u8], replacements: &BTreeMap<u32, Vec<u8>>, name: &str) -> Result<(Vec<u8>, usize)> {
    let broken = || Error::Text(format!("world map {name} is not laid out as a world map is"));
    let place = world_map_entries(member).and_then(|entries| entries.get(WORLD_MAP_SCRIPT).cloned());
    let (Some(place), Some(script)) = (place, world_map_script(member)) else { return Err(broken()) };
    let (script, moved) = rebuild_text(script.to_vec(), replacements, name)?;
    let size = u32::try_from(script.len()).map_err(|_| broken())?;
    let mut rebuilt = member[..place.start].to_vec();
    let table = 4 + 4 * WORLD_MAP_SCRIPT;
    rebuilt[table..table + 4].copy_from_slice(&size.to_le_bytes());
    rebuilt.extend(script);
    rebuilt.extend_from_slice(&member[place.end..]);
    if rebuilt.len() > WORLD_MAP_ROOM {
        return text_error(format!(
            "world map {name} would take {} bytes with the pack's strings, and the game reads it into a block of \
             {WORLD_MAP_ROOM}: shorten some of its strings (a wide character takes two bytes and a narrow one takes \
             one)",
            rebuilt.len()
        ));
    }
    Ok((rebuilt, moved))
}

/// A skit member with some of its strings replaced, and how many of them had to move.
///
/// The pack is written out again around the new script. When the script has grown past where the
/// next entry started, that entry and those after it start later, and the pack's table says where.
/// `paces` are numbers to write over the script's own, each as where it is in the script and its
/// new bytes, which take as many as the old (see `skit_paces`); with none, only strings change.
pub fn rebuild_skit(
    member: &[u8],
    replacements: &BTreeMap<u32, Vec<u8>>,
    paces: &[(usize, Vec<u8>)],
    name: &str,
) -> Result<(Vec<u8>, usize)> {
    let broken = || Error::Text(format!("skit {name} is not laid out as a skit is"));
    let (mode, _, _) = lzss_ring::read_header(member, 0)?;
    let pack = lzss_ring::decompress_block(member, 0)?;
    let entries = pack_entries(&pack).filter(|entries| entries.len() == SKIT_ENTRIES).ok_or_else(broken)?;
    let part = |(at, size): &(usize, usize)| pack.get(*at..at.checked_add(*size)?);
    let mut parts: Vec<&[u8]> = entries.iter().map(part).collect::<Option<_>>().ok_or_else(broken)?;
    let (mut script, moved) = rebuild_text(parts[SKIT_SCRIPT].to_vec(), replacements, name)?;
    // The numbers are in the code, which comes before the text, so they are where they were.
    for (at, bytes) in paces {
        script.get_mut(*at..at + bytes.len()).ok_or_else(broken)?.copy_from_slice(bytes);
    }
    parts[SKIT_SCRIPT] = &script;
    Ok((lzss_ring::compress_block(&pack_of(&parts), mode)?, moved))
}

// --- how fast a skit's lines are typed

/// The build option that types a skit's changed lines faster, so that each is whole on the screen
/// before the game takes it away.
pub const SKIT_PACE: &str = "skit-pace";
/// Which entry of a skit's pack moves the speakers' lips: a 32-bit count of bytes, then a nibble a
/// frame. The skit's clock counts frames of the voice and stops when these run out, and the skit
/// ends then, taking its last lines away.
const SKIT_LIPS: usize = 1;
/// The instruction of a script's code that calls the routine at the place in the code its 16-bit
/// number gives. A skit shows a line by calling one with the string as its last argument.
const ROUTINE_CALL: u8 = 0xF5;
/// The comparison a line's block starts with: the skit's clock is at least the number before it.
const AT_LEAST: u8 = 0xCE;
/// Frames that a line is whole later than its letters times its pace says. Measured on screen, a
/// line was whole up to 13 frames later (a letter fades in, and a long line scrolls a little behind
/// the typing); clips of whole skits then found what is left of a line up to 4 frames shorter than
/// that figure gave, so it is 18.
const TYPING_ALLOWANCE: u32 = 18;
/// The frames a changed line is left whole, at most, before the game takes it away: a second. A
/// line the game itself leaves whole for less needs no more than the game gives it.
const SKIT_REST: u32 = 60;
/// The fewest frames a letter that the option types a line at. Below the game's own fastest pace
/// for most lines, and still a pace a reader can follow.
const FASTEST_PACE: u32 = 5;

// --- how the grids of artes draw a spell's name

/// The build option that draws a spell's name in the grids of artes in the narrow font.
pub const NARROW_SPELL_NAMES: &str = "narrow-spell-names";
/// Where the Artes screen's module (member 6798) and the Enchant screen's (6804) mark a spell's
/// row to be drawn large.
///
/// A grid of artes is a list the program draws, which asks the screen's module for each row: the
/// module fills in what the row is, and a flag in it (bit 0 of its first word) has the row drawn by
/// another routine, in larger letters. The module sets that flag for a spell and not for a skill,
/// with one `ori v1, v1, 1` each, in the function that answers for a row. A skill's name is drawn
/// narrow, with room for 17 letters in its column; a spell's large, with room for 9. With the
/// option, the instruction becomes `ori v1, v1, 0`, which leaves the row as it is, so a spell's
/// name is drawn as a skill's. Nothing else of the row changes: its name, its colour, its place.
const SPELL_FLAGS: [(usize, usize); 2] = [(6798, 0x1250), (6804, 0xE40)];
/// `ori v1, v1, 1`, and `ori v1, v1, 0`.
const MARK_LARGE: u32 = 0x3463_0001;
const LEAVE_AS_IT_IS: u32 = 0x3463_0000;

/// A line of a skit, as its script shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SkitLine {
    /// The string it shows, by its offset in the script's text.
    string: usize,
    /// The skit's clock, in frames, when the line starts; None where the script does not say it the
    /// way the game's skits do.
    start: Option<u32>,
    /// When it is taken away: when the line two after it starts, the newer of two rows being below,
    /// or when the skit ends.
    removed: u32,
    /// Its pace, in frames per letter, if the script gives it as a plain number.
    pace: Option<u32>,
    /// Where that number is in the script, from its header, and how many bytes it takes.
    pace_at: usize,
    pace_size: usize,
}

/// A plain number of a script's code: $8x is x, $9x is x and one more byte, and $Ax x and two more
/// (see `value_length`).
fn plain_number(token: &[u8]) -> Option<u32> {
    match *token {
        [first @ 0x80..0x90] => Some(u32::from(first & 0x0F)),
        [first @ 0x90..0xA0, low] => Some(u32::from(first & 0x0F) << 8 | u32::from(low)),
        [first @ 0xA0..0xB0, low, middle] => {
            Some(u32::from(first & 0x0F) << 16 | u32::from(low) | u32::from(middle) << 8)
        }
        _ => None,
    }
}

/// `value` written as a plain number in `size` bytes, if it fits in them.
fn plain_number_of(value: u32, size: usize) -> Option<Vec<u8>> {
    let bytes = match size {
        1 if value < 0x10 => vec![0x80 | value as u8],
        2 if value < 0x1000 => vec![0x90 | (value >> 8) as u8, value as u8],
        3 if value < 0x10_0000 => vec![0xA0 | (value >> 16) as u8, value as u8, (value >> 8) as u8],
        _ => return None,
    };
    Some(bytes)
}

/// The lines of a skit's script (its header, its code, then its text), in the order of its code.
/// `end` is when the skit ends, on its clock.
///
/// Each line is a block of the code that runs while the clock is at least its start and below its
/// start and 3: `start $CE` and `start+3 $CF`, then the call that shows it, whose arguments are who
/// speaks, the row, 0, the frames per letter, and the string. A line is taken away when the line
/// two after it starts, or when the skit ends. 2,412 of the game's 2,413 lines are written so; the
/// one that is not is the only line of skit 00054, which tests the clock with another comparison.
fn skit_lines(script: &[u8], end: u32) -> Result<Vec<SkitLine>> {
    let broken = || Error::Text("a skit's script is not laid out as a script is".into());
    let code_start = number(script, 4).ok_or_else(broken)? as usize;
    let text_start = number(script, 8).ok_or_else(broken)? as usize;
    let code = script.get(code_start..text_start).ok_or_else(broken)?;
    let tokens = code_tokens(code)?;
    let mut lines = Vec::new();
    let mut block_start = 0; // where the block of the next line can start: after the call of the last
    for (index, token) in tokens.iter().enumerate() {
        let (Some([STRING_REFERENCE, low, high]), [ROUTINE_CALL, ..]) =
            (index.checked_sub(1).map(|at| tokens[at]), token)
        else {
            continue;
        };
        let Some(pace_token) = index.checked_sub(2).map(|at| tokens[at]) else { continue };
        let block = &tokens[block_start..index];
        let start = block.iter().rposition(|token| *token == [AT_LEAST]);
        let start = start.and_then(|at| at.checked_sub(1)).and_then(|at| plain_number(block[at]));
        lines.push(SkitLine {
            string: usize::from(*low) | usize::from(*high) << 8,
            start,
            removed: end,
            pace: plain_number(pace_token),
            pace_at: code_start + token_start(code, pace_token),
            pace_size: pace_token.len(),
        });
        block_start = index + 1;
    }
    let mut timed: Vec<usize> = (0..lines.len()).filter(|line| lines[*line].start.is_some()).collect();
    timed.sort_by_key(|line| lines[*line].start);
    for (order, line) in timed.iter().enumerate() {
        if let Some(later) = timed.get(order + 2) {
            lines[*line].removed = lines[*later].start.unwrap_or(end);
        }
    }
    Ok(lines)
}

/// How many letters a skit's line types, counted as the game draws them: each character and space
/// one, a party member's name as `NAME_CHARACTERS` (what a build cannot know: the player may rename
/// the party), a button's picture one, and a single byte one.
/// Anything else is given back as written: no skit of the game uses it, so how long it takes is not
/// known. A line break is one of these, since a skit's line is one row.
fn skit_letters(text: &str) -> std::result::Result<u32, String> {
    let mut letters = 0;
    for token in tokens(text) {
        letters += match token {
            Token::Char('\n') => return Err("a line break".into()),
            Token::Char(_) => 1,
            Token::Tag { name, .. } if NAME_CODES.contains(&name) => NAME_CHARACTERS as u32,
            Token::Tag { name, .. } if name == BUTTON_CODE || name == "G" => 1,
            Token::Tag { name, value: None, .. }
                if u8::from_str_radix(name, 16).is_ok_and(|byte| byte >= 0x20) && name.len() == 2 =>
            {
                1
            }
            Token::Tag { raw, .. } => return Err(raw.to_string()),
        };
    }
    Ok(letters)
}

/// The pace for a changed line of a skit, in frames per letter, or, if none will do, the most letters
/// that would fit.
///
/// The line shows from its start to when it is taken away, `window` frames. Typing it takes its
/// letters times the pace and `TYPING_ALLOWANCE`, and what is left of the window must be at least
/// `SKIT_REST`, or what is left for the game's own line at the game's own pace if that is less. The
/// pace is the slowest that leaves that, no slower than the game's own and no faster than
/// `FASTEST_PACE` (or the game's own, where that is faster still).
fn line_pace(window: u32, letters: u32, own_pace: u32, own_letters: u32) -> std::result::Result<u32, u32> {
    let typing = |letters: u32, pace: u32| letters.saturating_mul(pace).saturating_add(TYPING_ALLOWANCE);
    let rest = SKIT_REST.min(window.saturating_sub(typing(own_letters, own_pace)));
    let fastest = FASTEST_PACE.min(own_pace).max(1);
    match (fastest..=own_pace.max(fastest)).rev().find(|pace| typing(letters, *pace).saturating_add(rest) <= window) {
        Some(pace) => Ok(pace),
        None => Err(window.saturating_sub(TYPING_ALLOWANCE + rest) / fastest),
    }
}

/// What the option `skit-pace` does to one skit.
#[derive(Debug, Default, PartialEq, Eq)]
struct Paced {
    /// The numbers to write over the script's own, as for `rebuild_skit`.
    writes: Vec<(usize, Vec<u8>)>,
    /// How many changed lines were given a pace, and how many of them a faster one than the game's.
    timed: usize,
    faster: usize,
    /// The lines that cannot be shown whole, each as a translator is told of it.
    refused: Vec<String>,
}

/// The pace of each line of a skit member whose string the pack changes (see `line_pace`).
/// `changed` gives, by the string's number, the pack's text and the game's own. Lines whose string
/// is not changed keep the game's pace.
fn skit_paces(member: &[u8], name: &str, changed: &BTreeMap<u32, (String, String)>) -> Result<Paced> {
    let broken = || Error::Text(format!("skit {name} is not laid out as a skit is"));
    let pack = lzss_ring::decompress_block(member, 0)?;
    let entries = pack_entries(&pack).filter(|entries| entries.len() == SKIT_ENTRIES).ok_or_else(broken)?;
    let part = |(at, size): (usize, usize)| pack.get(at..at.checked_add(size)?);
    let script = part(entries[SKIT_SCRIPT]).ok_or_else(broken)?;
    let end = part(entries[SKIT_LIPS]).and_then(|lips| number(lips, 0)).ok_or_else(broken)?.saturating_mul(2);
    let mut numbers: HashMap<usize, u32> = HashMap::new();
    let mut offset = 0;
    for (number, string) in split_strings(text_of(script)).iter().enumerate() {
        numbers.insert(offset, number as u32);
        offset += string.len() + 1;
    }
    let mut paced = Paced::default();
    for line in skit_lines(script, end)? {
        let Some((number, (text, own))) = numbers.get(&line.string).and_then(|number| changed.get_key_value(number))
        else {
            continue;
        };
        let string = string_name(name, *number);
        let (Some(start), Some(pace)) = (line.start, line.pace) else {
            paced.refused.push(format!(
                "{string}: its skit's script does not say when the line starts the way the others do, so it cannot \
                 be timed"
            ));
            continue;
        };
        let letters = match skit_letters(text) {
            Ok(letters) => letters,
            Err(what) => {
                paced
                    .refused
                    .push(format!("{string}: {what} cannot be timed in a skit, since no skit of the game has one"));
                continue;
            }
        };
        // The game's own line has nothing that cannot be counted; if it had, its rest would count as
        // the whole of `SKIT_REST`.
        let own_letters = skit_letters(own).unwrap_or(0);
        match line_pace(line.removed.saturating_sub(start), letters, pace, own_letters) {
            Ok(new) => {
                paced.timed += 1;
                if new != pace {
                    paced.faster += 1;
                    paced.writes.push((line.pace_at, plain_number_of(new, line.pace_size).ok_or_else(broken)?));
                }
            }
            Err(most) => paced.refused.push(format!("{string} has {letters} letters, and at most {most} fit")),
        }
    }
    Ok(paced)
}

/// What a translator is told of the skit lines that cannot be shown whole, each named in `refused`.
fn skit_refusal(refused: &[String]) -> Option<String> {
    const WHY: &str = "with the option skit-pace a changed line is typed at 5 frames a letter at the fastest, and \
                       must then stay whole for a second, or as long as the game's own line does if that is less, \
                       before the line two after it starts or the skit ends";
    match refused {
        [] => None,
        [one] => Some(format!("skit line {one}: {WHY}")),
        several => Some(format!(
            "{} skit lines cannot be shown whole: {WHY}\n{}",
            several.len(),
            several.iter().map(|line| format!("  {line}")).collect::<Vec<_>>().join("\n")
        )),
    }
}

/// A script (its header, its code, then its text) with some of its strings replaced, and how many
/// of them had to move. `replacements` gives the new bytes of each string by its number, and `name`
/// is the script's.
fn rebuild_text(mut data: Vec<u8>, replacements: &BTreeMap<u32, Vec<u8>>, name: &str) -> Result<(Vec<u8>, usize)> {
    let broken = || Error::Text(format!("script {name} is not laid out as a script is"));
    let code_start = number(&data, 4).ok_or_else(broken)? as usize;
    let text_start = number(&data, 8).ok_or_else(broken)? as usize;
    if code_start > text_start || text_start > data.len() {
        return Err(broken());
    }
    let mut text = data.split_off(text_start);
    let references = string_references(&data[code_start..])?;

    let lengths: Vec<usize> = split_strings(&text).iter().map(|string| string.len()).collect();
    let mut moving = Vec::new(); // (the string's number, its old offset, its new bytes)
    let mut free: Vec<(usize, usize)> = Vec::new(); // the start and end of room in the text that nothing refers to any more
    let mut position = 0;
    for (number, old) in lengths.into_iter().enumerate() {
        let number = number as u32;
        match replacements.get(&number) {
            Some(new) if new.len() <= old => {
                text[position..position + new.len()].copy_from_slice(new);
                text[position + new.len()..position + old].fill(0); // the rest reads as empty strings
            }
            Some(new) => {
                if !references.contains_key(&position) {
                    return text_error(format!(
                        "string {} takes {} bytes where the game's has {old}, and it cannot move: nothing in the \
                         script's code refers to it",
                        string_name(name, number),
                        new.len()
                    ));
                }
                moving.push((number, position, new));
                let end = position + old + 1;
                // The last string may have no zero byte after it; the room it leaves gets one.
                text.resize(text.len().max(end), 0);
                text[position..end].fill(0);
                match free.last_mut() {
                    Some(last) if last.1 == position => last.1 = end,
                    _ => free.push((position, end)),
                }
            }
            None => {}
        }
        position += old + 1;
    }

    let moved = moving.len();
    moving.sort_by_key(|(_, _, new)| Reverse(new.len())); // the longest first
    for (number, old_offset, new) in moving {
        let offset = match free.iter_mut().find(|(start, end)| *end - *start > new.len()) {
            Some(room) => {
                let offset = room.0;
                room.0 += new.len() + 1;
                offset
            }
            None => {
                let offset = text.len();
                text.resize(offset + new.len() + 1, 0);
                offset
            }
        };
        let Ok(reference) = u16::try_from(offset) else {
            return text_error(format!(
                "string {} does not fit: the text of script {name} would pass 65,535 bytes, the furthest the code \
                 can refer to",
                string_name(name, number)
            ));
        };
        text[offset..offset + new.len()].copy_from_slice(new);
        for place in &references[&old_offset] {
            data[code_start + place..code_start + place + 2].copy_from_slice(&reference.to_le_bytes());
        }
    }

    data.extend(text);
    Ok((data, moved))
}

/// The table of contents of the archive once some members are replaced: `old` is the game's, and
/// `members` holds the new contents by member number. Every member after one that changed size
/// sits at another offset.
fn new_table(old: &[u32], members: &BTreeMap<usize, Vec<u8>>) -> Result<Vec<u32>> {
    let too_large =
        || Error::Text(format!("{ARCHIVE} would pass 4 GiB, the furthest its table of contents can refer to"));
    let entry =
        |position: u64, padding: u32| u32::try_from(position).map(|offset| offset | padding).map_err(|_| too_large());
    let mut table = Vec::with_capacity(old.len());
    let mut position = 0; // in the new archive
    for index in 0..MEMBER_COUNT {
        match members.get(&index) {
            Some(data) => {
                let padding = (data.len() as u64).wrapping_neg() & u64::from(PADDING_BITS);
                table.push(entry(position, padding as u32)?);
                position += data.len() as u64 + padding;
            }
            None => {
                table.push(entry(position, old[index] & PADDING_BITS)?);
                let size = (old[index + 1] & !PADDING_BITS).checked_sub(old[index] & !PADDING_BITS);
                let Some(size) = size else {
                    return text_error(format!("the table of contents of {ARCHIVE} is not in order at member {index}"));
                };
                position += u64::from(size);
            }
        }
    }
    table.push(entry(position, 0)?);
    Ok(table)
}

/// A stretch of the archive as it is written out again.
enum Piece {
    /// Unchanged members, copied from the game's archive: where the stretch starts, and its length.
    Kept(u64, u64),
    /// A member with new contents, by its number.
    Replaced(usize),
}

/// The archive in pieces, with some members replaced. `old` is the game's table of contents.
fn archive(disc: &Disc, old: &[u32], members: &BTreeMap<usize, Vec<u8>>) -> impl Iterator<Item = Result<Vec<u8>>> {
    let mut pieces = Vec::new();
    let mut copy_from = 0; // in the old archive: where the stretch of unchanged members being gathered starts
    for index in 0..=MEMBER_COUNT {
        let old_offset = u64::from(old[index] & !PADDING_BITS);
        if index == MEMBER_COUNT || members.contains_key(&index) {
            // The unchanged members before this one.
            let mut start = copy_from;
            while start < old_offset {
                pieces.push(Piece::Kept(start, COPY_CHUNK.min(old_offset - start)));
                start += COPY_CHUNK;
            }
            if index < MEMBER_COUNT {
                pieces.push(Piece::Replaced(index));
                copy_from = u64::from(old[index + 1] & !PADDING_BITS);
            }
        }
    }
    pieces.into_iter().map(move |piece| match piece {
        Piece::Kept(start, length) => disc.read(ARCHIVE, start, length),
        Piece::Replaced(index) => {
            let mut data = members[&index].clone();
            data.resize(data.len().next_multiple_of(PADDING_BITS as usize + 1), 0);
            Ok(data)
        }
    })
}

/// Write to `output` a disc image holding the strings `texts` gives; the rest keeps the game's own.
///
/// Characters the game cannot draw are drawn from `sources`, fonts to try in order (BDF, or
/// Unifont's `.hex`), into spare cells of the game's fonts; with none given, from this game's own
/// (`FONTS`). No line is rewrapped.
pub fn build(disc: &Disc, texts: &Script, output: &Path, sources: &[Font]) -> Result<BuildResult> {
    build_as(disc, texts, output, sources, true, Switches::default())
}

/// A line of dialogue broken at spaces where it is too wide for the box, and narrower where a page
/// would draw more characters than the game can (see `PAGE_CHARACTERS`). `chars` says how each
/// character of the pack is written, which decides whether it is drawn narrow or wide, and `names`
/// the columns of each party member's name (see `party_name_columns`).
fn wrap_dialogue(text: &str, chars: &Characters, names: &NameColumns) -> Result<String> {
    let room = PAGE_CHARACTERS - PAGE_MARGIN;
    let fits = |wrapped: &str| page_characters(wrapped).iter().all(|count| *count <= room);
    let wrapped = wrap_to(text, chars, LINE_COLUMNS, false, names)?;
    if fits(&wrapped) {
        return Ok(wrapped);
    }
    // Narrower, with the text the game fills in taking the columns it is counted as, so that a
    // line holding it is broken sooner and its page holds less.
    let mut over = (0, 0); // the first page that is too full at the narrowest width, and its count
    for width in (NARROWEST_COLUMNS..LINE_COLUMNS).rev() {
        let wrapped = wrap_to(text, chars, width, true, names)?;
        let pages = page_characters(&wrapped);
        match pages.iter().position(|count| *count > room) {
            None => return Ok(wrapped),
            Some(page) => over = (page + 1, pages[page]),
        }
    }
    let (page, count) = over;
    text_error(format!(
        "page {page} of this text draws {count} characters even with its lines broken at {NARROWEST_COLUMNS} \
         columns, and a page of a speech bubble shows at most {room} (spaces count, a name as {NAME_CHARACTERS}, \
         a number as {NUMBER_CHARACTERS} and an item as {THING_CHARACTERS}): shorten it, or end the page sooner \
         with a line break or {{{PAGE_BREAK}}}"
    ))
}

/// A line of dialogue broken at spaces to `width` columns. A page break starts a line afresh.
///
/// A party member's name takes the columns `names` gives it (see `name_columns`). With `filled`,
/// the other text the game fills in takes room too: a name worked out as the game runs that of the
/// longest, a number or a thing's name what it is counted as, and a button's picture two. Without
/// it they take none, which is how a string that fits at the full width has always been broken.
fn wrap_to(text: &str, chars: &Characters, width: usize, filled: bool, names: &NameColumns) -> Result<String> {
    let columns = |char: char| Ok(char_columns(char, chars));
    let tag = |name: &str, value: Option<&str>| tag_columns(name, value, filled, names);
    // A character may start left of the limit, and a wide one takes two columns from there.
    layout::wrap_lines(text, &columns, width - 1, &tag, &[PAGE_BREAK])
}

/// The columns a character of the pack takes: one if it is drawn narrow, two if wide.
fn char_columns(char: char, chars: &Characters) -> usize {
    let narrow =
        chars.singles.contains_key(&char) || (!chars.glyphs.contains_key(&char) && single_byte(char).is_some());
    if narrow { 1 } else { 2 }
}

/// The columns a text of plain characters takes, as `char_columns` counts them; None if it holds a
/// tag or a line break, whose width is not that simple.
fn text_columns(text: &str, chars: &Characters) -> Option<usize> {
    tokens(text)
        .into_iter()
        .map(|token| match token {
            Token::Char('\n') | Token::Tag { .. } => None,
            Token::Char(char) => Some(char_columns(char, chars)),
        })
        .sum()
}

/// The columns each party member's name takes on screen, members 1 to 6 in order.
type NameColumns = [usize; PARTY_MEMBERS];

/// Every party member's name as a build counts it when it does not know them.
#[cfg(test)]
const UNKNOWN_NAMES: NameColumns = [NAME_COLUMNS; PARTY_MEMBERS];

/// The columns of each party member's name as a new game has it: the name the pack gives the
/// member, where it changes the string of the program that the game takes the name from (see
/// `PARTY_NAME_TABLE`), or else the game's own, at the width it is drawn. `program` is the game's
/// own program, `pack` the strings of the program the pack changes, by their offsets, and `chars`
/// says how each character of the pack is written.
///
/// A name that cannot be told is counted as `NAME_COLUMNS`: one the table does not point at, and
/// one the pack writes with a tag or a line break in it.
fn party_name_columns(program: &[u8], pack: &HashMap<u32, &str>, chars: &Characters) -> NameColumns {
    party_name_offsets(program).map(|offset| {
        let Some(offset) = offset else {
            return NAME_COLUMNS;
        };
        match pack.get(&offset) {
            Some(text) => text_columns(text, chars).unwrap_or(NAME_COLUMNS),
            // A narrow character of the game's is one byte and a wide one two, so a name of its own
            // takes as many columns as it has bytes.
            None => {
                let name = program.get(offset as usize..).and_then(|rest| rest.iter().position(|byte| *byte == 0));
                name.filter(|length| *length > 0).unwrap_or(NAME_COLUMNS)
            }
        }
    })
}

/// The offset in the program of the string each party member's name is taken from when a new game
/// starts, members 1 to 6 in order, as the table at `PARTY_NAME_TABLE` points at them; None where
/// the table does not point into the program.
fn party_name_offsets(program: &[u8]) -> [Option<u32>; PARTY_MEMBERS] {
    std::array::from_fn(|member| {
        let at = PARTY_NAME_TABLE + 8 * member;
        let word = program.get(at..at + 4).map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        word.and_then(|address| address.checked_sub(PROGRAM_BASE))
    })
}

/// The columns the name a `{NAME:..}` tag prints takes, by the tag's number: party member 1 to 6's
/// as `names` has it, and for any other number `NAME_COLUMNS`, since the game chooses the name as it
/// runs or prints only part of it (see `PARTY_MEMBERS`).
fn name_columns(value: Option<&str>, names: &NameColumns) -> usize {
    let number = value.and_then(from_hex).and_then(|bytes| <[u8; 4]>::try_from(bytes).ok()).map(u32::from_le_bytes);
    match number {
        Some(number) if (1..=PARTY_MEMBERS as u32).contains(&number) => names[number as usize - 1],
        _ => NAME_COLUMNS,
    }
}

/// The columns a tag takes, by its name and what follows its colon: see `wrap_to`.
fn tag_columns(name: &str, value: Option<&str>, filled: bool, names: &NameColumns) -> usize {
    match name {
        "NAME" => name_columns(value, names),
        _ if !filled => 0,
        _ if NAME_CODES.contains(&name) => NAME_COLUMNS,
        _ if NUMBER_CODES.contains(&name) => NUMBER_CHARACTERS,
        _ if THING_CODES.contains(&name) => THING_CHARACTERS,
        BUTTON_CODE => 2,
        _ => 0,
    }
}

/// A list to choose from (see `choice_lists`), left as the translator wrote it: a line is one
/// choice, or a line of the question before them, so breaking a line would make two choices of
/// one. `original` is the game's own string. Refused, with what to do, if it has more lines than
/// the game's own (a line more would be a choice more, or push the last choice onto a page of its
/// own), if a line is wider than the widest of the game's own lists, or if it holds more than a
/// page shows (see `PAGE_CHARACTERS`). A line is measured as `wrap_to` measures one at the full
/// width, a party member's name taking the columns `names` gives it.
fn fit_choices(text: &str, original: &str, chars: &Characters, names: &NameColumns) -> Result<String> {
    let (lines, own) = (text.split('\n').count(), original.split('\n').count());
    if lines > own {
        return text_error(format!(
            "this is a list to choose from, one choice to a line, and it has {lines} lines where the game's own has \
             {own}: write each choice on a line of its own, and the question before them, if there is one, on no \
             more lines than the game's own"
        ));
    }
    for (number, line) in text.split('\n').enumerate() {
        let width: usize = tokens(line)
            .into_iter()
            .map(|token| match token {
                Token::Char(char) => char_columns(char, chars),
                Token::Tag { name, value, .. } => tag_columns(name, value, false, names),
            })
            .sum();
        if width > CHOICE_COLUMNS {
            return text_error(format!(
                "line {} of this list to choose from takes {width} columns, and a line of a list may take \
                 {CHOICE_COLUMNS} (a wide character takes two, and a party member's name as many as it is drawn \
                 with): a list is not rewrapped, \
                 since each line is a choice, so shorten the line",
                number + 1
            ));
        }
    }
    let room = PAGE_CHARACTERS - PAGE_MARGIN;
    if let Some(count) = page_characters(text).into_iter().find(|count| *count > room) {
        return text_error(format!(
            "this list to choose from draws {count} characters, and a window shows at most {room} (spaces count, a \
             name as {NAME_CHARACTERS}, a number as {NUMBER_CHARACTERS} and an item as {THING_CHARACTERS}): shorten \
             its lines"
        ));
    }
    Ok(text.to_string())
}

/// How many places of the game's list each page of a string takes, as the game breaks it into
/// pages: four lines that are not empty, or fewer where a page break comes first.
fn page_characters(text: &str) -> Vec<usize> {
    let mut pages = Vec::new();
    let (mut count, mut lines, mut in_line) = (0, 0, false);
    for token in tokens(text) {
        match token {
            Token::Char('\n') => {
                // The game passes over empty lines.
                if in_line {
                    lines += 1;
                    in_line = false;
                }
                if lines == PAGE_LINES {
                    pages.push(count);
                    (count, lines) = (0, 0);
                }
                continue;
            }
            Token::Tag { name, .. } if name == PAGE_BREAK => {
                pages.push(count);
                (count, lines, in_line) = (0, 0, false);
                continue;
            }
            Token::Char(_) => count += 1,
            Token::Tag { name, .. } if NAME_CODES.contains(&name) => count += NAME_CHARACTERS,
            Token::Tag { name, .. } if NUMBER_CODES.contains(&name) => count += NUMBER_CHARACTERS,
            Token::Tag { name, .. } if THING_CODES.contains(&name) => count += THING_CHARACTERS,
            Token::Tag { name, .. } if name == BUTTON_CODE || name == "G" => count += 1,
            Token::Tag { .. } => {}
        }
        in_line = true;
    }
    if in_line || lines > 0 {
        pages.push(count);
    }
    pages
}

/// What a translator is told of the strings that take more bytes than they have room for, each
/// given as its name, the bytes it takes and the room it has: one sentence for one string, and for
/// several a line each, so that all can be shortened before the next build.
fn refusal_of(too_long: &[(String, usize, usize)]) -> Option<String> {
    const WHY: &str = "this text stays where it is, and a wide character takes two bytes where a narrow one takes one";
    match too_long {
        [] => None,
        [(name, bytes, room)] => Some(format!("string {name} takes {bytes} bytes and has room for {room}: {WHY}")),
        several => {
            let lines: Vec<String> = several
                .iter()
                .map(|(name, bytes, room)| {
                    format!("  {name} takes {bytes} bytes and has room for {room}, {} over", bytes - room)
                })
                .collect();
            Some(format!(
                "{} strings take more bytes than they have room for: {WHY}\n{}",
                several.len(),
                lines.join("\n")
            ))
        }
    }
}

/// What a translator is told when strings found no place in the room they share. `of` says whose
/// they are, the lists' or one menu's, and `shared` what they share. `left` names each string with
/// the bytes it needs, `room` is what the pack's strings have between them, `need` what they take,
/// and `largest` the largest piece that was left. All count the zero byte that ends a string.
fn no_place_refusal(of: &str, shared: &str, left: &[(String, usize)], sizes: (usize, usize, usize)) -> String {
    let (room, need, largest) = sizes;
    let which = match left {
        [(name, _)] => format!("string {name} found no place"),
        several => format!("{} strings of {of} found no place", several.len()),
    };
    let lines: String = left.iter().map(|(name, bytes)| format!("\n  {name} needs {bytes} bytes")).collect();
    format!(
        "{which}: the strings of {of} that a pack changes share {shared}, {room} bytes here with the zero byte \
         that ends each, and this pack's need {need}; the largest piece left has {largest}. Shorter text in any of \
         them makes room{lines}"
    )
}

/// The build options a pack can turn on, each off unless asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// The changed lines of skits are typed fast enough to be whole on the screen before they are
    /// taken away (see `skit_paces`), and a line too long for that is refused.
    pub skit_pace: bool,
    /// The grids of artes draw a spell's name in the narrow font, as they draw a skill's (see
    /// `SPELL_FLAGS`).
    pub narrow_spell_names: bool,
}

/// The same as `build`; with `wrap`, the lines of changed dialogue are broken at spaces to fit the
/// box, and without, they are left as the pack has them. `switches` are the options the pack asks
/// for.
pub fn build_as(
    disc: &Disc,
    texts: &Script,
    output: &Path,
    sources: &[Font],
    wrap: bool,
    switches: Switches,
) -> Result<BuildResult> {
    let skit_pace = switches.skit_pace;
    let mut chars = (*characters(disc)?).clone();
    let skits = skit_scripts(disc)?;
    let world_maps = world_map_scripts(disc)?;
    let members_with_text =
        script_members(disc)?.into_iter().chain(skits.keys().copied()).chain(world_maps.keys().copied());
    let sections: BTreeMap<String, usize> = members_with_text.map(|index| (section_name(index), index)).collect();
    let ascii_table = disc.read(EXECUTABLE, ASCII_TABLE, ASCII_TABLE_SIZE)?;
    if ascii_table.len() as u64 != ASCII_TABLE_SIZE {
        return text_error(format!("{EXECUTABLE} is cut short"));
    }

    // Text that is found by what points at it is put back in its own way, so the pack's menus and
    // its strings of the program are taken apart from the rest.
    let own_chars = characters(disc)?;
    let modules = menu_modules(disc)?;
    let program = program_file(disc)?;
    let enemies = enemy_names(disc)?;
    let games = game_scripts(disc)?;
    let (menu_pack, texts): (Script, Script) = texts
        .iter()
        .map(|(section, items)| (section.clone(), items.clone()))
        .partition(|(section, _)| is_placed(section));
    let texts = &texts;
    let menu_original = |section: &str| {
        let strings = match section {
            PROGRAM_SECTION => program_strings(&program, &own_chars),
            _ if is_enemy(section) => enemies.get(&member_of(section)?)?.clone(),
            _ if is_game(section) => game_strings(games.get(&member_of(section)?)?, &own_chars),
            _ => {
                let index = member_of(section)?;
                strings_of_module(index, modules.get(&index)?, &own_chars)
            }
        };
        Some(strings.iter().map(|(offset, data)| (*offset, decode(data, &own_chars))).collect())
    };
    let menu_changes = script::changes_by_place(&menu_pack, menu_original)?;

    // The game's own strings, for the sections the pack names, and which of a story script's are
    // lists to choose from.
    let mut originals: HashMap<&str, Vec<String>> = HashMap::new();
    let mut choices: HashMap<&str, BTreeSet<u32>> = HashMap::new();
    for section in texts.keys() {
        let text = match sections.get(section) {
            Some(index) => match skits.get(index) {
                Some(script) => text_of(script).to_vec(),
                None => {
                    let script = match world_maps.get(index) {
                        Some(script) => script.clone(),
                        None => read_script_data(disc, *index)?,
                    };
                    choices.insert(section, choice_strings(&script)?);
                    text_of(&script).to_vec()
                }
            },
            None if section == PROGRAM_SCRIPT_SECTION => {
                choices.insert(section, PROGRAM_SCRIPT_LISTS.into());
                text_of(program_script(&program)?).to_vec()
            }
            None => continue,
        };
        originals.insert(section, split_strings(&text).iter().map(|data| decode(data, &chars)).collect());
    }
    let changes = script::changes(texts, |section| originals.get(section).cloned())?;

    // What the changed strings need that the game cannot draw. ASCII it draws as something else
    // keeps its byte and gets a cell of its own in the half-width font. Other letters take a spare
    // single byte and a half-width cell while both last; what is left over, and anything from a
    // 16-pixel cell, goes into the main font and is drawn at full width.
    let changed = changes.iter().chain(&menu_changes);
    let used = plain_characters(&changed.map(|(_, _, text)| text.as_str()).collect::<String>());
    let is_new = |char: &char| {
        !chars.glyphs.contains_key(char)
            && (needs_narrow_glyph(*char, &ascii_table, &chars.by_glyph) || single_byte(*char).is_none())
    };
    let new: Vec<char> = used.iter().copied().filter(is_new).collect();
    let mut members: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut patches: Vec<(&str, u64, Vec<u8>)> = Vec::new();
    let (mut narrow, mut wide) = (Vec::new(), Vec::new());
    if !new.is_empty() {
        let own_fonts;
        let sources = if sources.is_empty() {
            own_fonts = Font::built_ins(FONTS);
            &own_fonts[..]
        } else {
            sources
        };
        let mut drawn = fonts::new_bitmaps(sources, &new, false)?;
        let latin: Vec<char> = LATIN_SET.chars().filter(|char| !drawn.contains_key(char)).collect();
        drawn.extend(fonts::new_bitmaps(sources, &latin, true)?);
        let kana_table = disc.read(EXECUTABLE, KANA_TABLE, KANA_TABLE_SIZE)?;
        let in_use = |cell: &usize| ascii_table.iter().chain(&kana_table).any(|used| usize::from(*used) == *cell);
        let cells: Vec<usize> = HALF_SPARE_CELLS.filter(|cell| !in_use(cell)).collect();
        let spare: Vec<u8> = SPARE_BYTES.filter(|byte| !used.contains(&kana(*byte))).collect();
        let (fixes, others): (Vec<char>, Vec<char>) = new.iter().partition(|char| single_byte(**char).is_some());
        let fix_cells = fonts::assign_slots(&fixes, &cells, "the game's half-width font")?;
        let room = spare.len().min(cells.len() - fixes.len());
        let fits: Vec<char> = others.iter().copied().filter(|char| fits_half_cell(&drawn[char])).take(room).collect();
        let fit_bytes: HashMap<char, u8> = fits.iter().copied().zip(spare).collect();
        let fit_cells: Vec<(char, usize)> = fits.iter().copied().zip(cells[fixes.len()..].iter().copied()).collect();
        wide = others.into_iter().filter(|char| !fit_bytes.contains_key(char)).collect();
        narrow = [fixes.as_slice(), &fits].concat();

        let header = disc.read(EXECUTABLE, HALF_FONT, lzss_ring::HEADER_SIZE as u64)?;
        let (mode, payload_size, _) = lzss_ring::read_header(&header, 0)?;
        let old_size = lzss_ring::HEADER_SIZE + payload_size;
        let mut texture = lzss_ring::decompress_block(&disc.read(EXECUTABLE, HALF_FONT, old_size as u64)?, 0)?;
        // Where the game has its own capitals and digits, for those the pack's fonts can draw; then
        // the new characters, a cell given later taking the place of one given earlier.
        let own = LATIN_SET.chars().filter(|char| drawn.contains_key(char));
        let mut to_draw: Vec<(char, usize)> =
            own.map(|char| (char, usize::from(ascii_table[char as usize - 0x20]))).collect();
        for (char, cell) in fixes.iter().map(|char| (*char, fix_cells[char])).chain(fit_cells.iter().copied()) {
            match to_draw.iter_mut().find(|(known, _)| *known == char) {
                Some(entry) => entry.1 = cell,
                None => to_draw.push((char, cell)),
            }
        }
        for (char, cell) in to_draw {
            draw_half_glyph(&mut texture, cell, &drawn[&char])?;
        }
        let mut block = lzss_ring::compress_block(&texture, mode)?;
        if block.len() > old_size {
            return text_error(format!(
                "the half-width font takes {} bytes compressed with {} new characters, and the program has room \
                 for {old_size}",
                block.len(),
                narrow.len()
            ));
        }
        block.resize(old_size, 0);
        patches.push((EXECUTABLE, HALF_FONT, block));
        for char in &fixes {
            patches.push((EXECUTABLE, ASCII_TABLE + *char as u64 - 0x20, vec![fix_cells[char] as u8]));
        }
        for (char, cell) in &fit_cells {
            patches.push((EXECUTABLE, KANA_TABLE + u64::from(fit_bytes[char]) - 0xA0, vec![*cell as u8]));
        }

        if !wide.is_empty() {
            let mut font_data = (*font(disc)?).clone();
            let wide_slots = fonts::assign_slots(&wide, &free_glyphs(&font_data), "the game's font")?;
            for (char, slot) in &wide_slots {
                font_data[slot * GLYPH_BYTES..(slot + 1) * GLYPH_BYTES].copy_from_slice(&draw_glyph(&drawn[char]));
            }
            members.insert(FONT_MEMBER, lzss_ring::compress_block(&font_data, lzss_ring::MODE_LZSS_RLE)?);
            chars.glyphs.extend(wide_slots);
        }
        chars.singles = fit_bytes;
    }

    // The party's names as a new game starts with them, the pack's where it gives them, for the
    // lines of dialogue that hold one.
    let pack_program: HashMap<u32, &str> = menu_changes
        .iter()
        .filter(|(section, ..)| section == PROGRAM_SECTION)
        .map(|(_, offset, text)| (*offset, text.as_str()))
        .collect();
    let names = party_name_columns(&program, &pack_program, &chars);

    let mut replacements: BTreeMap<usize, BTreeMap<u32, Vec<u8>>> = BTreeMap::new();
    // The same for the script the program holds itself, which is no member of the archive.
    let mut program_script_strings: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    // With `skit_pace`, the text of each changed line of a skit and the game's own, to time it by.
    let mut skit_texts: BTreeMap<usize, BTreeMap<u32, (String, String)>> = BTreeMap::new();
    for (section, number, text) in &changes {
        if skit_pace && is_skit(section) {
            let own = originals[section.as_str()][*number as usize].clone();
            skit_texts.entry(sections[section]).or_default().insert(*number, (text.clone(), own));
        }
        // A skit's line is one row that scrolls, so only a story script's lines are fitted to a box.
        let named = |error| match error {
            Error::Text(message) => Error::Text(format!("string {}: {message}", string_name(section, *number))),
            other => other,
        };
        // A list to choose from is left as it is written, a line to a choice.
        let is_choice = choices.get(section.as_str()).is_some_and(|lists| lists.contains(number));
        let fitted = match (wrap && is_dialogue(section), is_choice) {
            (false, _) => text.clone(),
            (true, true) => {
                fit_choices(text, &originals[section.as_str()][*number as usize], &chars, &names).map_err(named)?
            }
            (true, false) => wrap_dialogue(text, &chars, &names).map_err(named)?,
        };
        let data = encode(&fitted, &chars).map_err(named)?;
        match sections.get(section) {
            Some(index) => replacements.entry(*index).or_default().insert(*number, data),
            None => program_script_strings.insert(*number, data),
        };
    }

    let mut result = BuildResult::default();
    let mut skit_refused: Vec<String> = Vec::new();
    for (index, strings) in &replacements {
        let (offset, size) = member_span(disc, *index)?;
        let member = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
        let name = section_name(*index);
        if skits.contains_key(index) {
            let paces = match skit_texts.get(index) {
                Some(changed) => {
                    let paced = skit_paces(&member, &name, changed)?;
                    result.skit_timed += paced.timed;
                    result.skit_faster += paced.faster;
                    skit_refused.extend(paced.refused);
                    paced.writes
                }
                None => Vec::new(),
            };
            let (member, moved) = rebuild_skit(&member, strings, &paces, &name)?;
            members.insert(*index, member);
            result.skit_strings += strings.len();
            result.skit_relocated += moved;
            result.skits += 1;
        } else {
            let (member, moved) = match world_maps.contains_key(index) {
                true => rebuild_world_map(&member, strings, &name)?,
                false => rebuild_script(&member, strings, &name)?,
            };
            members.insert(*index, member);
            result.strings += strings.len();
            result.relocated += moved;
            result.scripts += 1;
        }
    }
    // The script the program holds itself goes back where it is, in the program's file.
    let mut program_script_refusal = None;
    if !program_script_strings.is_empty() {
        match rebuild_program_script(program_script(&program)?, &program_script_strings) {
            Ok((script, moved)) => {
                patches.push((EXECUTABLE, PROGRAM_SCRIPT as u64, script));
                result.strings += program_script_strings.len();
                result.relocated += moved;
                result.scripts += 1;
            }
            Err(Error::Text(message)) => program_script_refusal = Some(message),
            Err(other) => return Err(other),
        }
    }

    // A string of a menu or of the program goes where the game's own was: in the module and in the
    // module's compressed twin, or in the program's file.
    let mut changed_modules: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut renamed: BTreeMap<usize, BTreeMap<u32, Vec<u8>>> = BTreeMap::new();
    // The name of each renamed enemy's string and its text, to tell a translator of two names for one.
    let mut enemy_texts: BTreeMap<(usize, u32), (String, String)> = BTreeMap::new();
    let mut changed_games: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let program_targets = program_targets(&program);
    // Every string that takes more bytes than it has room for is noted, so that a translator is told
    // of all of them at once: its name, the bytes it takes, and the room it has.
    let mut too_long: Vec<(String, usize, usize)> = Vec::new();
    // The strings of the lists that can move, and those of them the pack changes: each with its
    // name, its offset, the bytes that were its own, and its new bytes.
    let changes_lists = menu_changes.iter().any(|(section, ..)| section == PROGRAM_SECTION);
    let reach = match changes_lists {
        true => program_reach(&program, &program_strings(&program, &own_chars)),
        false => ProgramReach::default(),
    };
    let mut shared: Vec<Waiting> = Vec::new();
    // The same for the menus, by their member: what can move in each, and what the pack changes of it.
    let mut menu_reach: BTreeMap<usize, BTreeMap<u32, Reached>> = BTreeMap::new();
    let mut menu_shared: BTreeMap<usize, Vec<Waiting>> = BTreeMap::new();
    // A party member's name that a new game would copy past the end of its field in the save.
    let party_names = party_name_offsets(&program);
    let mut name_refusals: Vec<String> = Vec::new();
    for (section, offset, text) in &menu_changes {
        let name = string_name(section, *offset);
        let mut data = encode(text, &chars).map_err(|error| match error {
            Error::Text(message) => Error::Text(format!("string {name}: {message}")),
            other => other,
        })?;
        if section == PROGRAM_SECTION && party_names.contains(&Some(*offset)) && data.len() > PARTY_NAME_ROOM {
            name_refusals.push(format!(
                "string {name} is a party member's name, which a new game copies into the member's record in the \
                 save, where a name has room for {PARTY_NAME_ROOM} bytes: this one takes {} (a wide character \
                 takes two bytes where a narrow one takes one), so shorten it",
                data.len()
            ));
            continue;
        }
        if section == PROGRAM_SECTION && *offset == EXIT_PROMPT && !has_code_at(&data, EXIT_PROMPT_CODE, 0x0A) {
            name_refusals.push(format!(
                "string {name} is the prompt to leave a town, and the game writes the town's name into the number \
                 of a {{0A}} at the string's seventh byte, where the game's own has it: start it as the game's own \
                 does, with {{09:35 00 00 00}} {{0A:00 00 00 00}}, and put the words after the name"
            ));
            continue;
        }
        if is_enemy(section) {
            // An enemy's name is in a field of its record, a zero byte ending it.
            if data.contains(&END) {
                return text_error(format!("string {name}: an enemy's name cannot hold a code with a number"));
            }
            // The same name goes on the battle's name plate (see `plate_kinds`), whose field has the
            // same room.
            let room = (ENEMY_NAME_FIELD - 1).min(PLATE_ROOM);
            if data.len() > room {
                too_long.push((name, data.len(), room));
                continue;
            }
            let index = member_of(section).ok_or_else(|| Error::Text(format!("{section} is no pack of enemies")))?;
            enemy_texts.insert((index, *offset), (name, text.clone()));
            renamed.entry(index).or_default().insert(*offset, data);
            result.enemy_names += 1;
            continue;
        }
        let at = *offset as usize;
        if is_game(section) {
            // A string of such a script has the next one right after it, so it has its own bytes and no more.
            let index = member_of(section).ok_or_else(|| Error::Text(format!("{section} is no script")))?;
            let room = game_text_length(&games[&index], at, &own_chars).unwrap_or(0);
            if data.len() > room {
                too_long.push((name, data.len(), room));
                continue;
            }
            data.resize(room, 0);
            let script = changed_games.entry(index).or_insert_with(|| games[&index].clone());
            script[at..at + room].copy_from_slice(&data);
            result.game_strings += 1;
            continue;
        }
        let in_program = section == PROGRAM_SECTION;
        let room = if in_program {
            let length = program_text_length(&program, at, &own_chars).unwrap_or(0);
            match reach.read_straight.contains(offset) {
                true => length, // the code copies that many bytes and the zero after them, and no more
                false => menu_room(&program, at, length, &program_targets),
            }
        } else {
            let (index, about) = sections_of_menus(section)?;
            let original = &modules[&index];
            let length = menu_text_length(original, at, &own_chars, &about).unwrap_or(0);
            menu_room(original, at, length, &pointed_at(original, about.base))
        };
        if in_program && reach.movable.contains_key(offset) {
            shared.push((name, at, room + 1, data));
            continue;
        }
        if !in_program {
            // Only the menus proper: nothing shows what else points at a string of the three
            // larger modules, or what follows them in memory.
            let (index, about) = sections_of_menus(section)?;
            let movable = menu_reach.entry(index).or_insert_with(|| match about.twin {
                Some(_) => module_reach(&modules[&index], &about, &menu_strings(&modules[&index], &about, &own_chars)),
                None => BTreeMap::new(),
            });
            if movable.contains_key(offset) {
                menu_shared.entry(index).or_default().push((name, at, room + 1, data));
                continue;
            }
        }
        if data.len() > room {
            too_long.push((name, data.len(), room));
            continue;
        }
        data.resize(room + 1, 0); // zero bytes up to the end of its room, one of them ending it
        if in_program {
            patches.push((EXECUTABLE, at as u64, data));
            result.list_strings += 1;
        } else {
            let (index, _) = sections_of_menus(section)?;
            let module = changed_modules.entry(index).or_insert_with(|| modules[&index].clone());
            module[at..at + data.len()].copy_from_slice(&data);
            result.menu_strings += 1;
        }
    }
    // The strings of the lists that can move are put into the room they had between them, each
    // stretch of it written out whole. For a string that moved, the words that point at it are
    // changed, and so is the low half in each instruction that builds its address.
    let reached = |at: usize| &reach.movable[&(at as u32)];
    let need = |(_, at, room, data): &Waiting| {
        (*at, *room, data.len() + 1, within_reach(&program, PROGRAM_BASE, *at, &reached(*at).code))
    };
    let needs: Vec<(usize, usize, usize, Range<usize>)> = shared.iter().map(need).collect();
    let (stretches, places, largest) = share_room(&needs, None);
    let mut areas: BTreeMap<usize, Vec<u8>> =
        stretches.iter().map(|stretch| (stretch.start, vec![0; stretch.len()])).collect();
    let mut left: Vec<(String, usize)> = Vec::new();
    for ((name, at, _, data), place) in shared.iter().zip(&places) {
        let Some(place) = *place else {
            left.push((name.clone(), data.len() + 1));
            continue;
        };
        let (start, area) = areas.range_mut(..=place).next_back().expect("a string is put inside a stretch");
        area[place - start..place - start + data.len()].copy_from_slice(data);
        result.list_strings += 1;
        if place != *at {
            let address = place as u32 + PROGRAM_BASE;
            let words = reached(*at).words.iter().map(|word| (*word, address));
            let code = reached(*at).code.iter().filter_map(|instruction| {
                Some((*instruction, number(&program, *instruction)? & 0xFFFF_0000 | address & 0xFFFF))
            });
            patches.extend(words.chain(code).map(|(at, word)| (EXECUTABLE, at as u64, word.to_le_bytes().to_vec())));
            result.list_moved += 1;
        }
    }
    patches.extend(areas.into_iter().map(|(start, area)| (EXECUTABLE, start as u64, area)));
    // The same in each menu's module, which may also grow at its end, up to a size the game is
    // known to load. The module is changed in memory, and written out further on.
    let mut menu_refusals: Vec<String> = Vec::new();
    for (index, strings) in &menu_shared {
        let (original, movable) = (&modules[index], &menu_reach[index]);
        let base = module_of(*index).map_or(MENU_BASE, |about| about.base);
        let need = |(_, at, room, data): &Waiting| {
            (*at, *room, data.len() + 1, within_reach(original, base, *at, &movable[&(*at as u32)].code))
        };
        let needs: Vec<(usize, usize, usize, Range<usize>)> = strings.iter().map(need).collect();
        let spare = original.len()..MENU_MODULE_LIMIT.max(original.len());
        let (stretches, places, largest) = share_room(&needs, Some(spare.clone()));
        let module = changed_modules.entry(*index).or_insert_with(|| original.clone());
        for stretch in stretches.iter().filter(|stretch| stretch.end <= original.len()) {
            module[stretch.clone()].fill(0);
        }
        let mut left: Vec<(String, usize)> = Vec::new();
        for ((name, at, _, data), place) in strings.iter().zip(&places) {
            let Some(place) = *place else {
                left.push((name.clone(), data.len() + 1));
                continue;
            };
            if module.len() < place + data.len() + 1 {
                module.resize(place + data.len() + 1, 0);
            }
            module[place..place + data.len()].copy_from_slice(data);
            result.menu_strings += 1;
            if place != *at {
                let address = place as u32 + base;
                let reached = &movable[&(*at as u32)];
                let words = reached.words.iter().map(|word| (*word, address));
                let code = reached.code.iter().filter_map(|instruction| {
                    Some((*instruction, number(original, *instruction)? & 0xFFFF_0000 | address & 0xFFFF))
                });
                for (at, word) in words.chain(code).collect::<Vec<_>>() {
                    module[at..at + 4].copy_from_slice(&word.to_le_bytes());
                }
                result.menu_moved += 1;
            }
        }
        if !left.is_empty() {
            let (room, need) =
                needs.iter().fold((spare.len(), 0), |(room, need), string| (room + string.1, need + string.2));
            let shared =
                format!("the room they had between them and the {} bytes their module may grow by", spare.len());
            let of = format!("menu {}", section_name(*index));
            menu_refusals.push(no_place_refusal(&of, &shared, &left, (room, need, largest)));
        }
    }
    let (room, need) = needs.iter().fold((0, 0), |(room, need), string| (room + string.1, need + string.2));
    let no_place = (!left.is_empty())
        .then(|| no_place_refusal("the lists", "the room they had between them", &left, (room, need, largest)));
    // A renamed enemy's name goes on the battle's name plate as well, which has one name for each
    // enemy however many packs it is in: two strings that give one enemy two names are refused.
    let mut plates: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    let mut plate_refusals: Vec<String> = Vec::new();
    let plate_member = match renamed.is_empty() {
        true => None,
        false => {
            let (offset, size) = member_span(disc, PLATE_MEMBER)?;
            let member = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
            let broken =
                || Error::Text(format!("member {PLATE_MEMBER} of {ARCHIVE} is not laid out as the plates' list is"));
            let (groups, names) = plate_lists(&member).ok_or_else(broken)?;
            let kinds = plate_kinds(&enemies, &groups, &names)?;
            let mut given: BTreeMap<u8, (usize, u32)> = BTreeMap::new();
            for (pack, strings) in &renamed {
                for (entry, data) in strings {
                    let kind = kinds[&(*pack, *entry)];
                    match given.get(&kind) {
                        Some(first) if plates[&kind] != *data => {
                            let ((one, one_text), (other, other_text)) =
                                (&enemy_texts[first], &enemy_texts[&(*pack, *entry)]);
                            plate_refusals.push(format!(
                                "strings {one} and {other} name the same enemy, whose name plate in battle has one name for \
                                 every pack it is in: they give it \"{one_text}\" and \"{other_text}\""
                            ));
                        }
                        Some(_) => {}
                        None => {
                            given.insert(kind, (*pack, *entry));
                            plates.insert(kind, data.clone());
                        }
                    }
                }
            }
            Some(member)
        }
    };
    let refusals: Vec<String> = skit_refusal(&skit_refused)
        .into_iter()
        .chain(name_refusals)
        .chain(program_script_refusal)
        .chain(refusal_of(&too_long))
        .chain(no_place)
        .chain(menu_refusals)
        .chain(plate_refusals)
        .collect();
    if !refusals.is_empty() {
        return text_error(refusals.join("\n"));
    }
    result.menus = changed_modules.len();
    if switches.narrow_spell_names {
        // The instruction that marks a spell's row of a grid to be drawn large is made to leave the
        // row as it is, in the module and so in its compressed twin.
        for (index, at) in SPELL_FLAGS {
            let module = changed_modules.entry(index).or_insert_with(|| modules[&index].clone());
            if number(module, at) != Some(MARK_LARGE) {
                return text_error(format!(
                    "menu {} does not have, at {at:#X}, the instruction that marks a spell to be drawn large",
                    section_name(index)
                ));
            }
            module[at..at + 4].copy_from_slice(&LEAVE_AS_IT_IS.to_le_bytes());
            result.narrow_spells += 1;
        }
    }
    result.enemy_packs = renamed.len();
    result.games = changed_games.len();
    for (index, script) in changed_games {
        // A script is stored as it is or as one compressed block, and goes back the way it was.
        let (offset, size) = member_span(disc, index)?;
        let stored = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
        let member = match lzss_ring::is_block(&stored) {
            true => lzss_ring::compress_block(&script, lzss_ring::read_header(&stored, 0)?.0)?,
            false => script,
        };
        members.insert(index, member);
    }
    for (index, names) in &renamed {
        let (offset, size) = member_span(disc, *index)?;
        let pack = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
        members.insert(*index, rename_enemies(&pack, names, &section_name(*index))?);
    }
    if let Some(member) = plate_member {
        members.insert(PLATE_MEMBER, rename_plates(&member, &plates)?);
        result.enemy_plates = plates.len();
    }
    // The battle's and the field's modules that changed, by their entry in the member the game loads
    // them from.
    let mut compressed: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for (index, module) in changed_modules {
        if let Some(twin) = module_of(index).and_then(|about| about.twin).map(|after| index + after) {
            let (offset, _) = member_span(disc, twin)?;
            let header = disc.read(ARCHIVE, offset, lzss_ring::HEADER_SIZE as u64)?;
            let (mode, _, _) = lzss_ring::read_header(&header, 0)?;
            members.insert(twin, lzss_ring::compress_block(&module, mode)?);
        }
        // Only a module whose bytes changed is compressed again: the engine's compressor does not
        // give the game's bytes, and a pack that leaves the module as it was must rebuild them.
        let entry = COMPRESSED_ENTRIES.iter().find(|(_, member)| *member == index).map(|(entry, _)| *entry);
        if let Some(entry) = entry
            && module != modules[&index]
        {
            compressed.insert(entry, module.clone());
        }
        members.insert(index, module);
    }
    if !compressed.is_empty() {
        let (offset, size) = member_span(disc, COMPRESSED_MODULES)?;
        let member = disc.read(ARCHIVE, offset, size.max(0) as u64)?;
        members.insert(COMPRESSED_MODULES, rebuild_compressed_modules(&member, &compressed)?);
    }

    let old = table(disc)?;
    let table = new_table(&old, &members)?;
    if table != *old {
        patches.push((EXECUTABLE, TABLE, table.iter().flat_map(|entry| entry.to_le_bytes()).collect()));
    }
    write_image(disc, output, &patches, Some((ARCHIVE, &mut archive(disc, &old, &members))))?;
    let offset = |entry: &u32| entry & !PADDING_BITS;
    let moved = (0..MEMBER_COUNT).filter(|index| offset(&table[*index]) != offset(&old[*index])).count();
    let growth = i64::from(table[MEMBER_COUNT]) - i64::from(old[MEMBER_COUNT]);
    Ok(BuildResult { moved, growth, narrow, wide, ..result })
}

// --- the font

/// The game's main font, expanded once for an image.
fn font(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("tod2-ps2: the font", || read_member(disc, FONT_MEMBER))
}

/// A glyph as rows of 24 pixels, each 0 to 15: its 22 rows, then two empty ones.
pub fn glyph(disc: &Disc, index: usize) -> Result<Vec<Vec<u8>>> {
    let font = font(disc)?;
    let Some(data) = font.get(index * GLYPH_BYTES..(index + 1) * GLYPH_BYTES) else {
        return text_error(format!("the game's font has no glyph {index}"));
    };
    let mut rows = vec![vec![0; GLYPH_SIZE]; GLYPH_SIZE];
    for (y, row) in rows.iter_mut().take(GLYPH_ROWS).enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            let byte = data[y * (GLYPH_SIZE / 2) + x / 2];
            *pixel = if x & 1 == 1 { byte >> 4 } else { byte & 0x0F };
        }
    }
    Ok(rows)
}

// --- what the engine sees

/// Sections are named after the member of the archive their script is, and the skits are all in one
/// stretch of the archive, where no story script is.
fn is_skit(section: &str) -> bool {
    member_of(section).is_some_and(|member| SKIT_MEMBERS.contains(&member))
}

/// The menus, and the three other modules that hold text as theirs do, are one member each.
fn is_menu(section: &str) -> bool {
    member_of(section).and_then(module_of).is_some()
}

/// The packs of enemies are all in one stretch of the archive, where nothing else that holds text is.
fn is_enemy(section: &str) -> bool {
    member_of(section).is_some_and(|member| ENEMY_MEMBERS.contains(&member))
}

/// The scripts of the games within the game are in a stretch of their own, before the story scripts.
fn is_game(section: &str) -> bool {
    member_of(section).is_some_and(|member| GAME_MEMBERS.contains(&member))
}

/// The story scripts, each a member of the archive, and the script the program holds itself.
fn is_dialogue(section: &str) -> bool {
    let other = |member: usize| {
        SKIT_MEMBERS.contains(&member)
            || ENEMY_MEMBERS.contains(&member)
            || GAME_MEMBERS.contains(&member)
            || module_of(member).is_some()
    };
    section == PROGRAM_SCRIPT_SECTION || member_of(section).is_some_and(|member| !other(member))
}

/// The program's own text is one section.
fn is_program(section: &str) -> bool {
    section == PROGRAM_SECTION
}

/// Whether a section's strings are found by what points at them, and numbered by where they are.
fn is_placed(section: &str) -> bool {
    is_menu(section) || is_program(section) || is_enemy(section) || is_game(section)
}

/// The member of the archive that is the module of a section, and what is known of it.
fn sections_of_menus(section: &str) -> Result<(usize, Module)> {
    let found = member_of(section).and_then(|member| Some((member, module_of(member)?)));
    found.ok_or_else(|| Error::Text(format!("{section} is not a part of this game's script")))
}

const SCRIPT_LIMITS: &str = "the text of one script cannot pass 65,535 bytes, and a string that nothing in its \
                             script's code refers to cannot take more bytes than the game's own; the strings of \
                             program-script share the 588 bytes its text has, strings that are the same kept once";

const SKIT_LIMITS: &str = "a line is shown on one row, about 54 narrow letters or 27 wide ones, and a longer one \
                           scrolls sideways while it is typed; the text of one skit cannot pass 65,535 bytes, and a \
                           string that nothing in its script's code refers to cannot take more bytes than the game's own";

const MENU_LIMITS: &str = "the strings a pack changes in one menu share the room they had between them, and a \
                           string that finds no place there goes to the end of the menu's module, which may grow \
                           until it has 63,736 bytes, as the largest has already; the strings of the quiz's, the \
                           battles' and the field's modules stay where they are, each with its own bytes and the \
                           few unused ones after it; a wide character takes two bytes and a narrow one takes one";

const LIST_LIMITS: &str = "the strings a pack changes share the room they had between them, so one may take more \
                           bytes where others take fewer, and a pack that changes one string alone can give it no \
                           more than its own bytes and the few unused ones after it; 18 strings the program reaches \
                           in ways that cannot be followed stay where they are, each with its own bytes; a wide \
                           character takes two bytes and a narrow one takes one";

static KINDS: [TextKind; 6] = [
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        wrap: true,
        limits: SCRIPT_LIMITS,
        trial: Some(("06307.004", "06307.012")),
        ..TextKind::new("dialogue", is_dialogue)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        limits: SKIT_LIMITS,
        trial: Some(("00058.001", "00058.002")),
        ..TextKind::new("skits", is_skit)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        numbered_by_place: true,
        limits: MENU_LIMITS,
        trial: Some(("06816.4136", "06816.4320")),
        ..TextKind::new("menus", is_menu)
    },
    TextKind {
        insert: true,
        grow: true,
        new_characters: true,
        numbered_by_place: true,
        limits: LIST_LIMITS,
        trial: Some(("program.1094168", "program.1042472")),
        ..TextKind::new("lists", is_program)
    },
    TextKind {
        insert: true,
        new_characters: true,
        numbered_by_place: true,
        limits: "a name takes at most 22 bytes: 11 wide characters, or 22 narrow ones",
        trial: Some(("08211.001", "08211.004")),
        ..TextKind::new("enemies", is_enemy)
    },
    TextKind {
        insert: true,
        new_characters: true,
        numbered_by_place: true,
        limits: "a string stays where it is and the next one follows it at once, so it cannot take more bytes \
                 than the game's own; a wide character takes two bytes and a narrow one takes one",
        trial: Some(("06178.17786", "06178.17817")),
        ..TextKind::new("minigames", is_game)
    },
];

/// The adapter itself: what the rest of this module knows, on the contract every game follows.
pub struct Tod2;

pub static ADAPTER: Tod2 = Tod2;

impl Adapter for Tod2 {
    fn id(&self) -> &'static str {
        "tod2-ps2"
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn platform(&self) -> &'static dyn Platform {
        &ps2::PLATFORM
    }

    fn images(&self) -> &'static [Release] {
        // Redump's "Tales of Destiny 2 (Japan, Asia)", disc 15921, whose record gives this SHA-1: the
        // print sold in Asia, SCPS-55050, is the same image, and its SYSTEM.CNF names the same program.
        &[Release {
            sha1: ROM_SHA1,
            name: "Japan, version 1.01",
            size: 3_234_004_992,
            label: "SLPS-25172",
            version: "1.01",
            // redump.org does not answer over https, so its address is http.
            catalogue: Some(CatalogueEntry {
                catalogue: &ps2::REDUMP,
                name: "Tales of Destiny 2 (Japan, Asia)",
                page: Some("http://redump.org/disc/15921/"),
            }),
        }]
    }

    fn refuses(&self) -> &'static [Refused] {
        // The other releases, from Redump and PCSX2's list of games. Each serial is one edition,
        // whatever version its disc gives. None has been taken apart: a reissue's program and
        // archive differ from this one's, so none of the offsets here would hold.
        const fn release(label: &'static str, about: &'static str) -> Refused {
            Refused { label, version: None, about, patched: false }
        }
        const REFUSED: &[Refused] = &[
            release("SLPS-72502", "the Mega Hits! reissue, in Japanese"),
            release("SLPS-73219", "the PlayStation 2 the Best reissue, in Japanese"),
            release("SCAJ-20028", "the release for Asia"),
            release("SCKA-20008", "the release in Korean"),
            release("SCKA-20054", "the PlayStation 2 BigHit Series reissue, in Korean"),
        ];
        REFUSED
    }

    fn source_language(&self) -> &'static str {
        SOURCE_LANGUAGE
    }

    fn fonts(&self) -> &'static [&'static str] {
        FONTS
    }

    fn kinds(&self) -> &'static [TextKind] {
        &KINDS
    }

    fn switches(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                SKIT_PACE,
                "type each changed line of a skit fast enough to be read whole before the game takes it away, and \
                 refuse a line too long for that",
            ),
            (
                NARROW_SPELL_NAMES,
                "draw a spell's name in the grids of artes (the Artes and Enchant screens) in the narrow font a \
                 skill's is drawn in, with room for 17 letters where the game's large letters leave 9",
            ),
        ]
    }

    fn extract(&self, image: &Image) -> Result<Pack> {
        Ok(Pack::of(extract(image.disc()?)?))
    }

    fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
        verify_script(image.disc()?, texts)
    }

    fn write(&self, image: &Image, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
        let switches = Switches {
            skit_pace: options.switches.contains(SKIT_PACE),
            narrow_spell_names: options.switches.contains(NARROW_SPELL_NAMES),
        };
        let result = build_as(image.disc()?, &pack.texts, output, &options.fonts, options.wrap, switches)?;
        Ok(Built { summary: result.summary() })
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

    /// A two-byte code the character list does not cover is written `{G:xxxx}`, four hexadecimal
    /// digits in capitals.
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
