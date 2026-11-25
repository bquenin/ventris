use sha1::{Digest, Sha1};

use crate::adapter::declaration_faults;

use super::*;

fn hex(bytes: &str) -> Vec<u8> {
    from_hex(bytes).unwrap()
}

/// A character list like the game's: glyph 0 a blank, a character that appears twice, a blank
/// again. It is "　あい漢あ　ー" in Shift-JIS.
fn chars() -> Characters {
    Characters::from_list(&hex("81 40 82 a0 82 a2 8a bf 82 a0 81 40 81 5b"))
}

fn decoded(bytes: &str) -> String {
    decode(&hex(bytes), &chars())
}

fn encoded(text: &str) -> Result<Vec<u8>> {
    encode(text, &chars())
}

/// What a translator is told when something is refused.
fn message<T>(result: Result<T>) -> String {
    match result {
        Err(error @ Error::Text(_)) => error.to_string(),
        Err(other) => panic!("refused, but not in words for a translator: {other:?}"),
        Ok(_) => panic!("it was not refused"),
    }
}

/// The game's disc, opened, or None, with a word about why, where it is not there to test with.
fn game() -> Option<Disc> {
    Some(Disc::open(&crate::testing::game("tod2-ps2")?).unwrap())
}

fn script_of(pairs: &[(&str, u32, &str)]) -> Script {
    let mut script = Script::new();
    for (section, number, text) in pairs {
        script.entry(section.to_string()).or_default().insert(*number, text.to_string());
    }
    script
}

fn sha1_of(data: &[u8]) -> String {
    Sha1::digest(data).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn codes_skip_two_second_bytes_and_the_half_width_range() {
    assert_eq!(glyph_of(0x9940), Some(0));
    assert_eq!((glyph_of(0x995B), glyph_of(0x995D)), (Some(27), Some(28))); // no $5C
    assert_eq!((glyph_of(0x997E), glyph_of(0x9980)), (Some(61), Some(62))); // no $7F
    assert_eq!((glyph_of(0x99FC), glyph_of(0x9A40)), (Some(186), Some(187)));
    // Lead bytes $A0-$DF are single characters.
    assert_eq!((glyph_of(0x9FFC), glyph_of(0xE040)), (Some(1308), Some(1309)));
    assert_eq!((glyph_of(0x995C), glyph_of(0x993F), glyph_of(0xA040), glyph_of(0x8140)), (None, None, None, None));
    assert_eq!(code_at(2559), 0xE6C2);
    for glyph in 0..GLYPH_COUNT {
        assert_eq!(glyph_of(code_at(glyph) as u16), Some(glyph));
        assert!(!SKIPPED_TRAILS.contains(&(code_at(glyph) as u8)));
    }
}

#[test]
fn a_character_is_written_with_the_first_glyph_that_has_it() {
    let chars = chars();
    assert_eq!(chars.by_glyph, ['　', 'あ', 'い', '漢', 'あ', '　', 'ー'].map(Some));
    assert_eq!(chars.glyphs, HashMap::from([('　', 0), ('あ', 1), ('い', 2), ('漢', 3), ('ー', 6)]));
    // The second glyph for a character keeps its code; a code past the end of the list does too.
    assert_eq!(decoded("99 41 99 44 99 45 99 50"), "あ{G:9944}{G:9945}{G:9950}");
    assert_eq!(encoded("あ{G:9944}{G:9945}{G:9950}").unwrap(), hex("99 41 99 44 99 45 99 50"));
    // Bytes of the list that are not one character, or are one the script writes another way,
    // name no glyph: a pair of ASCII letters, a brace, a half-width kana.
    let odd = Characters::from_list(&hex("41 42 81 6f 83 41 b1 b2 82 a0 99"));
    assert_eq!(odd.by_glyph, [None, Some('｛'), Some('ア'), None, Some('あ')]);
    assert_eq!(odd.glyphs, HashMap::from([('｛', 1), ('ア', 2), ('あ', 4)]));
}

#[test]
fn decode_and_encode_a_string() {
    let data = hex("99 41 99 42 01 07 01 00 00 00 21 a5 a5 99 43 20 4f 6b 02");
    let text = "あい\n{NAME:01 00 00 00}!･･漢 Ok{02}";
    assert_eq!(decode(&data, &chars()), text);
    assert_eq!(encoded(text).unwrap(), data);
    // A code's number is accepted for its name.
    assert_eq!(encoded(&text.replace("{NAME:", "{07:")).unwrap(), data);
    // A character a build has given a single byte to is written with it.
    let mut with_single = chars();
    with_single.singles.insert('é', 0xA6);
    assert_eq!(encode("aéあ", &with_single).unwrap(), hex("61 a6 99 41"));
}

#[test]
fn an_expression_is_read_token_by_token() {
    // A 16-bit number holding a zero byte, a variable, an operator, then the end.
    assert_eq!(decoded("16 a0 00 08 24 72 ca c0"), "{16:A0 00 08 24 72 CA}");
    assert_eq!(encoded("{16:A0 00 08 24 72 CA}").unwrap(), hex("16 a0 00 08 24 72 ca c0"));
    // A variable with a 16-bit address, and one whose address byte is the byte that ends an expression.
    for raw in ["16 28 56 08 c0", "14 34 c0 c0", "18 44 8c 81 35 60 ca c6 c0", "14 21 56 90 3c c9 c0", "12 c0"] {
        assert_eq!(encoded(&decoded(raw)).unwrap(), hex(raw), "{raw}");
    }
    assert_eq!(decoded("14 34 c0 c0"), "{14:34 C0}");
    assert_eq!(decoded("12 c0"), "{12:}"); // an expression of nothing is not the byte on its own
}

#[test]
fn strings_are_split_past_zero_bytes_that_do_not_end_them() {
    let text = hex("99 41 00 16 a0 00 08 c0 99 42 00 07 00 00 00 00 00 00 0b 01 00 00 00 14 34 08 c0 00");
    assert_eq!(
        split_strings(&text),
        [hex("99 41"), hex("16 a0 00 08 c0 99 42"), hex("07 00 00 00 00"), hex(""), hex("0b 01 00 00 00 14 34 08 c0")]
    );
    assert!(split_strings(b"").is_empty());
    assert_eq!(split_strings(b"abc"), [b"abc"]); // text that does not end in zero keeps its last string
}

#[test]
fn bytes_that_fit_nothing_are_kept() {
    for raw in ["99", "99 3f", "99 5c", "07 01 02", "16 a0 01", "16", "83 6f", "f8 c0 e0", "7b 7d"] {
        assert_eq!(encoded(&decoded(raw)).unwrap(), hex(raw), "{raw}");
    }
    assert_eq!(decoded("99 5c"), "{99}\\"); // $5C is never a second byte
    assert_eq!(decoded("07 01 02"), "{07}\n{02}"); // too short to be a code and its number
    assert_eq!(decoded("16 a0 01"), "{16}{A0}\n"); // an expression that never ends
    assert_eq!(decode(b"{}", &chars()), "{7B}{7D}"); // braces would read as a tag
}

#[test]
fn text_the_game_cannot_hold_is_refused() {
    for (text, expected) in [
        ("é", "the game's font has no 'é'"),
        ("漢字", "the game's font has no '字'"),
        ("{ZZ}", "unknown tag {ZZ}"),
        ("{00}", "unknown tag {00}"),
        ("{G:9900}", "unknown tag {G:9900}"),
        ("{G:41}", "unknown tag {G:41}"),
        ("{G}", "unknown tag {G}"),
        ("{7}", "unknown tag {7}"),
        ("{100}", "unknown tag {100}"),
        ("{07:0}", "unknown tag {07:0}"),
        // A code's number is two hexadecimal digits and nothing else; the Python reference takes these.
        ("{+F}", "unknown tag {+F}"),
        ("{１２}", "unknown tag {１２}"),
        ("{NAME:01}", "{NAME:01} needs 4 parameter bytes"),
        ("{21:00}", "{21:00} needs 0 parameter bytes"),
        ("{16:A0 00}", "{16:A0 00} is not a whole expression"),
        ("{16:34 08 C0}", "{16:34 08 C0} is not a whole expression"),
    ] {
        assert_eq!(message(encoded(text)), expected);
    }
}

/// A story script member like the game's: a container of two entries, the last being the script.
fn member(strings: &[&[u8]], code: &[u8]) -> Vec<u8> {
    let text: Vec<u8> = strings.iter().flat_map(|string| [string, [0].as_slice()].concat()).collect();
    let starts = [12u32, 12 + code.len() as u32].map(u32::to_le_bytes).concat();
    let sced = [b"SCED".as_slice(), &starts, code, &text].concat();
    let entries = [b"another entry".to_vec(), lzss_ring::compress_block(&sced, lzss_ring::MODE_LZSS_RLE).unwrap()];
    let header = [1u16.to_le_bytes(), 7u16.to_le_bytes()].concat();
    let counts = [2u32, 0, entries[0].len() as u32, entries[1].len() as u32].map(u32::to_le_bytes).concat();
    [b"SCPK".as_slice(), &header, &counts, &entries[0], &entries[1]].concat()
}

/// The code and the text of the script in a member.
fn script_in(member: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let first = number(member, 16).unwrap() as usize;
    let sced = lzss_ring::decompress_block(member, 24 + first).unwrap();
    let (code_start, text_start) = (number(&sced, 4).unwrap() as usize, number(&sced, 8).unwrap() as usize);
    (sced[code_start..text_start].to_vec(), sced[text_start..].to_vec())
}

const ONE_REFERENCE: [u8; 4] = [0xF8, 0x00, 0x00, 0xF1];

#[test]
fn a_string_that_fits_is_replaced_in_the_place_it_had() {
    let strings = [hex("99 41 99 42"), hex("99 43 99 43 99 43"), b"last".to_vec()];
    let strings: Vec<&[u8]> = strings.iter().map(Vec::as_slice).collect();
    let original = member(&strings, &ONE_REFERENCE);
    let replacements = BTreeMap::from([(1, hex("99 41 21")), (2, b"LAST".to_vec())]);
    let (rebuilt, moved) = rebuild_script(&original, &replacements, "00042").unwrap();
    assert_eq!(moved, 0);
    assert_eq!(rebuilt[..16], original[..16]);
    let (first, last) = (number(&rebuilt, 16).unwrap() as usize, number(&rebuilt, 20).unwrap() as usize);
    assert_eq!(&rebuilt[24..24 + first], b"another entry");
    assert_eq!(rebuilt.len(), 24 + first + last);
    let (code, text) = script_in(&rebuilt);
    assert_eq!(code, ONE_REFERENCE);
    // The shorter string is followed by zero bytes up to where the next one starts, which has not moved.
    assert_eq!(text, [hex("99 41 99 42 00 99 41 21 00 00 00 00"), b"LAST\x00".to_vec()].concat());
    assert_eq!(split_strings(&text), [strings[0], &hex("99 41 21"), b"", b"", b"", b"LAST"]);
    // A pack that changes nothing of a script gives its text back as it was.
    let (same, moved) = rebuild_script(&original, &BTreeMap::new(), "00042").unwrap();
    assert_eq!((script_in(&same), moved), (script_in(&original), 0));
}

#[test]
fn code_is_read_instruction_by_instruction_to_find_references() {
    // Two references, a jump, a 32-bit number, and a variable whose address byte is $F8 and is
    // followed by bytes that look like a string's offset.
    let code = hex("f8 00 00 f8 05 00 f2 06 00 b0 f8 05 00 00 24 f8 05 00 e0 3d c0 f0 f6 01 fe f1");
    assert_eq!(string_references(&code).unwrap(), BTreeMap::from([(0, vec![1]), (5, vec![4])]));
    assert_eq!(string_references(&hex("f8 05 00 f8 05 00 28 f8 05 c0")).unwrap(), BTreeMap::from([(5, vec![1, 4])]));
    assert_eq!(
        message(string_references(&hex("f8 00 00 f7"))),
        "the script's code has an instruction that is not known, $F7"
    );
    assert_eq!(
        message(string_references(&hex("f8 00 00 f2 06"))),
        "the script's code does not end where its text starts"
    );
}

#[test]
fn a_string_that_outgrows_its_place_moves_and_its_references_follow() {
    let strings = [hex("99 41 99 42"), hex("99 43 99 43 99 43"), b"last".to_vec()];
    let strings: Vec<&[u8]> = strings.iter().map(Vec::as_slice).collect();
    let code = hex("f8 00 00 f8 05 00 24 f8 05 00 c0 f1");
    let longer = BTreeMap::from([(0, hex("99 41 99 42 99 43")), (1, hex("99 43 99 43 99 43 99 41"))]);
    let (rebuilt, moved) = rebuild_script(&member(&strings, &code), &longer, "00042").unwrap();
    assert_eq!(moved, 2);
    let (new_code, text) = script_in(&rebuilt);
    // The longer of the two takes the room both left, which is one stretch; the other goes to the end.
    assert_eq!(text, [longer[&1].as_slice(), &[0; 4], b"last\x00", &longer[&0], &[0]].concat());
    // Both references follow. The $F8 inside the variable is left alone.
    assert_eq!(new_code, hex("f8 11 00 f8 00 00 24 f8 05 00 c0 f1"));
    // A string the code never refers to can change in place but cannot move.
    let unreferenced = BTreeMap::from([(2, b"longer".to_vec())]);
    assert_eq!(
        message(rebuild_script(&member(&strings, &code), &unreferenced, "00042")),
        "string 00042.002 takes 6 bytes where the game's has 4, and it cannot move: nothing in the script's code \
         refers to it"
    );
    // References are 16 bits.
    let large = member(&[b"a", &b"x".repeat(70000)], &ONE_REFERENCE);
    assert_eq!(
        message(rebuild_script(&large, &BTreeMap::from([(0, b"abc".to_vec())]), "00042")),
        "string 00042.000 does not fit: the text of script 00042 would pass 65,535 bytes, the furthest the code \
         can refer to"
    );
    // A last string with no zero byte after it leaves room that ends in one.
    let mut open = member(&[b"ab", b"cd"], &hex("f8 00 00 f8 03 00 f1"));
    let (code, text) = script_in(&open);
    let cut = [b"SCED".as_slice(), &12u32.to_le_bytes(), &19u32.to_le_bytes(), &code, &text[..5]].concat();
    let packed = lzss_ring::compress_block(&cut, lzss_ring::MODE_LZSS).unwrap();
    open.truncate(24 + 13);
    open[20..24].copy_from_slice(&(packed.len() as u32).to_le_bytes());
    open.extend(&packed);
    let (rebuilt, moved) = rebuild_script(&open, &BTreeMap::from([(1, b"cdefgh".to_vec())]), "00042").unwrap();
    assert_eq!((script_in(&rebuilt), moved), ((hex("f8 00 00 f8 06 00 f1"), b"ab\0\0\0\0cdefgh\0".to_vec()), 1));
    assert_eq!(rebuilt[24 + 13], lzss_ring::MODE_LZSS); // packed again the way it was
}

#[test]
fn a_member_that_is_no_script_is_refused() {
    let good = member(&[b"ab"], &ONE_REFERENCE);
    for broken in [&good[..10], &good[..22], &[&good[..8], [0; 8].as_slice()].concat()] {
        assert_eq!(
            message(rebuild_script(broken, &BTreeMap::new(), "00042")),
            "script 00042 is not laid out as a story script is"
        );
    }
    assert!(matches!(rebuild_script(&good[..good.len() - 3], &BTreeMap::new(), "00042"), Err(Error::Codec(_))));
}

/// A skit member like the game's: a compressed pack of six entries, the third being the script.
fn skit(strings: &[&[u8]], code: &[u8]) -> Vec<u8> {
    let text: Vec<u8> = strings.iter().flat_map(|string| [string, [0].as_slice()].concat()).collect();
    let starts = [12u32, 12 + code.len() as u32].map(u32::to_le_bytes).concat();
    let sced = [b"SCED".as_slice(), &starts, code, &text].concat();
    let entries: [&[u8]; 6] = [b"who", b"when each one speaks", &sced, b"a picture", &[7; 32], b"faces"];
    lzss_ring::compress_block(&pack_of(&entries), lzss_ring::MODE_LZSS_RLE).unwrap()
}

/// Where the entries of a skit member are, and what they hold.
fn entries_of(member: &[u8]) -> (Vec<(usize, usize)>, Vec<Vec<u8>>) {
    let pack = lzss_ring::decompress_block(member, 0).unwrap();
    let places = pack_entries(&pack).unwrap();
    let entries = places.iter().map(|(at, size)| pack[*at..*at + *size].to_vec()).collect();
    (places, entries)
}

#[test]
fn a_pack_is_a_table_then_entries_on_multiples_of_sixteen() {
    let pack = pack_of(&[b"abc", b"", &[9; 17], b"z"]);
    // The table of four entries ends at 36, so the first entry starts at 48.
    assert_eq!(pack_entries(&pack).unwrap(), [(48, 3), (64, 0), (64, 17), (96, 1)]);
    assert_eq!(pack.len(), 97);
    assert_eq!(pack[36..64], [[0; 12].as_slice(), b"abc", &[0; 13]].concat());
    assert_eq!(pack_entries(&pack[..30]), None);
}

#[test]
fn a_skit_s_script_is_changed_inside_its_pack() {
    let strings: [&[u8]; 3] = [b"", &hex("99 41 99 42"), b"last"];
    let code = hex("f8 00 00 f8 01 00 f8 06 00 f1");
    let original = skit(&strings, &code);
    let (old_places, old) = entries_of(&original);

    // A string that fits goes where the old one was, and nothing in the pack moves.
    let (rebuilt, moved) = rebuild_skit(&original, &BTreeMap::from([(1, hex("99 43"))]), &[], "00054").unwrap();
    let (places, new) = entries_of(&rebuilt);
    assert_eq!((moved, &places), (0, &old_places));
    assert_eq!(text_of(&new[SKIT_SCRIPT]), b"\0\x99\x43\0\0\0last\0");
    assert!((0..SKIT_ENTRIES).all(|entry| entry == SKIT_SCRIPT || new[entry] == old[entry]));

    // One that outgrows its place goes to the end of the text, and its reference follows. The
    // script is longer, so the entries after it start later; what they hold is as it was.
    let longer = BTreeMap::from([(1, hex("99 41").repeat(20))]);
    let (rebuilt, moved) = rebuild_skit(&original, &longer, &[], "00054").unwrap();
    let (places, new) = entries_of(&rebuilt);
    assert_eq!(moved, 1);
    assert_eq!(text_of(&new[SKIT_SCRIPT]), [b"\0\0\0\0\0\0last\0".as_slice(), &longer[&1], &[0]].concat());
    assert_eq!(new[SKIT_SCRIPT][12..22], hex("f8 00 00 f8 0b 00 f8 06 00 f1"));
    assert!((0..SKIT_ENTRIES).all(|entry| entry == SKIT_SCRIPT || new[entry] == old[entry]));
    assert_eq!(places[..SKIT_SCRIPT], old_places[..SKIT_SCRIPT]);
    assert_eq!(places[SKIT_SCRIPT], (old_places[SKIT_SCRIPT].0, old_places[SKIT_SCRIPT].1 + 41));
    for entry in SKIT_SCRIPT + 1..SKIT_ENTRIES {
        // The script ended 15 bytes short of the next multiple of 16 and is now 41 longer.
        assert_eq!(places[entry], (old_places[entry].0 + 32, old_places[entry].1));
    }
    assert_eq!(rebuilt[0], lzss_ring::MODE_LZSS_RLE); // packed again the way it was

    // The same rules as in a story script hold for what cannot be done.
    assert_eq!(
        message(rebuild_skit(&skit(&strings, &hex("f8 00 00 f1")), &longer, &[], "00054")),
        "string 00054.001 takes 40 bytes where the game's has 4, and it cannot move: nothing in the script's code \
         refers to it"
    );
    let other = lzss_ring::compress_block(b"a block that is no pack", lzss_ring::MODE_LZSS).unwrap();
    assert_eq!(
        message(rebuild_skit(&other, &BTreeMap::new(), &[], "00054")),
        "skit 00054 is not laid out as a skit is"
    );
}

/// The code of a skit like the game's: for each line its start on the skit's clock, its pace as
/// the script writes it, and its string's offset in the text, each line in a block of its own.
fn skit_code(lines: &[(u32, &str, u16)]) -> Vec<u8> {
    let plain = |value: u32| plain_number_of(value, [1, 2, 3][usize::from(value >= 16) + usize::from(value >= 4096)]);
    let mut code = Vec::new();
    for (start, pace, string) in lines {
        code.push(0xFE);
        code.extend(plain(*start).unwrap());
        code.extend(hex("ce c0 f3 00 00 fe"));
        code.extend(plain(start + 3).unwrap());
        code.extend(hex("cf c0 f3 00 00 83 81 80"));
        code.extend(hex(pace));
        code.push(STRING_REFERENCE);
        code.extend(string.to_le_bytes());
        code.extend(hex("f5 fa 01 c0"));
    }
    code.push(0xF0);
    code
}

/// A skit member like the game's whose voice, and so whose lips, last `frames` frames.
fn timed_skit(strings: &[&[u8]], code: &[u8], frames: u32) -> Vec<u8> {
    let text: Vec<u8> = strings.iter().flat_map(|string| [string, [0].as_slice()].concat()).collect();
    let starts = [12u32, 12 + code.len() as u32].map(u32::to_le_bytes).concat();
    let sced = [b"SCED".as_slice(), &starts, code, &text].concat();
    let lips = [(frames / 2).to_le_bytes().as_slice(), &vec![0x11; (frames / 2) as usize]].concat();
    let entries: [&[u8]; 6] = [b"who", &lips, &sced, b"a picture", &[7; 32], b"faces"];
    lzss_ring::compress_block(&pack_of(&entries), lzss_ring::MODE_LZSS_RLE).unwrap()
}

/// The lines of skit 00058 as the game has them: when each starts, its pace, its string's offset.
const LINES_OF_00058: [(u32, &str, u16); 5] =
    [(1, "90 08", 1), (250, "86", 3), (520, "90 08", 5), (1160, "90 08", 7), (1310, "86", 9)];

#[test]
fn a_skit_s_lines_are_read_from_its_code() {
    let code = skit_code(&LINES_OF_00058);
    let script = [b"SCED".as_slice(), &[12, 0, 0, 0], &(12 + code.len() as u32).to_le_bytes(), &code].concat();
    let lines = skit_lines(&script, 1454).unwrap();
    let read: Vec<(usize, Option<u32>, u32, Option<u32>)> =
        lines.iter().map(|line| (line.string, line.start, line.removed, line.pace)).collect();
    // Each line is taken away when the line two after it starts, the last two when the skit ends.
    assert_eq!(
        read,
        [
            (1, Some(1), 520, Some(8)),
            (3, Some(250), 1160, Some(6)),
            (5, Some(520), 1310, Some(8)),
            (7, Some(1160), 1454, Some(8)),
            (9, Some(1310), 1454, Some(6)),
        ]
    );
    // Where the pace is: the number before the string, two bytes or one.
    for line in &lines {
        let written = &script[line.pace_at..line.pace_at + line.pace_size];
        assert_eq!(plain_number(written), line.pace);
        assert_eq!(script[line.pace_at + line.pace_size], STRING_REFERENCE);
    }
    assert_eq!(lines.iter().map(|line| line.pace_size).collect::<Vec<_>>(), [2, 1, 2, 2, 1]);
    // A line whose block does not test the clock with "at least" has no start, and the lines
    // around it are timed among themselves.
    let odd = [hex("fe 81 d0 c0 f3 00 00 83 81 80 83 f8 01 00 f5 fa 01 c0"), skit_code(&LINES_OF_00058[1..])].concat();
    let script = [b"SCED".as_slice(), &[12, 0, 0, 0], &(12 + odd.len() as u32).to_le_bytes(), &odd].concat();
    let lines = skit_lines(&script, 1454).unwrap();
    assert_eq!((lines[0].start, lines[0].pace), (None, Some(3)));
    assert_eq!(lines[1..].iter().map(|line| line.removed).collect::<Vec<_>>(), [1160, 1310, 1454, 1454]);
}

#[test]
fn a_number_keeps_its_bytes_when_it_changes() {
    for (bytes, value) in [("86", 6), ("90 08", 8), ("9f ff", 4095), ("a1 02 03", 0x10302)] {
        assert_eq!(plain_number(&hex(bytes)), Some(value), "{bytes}");
        assert_eq!(plain_number_of(value, hex(bytes).len()), Some(hex(bytes)), "{bytes}");
    }
    assert_eq!(plain_number_of(5, 2), Some(hex("90 05")));
    assert_eq!(plain_number_of(16, 1), None);
    assert_eq!(plain_number(&hex("38 40 0c")), None); // a variable, not a number
}

#[test]
fn a_skit_s_letters_are_counted_as_the_game_draws_them() {
    assert_eq!(skit_letters("Hmm... Is that so."), Ok(18));
    assert_eq!(skit_letters("ふぅん‥‥そうなのか。"), Ok(11));
    // A party member's name as six letters, whichever way it is written; a button and a byte one.
    assert_eq!(skit_letters("Hey, {NAME:01 00 00 00}."), Ok(12));
    assert_eq!(skit_letters("{15:80}!"), Ok(7));
    assert_eq!(skit_letters("{09:25 00 00 00}{82}{G:99D2}"), Ok(3));
    // What no skit of the game has is given back, to be refused.
    assert_eq!(skit_letters("two\nrows"), Err("a line break".into()));
    assert_eq!(skit_letters("{14:80} gald"), Err("{14:80}".into()));
    assert_eq!(skit_letters("{02}"), Err("{02}".into()));
}

#[test]
fn a_changed_skit_line_is_given_the_slowest_pace_that_shows_it_whole() {
    // 00058.001: 519 frames, 64 letters where the game's 27 type at 8. The game's line is left
    // whole for 285 frames, so this one must be for 60: at 6 it is (519 - 384 - 18 = 117), at 7
    // it is not (519 - 448 - 18 = 53).
    assert_eq!(line_pace(519, 64, 8, 27), Ok(6));
    // A line that is whole at the game's own pace keeps it.
    assert_eq!(line_pace(294, 18, 8, 11), Ok(8));
    // The game leaves 00060.001 whole for only 1 frame (219 - 200 - 18), so that is all it needs.
    assert_eq!(line_pace(219, 37, 8, 25), Ok(5));
    // A game's line typed past its own window leaves no rest to keep, and none is asked for.
    assert_eq!(line_pace(100, 16, 8, 20), Ok(5));
    assert_eq!(line_pace(100, 17, 8, 20), Err(16)); // (100 - 18) / 5
    // No faster than 5 frames a letter: one letter more than fits at 5 is refused, with the most
    // that fit.
    assert_eq!(line_pace(219, 40, 8, 25), Ok(5));
    assert_eq!(line_pace(219, 41, 8, 25), Err(40));
    assert_eq!(line_pace(519, 200, 8, 27), Err(88)); // (519 - 18 - 60) / 5
    // Where the game types faster than 5 already, its own pace is the fastest.
    assert_eq!(line_pace(300, 50, 3, 40), Ok(3));
    assert_eq!(line_pace(300, 100, 3, 40), Err(74)); // (300 - 18 - 60) / 3
}

#[test]
fn a_skit_s_changed_lines_are_timed_and_the_rest_keep_the_game_s_pace() {
    let strings: [&[u8]; 6] = [b"", b"a", b"b", b"c", b"d", b"e"];
    let member = timed_skit(&strings, &skit_code(&LINES_OF_00058), 1454);
    let own = |letters: usize| "あ".repeat(letters);
    let changed = |lines: &[(u32, usize, usize)]| -> BTreeMap<u32, (String, String)> {
        lines.iter().map(|(number, letters, of)| (*number, ("x".repeat(*letters), own(*of)))).collect()
    };
    // Nothing changed, nothing timed.
    assert_eq!(skit_paces(&member, "00058", &BTreeMap::new()).unwrap(), Paced::default());

    // 00058.001 goes from 8 to 6 and .005 from 6 to 5; .004 keeps its 8; .002 and .003 are the
    // game's own and keep theirs.
    let paced = skit_paces(&member, "00058", &changed(&[(1, 64, 27), (4, 18, 11), (5, 21, 20)])).unwrap();
    assert_eq!((paced.timed, paced.faster, paced.refused.len()), (3, 2, 0));
    let replacements = BTreeMap::from([(1, b"x".repeat(64)), (4, b"y".repeat(18)), (5, b"z".repeat(21))]);
    let (rebuilt, _) = rebuild_skit(&member, &replacements, &paced.writes, "00058").unwrap();
    let (unpaced, _) = rebuild_skit(&member, &replacements, &[], "00058").unwrap();
    let script = |member: &[u8]| entries_of(member).1[SKIT_SCRIPT].clone();
    let code_of = |script: &[u8]| script[12..number(script, 8).unwrap() as usize].to_vec();
    let mut expected = LINES_OF_00058;
    (expected[0].1, expected[4].1) = ("90 06", "85");
    // The same build without the paces has the game's code, the references to the strings that
    // moved apart; with them, only the two paces differ from it, and the text is the same.
    let (game, faster) = (skit_code(&LINES_OF_00058), skit_code(&expected));
    let paces: Vec<usize> = (0..game.len()).filter(|at| game[*at] != faster[*at]).collect();
    assert_eq!(paces.len(), 2);
    let mut wanted = code_of(&script(&unpaced));
    for at in paces {
        assert_eq!(wanted[at], game[at]);
        wanted[at] = faster[at];
    }
    assert_eq!(code_of(&script(&rebuilt)), wanted);
    assert_eq!(text_of(&script(&rebuilt)), text_of(&script(&unpaced)));
    assert_eq!(entries_of(&rebuilt).1[SKIT_LIPS], entries_of(&member).1[SKIT_LIPS]);

    // Every line too long is named at once, with its letters and the most that fit: .001 has 519
    // frames and must rest 60, so (519 - 18 - 60) / 5 letters; .005 has 144 and must rest only
    // the 6 that the game's own line does, so (144 - 18 - 6) / 5. The other two fit.
    let lines = changed(&[(1, 120, 27), (3, 135, 62), (4, 18, 11), (5, 30, 20)]);
    let paced = skit_paces(&member, "00058", &lines).unwrap();
    let refused = ["00058.001 has 120 letters, and at most 88 fit", "00058.005 has 30 letters, and at most 24 fit"];
    assert_eq!((paced.refused.as_slice(), paced.timed), (refused.map(String::from).as_slice(), 2));
    let one = skit_refusal(&paced.refused[..1]).unwrap();
    assert!(one.starts_with("skit line 00058.001 has 120 letters, and at most 88 fit: with the option skit-pace"));
    let several = skit_refusal(&paced.refused).unwrap();
    assert!(several.starts_with("2 skit lines cannot be shown whole: with the option skit-pace"), "{several}");
    assert!(
        several.ends_with(&format!(
            "
  {}
  {}",
            refused[0], refused[1]
        )),
        "{several}"
    );
    assert_eq!(skit_refusal(&[]), None);

    // What cannot be counted, and a line whose start is not known, are refused too.
    let odd = [hex("fe 81 d0 c0 f3 00 00 83 81 80 83 f8 01 00 f5 fa 01 c0"), skit_code(&LINES_OF_00058[1..])].concat();
    let member = timed_skit(&strings, &odd, 1454);
    let mut lines = changed(&[(1, 10, 10)]);
    lines.insert(2, ("{14:80} gald".into(), own(5)));
    assert_eq!(
        skit_paces(&member, "00058", &lines).unwrap().refused,
        [
            "00058.001: its skit's script does not say when the line starts the way the others do, so it cannot be \
             timed",
            "00058.002: {14:80} cannot be timed in a skit, since no skit of the game has one",
        ]
    );
}

/// A menu module made up for a test: `words` are 32-bit words at its start, then `data` from
/// offset 64 on.
fn module(words: &[u32], data: &[u8]) -> Vec<u8> {
    let mut module: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    assert!(module.len() <= 64);
    module.resize(64, 0);
    module.extend_from_slice(data);
    module
}

#[test]
fn what_a_module_points_at_is_found_in_its_words_and_its_instructions() {
    // lui $4, $002A, and far from it addiu $5, $4, -$4500 + $40: $29BB40.
    let pair = [0x3C04_002A, 0, 0, 0, 0, 0, 0, 0, 0, 0x2485_BB40];
    // lui $6, $0029; ori $6, $6, $BB48: $29BB48.
    let other = [0x3C06_0029, 0x34C6_BB48];
    // A word that is an address inside the module, and two that are outside it.
    let words = [MENU_BASE + 0x50, MENU_BASE + 0x1000, MENU_BASE - 4];
    let data = module(&[pair.as_slice(), &other, &words].concat(), &[0; 32]);
    assert_eq!(pointed_at(&data, MENU_BASE), BTreeSet::from([0x40, 0x48, 0x50]));
    // A low half is tried with every high half its register is given, and with no other's.
    let two = module(&[0x3C04_002A, 0x3C04_0029, 0x2484_BB40, 0x24A5_BB48], &[0; 32]);
    assert_eq!(pointed_at(&two, MENU_BASE), BTreeSet::from([0x40]));
}

#[test]
fn menu_text_is_what_is_pointed_at_and_reads_as_text() {
    let chars = chars();
    // At 64 a label; at 72 a help bar, whose button numbers hold zero bytes; at 96 text nothing
    // points at; at 104 a character between a wide space and a long mark written in Shift-JIS,
    // as the cooking screen has brackets; at 112 plain ASCII; at 120 a programmer's note in
    // Shift-JIS alone.
    let data = [
        hex("99 41 99 42 00 00 00 00"),
        hex("09 35 00 00 00 99 43 01 09 36 00 00 00 99 41 00 00 00 00 00 00 00 00 00"),
        hex("99 43 99 43 00 00 00 00"),
        hex("81 40 99 41 81 5b 00 00"),
        b"%s%d\0\0\0\0".to_vec(),
        hex("81 40 82 a0 00 00 00 00"),
    ]
    .concat();
    let words = [MENU_BASE + 64, MENU_BASE + 72, MENU_BASE + 104, MENU_BASE + 112, MENU_BASE + 120];
    let data = module(&words, &data);
    let strings = menu_strings(&data, &MENU, &chars);
    assert_eq!(strings.keys().copied().collect::<Vec<_>>(), [64, 72, 104]);
    // Those are kept as the game has them, and only the menus proper write them so. A character
    // the font does not have is no text that way either.
    assert_eq!(decode(&strings[&104], &chars), "{81}@".to_string() + &decoded("99 41") + "{81}[");
    assert_eq!(menu_text_length(&hex("99 41 81 69 00"), 0, &chars, &MENU), None);
    assert_eq!(encode(&decode(&strings[&104], &chars), &chars).unwrap(), strings[&104]);
    let without = Module { shift_jis: false, ..MENU };
    assert_eq!(menu_strings(&data, &without, &chars).keys().copied().collect::<Vec<_>>(), [64, 72]);
    assert_eq!(strings[&72], hex("09 35 00 00 00 99 43 01 09 36 00 00 00 99 41"));
    assert_eq!(
        decode(&strings[&72], &chars),
        "{09:35 00 00 00}".to_string() + &decoded("99 43") + "\n{09:36 00 00 00}" + &decoded("99 41")
    );
    // A string may take its own bytes and the unused ones up to the next multiple of 8.
    let targets = pointed_at(&data, MENU_BASE);
    assert_eq!(menu_room(&data, 64, 4, &targets), 7);
    assert_eq!(menu_room(&data, 72, 15, &targets), 15);
    // Not past a byte that is not zero, and not past one that something points at.
    let mut used = data.clone();
    used[70] = 1;
    assert_eq!(menu_room(&used, 64, 4, &targets), 5);
    assert_eq!(menu_room(&data, 64, 4, &BTreeSet::from([64, 69])), 4);
    // What ends before its zero byte, or holds no character of the list, is no text.
    assert_eq!(menu_text_length(&data, 112, &chars, &MENU), None);
    assert_eq!(menu_text_length(&data, 120, &chars, &MENU), None);
    assert_eq!(menu_text_length(&data[..68], 64, &chars, &MENU), None);
    // Nor is what starts off a multiple of 8, or inside the string before it, though something
    // may seem to point there; nor a word that is an address and reads as a character and a bracket.
    let inside = module(&[MENU_BASE + 64, MENU_BASE + 66, MENU_BASE + 72, MENU_BASE + 80], &{
        let mut data = hex("99 41 99 42 99 43 99 41 99 42 99 43 00 00 00 00");
        data.extend(hex("99 41 29 00 00 00 00 00"));
        data
    });
    assert_eq!(menu_strings(&inside, &MENU, &chars).keys().copied().collect::<Vec<_>>(), [64]);
    // Nor what is in the module's code: its data starts past the last return from a function.
    let mut code = data.clone();
    code[68..72].copy_from_slice(&RETURN.to_le_bytes());
    assert_eq!((code_end(&data), code_end(&code)), (0, 72));
    assert_eq!(menu_strings(&code, &MENU, &chars).keys().copied().collect::<Vec<_>>(), [72, 104]);
    // A module whose text is written as the program's is takes the dot of an ellipsis and a name.
    let named = module(&[LARGE_MODULE_BASE + 64], &hex("99 41 a5 07 01 00 00 00 00 00 00 00"));
    let about = module_of(BATTLE_MODULE).unwrap();
    assert_eq!(menu_strings(&named, &about, &chars)[&64], hex("99 41 a5 07 01 00 00 00"));
    assert!(menu_strings(&named, &Module { wide: false, ..about }, &chars).is_empty());
}

#[test]
fn the_program_s_text_is_what_its_words_point_at() {
    let chars = chars();
    // A program's file: words in its code that point into its data, where three strings are.
    // The second has the dot of an ellipsis and a party member's name, whose number holds zero
    // bytes; the third is plain ASCII, which is no text of the game's.
    let mut program = vec![0; PROGRAM_DATA + 64];
    let strings = [hex("99 41 99 42"), hex("99 43 a5 a5 07 02 00 00 00"), b"%d".to_vec()];
    let places = [PROGRAM_DATA, PROGRAM_DATA + 8, PROGRAM_DATA + 24];
    for (place, string) in places.iter().zip(&strings) {
        program[*place..*place + string.len()].copy_from_slice(string);
    }
    // A word that points into the code, and one past the end of the file, point at no text.
    let words = [places[0], places[1], places[2], 0x1000, PROGRAM_DATA + 64];
    for (at, target) in words.iter().enumerate() {
        program[4 * at..4 * at + 4].copy_from_slice(&(*target as u32 + PROGRAM_BASE).to_le_bytes());
    }
    assert_eq!(program_targets(&program), BTreeSet::from(places));
    let found = program_strings(&program, &chars);
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), [PROGRAM_DATA as u32, PROGRAM_DATA as u32 + 8]);
    assert_eq!(found[&(PROGRAM_DATA as u32 + 8)], strings[1]);
    assert_eq!(decode(&strings[1], &chars), decoded("99 43") + "･･{NAME:02 00 00 00}");
    // The half-width dot and a member's name are no part of a menu module's text.
    assert_eq!(menu_text_length(&program, places[1], &chars, &MENU), None);
    assert_eq!(program_text_length(&program, places[1], &chars), Some(9));
}

#[test]
fn a_list_string_moves_only_when_every_pointer_at_it_is_known() {
    // A program's file with eight strings past its tables, 16 bytes apart.
    let first = PROGRAM_TABLES.end + 0x100;
    let mut program = vec![0; first + 0x100];
    let place = |string: usize| first + 16 * string;
    let strings: Numbered = (0..8).map(|string| (place(string) as u32, hex("99 41 99 42"))).collect();
    for offset in strings.keys() {
        program[*offset as usize..*offset as usize + 4].copy_from_slice(&hex("99 41 99 42"));
    }
    let mut point = |at: usize, string: usize| {
        program[at..at + 4].copy_from_slice(&(place(string) as u32 + PROGRAM_BASE).to_le_bytes());
    };
    // A column of pointers in the tables, a step of 8 apart, at strings 0 to 6.
    let column = PROGRAM_TABLES.start + 0x400;
    (0..7).for_each(|string| point(column + 8 * string, string));
    // String 4 has its address in a word outside the tables too, and string 5 in a word of the
    // tables that stands alone: neither word is known to be a pointer.
    point(PROGRAM_TABLES.end + 8, 4);
    point(column + 0x1000, 5);
    // String 7 has three words side by side, which is how a number that is repeated looks.
    (0..3).for_each(|word| point(column + 0x2000 + 8 * word, 7));
    // The code builds the address of string 6: a high half, something else, then the low half.
    let address = place(6) as u32 + PROGRAM_BASE;
    let (high, low) = ((address + 0x8000) >> 16, address & 0xFFFF);
    let code = [LUI << 26 | 4 << 16 | high, 0, ADDIU << 26 | 4 << 21 | 4 << 16 | low, RETURN, 0];
    for (word, instruction) in code.iter().enumerate() {
        program[PROGRAM_CODE.start + 4 * word..][..4].copy_from_slice(&instruction.to_le_bytes());
    }
    let low_at = PROGRAM_CODE.start + 8;
    assert_eq!(program_code_use(&program).built, BTreeMap::from([(address, vec![low_at])]));
    // Strings 0 to 3 and 6 can move: 6 through its pointer and the instruction that builds it.
    let reach = program_reach(&program, &strings);
    assert_eq!(reach.movable.keys().copied().collect::<Vec<_>>(), [0, 1, 2, 3, 6].map(|string| place(string) as u32));
    assert_eq!(reach.movable[&(place(2) as u32)], Reached { words: vec![column + 16], code: vec![] });
    assert_eq!(reach.movable[&(place(6) as u32)], Reached { words: vec![column + 48], code: vec![low_at] });
    assert!(reach.read_straight.is_empty());
    // It can go wherever the high half the code gives still reaches: 32,768 bytes to either side.
    let half = ((high << 16) - PROGRAM_BASE) as usize;
    assert_eq!(within_reach(&program, PROGRAM_BASE, place(6), &[low_at]), half - 0x8000..half + 0x8000);
    assert_eq!(within_reach(&program, PROGRAM_BASE, place(2), &[]), 0..usize::MAX);
}

#[test]
fn the_code_s_addresses_are_followed_through_its_registers() {
    let mut program = vec![0; PROGRAM_CODE.start + 0x100];
    let mut run = |code: &[u32]| {
        program[PROGRAM_CODE.start..].fill(0);
        for (word, instruction) in code.iter().enumerate() {
            program[PROGRAM_CODE.start + 4 * word..][..4].copy_from_slice(&instruction.to_le_bytes());
        }
        program_code_use(&program)
    };
    let at = |word: usize| PROGRAM_CODE.start + 4 * word;
    // Register 4 is one a call may change, and register 16 is one it may not.
    let (lui, addiu) = (
        |to: u32, half: u32| LUI << 26 | to << 16 | half,
        |to: u32, from: u32, low: u32| ADDIU << 26 | from << 21 | to << 16 | low,
    );
    let (load, call, branch_likely) = (0x23 << 26 | 16 << 21 | 2 << 16 | 0x2C48, CALL << 26 | 0x400, 0x14 << 26);
    // One high half serves two addresses, the low half of the second a negative number; `ori` joins its half on.
    let found =
        run(&[lui(16, 0x21), addiu(4, 16, 0x50E0), addiu(5, 16, 0x8000), ORI << 26 | 16 << 21 | 6 << 16 | 0x8000]);
    assert_eq!(
        found.built,
        BTreeMap::from([(0x2150E0, vec![at(1)]), (0x208000, vec![at(2)]), (0x218000, vec![at(3)])])
    );
    assert!(found.straight.is_empty());
    // Once the low half is added in the same register, it holds an address: adding to it builds no other.
    assert_eq!(run(&[lui(4, 0x20), addiu(4, 4, 0x1000), addiu(5, 4, 0x200)]).built.len(), 1);
    // A register given a plain number, or loaded from memory, is forgotten.
    assert!(run(&[lui(4, 0x20), addiu(4, 0, 3), addiu(5, 4, 8)]).built.is_empty());
    assert!(run(&[lui(4, 0x20), 0x23 << 26 | 29 << 21 | 4 << 16, addiu(5, 4, 8)]).built.is_empty());
    // After a return everything is forgotten, and after a call what a function may change; in
    // both cases the instruction that follows still runs.
    assert!(run(&[lui(16, 0x20), RETURN, 0, addiu(4, 16, 8)]).built.is_empty());
    assert_eq!(run(&[lui(16, 0x20), RETURN, addiu(4, 16, 8)]).built.len(), 1);
    assert!(run(&[lui(4, 0x20), call, 0, addiu(5, 4, 8)]).built.is_empty());
    assert_eq!(run(&[lui(4, 0x20), call, addiu(4, 4, 8)]).built.len(), 1);
    assert_eq!(run(&[lui(16, 0x20), call, 0, addiu(4, 16, 8)]).built.len(), 1);
    // A load through a high half reads memory straight, and what is loaded replaces its register.
    let found = run(&[lui(16, 0x20), load, addiu(3, 16, 0x2C48)]);
    assert_eq!((found.straight, found.built.len()), (BTreeSet::from([0x202C48]), 1));
    // A high half given only when a branch is taken is there at the branch's target, two
    // instructions on, and not at the instruction the branch skips to when it is not taken.
    let found = run(&[branch_likely | 2, lui(16, 0x20), load, addiu(3, 16, 0x2C48)]);
    assert_eq!((found.straight.len(), found.built.len()), (0, 1));
    // Two ways to one register, as the game's compiler writes a choice between two strings: a
    // branch taken with the high half in the instruction after it leads to the second low
    // half, and the way on has its own high half and low half and then jumps past the second.
    let always = 0x04 << 26;
    let choice = [branch_likely | 4, lui(2, 0x2A), lui(2, 0x2A), always | 2, addiu(2, 2, 0xF910), addiu(2, 2, 0xF978)];
    let found = run(&choice);
    assert_eq!(found.built, BTreeMap::from([(0x29F910, vec![at(4)]), (0x29F978, vec![at(5)])]));
    // A jump leads to its target alone, here over the instruction that would add a low half.
    let jump = 0x02 << 26 | (PROGRAM_CODE.start as u32 + PROGRAM_BASE + 16) >> 2;
    assert!(run(&[lui(16, 0x20), jump, 0, addiu(4, 16, 8), 0]).built.is_empty());
    assert_eq!(run(&[lui(16, 0x20), jump, 0, 0, addiu(4, 16, 8)]).built.len(), 1);
}

#[test]
#[allow(clippy::single_range_in_vec_init)] // a list of one stretch is what is meant
fn changed_list_strings_share_the_room_they_had() {
    // Each string: where it is, the bytes that were its own, the bytes it needs.
    // Two side by side make one stretch of 32 bytes, and what they leave is at its end.
    const ANY: Range<usize> = 0..usize::MAX; // a string that may be put anywhere
    assert_eq!(
        share_room(&[(100, 16, 20, ANY), (116, 16, 5, ANY)], None),
        (vec![100..132], vec![Some(100), Some(120)], 7)
    );
    // One that fits stays where it was, and the order given does not matter.
    assert_eq!(share_room(&[(116, 16, 5, ANY), (100, 16, 10, ANY)], None).1, [Some(110), Some(100)]);
    assert_eq!(share_room(&[(100, 16, 16, ANY)], None), (vec![100..116], vec![Some(100)], 0));
    // A string its own stretch has no room for goes to the first stretch that has: here the
    // second of the first stretch, which is passed over so that the third still fits there.
    let strings = [(100, 16, 10, ANY), (116, 16, 35, ANY), (132, 8, 6, ANY), (500, 40, 4, ANY), (900, 40, 4, ANY)];
    let (stretches, places, largest) = share_room(&strings, None);
    assert_eq!(stretches, [100..140, 500..540, 900..940]);
    assert_eq!(places, [Some(100), Some(504), Some(110), Some(500), Some(900)]);
    assert_eq!(largest, 36);
    // Of those left over the longest is placed first, and one that fits nowhere is not placed.
    let strings = [(100, 8, 30, ANY), (108, 8, 20, ANY), (116, 8, 50, ANY), (500, 40, 4, ANY)];
    assert_eq!(share_room(&strings, None).1, [Some(504), Some(100), None, Some(500)]);
    // A string that may only be put in some places passes over the others: here the second
    // cannot follow the first in their own stretch, nor go to the next, and goes to the last.
    let strings = [(100, 16, 10, ANY), (116, 16, 4, 100..110), (500, 40, 4, ANY), (900, 40, 4, ANY)];
    assert_eq!(share_room(&strings, None).1, [Some(100), None, Some(500), Some(900)]);
    let strings = [(100, 16, 10, ANY), (116, 16, 4, 900..1000), (500, 40, 4, ANY), (900, 40, 4, ANY)];
    assert_eq!(share_room(&strings, None).1, [Some(100), Some(904), Some(500), Some(900)]);
    assert_eq!(share_room(&[], None), (vec![], vec![], 0));
    // Spare room is one more stretch, used when the others have no place, and only as far as needed.
    let strings = [(100, 16, 10, ANY), (116, 16, 35, ANY), (132, 8, 6, ANY)];
    let (stretches, places, largest) = share_room(&strings, Some(1000..1100));
    assert_eq!(stretches, [100..140, 1000..1100]);
    assert_eq!((places, largest), (vec![Some(100), Some(1000), Some(110)], 65));
    assert_eq!(share_room(&strings, Some(1000..1020)).1, [Some(100), None, Some(110)]);
    assert_eq!(share_room(&strings, Some(1000..1000)).0, [100..140]);
}

/// An enemy's record like the game's: the header with its table of sections, the name in its
/// field at `at`, and what follows the field.
fn enemy(name: &[u8], at: usize) -> Vec<u8> {
    let mut record = vec![0; at + 0x70];
    record[..4].copy_from_slice(ENEMY_MAGIC);
    let place = ENEMY_SECTIONS + 8 * ENEMY_NAME_SECTION;
    record[place..place + 8].copy_from_slice(&[(at as u32).to_le_bytes(), 1u32.to_le_bytes()].concat());
    record[at..at + name.len()].copy_from_slice(name);
    record[at + ENEMY_NAME_FIELD..].fill(0x5A); // what comes after the field is not the name's
    record
}

/// A pack of enemies like the game's: for each record a picture after it, every entry compressed.
fn enemy_pack(records: &[Vec<u8>]) -> Vec<u8> {
    let packed = |data: &[u8]| lzss_ring::compress_block(data, lzss_ring::MODE_LZSS).unwrap();
    let mut entries = vec![packed(b"GR and what the group is")];
    for record in records {
        entries.extend([packed(record), packed(&[b"TM2@".as_slice(), &[3; 200]].concat())]);
    }
    let mut pack = pack_on(&entries.iter().map(Vec::as_slice).collect::<Vec<_>>(), ENEMY_PACK_ALIGNMENT);
    pack.resize(pack.len().next_multiple_of(ENEMY_PACK_ALIGNMENT), 0);
    pack
}

#[test]
fn an_enemy_s_name_is_in_a_field_of_its_record() {
    let (first, second) = (hex("99 41 99 42"), hex("99 43"));
    let pack = enemy_pack(&[enemy(&first, 0x1E4), enemy(&second, 0x268)]);
    let records = enemy_records(&pack);
    assert_eq!(records.iter().map(|(entry, _)| *entry).collect::<Vec<_>>(), [1, 3]);
    assert_eq!((enemy_name(&records[0].1), enemy_name(&records[1].1)), (Some(&first[..]), Some(&second[..])));
    // What is no record has no name: another entry of the pack, and a record cut short.
    assert_eq!(enemy_name(b"TM2@ and the rest of a picture"), None);
    assert_eq!(enemy_name(&enemy(&first, 0x1E4)[..0x1F0]), None);
    assert!(enemy_records(b"no pack at all").is_empty());

    // A new name fills the field, and the pack is written out again around the record.
    let longer = hex("99 41").repeat(11); // 22 bytes, all the field holds
    let renamed = rename_enemies(&pack, &BTreeMap::from([(3, longer.clone())]), "08211").unwrap();
    let after = enemy_records(&renamed);
    assert_eq!((enemy_name(&after[0].1), enemy_name(&after[1].1)), (Some(&first[..]), Some(&longer[..])));
    // Nothing of the record but the field changed, and the other entries are as they were.
    let (old, new) = (&records[1].1, &after[1].1);
    assert!(old.len() == new.len() && (0..old.len()).all(|at| (0x268..0x268 + 23).contains(&at) || old[at] == new[at]));
    let entries = |pack: &[u8]| -> Vec<Vec<u8>> {
        pack_entries(pack).unwrap().iter().map(|(at, size)| pack[*at..*at + *size].to_vec()).collect()
    };
    let (before, now) = (entries(&pack), entries(&renamed));
    assert!((0..before.len()).all(|entry| entry == 3 || before[entry] == now[entry]));
    let on_four = pack_entries(&renamed).unwrap().iter().all(|(at, _)| at.is_multiple_of(4));
    assert!(on_four && renamed.len().is_multiple_of(4));
    // A pack that changes nothing of a pack gives it back as it was.
    assert!(rename_enemies(&pack, &BTreeMap::new(), "08211").unwrap() == pack);
    assert_eq!(
        message(rename_enemies(&pack, &BTreeMap::from([(2, first.clone())]), "08211")),
        "08211 is not laid out as a pack of enemies is"
    );
}

/// The member that holds the name plates' list, like the game's: a picture, then `ALLG` with an
/// entry of eight bytes for each pack of enemies in `groups`, then `TEKI` with the names given,
/// each padded with spaces and followed by `tail`.
fn plate_member(groups: &[[u8; 8]], names: &[&[u8]], tail: [u8; 4]) -> Vec<u8> {
    let packed = |data: &[u8]| lzss_ring::compress_block(data, lzss_ring::MODE_LZSS_RLE).unwrap();
    let groups: Vec<u8> = [GROUPS_MAGIC, &groups.concat()].concat();
    let mut list = PLATES_MAGIC.to_vec();
    for name in names {
        let mut field = name.to_vec();
        field.resize(PLATE_FIELD, PLATE_PADDING);
        list.extend(field.into_iter().chain(tail));
    }
    let entries = [packed(&[b"TM2@".as_slice(), &[7; 300]].concat()), packed(&groups), packed(&list)];
    let mut member = pack_on(&entries.iter().map(Vec::as_slice).collect::<Vec<_>>(), ENEMY_PACK_ALIGNMENT);
    member.resize(member.len().next_multiple_of(ENEMY_PACK_ALIGNMENT), 0);
    member
}

#[test]
fn the_name_plate_takes_an_enemy_s_name_from_a_list_of_its_own() {
    let (owl, slime, kobold) = (hex("99 cf 99 cb 9a 54"), hex("99 41 99 42"), hex("99 43"));
    // The first pack has the owl and two kobolds; the second, a slime, none of the owls the
    // group names, and a kobold.
    let groups = [[0, 2, 0, 1, 2, 0, 0, 0], [1, 0, 2, 1, 0, 1, 0, 0]];
    let member = plate_member(&groups, &[&owl, &slime, &kobold], [0, 1, 0, 0]);
    let (read_groups, list) = plate_lists(&member).unwrap();
    assert_eq!(read_groups.len(), 4 + 16);
    assert_eq!(
        (plate_name(&list, 0), plate_name(&list, 2), plate_name(&list, 3)),
        (Some(&owl[..]), Some(&kobold[..]), None)
    );
    let first = ENEMY_MEMBERS.start();
    let enemies = BTreeMap::from([
        (*first, BTreeMap::from([(1, owl.clone()), (4, kobold.clone())])),
        (first + 1, BTreeMap::from([(1, slime.clone()), (4, kobold.clone())])),
    ]);
    let kinds = plate_kinds(&enemies, &read_groups, &list).unwrap();
    let expected = [((*first, 1), 0), ((*first, 4), 2), ((first + 1, 1), 1), ((first + 1, 4), 2)];
    assert_eq!(kinds, HashMap::from(expected));
    // A pack whose names are not those the list gives it is said, not guessed at.
    let mut wrong = enemies.clone();
    wrong.get_mut(first).unwrap().insert(4, slime.clone());
    assert_eq!(
        message(plate_kinds(&wrong, &read_groups, &list)),
        "the enemies of 08178 do not agree with the list the battle's name plate takes their names from \
         (member 8063 of FILE.FPB)"
    );

    // A new name is padded with spaces to the end of the field, and the bytes after it stay.
    let new = b"KOBOLD OF THE FAR HILL".to_vec(); // 22 bytes, all the room
    let renamed = rename_plates(&member, &BTreeMap::from([(2, new.clone())])).unwrap();
    let (groups_after, after) = plate_lists(&renamed).unwrap();
    assert_eq!(groups_after, read_groups);
    assert_eq!((plate_name(&after, 0), plate_name(&after, 2)), (Some(&owl[..]), Some(&new[..])));
    let at = 4 + 2 * PLATE_SIZE;
    assert_eq!(&after[at..at + PLATE_SIZE], [new.as_slice(), b"  ", &[0, 1, 0, 0]].concat());
    assert!((0..list.len()).all(|byte| (at..at + PLATE_FIELD).contains(&byte) || list[byte] == after[byte]));
    // The other entries are as they were, every entry starts on a multiple of 4, and so does the end.
    let entries = |member: &[u8]| -> Vec<Vec<u8>> {
        pack_entries(member).unwrap().iter().map(|(at, size)| member[*at..*at + *size].to_vec()).collect()
    };
    assert_eq!(entries(&renamed)[..2], entries(&member)[..2]);
    let on_four = pack_entries(&renamed).unwrap().iter().all(|(at, _)| at.is_multiple_of(4));
    assert!(on_four && renamed.len().is_multiple_of(4));
    // Nothing to change gives the member back as it was; a name too long for the field is refused.
    assert_eq!(rename_plates(&member, &BTreeMap::new()).unwrap(), member);
    assert_eq!(
        message(rename_plates(&member, &BTreeMap::from([(1, b"KOBOLDS OF THE FAR HILL".to_vec())]))),
        "enemy 1 of the name plates takes 23 bytes and has room for 22"
    );
    assert!(plate_lists(b"no pack at all").is_none());
}

/// A script of a game within the game like the game's: a header, `code`, then `data`.
fn game_script(code: &[u8], data: &[u8]) -> Vec<u8> {
    let start = 32 + code.len();
    let header = [0x3FC, (start + data.len()) as u32, code.len() as u32, data.len() as u32, 32, start as u32, 0];
    let header: Vec<u8> = header.iter().flat_map(|word| word.to_le_bytes()).collect();
    [GAME_MAGIC, &header, code, data].concat()
}

#[test]
fn a_string_of_a_game_is_what_its_script_refers_to() {
    let chars = chars();
    // The data: a number, then four strings one after another. The third is a picture that
    // happens to read as text, and the last is plain ASCII.
    let strings = [hex("99 41 99 42"), hex("07 02 00 00 00 99 43 a5"), hex("99 41 99 41 99 41"), b"%d".to_vec()];
    let mut data = vec![0x7F, 0, 0, 0];
    let mut offsets = Vec::new();
    for string in &strings {
        offsets.push(data.len() as u32);
        data.extend([string.as_slice(), &[0]].concat());
    }
    // The code refers to the first string off a multiple of 4, and a table would to the second and the last.
    let code =
        [&[0xAB][..], &offsets[0].to_le_bytes(), &[0, 0, 0], &offsets[1].to_le_bytes(), &offsets[3].to_le_bytes()]
            .concat();
    let script = game_script(&code, &data);
    let start = 32 + code.len() as u32;
    assert_eq!(game_data(&script), Some(start as usize..script.len()));
    let found = game_strings(&script, &chars);
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), [start + offsets[0], start + offsets[1]]);
    assert_eq!(found[&(start + offsets[1])], strings[1]);
    assert_eq!(decode(&strings[1], &chars), "{NAME:02 00 00 00}".to_string() + &decoded("99 43") + "･");
    // What is no such script, or says its data goes on past its end, has no strings.
    assert_eq!(game_data(b"SCED and the rest of another kind of script"), None);
    assert_eq!(game_data(&script[..script.len() - 1]), None);
    assert!(game_strings(&script[..script.len() - 1], &chars).is_empty());
}

#[test]
fn a_string_of_a_game_may_follow_other_data_where_a_table_refers_to_it() {
    let chars = chars();
    // The data, as the quiz's book has it: a table of three numbers, two 16-bit numbers, then a
    // string with no zero byte before it, and two strings after it. Last, another 16-bit number
    // and a string that only a lone number in the code refers to: no table, so no string.
    let first = hex("99 41 99 42");
    let (second, third) = (hex("99 43 99 43"), hex("99 42 99 41"));
    let at = 12 + 4; // past the table and the 16-bit numbers
    let after = [at + first.len() + 1, at + first.len() + 1 + second.len() + 1];
    let mut data: Vec<u8> = [after[0], at, after[1]].iter().flat_map(|n| (*n as u32).to_le_bytes()).collect();
    data.extend(hex("1c 27 1b 27"));
    for string in [&first, &second, &third] {
        data.extend([string.as_slice(), &[0]].concat());
    }
    data.extend(hex("1d 27"));
    let lone = data.len() as u32;
    data.extend(hex("99 41 99 41 00"));
    let code = [&[0xAB][..], &lone.to_le_bytes(), &[0xAB; 4]].concat();
    let script = game_script(&code, &data);
    let start = 32 + code.len();
    let found = game_strings(&script, &chars);
    let expected = [at, after[0], after[1]].map(|offset| (start + offset) as u32);
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), expected);
    assert_eq!(found[&expected[0]], first);
    // A table's number that points inside a string found the usual way gives none either.
    let mut inside = data.clone();
    inside[4..8].copy_from_slice(&((after[0] + 2) as u32).to_le_bytes());
    let found = game_strings(&game_script(&code, &inside), &chars);
    assert_eq!(found.keys().copied().collect::<Vec<_>>(), [expected[1], expected[2]]);
}

#[test]
fn dialogue_is_broken_at_spaces_to_fit_the_box() {
    let chars = chars();
    let wide = decoded("99 41"); // one wide character, two columns
    let wrapped = |text: &str| wrap_dialogue(text, &chars, &UNKNOWN_NAMES).unwrap();
    let (a19, b19, b20) = ("A".repeat(19), "B".repeat(19), "B".repeat(20));
    // A character may start left of the fortieth column: thirty-nine narrow ones fit on a line,
    // and the word that would pass that goes to the next one.
    assert_eq!(wrapped(&format!("{a19} {b19} tail")), format!("{a19} {b19}\ntail"));
    assert_eq!(wrapped(&format!("{a19} {b20} tail")), format!("{a19}\n{b20} tail"));
    // A wide character takes two columns, and a party member's name not known room for the longest.
    let (first, second) = (wide.repeat(15), wide.repeat(6));
    assert_eq!(wrapped(&format!("{first} {second}")), format!("{first}\n{second}"));
    let (a20, b10) = ("A".repeat(20), "B".repeat(10));
    assert_eq!(wrapped(&format!("{{NAME:01 00 00 00}} {a20} {b10}")), format!("{{NAME:01 00 00 00}} {a20}\n{b10}"));
    // Breaks the pack already has are kept, and a line with no space in it is left alone.
    let long = "C".repeat(60);
    assert_eq!(wrapped(&format!("one\ntwo {long}")), format!("one\ntwo\n{long}"));
    // A page break starts the next line afresh.
    assert_eq!(wrapped(&format!("{a19}{{02}}{b19} tail")), format!("{a19}{{02}}{b19} tail"));
}

#[test]
fn a_party_member_s_name_takes_the_columns_it_is_drawn_with() {
    let chars = chars();
    // Kyle, Reala, Loni, Judas, Nanaly and Harold, as an English pack names them.
    let english = [4, 5, 4, 5, 6, 6];
    let wrapped = |text: &str, names: &NameColumns| wrap_dialogue(text, &chars, names).unwrap();
    // The line the owner saw broken before its last word: 28 columns, a space, Kyle and "!" make
    // 34, which fit; with Kyle as wide as the longest name they made 40, which did not.
    let line = "Hang in there, everyone! We'll all pull together and fight alongside {NAME:01 00 00 00}!";
    let (first, rest) = ("Hang in there, everyone! We'll all pull", "together and fight alongside");
    assert_eq!(wrapped(line, &english), format!("{first}\n{rest} {{NAME:01 00 00 00}}!"));
    assert_eq!(wrapped(line, &UNKNOWN_NAMES), format!("{first}\n{rest}\n{{NAME:01 00 00 00}}!"));
    // Each tag is its own member's name: Harold's six columns where Kyle's four fit. A line breaks
    // before a character would start at the fortieth column, so the "!" after the name decides.
    let (a33, a34) = ("a".repeat(33), "a".repeat(34));
    let line = |letters: &str, member: u8| format!("{letters} {{NAME:{member:02} 00 00 00}}!");
    let broken = |letters: &str, member: u8| {
        format!(
            "{letters}
{{NAME:{member:02} 00 00 00}}!"
        )
    };
    assert_eq!(wrapped(&line(&a33, 6), &english), broken(&a33, 6));
    assert_eq!(wrapped(&line(&a33, 1), &english), line(&a33, 1));
    assert_eq!(wrapped(&line(&a34, 1), &english), broken(&a34, 1));
    // A name the game chooses as it runs, or prints only part of, takes room for the longest.
    for other in ["E8 03 00 00", "00 00 00 00", "07 00 00 00", "12 27 00 00", "01"] {
        assert_eq!(name_columns(Some(other), &english), NAME_COLUMNS, "{other}");
    }
    assert_eq!(name_columns(None, &english), NAME_COLUMNS);
    assert_eq!(name_columns(Some("06 00 00 00"), &english), 6);
    // A name in wide letters takes two columns a letter; a tag or a line break in it is not measured.
    let wide = decoded("99 41");
    assert_eq!(text_columns(&format!("{wide}{wide}{wide}"), &chars), Some(6));
    assert_eq!(text_columns("Kyle", &chars), Some(4));
    assert_eq!((text_columns("K{02}", &chars), text_columns("K\nX", &chars)), (None, None));
}

#[test]
fn pages_are_counted_as_the_game_fills_its_list() {
    let line = |length: usize| "a".repeat(length);
    // Four lines to a page; a line break takes no place, and an empty line is passed over.
    let five = [10, 10, 10, 10, 7].map(line).join("\n");
    assert_eq!(page_characters(&five), [40, 7]);
    assert_eq!(page_characters("ab\n\n\ncd\nef\ngh\nij"), [8, 2]);
    // A page break ends a page early, and ending a string with one adds no page.
    assert_eq!(page_characters("abc{02}de\nf{02}"), [3, 3]);
    // Spaces and wide characters take one place each, and so does a button's picture; a party
    // member's name and other filled-in text are counted as their longest, and the codes for
    // colour and position as nothing.
    assert_eq!(page_characters("a b　c"), [5]);
    assert_eq!(page_characters("{09:28 00 00 00}{0B:01 00 00 00}x{0B:00 00 00 00}"), [2]);
    let filled = "{NAME:01 00 00 00}{15:24 A2}{06:05 00 00 00}{14:34 08}{18:44 88}{16:25 BA}{08:0B 08 00 00}";
    assert_eq!(
        page_characters(&format!("{filled}{{05:80 01 00 00}}")),
        [2 * NAME_CHARACTERS + 3 * NUMBER_CHARACTERS + 2 * THING_CHARACTERS]
    );
}

#[test]
fn a_list_to_choose_from_is_the_message_a_choice_follows() {
    // A message (six numbers, a label, the text, $E0 $00) and the choice ($34 $00 keeps the
    // answer; window 1, two choices, cancel picks the second), each ended by $C0. Then one list
    // or another, the first jumping ($F2) past the second to the choice they share; a message no
    // choice follows; and one whose jump goes back to a choice, which is not followed.
    let code = hex("81 a0 00 80 80 80 91 5e 80 80 f8 00 00 f8 07 00 e0 00 c0 \
         34 00 81 82 82 80 e0 3f d7 c0 \
         80 80 80 80 80 80 f8 10 00 f8 12 00 e0 00 f2 3d 00 \
         80 80 80 80 80 80 f8 20 00 f8 22 00 e0 00 c0 \
         34 00 81 82 82 80 e0 3f d7 c0 \
         80 80 80 80 80 80 f8 30 00 f8 32 00 e0 00 c0 81 e0 03 c0 \
         80 80 80 80 80 80 f8 40 00 f8 42 00 e0 00 f2 13 00");
    assert_eq!(choice_lists(&code).unwrap(), BTreeSet::from([7, 0x12, 0x22]));
    let referred: Vec<usize> = string_references(&code).unwrap().keys().copied().collect();
    assert_eq!(referred, [0, 7, 0x10, 0x12, 0x20, 0x22, 0x30, 0x32, 0x40, 0x42]);
    assert_eq!(message(choice_lists(&hex("f8 00"))), "the script's code does not end where its text starts");
}

#[test]
fn the_game_s_own_lists_to_choose_from() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    let (mut count, mut widest) = (0, 0);
    for index in script_members(&disc).unwrap() {
        let script = read_script_data(&disc, index).unwrap();
        let lists = choice_strings(&script).unwrap();
        let strings: Vec<String> = split_strings(text_of(&script)).iter().map(|data| decode(data, &chars)).collect();
        for number in &lists {
            let text = &strings[*number as usize];
            // Each fits as the game has it.
            let fits = fit_choices(text, text, &chars, &UNKNOWN_NAMES);
            fits.unwrap_or_else(|error| panic!("{index}.{number}: {error}"));
            let columns = |line: &str| line.chars().map(|char| char_columns(char, &chars)).sum::<usize>();
            widest = widest.max(text.split('\n').filter(|line| !line.contains('{')).map(columns).max().unwrap_or(0));
        }
        count += lists.len();
        if index == 6308 {
            // The controls question, the maid's offer, the inn, and the shop's lists, among them
            // the two pages of the refining lessons, which share one choice.
            assert!([19, 315, 385, 407, 409, 415, 418, 420].iter().all(|number| lists.contains(number)), "{lists:?}");
            // The notices and the speech around them are not lists.
            assert!([7, 9, 11, 13, 15, 313, 383, 413, 416].iter().all(|number| !lists.contains(number)));
        }
    }
    assert_eq!((count, widest), (CHOICE_LISTS, 40));
}

#[test]
fn a_list_to_choose_from_is_left_as_written_and_refused_if_it_does_not_fit() {
    let chars = chars();
    let fit = |text: &str, original: &str| fit_choices(text, original, &chars, &UNKNOWN_NAMES);
    let own = "{03:00 00 00 00}Hear it?\n　Yes　\n　No　\n　Never　";
    // A line is not broken where dialogue's would be: each is a choice.
    let long = "{03:00 00 00 00}Hear about the controls?\n　Yes　\n　No　\n　Do not ask me about the controls again";
    assert_ne!(wrap_dialogue(long, &chars, &UNKNOWN_NAMES).unwrap(), long);
    assert_eq!(fit(long, own).unwrap(), long);
    // A line more than the game's own is refused.
    assert_eq!(
        message(fit("{03:00 00 00 00}Would you like to hear\nhow the controls work?\n　Yes　\n　No　\n　Never　", own)),
        "this is a list to choose from, one choice to a line, and it has 5 lines where the game's own has 4: write \
         each choice on a line of its own, and the question before them, if there is one, on no more lines than \
         the game's own"
    );
    // A line wider than the window can be is refused: 55 columns pass, 56 do not.
    let line = |columns: usize| format!("　{}　", "a".repeat(columns - 4));
    assert!(fit(&format!("{}\n　No　", line(55)), own).is_ok());
    assert_eq!(
        message(fit(&format!("　Yes　\n{}", line(56)), own)),
        "line 2 of this list to choose from takes 56 columns, and a line of a list may take 55 (a wide character \
         takes two, and a party member's name as many as it is drawn with): a list is not rewrapped, since each \
         line is a choice, so shorten the line"
    );
    // A party member's name takes the columns it is drawn with, or those of the longest if not known.
    assert!(fit(&format!("{}{{NAME:01 00 00 00}}", "a".repeat(45)), own).is_ok());
    assert!(message(fit(&format!("{}{{NAME:01 00 00 00}}", "a".repeat(46)), own)).starts_with("line 1 of this list"));
    let kyle = [4, 5, 4, 5, 6, 6];
    let name_line = |letters: usize| format!("{}{{NAME:01 00 00 00}}", "a".repeat(letters));
    assert!(fit_choices(&name_line(51), own, &chars, &kyle).is_ok());
    assert!(fit_choices(&name_line(52), own, &chars, &kyle).is_err());
    // A page counts as dialogue's does: four lines of 38 narrow letters are more than a window shows.
    let full = [38, 38, 38, 38].map(|length| "a".repeat(length)).join("\n");
    assert_eq!(
        message(fit(&full, own)),
        "this list to choose from draws 152 characters, and a window shows at most 123 (spaces count, a name as \
         6, a number as 7 and an item as 16): shorten its lines"
    );
}

#[test]
fn long_dialogue_is_broken_narrower_until_its_pages_fit() {
    let chars = chars();
    let wrapped = |text: &str| wrap_dialogue(text, &chars, &UNKNOWN_NAMES);
    let room = PAGE_CHARACTERS - PAGE_MARGIN;
    let words = |count: usize| vec!["word"; count].join(" ");
    // A string whose pages fit at forty columns is broken there: here two lines of 39.
    let fits = words(16);
    assert_eq!(wrapped(&fits).unwrap(), wrap_to(&fits, &chars, LINE_COLUMNS, false, &UNKNOWN_NAMES).unwrap());
    // Four lines of 39 would be 156 on one page: the lines are made narrower until no page
    // passes the room, and the string pages by itself, four lines at a time.
    let long = words(60);
    let fitted = wrapped(&long).unwrap();
    assert_ne!(fitted, wrap_to(&long, &chars, LINE_COLUMNS, false, &UNKNOWN_NAMES).unwrap());
    assert!(page_characters(&fitted).iter().all(|count| *count <= room));
    assert!(fitted.lines().all(|line| line.len() < LINE_COLUMNS - 5));
    assert_eq!(fitted.replace('\n', " "), long);
    // It goes no narrower than it must. Down to 35 columns a line holds seven words, 34 letters,
    // and a page 136; at 34 it holds six, and a page 116.
    assert_eq!(fitted, wrap_to(&long, &chars, 34, true, &UNKNOWN_NAMES).unwrap());
    assert_eq!(page_characters(&wrap_to(&long, &chars, 35, true, &UNKNOWN_NAMES).unwrap())[0], 136);
    // Text the game fills in is given its columns once the lines are narrowed: four items' names
    // and some words fit on three lines when the names take no room, but are 134 on one page.
    let items = "a {16:25 BA} b {16:25 BA} c {16:25 BA} d {16:25 BA} ".to_string() + &words(12);
    let fitted = wrapped(&items).unwrap();
    assert!(page_characters(&fitted).iter().all(|count| *count <= room), "{fitted}");
    assert_eq!(page_characters(&wrap_to(&items, &chars, LINE_COLUMNS, false, &UNKNOWN_NAMES).unwrap()), [4 * 16 + 70]);
    // Breaks the pack has are kept: a page the translator ended sooner is not narrowed.
    let pages = [words(5), words(5), words(5), words(5), words(5)].join("\n");
    assert_eq!(wrapped(&pages).unwrap(), pages);
    // What cannot be made to fit is refused, saying which page and why.
    let line = |length: usize| "a".repeat(length);
    let unbreakable = [line(40), line(40), line(40), line(40)].join(" ");
    assert_eq!(
        message(wrapped(&unbreakable)),
        "page 1 of this text draws 160 characters even with its lines broken at 24 columns, and a page of a \
         speech bubble shows at most 123 (spaces count, a name as 6, a number as 7 and an item as 16): shorten \
         it, or end the page sooner with a line break or {02}"
    );
}

#[test]
fn the_table_of_contents_follows_members_that_change_size() {
    let mut old = vec![0u32; MEMBER_COUNT + 1];
    for (index, entry) in old.iter_mut().enumerate() {
        *entry = index as u32 * 128 + if index < MEMBER_COUNT { 28 } else { 0 }; // each 100 bytes and 28 of padding
    }
    assert_eq!(new_table(&old, &BTreeMap::new()).unwrap(), old);
    // Member 2 grows to 130 bytes: it takes three steps of 64 where it had two.
    let table = new_table(&old, &BTreeMap::from([(2, vec![1; 130])])).unwrap();
    assert_eq!(table[..5], [28, 128 + 28, 256 + 62, 448 + 28, 576 + 28]);
    assert_eq!(table[MEMBER_COUNT], old[MEMBER_COUNT] + 64);
    // A member that fills its last step has no padding.
    assert_eq!(new_table(&old, &BTreeMap::from([(0, vec![1; 64])])).unwrap()[..2], [0, 64 + 28]);
    // The last member starts at the last offset 32 bits can hold.
    old[MEMBER_COUNT - 1] = !PADDING_BITS;
    old[MEMBER_COUNT] = !PADDING_BITS;
    let grown = new_table(&old, &BTreeMap::from([(MEMBER_COUNT - 1, vec![1; 65])]));
    assert_eq!(message(grown), "FILE.FPB would pass 4 GiB, the furthest its table of contents can refer to");
}

/// A glyph of a font, from what says where its ink is.
fn bitmap(width: usize, height: usize, ascent: i64, ink: impl Fn(usize, usize) -> bool) -> Bitmap {
    let mut rows = vec![vec![0u8; width.div_ceil(8)]; height];
    for (y, row) in rows.iter_mut().enumerate() {
        for x in (0..width).filter(|x| ink(*x, y)) {
            row[x / 8] |= 0x80 >> (x % 8);
        }
    }
    Bitmap { rows, width, ascent }
}

/// Levels as a row of hexadecimal digits each, to compare with what the Python reference draws.
fn shown(levels: &[Vec<u8>]) -> Vec<String> {
    levels.iter().map(|row| row.iter().map(|level| format!("{level:x}")).collect()).collect()
}

/// The letter i of GNU Unifont: 8 pixels wide, 16 rows, 14 of them above the baseline.
fn letter() -> Bitmap {
    let rows = hex("00 00 00 08 08 00 18 08 08 08 08 08 08 3E 00 00");
    Bitmap { rows: rows.iter().map(|row| vec![*row]).collect(), width: 8, ascent: 14 }
}

#[test]
fn ink_is_surrounded_as_the_game_s_letters_are() {
    let mut one = vec![vec![false; 7]; 7];
    one[3][3] = true;
    assert_eq!(shown(&outlined(&one)), ["0000000", "0111110", "0123210", "014f410", "0128210", "0111110", "0000000"]);
    assert!(outlined(&[]).is_empty());
}

#[test]
fn a_letter_is_drawn_pixel_for_pixel_on_the_baseline() {
    let rows = shown(&placed(&letter(), GLYPH_SIZE, GLYPH_ROWS, BASELINE));
    // The dot, the stem, and the serif two rows thick, with its half-bright row on the baseline.
    assert_eq!(
        rows[7..],
        [
            "000000000012321000000000",
            "000000000014f41000000000",
            "000000000114f41000000000",
            "000000000124f41000000000",
            "00000000014ff41000000000",
            "00000000014ff41000000000",
            "000000000128f41000000000",
            "000000000114f41000000000",
            "000000000014f41000000000",
            "000000001114f41110000000",
            "000000001234f43210000000",
            "0000000014fffff410000000",
            "0000000014fffff410000000",
            "000000001288888210000000",
            "000000001111111110000000",
        ]
    );
    let drawn = draw_glyph(&letter());
    assert_eq!(drawn.len(), GLYPH_BYTES);
    assert_eq!(sha1_of(&drawn), "2f67a61642c83589a0b3bd4a8ee512c864e84b7e");
    // The left pixel of each pair is in the low half of its byte.
    assert_eq!(drawn[18 * 12 + 4..18 * 12 + 8], [0x41, 0xFF, 0xFF, 0x4F]);
}

#[test]
fn a_wide_character_is_stretched_to_the_cell_as_the_reference_stretches_it() {
    // 18 pixels to 24 and to 22: neither is a ratio a binary fraction holds exactly, so that the
    // shares of a pixel only come out the same if they are added up the same way.
    let wide = bitmap(18, 18, 15, |x, y| (x + 2 * y) % 5 < 2);
    assert_eq!(
        shown(&scaled(&wide, GLYPH_SIZE, GLYPH_ROWS))[..4],
        [
            "fff4348ff8434fff4334fff4",
            "6684fff6686ffa664dff6683",
            "89eebb78eedbb88eecbb89ee",
            "8eff8688ffb888bff6888eff"
        ]
    );
    assert_eq!(sha1_of(&draw_glyph(&wide)), "b4944bd149f0068c6ddebec84c887fafd10f1fce");
    let square = bitmap(16, 16, 14, |x, y| (3..12).contains(&x) && (2..13).contains(&y) && (x + y) % 3 != 0);
    assert_eq!(sha1_of(&draw_glyph(&square)), "cc5334eb76eb07208627daffdee33976f928e8e2");
}

#[test]
fn numbers_are_added_up_as_python_adds_them() {
    let sum = |numbers: &[(f64, bool)]| {
        python_sum(numbers.iter().map(|(amount, whole)| Cover { amount: *amount, whole: *whole }))
    };
    // Fractions lose nothing between them: added one after the other these give 0.6000000000000001.
    assert_eq!(sum(&[(0.1, false), (0.2, false), (0.3, false)]), 0.6);
    assert_eq!(sum(&[(0.1, false); 10]), 1.0);
    assert_eq!(sum(&[(1.0, true), (0.1, false), (0.2, false), (0.3, false)]), 1.6);
    assert_eq!(sum(&[(0.1, false), (1.0, true), (0.2, false), (0.3, false), (1.0, true)]), 2.6);
    // A whole number met after a fraction is added without that care, and may be lost.
    assert_eq!(sum(&[(1e16, false), (1.0, false), (1.0, false)]), 1.0000000000000002e16);
    assert_eq!(sum(&[(1e16, false), (1.0, true), (1.0, true)]), 1e16);
    assert_eq!(sum(&[]), 0.0);
}

#[test]
fn glyphs_go_into_the_half_width_font_small_or_reduced() {
    let empty = |width, height| bitmap(width, height, 0, |_, _| false);
    let fits = [(16, 16), (12, 40), (24, 32), (24, 30), (96, 128)]
        .map(|(width, height)| fits_half_cell(&empty(width, height)));
    assert_eq!(fits, [false, true, true, false, true]);
    // A drawing twice the size of a cell: each pixel is the share of four that are ink.
    let big = bitmap(24, 32, 26, |x, y| (x * y) % 7 < 3);
    assert_eq!(
        shown(&reduced(&big, HALF_WIDTH_CELL, HALF_HEIGHT_CELL))[..4],
        ["fcacfaafcacf", "c7aa7a7c7aa7", "aa7c7a7aa7c7", "cacfaafcacfa"]
    );
    let mut texture = vec![0; HALF_FONT_PIXELS + HALF_ROW_BYTES * 160];
    for (cell, glyph) in [(49, letter()), (50, big.clone()), (13, big), (14, letter())] {
        draw_half_glyph(&mut texture, cell, &glyph).unwrap();
    }
    assert_eq!(sha1_of(&texture), "d5b78663311a83f268a0e4113bedc4bd42752310");
    // Cell 49 is the last of the fifth row: its first pixel is 108 across and 64 down.
    assert!(texture[HALF_FONT_PIXELS + 64 * HALF_ROW_BYTES..][..54].iter().all(|byte| *byte == 0));
    assert_eq!(
        message(draw_half_glyph(&mut texture, 100, &letter())),
        "the half-width font has no cell 100: it is not as large as expected"
    );
}

#[test]
fn what_the_game_draws_as_something_else_needs_a_glyph_of_its_own() {
    // A list of glyphs: the wide space, a full-width capital Z and digit 0, a closing double
    // quote, a minus sign, an opening corner bracket.
    let by_glyph = ['\u{3000}', 'Ｚ', '０', '”', '−', '「'].map(Some);
    // The game's table for $20-$7F: nothing for the number sign; Z and z from the capital, '
    // from the double quote, - and ~ from the minus sign, [ from the corner bracket.
    let mut table = [0; 0x60];
    for (chars, cell) in [("Zz", 1), ("0", 2), ("'", 3), ("-~", 4), ("[", 5)] {
        chars.chars().for_each(|char| table[char as usize - 0x20] = cell);
    }
    let needing: String = " '#aZz{~-0[é".chars().filter(|char| needs_narrow_glyph(*char, &table, &by_glyph)).collect();
    assert_eq!(needing, "'#az~[");
    // On the game's own disc: every printable ASCII character that it draws as something else.
    if let Some(disc) = game() {
        let table = disc.read(EXECUTABLE, ASCII_TABLE, ASCII_TABLE_SIZE).unwrap();
        let chars = characters(&disc).unwrap();
        let differing: String = (' '..='~').filter(|char| needs_narrow_glyph(*char, &table, &chars.by_glyph)).collect();
        assert_eq!(differing, "\"#$'*;<>@[\\]^_`abcdefghijklmnopqrstuvwxyz|~");
    }
    // The empty glyphs of the main font, but for the first, which is the space.
    let mut font = vec![0; 5 * GLYPH_BYTES];
    font[GLYPH_BYTES + 3] = 1;
    font[4 * GLYPH_BYTES - 1] = 0xF0;
    assert_eq!(free_glyphs(&font), [2, 4]);
}

#[test]
fn the_adapter_declares_itself_properly() {
    assert_eq!(declaration_faults(&ADAPTER), Vec::<String>::new());
    assert!(ADAPTER.kind_of("06307").is_some() && ADAPTER.kind_of("6307").is_none());
    let kind = |section: &str| ADAPTER.kind_of(section).map(|kind| kind.name);
    assert_eq!(
        [kind("06307"), kind("00054"), kind("01298"), kind("01299")].map(Option::unwrap),
        ["dialogue", "skits", "skits", "dialogue"]
    );
    assert_eq!(
        [kind("06797"), kind("06798"), kind("06816"), kind("06817")].map(Option::unwrap),
        ["dialogue", "menus", "menus", "dialogue"]
    );
    assert_eq!((kind("program"), kind("programs")), (Some("lists"), None));
    assert_eq!(
        [kind("08177"), kind("08178"), kind("08727"), kind("08728")].map(Option::unwrap),
        ["dialogue", "enemies", "enemies", "dialogue"]
    );
    assert_eq!(
        [kind("06170"), kind("06171"), kind("06301"), kind("06302")].map(Option::unwrap),
        ["dialogue", "minigames", "minigames", "dialogue"]
    );
    assert!(ADAPTER.kind_of("063070").is_none() && ADAPTER.kind_of("0630a").is_none());
    // The archive's last member is 09202: a number past it is no section of the game.
    assert_eq!([kind("09202"), kind("09203"), kind("99999")], [Some("dialogue"), None, None]);
    assert_eq!(ADAPTER.unmapped("あ{G:9944}い{G:99a4}{G:994}{G:E6C2}{G:9944"), ["9944", "E6C2"]);
    assert!(std::ptr::addr_eq(ADAPTER.platform(), &ps2::PLATFORM));
    assert_eq!(ADAPTER.glyph_shades(), (0..16).map(|level| level * 17).collect::<Vec<u8>>());
    assert_eq!((code_at(0), code_at(GLYPH_COUNT - 1)), (0x9940, 0xE6C2));
    let summary = |narrow: &str, moved, growth| {
        let result = BuildResult {
            strings: 3,
            relocated: 1,
            scripts: 2,
            moved,
            growth,
            narrow: narrow.chars().collect(),
            wide: vec!['한'],
            ..Default::default()
        };
        result.summary()
    };
    let with_skits = BuildResult {
        skit_strings: 4,
        skit_relocated: 2,
        skits: 3,
        menu_strings: 5,
        menus: 2,
        menu_moved: 3,
        list_strings: 6,
        list_moved: 1,
        enemy_names: 7,
        enemy_plates: 5,
        enemy_packs: 4,
        game_strings: 8,
        games: 3,
        ..Default::default()
    };
    assert_eq!(
        with_skits.summary(),
        [
            "dialogue:  0 strings changed in 0 scripts, 0 of them moved",
            "skits:     4 strings changed in 3 skits, 2 of them moved",
            "menus:     5 strings changed in 2 menus, 3 of them moved",
            "lists:     6 strings changed, 1 of them moved",
            "enemies:   7 names changed in 4 packs, and 5 on the battle's name plates",
            "minigames: 8 strings changed in 3 scripts",
        ]
    );
    // With the option skit-pace, how many changed lines were timed and how many type faster.
    let paced = BuildResult {
        skit_strings: 4,
        skit_relocated: 2,
        skits: 3,
        skit_timed: 4,
        skit_faster: 3,
        ..Default::default()
    };
    assert_eq!(
        paced.summary()[1..],
        [
            "skits:     4 strings changed in 3 skits, 2 of them moved",
            "skit pace: 4 changed lines timed, 3 of them typed faster than the game's pace",
        ]
    );
    assert_eq!(
        summary("ab", 7, -64),
        [
            "dialogue:  3 strings changed in 2 scripts, 1 of them moved",
            "font:      3 new characters, 2 at half width and 1 at full width",
            "archive:   -64 bytes, 7 members moved",
        ]
    );
    assert_eq!(summary("", 7, 0)[2], "archive:   +0 bytes, 7 members moved");
    assert_eq!(summary("", 0, 0).len(), 2);
}

#[test]
fn the_archive_and_its_scripts() {
    let Some(disc) = game() else { return };
    assert_eq!(member_span(&disc, 0).unwrap(), (0, 0xC5B));
    assert_eq!(member_span(&disc, 6307).unwrap(), (0x39FDF6C0, 0x821CE));
    assert_eq!(message(member_span(&disc, MEMBER_COUNT)), "FILE.FPB has no member 9203");
    let scripts = script_members(&disc).unwrap();
    assert_eq!((scripts.len(), scripts[0], scripts[scripts.len() - 1]), (461, 6305, 6790));
    let text = read_script_text(&disc, 6307).unwrap();
    let strings = split_strings(&text);
    assert_eq!(strings[4], hex("99 a1 99 b6 99 81 9a 85 99 a1 99 b6 99 81 99 46"));
    assert_eq!(strings[6], hex("07 01 00 00 00 9a 85 9b 44 99 75 99 99 21 21"));
    // The font is one compressed block, and no script.
    assert_eq!(read_member(&disc, FONT_MEMBER).unwrap().len(), GLYPH_COUNT * GLYPH_BYTES);
    assert!(read_script_data(&disc, FONT_MEMBER).is_err());
}

#[test]
fn the_skits() {
    let Some(disc) = game() else { return };
    let skits = skit_scripts(&disc).unwrap();
    let (first, last) = (skits.keys().next().unwrap(), skits.keys().last().unwrap());
    assert_eq!((skits.len(), *first, *last), (609, 54, 1298));
    // Between two skits is a recording of voices, which is none.
    assert!(!skits.contains_key(&55) && !skits.contains_key(&57));
    // Every skit's script is laid out as a story script is, and its code can be read to its end.
    for (index, script) in skits.iter() {
        let (code_start, text_start) = (number(script, 4).unwrap() as usize, number(script, 8).unwrap() as usize);
        assert!(code_start <= text_start && text_start <= script.len(), "skit {index}");
        let references = string_references(&script[code_start..text_start]);
        assert!(references.is_ok(), "skit {index}: {}", message(references));
    }
    assert_eq!(split_strings(text_of(&skits[&60])).len(), 2);
    // A skit's pack written out again from its entries is the game's own, byte for byte: that is
    // what makes it safe to write one out around a changed script.
    for index in skits.keys().copied() {
        let pack = read_member(&disc, index).unwrap();
        let places = pack_entries(&pack).unwrap();
        let entries: Vec<&[u8]> = places.iter().map(|(at, size)| &pack[*at..*at + *size]).collect();
        assert!(pack_of(&entries) == pack, "skit {index}");
        assert!(entries[SKIT_SCRIPT] == skits[&index], "skit {index}");
    }
}

#[test]
fn the_lines_of_the_skits_and_their_times() {
    let Some(disc) = game() else { return };
    let mut lines = Vec::new();
    let mut paces: BTreeMap<u32, usize> = BTreeMap::new();
    let mut end_of_00058 = 0;
    for (index, script) in skit_scripts(&disc).unwrap().iter() {
        let pack = read_member(&disc, *index).unwrap();
        let (at, _) = pack_entries(&pack).unwrap()[SKIT_LIPS];
        let end = number(&pack, at).unwrap() * 2;
        if *index == 58 {
            end_of_00058 = end;
        }
        for line in skit_lines(script, end).unwrap() {
            *paces.entry(line.pace.unwrap()).or_default() += 1;
            // Every line shows a string of its skit, and starts before the skit ends.
            assert!(split_strings(text_of(script)).len() > 1, "skit {index}");
            assert!(line.start.is_none_or(|start| start < line.removed), "skit {index}");
            lines.push((*index, line));
        }
    }
    // Every line has its pace as a plain number; all but one say when they start the same way.
    assert_eq!(lines.len(), 2413);
    let untimed: Vec<usize> = lines.iter().filter(|(_, line)| line.start.is_none()).map(|(index, _)| *index).collect();
    assert_eq!(untimed, [54]);
    assert_eq!(
        paces,
        BTreeMap::from([
            (2, 4),
            (3, 2),
            (4, 4),
            (5, 29),
            (6, 189),
            (7, 512),
            (8, 1586),
            (9, 61),
            (10, 21),
            (11, 3),
            (12, 1),
            (13, 1)
        ])
    );
    // Skit 00058 as seen on screen: its lines start at 1, 250, 520, 1160 and 1310 and type at 8,
    // 6, 8, 8 and 6 frames a letter, and it ends at 1454.
    let of_00058: Vec<(Option<u32>, u32, Option<u32>)> =
        lines.iter().filter(|(index, _)| *index == 58).map(|(_, line)| (line.start, line.removed, line.pace)).collect();
    assert_eq!(end_of_00058, 1454);
    assert_eq!(
        of_00058,
        [
            (Some(1), 520, Some(8)),
            (Some(250), 1160, Some(6)),
            (Some(520), 1310, Some(8)),
            (Some(1160), 1454, Some(8)),
            (Some(1310), 1454, Some(6)),
        ]
    );
}

#[test]
fn the_menus() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    let modules = menu_modules(&disc).unwrap();
    assert_eq!(modules.len(), 22);
    let mut count = 0;
    let mut counts = BTreeMap::new();
    for (index, module) in modules.iter() {
        let about = module_of(*index).unwrap();
        // The compressed copy of a menu's module, 19 members later, is the same module.
        assert_eq!(about.twin.is_some(), MENU_MEMBERS.contains(index));
        if let Some(after) = about.twin {
            assert!(read_member(&disc, index + after).unwrap() == *module, "menu {index}");
        }
        let targets = pointed_at(module, about.base);
        let strings = menu_strings(module, &about, &chars);
        count += strings.len();
        counts.insert(*index, strings.len());
        // All of it is in the module's data.
        assert!(strings.keys().all(|offset| *offset as usize >= code_end(module)), "module {index}");
        for (offset, data) in &strings {
            // Each ends in a zero byte, and what is counted as its room is zero and its own.
            let (at, room) = (*offset as usize, menu_room(module, *offset as usize, data.len(), &targets));
            assert!(room >= data.len() && module[at + data.len()..=at + room].iter().all(|byte| *byte == 0));
            assert!(strings.range(offset + 1..).next().is_none_or(|(next, _)| *next as usize > at + room));
        }
    }
    assert_eq!(count, 716);
    assert_eq!((counts[&QUIZ_MODULE], counts[&BATTLE_MODULE], counts[&FIELD_MODULE]), (2, 102, 25));
    let skills = menu_strings(&modules[&6798], &MENU, &chars);
    assert_eq!(decode(&skills[&22856], &chars), "術・技");
    assert_eq!(decode(&skills[&23272], &chars), "{09:35 00 00 00}使用\n{09:36 00 00 00}戻る\n{09:37 00 00 00}説明切替");

    // The battle's and the field's modules are byte for byte the entries of member 17 that the game
    // loads them from, so their strings are read from members 8055 and 8996 under the same names.
    let (offset, size) = member_span(&disc, COMPRESSED_MODULES).unwrap();
    let member = disc.read(ARCHIVE, offset, size as u64).unwrap();
    let entries = compressed_entries(&member).unwrap();
    assert_eq!(entries.iter().map(|entry| entry.len()).collect::<Vec<_>>(), [210_340, 128_221, 102_358]);
    for (entry, index) in COMPRESSED_ENTRIES {
        assert!(lzss_ring::decompress_block(entries[entry], 0).unwrap() == modules[&index], "entry {entry}");
    }
    // The member is laid out as it is written out again, and fits the memory it is kept in.
    assert!(rebuild_compressed_modules(&member, &BTreeMap::new()).unwrap() == member);
    assert_eq!((member.len(), COMPRESSED_MODULES_LIMIT), (440_940, 441_088));
}

#[test]
fn the_exit_prompt_s_code_is_found_where_the_game_writes_the_name() {
    let code = |text: &str| has_code_at(&encoded(text).unwrap(), EXIT_PROMPT_CODE, 0x0A);
    assert!(code("{09:35 00 00 00} {0A:00 00 00 00}あい"));
    assert!(code("ababab{0A:00 00 00 00}")); // six single bytes before it do as well
    assert!(code("あいあ{0A:00 00 00 00}"));
    assert!(!code("{09:35 00 00 00} Q{0A:00 00 00 00}"));
    assert!(!code("{09:35 00 00 00}{0A:00 00 00 00}"));
    assert!(!code("{09:35 00 00 00} ")); // too short for the code and its number
    assert!(!code("{09:0A 00 00 00} a")); // a $0A inside another code's number is none
    assert!(!code("{0A:00 00 00 00}"));
}

#[test]
fn the_compressed_modules_are_written_out_again_around_a_changed_entry() {
    // Three blocks, the first two stored as they are, the second followed by three unused bytes.
    let block = |data: &[u8], mode: u8| lzss_ring::compress_block(data, mode).unwrap();
    let (first, second, third) =
        (block(b"battle", lzss_ring::MODE_STORED), block(b"no", lzss_ring::MODE_STORED), block(b"field!", 3));
    let mut member = [3u32, 16, 16 + 16, 16 + 16 + 12].iter().flat_map(|word| word.to_le_bytes()).collect::<Vec<_>>();
    for entry in [&first, &second, &third] {
        member.resize(member.len().next_multiple_of(4), 0);
        member.extend_from_slice(entry);
    }
    member.resize(member.len().next_multiple_of(4), 0);
    assert_eq!(compressed_entries(&member).unwrap(), [&first[..], &second[..], &third[..]]);
    assert_eq!(rebuild_compressed_modules(&member, &BTreeMap::new()).unwrap(), member);
    // The first grows by four bytes: it is stored again as it was, and the others keep their bytes
    // and move.
    let rebuilt = rebuild_compressed_modules(&member, &BTreeMap::from([(0, b"battle!!!!".to_vec())])).unwrap();
    let entries = compressed_entries(&rebuilt).unwrap();
    assert_eq!(rebuilt[..16], [3u32, 16, 36, 48].iter().flat_map(|word| word.to_le_bytes()).collect::<Vec<_>>());
    assert_eq!(entries[0], block(b"battle!!!!", lzss_ring::MODE_STORED));
    assert_eq!((entries[1], entries[2]), (&second[..], &third[..]));
    assert_eq!(rebuilt.len(), (48 + third.len()).next_multiple_of(4));
    // A member longer than the memory the game keeps it in is refused.
    let large = BTreeMap::from([(1, vec![0x55; COMPRESSED_MODULES_LIMIT])]);
    assert_eq!(
        message(rebuild_compressed_modules(&member, &large)),
        "the battle's and the field's modules, compressed again with the strings the pack changes in 08055 and \
         08996, take 441148 bytes, and the game keeps them in memory where they have room for 441088, since \
         other data follows them there; shorten some of those strings"
    );
    assert!(rebuild_compressed_modules(&[1, 0, 0, 0, 9, 0, 0, 0], &BTreeMap::new()).is_err());
}

#[test]
fn the_program_s_lists() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    let program = program_file(&disc).unwrap();
    let targets = program_targets(&program);
    let strings = program_strings(&program, &chars);
    assert_eq!(strings.len(), 3259);
    let text = |offset: u32| decode(&strings[&offset], &chars);
    assert_eq!((text(1094112).as_str(), text(1094168).as_str()), ("オレンジグミ", "アップルグミ"));
    assert_eq!(text(1094128), "ゼリー状薬品\n最大ＨＰの３０％回復");
    assert_eq!(text(1255904), "ガルド"); // a label of the main menu
    assert_eq!((text(1044520).as_str(), text(1048184).as_str()), ("フレイムドライブ", "蒼破刃")); // two skills
    // The prompt to leave a town, which fills in the town's name with $0A, beside its way back.
    assert_eq!(text(1109536), "{09:35 00 00 00} {0A:00 00 00 00}を出る");
    assert_eq!(text(1109560), "{09:36 00 00 00} もどる");
    // Without $0A the program's strings are those it had before: that one is the only one $0A adds.
    let without: BTreeMap<u32, Vec<u8>> = strings_among(&program, &targets, |offset| {
        text_length(&program, offset, &chars, &[0x04, 0x05, 0x07, 0x09], true, false)
    });
    let mut with = strings.clone();
    with.remove(&1109536);
    assert_eq!(without, with);
    for (offset, data) in &strings {
        // Each ends in a zero byte, and what is counted as its room is zero and its own.
        let (at, room) = (*offset as usize, menu_room(&program, *offset as usize, data.len(), &targets));
        assert!(room >= data.len() && program[at + data.len()..=at + room].iter().all(|byte| *byte == 0));
        assert!(strings.range(offset + 1..).next().is_none_or(|(next, _)| *next as usize > at + room));
    }
    // All of it is past what else a build changes in the program: the archive's table of
    // contents, and the tables and the texture of the half-width font.
    let first = *strings.keys().next().unwrap() as u64;
    let changed = [TABLE + 4 * (MEMBER_COUNT as u64 + 1), ASCII_TABLE + ASCII_TABLE_SIZE, KANA_TABLE + KANA_TABLE_SIZE];
    assert!(first >= PROGRAM_DATA as u64 && changed.iter().all(|end| *end <= PROGRAM_DATA as u64));
    assert!(HALF_FONT < PROGRAM_DATA as u64);

    // Nearly all of them can move, through the words that point at them and the instructions
    // that build their address: everything that refers to them is known.
    let reach = program_reach(&program, &strings);
    let movable = &reach.movable;
    let count = |of: fn(&Reached) -> usize| movable.values().map(of).sum::<usize>();
    assert_eq!(
        (movable.len(), count(|reached| reached.words.len()), count(|reached| reached.code.len())),
        (3241, 3420, 56)
    );
    // The code builds the address of forty of them. Five are labels of the main menu that a
    // table points at as well, and the others are known to the code alone.
    let code = program_code_use(&program);
    let built = |offset: &u32| code.built.contains_key(&(offset + PROGRAM_BASE));
    let by_code: Vec<u32> = strings.keys().copied().filter(built).collect();
    assert_eq!(by_code.len(), 40);
    let both: Vec<u32> = by_code
        .iter()
        .copied()
        .filter(|offset| movable.get(offset).is_some_and(|reached| !reached.words.is_empty()))
        .collect();
    assert_eq!(both, [1140944, 1140960, 1140976, 1141496, 1141504]);
    assert_eq!((text(1140944).as_str(), text(1141504).as_str()), ("移動速度", "攻撃"));
    // Words the code puts into a sentence, and a message of the main menu.
    assert_eq!((text(1139576).as_str(), text(1139944).as_str()), ("を料理しました", "％回復\n"));
    assert_eq!(text(1256176), "戦闘メンバーに戦闘可能なキャラがいません");
    assert_eq!(movable[&1139576], Reached { words: vec![], code: vec![0x58234, 0x58258] });
    // Each can go wherever its high half reaches, and all of them are within that now.
    let reaches = |offset: &u32| within_reach(&program, PROGRAM_BASE, *offset as usize, &movable[offset].code);
    assert_eq!(reaches(&1139576), 1085440..1150976);
    assert!(by_code.iter().filter(|offset| movable.contains_key(offset)).all(|offset| {
        let reach = reaches(offset);
        reach.len() == 0x10000 && reach.contains(&(*offset as usize))
    }));
    // One the code copies eleven bytes of, eight of them read straight from memory, stays.
    assert_eq!(reach.read_straight, BTreeSet::from([1064008]));
    assert_eq!(text(1064008), "名称未設定");
    assert!(!movable.contains_key(&1064008));
    assert_eq!(strings.len() - movable.len(), 18);
    // So does one at $200020: 120 words hold that number as a size of 32 by 32.
    assert_eq!(text(1052704), "晶術追撃");
    assert!(!movable.contains_key(&1052704));
    // The pointers are where a table of pointers is, and nowhere else that a build changes.
    let table = TABLE as usize..TABLE as usize + 4 * (MEMBER_COUNT + 1);
    let pointers = || movable.values().flat_map(|reached| &reached.words);
    assert!(pointers().all(|at| PROGRAM_TABLES.contains(at) && !table.contains(at)));
    let font_size = lzss_ring::HEADER_SIZE + lzss_ring::read_header(&program, HALF_FONT as usize).unwrap().1;
    let font = ASCII_TABLE as usize..HALF_FONT as usize + font_size; // the tables of the half-width font, and it
    assert!(pointers().all(|at| !font.contains(at)));
}

#[test]
fn the_enemies() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    let enemies = enemy_names(&disc).unwrap();
    let count = enemies.values().map(BTreeMap::len).sum::<usize>();
    let (first, last) = (enemies.keys().next().unwrap(), enemies.keys().last().unwrap());
    assert_eq!((enemies.len(), count, *first, *last), (550, 933, 8178, 8727));
    let name = |pack: usize, entry: u32| decode(&enemies[&pack][&entry], &chars);
    assert_eq!((name(8211, 1).as_str(), name(8211, 4).as_str()), ("スライム", "コボルト"));
    // No name is empty, and each reads as characters of the game's list and nothing else.
    let names = enemies.values().flat_map(BTreeMap::values);
    assert!(names.clone().all(|name| !name.is_empty() && !decode(name, &chars).contains('{')));
    assert_eq!(names.map(Vec::len).max(), Some(20));
    // A pack written out again with nothing renamed is the game's own, byte for byte: that is
    // what makes it safe to write one out around a renamed enemy.
    for index in enemies.keys().copied() {
        let (offset, size) = member_span(&disc, index).unwrap();
        let pack = disc.read(ARCHIVE, offset, size as u64).unwrap();
        assert!(rename_enemies(&pack, &BTreeMap::new(), "x").unwrap() == pack, "pack {index}");
        // In a record, what follows the name in its field is zero.
        for (_, record) in enemy_records(&pack) {
            let at = enemy_name_place(&record).unwrap();
            let name = enemy_name(&record).unwrap().len();
            assert!(record[at + name..at + ENEMY_NAME_FIELD].iter().all(|byte| *byte == 0), "pack {index}");
        }
    }

    // The name plate's list: every enemy of every pack is on it, under the same name, and every
    // name on it is some enemy's. The Owlbear of the first battle is its first.
    let (offset, size) = member_span(&disc, PLATE_MEMBER).unwrap();
    let member = disc.read(ARCHIVE, offset, size as u64).unwrap();
    let (groups, list) = plate_lists(&member).unwrap();
    assert_eq!(((groups.len() - 4) / GROUP_SIZE, (list.len() - 4) / PLATE_SIZE), (550, 256));
    let kinds = plate_kinds(&enemies, &groups, &list).unwrap();
    assert_eq!(
        (kinds.len(), kinds[&(8208, 1)], decode(plate_name(&list, 0).unwrap(), &chars).as_str()),
        (933, 0, "オウルベア")
    );
    let named = (0..256).filter(|kind| !plate_name(&list, *kind).unwrap().is_empty());
    assert!(named.clone().all(|kind| kinds.values().any(|used| usize::from(*used) == kind)));
    assert_eq!(named.count(), 218);
    // Each name on it is padded with spaces to the end of its field, and a zero ends the field.
    for kind in 0..256 {
        let at = 4 + kind * PLATE_SIZE;
        let name = plate_name(&list, kind).unwrap().len();
        assert!(list[at + name..at + PLATE_FIELD].iter().all(|byte| *byte == PLATE_PADDING), "kind {kind}");
        assert_eq!(list[at + PLATE_FIELD], 0, "kind {kind}");
    }
    // Written out again with nothing renamed, it is the game's own.
    assert!(rename_plates(&member, &BTreeMap::new()).unwrap() == member);
}

#[test]
fn the_games_within_the_game() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    let scripts = game_scripts(&disc).unwrap();
    let (mut with_text, mut count) = (0, 0);
    for (index, script) in scripts.iter() {
        let data = game_data(script).unwrap();
        let strings = game_strings(script, &chars);
        with_text += usize::from(!strings.is_empty());
        count += strings.len();
        for (offset, string) in &strings {
            // Each is in the data, ends in a zero byte, and stops before the next one starts.
            let (at, end) = (*offset as usize, *offset as usize + string.len());
            assert!(data.contains(&at) && script[end] == 0, "script {index}");
            assert!(strings.range(offset + 1..).next().is_none_or(|(next, _)| *next as usize > end));
        }
    }
    assert_eq!((scripts.len(), with_text, count), (131, 96, 5732));
    let quiz = game_strings(&scripts[&6178], &chars);
    assert_eq!(decode(&quiz[&17817], &chars), "よし、行くぞ");
    assert_eq!(decode(&quiz[&17830], &chars), "%sには簡単過ぎるかもね");
}

#[test]
fn the_character_list_and_font() {
    let Some(disc) = game() else { return };
    let chars = characters(&disc).unwrap();
    assert_eq!(chars.by_glyph.len(), GLYPH_COUNT);
    assert_eq!(chars.by_glyph[..8].iter().flatten().collect::<String>(), "　，．・：？！ー");
    assert_eq!(chars.glyphs.get(&'急').copied(), glyph_of(0x9B44));
    assert_eq!(glyph(&disc, 0).unwrap(), vec![vec![0; 24]; 24]); // the blank
    let comma = glyph(&disc, 1).unwrap();
    assert_eq!(comma[12][..6], [0, 0, 0, 1, 1, 1]); // the top of the comma, low in the left of its cell
    assert!(comma[..12].iter().chain(&comma[22..]).all(|row| row.iter().all(|pixel| *pixel == 0)));
    assert_eq!(message(glyph(&disc, GLYPH_COUNT)), "the game's font has no glyph 2560");
}

#[test]
fn the_party_s_names_are_where_a_new_game_takes_them_from() {
    let Some(disc) = game() else { return };
    let program = program_file(&disc).unwrap();
    let chars = characters(&disc).unwrap();
    // For each member a word pointing at the name, then one at the surname: strings of the program.
    let word = |at: usize| u32::from_le_bytes(program[at..at + 4].try_into().unwrap()) - PROGRAM_BASE;
    let names: Vec<u32> = (0..PARTY_MEMBERS).map(|member| word(PARTY_NAME_TABLE + 8 * member)).collect();
    assert_eq!(names, [1064000, 1063976, 1063960, 1063944, 1063928, 1063896]);
    let strings = program_strings(&program, &chars);
    let text = |offset: &u32| decode(&strings[offset], &chars);
    assert_eq!(
        names.iter().map(text).collect::<Vec<_>>(),
        ["カイル", "リアラ", "ロニ", "ジューダス", "ナナリー", "ハロルド"]
    );
    assert_eq!(text(&word(PARTY_NAME_TABLE + 4)), "デュナミス"); // Kyle's surname
    // A pack that keeps the game's names has them counted as drawn, two columns a letter.
    assert_eq!(party_name_columns(&program, &HashMap::new(), &chars), [6, 6, 4, 10, 8, 8]);
    // A pack's own names where it gives them; one with a tag in it is not measured.
    let pack = HashMap::from([(1064000, "Kyle"), (1063896, "Harold"), (1063976, "{G:0001}"), (1063880, "X")]);
    assert_eq!(party_name_columns(&program, &pack, &chars), [4, NAME_COLUMNS, 4, 10, 8, 6]);
}

#[test]
fn a_party_member_s_name_longer_than_its_field_in_the_save_is_refused() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // Thirteen narrow letters, and seven wide ones taking fourteen bytes: each is named.
    let pack = script_of(&[("program", 1064000, "KYLE THE BOLD"), ("program", 1063944, "ジューダスさん")]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "string program.1063944 is a party member's name, which a new game copies into the member's record in \
         the save, where a name has room for 12 bytes: this one takes 14 (a wide character takes two bytes where \
         a narrow one takes one), so shorten it\n\
         string program.1064000 is a party member's name, which a new game copies into the member's record in \
         the save, where a name has room for 12 bytes: this one takes 13 (a wide character takes two bytes where \
         a narrow one takes one), so shorten it"
    );
    assert!(!out.exists(), "a refused build left a file behind");
    // Twelve fill the field with the zero byte after them, so they are no name too long; alone they
    // still need more room than カイル had among the lists that can move.
    let twelve = message(build(&disc, &script_of(&[("program", 1064000, "KYLE DUNAMIS")]), &out, &[]));
    assert!(twelve.starts_with("string program.1064000 found no place: "), "{twelve}");
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn extract_and_verify() {
    let Some(disc) = game() else { return };
    let mut texts = extract(&disc).unwrap();
    let strings = texts.values().map(BTreeMap::len).sum::<usize>();
    assert_eq!((texts.len(), strings), (461 + 609 + 22 + 1 + 550 + 96, 39929 + 3022 + 716 + 3259 + 933 + 5732));
    assert_eq!(texts["program"][&1094168], "アップルグミ");
    assert_eq!(texts["06816"][&4136], "ゲーム開始");
    assert_eq!(texts["00060"].len(), 2);
    assert_eq!(texts["00060"][&0], "");
    assert_eq!(texts["06307"][&4], "はやく、はやく！");
    assert_eq!(texts["06307"][&6], "{NAME:01 00 00 00}、急いで!!");
    assert!(texts.values().flat_map(BTreeMap::values).all(|text| ADAPTER.unmapped(text).is_empty()));
    verify_script(&disc, &texts).unwrap();
    texts.get_mut("06307").unwrap().get_mut(&4).unwrap().push('！');
    assert_eq!(
        message(verify_script(&disc, &texts)),
        "string 06307.004 does not survive a round trip through its script file"
    );
}

#[test]
fn build_changes_strings_and_moves_what_follows() {
    let Some(disc) = game() else { return };
    // The folder is made first, so that it is removed last: after the images in it are closed.
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let pack = script_of(&[("06307", 4, "てすと、てすと、てすと！"), ("06307", 6, "{NAME:01 00 00 00} HURRY UP!")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[0], "dialogue:  2 strings changed in 1 scripts, 1 of them moved");
    assert_eq!((result.moved, result.growth), (2895, -64)); // the script packs smaller, by one step of 64
    {
        let built = Disc::open(&out).unwrap();
        let chars = characters(&built).unwrap();
        let (old, new) = (read_script_data(&disc, 6307).unwrap(), read_script_data(&built, 6307).unwrap());
        let (code_start, text_start) = (number(&new, 4).unwrap() as usize, number(&new, 8).unwrap() as usize);
        assert_eq!((number(&old, 4), number(&old, 8)), (number(&new, 4), number(&new, 8)));
        let mut before = string_references(&old[code_start..text_start]).unwrap();
        let mut after = string_references(&new[code_start..text_start]).unwrap();
        // The string that grew went to the end of the text, and the one reference to it followed.
        let moved_to = old.len() - text_start;
        assert!(!after.contains_key(&0x24));
        assert_eq!(before.remove(&0x24), after.remove(&moved_to));
        assert_eq!(before, after);
        let text = &new[text_start..];
        let moved = text[moved_to..].split(|byte| *byte == 0).next().unwrap();
        assert_eq!(decode(moved, &chars), "てすと、てすと、てすと！");
        assert_eq!(text[0x24..0x24 + 17], [0; 17]); // where it was
        assert_eq!(decode(&text[0x3C..0x3C + 15], &chars), "{NAME:01 00 00 00} HURRY UP!"); // this one fitted
        assert_eq!(built.size(ARCHIVE).unwrap(), disc.size(ARCHIVE).unwrap() - 64);
        // Members before the change, and after it, moved.
        for index in [0, 19, 6306, 6308, 9202] {
            let (old_offset, size) = member_span(&disc, index).unwrap();
            let (new_offset, new_size) = member_span(&built, index).unwrap();
            assert_eq!((new_offset, new_size), (old_offset - if index > 6307 { 64 } else { 0 }, size));
            let (old, new) =
                (disc.read(ARCHIVE, old_offset, size as u64), built.read(ARCHIVE, new_offset, size as u64));
            assert!(old.unwrap() == new.unwrap());
        }
    }
    fs_remove(&out);
    // With no font given, a character the game's font lacks is drawn from the game's own fonts.
    let drawn = build(&disc, &script_of(&[("06307", 4, "é")]), &out, &[]).unwrap();
    let font_line = "font:      1 new characters, 1 at half width and 0 at full width".to_string();
    assert!(drawn.summary().contains(&font_line), "{:?}", drawn.summary());
    fs_remove(&out);
    assert_eq!(
        message(build(&disc, &script_of(&[("99999", 0, "あ")]), &out, &[])),
        "99999 is not a part of this game's script"
    );
    assert_eq!(
        message(build(&disc, &script_of(&[("06307", 4, "{ZZ}")]), &out, &[])),
        "string 06307.004: unknown tag {ZZ}"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn build_changes_a_skit_inside_its_pack() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let longer = "てすと、てすと、てすと！".repeat(8);
    let result = build(&disc, &script_of(&[("00058", 1, &longer), ("00058", 4, "みじかい")]), &out, &[]).unwrap();
    assert_eq!(
        result.summary()[..2],
        [
            "dialogue:  0 strings changed in 0 scripts, 0 of them moved",
            "skits:     2 strings changed in 1 skits, 1 of them moved",
        ]
    );
    {
        let built = Disc::open(&out).unwrap();
        let (old, new) = (read_member(&disc, 58).unwrap(), read_member(&built, 58).unwrap());
        let (old_places, places) = (pack_entries(&old).unwrap(), pack_entries(&new).unwrap());
        let part = |pack: &[u8], (at, size): (usize, usize)| pack[at..at + size].to_vec();
        // Everything in the pack but the script is as it was, and after the script it starts later.
        for entry in (0..SKIT_ENTRIES).filter(|entry| *entry != SKIT_SCRIPT) {
            assert!(part(&old, old_places[entry]) == part(&new, places[entry]), "entry {entry}");
            assert_eq!(places[entry].0 > old_places[entry].0, entry > SKIT_SCRIPT);
        }
        assert!(pack_of(&places.iter().map(|place| &new[place.0..place.0 + place.1]).collect::<Vec<_>>()) == new);
        // The string that grew is now the last of the skit's text, and the one that fitted is where
        // it was. The room the first left reads as empty strings, so the numbers are not what they were.
        let texts = extract(&built).unwrap();
        let strings: Vec<&str> = texts["00058"].values().map(String::as_str).collect();
        assert_eq!(strings.last(), Some(&longer.as_str()));
        assert_eq!(strings.iter().filter(|text| **text == "みじかい").count(), 1);
        // The story scripts, far after it in the archive, are what they were.
        assert_eq!(texts["06307"], extract(&disc).unwrap()["06307"]);
    }
    fs_remove(&out);
}

/// The option `skit-pace` alone.
const PACED: Switches = Switches { skit_pace: true, narrow_spell_names: false };

#[test]
fn build_draws_spells_narrow_in_the_grids_of_artes_only_when_asked() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // The game's own modules mark a spell's row there, and do it nowhere else in that function.
    for (index, at) in SPELL_FLAGS {
        assert_eq!(number(&read_member(&disc, index).unwrap(), at), Some(MARK_LARGE), "menu {index}");
    }
    // A pack that changes nothing, with the option: the two instructions change and nothing else,
    // in each module and its compressed twin.
    let narrow = Switches { narrow_spell_names: true, ..Switches::default() };
    let result = build_as(&disc, &Script::new(), &out, &[], true, narrow).unwrap();
    assert!(result.summary().contains(&"spells:    drawn in the narrow font in 2 grids of artes".to_string()));
    {
        let built = Disc::open(&out).unwrap();
        for (index, at) in SPELL_FLAGS {
            let (old, new) = (read_member(&disc, index).unwrap(), read_member(&built, index).unwrap());
            assert_eq!(number(&new, at), Some(LEAVE_AS_IT_IS));
            assert!(
                old.len() == new.len()
                    && (0..old.len()).all(|byte| (at..at + 4).contains(&byte) || old[byte] == new[byte])
            );
            assert!(read_member(&built, index + MENU_TWIN).unwrap() == new);
        }
        // The text is all as it was.
        assert!(extract(&built).unwrap() == extract(&disc).unwrap());
    }
    fs_remove(&out);
    // Without it, a pack that changes nothing gives back the game's own image.
    let result = build_as(&disc, &Script::new(), &out, &[], true, Switches::default()).unwrap();
    assert!(!result.summary().iter().any(|line| line.starts_with("spells")));
    assert_eq!(result.moved, 0);
    fs_remove(&out);
}

#[test]
fn build_types_a_changed_skit_line_faster_only_when_asked() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let code_of_00058 = |disc: &Disc| {
        let script = skit_scripts(disc).unwrap()[&58].clone();
        script[number(&script, 4).unwrap() as usize..number(&script, 8).unwrap() as usize].to_vec()
    };
    let built_code = || code_of_00058(&Disc::open(&out).unwrap());
    let own = code_of_00058(&disc);
    let code_start = number(&skit_scripts(&disc).unwrap()[&58], 4).unwrap() as usize;
    let first = &skit_lines(&skit_scripts(&disc).unwrap()[&58], 1454).unwrap()[0];
    let pace_at = first.pace_at - code_start;
    // 64 letters in 519 frames, where the game types 27 at 8 frames a letter: 6 shows it whole.
    let pack = script_of(&[("00058", 1, "Let's have a wander round town, then head back to the orphanage.")]);

    // Without the option the code is the game's but for where the line's string now is.
    let result = build_as(&disc, &pack, &out, &[], true, Switches::default()).unwrap();
    assert!(!result.summary().iter().any(|line| line.starts_with("skit pace")));
    let plain = built_code();
    assert_eq!(plain[pace_at..pace_at + 2], hex("90 08"));
    let differing: Vec<usize> = (0..own.len()).filter(|at| plain[*at] != own[*at]).collect();
    assert_eq!((own[pace_at + 2], plain.len()), (STRING_REFERENCE, own.len()));
    assert!(!differing.is_empty() && differing.iter().all(|at| [pace_at + 3, pace_at + 4].contains(at)));
    // With it, the line types at 6 frames a letter, and nothing else in the code changes.
    let result = build_as(&disc, &pack, &out, &[], true, PACED).unwrap();
    assert_eq!(result.summary()[2], "skit pace: 1 changed lines timed, 1 of them typed faster than the game's pace");
    let paced = built_code();
    let mut expected = plain.clone();
    expected[pace_at + 1] = 6;
    assert_eq!(paced, expected);
    // A line that does not fit even at 5 frames a letter is refused, and nothing is written.
    fs_remove(&out);
    let pack = script_of(&[("00058", 1, &"x".repeat(200))]);
    let refused = message(build_as(&disc, &pack, &out, &[], true, PACED));
    assert!(refused.starts_with("skit line 00058.001 has 200 letters, and at most 88 fit: "), "{refused}");
    assert!(!out.exists());
}

#[test]
fn build_changes_a_menu_s_text_where_it_is() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // The first takes all the room there is: the ten bytes of the game's own and five unused ones.
    let pack = script_of(&[("06816", 4136, "START THE GAME!"), ("06816", 4224, "CHAIN")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "menus:     2 strings changed in 1 menus, 0 of them moved");
    assert_eq!((result.moved, result.growth), (0, 0)); // the module is as long as it was
    {
        let built = Disc::open(&out).unwrap();
        let (old, new) = (read_member(&disc, 6816).unwrap(), read_member(&built, 6816).unwrap());
        assert_eq!(new[4136..4152], *b"START THE GAME!\0");
        assert_eq!(new[4224..4232], *b"CHAIN\0\0\0");
        // Nothing else in the module changed, and its compressed copy is the same module.
        let same = |at: usize| (4136..4152).contains(&at) || (4224..4232).contains(&at) || old[at] == new[at];
        assert!(old.len() == new.len() && (0..old.len()).all(same));
        assert!(read_member(&built, 6816 + MENU_TWIN).unwrap() == new);
        assert!(lzss_ring::is_block(&{
            let (offset, size) = member_span(&built, 6816 + MENU_TWIN).unwrap();
            built.read(ARCHIVE, offset, size as u64).unwrap()
        }));
        // Read from the built image, the menu still has every other string under its number. The two
        // that changed hold no character of the game's list any more, so they no longer read as its text.
        let (before, after) = (extract(&disc).unwrap(), extract(&built).unwrap());
        let rest = |texts: &Script| {
            let kept = texts["06816"].iter().filter(|(offset, _)| ![4136, 4224].contains(*offset));
            kept.map(|(offset, text)| (*offset, text.clone())).collect::<Vec<_>>()
        };
        assert_eq!(rest(&before), rest(&after));
        assert!(!after["06816"].contains_key(&4136) && !after["06816"].contains_key(&4224));
    }
    fs_remove(&out);
    // A string of the field's module, which is no menu, stays where it is and has its room and no more.
    assert_eq!(
        message(build(&disc, &script_of(&[("08996", 178456, "OUTER SHELL")]), &out, &[])),
        "string 08996.178456 takes 11 bytes and has room for 7: this text stays where it is, and a wide \
         character takes two bytes where a narrow one takes one"
    );
    assert_eq!(
        message(build(&disc, &script_of(&[("06816", 4137, "X")]), &out, &[])),
        "string 06816.4137 does not exist"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

/// Member 17 of an image, as it is stored, and its entries expanded.
fn compressed_modules(disc: &Disc) -> (Vec<u8>, Vec<Vec<u8>>) {
    let (offset, size) = member_span(disc, COMPRESSED_MODULES).unwrap();
    let member = disc.read(ARCHIVE, offset, size as u64).unwrap();
    let entries = compressed_entries(&member).unwrap().iter().map(|entry| entry.to_vec()).collect();
    (member, entries)
}

#[test]
fn build_writes_the_battle_s_and_the_field_s_text_where_the_game_loads_it() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let (member, entries) = compressed_modules(&disc);
    // The banner of a battle that was fled: the battle's module changes, in member 8055 and in
    // entry 0 of member 17, which is compressed again; the other two entries keep their bytes.
    let pack = script_of(&[("08055", 340160, "RAN AWAY...")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "menus:     1 strings changed in 1 menus, 0 of them moved");
    {
        let built = Disc::open(&out).unwrap();
        let (old, new) = (read_member(&disc, BATTLE_MODULE).unwrap(), read_member(&built, BATTLE_MODULE).unwrap());
        assert_eq!(new[340160..340176], *b"RAN AWAY...\0\0\0\0\0");
        assert!(
            old.len() == new.len() && (0..old.len()).all(|at| (340160..340176).contains(&at) || old[at] == new[at])
        );
        let (_, built_entries) = compressed_modules(&built);
        assert!(lzss_ring::decompress_block(&built_entries[0], 0).unwrap() == new);
        assert_eq!(lzss_ring::read_header(&built_entries[0], 0).unwrap().0, lzss_ring::MODE_LZSS_RLE);
        assert!(built_entries[1] == entries[1] && built_entries[2] == entries[2]);
        // The field's module is as it was.
        assert!(read_member(&built, FIELD_MODULE).unwrap() == read_member(&disc, FIELD_MODULE).unwrap());
    }
    fs_remove(&out);
    // A place name of the field's module: entry 2 is compressed again, and entry 0 keeps its bytes.
    build(&disc, &script_of(&[("08996", 180672, "FOG")]), &out, &[]).unwrap();
    {
        let built = Disc::open(&out).unwrap();
        let new = read_member(&built, FIELD_MODULE).unwrap();
        assert_eq!(new[180672..180676], *b"FOG\0");
        let (_, built_entries) = compressed_modules(&built);
        assert!(lzss_ring::decompress_block(&built_entries[2], 0).unwrap() == new);
        assert!(built_entries[0] == entries[0] && built_entries[1] == entries[1]);
    }
    fs_remove(&out);
    // A pack that changes neither module, or gives one of their strings the text it had, leaves member
    // 17 as the game has it: it is not compressed again.
    let same = extract(&disc).unwrap()["08055"][&340128].clone();
    assert_eq!(same, "作戦");
    let pack = script_of(&[("06816", 4224, "CHAIN"), ("08055", 340128, &same)]);
    build(&disc, &pack, &out, &[]).unwrap();
    {
        let built = Disc::open(&out).unwrap();
        assert!(compressed_modules(&built).0 == member);
    }
}

#[test]
fn build_lets_a_menu_s_strings_grow_past_its_module_s_end() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let chars = characters(&disc).unwrap();
    let modules = menu_modules(&disc).unwrap();
    // What can move, in the 19 menus: nearly all of their strings.
    let reach = |index: usize| module_reach(&modules[&index], &MENU, &menu_strings(&modules[&index], &MENU, &chars));
    let counts = MENU_MEMBERS.fold((0, 0, 0, 0), |(strings, movable, words, code), index| {
        let found = reach(index);
        let count = |of: fn(&Reached) -> usize| found.values().map(of).sum::<usize>();
        let all = menu_strings(&modules[&index], &MENU, &chars).len();
        (strings + all, movable + found.len(), words + count(|r| r.words.len()), code + count(|r| r.code.len()))
    });
    // 586 of 587, through 468 words and 299 instructions.
    assert_eq!(counts, (587, 586, 468, 299));
    // The largest of them is the limit, so it alone has no room to grow by.
    assert_eq!(MENU_MEMBERS.map(|index| modules[&index].len()).max(), Some(MENU_MODULE_LIMIT));
    assert_eq!(modules[&6806].len(), MENU_MODULE_LIMIT);

    // One byte more than its room has: the string goes to the end of the module, which grows by
    // the string and the zero byte that ends it.
    let old = read_member(&disc, 6816).unwrap();
    let movable = reach(6816);
    let result = build(&disc, &script_of(&[("06816", 4136, "START THE GAME!!")]), &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "menus:     1 strings changed in 1 menus, 1 of them moved");
    {
        let built = Disc::open(&out).unwrap();
        let new = read_member(&built, 6816).unwrap();
        assert_eq!((old.len(), new.len()), (6920, 6937));
        assert_eq!(new[6920..], *b"START THE GAME!!\0");
        assert!(new[4136..4152].iter().all(|byte| *byte == 0));
        // What referred to it refers to its new place, and nothing else in the module changed.
        let reached = &movable[&4136];
        assert_eq!(*reached, Reached { words: vec![2632], code: vec![] });
        let mut ours: BTreeSet<usize> = (4136..4152).collect();
        for word in &reached.words {
            assert_eq!(number(&old, *word), Some(MENU_BASE + 4136));
            assert_eq!(number(&new, *word), Some(MENU_BASE + 6920));
            ours.extend(*word..*word + 4);
        }
        for instruction in &reached.code {
            let (was, is) = (number(&old, *instruction).unwrap(), number(&new, *instruction).unwrap());
            assert_eq!((was >> 16, was & 0xFFFF), (is >> 16, (MENU_BASE + 4136) & 0xFFFF));
            assert_eq!(is & 0xFFFF, (MENU_BASE + 6920) & 0xFFFF);
            ours.extend(*instruction..*instruction + 4);
        }
        assert!((0..old.len()).all(|at| ours.contains(&at) || old[at] == new[at]));
        // Its compressed copy is the same module. Seventeen bytes more fit in the padding that
        // follows a member of the archive, so nothing after it had to move.
        assert!(read_member(&built, 6816 + MENU_TWIN).unwrap() == new);
        // Every other string of the menu is still read under its number.
        let (before, after) = (extract(&disc).unwrap(), extract(&built).unwrap());
        let rest = |texts: &Script| {
            let kept = texts["06816"].iter().filter(|(offset, _)| **offset != 4136);
            kept.map(|(offset, text)| (*offset, text.clone())).collect::<Vec<_>>()
        };
        assert_eq!(rest(&before), rest(&after));
    }
    fs_remove(&out);

    // The largest menu cannot grow: its strings share the room they had, and no more.
    let pack = script_of(&[("06806", 59632, "OTHER CHATS OF THE JOURNEY"), ("06806", 59648, "CH. 14")]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "string 06806.59648 found no place: the strings of menu 06806 that a pack changes share the room they \
         had between them and the 0 bytes their module may grow by, 32 bytes here with the zero byte that ends \
         each, and this pack's need 34; the largest piece left has 5. Shorter text in any of them makes room\n\
         \x20 06806.59648 needs 7 bytes"
    );
    assert!(!out.exists(), "a refused build left a file behind");
    // Within that room one may be longer where its neighbour is shorter, and the neighbour moves up.
    let pack = script_of(&[("06806", 59632, "OTHER CHATS, ALL"), ("06806", 59648, "CH. 14")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "menus:     2 strings changed in 1 menus, 1 of them moved");
    {
        let built = Disc::open(&out).unwrap();
        let new = read_member(&built, 6806).unwrap();
        assert_eq!(new.len(), MENU_MODULE_LIMIT);
        assert_eq!(new[59632..59664], *b"OTHER CHATS, ALL\0CH. 14\0\0\0\0\0\0\0\0\0");
    }
    fs_remove(&out);
}

#[test]
fn build_changes_the_program_s_text_where_it_is() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // The name of an item, which has twelve bytes and three unused ones after it, and its neighbour's.
    let pack = script_of(&[("program", 1094168, "APPLE GUMMY ONE"), ("program", 1094112, "ORANGE")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "lists:     2 strings changed, 0 of them moved");
    assert_eq!((result.moved, result.growth), (0, 0)); // nothing in the archive changed
    {
        let built = Disc::open(&out).unwrap();
        let size = disc.size(EXECUTABLE).unwrap();
        let (old, new) = (disc.read(EXECUTABLE, 0, size).unwrap(), built.read(EXECUTABLE, 0, size).unwrap());
        assert_eq!(new[1094168..1094184], *b"APPLE GUMMY ONE\0");
        assert_eq!(new[1094112..1094128], *b"ORANGE\0\0\0\0\0\0\0\0\0\0");
        let same =
            |at: usize| (1094168..1094184).contains(&at) || (1094112..1094128).contains(&at) || old[at] == new[at];
        assert!(old.len() == new.len() && (0..old.len()).all(same));
    }
    fs_remove(&out);
    // The prompt to leave a town, with the town's name the game fills in, which $0A lets be found. It
    // fits its room, so it stays where it was and the word of the table that points at it is the same.
    let pack = script_of(&[("program", 1109536, "{09:35 00 00 00} {0A:00 00 00 00} QUITTER")]);
    build(&disc, &pack, &out, &[]).unwrap();
    {
        let built = Disc::open(&out).unwrap();
        let size = disc.size(EXECUTABLE).unwrap();
        let (old, new) = (disc.read(EXECUTABLE, 0, size).unwrap(), built.read(EXECUTABLE, 0, size).unwrap());
        let wanted = [&hex("09 35 00 00 00 20 0A 00 00 00 00")[..], b" QUITTER\0"].concat();
        let chars = characters(&built).unwrap();
        assert_eq!(number(&new, 0xC7100), Some(0x10EE20 + PROGRAM_BASE));
        assert_eq!(new[0x10EE20..0x10EE20 + wanted.len()], wanted);
        assert_eq!(decode(&wanted[..wanted.len() - 1], &chars), pack["program"][&1109536]);
        // Nothing else of the program changed but the string's room and that word.
        let ours = |at: usize| (0x10EE20..0x10EE38).contains(&at) || (0xC7100..0xC7104).contains(&at);
        assert!(old.len() == new.len() && (0..old.len()).all(|at| ours(at) || old[at] == new[at]));
    }
    fs_remove(&out);
    // The game writes the town's name into the number of the $0A at the prompt's seventh byte, so the
    // code must stay there.
    let elsewhere = script_of(&[("program", 1109536, "{09:35 00 00 00} QUITTER {0A:00 00 00 00}")]);
    assert_eq!(
        message(build(&disc, &elsewhere, &out, &[])),
        "string program.1109536 is the prompt to leave a town, and the game writes the town's name into the number \
         of a {0A} at the string's seventh byte, where the game's own has it: start it as the game's own does, \
         with {09:35 00 00 00} {0A:00 00 00 00}, and put the words after the name"
    );
    // A string alone in its pack has no more than its own room.
    assert_eq!(
        message(build(&disc, &script_of(&[("program", 1094168, "APPLE GUMMY ONE!")]), &out, &[])),
        "string program.1094168 found no place: the strings of the lists that a pack changes share the room \
         they had between them, 16 bytes here with the zero byte that ends each, and this pack's need 17; the \
         largest piece left has 16. Shorter text in any of them makes room\n\
         \x20 program.1094168 needs 17 bytes"
    );
    assert_eq!(
        message(build(&disc, &script_of(&[("program", 1094169, "X")]), &out, &[])),
        "string program.1094169 does not exist"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn build_lets_the_program_s_lists_share_their_room() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    let chars = characters(&disc).unwrap();
    let program = program_file(&disc).unwrap();
    let movable = program_reach(&program, &program_strings(&program, &chars)).movable;
    // An item's name, its description and the next item's name are side by side: 16, 40 and 16
    // bytes. The first takes most of that, so the third goes to the room a skill's name, far
    // from them, leaves.
    let long = "ORANGE GUMMY OF THE FAR EASTERN ISLANDS, VERY SWEET";
    let pack = script_of(&[
        ("program", 1094112, long),
        ("program", 1094128, "HEALS 30% HP"),
        ("program", 1094168, "APPLE GUMMY"),
        ("program", 1042472, "X"),
    ]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "lists:     4 strings changed, 2 of them moved");
    {
        let built = Disc::open(&out).unwrap();
        let size = disc.size(EXECUTABLE).unwrap();
        let (old, new) = (disc.read(EXECUTABLE, 0, size).unwrap(), built.read(EXECUTABLE, 0, size).unwrap());
        let mut stretch = [long.as_bytes(), b"\0HEALS 30% HP\0"].concat();
        stretch.resize(72, 0);
        assert_eq!(new[1094112..1094184], stretch);
        assert_eq!(new[1042472..1042488], *b"X\0APPLE GUMMY\0\0\0");
        // Every pointer at a string that moved has its new address, and nothing else changed.
        let moved = [(1094128, 1094112 + long.len() + 1), (1094168, 1042474)];
        let mut words = BTreeSet::new();
        for (from, to) in moved {
            assert!(!movable[&from].words.is_empty() && movable[&from].code.is_empty());
            for word in &movable[&from].words {
                assert_eq!(number(&old, *word), Some(from + PROGRAM_BASE));
                assert_eq!(number(&new, *word), Some(to as u32 + PROGRAM_BASE));
                words.extend(*word..*word + 4);
            }
        }
        let ours =
            |at: &usize| (1094112..1094184).contains(at) || (1042472..1042488).contains(at) || words.contains(at);
        assert!(old.len() == new.len() && (0..old.len()).all(|at| ours(&at) || old[at] == new[at]));
        // The string after them is where it was, and reads as it did.
        assert_eq!(decode(&program_strings(&new, &chars)[&1094184], &chars), "アイテムの説明");
    }
    fs_remove(&out);

    // What the strings need is added up against what they had, and those left out are named.
    // A string that stays where it is has its own refusal, in the same answer.
    let pack = script_of(&[
        ("program", 1094112, long),
        ("program", 1094128, "HEALS 30% HP, AND TASTES OF ORANGES"),
        ("program", 1094168, "APPLE GUMMY"),
        ("program", 1052704, "FOLLOW-UP CASTING"),
    ]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "string program.1052704 takes 17 bytes and has room for 15: this text stays where it is, and a wide \
         character takes two bytes where a narrow one takes one\n\
         string program.1094128 found no place: the strings of the lists that a pack changes share the room \
         they had between them, 72 bytes here with the zero byte that ends each, and this pack's need 100; the \
         largest piece left has 8. Shorter text in any of them makes room\n\
         \x20 program.1094128 needs 36 bytes"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn build_moves_a_string_the_code_builds_the_address_of() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // Three pieces the code puts into an item's description, side by side with 8 bytes each:
    // the first takes twelve of them now, so the second moves four bytes on.
    let pack =
        script_of(&[("program", 1139944, "% RESTORED\n"), ("program", 1139952, "TP "), ("program", 1139960, ", ")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "lists:     3 strings changed, 1 of them moved");
    {
        let built = Disc::open(&out).unwrap();
        let size = disc.size(EXECUTABLE).unwrap();
        let (old, new) = (disc.read(EXECUTABLE, 0, size).unwrap(), built.read(EXECUTABLE, 0, size).unwrap());
        assert_eq!(new[1139944..1139968], *b"% RESTORED\x01\0TP \0, \0\0\0\0\0\0");
        // The one instruction that adds the low half of the second string's address has the
        // new one, $2154F4 where it was $2154F0, and nothing else changed.
        assert_eq!((number(&old, 0x584D4), number(&new, 0x584D4)), (Some(0x2610_54F0), Some(0x2610_54F4)));
        let ours = |at: &usize| (1139944..1139968).contains(at) || (0x584D4..0x584D8).contains(at);
        assert!(old.len() == new.len() && (0..old.len()).all(|at| ours(&at) || old[at] == new[at]));
    }
    fs_remove(&out);
    // The string the code copies a set number of bytes of has those and no more.
    build(&disc, &script_of(&[("program", 1064008, "NOT NAMED!")]), &out, &[]).unwrap();
    fs_remove(&out);
    assert_eq!(
        message(build(&disc, &script_of(&[("program", 1064008, "NOT NAMED!!")]), &out, &[])),
        "string program.1064008 takes 11 bytes and has room for 10: this text stays where it is, and a wide character takes two bytes where a narrow one takes one"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn build_renames_an_enemy_in_its_pack() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // The second name is as long as a name can be.
    let pack = script_of(&[("08211", 1, "SLIME"), ("08211", 4, "KOBOLD OF THE FAR HILLS")]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "string 08211.004 takes 23 bytes and has room for 22: this text stays where it is, and a wide \
         character takes two bytes where a narrow one takes one"
    );
    let pack = script_of(&[("08211", 1, "SLIME"), ("08211", 4, "KOBOLD OF THE FAR HILL")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "enemies:   2 names changed in 1 packs, and 2 on the battle's name plates");
    {
        let built = Disc::open(&out).unwrap();
        // The battle's name plate has the new names too, and the rest of its list is as it was.
        let plates = |disc: &Disc| {
            let (offset, size) = member_span(disc, PLATE_MEMBER).unwrap();
            plate_lists(&disc.read(ARCHIVE, offset, size as u64).unwrap()).unwrap()
        };
        let ((old_groups, old_list), (new_groups, new_list)) = (plates(&disc), plates(&built));
        let kinds = plate_kinds(&enemy_names(&disc).unwrap(), &old_groups, &old_list).unwrap();
        let (slime, kobold) = (usize::from(kinds[&(8211, 1)]), usize::from(kinds[&(8211, 4)]));
        assert_eq!(plate_name(&new_list, slime), Some(b"SLIME".as_slice()));
        assert_eq!(plate_name(&new_list, kobold), Some(b"KOBOLD OF THE FAR HILL".as_slice()));
        let theirs = |at: usize| {
            [slime, kobold]
                .iter()
                .any(|kind| (4 + kind * PLATE_SIZE..4 + kind * PLATE_SIZE + PLATE_FIELD).contains(&at))
        };
        assert!(new_groups == old_groups && (0..old_list.len()).all(|at| theirs(at) || old_list[at] == new_list[at]));
        let texts = extract(&built).unwrap();
        let names = |pack: &str| texts[pack].values().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(names("08211"), ["SLIME", "KOBOLD OF THE FAR HILL"]);
        // The packs around it, and the story scripts before it in the archive, are what they were.
        let before = extract(&disc).unwrap();
        assert!(texts["08210"] == before["08210"] && texts["08212"] == before["08212"]);
        assert_eq!(texts["06307"], before["06307"]);
        // The pictures and movements in the pack are as they were.
        let read = |disc: &Disc| {
            let (offset, size) = member_span(disc, 8211).unwrap();
            disc.read(ARCHIVE, offset, size as u64).unwrap()
        };
        let (old, new) = (read(&disc), read(&built));
        let entries = |pack: &[u8]| -> Vec<Vec<u8>> {
            pack_entries(pack).unwrap().iter().map(|(at, size)| pack[*at..*at + *size].to_vec()).collect()
        };
        let (old, new) = (entries(&old), entries(&new));
        assert!(
            old.len() == new.len() && (0..old.len()).all(|entry| [1, 4].contains(&entry) || old[entry] == new[entry])
        );
    }
    fs_remove(&out);
    assert_eq!(message(build(&disc, &script_of(&[("08211", 2, "X")]), &out, &[])), "string 08211.002 does not exist");
    // The slime of 08211 is the slime of 08209 too, and its plate has one name for both.
    let pack = script_of(&[("08209", 1, "SLIME"), ("08211", 1, "GOO")]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "strings 08209.001 and 08211.001 name the same enemy, whose name plate in battle has one name for every \
         pack it is in: they give it \"SLIME\" and \"GOO\""
    );
    build(&disc, &script_of(&[("08209", 1, "SLIME"), ("08211", 1, "SLIME")]), &out, &[]).unwrap();
    fs_remove(&out);
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn build_changes_a_game_s_text_where_it_is() {
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // The first takes all the bytes the game's own has, twelve; the second fewer than its own.
    let pack = script_of(&[("06178", 17817, "LET'S GO NOW"), ("06178", 17853, "HEH")]);
    assert_eq!(
        message(build(&disc, &script_of(&[("06178", 17817, "LET US GO NOW")]), &out, &[])),
        "string 06178.17817 takes 13 bytes and has room for 12: this text stays where it is, and a wide \
         character takes two bytes where a narrow one takes one"
    );
    // The game's narrow font has no apostrophe: it is drawn from the game's own fonts.
    let drawn = build(&disc, &pack, &out, &[]).unwrap();
    let font_line = "font:      1 new characters, 1 at half width and 0 at full width".to_string();
    assert!(drawn.summary().contains(&font_line), "{:?}", drawn.summary());
    let pack = script_of(&[("06178", 17817, "LETS GO NOW!"), ("06178", 17853, "HEH")]);
    let result = build(&disc, &pack, &out, &[]).unwrap();
    assert_eq!(result.summary()[1], "minigames: 2 strings changed in 1 scripts");
    {
        let built = Disc::open(&out).unwrap();
        let (old, new) = (read_member(&disc, 6178).unwrap(), read_member(&built, 6178).unwrap());
        assert_eq!(new[17817..17830], *b"LETS GO NOW!\0");
        assert_eq!(new[17853..17866], *b"HEH\0\0\0\0\0\0\0\0\0\0");
        let same = |at: usize| (17817..17830).contains(&at) || (17853..17866).contains(&at) || old[at] == new[at];
        assert!(old.len() == new.len() && (0..old.len()).all(same));
        // The script is stored compressed, as it was.
        let stored = |disc: &Disc| {
            let (offset, size) = member_span(disc, 6178).unwrap();
            disc.read(ARCHIVE, offset, size as u64).unwrap()
        };
        assert!(lzss_ring::is_block(&stored(&disc)) && lzss_ring::is_block(&stored(&built)));
        assert_eq!(stored(&built)[0], stored(&disc)[0]);
    }
    fs_remove(&out);
    assert_eq!(
        message(build(&disc, &script_of(&[("06178", 17818, "X")]), &out, &[])),
        "string 06178.17818 does not exist"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

#[test]
fn every_string_that_does_not_fit_is_reported_at_once() {
    assert_eq!(refusal_of(&[]), None);
    let Some(disc) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.iso");
    // One of each kind that stays where it is, each too long, and one that fits.
    let pack = script_of(&[
        ("08996", 178456, "OUTER SHELL"),
        ("program", 1052704, "FOLLOW-UP CASTING"),
        ("08211", 1, "A SLIME OF THE FAR HILLS"),
        ("06178", 17817, "LET US GO NOW"),
        ("06178", 17853, "HEH"),
    ]);
    assert_eq!(
        message(build(&disc, &pack, &out, &[])),
        "4 strings take more bytes than they have room for: this text stays where it is, and a wide character \
         takes two bytes where a narrow one takes one\n\
         \x20 06178.17817 takes 13 bytes and has room for 12, 1 over\n\
         \x20 08211.001 takes 24 bytes and has room for 22, 2 over\n\
         \x20 08996.178456 takes 11 bytes and has room for 7, 4 over\n\
         \x20 program.1052704 takes 17 bytes and has room for 15, 2 over"
    );
    assert!(!out.exists(), "a refused build left a file behind");
}

/// Remove a built image as soon as it has been looked at: it is as large as the game.
fn fs_remove(path: &Path) {
    std::fs::remove_file(path).unwrap();
}
