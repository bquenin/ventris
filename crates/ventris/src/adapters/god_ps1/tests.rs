use std::collections::BTreeSet;

use crate::adapter::declaration_faults;

use super::battle::{ANSWERS, When, read_script};
use super::program::{PARTS, SHOP_LINES};
use super::*;
use crate::platforms::mips;

fn hex(bytes: &str) -> Vec<u8> {
    from_hex(bytes).unwrap()
}

/// A table of bytes like the game's: $20 a blank, $A1 。, $A2 あ, $A3 a code the character table
/// does not have, $10 to $1F other numbers, as in the program, and the rest drawing nothing.
fn codes() -> ByteCodes {
    let mut codes = [0u16; 256];
    codes[0x10] = 0x8002;
    codes[0x20] = 0x04D7;
    codes[0xA1] = 0x0101;
    codes[0xA2] = 0x0136;
    codes[0xA3] = 0x08F0;
    ByteCodes::new(codes)
}

fn decoded(bytes: &str) -> String {
    decode(&hex(bytes), &codes())
}

fn encoded(text: &str) -> Result<Vec<u8>> {
    encode(text, &codes())
}

/// The game's disc, opened, or None, with a word about why, where it is not there to test with.
fn game() -> Option<Image> {
    Some(Image::Ps1Disc(Disc::open(&crate::testing::game("god-ps1")?).unwrap()))
}

#[test]
fn the_adapter_declares_itself_properly() {
    assert_eq!(declaration_faults(&ADAPTER), Vec::<String>::new());
    assert_eq!(ADAPTER.kinds().len(), 4);
    assert!(ADAPTER.can_insert());
    let insertable: Vec<&str> = ADAPTER.kinds().iter().filter(|kind| kind.insert).map(|kind| kind.name).collect();
    assert_eq!(insertable, ["dialogue", "program", "menu"]);
}

#[test]
fn characters_are_written_in_one_byte_where_the_game_has_one() {
    // あ has a byte; 目 (0384) does not, and is written in two.
    assert_eq!(encoded("あ。　").unwrap(), hex("a2 a1 20"));
    assert_eq!(decoded("a2 a1 20"), "あ。　");
    assert_eq!(encoded("目").unwrap(), hex("03 84"));
    assert_eq!(decoded("01 36"), "{W:0136}"); // あ in two bytes, which the game never does
    assert_eq!(encoded("{W:0136}").unwrap(), hex("01 36"));
    // A code with no character: in the form the game writes it, one byte or two.
    assert_eq!(decoded("a3 08 f1"), "{K:08F0}{K:08F1}");
    assert_eq!(encoded("{K:08F0}{K:08F1}").unwrap(), hex("a3 08 f1"));
    // A byte that draws nothing, or whose entry is not a glyph.
    assert_eq!(decoded("7e 10"), "{B:7E}{B:10}");
    assert_eq!(encoded("{B:7E}{B:10}").unwrap(), hex("7e 10"));
}

#[test]
fn control_codes_are_tags() {
    let bytes = "0e a2 0d a2 0b a2 0e 0c 01 0c 19 0c 0d 0c 12 0c 10 0f 00 14 0f 01 0f 02 0f 03";
    let text = "{SAY}あ\nあ{WAIT}\nあ{/SAY}{NAME:01}{NAME:19}{NUM}{WORD}{0C:10}{PAUSE:14}{HOLD}{CLEAR}{0F:03}";
    assert_eq!(decoded(bytes), text);
    assert_eq!(encoded(text).unwrap(), hex(bytes));
    // The line break after a wait is layout: without it the bytes are the same.
    assert_eq!(encoded("あ{WAIT}あ").unwrap(), hex("a2 0b a2"));
    assert_eq!(encoded("あ{WAIT}\n\nあ").unwrap(), hex("a2 0b 0d a2"));
    assert_eq!(decoded("0b 0d"), "{WAIT}\n\n");
    // Both forms of a spoken line's mark are the one code.
    assert_eq!(encoded("{/SAY}{SAY}").unwrap(), hex("0e 0e"));
}

#[test]
fn tags_the_game_has_no_code_for_are_refused() {
    for text in ["{NAME:0D}", "{0C:01}", "{0C:1F}", "{0F:01}", "{K:0050}", "{K:0B00}", "{B:0F}", "{SAY:01}", "{X}"] {
        assert!(matches!(encoded(text), Err(Error::Text(_))), "{text} was taken");
    }
    assert!(encoded("{K:01").is_err() || encoded("{K:01").unwrap() == "{K:01".bytes().collect::<Vec<_>>());
}

#[test]
fn a_text_block_holds_strings_lists_of_numbers_and_shared_entries() {
    // Five entries: 0 and 1 at one place, a string ending on an odd offset with a left-over byte
    // after it, a list of two pairs of numbers, and a last string with the block's four zero bytes.
    let mut text = hex("14 00 00 00 14 00 00 00 18 00 00 00 1c 00 00 00 2a 00 00 00");
    text.extend(hex("0e a2 0e 00")); // 14: {SAY}あ{/SAY}
    text.extend(hex("a2 a1 00 55")); // 18: あ。 and a left-over byte
    text.extend(hex("02 00 01 00 02 00 03 00 04 00 00 00 00 00")); // 1c: a list, with padding
    text.extend(hex("a1 00 00 00 00 00")); // 2a: 。
    let found = entries(&text, &codes()).unwrap();
    assert_eq!(
        found,
        [
            Entry::Text { start: 0x14, end: 0x17 },
            Entry::Same(0),
            Entry::Text { start: 0x18, end: 0x1A },
            Entry::Numbers,
            Entry::Text { start: 0x2A, end: 0x2B },
        ]
    );
    // A string that runs into the next entry, or with something after its end, is no string.
    let mut cut = text.clone();
    cut[0x1A] = 0xA1;
    assert!(entries(&cut, &codes()).unwrap_err().contains("entry 2"));
    assert_eq!(entries(&[], &codes()), Ok(Vec::new()));
    assert!(entries(&hex("08 00 00 00 04 00 00 00"), &codes()).is_err());
}

#[test]
fn a_list_of_numbers_is_told_by_its_count() {
    assert!(numbers_at(&hex("02 00 07 00 08 00")));
    assert!(numbers_at(&hex("01 00 a8 01 40 01")));
    assert!(numbers_at(&hex("01 00 a8 01 40 01 00 00 00 00")));
    assert!(!numbers_at(&hex("01 00 a8 01 40 01 00 00 00 00 00")));
    assert!(!numbers_at(&hex("02 00 07 00 08 00 09")));
    assert!(!numbers_at(&hex("0e a2 0e 00")));
}

#[test]
fn sections_are_areas_in_three_digits() {
    assert!(is_dialogue("096") && is_dialogue("188") && is_dialogue("121"));
    assert!(!is_dialogue("95") && !is_dialogue("095") && !is_dialogue("189") && !is_dialogue("p001"));
    assert!(!is_dialogue("0096") && !is_dialogue("+96"));
    assert_eq!(code_at(0), 0x100);
    assert_eq!(code_at(GLYPH_COUNT - 1), 0x8FF);
}

#[test]
fn notes_say_what_repeats() {
    assert!(ADAPTER.note("122", None).unwrap().contains("area 121"));
    assert_eq!(ADAPTER.note("121", None), None);
    assert!(ADAPTER.note("096", Some(0)).unwrap().contains("001"));
    assert_eq!(ADAPTER.note("096", Some(2)), None);
    assert_eq!(ADAPTER.note("099", Some(0)), None);
    assert_eq!(ADAPTER.note("names", Some(0)), None);
}

#[test]
fn the_program_s_text_is_in_three_sections_named_in_words() {
    assert_eq!(ADAPTER.kind_of("program").unwrap().name, "program");
    assert_eq!(ADAPTER.kind_of("menu").unwrap().name, "menu");
    assert_eq!(ADAPTER.kind_of("battle").unwrap().name, "battle");
    assert_eq!(ADAPTER.kind_of("096").unwrap().name, "dialogue");
    assert!(ADAPTER.kind_of("p").is_none());
    assert!(ADAPTER.kinds().iter().all(|kind| kind.numbered_by_place));
}

#[test]
fn notes_say_what_the_battle_s_window_is_and_which_strings_answer_a_question() {
    assert!(ADAPTER.note("battle", None).unwrap().contains("4 lines of 18 cells"));
    let (yes, no) = ANSWERS[0];
    let yes = ADAPTER.note("battle", Some(yes)).unwrap();
    assert!(yes.contains("answers はい") && yes.contains("program.748709"), "{yes}");
    assert!(ADAPTER.note("battle", Some(no)).unwrap().contains("answers いいえ"));
    assert_eq!(ADAPTER.note("battle", Some(0x9_812A)), None);
}

/// A battle's script read from its first byte, with the table of bytes of `codes`.
fn script(bytes: &str) -> std::result::Result<(Vec<(usize, When)>, usize), String> {
    read_script(&hex(bytes), 0, &codes())
}

#[test]
fn a_battle_s_script_is_read_command_by_command() {
    // As the battle starts: two settings, the opening string, a setting with a byte, the string for a
    // win. Then, as it goes on: a wait for a round, an attack, a string at round 2, a question and
    // its two answers, a wait on hit points, a command with two bytes, a string with no wait, a
    // command alone, and the end.
    let read = script(
        "08 04 01 A2 00 05 07 02 A1 00 0B 01 0A DE 07 02 A2 0D A2 00 06 A2 00 A1 00 13 32 11 01 02 0C 0F 02 A2 00 \
         0F 00",
    );
    let strings =
        vec![(3, When::Start), (8, When::Won), (16, When::Round(2)), (21, When::Yes), (23, When::No), (31, When::Next)];
    assert_eq!(read, Ok((strings, 36)));
    // The strings of the start are read only before the rest: after it, the game stops at them.
    assert_eq!(script("0B 00 01 A2 00 00"), Ok((vec![], 2)));
    assert_eq!(script("14 A2 00 03 A2 00 00"), Ok((vec![(1, When::Lost), (4, When::AfterFirstRound)], 6)));
    // $04 is a setting as the battle starts, and where the game stops after it; a question follows
    // only a string of the rest.
    assert_eq!(script("04 0B 00 04 0C A2 00"), Ok((vec![], 3)));
    assert_eq!(script("01 A2 00 06 A2 00 A1 00 00"), Ok((vec![(1, When::Start)], 3)));
    // A string with no zero byte, or with a byte that is no text, and a command cut short, are refused.
    assert!(script("01 A2").is_err());
    assert!(script("0C 0C FF 00 00").is_err());
    assert!(script("0B").is_err());
    // A round command whose number is missing is a script cut short, not a stop.
    assert!(script("0C A2 00 07").is_err());
    assert_eq!(script("0C A2 00 07 02 A2 00 00"), Ok((vec![(1, When::Next), (5, When::Round(2))], 7)));
}

#[test]
fn notes_say_what_each_part_of_the_program_s_text_is() {
    assert!(ADAPTER.note("program", None).unwrap().contains("program's file"));
    assert!(ADAPTER.note("menu", None).unwrap().contains("small font"));
    // The first string of a part says what the part is; a shopkeeper's line says which kind it is.
    let first = ADAPTER.note("program", Some(0xA_0954)).unwrap();
    assert!(first.contains("68 shops") && first.ends_with("A shopkeeper's line: the greeting."), "{first}");
    let line = |number: u32| ADAPTER.note("program", Some(number)).unwrap();
    assert_eq!(line(0xA_0EC8), format!("A shopkeeper's line: {}.", SHOP_LINES[8].1));
    assert_eq!(line(0xA_0A51), format!("A shopkeeper's line: {}.", SHOP_LINES[1].1));
    assert!(line(0xA_0F4C).contains("monster book"));
    // The first string of a part says the room its lines have; a choice, that its box is the game's own.
    assert!(first.contains("Lines are at most 13 characters, and a string has at most 2 lines."), "{first}");
    assert!(line(0xA_9088).ends_with("a string of one line stays one line, and one of several lines has at most 109."));
    assert!(line(0xA_B380).ends_with("no line may be wider than the game's own widest."));
    assert_eq!(ADAPTER.note("program", Some(0xA_B354)), None);
    assert_eq!(ADAPTER.note("menu", Some(0xA_7D1C)), None);
    // The parts are in the order of the file.
    assert!(PARTS.windows(2).all(|pair| pair[0].start < pair[1].start));
    assert!(SHOP_LINES.windows(2).all(|pair| pair[0].0 < pair[1].0));
}

/// A label of the small font, written as Shift-JIS: `@` and the half-width katakana.
fn small(text: &str) -> Vec<u8> {
    text.chars()
        .map(|char| match char as u32 {
            0xFF61..=0xFF9F => (char as u32 - 0xFF61 + 0xA1) as u8,
            ascii => ascii as u8,
        })
        .collect()
}

#[test]
fn the_small_font_s_labels_are_kana_of_the_game_s_width() {
    for (bytes, text) in [
        ("@ﾄﾞｳｸﾞ", "どうぐ"),
        ("ﾊﾞﾄﾙﾒｯｾｰｼﾞ@ｿｸﾄﾞ", "バトルメッセージそくど"),
        ("@ﾂｷﾞﾉ@ﾚﾍﾞﾙ@ﾏﾃﾞ ｱﾄ", "つぎのレベルまで あと"),
        ("ｶｰｿﾙ@ｲﾁ.....", "カーソルいち....."),
        ("@SFX VOL....", "{KANA}SFX VOL...."),
        ("%8dｺ", "%8dコ"),
        ("ｳﾞﾊﾟｱﾞﾟ", "ヴパア゛゜"),
        ("@@ｱ@", "{KANA}ア{KANA}"),
        ("@ｰｱ", "{KANA}ーあ"),
        ("｢ｦ｣､･｡", "「ヲ」、・。"),
    ] {
        assert_eq!(program::decode_small(&small(bytes)), text, "{bytes}");
        assert_eq!(program::encode_small(text).unwrap(), small(bytes), "{text}");
    }
    // Every label made of these bytes, up to four long, survives a round trip.
    let alphabet = small("@A ｶﾞﾟﾊｰｳｦ.");
    let mut labels: Vec<Vec<u8>> = vec![Vec::new()];
    for _ in 0..4 {
        let longer: Vec<Vec<u8>> = (labels.iter())
            .flat_map(|label| alphabet.iter().map(move |byte| [label.as_slice(), &[*byte]].concat()))
            .collect();
        for label in &longer {
            assert_eq!(&program::encode_small(&program::decode_small(label)).unwrap(), label, "{label:02X?}");
        }
        labels = longer;
    }
    // What the small font cannot write is refused.
    for text in ["@", "ヶ", "漢", "{WAIT}", "{"] {
        assert!(program::encode_small(text).is_err(), "{text}");
    }
}

#[test]
fn menu_labels_are_what_the_code_points_at_and_the_slots_after_it() {
    let mut file = vec![0u8; 0x200];
    let mut put = |at: usize, label: &str| file[at..at + small(label).len()].copy_from_slice(&small(label));
    put(0x10, "@ﾄﾞｳｸﾞ");
    put(0x1A, "@ｿｳﾋﾞ");
    put(0x24, "@SFX VOL"); // no kana, but a switch: a label in a run of them
    put(0x30, "TIME"); // ASCII alone ends the run
    put(0x40, "ｻｲｺ");
    put(0x80, "TIME"); // pointed at, but ASCII alone
    put(0x90, "ｾｰﾌﾞ");
    put(0xD0, "ﾛｰﾄﾞ"); // too far after the label before
    put(0x100, "@SFX"); // pointed at, but no kana
    let built = BTreeSet::from([0x10, 0x80, 0x90, 0x100]);
    let labels = program::menu_strings(&file, &built);
    let found: Vec<(u32, String)> = labels.into_iter().map(|(at, label)| (at, program::decode_small(&label))).collect();
    let expected = [(0x10, "どうぐ"), (0x1A, "そうび"), (0x24, "{KANA}SFX VOL"), (0x90, "セーブ")];
    assert_eq!(found, expected.map(|(at, text)| (at, text.to_string())));
}

#[test]
fn text_is_read_to_its_zero_byte_and_its_characters_counted() {
    let codes = codes();
    assert_eq!(read_text(&hex("a2 0d 01 36 0c 19 0f 00 1e a1 00 a2"), &codes), Some((10, 3)));
    assert_eq!(read_text(&hex("0b 0e 0c 19 0f 02 00"), &codes), Some((6, 0)));
    assert_eq!(read_text(&hex("7e 00"), &codes), Some((1, 0))); // a byte that draws nothing
    assert_eq!(read_text(&hex("10 00"), &codes), None); // a byte whose entry is not a glyph
    assert_eq!(read_text(&hex("a2 a1"), &codes), None); // no zero byte
    assert!(every_character_known(&hex("a2 a1 20 0d 03 84"), &codes));
    assert!(!every_character_known(&hex("a2 7e"), &codes));
    assert!(!every_character_known(&hex("a3"), &codes)); // a code the table of characters lacks
}

#[test]
fn unmapped_codes_are_the_tags_of_codes_with_no_character() {
    assert_eq!(ADAPTER.unmapped("あ{K:08F0}い{W:0820}{K:0136}{K:38}{K:08F0"), ["08F0", "0820"]);
}

#[test]
fn the_table_of_characters_is_what_is_certain() {
    assert_eq!(TABLE.characters[&0x100], '、');
    assert_eq!(TABLE.characters[&0x183], 'を');
    assert_eq!(TABLE.characters[&0x184], 'ん');
    assert_eq!(TABLE.characters[&0x185], 'ァ');
    assert_eq!(TABLE.characters[&0x1D4], 'ン');
    assert_eq!(TABLE.characters[&0x780], 'ａ');
    assert_eq!(TABLE.characters[&0x4D7], '\u{3000}');
    // Two codes draw ｖ; it is written with the one that has a byte, and the other is a tag.
    assert_eq!(TABLE.codes[&'ｖ'], [0x4F9, 0x795]);
    let mut table = [0u16; 256];
    table[0x76] = 0x795;
    let codes = ByteCodes::new(table);
    assert_eq!(encode("ｖ", &codes).unwrap(), [0x76]);
    assert_eq!(decode(&[0x76, 0x04, 0xF9, 0x07, 0x95], &codes), "ｖ{K:04F9}{W:0795}");
    assert_eq!(encode("ｖ{K:04F9}{W:0795}", &codes).unwrap(), [0x76, 0x04, 0xF9, 0x07, 0x95]);
    assert!(ADAPTER.unmapped("{K:04F9}{W:0795}").is_empty());
    // Without a byte for either, the lower is used.
    assert_eq!(encoded("ｖ").unwrap(), [0x04, 0xF9]);
}

// --- with the game ---------------------------------------------------------

#[test]
fn the_game_s_areas_are_read() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let program = program(disc).unwrap();
    // 164 members fill the archive, and entries 96 and 189 to 195 are one.
    let members: BTreeSet<(u32, u32)> = program.contents.iter().copied().collect();
    assert_eq!(members.len(), 164);
    assert!((189..=195).all(|entry| program.contents[entry] == program.contents[96]));
    let areas = areas(disc).unwrap();
    assert_eq!(areas.len(), 93);
    let entries: usize = areas.iter().map(|area| area.entries.len()).sum();
    let numbers = areas.iter().flat_map(|area| &area.entries).filter(|entry| **entry == Entry::Numbers).count();
    let strings: usize = areas.iter().map(|area| area.strings().len()).sum();
    assert_eq!((entries, numbers, strings), (9704, 427, 9185));
    // Area 100 has no text; in every other, entry 1 is entry 0, and no other entries share a place.
    for area in areas.iter() {
        let shared: Vec<(usize, u32)> = (area.entries.iter().enumerate())
            .filter_map(
                |(number, entry)| if let Entry::Same(earlier) = entry { Some((number, *earlier)) } else { None },
            )
            .collect();
        if area.entry == 100 {
            assert!(area.text.is_empty());
        } else {
            assert_eq!(shared, [(SHARED_ENTRY.0 as usize, SHARED_ENTRY.1)], "area {}", area.entry);
            let text = matches!(area.entries[0], Entry::Text { .. });
            assert_eq!(text, !LIST_FIRST.contains(&area.entry), "area {}", area.entry);
            assert!(text || area.entries[0] == Entry::Numbers);
        }
    }
    // The areas that repeat another's text, and no others.
    let mut repeated = Vec::new();
    for (index, area) in areas.iter().enumerate() {
        if let Some(original) = areas[..index].iter().find(|earlier| !area.text.is_empty() && earlier.text == area.text)
        {
            repeated.push((area.entry, original.entry));
        }
    }
    assert_eq!(repeated, REPEATED_AREAS);
}

#[test]
fn the_game_s_script_is_extracted_and_encodes_back() {
    let Some(image) = game() else { return };
    let pack = ADAPTER.extract(&image).unwrap();
    assert_eq!(pack.texts.len(), 95); // 92 areas, the program's text, the menus' labels, the battles' scripts
    assert!(!pack.texts.contains_key("100"));
    assert_eq!(pack.texts["096"][&109], "{SAY}目を覚ましなさい。{/SAY}");
    ADAPTER.verify(&image, &pack.texts).unwrap();
    // A string changed by one character no longer encodes back.
    let mut changed = pack.texts.clone();
    changed.get_mut("096").unwrap().insert(109, "{SAY}目を覚ましなさい！{/SAY}".to_string());
    assert!(ADAPTER.verify(&image, &changed).is_err());
    // Nor does a script missing a string, or with one the game does not have.
    let mut missing = pack.texts.clone();
    missing.get_mut("096").unwrap().remove(&109);
    assert!(ADAPTER.verify(&image, &missing).is_err());
    let mut extra = pack.texts.clone();
    extra.get_mut("096").unwrap().insert(1, String::new());
    assert!(ADAPTER.verify(&image, &extra).is_err());
}

#[test]
fn the_game_s_program_text_is_found() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let file = program::program_file(disc).unwrap();
    // The program is loaded so that its code starts at $80010000, past its header of $800 bytes, and
    // its code ends with a return, then the instruction after it.
    assert_eq!(mips::number(&file, 0x18), Some(program::PROGRAM_BASE + 0x800));
    let end = program::PROGRAM_CODE.end;
    assert_eq!(mips::number(&file, end - 8), Some(mips::RETURN));
    assert!((end..0x8_0000).step_by(4).all(|at| mips::number(&file, at) != Some(mips::RETURN)));
    let text = program::program_text(disc).unwrap();
    let (strings, labels) = (&text[program::PROGRAM_SECTION], &text[program::MENU_SECTION]);
    assert_eq!((strings.len(), labels.len()), (3303, 56));
    let codes = &program(disc).unwrap().codes;
    let shown = |at: u32| decode(&strings[&at], codes);
    assert_eq!(shown(0xA_B350), "ゲン");
    assert_eq!(shown(0x9_F2B8), "武器屋"); // right after the table that points at it, with no zero byte
    assert_eq!(shown(0xA_0954), "いらっしゃいませ！");
    assert_eq!(shown(0xA_BB33), "メモリーカードが入っていません"); // known to the code alone
    assert_eq!(program::decode_small(&labels[&0xA_7D1C]), "どうぐ");
    assert_eq!(program::decode_small(&labels[&0xA_82AD]), "{KANA}SFX VOL....");
    // Every string is in a stretch of the text, and the labels in none.
    let in_text = |at: &u32| program::TEXT.iter().any(|stretch| stretch.contains(&(*at as usize)));
    assert!(strings.keys().all(in_text));
    assert!(!labels.keys().any(in_text));
    // Each part starts at a string.
    for program::Part { start, .. } in PARTS {
        assert!(strings.contains_key(&start), "{start:X}");
    }
    // The shops' records point at their lines, the nth of each record among the lines of the nth kind.
    for shop in 0..program::SHOP_COUNT {
        let record = mips::number(&file, program::SHOPS + 4 * shop).unwrap() - program::PROGRAM_BASE;
        for (kind, (start, _)) in SHOP_LINES.iter().enumerate() {
            let line = mips::number(&file, record as usize + 8 + 4 * kind).unwrap() - program::PROGRAM_BASE;
            let end = SHOP_LINES.get(kind + 1).map_or(0xA_0F4C, |(next, _)| *next);
            assert!((*start..end).contains(&line) && strings.contains_key(&line), "shop {shop}, line {kind}");
        }
    }
}

#[test]
fn the_game_s_program_text_encodes_back() {
    let Some(image) = game() else { return };
    let pack = ADAPTER.extract(&image).unwrap();
    assert_eq!(pack.texts["menu"][&0xA_8244], "バトルメッセージそくど");
    ADAPTER.verify(&image, &pack.texts).unwrap();
    let changes =
        [("program", 0xA_B350, "ゲソ"), ("menu", 0xA_8244, "バトルメッセージそくと"), ("battle", 0x9_812A, "")];
    for (section, number, text) in changes {
        let mut changed = pack.texts.clone();
        changed.get_mut(section).unwrap().insert(number, text.to_string());
        assert!(ADAPTER.verify(&image, &changed).is_err(), "{section}");
    }
    // A label written in the dialogue's way does not encode back.
    let mut swapped = pack.texts.clone();
    swapped.get_mut("menu").unwrap().insert(0xA_7D1C, "{SAY}".to_string());
    assert!(ADAPTER.verify(&image, &swapped).is_err());
}

#[test]
fn the_bosses_speeches_are_read_from_the_battles_scripts() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let file = program::program_file(disc).unwrap();
    let codes = &program(disc).unwrap().codes;
    // The table of the scripts, then the scripts, in the order of the battles.
    let starts: Vec<usize> = (0..battle::SCRIPT_COUNT)
        .map(|entry| (mips::number(&file, battle::SCRIPTS + 4 * entry).unwrap() - program::PROGRAM_BASE) as usize)
        .collect();
    assert_eq!(starts[0], battle::SCRIPTS + 4 * battle::SCRIPT_COUNT);
    assert!(starts.is_sorted());
    // Each script stops at the byte before the next, but six, which have bytes left that the game
    // never reads; the last is followed by zero bytes.
    let mut left = Vec::new();
    for (entry, start) in starts.iter().enumerate() {
        let (_, end) = read_script(&file, *start, codes).unwrap();
        let next = starts.get(entry + 1).copied().unwrap_or(end + 1);
        assert!(end < next, "battle {}", battle::FIRST_BATTLE as usize + entry);
        if end + 1 != next {
            left.push(battle::FIRST_BATTLE as usize + entry);
        }
    }
    assert_eq!(left, [210, 218, 225, 233, 238, 253]);
    let (_, last) = read_script(&file, starts[battle::SCRIPT_COUNT - 1], codes).unwrap();
    assert_eq!(last, 0x9_AC23);
    assert!(file[last..last + 16].iter().all(|byte| *byte == 0));

    let lines = battle::battle_text(disc).unwrap();
    let count = |wanted: fn(&When) -> bool| lines.values().filter(|line| wanted(&line.when)).count();
    assert_eq!(lines.len(), 144);
    assert_eq!(count(|when| *when == When::Start), 29);
    assert_eq!(count(|when| *when == When::Won), 10);
    assert_eq!(count(|when| *when == When::Lost), 3);
    assert_eq!(count(|when| matches!(when, When::Yes | When::No)), 2 * ANSWERS.len());
    for (yes, no) in ANSWERS {
        assert_eq!((lines[&yes].when, lines[&no].when), (When::Yes, When::No));
    }
    assert_eq!(lines[&0x9_812A].battle, 200);
    let first = decode(&lines[&0x9_812A].data, codes);
    assert_eq!(first, "ガキ大将だ！{WAIT}\n{CLEAR}ガキ大将\n{SAY}覚悟はいいか！{/SAY}{WAIT}\n");
    let text = program::program_text(disc).unwrap();
    assert_eq!(decode(&text[program::PROGRAM_SECTION][&battle::CHOICE], codes), "　　　　はい　　　　いいえ");
    // No byte of the program is in two strings, of one section or of two.
    let mut taken: Vec<(u32, usize)> = lines.iter().map(|(at, line)| (*at, line.data.len())).collect();
    taken.extend(text.values().flat_map(|strings| strings.iter().map(|(at, data)| (*at, data.len()))));
    taken.sort();
    for pair in taken.windows(2) {
        assert!(pair[0].0 as usize + pair[0].1 < pair[1].0 as usize, "{:X} and {:X}", pair[0].0, pair[1].0);
    }
}

#[test]
fn the_game_writes_no_character_in_two_bytes_that_it_has_one_byte_for() {
    let Some(image) = game() else { return };
    let pack = ADAPTER.extract(&image).unwrap();
    let strings = || pack.texts.values().flat_map(|strings| strings.values());
    assert_eq!(strings().filter(|text| text.contains("{W:")).count(), 0);
    // Bytes that draw nothing: $22 twice, $5F once, $7E three times.
    let blank: usize = strings().map(|text| text.matches("{B:").count()).sum();
    assert_eq!(blank, 6);
}

#[test]
fn the_game_s_font_is_read() {
    let Some(image) = game() else { return };
    let blank = vec![vec![0; GLYPH_SIZE]; GLYPH_SIZE];
    // ん is drawn; code $4D7, the byte $20, is a blank.
    assert_ne!(ADAPTER.glyph(&image, 0x184 - 0x100).unwrap(), blank);
    assert_eq!(ADAPTER.glyph(&image, 0x4D7 - 0x100).unwrap(), blank);
    assert!(ADAPTER.glyph(&image, GLYPH_COUNT).is_err());
    assert_ne!(ADAPTER.glyph(&image, GLYPH_COUNT - 1).unwrap().len(), 0);
}

/// A program's file made up for the placing of its text: five strings of あ ($A2) in the first
/// stretch of the shopkeepers' lines, a table that points at each, and code that builds the address
/// of the third. The second has a piece of text after it that nothing points at; the third has zero
/// bytes after it; and something points at the zero byte after the fourth.
fn made_up_program() -> (Vec<u8>, BTreeMap<u32, Vec<u8>>) {
    let mut file = vec![0u8; 0xA_1000];
    let strings: BTreeMap<u32, Vec<u8>> = [(0xA_0960, 2), (0xA_0963, 3), (0xA_096A, 1), (0xA_0970, 2), (0xA_0974, 1)]
        .into_iter()
        .map(|(at, length)| (at, vec![0xA2; length]))
        .collect();
    for (at, data) in &strings {
        file[*at as usize..*at as usize + data.len()].copy_from_slice(data);
    }
    file[0xA_0967] = 0xA2; // the piece of text after the second, ended by the zero byte at $A0968
    let pointers = strings.keys().copied().chain([0xA_0973]);
    for (index, at) in pointers.enumerate() {
        let word = at + program::PROGRAM_BASE;
        file[0xA_0A00 + 4 * index..][..4].copy_from_slice(&word.to_le_bytes());
    }
    let address = 0xA_096A + program::PROGRAM_BASE;
    let code = [mips::LUI << 26 | 4 << 16 | address >> 16, mips::ADDIU << 26 | 4 << 21 | 4 << 16 | (address & 0xFFFF)];
    for (index, word) in code.into_iter().chain([mips::RETURN, 0]).enumerate() {
        file[0x800 + 4 * index..][..4].copy_from_slice(&word.to_le_bytes());
    }
    (file, strings)
}

#[test]
fn a_string_s_room_is_its_bytes_the_zero_bytes_after_it_and_the_text_nothing_points_at() {
    let (file, strings) = made_up_program();
    let references = program::references_in(&file);
    let rooms = program::rooms(&file, &strings, &references, &codes());
    let sizes: Vec<usize> = rooms.values().map(|room| room.size).collect();
    assert_eq!(sizes, [3, 7, 6, 3, 2]);
    assert_eq!(rooms[&0xA_0963].tail, [0xA2, 0]);
    assert!(rooms.values().filter(|room| room.tail.is_empty()).count() == 4);
    assert_eq!(rooms[&0xA_0960].reached, Some(program::Reached { words: vec![0xA_0A00], code: vec![] }));
    assert_eq!(rooms[&0xA_096A].reached, Some(program::Reached { words: vec![0xA_0A08], code: vec![0x804] }));
    // Something that points inside a string keeps it where it is.
    let mut file = file;
    file[0xA_0A20..0xA_0A24].copy_from_slice(&(0xA_0961 + program::PROGRAM_BASE).to_le_bytes());
    let references = program::references_in(&file);
    let rooms = program::rooms(&file, &strings, &references, &codes());
    assert_eq!(rooms[&0xA_0960].reached, None);
}

#[test]
fn changed_strings_share_their_room_and_what_refers_to_them_follows() {
    let (file, strings) = made_up_program();
    let references = program::references_in(&file);
    let rooms = program::rooms(&file, &strings, &references, &codes());
    // The first grows past its room and the third shrinks: the first takes the third's room, which
    // is the one that leaves least unused, and the third the first's.
    let changed = BTreeMap::from([(0xA_0960, vec![0xA2; 4]), (0xA_096A, vec![0xA2])]);
    let written = program::write_strings(&file, &rooms, &changed).unwrap();
    assert_eq!((written.moved, written.once), (2, 0));
    let mut out = file.clone();
    for (at, data) in &written.patches {
        out[*at..*at + data.len()].copy_from_slice(data);
    }
    assert_eq!(out[0xA_096A..0xA_0970], [0xA2, 0xA2, 0xA2, 0xA2, 0, 0]);
    assert_eq!(out[0xA_0960..0xA_0963], [0xA2, 0, 0]);
    assert_eq!(mips::number(&out, 0xA_0A00), Some(0xA_096A + program::PROGRAM_BASE));
    let low = mips::number(&out, 0x804).unwrap();
    assert_eq!(
        (low >> 16, low & 0xFFFF),
        (mips::ADDIU << 10 | 4 << 5 | 4, (0xA_0960 + program::PROGRAM_BASE) & 0xFFFF)
    );
    // The second and its piece of text are left alone.
    assert_eq!(out[0xA_0963..0xA_096A], file[0xA_0963..0xA_096A]);

    // Two strings given the same text are written once, and both point at the one copy.
    let changed = BTreeMap::from([(0xA_0970, vec![0xA2]), (0xA_0974, vec![0xA2])]);
    let written = program::write_strings(&file, &rooms, &changed).unwrap();
    assert_eq!((written.moved, written.once), (1, 1));
    let words: Vec<(usize, Vec<u8>)> = written.patches.into_iter().filter(|(at, _)| *at == 0xA_0A10).collect();
    assert_eq!(words, [(0xA_0A10, (0xA_0970 + program::PROGRAM_BASE).to_le_bytes().to_vec())]);

    // A string that moves takes its piece of text with it. The first and the third, both made empty,
    // are written once, and the three rooms side by side make one stretch.
    let changed = BTreeMap::from([(0xA_0963, vec![0xA2; 7]), (0xA_096A, vec![]), (0xA_0960, vec![])]);
    let written = program::write_strings(&file, &rooms, &changed).unwrap();
    assert_eq!((written.moved, written.once), (2, 1));
    let stretch = written.patches.iter().find(|(at, _)| *at == 0xA_0960).unwrap();
    assert_eq!(stretch.1, [&[0][..], &[0xA2; 7], &[0, 0xA2, 0], &[0; 5]].concat());
    // What does not fit is refused, with what it needs.
    let changed = BTreeMap::from([(0xA_0963, vec![0xA2; 13]), (0xA_096A, vec![]), (0xA_0960, vec![])]);
    let refused = program::write_strings(&file, &rooms, &changed).unwrap_err();
    assert!(refused.starts_with("string program.657760 found no place: "), "{refused}");
    assert!(refused.contains("16 bytes here with the zero byte that ends each, and this pack's need 17"), "{refused}");
    assert!(refused.ends_with("\n  program.657760 needs 1 byte"), "{refused}");
}

#[test]
fn an_instruction_that_builds_two_addresses_keeps_its_string_where_it_is() {
    // The third string's `addiu` is reached with `a0` given its high half on one way and another
    // high half on the other: a new low half for the string would change the other address too.
    let (mut file, strings) = made_up_program();
    let address = 0xA_096A + program::PROGRAM_BASE;
    let code = [
        mips::LUI << 26 | 4 << 16 | address >> 16,
        0x04 << 26 | 5 << 21 | 2, // beq a1, zero: on to the `addiu`, or through the second `lui`
        0,
        mips::LUI << 26 | 4 << 16 | ((address >> 16) + 1),
        mips::ADDIU << 26 | 4 << 21 | 4 << 16 | (address & 0xFFFF),
        mips::RETURN,
        0,
    ];
    for (index, word) in code.into_iter().enumerate() {
        file[0x800 + 4 * index..][..4].copy_from_slice(&word.to_le_bytes());
    }
    let references = program::references_in(&file);
    assert_eq!(references.code.built.get(&address), Some(&vec![0x810]));
    let rooms = program::rooms(&file, &strings, &references, &codes());
    assert_eq!(rooms[&0xA_096A].reached, None);
    assert!(rooms[&0xA_0960].reached.is_some());
}

#[test]
fn a_string_that_stays_is_told_the_room_its_tail_leaves_it() {
    // Something points inside the second string, whose room of 7 bytes holds its zero byte and a
    // piece of text of 2 bytes after it: 4 are left for its text.
    let (mut file, strings) = made_up_program();
    file[0xA_0A20..0xA_0A24].copy_from_slice(&(0xA_0964 + program::PROGRAM_BASE).to_le_bytes());
    let references = program::references_in(&file);
    let rooms = program::rooms(&file, &strings, &references, &codes());
    assert_eq!((rooms[&0xA_0963].reached.as_ref(), rooms[&0xA_0963].size), (None, 7));
    let changed = BTreeMap::from([(0xA_0963, vec![0xA2; 4])]);
    assert!(program::write_strings(&file, &rooms, &changed).is_ok());
    let changed = BTreeMap::from([(0xA_0963, vec![0xA2; 5])]);
    let refused = program::write_strings(&file, &rooms, &changed).unwrap_err();
    assert!(refused.starts_with("string program.657763 takes 5 bytes, and has 4: "), "{refused}");
}

#[test]
fn strings_given_the_same_text_are_written_apart_when_no_room_is_within_reach_of_both() {
    // One string's address is built by an `addiu` and reaches $A8800 to $B8800 of the file; the
    // other's by an `ori`, and reaches $B0800 to $C0800. Their rooms are each outside the other's
    // reach, so one copy cannot serve both, but each fits in its own room.
    let mut file = vec![0u8; 0xC_1000];
    let (first, second) = (0xB_0000u32, 0xB_9000u32);
    let high = |offset: u32| (offset + program::PROGRAM_BASE) >> 16;
    let low = |offset: u32| (offset + program::PROGRAM_BASE) & 0xFFFF;
    let code = [
        mips::LUI << 26 | 4 << 16 | (high(first) + 1), // the `addiu` adds its half as a negative number
        mips::ADDIU << 26 | 4 << 21 | 4 << 16 | low(first),
        mips::LUI << 26 | 5 << 16 | high(second),
        mips::ORI << 26 | 5 << 21 | 5 << 16 | low(second),
    ];
    for (index, word) in code.into_iter().enumerate() {
        file[0x800 + 4 * index..][..4].copy_from_slice(&word.to_le_bytes());
    }
    let room = |code: usize| program::Room {
        size: 8,
        tail: Vec::new(),
        reached: Some(program::Reached { words: Vec::new(), code: vec![code] }),
    };
    let rooms = BTreeMap::from([(first, room(0x804)), (second, room(0x80C))]);
    let changed = BTreeMap::from([(first, vec![0xA2; 5]), (second, vec![0xA2; 5])]);
    let written = program::write_strings(&file, &rooms, &changed).unwrap();
    assert_eq!((written.moved, written.once), (0, 0));
    let patches: BTreeMap<usize, Vec<u8>> = written.patches.into_iter().collect();
    let copy = [vec![0xA2; 5], vec![0; 3]].concat();
    assert_eq!((&patches[&(first as usize)], &patches[&(second as usize)]), (&copy, &copy));
}

#[test]
fn a_label_has_its_slot_and_the_width_of_its_screen() {
    let mut file = vec![0u8; 0x200];
    let mut put = |at: usize, label: &str| file[at..at + small(label).len()].copy_from_slice(&small(label));
    put(0x10, "@ﾄﾞｳｸﾞ"); // 6 bytes, 3 characters
    put(0x1A, "ｽﾃｰﾀｽ"); // 5 bytes, 5 characters, the last of its run
    put(0x90, "ｾｰﾌﾞ");
    let built = BTreeSet::from([0x10, 0x90]);
    let labels = program::menu_strings(&file, &built);
    assert_eq!(labels.len(), 3);
    let none = BTreeSet::new();
    // Up to the next label of the run; after the last, up to the next multiple of 4.
    assert_eq!(program::label_room(&file, 0x10, &labels, &none), 9);
    assert_eq!(program::label_room(&file, 0x1A, &labels, &none), 5);
    assert_eq!(program::label_room(&file, 0x90, &labels, &none), 7);
    assert_eq!(program::label_room(&file, 0x90, &labels, &BTreeSet::from([0x96])), 5);
    assert_eq!(program::label_cells(&small("@ﾄﾞｳｸﾞ")), 3);
    assert_eq!(program::label_width(0x10, &labels), 5);
    assert_eq!(program::label_width(0x90, &labels), 3);
    // A pack's lowercase letters are written as the capitals the font draws.
    assert_eq!(program::encode_label("Items").unwrap(), b"ITEMS");
    assert_eq!(program::encode_label("どうぐ").unwrap(), small("@ﾄﾞｳｸﾞ"));
    for text in ["a~b", "x_y", "a|b", "1^2", "`", "\\"] {
        assert!(program::encode_label(text).is_err(), "{text}");
    }
}

#[test]
fn each_part_s_room_on_screen_is_what_the_game_s_own_strings_take() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let codes = &super::program(disc).unwrap().codes;
    let text = program::program_text(disc).unwrap();
    let strings = &text["program"];
    for (index, part) in PARTS.iter().enumerate() {
        let end = PARTS.get(index + 1).map_or(u32::MAX, |next| next.start);
        let lines: Vec<Vec<usize>> =
            strings.range(part.start..end).map(|(_, data)| insert::line_cells(data, codes)).collect();
        let widest = *lines.iter().flatten().max().unwrap();
        let most = lines.iter().map(Vec::len).max().unwrap();
        // The names of the party have the room the name entry gives, 6 characters.
        let width = if part.start == program::NAMES { 6 } else { widest };
        assert_eq!(part.width, width, "part {:X}", part.start);
        if let program::Lines::Most(lines) | program::Lines::Running(lines) = part.lines {
            assert_eq!(lines, most, "part {:X}", part.start);
        }
    }
    assert!(program::OWN_WIDTH.iter().all(|offset| strings.contains_key(offset)));
    assert!(strings.range(program::NAMES..program::NAMES_END).count() == 8);
    // The room the strings have between them, as the kind's limits say; every string can move, and 63
    // have a piece of text after them that goes with them.
    let file = program::program_file(disc).unwrap();
    let references = program::references(disc).unwrap();
    let rooms = program::rooms(&file, strings, &references, codes);
    assert_eq!(rooms.values().map(|room| room.size).sum::<usize>(), 70_402);
    assert!(program::LIMITS.contains("70,402 bytes"));
    assert!(rooms.values().all(|room| room.reached.is_some()));
    assert_eq!(rooms.values().filter(|room| !room.tail.is_empty()).count(), 63);
    // What a monster does, and its effect, which nothing points at: "a sting!" and "poisoned!".
    assert_eq!(decode(&rooms[&0xB_775C].tail[..rooms[&0xB_775C].tail.len() - 1], codes), "ドクにおかされた！");
    // The main menu's labels have their slots, and the configuration's the 11 characters before the
    // settings.
    let labels = &text["menu"];
    assert_eq!(program::label_room(&file, 0xA_7D1C, labels, &references.referred), 9);
    assert_eq!(program::label_width(0xA_7D1C, labels), 8);
    assert_eq!(program::label_width(0xA_8244, labels), 11);
    assert_eq!(program::label_room(&file, 0xA_8244, labels, &references.referred), 34);
}
