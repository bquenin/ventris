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
    // あ fills its square; a narrow glyph, the capital A of the ASCII table, only its left half.
    let wide = glyph(&rom, 0x0E).unwrap();
    assert!(wide.iter().flatten().any(|pixel| *pixel == 15));
    let narrow = glyph(&rom, 0x5CA).unwrap();
    assert!(narrow.iter().any(|row| row[..8].iter().any(|pixel| *pixel != 0)));
    assert!(narrow.iter().all(|row| row[8..].iter().all(|pixel| *pixel == 0)));
}
