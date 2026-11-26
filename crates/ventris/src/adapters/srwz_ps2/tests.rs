use crate::adapter::declaration_faults;
use crate::text::from_hex;

use super::*;

fn hex(bytes: &str) -> Vec<u8> {
    from_hex(bytes).unwrap()
}

/// A table of glyphs for codes from 0x989F up, made up here: 翅 (E3C0) draws glyph 1059.
fn table() -> HashMap<u16, usize> {
    HashMap::from([(0xE3C0, 1059)])
}

fn decoded(bytes: &str) -> String {
    decode(&hex(bytes), Breaks::Byte, &table())
}

fn encoded(text: &str) -> Result<Vec<u8>> {
    encode(text, Breaks::Byte, &table())
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
    Some(Disc::open(&crate::testing::game("srwz-ps2")?).unwrap())
}

#[test]
fn the_adapter_declares_itself_properly() {
    assert_eq!(declaration_faults(&ADAPTER), Vec::<String>::new());
    assert!(!ADAPTER.can_insert());
    let kind = |section: &str| ADAPTER.kind_of(section).map(|kind| kind.name);
    assert_eq!(
        (kind("stg_010"), kind("stg_003a"), kind("stg_clear"), kind("flow")),
        (Some("dialogue"), Some("dialogue"), Some("dialogue"), Some("dialogue"))
    );
    assert_eq!((kind("stg_010-info"), kind("flow-info")), (Some("captions"), Some("captions")));
    assert_eq!(
        (kind("battle_267"), kind("battle_1"), kind("compdata"), kind("program")),
        (Some("battle"), None, Some("lists"), Some("menus"))
    );
    assert_eq!(
        (kind("library-robots"), kind("library-keywords"), kind("srpoint"), kind("tutorial")),
        (Some("library"), Some("library"), Some("help"), Some("help"))
    );
    assert_eq!(
        (kind("summary"), kind("credits"), kind("prologue"), kind("places"), kind("squads")),
        (Some("summaries"), Some("credits"), Some("prologue"), Some("places"), Some("squads"))
    );
    assert_eq!((kind("stg_"), kind("stage"), kind("library-weapons")), (None, None, None));
}

#[test]
fn the_font_draws_a_code_by_its_formula_below_989f_and_by_the_programs_table_above() {
    let table = table();
    let glyph = |char: char| glyph_of(CODES[&char], &table);
    // Checked against the font: a wide digit, capital and kana, the first kanji, the last of the
    // linear part, and the NEC row.
    assert_eq!((glyph('０'), glyph('Ａ'), glyph('ぁ'), glyph('ア')), (Some(207), Some(224), Some(287), Some(385)));
    assert_eq!((glyph('亜'), glyph('腕'), glyph('①'), glyph('Ⅱ')), (Some(1439), Some(4466), Some(1152), Some(1173)));
    assert_eq!(glyph('翅'), Some(1059));
    // A code from 0x989F up that the table lacks is drawn blank, and so is a first byte past 0x98.
    assert_eq!((glyph('弌'), glyph_of(0x989F, &table), glyph_of(0x9940, &table)), (None, None, None));
    assert_eq!((code_at(0), code_at(207), code_at(1439), code_at(GLYPH_COUNT - 1)), (0x8140, 0x824F, 0x889F, 0x987F));
    for index in 0..GLYPH_COUNT {
        assert_eq!(glyph_of(code_at(index) as u16, &table), Some(index));
    }
}

#[test]
fn characters_are_code_page_932_with_the_nec_row() {
    assert_eq!(
        (character(0x8140), character(0x82A0), character(0x8754), character(0x8794)),
        (Some('　'), Some('あ'), Some('Ⅰ'), Some('∑'))
    );
    assert_eq!((character(0x875E), character(0x8776), character(0x879D)), (None, None, None));
    // A character the NEC row has twice over is written with the standard's code.
    assert_eq!((CODES[&'≒'], CODES[&'∵'], CODES[&'∑']), (0x81E0, 0x81E6, 0x8794));
    assert_eq!(NEC_ROW.iter().map(|(trail, _)| *trail).max(), Some(0x9C));
    // The standard's reading is shown; the one Windows gives is taken as well.
    assert_eq!((character(0x8160), CODES[&'〜'], CODES[&'～']), (Some('〜'), 0x8160, 0x8160));
}

#[test]
fn a_string_is_characters_line_breaks_and_tags() {
    // 翅 is drawn through the table; the NEC row's duplicate of ≒ keeps its code; a level-2 kanji the
    // table lacks keeps its code too.
    assert_eq!(decoded("82 a0 0a 87 55 e3 c0 87 90 e3 c1"), "あ\nⅡ翅{G:8790}{G:E3C1}");
    assert_eq!(encoded("あ\nⅡ翅{G:8790}{G:E3C1}").unwrap(), hex("82 a0 0a 87 55 e3 c0 87 90 e3 c1"));
    // The battle lines write a line break as `\n`, and a byte 0x0A there is a byte like any other.
    let table = table();
    assert_eq!(decode(&hex("81 75 5c 6e 81 76 0a"), Breaks::Escaped, &table), "「\n」{0A}");
    assert_eq!(encode("「\n」{0A}", Breaks::Escaped, &table).unwrap(), hex("81 75 5c 6e 81 76 0a"));
    // What the font lacks is refused, in words a translator can act on.
    assert_eq!(message(encoded("Hi")), "the game's font has no 'H'; a Latin letter or a digit is written full-width");
    assert_eq!(message(encoded("{G:4142}")), "unknown tag {G:4142}");
}

#[test]
fn the_names_the_stages_fill_in_are_tags() {
    let text = "{HERO}{HERO_FULL}{HERO_SHORT}{HERO_FAMILY}{SQUAD}";
    assert_eq!(decoded("24 6e 24 46 24 66 24 6c 24 63"), text);
    assert_eq!(encoded(text).unwrap(), hex("24 6e 24 46 24 66 24 6c 24 63"));
    // `$` before any other letter is two bytes of their own.
    assert_eq!(decoded("24 78"), "{24}{78}");
    assert_eq!(message(encoded("{HERO:01}")), "unknown tag {HERO:01}");
}

#[test]
fn the_drawing_loops_codes_are_tags_with_their_bytes() {
    // 0x2E to 0x3D, each with the byte after it where it takes one.
    let bytes = "2e 2f 30 01 31 0e 32 10 33 09 34 10 35 09 36 02 37 03 38 04 39 05 3a 3b 3c 3d";
    let text = "{2E}{2F}{30:01}{31:0E}{WIDTH:10}{HEIGHT:09}{ADVANCE:10}{LINE_STEP:09}{36:02}{37:03}{38:04}{39:05}\
                {GIVEN_NAME}{FAMILY_NAME}{SHORT_NAME}{SQUAD_NAME}";
    assert_eq!(decoded(bytes), text);
    assert_eq!(encoded(text).unwrap(), hex(bytes));
    // The names of the codes that take a byte may be their numbers.
    assert_eq!(encoded("{32:10}{3A}").unwrap(), hex("32 10 3a"));
    // A code whose byte is missing, at the end of the string, is the byte alone.
    assert_eq!(decoded("82 a0 31"), "あ{31}");
    assert_eq!(encoded("あ{31}").unwrap(), hex("82 a0 31"));
    assert_eq!(message(encoded("{WIDTH}")), "{WIDTH} needs 1 parameter bytes");
    assert_eq!(message(encoded("{GIVEN_NAME:01}")), "{GIVEN_NAME:01} needs 0 parameter bytes");
    // Any other byte is its number; a zero byte ends the string and is no tag.
    assert_eq!(decoded("1f 41 b1"), "{1F}{41}{B1}");
    assert_eq!(encoded("{1F}{41}{B1}").unwrap(), hex("1f 41 b1"));
    assert_eq!(message(encoded("{00}")), "unknown tag {00}");
}

#[test]
fn icons_and_the_bracketed_code_are_kept_as_written() {
    // <14>, <-5> and [10], as the data lists' help writes them.
    let bytes = "3c 31 34 3e 82 a0 3c 2d 35 3e 5b 31 30 5d";
    assert_eq!(decoded(bytes), "{<14>}あ{<-5>}{[10]}");
    assert_eq!(encoded("{<14>}あ{<-5>}{[10]}").unwrap(), hex(bytes));
    // An unclosed one is the codes it is made of.
    assert_eq!(decoded("3c 31 34"), "{SHORT_NAME}{31:34}");
    assert_eq!(message(encoded("{<1a>}")), "unknown tag {<1a>}");
}

#[test]
fn text_is_told_from_what_merely_points_at_it() {
    let table = table();
    assert!(reads_as_text(&hex("82 a0 0a 24 6e 3a"), &[0x3A], &table));
    // No character; a byte that is no code; a code that is not allowed there; a blank level-2 kanji.
    assert!(!reads_as_text(&hex("0a 24 6e"), &[], &table));
    assert!(!reads_as_text(&hex("82 a0 41"), &[], &table));
    assert!(!reads_as_text(&hex("82 a0 3a"), &[], &table));
    assert!(!reads_as_text(&hex("e3 c1"), &[], &table));
    assert!(reads_as_text(&hex("31 0e 82 a0"), &PROGRAM_CODES, &table));
}

#[test]
fn a_summary_is_three_fields_shown_as_lines() {
    let table = table();
    let stored = hex("82 a0 00 82 a2 00");
    assert_eq!(decode_summary(&stored, &table), "あ\nい");
    assert_eq!(encode_summary("あ\nい", &table).unwrap(), stored);
    assert_eq!(decode_summary(&hex("00 00"), &table), "");
    assert_eq!(encode_summary("", &table).unwrap(), hex("00 00"));
    assert_eq!(encode_summary("\n\nあ", &table).unwrap(), hex("00 00 82 a0"));
    assert_eq!(message(encode_summary("あ\nい\nう\nえ", &table)), "a summary has 3 lines at most, and this one has 4");
}

#[test]
fn a_pack_counts_its_offsets_from_the_end_of_its_header() {
    // Two entries after a header of 0x18 bytes: 4 bytes at 0x18, 2 at 0x1C.
    let pack = hex("02 00 00 00 18 00 00 00 00 00 00 00 04 00 00 00 04 00 00 00 02 00 00 00 aa bb cc dd ee ff");
    assert_eq!(pack_entries(&pack), Some(vec![0x18..0x1C, 0x1C..0x1E]));
    assert_eq!(pack_entries(&pack[..0x1D]), None);
}

#[test]
fn a_help_page_is_items_of_a_header_and_a_string() {
    // A length of 0x18, then two items: an 8-byte header and あ, an 8-byte header and いう.
    let page = hex("18 00 01 00 11 00 01 00 01 00 82 a0 00 01 00 11 00 0b 00 01 00 82 a2 82 a4 00 00 00");
    let strings = help_strings(&page, 0..page.len()).unwrap();
    assert_eq!(strings, Numbered::from([(10, hex("82 a0")), (21, hex("82 a2 82 a4"))]));
    // A length that does not end where an item does.
    let mut wrong = page.clone();
    wrong[0] = 0x17;
    assert_eq!(help_strings(&wrong, 0..wrong.len()), None);
}

#[test]
fn a_battle_segment_finds_its_strings_through_its_records() {
    // No small or situation entries, two records; the text follows the records.
    let mut segment = hex("00 4f 00 00 00 00 02 00");
    segment.resize(BATTLE_FIRST_TABLE, 0xFF);
    segment.extend(hex("01 00 00 00 05 00 00 00 02 00 00 00 00 00 00 00"));
    let text = segment.len();
    segment.extend(hex("81 75 81 76 00 82 a0 5c 6e 82 a2 00 00 00"));
    let strings = battle_strings(&segment).unwrap();
    assert_eq!(strings.keys().copied().collect::<Vec<_>>(), [text as u32, text as u32 + 5]);
    assert_eq!(strings[&(text as u32 + 5)], hex("82 a0 5c 6e 82 a2"));
    // A record that points into the middle of a string.
    segment[BATTLE_FIRST_TABLE + 4] = 1;
    assert_eq!(battle_strings(&segment), None);
}

#[test]
fn a_library_entry_is_unmasked_and_read_chunk_by_chunk() {
    let mask = |bytes: Vec<u8>| unmask(&bytes); // the mask is its own inverse
    let mut entry = b"ZKANROBO".to_vec();
    entry.extend([0; 8]);
    entry.extend(b"DSIZ\x10\x00\x00\x00DATA\x20\x00\x00\x00");
    entry.extend(b"RBTN\x04\x00\x00\x00");
    entry.extend(hex("82 a0 00 00"));
    entry.extend(b"PRDC\x02\x00\x00\x00");
    entry.extend(hex("82 a2"));
    let mut block = hex("01 00 00 00 10 00 00 00 00 00 00 00");
    block.extend((entry.len() as u32).to_le_bytes());
    block.extend(mask(entry));
    let chunks = library_chunks(&block).unwrap();
    assert_eq!(chunks, [(*b"RBTN", hex("82 a0")), (*b"PRDC", hex("82 a2"))]);
    // 0x00 and 0x5E are left as they are.
    assert_eq!(unmask(&[0x00, 0x5E, 0xDF, 0x1F]), [0x00, 0x5E, 0x81, 0x41]);
}

#[test]
fn library_strings_are_numbered_by_entry_and_field() {
    for (section, fields) in LIBRARY_FIELDS {
        let numbers: BTreeSet<u32> = fields.iter().map(|(_, field, _)| *field).collect();
        assert_eq!(numbers.len(), fields.len(), "{section}");
        assert!(numbers.iter().all(|field| *field < LIBRARY_STRIDE));
    }
    assert_eq!(ADAPTER.note("library-robots", Some(3205)).as_deref(), Some("Entry 320: its description."));
    assert_eq!(ADAPTER.note("library-robots", Some(3208)), None);
    assert!(ADAPTER.note("stg_010-info", None).is_some() && ADAPTER.note("stg_010", None).is_none());
}

// --- on the game itself

#[test]
fn the_compressed_blocks_expand_to_the_sizes_the_game_expects() {
    let Some(disc) = game() else { return };
    // The IOP modules: one block that uses up the whole file.
    let modules = disc.read_from("MODULES/MODULE.BN", 0).unwrap();
    let (out, end) = lz_varint::decompress(&modules, 0).unwrap();
    assert_eq!((out.len(), end), (403_360, 183_661));
    // The data lists fill the memory from 0x6D6800 to where a stage's overlay is loaded.
    let lists = lz_varint::decompress(&disc.read_from(DATA_LISTS, 0).unwrap(), 0).unwrap().0;
    assert_eq!(lists.len() as u32, STAGE_BASE - DATA_LISTS_BASE);
    // The font, and the size the program sets aside for it.
    assert_eq!(font(&disc).unwrap().len(), GLYPH_COUNT * GLYPH_BYTES);
    let program = program_file(&disc).unwrap();
    assert_eq!(word(&program, program_offset(0x3F_7D68)), Some(0x13_B000));
    // Every stage block ends where the next starts, give or take the padding to 16.
    let offsets = stage_offsets(&disc).unwrap();
    let stages = disc.read_from(STAGES, 0).unwrap();
    for pair in offsets.windows(2) {
        let end = lz_varint::decompress(&stages, pair[0] as usize).unwrap().1;
        assert_eq!(end.next_multiple_of(16), pair[1] as usize);
    }
}

#[test]
fn the_glyph_table_and_the_font_agree_with_what_was_seen() {
    let Some(disc) = game() else { return };
    let table = looked_up(&disc).unwrap();
    // 229 entries, six codes among them twice.
    assert_eq!(table.len(), 223);
    assert_eq!(table.get(&0x998B), Some(&1012)); // 刹, there with 1012 and then 1053
    assert_eq!(table.get(&0xE3C0), Some(&1059));
    assert!(table.values().all(|glyph| (704..=1248).contains(glyph)));
    // Each looked-up glyph has ink, and so does that of あ; the space has none.
    let ink = |index: usize| glyph(&disc, index).unwrap().iter().flatten().any(|pixel| *pixel != 0);
    assert!(table.values().all(|glyph| ink(*glyph)));
    assert!(ink(glyph_of(0x82A0, &table).unwrap()) && !ink(0));
    assert_eq!(glyph(&disc, 0).unwrap().len(), GLYPH_SIZE);
    assert!(glyph(&disc, GLYPH_COUNT).is_err());
}

#[test]
fn every_body_of_text_has_the_strings_it_was_counted_to_have() {
    let Some(disc) = game() else { return };
    let script = extract(&disc).unwrap();
    let count = |kind: fn(&str) -> bool| -> (usize, usize) {
        let sections: Vec<_> = script.iter().filter(|(section, _)| kind(section)).collect();
        (sections.len(), sections.iter().map(|(_, items)| items.len()).sum())
    };
    // 70,794 lines and choices in 171 stages and 296 in the intermission.
    assert_eq!(count(is_stage_lines), (172, 71_090));
    assert_eq!(count(is_stage_info), (163, 1_894));
    assert_eq!(count(is_battle), (288, 58_740));
    assert_eq!(count(is_data_lists), (1, 3_435));
    assert_eq!(count(is_program), (1, 2_240));
    // Robots: 321 of 7 fields and one reading; characters: 411 of 6; keywords: 52 of 4.
    assert_eq!(count(is_library), (3, 321 * 7 + 1 + 411 * 6 + 52 * 4));
    assert_eq!(count(is_help), (2, 114 + 2_609));
    assert_eq!(count(is_summary), (1, 209));
    assert_eq!(count(is_credits), (1, 149));
    assert_eq!(count(is_prologue), (1, 28));
    assert_eq!(count(is_places), (1, 195));
    assert_eq!(count(is_squads), (1, 104));
    // The first line of a new game on Rand's route, seen on screen, and its address.
    assert_eq!(
        script["stg_010"][&0x6720],
        "ブレーカー\n「なあ、姉ちゃん…\n　さっきから言ってるだろ？\n　こいつは掘り出し物のお宝だって」"
    );
    // A stage's conditions, filled in with the hero's given name.
    assert!(script["stg_024-info"].values().any(|text| text == "{GIVEN_NAME}の撃墜。"));
    assert_eq!(script["summary"][&0], "");
    assert_eq!(script["program"][&0x33_A9F0], "メモリーカード（ＰＳ２）をチェックしています。");
    // The game's own text has two codes the font cannot draw, both in names in the credits.
    let unmapped: Vec<String> =
        script.values().flat_map(|items| items.values()).flat_map(|text| ADAPTER.unmapped(text)).collect();
    assert_eq!(unmapped, ["FABA", "FAE5"]);
    verify_script(&disc, &script).unwrap();
}

#[test]
fn every_string_of_the_data_lists_is_pointed_at_and_every_record_finds_text() {
    let Some(disc) = game() else { return };
    let lists = lz_varint::decompress(&disc.read_from(DATA_LISTS, 0).unwrap(), 0).unwrap().0;
    let strings = data_list_strings(&lists);
    // Every string after the tables, not only those pointed at.
    let mut all = 0;
    let mut at = DATA_LISTS_STRINGS;
    while at < lists.len() {
        if lists[at] != 0 {
            all += 1;
            at += zero_ended(&lists, at).map_or(lists.len(), <[u8]>::len);
        }
        at += 1;
    }
    assert_eq!((strings.len(), all), (3_435, 3_435));
}

#[test]
fn a_changed_string_is_refused_by_the_round_trip() {
    let Some(disc) = game() else { return };
    let mut script = extract(&disc).unwrap();
    script.get_mut("stg_010").unwrap().insert(0x6720, "ブレーカー".to_string());
    assert_eq!(
        verify_script(&disc, &script).unwrap_err().to_string(),
        "string stg_010.26400 does not survive a round trip through its script file"
    );
    let mut script = Script::new();
    script.insert("stg_999".to_string(), BTreeMap::new());
    assert_eq!(verify_script(&disc, &script).unwrap_err().to_string(), "stg_999 is not a part of this game's script");
}
