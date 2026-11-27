//! The text the program holds itself, in its file `SLPS_009.44`.
//!
//! The file is a PlayStation program: a header of $800 bytes, then what is loaded at $80010000, so
//! the address of a byte is its offset in the file plus $8000F800. Its code runs from just past the
//! header to $7F4C8 of the file, just past its last return; its data follows.
//!
//! Two routines draw the program's text, and each reads its own encoding.
//!
//! Most of it is written as the dialogue is (see `decode`), and drawn with the dialogue's font. It
//! is section `program`. Nothing lists these strings, but something points at each one: a 32-bit
//! word in a table, which is an address, or the code itself, which builds the address with two
//! instructions (see `mips::code_use`). Many 32-bit words of the program hold such a value and are
//! no pointer at text: a table of shops points at each shop's record, a table of monsters at each
//! monster's, and the first bytes of a record may read as a character or two. So a string is taken
//! to be what is pointed at, in one of the stretches of the data where the text is (see `TEXT`),
//! that reads as text up to a zero byte, draws a character and only characters the table of
//! characters has, and does not start inside the string before it. In those stretches the strings follow one another, each after the zero byte
//! of the one before or after the table that points at them. A string is named by its offset in the
//! file, which no later finding changes.
//!
//! The labels of the main menu and of the screens it opens (items, equipment, chakras, the
//! configuration) are drawn another way, by the routine at $800583C0, from the small font of 8 by 8
//! pixels (member 3 of the archive). They are written in the single bytes of Shift-JIS: ASCII, and
//! the half-width katakana, where a voiced sound is the letter and then ﾞ or ﾟ. An `@` switches
//! between katakana and hiragana, which the small font also has; a label starts in katakana. The
//! small font draws a lowercase letter as its capital. These labels are section `menu`, written
//! in a script as kana of the width the game's own text has, the hiragana as hiragana (see
//! `decode_small`). Most of them sit in arrays of a fixed size per label, which the code indexes, so
//! nothing points at those but the first: a label is what the code builds the address of, outside
//! the stretches of the dialogue's text, that reads as such text with a kana letter in it, and the
//! labels that follow it with nothing but zero bytes between, with a kana letter or an `@` in each.
//! Labels written in ASCII alone are not taken: the same part of the data holds the names of files
//! and the formats of numbers, which look the same.
//!
//! Not covered yet: the battles' scripts, from $98128 of the file to about $9AA00, which a table at
//! $98048 lists (56 of them, the speeches before and after the bosses' fights). Their text is
//! written as the dialogue is, but among commands of another kind whose reader has not been found,
//! so where a string starts and ends in them is not known.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use super::{ByteCodes, PROGRAM, every_character_known, read_text};
use crate::error::{Result, text_error};
use crate::platforms::mips::{code_use, number};
use crate::platforms::ps1::Disc;
use crate::text::{Token, quoted, tag_at};

/// What is added to an offset in the program's file to get the address it is loaded at.
pub const PROGRAM_BASE: u32 = 0x8000_F800;
/// Where the program's code is in its file: from the end of the header to just past the last
/// instruction that returns from a function.
pub const PROGRAM_CODE: Range<usize> = 0x800..0x7_F4C8;
/// The stretches of the program's file where the text written as dialogue is: the kinds of shop;
/// the shopkeepers' lines and the monster book; the souvenirs; and the rest, from the professor's
/// notebook to the rumours. Between them are tables and records, and the menus' labels.
pub const TEXT: [Range<usize>; 4] = [0x9_F2B8..0x9_F2EC, 0xA_0954..0xA_79AD, 0xA_8A60..0xA_8EA2, 0xA_9088..0xB_90D0];
/// The most zero bytes between two labels of one array of the menus. The configuration's labels
/// are 35 bytes apart, the longest 13 of them.
const MENU_GAP: usize = 32;
/// The byte that switches the small font between katakana and hiragana.
const SWITCH: u8 = b'@';

/// The two sections of the program's text.
pub const PROGRAM_SECTION: &str = "program";
pub const MENU_SECTION: &str = "menu";

/// The parts of section `program`, in the order of the file: the first string of each, and what a
/// translator is told there. Each part runs to the next.
pub const PARTS: [(u32, &str); 25] = [
    (0x9_F2B8, "The kinds of shop, as the game names them."),
    (
        0xA_0954,
        "The shopkeepers' lines: each of the 68 shops has a set of nine, one of each kind, and many are in a \
         dialect. A note on each says which kind it is. Lines are at most 13 characters.",
    ),
    (
        0xA_0F4C,
        "The labels of the monster book, one per line, each padded with dots to six characters: type, height, \
         weight, experience, cosmo stones, money, weakness, where it is met.",
    ),
    (
        0xA_2348,
        "The monster book's entries: each monster's type, its height, its weight, its weakness and the rest. \
         ？？？？？？ is what the book shows where it does not say.",
    ),
    (
        0xA_3930,
        "The monster book's descriptions: five lines of 12 characters each. Most monsters have the same \
         description, each its own copy.",
    ),
    (0xA_8A60, "What the souvenirs are: one line each, at most 19 characters."),
    (
        0xA_9088,
        "The professor's notebook and the help pages it opens (psychic powers, the chakras, the game itself). \
         Lines are at most 17 characters.",
    ),
    (0xA_AA34, "The chakra screen's lessons. Lines are at most 16 characters."),
    (0xA_B350, "The names of the party, as a new game gives them."),
    (0xA_B371, "Choices, and messages of the field: something found, money found, a monster appears."),
    (
        0xA_B4AB,
        "The status screen's words: strength, defence, spirit, speed, love, experience, and the figures worked out \
         from them.",
    ),
    (0xA_B567, "The important things: the name of each, then what it is."),
    (0xA_BA50, "The name entry's prompt, the menus of the notebook and of the diary, and the memory card's messages."),
    (0xA_DD04, "The names of items and equipment, at most six characters."),
    (0xA_ED28, "What each item and piece of equipment does: one line each, at most 19 characters."),
    (0xB_0B2C, "The seven chakras, one character each."),
    (0xB_0D2E, "The psychic powers: the names, then what each does, one line of at most 19 characters."),
    (0xB_1D50, "The names of the monsters, at most nine characters."),
    (0xB_2AB0, "The names of places, at most eight characters."),
    (0xB_30C4, "The diary's entries, as the hero writes them. Lines are at most 16 characters."),
    (
        0xB_64C4,
        "The titles of the diary's entries, between 『 and 』. The game's own put a wide space after the \
         seventh character, where the title's line breaks.",
    ),
    (
        0xB_6A9C,
        "The battle's commands and messages. {NAME:19} is the name of whoever acts, {0C:0A} a monster's, \
         {WORD} a thing's or a power's, {NUM} a number.",
    ),
    (0xB_775C, "What monsters do in battle, as the battle's messages say it."),
    (0xB_8894, "How a monster looks at the party, which the game puts before what it does."),
    (0xB_8B68, "The rumours an informant tells. Lines are at most 22 characters."),
];

/// The kinds of a shopkeeper's line, in the order of a shop's record, each with the offset of the
/// first line of that kind: the compiler put the lines of one kind together.
pub const SHOP_LINES: [(u32, &str); 9] = [
    (0xA_0954, "the greeting"),
    (0xA_0A38, "asking what the player will buy"),
    (0xA_0AC2, "asking whether the player will buy more"),
    (0xA_0B53, "the player has not got the money"),
    (0xA_0C0A, "the player cannot carry more"),
    (0xA_0CD5, "asking what the player will sell"),
    (0xA_0D5A, "asking whether the player will sell more"),
    (0xA_0DE7, "there is nothing the player can buy"),
    (0xA_0EAC, "the price, which {0C:10} fills in, and asking whether that is all right"),
];
/// Where the shopkeepers' lines end: the monster book's labels follow.
const SHOP_LINES_END: u32 = 0xA_0F4C;
/// The table of the shops: a pointer at each shop's record, 68 of them. A record is a number, a
/// pointer at the list of what the shop sells, and a pointer at each of its nine lines.
pub const SHOPS: usize = 0x9_F2E4;
pub const SHOP_COUNT: usize = 68;

/// The program's file, read once for an open disc.
pub fn program_file(disc: &Disc) -> Result<Arc<Vec<u8>>> {
    disc.keep("god-ps1 program file", || disc.read(PROGRAM, 0, disc.size(PROGRAM)?))
}

/// The strings of each of the program's sections, each under its offset in the file, without the
/// zero byte that ends it.
pub type ProgramText = BTreeMap<&'static str, BTreeMap<u32, Vec<u8>>>;

/// The program's text, read once for an open disc.
pub fn program_text(disc: &Disc) -> Result<Arc<ProgramText>> {
    disc.keep("god-ps1 program text", || {
        let file = program_file(disc)?;
        let codes = &super::program(disc)?.codes;
        let built: BTreeSet<usize> = built_targets(&file);
        let strings = program_strings(&file, &built, codes);
        let labels = menu_strings(&file, &built);
        Ok(BTreeMap::from([(PROGRAM_SECTION, strings), (MENU_SECTION, labels)]))
    })
}

/// The offset in the file of an address, if the file holds it.
fn offset_of(file: &[u8], address: u32) -> Option<usize> {
    let offset = address.checked_sub(PROGRAM_BASE)? as usize;
    (offset < file.len()).then_some(offset)
}

/// The places in the program's data whose address its code builds, as offsets in its file.
pub fn built_targets(file: &[u8]) -> BTreeSet<usize> {
    let built = code_use(file, PROGRAM_CODE, PROGRAM_BASE).built;
    built.into_keys().filter_map(|address| offset_of(file, address)).filter(|at| *at >= PROGRAM_CODE.end).collect()
}

/// The places in the program's data that one of its 32-bit words points at, as offsets in its file.
pub fn word_targets(file: &[u8]) -> BTreeSet<usize> {
    let words = (0..file.len() / 4).filter_map(|word| number(file, 4 * word));
    words.filter_map(|word| offset_of(file, word)).filter(|at| *at >= PROGRAM_CODE.end).collect()
}

fn in_text(at: usize) -> bool {
    TEXT.iter().any(|stretch| stretch.contains(&at))
}

/// The strings of section `program`: what one of the program's words points at, or what its code
/// builds the address of (`built`), in the stretches of its text, that reads as text, draws a
/// character and only characters the table has, and does not start inside the string before it.
/// The records among the text (a monster's, a shop's) start with numbers, which can read as a code
/// or as a byte that draws nothing; no string of the program has either.
pub fn program_strings(file: &[u8], built: &BTreeSet<usize>, codes: &ByteCodes) -> BTreeMap<u32, Vec<u8>> {
    let mut strings = BTreeMap::new();
    let mut free = 0; // just past the zero byte of the string found last
    let targets: BTreeSet<usize> = word_targets(file).into_iter().chain(built.iter().copied()).collect();
    for at in targets.into_iter().filter(|at| in_text(*at)) {
        if at < free {
            continue;
        }
        let Some((length, characters)) = read_text(&file[at..], codes) else { continue };
        if characters == 0 || !every_character_known(&file[at..at + length], codes) {
            continue;
        }
        strings.insert(at as u32, file[at..at + length].to_vec());
        free = at + length + 1;
    }
    strings
}

/// A byte the small font's routine reads as a character: ASCII but for the braces, which a script
/// keeps for its tags, and the half-width katakana with their punctuation.
fn is_small(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7E if byte != b'{' && byte != b'}') || (0xA1..=0xDF).contains(&byte)
}

/// A half-width katakana letter: one the switch to hiragana changes. ｰ is the same in both.
fn is_kana_letter(byte: u8) -> bool {
    matches!(byte, 0xA6..=0xAF | 0xB1..=0xDD)
}

/// How long the label at the start of `data` is, up to its zero byte, if it is one: bytes the small
/// font's routine reads as characters, with a kana letter among them, or with `@` if `switch` is
/// allowed in place of one.
fn small_length(data: &[u8], switch: bool) -> Option<usize> {
    let length = data.iter().position(|byte| *byte == 0)?;
    let label = &data[..length];
    let letters = label.iter().any(|byte| is_kana_letter(*byte)) || (switch && label.contains(&SWITCH));
    (length > 0 && letters && label.iter().all(|byte| is_small(*byte))).then_some(length)
}

/// The labels of section `menu`: what the program's code builds the address of (`built`), in its
/// data outside the stretches of the dialogue's text, that reads as a label with a kana letter in
/// it, and the labels that follow such a one with nothing but zero bytes between, each with a kana
/// letter or an `@`. The labels of one screen are an array, each in a slot of the same size, and
/// the code builds the address of the array's first alone.
pub fn menu_strings(file: &[u8], built: &BTreeSet<usize>) -> BTreeMap<u32, Vec<u8>> {
    let mut labels = BTreeMap::new();
    let mut free = 0;
    for start in built.iter().copied().filter(|at| !in_text(*at)) {
        if start < free {
            continue;
        }
        let Some(length) = small_length(&file[start..], false) else { continue };
        labels.insert(start as u32, file[start..start + length].to_vec());
        let mut end = start + length + 1;
        while let Some(gap) = file[end..].iter().position(|byte| *byte != 0) {
            let next = end + gap;
            if gap > MENU_GAP || in_text(next) {
                break;
            }
            let Some(length) = small_length(&file[next..], true) else { break };
            labels.insert(next as u32, file[next..next + length].to_vec());
            end = next + length + 1;
        }
        free = end;
    }
    labels
}

// --- the small font's labels ---------------------------------------------------

/// The characters of the half-width bytes from $A1 to $DF, as the game's own text writes them.
const HALF_WIDTH: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン゛゜";
const FIRST_HALF_WIDTH: u8 = 0xA1;
const VOICED: u8 = 0xDE;
const SEMI_VOICED: u8 = 0xDF;
/// The tag for an `@` that a script cannot show as hiragana: one that comes before something other
/// than a kana letter.
const SWITCH_TAG: &str = "KANA";

fn half_width(byte: u8) -> Option<char> {
    HALF_WIDTH.chars().nth(usize::from(byte.checked_sub(FIRST_HALF_WIDTH)?))
}

fn half_width_byte(char: char) -> Option<u8> {
    HALF_WIDTH.chars().position(|known| known == char).map(|index| FIRST_HALF_WIDTH + index as u8)
}

/// The katakana a letter and ﾞ make, if they make one.
fn voiced(char: char) -> Option<char> {
    match char {
        'カ'..='ト' if "カキクケコサシスセソタチツテト".contains(char) => {
            char::from_u32(char as u32 + 1)
        }
        'ハ'..='ホ' if "ハヒフヘホ".contains(char) => char::from_u32(char as u32 + 1),
        'ウ' => Some('ヴ'),
        _ => None,
    }
}

/// The katakana a letter and ﾟ make, if they make one.
fn semi_voiced(char: char) -> Option<char> {
    "ハヒフヘホ".contains(char).then(|| char::from_u32(char as u32 + 2)).flatten()
}

/// A katakana letter and the mark it was made with, if it was: ガ is カ and ﾞ.
fn split_voiced(char: char) -> (char, Option<u8>) {
    let before = |offset: u32| (char as u32).checked_sub(offset).and_then(char::from_u32).unwrap_or(char);
    match char {
        'ヴ' => ('ウ', Some(VOICED)),
        _ if voiced(before(1)) == Some(char) => (before(1), Some(VOICED)),
        _ if semi_voiced(before(2)) == Some(char) => (before(2), Some(SEMI_VOICED)),
        _ => (char, None),
    }
}

const KATAKANA: std::ops::RangeInclusive<char> = 'ァ'..='ヴ';
const HIRAGANA: std::ops::RangeInclusive<char> = 'ぁ'..='ゔ';
/// How far apart a katakana and its hiragana are in Unicode.
const TO_HIRAGANA: u32 = 0x60;

/// A label of the small font, without its zero byte, as script text: half-width katakana as the
/// katakana of the game's own text, a letter and its ﾞ or ﾟ as one character, and what follows an
/// `@` as hiragana until the next. An `@` before anything but a kana letter is `{KANA}`.
pub fn decode_small(data: &[u8]) -> String {
    let mut out = String::new();
    let mut hiragana = false;
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        let next = data.get(at + 1).copied();
        at += 1;
        if byte == SWITCH {
            if !next.is_some_and(is_kana_letter) {
                out.push_str(&format!("{{{SWITCH_TAG}}}"));
            }
            hiragana = !hiragana;
        } else if is_kana_letter(byte) {
            let letter = half_width(byte).unwrap_or('?');
            let made = match next {
                Some(VOICED) => voiced(letter),
                Some(SEMI_VOICED) => semi_voiced(letter),
                _ => None,
            };
            at += usize::from(made.is_some());
            let letter = made.unwrap_or(letter);
            let shown = if hiragana { char::from_u32(letter as u32 - TO_HIRAGANA) } else { Some(letter) };
            out.push(shown.unwrap_or(letter));
        } else if let Some(char) = half_width(byte) {
            out.push(char);
        } else if is_small(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("{{B:{byte:02X}}}"));
        }
    }
    out
}

/// Script text as the bytes of a label of the small font, without its zero byte.
pub fn encode_small(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut hiragana = false;
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        if char == '{' {
            let Some(Token::Tag { raw, name, value }) = tag_at(rest) else {
                let shown: String = rest.chars().take(16).collect();
                return text_error(format!("malformed tag: {}", quoted(&shown)));
            };
            match (name, value.and_then(crate::text::from_hex).as_deref()) {
                (SWITCH_TAG, None) if value.is_none() => {
                    out.push(SWITCH);
                    hiragana = !hiragana;
                }
                ("B", Some([byte])) if !is_small(*byte) && *byte != 0 => out.push(*byte),
                _ => return text_error(format!("unknown tag {raw} in a label of the small font")),
            }
            rest = &rest[raw.len()..];
            continue;
        }
        let (katakana, wanted) = match char {
            _ if HIRAGANA.contains(&char) => (char::from_u32(char as u32 + TO_HIRAGANA).unwrap_or(char), Some(true)),
            _ if KATAKANA.contains(&char) => (char, Some(false)),
            _ => (char, None),
        };
        let (letter, mark) = split_voiced(katakana);
        let byte = match half_width_byte(letter) {
            Some(byte) => byte,
            None if char.is_ascii() && is_small(char as u8) && char as u8 != SWITCH => char as u8,
            None if char as u32 == u32::from(SWITCH) => {
                return text_error(format!(
                    "{} is not drawn in a label of the small font: {{{SWITCH_TAG}}} switches between katakana and \
                     hiragana",
                    quoted("@")
                ));
            }
            None => {
                return text_error(format!(
                    "character {} is not in the small font, which has ASCII and the kana",
                    quoted(&char.to_string())
                ));
            }
        };
        if let Some(wanted) = wanted.filter(|wanted| is_kana_letter(byte) && *wanted != hiragana) {
            out.push(SWITCH);
            hiragana = wanted;
        }
        out.push(byte);
        out.extend(mark);
        rest = &rest[char.len_utf8()..];
    }
    Ok(out)
}

// --- what a translator is told ---------------------------------------------------

/// What a translator is told about the program's text: with no number, about a section's file, and
/// with one, about a string.
pub fn note(section: &str, number: Option<u32>) -> Option<String> {
    match (section, number) {
        (PROGRAM_SECTION, None) => Some(
            "The program's own text: what its lists and screens show, written as the dialogue is. A string is \
             named by where it is in the program's file, so the numbers have gaps. A note under the first string \
             of each part says what the part is and how wide the game's own lines are."
                .to_string(),
        ),
        (PROGRAM_SECTION, Some(number)) => {
            let part = PARTS.iter().find(|(start, _)| *start == number).map(|(_, about)| about.to_string());
            let shop = (SHOP_LINES[0].0..SHOP_LINES_END).contains(&number).then(|| {
                let (_, kind) = SHOP_LINES.iter().rev().find(|(start, _)| *start <= number).unwrap_or(&SHOP_LINES[0]);
                format!("A shopkeeper's line: {kind}.")
            });
            match (part, shop) {
                (Some(part), Some(shop)) => Some(format!("{part} {shop}")),
                (part, shop) => part.or(shop),
            }
        }
        (MENU_SECTION, None) => Some(
            "The labels of the main menu and of its screens, drawn from the small font of 8 by 8 pixels. It has \
             capitals, digits, some ASCII punctuation and the kana, and draws a lowercase letter as its capital; \
             none of the dialogue's tags work here. Hiragana and katakana are written as they show; {KANA} is a \
             switch between the two with no kana after it. Most labels sit in a slot of a fixed size, with the \
             next label of the screen at the next slot: the game's own leave the bytes up to the next label \
             unused, and a label cannot take more. The dots after a configuration label lead to its setting."
                .to_string(),
        ),
        _ => None,
    }
}
