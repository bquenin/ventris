use super::*;
use crate::testing;

/// A made-up script file with these pieces of code, laid out as the game's are: the trailer's offset,
/// a record per piece, the code, then the trailer and its list of pieces.
fn make_script(pieces: &[&[u8]]) -> Vec<u8> {
    let mut data = vec![0u8; 8];
    let records = data.len();
    data.resize(records + 20 * pieces.len(), 0);
    let mut codes = Vec::new();
    for code in pieces {
        codes.push(data.len());
        data.extend_from_slice(code);
    }
    data.resize(data.len().div_ceil(4) * 4, 0);
    let trailer = data.len();
    data.extend_from_slice(&TRAILER.to_le_bytes());
    let list = trailer + 8;
    data.extend_from_slice(&((list as u32) | OFFSET_BIT).to_le_bytes());
    for index in 0..pieces.len() {
        data.extend_from_slice(&((records + 20 * index) as u32 | OFFSET_BIT).to_le_bytes());
        data.extend_from_slice(&OFFSET_BIT.to_le_bytes());
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data[0..4].copy_from_slice(&((trailer as u32) | OFFSET_BIT).to_le_bytes());
    for (index, code) in pieces.iter().enumerate() {
        let record = records + 20 * index;
        data[record + 4..record + 8].copy_from_slice(&((codes[index] as u32) | OFFSET_BIT).to_le_bytes());
        data[record + 8..record + 12].copy_from_slice(&((code.len() / 2) as u32).to_le_bytes());
    }
    data
}

fn op(op: u8, args: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    push_instruction(&mut out, op, args);
    out
}

#[test]
fn a_script_is_read_into_pieces_and_strings() {
    // あい, a reading over the second, a wait, a line, う; then a box closing (0xAD) ends the string.
    let first: Vec<u8> = [
        op(0xA9, &[]),
        op(TEXT, &[0x0E, 0x0F]),
        op(RUBY, &[1]),
        op(TEXT, &[0x0F]),
        op(RUBY_END, &[]),
        op(WAIT, &[0x1E]),
        op(LINE, &[]),
        op(TEXT, &[0x10]),
        op(LINE, &[]),
        op(0xAA, &[]),
        op(0xAD, &[]),
    ]
    .concat();
    let second: Vec<u8> =
        [op(0x20, &[0xFFFF, 0x40, 0x98]), op(TEXT, &[0x0E]), op(0x20, &[0xFFFF, 0x90, 0x98]), op(TEXT, &[0x0F])]
            .concat();
    let data = make_script(&[&first, &second]);
    let found = file_strings("test.dat", &data).unwrap();
    let texts: Vec<String> = found.iter().map(|(_, code)| decode(code)).collect();
    assert_eq!(texts, ["あい{RUBY:0001}い{RUBY_END}{WAIT:001E}\nう", "あ", "い"]);
    // A string is numbered by where its first character is; the box opening before it is not part of it.
    assert_eq!(found[0].0 as usize, 8 + 40 + 2);
    for (_, code) in &found {
        assert_eq!(encode(&decode(code)).unwrap(), bytes_of(code));
    }
}

#[test]
fn tags_are_written_and_read_back() {
    let code = vec![
        Instruction { offset: 0, op: TEXT, args: vec![0x0E, 0x5DB] },
        Instruction { offset: 0, op: 0x07, args: vec![5] },
        Instruction { offset: 0, op: 0x0D, args: vec![] },
        Instruction { offset: 0, op: TEXT, args: vec![0x0F] },
    ];
    let text = decode(&code);
    assert!(text.starts_with("あ"), "{text}");
    assert!(text.ends_with("{07:0005}{0D}い"), "{text}");
    assert_eq!(encode(&text).unwrap(), bytes_of(&code));
    assert_eq!(encode("{K:000E}").unwrap(), encode("あ").unwrap());
    assert_eq!(encode("{BOGUS}").unwrap_err().to_string(), "unknown tag {BOGUS}");
    assert_eq!(encode("{03:0001}").unwrap_err().to_string(), "unknown tag {03:0001}");
    assert!(encode("{07:05}").is_err());
    assert!(encode("{K:1000}").is_err());
    assert_eq!(ADAPTER.unmapped("あ{K:01AC}い{K:12}"), ["01AC"]);
}

#[test]
fn a_damaged_script_is_refused_in_words() {
    let data = make_script(&[&op(TEXT, &[1])]);
    let mut cut = data.clone();
    cut[0] = 0;
    assert!(pieces("x.dat", &cut).unwrap_err().to_string().starts_with("x.dat is damaged"));
    let mut overrun = data.clone();
    overrun[8 + 20] = 9; // the instruction claims nine arguments, past the end of its piece
    assert!(file_strings("x.dat", &overrun).is_err());
}

#[test]
fn sections_are_named_for_their_files() {
    assert_eq!(section_of("ss_demo_a1_n0.dat").as_deref(), Some("demo_a1_n0"));
    assert_eq!(section_of("ss_create_font.dat").as_deref(), Some("system"));
    assert_eq!(section_of("ss_test_shop.dat"), None);
    assert_eq!(section_of("ssfont.dat"), None);
    assert_eq!(file_of("system"), "ss_create_font.dat");
    assert_eq!(file_of("npc_town1_n0"), "ss_npc_town1_n0.dat");
}

#[test]
fn the_table_gives_each_character_one_glyph() {
    let table = table();
    let mut seen = HashMap::new();
    for (glyph, char) in table.chars.iter().enumerate() {
        if let Some(char) = char {
            assert!(seen.insert(*char, glyph).is_none(), "{char} is given to two glyphs");
        }
    }
    // The hiragana start the font, after the digits and four signs.
    assert_eq!(table.chars[0x0E], Some('あ'));
    assert_eq!(table.chars[0x5F], Some('ゴ'));
}

// --- with the game ---------------------------------------------------------

#[test]
fn the_script_is_extracted_and_survives_a_round_trip() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    let pack = extract(&rom).unwrap();
    assert_eq!(pack.texts.len(), 169);
    let strings: usize = pack.texts.values().map(BTreeMap::len).sum();
    assert_eq!(strings, 3164);
    verify_script(&rom, &pack.texts).unwrap();
    let first = &pack.texts["demo_a1_n0"];
    assert!(first.values().any(|text| text.starts_with("なるほど")));
}

#[test]
fn every_glyph_the_script_uses_has_a_character() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    let pack = extract(&rom).unwrap();
    let mut missing: Vec<String> =
        pack.texts.values().flat_map(BTreeMap::values).flat_map(|text| ADAPTER.unmapped(text)).collect();
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "glyphs with no character: {missing:?}");
}

#[test]
fn the_font_is_read() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    // The font's files are where their numbers say.
    let files = FileSystem::read(&rom).unwrap();
    assert_eq!(files.names[&FONT_FILE_NUMBER], FONT_FILE);
    assert_eq!(files.names[&FONT_RECORDS_FILE_NUMBER], FONT_RECORDS_FILE);
    // あ fills its square; a narrow glyph, the capital A of the ASCII table, only its left half.
    let wide = glyph(&rom, 0x0E).unwrap();
    assert!(wide.iter().flatten().any(|pixel| *pixel == 15));
    let narrow = glyph(&rom, 0x5CA).unwrap();
    assert!(narrow.iter().any(|row| row[..8].iter().any(|pixel| *pixel != 0)));
    assert!(narrow.iter().all(|row| row[8..].iter().all(|pixel| *pixel == 0)));
}

// --- building --------------------------------------------------------------

#[test]
fn a_script_is_laid_out_again_with_a_longer_string() {
    // Two pieces; the first string of the first grows by an odd number of instructions' bytes.
    let first: Vec<u8> = [op(0xA9, &[]), op(TEXT, &[0x0E]), op(0xAD, &[])].concat();
    let second: Vec<u8> = [op(TEXT, &[0x0F, 0x10]), op(0x0F, &[])].concat();
    let data = make_script(&[&first, &second]);
    let strings = file_strings("x.dat", &data).unwrap();
    let (number, code) = &strings[0];
    let longer = encode("あいうえお\nか").unwrap();
    let replaced = BTreeMap::from([(*number as usize, (bytes_of(code).len(), longer.clone()))]);
    let built = rebuild_script("x.dat", &data, &replaced).unwrap();
    // The pieces are found again through the moved trailer and records, each with its new length.
    let texts: Vec<String> = file_strings("x.dat", &built).unwrap().iter().map(|(_, code)| decode(code)).collect();
    assert_eq!(texts, ["あいうえお\nか", "いう"]);
    assert_eq!(pieces("x.dat", &built).unwrap().len(), 2);
    assert_eq!(built.len() % 4, data.len() % 4);
    // With nothing replaced, the file is the same.
    assert_eq!(rebuild_script("x.dat", &data, &BTreeMap::new()).unwrap(), data);
}

#[test]
fn the_storyteller_is_written_across_by_one_argument() {
    let code: Vec<u8> = [op(0x62, &[0]), op(VERTICAL, &[1]), op(TEXT, &[0x0E])].concat();
    let data = make_script(&[&code]);
    let across = written_across("x.dat", &data).unwrap();
    let expected: Vec<u8> = [op(0x62, &[0]), op(VERTICAL, &[0]), op(TEXT, &[0x0E])].concat();
    assert_eq!(across, make_script(&[&expected]));
}

#[test]
fn lines_are_measured_in_cells_of_8_dots() {
    let advance = |char: char| if char.is_ascii() { 8 } else { 16 };
    let width = |glyph: u16| if glyph == 0x5CA { 8 } else { 16 };
    let wrapped = |text: &str| wrap(text, 160, &advance, &width).unwrap();
    let letters = |count: usize| "a".repeat(count);
    // 17 letters, a space, then a letter and a wide character: the wide one would end past 160 dots.
    assert_eq!(wrapped(&format!("{} Bあ", letters(17))), format!("{}\nBあ", letters(17)));
    // A glyph written as a tag takes the room of its glyph, and goes to the next line whole.
    let tags = "{K:05CA}{K:0100}";
    assert_eq!(wrapped(&format!("{} {tags}", letters(18))), format!("{}\n{tags}", letters(18)));
    // 20 cells fit; breaks and other tags are kept.
    let text = format!("{} {{WAIT:001E}}b\nc d", letters(18));
    assert_eq!(wrapped(&text), text);
    assert_eq!(wrapped(&format!("{} bb", letters(18))), format!("{}\nbb", letters(18)));
    // A new box puts the pen back at the left: without it, the same letters run past the edge.
    let text = format!("{}{{0D}}{{0A}}{} c", letters(15), "b".repeat(15));
    assert_eq!(wrapped(&text), text);
    let text = format!("{} {} c", letters(5), "b".repeat(15));
    assert_eq!(wrapped(&text), format!("{}\n{} c", letters(5), "b".repeat(15)));
}

#[test]
fn new_glyphs_go_at_the_end_of_the_font() {
    // A made-up font of 1,500 glyphs of 4 units each, and its records.
    let mut font = vec![0u8; 32 + 1500 * 128 + 4];
    font[0] = 1;
    let mut records = vec![0u8; 0xD8];
    records[0..4].copy_from_slice(&(0xD8 | OFFSET_BIT).to_le_bytes());
    records[4..8].copy_from_slice(&1500u32.to_le_bytes());
    for index in 0..1500u16 {
        for value in [index * 4, 16, 16] {
            records.extend_from_slice(&value.to_le_bytes());
        }
    }
    records.extend_from_slice(&[0x10, 0, 0x10, 0]);
    let added = AddedGlyphs::new(&['A', 'é'], &fonts::Font::built_ins(&["unifont"])).unwrap();
    assert_eq!((added.numbers[&'A'], added.numbers[&'é']), (1500, 1501));
    assert_eq!((added.advance('A'), added.advance('あ'), added.advance('.')), (8, 16, 8));
    let (pixels, table) = added.extend(&font, &records).unwrap();
    assert_eq!(u32::from_le_bytes(table[4..8].try_into().unwrap()), 1502);
    // Each new glyph is on a unit after the old pixels, 8 dots wide, two tiles.
    let record = |index: usize| {
        let at = 0xD8 + 6 * index;
        [0, 2, 4].map(|step| u16::from_le_bytes([table[at + step], table[at + step + 1]]) as usize)
    };
    let [unit, width, height] = record(1500);
    assert_eq!((width, height), (8, 16));
    assert!(32 + unit * 32 >= font.len());
    assert!(pixels[32 + unit * 32..32 + unit * 32 + 64].iter().any(|byte| *byte != 0));
    assert_eq!(record(1501)[0], unit + 2);
    // The old records and what follows them are kept.
    assert_eq!(&table[16..0xD8 + 6 * 1500], &records[16..0xD8 + 6 * 1500]);
    assert_eq!(&table[table.len() - 4..], &[0x10, 0, 0x10, 0]);
    assert!(pixels.len() <= 32 + u32::from_le_bytes(table[12..16].try_into().unwrap()) as usize * 64);
}

#[test]
fn a_built_game_reads_back_the_pack() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    let original = extract(&rom).unwrap();
    // Japanese only, from the game's own font: the second string of a scene put in place of the first,
    // which makes the file longer.
    let scene = &original.texts["demo_a1_n0"];
    let (first, second) = (scene[&1032].clone(), scene[&1060].clone());
    let mut pack = Pack::default();
    pack.texts.insert("demo_a1_n0".into(), BTreeMap::from([(1032, second.clone())]));
    pack.texts.insert("system".into(), BTreeMap::from([(3056, "あいう".into())]));
    let (built, _) = build(&rom, &pack, &BuildOptions::default()).unwrap();
    let back = extract(&built).unwrap();
    assert_eq!(back.texts["system"][&3056], "あいう");
    let numbers = |texts: &BTreeMap<u32, String>| texts.keys().copied().collect::<Vec<_>>();
    // A string keeps its place among the others; those after it have moved in the file, so they are
    // numbered from there now, and read the same.
    let rebuilt: Vec<&String> = back.texts["demo_a1_n0"].values().collect();
    let own: Vec<&String> = scene.values().collect();
    assert_eq!(rebuilt[0], &second);
    assert_eq!(rebuilt[1..], own[1..]);
    assert_ne!(first, second);
    assert_eq!(numbers(&back.texts["npc_town1_n0"]), numbers(&original.texts["npc_town1_n0"]));
    // A build that changes nothing is the game.
    assert_eq!(build(&rom, &Pack::default(), &BuildOptions::default()).unwrap().0, rom);
}

#[test]
fn latin_letters_are_added_and_lines_broken_at_spaces() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    let mut pack = Pack::default();
    let line = "So you are the band of thieves the whole town is talking about?";
    pack.texts.insert("demo_a1_n0".into(), BTreeMap::from([(6244, line.into())]));
    let (built, summary) = build(&rom, &pack, &BuildOptions::default()).unwrap();
    assert!(summary.iter().any(|line| line.starts_with("font:")), "{summary:?}");
    let files = FileSystem::read(&built).unwrap();
    let records = files.file_at(&built, FONT_RECORDS_FILE).unwrap();
    let count = u32::from_le_bytes(records[4..8].try_into().unwrap()) as usize;
    assert!(count > GLYPH_COUNT);
    // Read back with the characters the build gave the added glyphs, the line is broken at spaces.
    let mut added: Vec<char> = line.chars().filter(char::is_ascii_graphic).collect();
    added.sort_unstable();
    added.dedup();
    let back = extract(&built).unwrap();
    let mut text = back.texts["demo_a1_n0"][&6244].clone();
    for (index, char) in added.iter().enumerate() {
        text = text.replace(&format!("{{K:{:04X}}}", GLYPH_COUNT + index), &char.to_string());
    }
    assert_eq!(text, "So you are the band\nof thieves the whole\ntown is talking\nabout?");
}

#[test]
fn the_storyteller_option_writes_across() {
    let Some(rom) = testing::cartridge("goemon-nds") else { return };
    let options = BuildOptions { switches: [HORIZONTAL.to_string()].into(), ..BuildOptions::default() };
    let (built, _) = build(&rom, &Pack::default(), &options).unwrap();
    let files = FileSystem::read(&built).unwrap();
    let script = files.file_at(&built, &file_of(STORYTELLER)).unwrap();
    let vertical = pieces("s.dat", script)
        .unwrap()
        .into_iter()
        .flat_map(|piece| instructions("s.dat", script, piece).unwrap())
        .filter(|instruction| instruction.op == VERTICAL && instruction.args == [1])
        .count();
    assert_eq!(vertical, 0);
    assert_ne!(built, rom);
}
