use std::fs;
use std::path::PathBuf;

use sha1::{Digest, Sha1};

use crate::adapter::declaration_faults;
use crate::script::{format_block, parse_block, read_names, read_script, write_names, write_pack, write_script};

use super::*;

fn hex(bytes: &str) -> Vec<u8> {
    from_hex(bytes).unwrap()
}

fn encoded(text: &str) -> Result<Vec<u8>> {
    encode(text, &Extra::new())
}

/// What a translator is told when something is refused.
fn message<T>(result: Result<T>) -> String {
    match result {
        Err(error @ Error::Text(_)) => error.to_string(),
        Err(other) => panic!("refused, but not in words for a translator: {other:?}"),
        Ok(_) => panic!("it was not refused"),
    }
}

/// The game, without the bytes a copier may have put before it, or None, with a word about why,
/// where it is not there to test with.
fn game() -> Option<Vec<u8>> {
    crate::testing::cartridge("top-sfc")
}

fn block(pairs: &[(u32, &str)]) -> BTreeMap<u32, String> {
    pairs.iter().map(|(number, text)| (*number, (*text).to_string())).collect()
}

/// A pack of the strings given as (section, number, text), and no names.
fn pack_of(pairs: &[(&str, u32, &str)]) -> Pack {
    let mut texts = Script::new();
    for (section, number, text) in pairs {
        texts.entry((*section).to_string()).or_default().insert(*number, (*text).to_string());
    }
    Pack::of(texts)
}

/// A pack that only gives names.
fn names_of(names: &[(&str, &str)]) -> Pack {
    let names = names.iter().map(|(tag, name)| ((*tag).to_string(), (*name).to_string())).collect();
    Pack { texts: Script::new(), names }
}

/// What a user asks of a build: fonts for dialogue, and one for the menus.
fn with_fonts(fonts: &[&PathBuf], menu_font: Option<&PathBuf>) -> BuildOptions {
    BuildOptions {
        fonts: fonts.iter().map(|font| Font::from(*font)).collect(),
        menu_font: menu_font.map(Font::from),
        ..Default::default()
    }
}

/// Build a pack the way the command line does: what is reported, and the image as written,
/// with its checksum brought up to date.
fn built(rom: &[u8], pack: &Pack, options: &BuildOptions) -> Result<(Vec<String>, Vec<u8>)> {
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("out.sfc");
    let summary = ADAPTER.build(&Image::Cartridge(rom.to_vec()), pack, &output, options)?.summary;
    Ok((summary, fs::read(&output).unwrap()))
}

/// The image a pack builds with nothing asked for besides.
fn plainly_built(rom: &[u8], pack: &Pack) -> (Vec<String>, Vec<u8>) {
    built(rom, pack, &BuildOptions::default()).unwrap()
}

fn word(data: &[u8], at: usize) -> usize {
    usize::from(data[at]) | usize::from(data[at + 1]) << 8
}

/// Where the four bytes of an image's checksum are, having checked that they are right.
fn checksum_fields(image: &[u8]) -> usize {
    let header = snes::detect_header(image, snes::LIKELY_HEADER).unwrap();
    assert_eq!(header.checksum, snes::compute_checksum(image));
    header.offset + 0x1C
}

/// A glyph of 8 rows as a Unifont cell of 16, with four empty rows above and below.
fn cell(rows: &str) -> String {
    format!("{}{rows}{}", "00".repeat(4), "00".repeat(4))
}

#[test]
fn decode_covers_every_kind_of_code() {
    let data = hex("02 01 00 53 10 39 56 05 f7 01 a5 05 01 03 10 04 02 81 80 80 1f 86 80 a9 0c 7c ff ff");
    let text = decode(&data).unwrap();
    assert_eq!(text, "{クレス}\n「あれ？　樹精{PAGE}\n{SPEED:10}{COLOR:02}{NUM:801F}{STR:A90C}‥");
    assert_eq!(encoded(&text).unwrap(), data);
    // The numeric form of a speaker tag is still accepted.
    assert_eq!(encoded(&text.replace("{クレス}", "{NAME:01}")).unwrap(), data);
    assert_eq!(decode(&hex("02 03 02 04 02 06 02 02 ff ff")).unwrap(), "{アーチェ}{チェスター}{クラース}{ミント}");
}

#[test]
fn unmapped_and_unexpected_bytes_survive() {
    let data = hex("d0 06 d4 06 02 ff 10 ff ff"); // two font slots the dialogue never uses
    let text = decode(&data).unwrap();
    assert_eq!(text, "{K:06D0}{K:06D4}{NAME:FF}あ");
    assert_eq!(encoded(&text).unwrap(), data);
    let odd = hex("ff 20 03 ff ff"); // FF is not a glyph, and 03 has no parameter left
    assert_eq!(decode(&odd).unwrap(), "{RAW:FF}ち{RAW:03}");
    assert_eq!(encoded(&decode(&odd).unwrap()).unwrap(), odd);
    // Codes cut short by the end of the string, each kept as the bytes it is.
    for data in ["02 ff ff", "81 80 01 ff ff", "86 ff ff", "81 ff ff", "c6 0c ff ff", "80 00 ff ff"] {
        let raw = hex(data);
        assert_eq!(encoded(&decode(&raw).unwrap()).unwrap(), raw, "{data}");
    }
}

#[test]
fn a_page_break_followed_by_a_line_break() {
    let data = hex("10 01 00 11 01 ff ff");
    let text = decode(&data).unwrap();
    assert_eq!(text, "あ{PAGE}\n\nい{PAGE}\n");
    assert_eq!(encoded(&text).unwrap(), data);
    assert_eq!(encoded("あ{PAGE}い").unwrap(), hex("10 01 11 ff ff"));
}

#[test]
fn encode_rejects_bad_input() {
    assert_eq!(message(encoded("hello")), "character 'h' is not in the game's character table");
    assert_eq!(message(encoded("あい{PAGE")), "malformed tag at position 2: '{PAGE'");
    assert_eq!(message(encoded("{not a tag at all}")), "malformed tag at position 0: '{not a tag at al'");
    assert_eq!(message(encoded("{NAME:0102}")), "unknown or malformed tag {NAME:0102}");
    assert_eq!(message(encoded("{BOGUS}")), "unknown or malformed tag {BOGUS:}");
    assert_eq!(message(encoded("{PAGE:01}")), "unknown or malformed tag {PAGE:01}");
    assert_eq!(message(encoded("{RAW}")), "unknown or malformed tag {RAW:}");
    assert_eq!(message(encoded("{RAW:1 2 3}")), "tag {RAW:123} does not hold hexadecimal bytes");
    assert_eq!(message(decode(&[0x10])), "string does not end with FF FF");
    // Spaces in a tag's value are only there to be read.
    assert_eq!(encoded("{RAW:1 0 1 1}{K: 04 E8 }{NAME: 0 1}").unwrap(), hex("10 11 e8 04 02 01 ff ff"));
}

#[test]
fn split_and_join_strings() {
    let strings: [&[u8]; 3] = [&[0x10, 0xFF, 0xFF], &[0xFF, 0xFF], &[0x11, 0x12, 0xFF, 0xFF]];
    let block = join_strings(&strings).unwrap();
    assert_eq!(block[..6], hex("06 00 09 00 0b 00"));
    assert_eq!(split_strings(&block).unwrap(), strings);
    assert_eq!(message(split_strings(&hex("04 00 02 00 ff ff"))), "string offsets are not strictly ascending");
    for data in ["", "02", "03 00 00", "08 00 ff ff", "00 00"] {
        assert_eq!(message(split_strings(&hex(data))), "block does not start with a string offset table", "{data}");
    }
    assert_eq!(
        message(join_strings(&[vec![0; 0x8000], vec![0; 0x8000], vec![0]])),
        "dialogue block is larger than 64 KiB"
    );
}

#[test]
fn glyph_codes() {
    assert_eq!(glyph_index(0x05), 0);
    assert_eq!(glyph_index(0x7F), 0x7A);
    assert_eq!(glyph_index(0x0180), 0x7B);
    assert_eq!(glyph_index(0x0280), 0xFB);
    for index in 0..GLYPH_COUNT {
        assert_eq!(glyph_index(code_at(index)), index);
    }
    assert_eq!(code_at(GLYPH_COUNT - 1), LAST_CODE);
    assert_eq!((GLYPH_COUNT, TABLE.characters.len(), TABLE.codes.len()), (1601, 1510, 1510));
}

#[test]
fn a_script_file_is_written_and_read_back() {
    let texts = block(&[
        (0, "one line"),
        (1, "two\nlines{PAGE}\nmore"),
        (2, ""),
        (3, "ends with a break\n"),
        (4, "\n\nstarts with breaks"),
    ]);
    let content = format_block("07", &texts);
    assert!(content.starts_with("[07.000]\none line\n\n[07.001]\n"));
    assert_eq!(parse_block(&content, None).unwrap(), texts);
    assert_eq!(parse_block(&content, Some("07")).unwrap(), texts);
    assert!(parse_block(&format_block("01", &BTreeMap::new()), None).unwrap().is_empty());
    let unfinished = parse_block("[01.000]\ntext\n[01.001]\ntext\n\n", None).unwrap_err();
    assert!(matches!(unfinished, Error::Script(_)) && unfinished.to_string().contains("blank line"));
    let stray = parse_block("stray\n[01.000]\ntext\n\n", None).unwrap_err();
    assert!(matches!(stray, Error::Script(_)) && stray.to_string().contains("before the first"));
}

#[test]
fn a_pack_holds_only_the_strings_it_changes() {
    // Any subset, in any order.
    let sparse = block(&[(41, "later string"), (3, "earlier string\nsecond line")]);
    assert!(format_block("05", &sparse).starts_with("[05.003]\nearlier string\nsecond line\n\n[05.041]\n"));
    let written = "[05.041]\nlater string\n\n[05.003]\nearlier string\nsecond line\n\n";
    assert_eq!(parse_block(written, Some("05")).unwrap(), sparse);
    assert!(parse_block("[05.003]\na\n\n[05.003]\nb\n\n", None).unwrap_err().to_string().contains("appears twice"));
    assert!(parse_block("[06.003]\na\n\n", Some("05")).unwrap_err().to_string().contains("does not belong"));
    let folder = tempfile::tempdir().unwrap();
    let script = Script::from([("05".to_string(), sparse), ("09".to_string(), block(&[(0, "x")]))]);
    write_script(folder.path(), &script).unwrap();
    assert_eq!(read_script(folder.path()).unwrap(), script);
    fs::write(folder.path().join("07.txt"), "[08.000]\nwrong file\n\n").unwrap();
    assert!(read_script(folder.path()).unwrap_err().to_string().contains("07.txt"));
}

#[test]
fn the_names_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    assert!(read_names(path).unwrap().is_empty());
    let names = names_of(&[("クレス", "Cress"), ("クラース", "Claus F. Lester")]).names;
    write_names(path, &names, "").unwrap();
    assert_eq!(fs::read_to_string(path.join("names.txt")).unwrap(), "クレス = Cress\nクラース = Claus F. Lester\n");
    assert_eq!(read_names(path).unwrap(), names);
    fs::write(path.join("names.txt"), "# a comment\n\n  Mint=Mint  \n").unwrap();
    assert_eq!(read_names(path).unwrap(), [("Mint".to_string(), "Mint".to_string())]);
    for (content, fault) in
        [("Cress\n", "line 1: expected"), ("Cress = \n", "line 1: expected"), ("A = b\nA = c\n", "twice")]
    {
        fs::write(path.join("names.txt"), content).unwrap();
        let error = read_names(path).unwrap_err();
        assert!(matches!(error, Error::Script(_)) && error.to_string().contains(fault), "{content:?}: {error}");
    }
}

#[test]
fn characters_outside_the_table_get_codes_after_the_last_one() {
    let extra = Extra::from([('A', GLYPH_COUNT + 5), ('я', GLYPH_COUNT + 6)]);
    assert_eq!(encode("あAя", &extra).unwrap(), hex("10 c6 0c c7 0c ff ff"));
    assert_eq!(plain_characters("{クレス}\n「Hi{PAGE}\n{SPEED:10}"), ['「', 'H', 'i']);
    assert_eq!(message(encoded("A")), "character 'A' is not in the game's character table");
    assert_eq!(
        message(encode("A", &Extra::from([('A', 0x4000)]))),
        "too many different characters for the game's encoding"
    );
    assert_eq!(encode("A", &Extra::from([('A', 0x3FFF)])).unwrap(), hex("ff 7f ff ff"));
}

fn ten_pixels(_: char) -> Result<usize> {
    Ok(10)
}

#[test]
fn wrap_lines_breaks_at_spaces() {
    let wrap = |text: &str| wrap_lines(text, &ten_pixels, 50, &[]).unwrap();
    // Ten pixels a character and a 50-pixel line: a character may start while the pen is left of 50.
    assert_eq!(wrap("aaa bbb ccc"), "aaa\nbbb\nccc");
    assert_eq!(wrap("aa bb cc dd"), "aa bb\ncc dd");
    assert_eq!(wrap("aaaa b"), "aaaa\nb");
    // Five characters fit exactly; a sixth does not, and with no space to break at the line is left alone.
    assert_eq!(wrap("aaaaa"), "aaaaa");
    assert_eq!(wrap("aaaaaaaaaaaa"), "aaaaaaaaaaaa");
    assert_eq!(wrap("aaaaaaaa bb"), "aaaaaaaa\nbb");
    // Breaks already in the text are kept, and each line is measured on its own.
    assert_eq!(wrap("aa\nbb cc dd ee"), "aa\nbb cc\ndd ee");
    assert_eq!(wrap(""), "");
    assert_eq!(wrap("aa bb\n"), "aa bb\n");
}

#[test]
fn wrap_lines_measures_tags() {
    let wrap = |text: &str, limit: usize, names: &[(&str, usize)]| wrap_lines(text, &ten_pixels, limit, names).unwrap();
    // Tags with no width are ignored; a page break starts the measuring again.
    assert_eq!(wrap("{SPEED:10}aa bb{COLOR:02} cc", 50, &[]), "{SPEED:10}aa bb{COLOR:02}\ncc");
    assert_eq!(wrap("aa bb{PAGE}cc dd", 50, &[]), "aa bb{PAGE}cc dd");
    assert_eq!(wrap("aa bb{PAGE}\ncc dd ee", 50, &[]), "aa bb{PAGE}\ncc dd\nee");
    // A character's name takes the room its default (or the pack's replacement) needs.
    let names = [("クレス", 50)];
    assert_eq!(wrap("{クレス} aa bb", 100, &names), "{クレス} aa\nbb");
    assert_eq!(wrap("{クレス} aa bb", 200, &names), "{クレス} aa bb");
    assert_eq!(wrap("{クレス} aa bb", 100, &[("クレス", 10)]), "{クレス} aa bb");
    // Other values are only known when the game runs, so they are estimates.
    assert_eq!(wrap("{NUM:801F} aa bb", 100, &[]), "{NUM:801F} aa\nbb");
    assert_eq!((tag_width("NAME", &[]), tag_width("STR", &[]), tag_width("SPEED", &[])), (70, 112, 0));
}

#[test]
fn menu_text_encoding() {
    let chars = BTreeMap::from([(0x42, '・'), (0x87, 'う'), (0x8E, 'こ'), (MENU_BLANK, '　')]);
    let codes = menu_codes(&chars);
    let data = [0x8E, 0x87, MENU_LINE, 0x42, MENU_BLANK, 0x0A];
    let text = decode_menu(&data, &chars);
    assert_eq!(text, "こう\n・　{RAW:0A}"); // a byte with no character survives as a raw code
    assert_eq!(encode_menu(&text, &codes).unwrap(), data);
    assert_eq!(message(encode_menu("Item", &codes)), "character 'I' cannot be used in menu text yet");
    assert_eq!(message(encode_menu("こ{PAGE}う", &codes)), "menu text has no tags; found '{PAGE}う'");
    assert_eq!(message(encode_menu("{う", &codes)), "menu text has no tags; found '{う'");
    assert_eq!(encode_menu("{RAW:8E 87}{RAW}{RAW:}こ", &codes).unwrap(), [0x8E, 0x87, 0x8E]);
    // The Python reference stops with an internal error on this one.
    assert_eq!(message(encode_menu("{RAW:8E 8}", &codes)), "tag {RAW:8E 8} does not hold hexadecimal bytes");
    // What a Latin keyboard types is written with the menu font's own full-width forms, where it has them.
    let codes = HashMap::from([('Ａ', 1), ('１', 2), ('　', 3)]);
    assert_eq!(normalise_menu("A1 b{RAW:4A}{ A", &codes), "Ａ１　b{RAW:4A}{　Ａ");
}

#[test]
fn the_adapter_declares_itself_properly() {
    assert_eq!(declaration_faults(&ADAPTER), Vec::<String>::new());
    assert_eq!(AREA_RAM_SIZE, 16_480);
    assert!(KINDS[0].limits.contains("16,480 bytes"));
    assert_eq!(KINDS[1].max_length, Some(MENU_MAX_LENGTH));
    let kind = |section: &str| ADAPTER.kind_of(section).map(|kind| kind.name);
    assert_eq!(
        (kind("05"), kind("menu"), kind("5"), kind("005"), kind("0a"), kind("Menu")),
        (Some("dialogue"), Some("menu"), None, None, None, None)
    );
    // The even blocks hold events, not text.
    assert_eq!(
        [kind("01"), kind("99"), kind("00"), kind("02"), kind("98")],
        [Some("dialogue"), Some("dialogue"), None, None, None]
    );
    assert_eq!(ADAPTER.unmapped("あ{K:04E8}い{K:04e8}{K:12}{K:0CC5}{K:04E8"), ["04E8", "0CC5"]);
    assert!(std::ptr::addr_eq(ADAPTER.platform(), &snes::PLATFORM));
    assert!(ADAPTER.sets_names() && ADAPTER.takes_menu_font());
    // The limits a name is said to have are the ones a build holds it to.
    let limits = ADAPTER.name_limits();
    assert!(limits.contains(&format!("1 to {MAX_NAME_LENGTH} characters, without tags")), "{limits}");
    assert!(limits.contains(&format!("names of up to {NAME_RECORD_LENGTH},")), "{limits}");
    assert_eq!(ADAPTER.switches().iter().map(|(name, _)| *name).collect::<Vec<_>>(), ["renderer", "cartridge"]);
    assert_eq!((ADAPTER.glyph_size(), ADAPTER.glyph_count(), ADAPTER.code_at(0)), (16, 1601, 0x05));
}

#[test]
fn the_injected_code_assembles_to_what_the_python_reference_assembles() {
    // Each source with the addresses a build gives it, and the SHA-1 of what the reference makes of
    // it: every segment's address, three bytes with the lowest first, followed by its code. The
    // renderer has since gained the centring of place names, which the reference never had: its
    // SHA-1 is of what this source assembles to, so that any change to it is seen.
    type Symbols = [(&'static str, i64)];
    let sources: [(&str, &Symbols, usize, &str); 4] = [
        (
            include_str!("vwf.asm"),
            &[("CODE", 0x700000), ("WIDTHS", 0x710000), ("FONT_BANK", 0x72), ("PLACE_FIELD", 224)],
            725,
            "dd0c63978b932c9ff04169c481cd5ac4f51b1c45",
        ),
        (
            include_str!("moved.asm"),
            &[("MOVED_CODE", 0x705000), ("MOVED", 0x708400)],
            71,
            "c29bcd97ab91b8ca22e6867b1c7ac9582f9e3b9d",
        ),
        (
            include_str!("menu.asm"),
            &[("MENU_CODE", 0x706000), ("MENU_BANK", 0x710000)],
            132,
            "217ce66c7e9450ab3ed278aa84904fa437e9bc30",
        ),
        (
            include_str!("names.asm"),
            &[
                ("NAME_CODE", 0x704000),
                ("NAME_DEFAULTS", 0x708000),
                ("NAME_DEFAULTS_SIZE", 40),
                ("NAME_TRANSLATED", 0x708100),
                ("NAME_GLYPHS", 0x708600),
            ],
            158,
            "6826e681559286a00de03614f306faf9d1a2f8fd",
        ),
    ];
    for (source, symbols, size, sha1) in sources {
        let assembly = assemble(source, symbols).unwrap();
        let mut hash = Sha1::new();
        for (address, code) in &assembly.segments {
            hash.update(&address.to_le_bytes()[..3]);
            hash.update(code);
        }
        let found: String = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(assembly.segments.iter().map(|(_, code)| code.len()).sum::<usize>(), size);
        assert_eq!(found, sha1);
    }
    // The renderer is one call in the game, the game's count of a place name's characters, which it
    // replaces with a call, the four calls into the engine its centring needs and where a place name
    // goes on after each character, the jump there, and the routines they go to: the renderer's 412
    // bytes, then the centring.
    let renderer = assemble(sources[0].0, sources[0].1).unwrap();
    let places: Vec<(u32, usize)> = renderer.segments.iter().map(|(address, code)| (*address, code.len())).collect();
    assert_eq!(places, [(0xC8DDFB, 4), (0xC8DB51, CENTRING_HOOK), (0xC8DE21, 3), (0x700000, 665)]);
    assert_eq!(renderer.segments[0].1, hex("22 00 00 70"));
    assert_eq!(renderer.segments[1].1[..7], hex("22 9c 01 70 4c 9f db"));
    // The code it replaces runs to $C8:DB9E, and the drawing loop it goes on to starts after it.
    const { assert!(CENTRING_HOOK <= 0xDB9F - 0xDB51) };
}

#[test]
fn the_real_script_survives_a_round_trip() {
    let Some(rom) = game() else { return };
    let blocks = read_blocks(&rom).unwrap();
    assert_eq!(blocks.len(), 100);
    let mut count = 0;
    for (_, block) in blocks.iter().enumerate().filter(|(index, _)| is_text_block(*index)) {
        for string in split_strings(block).unwrap() {
            let text = decode(string).unwrap();
            // Every character used by the dialogue has a table entry.
            assert!(!text.contains("{RAW:") && !text.contains("{K:"), "{text}");
            assert_eq!(encoded(&text).unwrap(), string);
            count += 1;
        }
    }
    assert_eq!(count, 3201);

    let game = extract(&rom).unwrap();
    assert_eq!(game.texts.keys().filter(|section| is_dialogue(section)).count(), 50);
    // Fifty areas of dialogue, the menus, and the captions.
    assert_eq!(game.texts.len(), 52);
    assert_eq!(game.texts.values().map(BTreeMap::len).sum::<usize>(), 3201 + 929 + 622);
    // The captions: what a battle says, a skill's name in kanji, a phrase that strings share.
    let captions = &game.texts[CAPTION_SECTION];
    assert_eq!((captions[&4].as_str(), captions[&14].as_str()), ("ＴＰが足りない", "魔神剣"));
    assert!(captions.values().any(|text| text.contains("{PHRASE:EA61}")));
    let values = caption_strings(&rom).unwrap();
    assert_eq!(values.iter().map(|string| 2 * (string.len() + 1)).sum::<usize>(), CAPTION_POOL.1 - CAPTION_POOL.0);
    // A pack that repeats them builds the game as it is.
    let same = Pack::of(Script::from([(CAPTION_SECTION.to_string(), captions.clone())]));
    assert!(build(&rom, &same, &BuildOptions::default()).unwrap().rom == rom);
    // One that changes a caption moves the pool, its two lists and the font to the expanded area.
    let items = BTreeMap::from([(4, "Not enough TP".to_string())]);
    let changed = Pack::of(Script::from([(CAPTION_SECTION.to_string(), items)]));
    let result = build(&rom, &changed, &BuildOptions::default()).unwrap();
    // Thirteen letters are seven pairs, the last one ending in a space, and seven cells are no
    // more than the game's own caption takes.
    assert_eq!((result.captions_changed.as_slice(), result.caption_glyphs), (&[4][..], 7));
    assert_eq!((result.caption_glyphs_replaced, result.captions_wider.len()), (0, 0));
    assert!(result.summary().contains(&"captions:  1 captions changed, with 7 glyphs added to their font".to_string()));
    let built = result.rom;
    // What the game finds for each number, read the way the game reads it: the number's place in
    // the list of numbers, the offset at that place in the other list, the pool from there on.
    let word = |image: &[u8], at: usize| usize::from(u16::from_le_bytes([image[at], image[at + 1]]));
    let found = |image: &[u8], bank: usize, base: usize, place: usize| -> Vec<u16> {
        let mut at = bank + base + word(image, bank + (CAPTION_OFFSETS & 0xFFFF) + 2 * place);
        let mut string = Vec::new();
        loop {
            let value = word(image, at) as u16;
            at += 2;
            if value == CAPTION_END {
                return string;
            }
            string.push(value);
            if value == CAPTION_PHRASE {
                string.push(word(image, at) as u16);
                at += 2;
            }
        }
    };
    // The pairs are numbered in the order of their letters, after the game's last glyph.
    let first = (GLYPH_COUNT as u16) + CAPTION_GLYPH_BIAS;
    let pairs = [" T", "No", "P ", "en", "gh", "ou", "t "];
    let new: Vec<u16> = ["No", "t ", "en", "ou", "gh", " T", "P "]
        .map(|pair| first + pairs.iter().position(|known| *known == pair).unwrap() as u16)
        .to_vec();
    let mut places = 0;
    while word(&rom, CAPTION_NUMBERS + 2 * places) != 0 {
        let game = found(&rom, 0x2F0000, CAPTION_POOL.0 & 0xFFFF, places);
        let now = found(&built, CAPTION_BANK, CAPTION_BASE, places);
        assert_eq!(now, if game == values[4] { new.clone() } else { game }, "place {places}");
        places += 1;
    }
    assert_eq!(places, 650);
    assert_eq!(built[CAPTION_BANK + 0xE5DA..CAPTION_BANK + 0xEAF0], rom[CAPTION_NUMBERS..CAPTION_OFFSETS]);
    // Nothing in the game's own bank changed: the pool and the lists are still there, unread.
    assert!(built[0x2F0000..0x300000] == rom[0x2F0000..0x300000]);
    // The game's glyphs are where they were in the copy of the font, and the new ones follow:
    // "P " has a letter on the left and nothing on the right.
    assert!(
        built[CAPTION_FONT..CAPTION_FONT + GLYPH_COUNT * GLYPH_BYTES] == rom[FONT..FONT + GLYPH_COUNT * GLYPH_BYTES]
    );
    let picture = &built[CAPTION_FONT + (GLYPH_COUNT + 2) * GLYPH_BYTES..][..GLYPH_BYTES];
    assert!(picture[..GLYPH_SIZE].iter().any(|row| *row != 0) && picture[GLYPH_SIZE..].iter().all(|row| *row == 0));
    // The routines look in the new places: the font's bank, the lists' bank, the pool's address.
    let at = |address: u32| snes::snes_to_pc(address, MAPPING).unwrap();
    assert_eq!([0xC8E6D8, 0xC8E785, 0xC9D4D3].map(|address| built[at(address)]), [0x7C; 3]);
    // The routine that draws dialogue as sprites keeps the game's font: no dialogue changed.
    assert_eq!(built[at(SPRITE_FONT_BANK_AT)], 0xF0);
    assert_eq!([0xC2B554, 0xC9D04F, 0xC9D40D, 0xC9D83C, 0xC1D32B].map(|address| built[at(address)]), [0x7D; 5]);
    assert_eq!(built[at(0xC2B262)..at(0xC2B262) + 3], [0x00, 0x01, 0x7D]);
    // A caption that does not exist is refused by its address.
    let missing = Pack::of(Script::from([(CAPTION_SECTION.to_string(), BTreeMap::from([(700, "a".to_string())]))]));
    assert_eq!(
        build(&rom, &missing, &BuildOptions::default()).unwrap_err().to_string(),
        "string captions.700 does not exist: the game has 622 captions"
    );
    // The characters' own names come out too, decoded through the game's naming-screen table.
    let names = ["クレス", "ミント", "アーチェ", "チェスター", "クラース"];
    assert_eq!(game.names, names.map(|name| (name.to_string(), name.to_string())));
    verify_script(&rom, &game.texts).unwrap();
    let mut changed = game.texts.clone();
    changed.get_mut("05").unwrap().insert(45, "あ".to_string());
    assert_eq!(
        message(verify_script(&rom, &changed)),
        "section 05 does not survive a round trip through its script file"
    );
    let stray = Script::from([("shop".to_string(), block(&[(0, "あ")]))]);
    assert_eq!(
        message(verify_script(&rom, &stray)),
        "section shop does not survive a round trip through its script file"
    );

    // The font, as the engine asks for it: the hiragana "a" has ink, the space has none.
    let image = Image::Cartridge(rom);
    let glyph = ADAPTER.glyph(&image, glyph_index(0x10)).unwrap();
    assert_eq!((glyph.len(), glyph[0].len()), (16, 16));
    assert!(glyph.iter().flatten().all(|pixel| *pixel <= 1) && glyph.iter().flatten().any(|pixel| *pixel == 1));
    assert!(ADAPTER.glyph(&image, 0).unwrap().iter().flatten().all(|pixel| *pixel == 0));
}

#[test]
fn a_real_script_is_put_back() {
    let Some(rom) = game() else { return };
    let mut script = extract(&rom).unwrap();

    // An unedited script rebuilds the original image exactly, through its files as well.
    let (summary, same) = plainly_built(&rom, &script);
    assert!(same == rom);
    assert_eq!(summary, ["changed:   nothing, the script matches the game"]);
    let folder = tempfile::tempdir().unwrap();
    write_pack(folder.path(), &script).unwrap();
    assert!(plainly_built(&rom, &crate::script::read_pack(folder.path()).unwrap()).1 == rom);
    assert!(plainly_built(&rom, &Pack::default()).1 == rom);

    // Edit one string, rebuild, and read the edit back out of the new image.
    let original = script.texts["01"][&0].clone();
    let edit = "あたらしい\nテキスト{PAGE}\nです。";
    script.texts.get_mut("01").unwrap().insert(0, edit.to_string());
    let (summary, built_rom) = plainly_built(&rom, &script);
    assert_eq!(summary, ["changed:   1 dialogue blocks (01)"]);
    assert!(built_rom != rom && built_rom.len() == rom.len());
    let fields = checksum_fields(&built_rom);
    let blocks = read_blocks(&built_rom).unwrap();
    let strings: Vec<String> = split_strings(&blocks[1]).unwrap().into_iter().map(|s| decode(s).unwrap()).collect();
    assert!(strings[0] == edit && strings[0] != original);
    assert!(strings.iter().eq(script.texts["01"].values()));
    // Every other block still decompresses to the same data, even where it moved.
    let before = read_blocks(&rom).unwrap();
    assert!((0..BLOCK_COUNT).all(|index| index == 1 || blocks[index] == before[index]));
    // The block still fits its slot, so nothing outside that slot and the checksum changed.
    let slot = block_offsets(&rom).unwrap()[1];
    let slot_end = slot + lzss_rle::block_size(&rom, slot).unwrap();
    assert_eq!(block_offsets(&built_rom).unwrap(), block_offsets(&rom).unwrap());
    assert!(built_rom[..slot] == rom[..slot]);
    assert!(built_rom[slot_end..fields] == rom[slot_end..fields] && built_rom[fields + 4..] == rom[fields + 4..]);

    // A pack holding just that one string builds the very same image.
    assert!(plainly_built(&rom, &pack_of(&[("01", 0, edit)])).1 == built_rom);

    // A pack can replace strings but not add them, and its tags must be ones the game has.
    let refused = |pack: &Pack| message(built(&rom, pack, &BuildOptions::default()));
    assert_eq!(refused(&pack_of(&[("01", 9999, "おまけ")])), "string 01.9999 does not exist: block 01 has 14");
    assert_eq!(refused(&pack_of(&[("01", 2, "あ{BOGUS:01}")])), "string 01.002: unknown or malformed tag {BOGUS:01}");
    assert_eq!(refused(&pack_of(&[("02", 0, "あ")])), "block 02 is not a dialogue block");
    assert_eq!(refused(&pack_of(&[("101", 0, "あ")])), "block 101 is not a dialogue block");
    assert_eq!(
        refused(&pack_of(&[("99999999999999999999999", 0, "あ")])),
        "block 99999999999999999999999 is not a dialogue block"
    );
    // A block may be named with more or fewer digits; of two names for one block the last is taken.
    let twice = pack_of(&[("001", 0, "あ"), ("1", 1, "い")]);
    assert!(plainly_built(&rom, &twice).1 == plainly_built(&rom, &pack_of(&[("01", 1, "い")])).1);

    // Every kanji once barely compresses, so the block outgrows its slot and moves to the expanded area.
    let mut codes: Vec<u32> = TABLE.characters.keys().copied().filter(|code| *code >= 0x0200).collect();
    codes.sort();
    let kanji: String = codes.iter().map(|code| TABLE.characters[code]).collect();
    script.texts.get_mut("01").unwrap().insert(0, kanji.clone());
    let (summary, grown) = plainly_built(&rom, &script);
    assert_eq!(
        summary,
        ["changed:   1 dialogue blocks (01)", "moved:     1 outgrew their space and went to the expanded area (01)"]
    );
    assert_eq!(grown.len(), EXPANDED_SIZE);
    let fields = checksum_fields(&grown);
    assert_eq!(block_offsets(&grown).unwrap()[1], SCRIPT_EXPANSION.0);
    assert_eq!(grown[BLOCK_TABLE + 3..BLOCK_TABLE + 6], [0x00, 0x00, 0x60]);
    let blocks = read_blocks(&grown).unwrap();
    assert_eq!(decode(split_strings(&blocks[1]).unwrap()[0]).unwrap(), kanji);
    assert!((0..BLOCK_COUNT).all(|index| index == 1 || blocks[index] == before[index]));
    // In the original 6 MiB only the vacated slot, the block's pointer and the checksum differ.
    assert!(grown[slot..slot_end].iter().all(|byte| *byte == 0xFF));
    let pointer = BLOCK_TABLE + 3;
    assert!(grown[..pointer] == rom[..pointer] && grown[pointer + 3..slot] == rom[pointer + 3..slot]);
    assert!(grown[slot_end..fields] == rom[slot_end..fields] && grown[fields + 4..rom.len()] == rom[fields + 4..]);
    let image = (Image::Cartridge(rom.clone()), Image::Cartridge(grown));
    assert_eq!(snes::PLATFORM.changes(&image.0, &image.1), ["size:      expanded from 6 to 8 MiB"]);

    // A string that memory cannot hold beside its area's events is refused, by its address.
    assert_eq!(
        refused(&pack_of(&[("01", 0, &"あ".repeat(20000))])),
        "string 01.000 is 5576 bytes too long for the memory the game has left beside the events of its area"
    );
    assert_eq!(refused(&pack_of(&[("01", 0, &"あ".repeat(70000))])), "dialogue block is larger than 64 KiB");
}

/// How many bytes of the game's count of a place name's characters the renderer writes over.
const CENTRING_HOOK: usize = 53;

#[test]
fn a_real_pack_with_new_characters() {
    let Some(rom) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let font = folder.path().join("font.hex");
    fs::write(&font, "0048:00000000424242427E42424242420000\n0069:000000080800180808080808083E0000\n").unwrap();
    let pack = pack_of(&[("05", 45, "{クレス}\n「Hi")]);

    // With no font given, the game's own fonts are drawn from.
    let (summary, _) = built(&rom, &pack, &BuildOptions::default()).unwrap();
    assert!(summary.contains(&"font:      2 characters the game's font lacks: Hi".to_string()), "{summary:?}");
    let font_without_i = folder.path().join("partial.hex");
    fs::write(&font_without_i, "0048:00000000424242427E42424242420000\n").unwrap();
    assert_eq!(
        message(built(&rom, &pack, &with_fonts(&[&font_without_i], None))),
        "the font has no glyph for 1 characters: i (U+0069)"
    );
    // Of several fonts, each character comes from the first that has it.
    let (_, from_two) = built(&rom, &pack, &with_fonts(&[&font_without_i, &font], None)).unwrap();

    let (summary, built_rom) = built(&rom, &pack, &with_fonts(&[&font], None)).unwrap();
    assert_eq!(
        summary,
        [
            "changed:   1 dialogue blocks (05)",
            "font:      2 characters the game's font lacks: Hi",
            "renderer:  variable-width renderer installed"
        ]
    );
    assert!(built_rom == from_two);
    assert_eq!(built_rom.len(), EXPANDED_SIZE);
    checksum_fields(&built_rom);

    // The string ends with the two new codes, which follow the game's last one.
    let blocks = read_blocks(&built_rom).unwrap();
    assert!(split_strings(&blocks[5]).unwrap()[45].ends_with(&hex("53 c6 0c c7 0c ff ff")));
    // The game's only call to its draw-glyph routine now goes to the renderer.
    let call = snes::snes_to_pc(0xC8DDFB, MAPPING).unwrap();
    assert_eq!(rom[call..call + 4], hex("22 a9 e5 c8"));
    assert_eq!(built_rom[call..call + 4], hex("22 00 00 70"));
    // The font holds the game's glyphs at their 14-pixel pitch, then the new ones at their own widths.
    let widths = &built_rom[RENDERER_WIDTHS..RENDERER_WIDTHS + GLYPH_COUNT + 2];
    assert!(widths[..GLYPH_COUNT].iter().all(|width| *width == 14) && widths[GLYPH_COUNT..] == [7, 6]);
    let record = RENDERER_FONT + GLYPH_COUNT * 64;
    // the bar of the H, then its outline
    assert_eq!(built_rom[record + 8 * 4..record + 8 * 4 + 4], hex("00 7e 00 ff"));
    let original = RENDERER_FONT + glyph_index(0x10) * 64;
    let rows = glyph(&rom, glyph_index(0x10));
    assert!((0..16).all(|row| word(&built_rom, original + 4 * row) == usize::from(rows[row])));
    // Dialogue the game shows as sprites is drawn by another routine, from a font of the game's
    // shape. It reads one that has the new letters at the same numbers, each in the middle of
    // the 14 dots a sprite is given: the H is 7 wide, so its bar starts 3 dots further right.
    let bank_at = snes::snes_to_pc(SPRITE_FONT_BANK_AT, MAPPING).unwrap();
    assert_eq!((rom[bank_at], built_rom[bank_at]), (0xF0, 0x7B));
    let own = GLYPH_COUNT * GLYPH_BYTES;
    assert!(built_rom[SPRITE_FONT..SPRITE_FONT + own] == rom[FONT..FONT + own]);
    let letter = SPRITE_FONT + own;
    assert_eq!((built_rom[letter + 8], built_rom[letter + GLYPH_SIZE + 8]), (0x0F, 0xC0));

    // The game's count that centres the name of a place now calls the renderer's own centring, and
    // goes on to the loop that draws the line; the four calls after are that routine's way into the
    // engine's own routines, and after them is where the engine goes on after each character of a
    // place name, which finishes a number before it looks for the end of the string. The engine's
    // jump back after such a character now goes there.
    let centring = snes::snes_to_pc(PLACE_CENTRING.0, MAPPING).unwrap();
    assert_eq!(rom[centring..centring + 12], PLACE_CENTRING.1);
    assert_eq!(built_rom[centring..centring + 7], hex("22 9c 01 70 4c 9f db"));
    let calls = "20 cc e4 6b ac c4 13 20 63 df c2 20 6b ac c4 13 20 40 e0 c2 20 6b c2 20 20 4d de c2 20 6b";
    let next_stub = "bd 0f 00 89 10 d0 03 4c 9f db 9e 2b 00 4c 26 dc";
    let stubs = format!("{calls} {next_stub}");
    assert_eq!(built_rom[centring + 7..centring + CENTRING_HOOK], hex(&stubs));
    let next = snes::snes_to_pc(PLACE_NEXT.0, MAPPING).unwrap();
    assert_eq!(rom[next..next + 9], PLACE_NEXT.1);
    assert_eq!(built_rom[next..next + 9], hex("a0 00 00 8c ca 13 4c 76 db"));

    // Installing the renderer alone changes only its call site, the centring of place names and the
    // jump after each of their characters, the bank of the sprite text's font and the checksum in the
    // original 6 MiB.
    let renderer = BuildOptions { switches: ["renderer".to_string()].into(), ..Default::default() };
    let (summary, plain) = built(&rom, &Pack::default(), &renderer).unwrap();
    assert_eq!(
        summary,
        ["changed:   nothing, the script matches the game", "renderer:  variable-width renderer installed"]
    );
    let fields = checksum_fields(&plain);
    let after = centring + CENTRING_HOOK;
    assert!(plain[..centring] == rom[..centring] && plain[after..call] == rom[after..call]);
    let jump = next + 6;
    assert!(plain[call + 4..jump] == rom[call + 4..jump] && plain[jump + 3..bank_at] == rom[jump + 3..bank_at]);
    assert!(plain[centring..after] == built_rom[centring..after] && plain[jump..jump + 3] == built_rom[jump..jump + 3]);
    assert_eq!(plain[bank_at], 0x7B);
    assert!(plain[bank_at + 1..fields] == rom[bank_at + 1..fields]);
    assert!(plain[fields + 4..rom.len()] == rom[fields + 4..]);
    assert_eq!(plain[call..call + 4], hex("22 00 00 70"));
}

#[test]
fn a_real_pack_that_renames_characters() {
    let Some(rom) = game() else { return };
    let folder = tempfile::tempdir().unwrap();
    let font = folder.path().join("font.hex");
    let glyphs = [
        ("0043", "3C42404040423C00"),
        ("0065", "00003C427E403C00"),
        ("0072", "00005C6240404000"),
        ("0073", "00003E403C027C00"),
    ];
    fs::write(&font, glyphs.map(|(code, rows)| format!("{code}:{}\n", cell(rows))).concat()).unwrap();
    let options = with_fonts(&[&font], None);
    // The second is the game's own name.
    let pack = names_of(&[("クレス", "Cress"), ("ミント", "ミント")]);
    let (summary, built_rom) = built(&rom, &pack, &options).unwrap();
    assert!(summary.contains(&"names:     1 characters renamed (クレス)".to_string()), "{summary:?}");
    assert_eq!(summary.last().unwrap(), "renderer:  variable-width renderer installed");
    checksum_fields(&built_rom);

    // Both places that convert a name for dialogue now go through the hook.
    for address in [0xC8DF9C, 0xC8E06D] {
        let call = snes::snes_to_pc(address, MAPPING).unwrap();
        assert_eq!(rom[call..call + 4], hex("22 c0 df c8"));
        assert_eq!(built_rom[call..call + 4], hex("22 00 40 70"));
    }
    // The hook recognises the default record and has the new name ready as glyph numbers plus $15.
    assert_eq!(built_rom[NAME_DEFAULTS..NAME_DEFAULTS + 8], hex("55 77 5a ff 00 00 00 00"));
    let first = GLYPH_COUNT + 0x15; // the new glyphs, in order: C e r s
    let slot = &built_rom[NAME_TRANSLATED..NAME_TRANSLATED + NAME_SLOT];
    let words: Vec<usize> = (0..12).step_by(2).map(|at| word(slot, at)).collect();
    assert_eq!(words, [first, first + 2, first + 1, first + 3, first + 3, 0xFFFF]);
    assert!(slot[12..].iter().all(|byte| *byte == 0));
    // A character the pack does not rename has an empty slot, which leaves it to the game.
    let second = NAME_TRANSLATED + NAME_SLOT;
    assert!(built_rom[second..second + NAME_SLOT].iter().all(|byte| *byte == 0));

    assert_eq!(
        message(built(&rom, &names_of(&[("Dhaos", "Dhaos")]), &options)),
        "Dhaos is not a character whose name can be set (クレス, ミント, アーチェ, チェスター, クラース)"
    );
    for name in ["CCCCCCCCCCCCCCCC", "C{PAGE}", "C\nC"] {
        assert_eq!(
            message(built(&rom, &names_of(&[("クレス", name)]), &options)),
            "the name for クレス must be 1 to 15 characters, without tags"
        );
    }
    // A name set in the menu text is what a new game starts with, in a record of eight bytes: it is
    // held to seven characters, whether or not names.txt gives the name as well.
    let number = name_string_numbers(&rom).unwrap()[0].1 as u32;
    let refused = format!(
        "the name for クレス, set in menu.{number:03}, must be 1 to 7 characters, without tags: that is what \
         the game keeps of a name. A longer one, for dialogue alone, goes in names.txt"
    );
    for name in ["CCCCCCCC", "C{PAGE}"] {
        let mut pack = pack_of(&[("menu", number, name)]);
        assert_eq!(message(built(&rom, &pack, &options)), refused);
        pack.names = names_of(&[("クレス", "Cress")]).names;
        assert_eq!(message(built(&rom, &pack, &options)), refused);
    }
}

#[test]
fn real_menu_text() {
    let Some(rom) = game() else { return };
    let chars = menu_characters(&rom).unwrap();
    let strings = menu_strings(&rom);
    assert_eq!(strings.len(), 929);
    let last = strings[strings.len() - 1];
    assert!(strings[0].0 == MENU_POOL.0 && last.0 + last.1.len() + 1 == MENU_POOL.1);
    // The menu encoding must be one-to-one.
    assert_eq!(chars.values().collect::<BTreeSet<_>>().len(), chars.len());
    let texts: Vec<String> = strings.iter().map(|(_, data)| decode_menu(data, &chars)).collect();
    assert!(!texts.iter().any(|text| text.contains("{RAW")));
    assert_eq!(texts[4..8], ["こうげき", "めいちゅう", "ぼうぎょ", "かいひ"]);

    // A string that is no longer than the original is rewritten where it is.
    let (summary, built_rom) = plainly_built(&rom, &pack_of(&[("menu", 4, "ぼうぎょ"), ("menu", 5, "かいひ")]));
    assert_eq!(summary[1], "menu:      2 menu strings changed");
    assert_eq!(built_rom.len(), rom.len());
    let after = menu_strings(&built_rom);
    let shown: Vec<String> = after[4..9].iter().map(|(_, data)| decode_menu(data, &chars)).collect();
    assert_eq!(shown, ["ぼうぎょ", "かいひ", "", "", "ぼうぎょ"]);
    // The shorter string leaves two terminators behind it, so the next original string still starts where it did.
    assert!(after[8] == strings[6] && after[9..] == strings[7..]);
    let (start, end) = MENU_POOL;
    let fields = checksum_fields(&built_rom);
    assert!(built_rom[..start] == rom[..start] && built_rom[end..fields] == rom[end..fields]);
    assert!(built_rom[fields + 4..] == rom[fields + 4..]);

    let refused = |pack: &Pack| message(built(&rom, pack, &BuildOptions::default()));
    // With no menu font given, the game's own is drawn from; what stops this pack is that no tile is free.
    assert_eq!(
        refused(&pack_of(&[("menu", 4, "Atk")])),
        "the menu font has room for 0 new characters and the pack needs 2 (kt); a tile only comes free once \
         every menu string that uses its kana is replaced"
    );
    // A longer string moves to the expanded area, and its old slot becomes a pointer to it.
    let (summary, grown) = plainly_built(&rom, &pack_of(&[("menu", 4, "こうげきこうげき")]));
    assert_eq!(summary[1], "menu:      1 menu strings changed");
    assert_eq!(grown.len(), EXPANDED_SIZE);
    checksum_fields(&grown);
    let (slot, original) = strings[4];
    let target = MENU_AREA.0;
    assert_eq!(grown[slot..slot + 5], [MENU_ESCAPE, target as u8, (target >> 8) as u8, 0xFF, 0xFF]);
    assert_eq!(grown[target..target + 9], [original, original, &[0xFF]].concat());
    assert!(grown[slot + 5..MENU_POOL.1] == rom[slot + 5..MENU_POOL.1]);
    // Every routine that reads menu text now fetches it through the hook.
    for (address, size) in [(0xC2B5E8, 6), (0xC24D02, 4), (0xC8E050, 4), (0xC9D014, 5), (0xC24957, 4)] {
        let site = snes::snes_to_pc(address, MAPPING).unwrap();
        // a call into the expanded area
        assert!(grown[site] == 0x22 && grown[site + 3] == 0x70, "${address:06X}");
        assert!(grown[site + 4..site + size].iter().all(|byte| *byte == 0xEA));
        assert!(grown[site..site + size] != rom[site..site + size]);
    }
    // A string that still fits needs none of this.
    let reader = snes::snes_to_pc(0xC2B5E8, MAPPING).unwrap();
    assert_eq!(built_rom[reader..reader + 6], hex("b9 00 70 29 ff 00"));

    let single = strings.iter().position(|(_, data)| data.len() == 1).unwrap() as u32;
    assert_eq!(
        refused(&pack_of(&[("menu", 4, &"あ".repeat(27))])),
        "string menu.004 is 27 characters; the longest a menu string can be is 26"
    );
    assert_eq!(
        refused(&pack_of(&[("menu", single, "ああ")])),
        format!("string menu.{single:03} cannot be made longer: the original is a single character")
    );
    assert_eq!(refused(&pack_of(&[("menu", 9999, "あ")])), "string menu.9999 does not exist: there are 929");
    assert_eq!(refused(&pack_of(&[("shop", 0, "あ")])), "shop is not a part of this game's script");
    assert_eq!(refused(&pack_of(&[("menu", 4, "こ{PAGE}")])), "string menu.004: menu text has no tags; found '{PAGE}'");
}

#[test]
fn real_menu_text_with_new_characters() {
    let Some(rom) = game() else { return };
    let chars = menu_characters(&rom).unwrap();
    let codes = menu_codes(&chars);
    let strings = menu_strings(&rom);
    let folder = tempfile::tempdir().unwrap();
    let font = folder.path().join("font8.hex");
    fs::write(&font, "006B:6060666C786C6600\n0074:1818\n0074:00307C3030301C00\n").unwrap(); // k, a line to skip, t
    let options = with_fonts(&[], Some(&font));

    // Plain capitals, digits and spaces are the menu font's own, so they need no new tile and no font.
    let (summary, built_rom) = plainly_built(&rom, &pack_of(&[("menu", 4, "AT 1")]));
    assert!(!summary.iter().any(|line| line.contains("menu font")));
    assert_eq!(menu_strings(&built_rom)[4].1, ['Ａ', 'Ｔ', '\u{3000}', '１'].map(|char| codes[&char]));

    // A new character needs the tile of a kana that nothing else uses, and in the original every kana is used.
    assert_eq!(
        message(built(&rom, &pack_of(&[("menu", 4, "Atk")]), &options)),
        "the menu font has room for 0 new characters and the pack needs 2 (kt); a tile only comes free once \
         every menu string that uses its kana is replaced"
    );

    // Once every string is replaced, the kana tiles are free.
    let mut full = Pack::default();
    let menu = full.texts.entry("menu".to_string()).or_default();
    for (number, (_, data)) in strings.iter().enumerate() {
        if !data.is_empty() {
            menu.insert(number as u32, "Ｘ".to_string());
        }
    }
    menu.insert(4, "Atk".to_string());
    let (summary, built_rom) = built(&rom, &full, &options).unwrap();
    assert!(summary.contains(&"menu font: 2 characters added: kt".to_string()), "{summary:?}");
    // The rebuilt font no longer fits where it was, so it moved.
    assert_eq!(built_rom.len(), EXPANDED_SIZE);
    checksum_fields(&built_rom);
    // Shortened strings leave terminators behind them, so read this one back by where the original was.
    let [capital, new_t, new_k, end] = built_rom[strings[4].0..strings[4].0 + 4] else { panic!() };
    assert_eq!((end, capital), (0xFF, codes[&'Ａ']));
    // The new characters took over kana tiles, in both copies of the font, and nothing else in them changed.
    assert!([new_k, new_t].iter().all(|byte| ('\u{3040}'..='\u{30FF}').contains(&chars[byte])));
    let glyphs = fonts::load_tiles(&Font::from(&font), &['k', 't']).unwrap();
    // The decompressor is hooked, and its table sends each font copy to its new place in the expanded area.
    let hook = snes::snes_to_pc(0xC7F4CF, MAPPING).unwrap();
    assert_eq!(rom[hook..hook + 5], hex("08 e2 20 64 7b"));
    assert_eq!(built_rom[hook..hook + 5], hex("5c 00 50 70 ea"));
    let table = &built_rom[MOVED_TABLE..MOVED_TABLE + 13];
    assert_eq!(table[12], 0);
    for (index, offset) in MENU_FONT_BLOCKS.into_iter().enumerate() {
        let entry = &table[6 * index..6 * index + 6];
        let address = |at: usize| (u32::from(entry[at]) << 16) | word(entry, at + 1) as u32;
        assert_eq!(address(0), snes::pc_to_snes(offset, MAPPING).unwrap());
        let moved = snes::snes_to_pc(address(3), MAPPING).unwrap();
        assert!((MOVED_AREA.0..MOVED_AREA.1).contains(&moved));
        let before = lzss_rle::decompress_block(&rom, offset).unwrap();
        // the original copy is left where it was
        assert!(lzss_rle::decompress_block(&built_rom, offset).unwrap() == before);
        let after = lzss_rle::decompress_block(&built_rom, moved).unwrap();
        assert_eq!((after.len(), before.len()), (192 * 16, 192 * 16));
        let tile = |font: &[u8], number: u8| font[usize::from(number) * 16..usize::from(number) * 16 + 16].to_vec();
        assert_eq!(tile(&after, new_k), fonts::shaded_tile(&glyphs[&'k'], 3));
        assert_eq!(tile(&after, new_t), fonts::shaded_tile(&glyphs[&'t'], 3));
        assert!(
            (0..192u8).all(|number| [new_k, new_t].contains(&number) || tile(&after, number) == tile(&before, number))
        );
    }
    // A kana the pack uses itself keeps its tile: the new character is given a different one.
    let kept = chars[&new_k];
    full.texts.get_mut("menu").unwrap().insert(5, kept.to_string());
    let (_, again) = built(&rom, &full, &options).unwrap();
    assert_eq!(again[strings[5].0], new_k); // the kana, still written with its own byte
    // "k" moved to another free tile
    assert!(![new_k, again[strings[4].0 + 1]].contains(&again[strings[4].0 + 2]));

    // A font that lacks a character says so in words for a translator.
    full.texts.get_mut("menu").unwrap().insert(5, "q".to_string());
    assert_eq!(message(built(&rom, &full, &options)), "the menu font has no 8x8 glyph for 1 characters: q (U+0071)");
}

#[test]
fn a_pack_moves_a_label_by_columns_and_rows() {
    // The table names every label once, by its string and which of the string's places it is.
    assert_eq!(LABELS.len(), 164);
    let names: BTreeSet<&str> = LABELS.iter().map(|label| label.name).collect();
    assert_eq!(names.len(), LABELS.len());
    let skills = LABELS.iter().find(|label| label.name == "menu.659#3").unwrap();
    assert_eq!((skills.string, skills.row_and_column(), skills.screen), (659, (15, 17), "status screen"));
    // `extract` says where each string is placed, under its header.
    let note = Top.note("menu", Some(659)).unwrap();
    assert!(note.starts_with("Placed where a pack can move it: menu.659#1 at column 3, row 2 on the main menu; "));
    assert!(note.ends_with("menu.659#3 at column 17, row 15 on the status screen."), "{note}");
    assert_eq!(Top.note("menu", Some(294)), None);

    let Some(rom) = game() else { return };
    // Every place in the table is what the game has there.
    for label in LABELS.iter() {
        assert_eq!(rom[label.offset..label.offset + 2], label.place.to_le_bytes(), "{}", label.name);
    }
    let moved = |layout: &[(&str, (i32, i32))]| {
        let layout = layout.iter().map(|(name, by)| (name.to_string(), *by)).collect();
        built(&rom, &Pack::default(), &BuildOptions { layout, ..Default::default() })
    };
    // Two columns to the left is four bytes less, and a row down is 64 more. Nothing else changes.
    let (_, image) = moved(&[("menu.659#3", (-2, 0)), ("menu.008#2", (1, 1))]).unwrap();
    let word = |image: &[u8], at: usize| u16::from_le_bytes([image[at], image[at + 1]]);
    let other = LABELS.iter().find(|label| label.name == "menu.008#2").unwrap();
    assert_eq!(word(&image, skills.offset), skills.place - 4);
    assert_eq!(word(&image, other.offset), other.place + 2 + 64);
    let fields = checksum_fields(&image);
    let same = |from: usize, to: usize| image[from..to] == rom[from..to];
    let (first, second) = (other.offset.min(skills.offset), other.offset.max(skills.offset));
    assert!(same(0, first) && same(first + 2, second) && same(second + 2, fields) && same(fields + 4, rom.len()));
    // A pack that moves nothing builds the game as it is.
    assert!(moved(&[]).unwrap().1 == rom);
    // The picture beside a label is placed in dots, eight to a column and to a row.
    let picture = &PICTURES[0];
    let (_, image) = moved(&[(picture.name, (1, -2))]).unwrap();
    assert_eq!((word(&rom, picture.left.0), word(&rom, picture.top.0)), (164, 164));
    assert_eq!((word(&image, picture.left.0), word(&image, picture.top.0)), (172, 148));
    let off = moved(&[(picture.name, (12, 0))]).unwrap_err().to_string();
    assert!(off.starts_with("picture.skills is 164 dots from the left and 164 from the top"), "{off}");
    // A name the game has no label for, and a move off the screen, are refused in words.
    let unknown = moved(&[("menu.659#9", (1, 0))]).unwrap_err().to_string();
    assert!(unknown.starts_with("'menu.659#9' is not a label a pack can move"), "{unknown}");
    let off = moved(&[("menu.659#3", (-18, 0))]).unwrap_err().to_string();
    assert!(off.starts_with("menu.659#3 is at column 17, row 15, and moving it by -18 columns"), "{off}");
}

#[test]
fn a_pack_joins_the_two_names_with_a_space() {
    // `extract` says what the sign is and how to close the gap, under the sign and each family name.
    let sign = Top.note("menu", Some(NAME_SIGN)).unwrap();
    assert!(sign.starts_with("Placed where a pack can move it: menu.759#1 at column 13, row 9 on the naming screen."));
    assert!(sign.ends_with("\"status.family\" = { columns = -2 }."), "{sign}");
    for number in FAMILY_NAMES.0..=FAMILY_NAMES.1 {
        assert!(Top.note("menu", Some(number)).unwrap().contains("\"status.family\""));
    }
    assert_eq!(Top.note("menu", Some(FAMILY_NAMES.1 + 1)), None);
    assert_eq!(FAMILY_NAME.row_and_column(), (6, 5));

    let Some(rom) = game() else { return };
    let strings = menu_strings(&rom);
    let chars = menu_characters(&rom).unwrap();
    assert_eq!(decode_menu(strings[NAME_SIGN as usize].1, &chars), "＝");
    assert_eq!(decode_menu(strings[FAMILY_NAMES.0 as usize].1, &chars), "アルベイン");
    assert_eq!(word(&rom, FAMILY_NAME.offset), usize::from(FAMILY_NAME.place));
    for (at, size) in NAME_COUNT {
        assert_eq!(rom[at..at + size], (NAME_COUNTED as u16).to_le_bytes()[..size]);
    }
    let fields = checksum_fields(&rom);
    // Bytes other than the checksum where an image differs from the game.
    let differences = |image: &[u8]| -> Vec<usize> {
        (0..rom.len()).filter(|at| image[*at] != rom[*at] && !(fields..fields + 4).contains(at)).collect()
    };

    // Moving the family name two columns to the left changes that one number by four.
    let layout = BTreeMap::from([(FAMILY_NAME.name.to_string(), (-2, 0))]);
    let (_, image) = built(&rom, &Pack::default(), &BuildOptions { layout, ..Default::default() }).unwrap();
    assert_eq!(word(&image, FAMILY_NAME.offset), 0x2186);
    assert_eq!(differences(&image), [FAMILY_NAME.offset]);
    let layout = BTreeMap::from([(FAMILY_NAME.name.to_string(), (-6, 0))]);
    let off = built(&rom, &Pack::default(), &BuildOptions { layout, ..Default::default() }).unwrap_err();
    assert!(off.to_string().starts_with("status.family is at column 5, row 6, and moving it by -6"), "{off}");

    // A name of seven characters in the menus is counted to the end; one of six leaves the count alone.
    let number = name_string_numbers(&rom).unwrap()[0].1 as u32;
    let (_, image) = plainly_built(&rom, &pack_of(&[("menu", number, "クレスクレスク")]));
    for (at, size) in NAME_COUNT {
        assert_eq!(image[at..at + size], (NAME_RECORD_LENGTH as u16).to_le_bytes()[..size]);
    }
    let (_, image) = plainly_built(&rom, &pack_of(&[("menu", number, "クレスクレス")]));
    assert!(NAME_COUNT.iter().all(|(at, size)| image[*at..*at + *size] == rom[*at..*at + *size]));
    // The sign as a space is one byte of the pool, the game's own blank.
    let (_, image) = plainly_built(&rom, &pack_of(&[("menu", NAME_SIGN, " ")]));
    let at = strings[NAME_SIGN as usize].0;
    assert_eq!((differences(&image), image[at]), (vec![at], MENU_BLANK));
}

#[test]
fn real_names_in_the_menus() {
    let Some(rom) = game() else { return };
    let chars = menu_characters(&rom).unwrap();
    let codes = menu_codes(&chars);
    let strings = menu_strings(&rom);
    let numbers = name_string_numbers(&rom).unwrap();
    for (tag, number) in &numbers {
        assert_eq!(decode_menu(strings[*number].1, &chars), *tag);
    }
    assert_eq!(numbers.iter().map(|(tag, _)| *tag).collect::<Vec<_>>(), SPEAKERS.map(|(_, tag)| tag));

    let folder = tempfile::tempdir().unwrap();
    let font = folder.path().join("font.hex"); // 8x16 glyphs for dialogue: C e r s
    let glyphs = [
        ("0043", "3C42404040423C00"),
        ("0065", "00003C427E403C00"),
        ("0072", "00005C6240404000"),
        ("0073", "00003E403C027C00"),
    ];
    fs::write(&font, glyphs.map(|(code, rows)| format!("{code}:{}\n", cell(rows))).concat()).unwrap();
    let menu_font = folder.path().join("font8.hex"); // 8x8 glyphs for the menus: e r s
    fs::write(&menu_font, glyphs[1..].iter().map(|(code, rows)| format!("{code}:{rows}\n")).collect::<String>())
        .unwrap();
    // Replace every other string, so that kana tiles come free for the new letters.
    let mut pack = names_of(&[("クレス", "Cress")]);
    let menu = pack.texts.entry("menu".to_string()).or_default();
    for (number, (_, data)) in strings.iter().enumerate() {
        if !data.is_empty() && !numbers.iter().any(|(_, name)| *name == number) {
            menu.insert(number as u32, "Ｘ".to_string());
        }
    }
    let both = with_fonts(&[&font], Some(&menu_font));
    let (summary, built_rom) = built(&rom, &pack, &both).unwrap();
    assert!(summary.contains(&"names:     1 characters renamed (クレス)".to_string()), "{summary:?}");
    assert!(summary.contains(&"menu font: 3 characters added: ers".to_string()), "{summary:?}");
    assert!(!summary.iter().any(|line| line.contains("menus keep")));

    // The name's menu string moved, being longer than the original, and holds the game's own capital
    // followed by the three new letters.
    let slot = strings[numbers[0].1].0;
    assert_eq!(built_rom[slot], MENU_ESCAPE);
    let target = MENU_AREA.0 & 0xFF0000 | word(&built_rom, slot + 1);
    let [capital, r, e, s, s2, end] = built_rom[target..target + 6] else { panic!() };
    assert_eq!((capital, s2, end), (codes[&'Ｃ'], s, 0xFF));
    assert!([r, e, s].iter().all(|byte| ('\u{3040}'..='\u{30FF}').contains(&chars[byte])));
    // A new game now starts with that as the name record, and dialogue recognises it.
    let record = &built_rom[NAME_DEFAULTS..NAME_DEFAULTS + 8];
    assert_eq!(record, [&built_rom[target..target + 6], &[0, 0]].concat());
    let first = GLYPH_COUNT + NAME_GLYPH_BIAS; // dialogue's new glyphs, in order: C e r s
    let name: Vec<usize> = (0..12).step_by(2).map(|at| word(&built_rom, NAME_TRANSLATED + at)).collect();
    assert_eq!(name, [first, first + 2, first + 1, first + 3, first + 3, 0xFFFF]);
    // Menu bytes are converted for dialogue through the pack's table: the new letters go to the new
    // glyphs, and everything else to what the game's own table says.
    let table = snes::snes_to_pc(NAME_GLYPH_TABLE, MAPPING).unwrap();
    for byte in 0..=255u8 {
        let expected = match byte {
            _ if byte == e => first + 1,
            _ if byte == r => first + 2,
            _ if byte == s => first + 3,
            _ => usize::from(rom[table + usize::from(byte)]) + NAME_GLYPH_BIAS,
        };
        assert_eq!(word(&built_rom, NAME_GLYPHS + 2 * usize::from(byte)), expected, "menu byte {byte:02X}");
    }
    // The captions' font draws menu strings and names too, a character to a cell. It gains the
    // three letters in their full-width forms, though no caption changed, and the two places that
    // turn a menu byte into a caption value read a table of words that knows them.
    let line = "captions:  0 captions changed, with 3 glyphs added to their font";
    assert!(summary.contains(&line.to_string()), "{summary:?}");
    let values = cpu_address(CAPTION_MENU_VALUES).unwrap();
    let reader =
        [&[0xDA, 0x0A, 0xAA, 0xBF, values as u8, (values >> 8) as u8, (values >> 16) as u8][..], &[0xEA; 6], &[0xFA]]
            .concat();
    for address in CAPTION_MENU_READERS {
        let at = snes::snes_to_pc(address, MAPPING).unwrap();
        assert_eq!(rom[at..at + 14], CAPTION_MENU_READER);
        assert_eq!(built_rom[at..at + 14], reader);
    }
    let bias = usize::from(CAPTION_GLYPH_BIAS);
    for byte in 0..=255u8 {
        let expected = match byte {
            _ if byte == e => GLYPH_COUNT + bias,
            _ if byte == r => GLYPH_COUNT + 1 + bias,
            _ if byte == s => GLYPH_COUNT + 2 + bias,
            _ => usize::from(rom[table + usize::from(byte)]) + bias,
        };
        let at = CAPTION_MENU_VALUES + 2 * usize::from(byte);
        assert_eq!(word(&built_rom, at), expected, "menu byte {byte:02X}");
    }
    // Each of the three is a picture, at a number the game's font has nothing at.
    for glyph in GLYPH_COUNT..GLYPH_COUNT + 3 {
        assert!(built_rom[CAPTION_FONT + glyph * GLYPH_BYTES..][..GLYPH_BYTES].iter().any(|byte| *byte != 0));
    }
    // The other characters keep their records.
    assert_eq!(built_rom[NAME_DEFAULTS + 8..NAME_DEFAULTS + 16], hex("6d 7a 61 ff 00 00 00 00"));

    // Where the menus cannot take a name, they keep the game's and dialogue still shows the new one.
    pack.names = names_of(&[("クレス", "Cresssss")]).names; // eight characters: one too many for the record
    let (summary, kept) = built(&rom, &pack, &both).unwrap();
    let skipped = "           the menus keep the game's names: the menus hold names of up to 7 characters, and クレス is \
                   longer";
    assert!(summary.contains(&skipped.to_string()), "{summary:?}");
    assert_eq!(kept[slot..slot + 4], rom[slot..slot + 4]);
    assert_eq!(kept[NAME_DEFAULTS..NAME_DEFAULTS + 8], hex("55 77 5a ff 00 00 00 00"));
    pack.names = names_of(&[("クレス", "Cress")]).names;
    // With no menu font given, the name's letters are drawn from the game's own font for its menus.
    let (summary, _) = built(&rom, &pack, &with_fonts(&[&font], None)).unwrap();
    assert!(summary.contains(&"menu font: 3 characters added: ers".to_string()), "{summary:?}");

    // A name given in the menu text alone is the name dialogue shows too.
    let mut in_menu = pack.clone();
    in_menu.names.clear();
    in_menu.texts.get_mut("menu").unwrap().insert(numbers[0].1 as u32, "Cress".to_string());
    let (summary, from_menu) = built(&rom, &in_menu, &both).unwrap();
    assert!(summary.contains(&"names:     1 characters renamed (クレス)".to_string()), "{summary:?}");
    assert!(from_menu == built_rom);
}

#[cfg(test)]
mod demand_tests {
    use super::super::*;

    fn pack_of(pairs: &[(&str, u32, &str)]) -> Pack {
        let mut texts = Script::new();
        for (section, number, text) in pairs {
            texts.entry((*section).to_string()).or_default().insert(*number, (*text).to_string());
        }
        Pack::of(texts)
    }

    /// The bytes at a CPU address of the image.
    fn at(rom: &[u8], address: u32, length: usize) -> &[u8] {
        let offset = snes::snes_to_pc(address, MAPPING).unwrap();
        &rom[offset..offset + length]
    }

    #[test]
    fn dialogue_on_the_cartridge_is_a_table_with_one_more_offset_and_the_strings() {
        // Three strings: the table has four offsets, the last being where the third string ends.
        let block = demand_block(&[&[1u8, 0xFF, 0xFF][..], &[0xFF, 0xFF], &[2, 3, 0xFF, 0xFF]]).unwrap();
        assert_eq!(block, [8, 0, 11, 0, 13, 0, 17, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF, 2, 3, 0xFF, 0xFF]);
        assert_eq!(demand_block::<&[u8]>(&[]).unwrap(), [2, 0]);
        let refused = demand_block(&[vec![0u8; 70000]]).unwrap_err().to_string();
        assert_eq!(refused, "dialogue block is larger than 64 KiB");
    }

    #[test]
    fn the_code_that_reads_from_the_cartridge_replaces_the_loader_step_and_the_two_lookups() {
        let symbols = [("DEMAND_CODE", 0x707000), ("DEMAND", 0x708800)];
        let assembly = assemble(include_str!("demand.asm"), &symbols).unwrap();
        let places: Vec<(u32, usize)> =
            assembly.segments.iter().map(|(address, code)| (*address, code.len())).collect();
        // The loader's five bytes, the eleven and the ten of the lookups, and the routines themselves.
        assert_eq!(places[..3], [(0xC67A16, 5), (0xC40D1E, 11), (0xC40EC8, 10)]);
        assert_eq!(places[3].0, 0x707000);
        assert_eq!(assembly.segments[0].1, [0x5C, 0x00, 0x70, 0x70, 0xEA]);
        // Each lookup is a call into the routines, and the rest of what it replaced does nothing.
        let (first, second) = (&assembly.segments[1].1, &assembly.segments[2].1);
        assert_eq!((first[0], first[2], first[3]), (0x22, 0x70, 0x70));
        assert!(first[4..].iter().chain(&second[4..]).all(|byte| *byte == 0xEA));
        // The second lookup enters one instruction earlier, to halve the number it has doubled.
        assert_eq!(second[..4], [0x22, first[1] - 1, 0x70, 0x70]);
        assert!(places[3].1 < 0x800, "the code must end before the tables at 0x708000");
    }

    /// With the game at hand: an area whose dialogue no longer fits in memory is built all the
    /// same, with its dialogue on the cartridge, and an area that fits is left to the game.
    #[test]
    fn an_area_too_large_for_memory_keeps_its_dialogue_on_the_cartridge() {
        let Some(rom) = crate::testing::cartridge("top-sfc") else { return };
        let original_loader = at(&rom, 0xC67A16, 5).to_vec();
        assert_eq!(original_loader, [0xA7, 0x16, 0x8D, 0x34, 0x14]); // LDA [$16], STA $1434
        assert_eq!(at(&rom, 0xC40D1E, 11), [0x0A, 0x18, 0x65, 0xD7, 0x85, 0x10, 0xA7, 0x10, 0x8D, 0xC4, 0x13]);
        assert_eq!(at(&rom, 0xC40EC8, 10), [0x18, 0x65, 0xD7, 0x85, 0x10, 0xA7, 0x10, 0x8D, 0xC4, 0x13]);

        // A small change fits: nothing of this is installed.
        let small = build(&rom, &pack_of(&[("03", 0, "あ")]), &BuildOptions::default()).unwrap();
        assert!(small.on_cartridge.is_empty());
        assert_eq!(at(&small.rom, 0xC67A16, 5), original_loader);

        // Twelve strings of 3,000 bytes: 36,000 bytes of dialogue for 16,480 of memory. String 03.000
        // is the village's name, held to one line, so they start after it.
        let long = "あ".repeat(3000);
        let strings: Vec<(&str, u32, &str)> = (1..=12).map(|number| ("03", number, long.as_str())).collect();
        let large = build(&rom, &pack_of(&strings), &BuildOptions::default()).unwrap();
        assert_eq!((large.changed.as_slice(), large.on_cartridge.as_slice()), (&[3][..], &[3][..]));
        assert!(large.moved.is_empty());
        let line =
            "cartridge: 1 are too large for the game's memory and are read from the cartridge a string at a time (03)";
        assert!(large.summary().contains(&line.to_string()), "{:?}", large.summary());
        assert_eq!(large.rom.len(), EXPANDED_SIZE);

        // The area's line in the table: where its dialogue is, and the room for its longest string.
        let entry = &large.rom[DEMAND_TABLE + 8..DEMAND_TABLE + 16];
        assert_eq!(entry[..3], [0x00, 0x00, 0x60]);
        // 3,000 bytes and the two that end a string, after the two bytes a string is copied past.
        assert_eq!(u16::from_le_bytes([entry[6], entry[7]]), 3006);
        // Every other area's line is empty, so the game unpacks those as it always has.
        let table = &large.rom[DEMAND_TABLE..DEMAND_TABLE + 8 * (BLOCK_COUNT / 2)];
        assert!(table[..8].iter().chain(&table[16..]).all(|byte| *byte == 0));

        // The dialogue on the cartridge holds the pack's strings and the game's own for the rest.
        let block = &large.rom[SCRIPT_EXPANSION.0..];
        let word = |at: usize| usize::from(u16::from_le_bytes([block[at], block[at + 1]]));
        let before = read_blocks(&rom).unwrap();
        let own = split_strings(&before[3]).unwrap();
        assert_eq!(word(0), 2 * (own.len() + 1));
        for (number, original) in own.iter().enumerate() {
            let string = &block[word(2 * number)..word(2 * number + 2)];
            if (1..=12).contains(&number) {
                assert_eq!(decode(string).unwrap(), long);
            } else {
                assert!(string == *original);
            }
        }
        // The game's own block is still where it was, and is no longer unpacked.
        let offsets = block_offsets(&rom).unwrap();
        let slot = lzss_rle::block_size(&rom, offsets[3]).unwrap();
        assert!(large.rom[offsets[3]..offsets[3] + slot] == rom[offsets[3]..offsets[3] + slot]);
        assert_eq!(at(&large.rom, 0xC67A16, 5), [0x5C, 0x00, 0x70, 0x70, 0xEA]);

        // The same on request for an area that would fit, and the result reads back as the pack.
        let asked = BuildOptions { switches: ["cartridge".to_string()].into(), ..Default::default() };
        let forced = build(&rom, &pack_of(&[("03", 0, "あ")]), &asked).unwrap();
        assert_eq!(forced.on_cartridge, [3]);
        // One string is too long for memory even alone: that is refused, by its address.
        let huge = "あ".repeat(15000);
        let refused = build(&rom, &pack_of(&[("03", 5, &huge)]), &BuildOptions::default()).unwrap_err().to_string();
        assert!(refused.starts_with("string 03.005 is "), "{refused}");
        assert!(refused.ends_with(" bytes too long for the memory the game has left beside the events of its area"));
    }
}

#[cfg(test)]
mod caption_tests {
    use super::super::*;

    #[test]
    fn a_caption_is_glyph_numbers_line_breaks_and_shared_phrases() {
        // あ is code 10 in the dialogue, so glyph 0x0B, and a caption holds that plus 0x10.
        let values = [0x001B, CAPTION_LINE, CAPTION_PHRASE, 0xEA61, 0x001B];
        let text = decode_caption(&values);
        assert_eq!(text, "あ\n{PHRASE:EA61}あ");
        assert_eq!(encode_caption(&text).unwrap(), values);
        // A value that is no glyph at all is kept as it is, and so is a phrase code with no number.
        let odd = [0x0001, CAPTION_PHRASE];
        let text = decode_caption(&odd);
        assert_eq!(text, "{RAW:0001}{RAW:000A}");
        assert_eq!(encode_caption(&text).unwrap(), odd);
        // A glyph the character table has no entry for is written as the dialogue writes its code.
        let unknown = (0..GLYPH_COUNT as u32).find(|glyph| !TABLE.characters.contains_key(&code_of_glyph(*glyph)));
        let value = unknown.unwrap() as u16 + CAPTION_GLYPH_BIAS;
        let unmapped = decode_caption(&[value]);
        assert!(unmapped.starts_with("{K:") && unmapped.len() == 8, "{unmapped}");
        assert_eq!(encode_caption(&unmapped).unwrap(), [value]);

        let refused = |text: &str| encode_caption(text).unwrap_err().to_string();
        assert_eq!(refused("a"), "a caption cannot hold 'a', which the game's font lacks");
        assert_eq!(refused("{PAGE}"), "a caption cannot hold the tag {PAGE}");
        assert_eq!(refused("{K:FFFF}"), "{K:FFFF} is not a character of the game's font");
    }

    #[test]
    fn narrow_characters_go_two_to_a_cell() {
        use CaptionCell::{New, Value};
        use CaptionGlyph::{Pair, Wide};
        // What the captions' font has, by width: Latin letters are narrow, a Cyrillic one too, and
        // a Hangul syllable is as wide as a cell.
        let widths: BTreeMap<char, usize> =
            " ab%+0:".chars().map(|char| (char, 8)).chain([('я', 8), ('한', 16)]).collect();
        let cells = |text: &str| caption_cells(text, Some(&widths)).unwrap();
        let blank = Value(CAPTION_GLYPH_BIAS);
        let a = Value(0x001B); // あ
        // Pairs are taken in the order the letters are written, and an odd run ends in a space.
        assert_eq!(cells("ab a"), [New(Pair('a', 'b')), New(Pair(' ', 'a'))]);
        assert_eq!(cells("aba"), [New(Pair('a', 'b')), New(Pair('a', ' '))]);
        // Two spaces are the game's own blank glyph, and so is its wide space, which takes a cell.
        assert_eq!(cells("ab  ab"), [New(Pair('a', 'b')), blank, New(Pair('a', 'b'))]);
        assert_eq!(cells("　ab"), [blank, New(Pair('a', 'b'))]);
        // A run ends at a line break, a phrase or a character of the game's font, and is filled up there.
        assert_eq!(cells("a\nb"), [New(Pair('a', ' ')), Value(CAPTION_LINE), New(Pair('b', ' '))]);
        assert_eq!(
            cells("a{PHRASE:EA62}b"),
            [New(Pair('a', ' ')), Value(CAPTION_PHRASE), Value(0xEA62), New(Pair('b', ' '))]
        );
        assert_eq!(cells("aあb"), [New(Pair('a', ' ')), a, New(Pair('b', ' '))]);
        // A wide character takes a cell of its own.
        assert_eq!(cells("a한я"), [New(Pair('a', ' ')), New(Wide('한')), New(Pair('я', ' '))]);
        // The game's font has a percent sign. Among the game's characters it is the game's, so the
        // game's captions read back as they are; next to a narrow character it is narrow.
        let percent = Value(encode_caption("%").unwrap()[0]);
        assert_eq!(cells("あ%あ"), [a, percent, a]);
        assert_eq!(cells("+0% a"), [New(Pair('+', '0')), New(Pair('%', ' ')), New(Pair('a', ' '))]);
        assert_eq!(cells("%a"), [New(Pair('%', 'a'))]);
        // A character neither font has is refused.
        assert_eq!(
            caption_cells("c", Some(&widths)).unwrap_err().to_string(),
            "a caption cannot hold 'c', which neither the game's font nor the captions' has"
        );
    }

    #[test]
    fn a_string_shown_as_sprites_is_laid_out_for_it() {
        let names = [("クレス", 5)];
        let laid = |text: &str| sprite_line(text, true, &names).unwrap();
        let wide = |text: &str| text.replace(' ', "\u{3000}");
        // Rows of 15 at most, broken at spaces, and every space the game's own, which it does not draw.
        assert_eq!(laid("Don't... defile the tree..."), wide("Don't... defile\nthe tree..."));
        assert_eq!(laid("A lone bridge joins it to the mainland."), wide("A lone bridge\njoins it to the\nmainland."));
        // A line break the translator wrote is kept, a tag that draws nothing takes no room, and
        // a name takes the room of its letters.
        assert_eq!(laid("A storied war\n~The Valhalla Campaign~"), wide("A storied war\n~The Valhalla\nCampaign~"));
        assert_eq!(
            laid("\"Can you{SPEED:10}...{SPEED:FF} hear my voice?\""),
            wide("\"Can you{SPEED:10}...{SPEED:FF}\nhear my voice?\"")
        );
        assert_eq!(laid("{クレス} is here now"), wide("{クレス} is here\nnow"));
        // A page holds 32 drawn characters, and a page break starts the count again.
        let full = "abcdefgh ijklmn\nopqrstuv wxyz\nABCDEF";
        assert_eq!(laid(full), wide(full));
        assert_eq!(laid(&format!("{full}{{PAGE}}\n{full}")), wide(&format!("{full}{{PAGE}}\n{full}")));
        let over = sprite_line(&format!("{full}{{PAGE}}\n{full} G"), true, &names).unwrap_err().to_string();
        assert!(over.contains("page 2 has more than the 32 characters a page can show"), "{over}");
        // A word longer than a row cannot be broken, and the game would cut it.
        let long = sprite_line("a abcdefghijklmnopq", true, &names).unwrap_err().to_string();
        assert!(long.contains("16 to a row, and a row of page 1 is longer"), "{long}");
        // A string with no space in it is in a script written without them, as the game's own
        // are: its rows are the game's to break, and it goes back as it was.
        let own = "あ".repeat(31);
        assert_eq!(sprite_line(&own, true, &names).unwrap(), own);
        assert!(sprite_line(&"あ".repeat(33), true, &names).is_err());
        // As written, when the build is told not to break lines: only the spaces change.
        // A row may then be the 16 the game holds, and no more.
        assert_eq!(sprite_line("abcdefg ijklmnop", false, &names).unwrap(), wide("abcdefg ijklmnop"));
        assert!(sprite_line("abcdefgh ijklmnop", false, &names).is_err());

        // `extract` says so under each of the eight strings, and at the head of two files.
        let note = |section: &str, number: Option<u32>| Top.note(section, number);
        assert!(note("01", Some(4)).unwrap().starts_with("The game shows this string as sprites"));
        assert_eq!(
            SPRITE_LINES.iter().filter(|(block, number)| note(&format!("{block:02}"), Some(*number)).is_some()).count(),
            8
        );
        assert_eq!((note("01", Some(3)), note("01", None), note("02", Some(4))), (None, None, None));
        assert!(note("captions", None).unwrap().contains("two narrow letters in a cell"));
        assert!(note("menu", None).unwrap().contains("a column of fixed width"));
        assert_eq!((note("captions", Some(4)), note("menu", Some(0))), (None, None));
    }

    #[test]
    fn a_new_glyph_sits_one_dot_to_the_left() {
        // A narrow shape with its outer columns set, and a wide one with every dot of a row set.
        let narrow = Glyph { rows: [0x8100; fonts::CELL], width: 8 };
        let wide = Glyph { rows: [0xFFFF; fonts::CELL], width: 16 };
        let shapes = BTreeMap::from([('a', narrow), ('한', wide)]);
        // Each half loses its first column and gains an empty last one.
        let pair = caption_picture(CaptionGlyph::Pair('a', 'a'), &shapes);
        assert!(pair.iter().all(|row| *row == 0x02));
        let alone = caption_picture(CaptionGlyph::Pair('a', ' '), &shapes);
        assert!(alone[..GLYPH_SIZE].iter().all(|row| *row == 0x02) && alone[GLYPH_SIZE..].iter().all(|row| *row == 0));
        let whole = caption_picture(CaptionGlyph::Wide('한'), &shapes);
        assert!(
            whole[..GLYPH_SIZE].iter().all(|row| *row == 0xFF) && whole[GLYPH_SIZE..].iter().all(|row| *row == 0xFE)
        );
        // A narrow character with a cell to itself is in its middle: columns 3 and 10 of the 16.
        let middle = caption_picture(CaptionGlyph::Alone('a'), &shapes);
        assert!(
            middle[..GLYPH_SIZE].iter().all(|row| *row == 0x10) && middle[GLYPH_SIZE..].iter().all(|row| *row == 0x20)
        );
        // A shape that has no empty row keeps its top row: there is nowhere to move it.
        assert!(pair[0] == 0x02 && whole[0] == 0xFF);
        // Unicode has a form as wide as a cell for the characters of ASCII, and for no others.
        assert_eq!(
            (full_width('a'), full_width('\''), full_width('я'), full_width(' ')),
            (Some('ａ'), Some('＇'), None, None)
        );
    }

    #[test]
    fn an_accent_in_the_top_row_moves_down_a_row() {
        // The game does not draw a glyph's top row. A shape with dots there and an empty row below
        // them has those rows moved down into it; the rest is where it was.
        let mut accent = [0x1800; fonts::CELL];
        (accent[1], accent[2], accent[3]) = (0x2400, 0, 0);
        let mut moved = accent;
        (moved[0], moved[1], moved[2]) = (0, 0x1800, 0x2400);
        assert_eq!(below_top_row(accent), moved);
        // A shape with nothing in its top row is left as it is, and so is one with no empty row.
        let mut low = accent;
        low[0] = 0;
        assert_eq!(below_top_row(low), low);
        assert_eq!(below_top_row([0x8100; fonts::CELL]), [0x8100; fonts::CELL]);

        // Unifont's capitals carry their accent in rows 0 and 1, with rows 2 and 3 empty; its plain
        // capitals and its small letters, accented or not, have nothing in row 0.
        let characters: Vec<char> = "ÉÂEaé ".chars().collect();
        let shapes = fonts::load_unifont(&Font::named("unifont"), &characters, false).unwrap();
        let rows = |char: char| shapes[&char].rows;
        assert!(rows('É')[0] != 0 && rows('É')[1] != 0 && rows('É')[2] == 0 && rows('É')[3] == 0);
        assert!(rows('E')[0] == 0 && rows('a')[0] == 0 && rows('é')[0] == 0);
        // The left half of a cell, and the right, as the font has them one dot to the left.
        let half = |row: u16| ((row >> 8 << 1) & 0xFF) as u8;
        let picture = |glyph| caption_picture(glyph, &shapes);
        let halves = |picture: [u8; GLYPH_BYTES]| (picture[..GLYPH_SIZE].to_vec(), picture[GLYPH_SIZE..].to_vec());

        // É with Â beside it: each accent is drawn a row lower, so both of its rows are seen, and
        // the letter under it is where the font has it.
        let (acute, circumflex) = halves(picture(CaptionGlyph::Pair('É', 'Â')));
        for (drawn, font) in [(acute, rows('É')), (circumflex, rows('Â'))] {
            assert_eq!(drawn[0], 0);
            assert_eq!(drawn[1..3], [half(font[0]), half(font[1])]);
            assert_eq!(drawn[3..], font[3..].iter().map(|row| half(*row)).collect::<Vec<_>>());
        }
        // Plain letters, and an accented small letter, are drawn as they always were.
        let (plain, small) = halves(picture(CaptionGlyph::Pair('E', 'é')));
        assert_eq!(plain, rows('E').iter().map(|row| half(*row)).collect::<Vec<_>>());
        assert_eq!(small, rows('é').iter().map(|row| half(*row)).collect::<Vec<_>>());
        let (a, blank) = halves(picture(CaptionGlyph::Pair('a', ' ')));
        assert_eq!(a, rows('a').iter().map(|row| half(*row)).collect::<Vec<_>>());
        assert!(blank.iter().all(|row| *row == 0));
        // A capital with a cell to itself, as on the naming screen, is moved the same way.
        let alone = picture(CaptionGlyph::Alone('É'));
        let middle = |row: u16| ((row & 0xFF00) >> 3).to_be_bytes();
        assert_eq!([alone[0], alone[GLYPH_SIZE]], [0, 0]);
        assert_eq!([alone[1], alone[GLYPH_SIZE + 1]], middle(rows('É')[0]));
        assert_eq!([alone[2], alone[GLYPH_SIZE + 2]], middle(rows('É')[1]));
    }
}

/// Party members' names as a place name meets them: (tag, characters in the record, name drawn).
/// A player can type letters of 14 dots for each, as the game's own characters are.
fn place_names_of<'a>(names: &[(&'a str, usize, &'a str)]) -> Vec<PlaceNameName<'a>> {
    let typed = (ORIGINAL_ADVANCE, ORIGINAL_ADVANCE);
    names.iter().map(|(tag, record, drawn)| PlaceNameName { tag, record: *record, drawn, typed }).collect()
}

#[test]
fn a_place_name_is_held_to_its_box() {
    // Two letters a pack adds: i, 4 dots wide, and W, 16. The game's own characters are 14.
    let extra = Extra::from([('i', 0x700), ('W', 0x701)]);
    let advance = |char: char| {
        Ok(match char {
            'i' => 4,
            'W' => 16,
            _ => ORIGINAL_ADVANCE,
        })
    };
    let names = place_names_of(&[("クレス", 5, "Cress"), ("ミント", 4, "Mint")]);
    // As the game's engine centres, by count, and as the routine the renderer brings does, by width.
    let by_count = |text: &str| place_name(text, &extra, &advance, &names, Centring::ByCount);
    let by_width = |text: &str| place_name(text, &extra, &advance, &names, Centring::ByWidth);
    let too_many = |count: usize| {
        format!(
            "the game shows this string as the name of a place, on one line of at most 16 characters, and it has \
             {count}: a party member's name counts as the letters the menus show for it, and one more"
        )
    };
    let too_wide = |dot: usize| {
        format!(
            "the game shows this string as the name of a place, centred on one line, and its last character would \
             start at dot {dot}, where the line ends at dot 208: use fewer or narrower letters"
        )
    };
    let too_wide_with = |said: &str, dot: usize| {
        format!(
            "the game shows this string as the name of a place, centred on one line, and with {said} its last \
             character would start at dot {dot}, where the line ends at dot 208: leave room for that, with fewer or \
             narrower letters"
        )
    };

    // Where the routine puts the pen: half of what the window's 224 dots leave, or its edge.
    assert_eq!(
        (PLACE_FIELD, place_name_pen(64), place_name_pen(83), place_name_pen(224), place_name_pen(240)),
        (224, 80, 70, 0, 0)
    );

    // By count, sixteen characters fit, and seventeen do not, however narrow. By width, only the line
    // limits them: thirty narrow letters are 120 dots.
    assert!(by_count(&"i".repeat(16)).is_ok());
    assert_eq!(message(by_count(&"i".repeat(17))), too_many(17));
    assert!(by_width(&"i".repeat(17)).is_ok() && by_width(&"i".repeat(30)).is_ok());
    // Sixteen of the game's own characters, 14 dots apart, put the last at dot 210, past the line,
    // both behind no space and centred on the 224 dots they fill.
    assert!(by_count(&"あ".repeat(15)).is_ok());
    assert_eq!(message(by_count(&"あ".repeat(16))), too_wide(210));
    assert!(by_width(&"あ".repeat(15)).is_ok());
    assert_eq!(message(by_width(&"あ".repeat(16))), too_wide(210));
    // By count, fourteen characters are centred behind one space of 14 dots: the last starts at 206,
    // or at 208.
    assert!(by_count(&format!("{}{}i", "あ".repeat(8), "W".repeat(5))).is_ok());
    assert_eq!(message(by_count(&format!("{}{}i", "あ".repeat(7), "W".repeat(6)))), too_wide(208));
    // By width, thirteen wide letters are 208 dots, centred from dot 8: the last starts at 200. A
    // fourteenth fills the window, which leaves the last at 208. A fifteenth is wider than the
    // window, which starts the name at its edge.
    assert!(by_width(&"W".repeat(13)).is_ok());
    assert_eq!(message(by_width(&"W".repeat(14))), too_wide(208));
    assert_eq!(message(by_width(&"W".repeat(15))), too_wide(224));
    // What fits behind the game's spaces may not fit centred, and the other way round: sixteen
    // characters, 208 dots, have no space before them by count and start at dot 8 centred; fourteen,
    // 210 dots, have one space of 14 by count and start at dot 7 centred.
    let sixteen = format!("{}{}", "W".repeat(12), "i".repeat(4));
    assert!(by_count(&sixteen).is_ok());
    assert_eq!(message(by_width(&sixteen)), too_wide(8 + 204));
    let fourteen = format!("{}{}", "あ".repeat(7), "W".repeat(7));
    assert!(by_width(&fourteen).is_ok());
    assert_eq!(message(by_count(&fourteen)), too_wide(14 + 98 + 96));

    // A name counts as the letters of the name in the record and one more: 6 for Cress.
    assert!(by_count("{クレス}{クレス}ii").is_ok());
    assert!(by_width("{クレス}{クレス}ii").is_ok());
    assert_eq!(message(by_count("{クレス}{クレス}{クレス}i")), too_many(19));
    assert!(by_count(&format!("{{クレス}}{}", "i".repeat(10))).is_ok());
    assert_eq!(message(by_count(&format!("{{クレス}}{}", "i".repeat(11)))), too_many(17));
    // The game counts the name in the record and draws the one dialogue shows, which may be longer:
    // three for the count, and fifteen wide letters on the line, after five spaces by count, or from
    // the window's edge by width, since the name is 248 dots.
    let long = place_names_of(&[("クレス", 3, "WWWWWWWWWWWWWWW")]);
    let with_long = |text: &str, centring| place_name(text, &extra, &advance, &long, centring);
    assert_eq!(message(with_long("{クレス}ii", Centring::ByCount)), too_wide(70 + 15 * 16 + 4));
    assert_eq!(message(with_long("{クレス}ii", Centring::ByWidth)), too_wide(15 * 16 + 4));
    // The same name written by the member's number is the same, and a character of the game's font
    // written as {K:..} is 14 dots like the others.
    assert_eq!(message(with_long("{NAME:01}ii", Centring::ByCount)), too_wide(70 + 15 * 16 + 4));
    assert_eq!(message(with_long("{NAME:01}ii", Centring::ByWidth)), too_wide(15 * 16 + 4));
    assert!(by_count(&"{K:0180}".repeat(15)).is_ok());
    assert_eq!(message(by_count(&"{K:0180}".repeat(16))), too_wide(210));
    assert_eq!(message(by_width(&"{K:0180}".repeat(16))), too_wide(210));

    // How the game counts, from the bytes. Mint's number is the name code itself, so the game reads
    // it as another name, of the member numbered by the byte after; for あ there is none, so nothing.
    let record = |member: u8| [(0x01, 5), (0x02, 4)].iter().find(|(known, _)| *known == member).map(|(_, n)| *n);
    let count = |text: &str| place_name_count(&encoded(text).unwrap(), &record);
    assert_eq!(count("{クレス}あ"), 7);
    assert_eq!(count("{ミント}あ"), 5);
    assert_eq!(count("{ミント}{クレス}"), 4 + 4 + 5 + 1);
    // A number is two characters to the count, whatever it shows; on the line it takes a digit's room.
    assert_eq!(count("モーリア坑道地下{NUM:801F}階"), 11);
    assert!(by_count(&format!("{}{{NUM:801F}}あ", "あ".repeat(13))).is_ok());
    assert_eq!(message(by_count(&format!("{}{{NUM:801F}}あ", "あ".repeat(14)))), too_many(17));
    // Centred by width, a number in the middle of the name is held to the most digits it can show:
    // three for a byte, which the first byte of {NUM:801F} says it is, and eight for anything wider.
    assert_eq!(
        (number_digits("801F"), number_digits("C01F"), number_digits("811F"), number_digits("8303")),
        (3, 3, 8, 8)
    );
    // The value is read without its spaces, as the build encodes it, and one that cannot be read as
    // bytes is taken at its widest.
    assert_eq!(
        (number_digits(" 801F"), number_digits("80 1F"), number_digits("8 01F"), number_digits("801f")),
        (3, 3, 3, 3)
    );
    assert_eq!((number_digits("801"), number_digits(""), number_digits("  ")), (8, 8, 8));
    // Nine wide letters, three digits and an i are 190 dots, which start at 17: the i at 203. A tenth
    // letter makes 206 dots from 9, and the i at 211, though with one digit it would start at 197.
    // Spaces in the tag change nothing.
    for number in ["{NUM:801F}", "{NUM: 801F}", "{NUM:80 1F}"] {
        assert!(by_width(&format!("{}{number}i", "W".repeat(9))).is_ok(), "{number}");
        assert_eq!(
            message(by_width(&format!("{}{number}i", "W".repeat(10)))),
            too_wide_with(&format!("{number} showing 3 digits"), 211)
        );
    }
    // A number that ends the name is drawn whole too, so it is held to its most digits as well:
    // eleven wide letters and three digits are 218 dots from 3, the last digit at 207; with a
    // twelfth letter, from the window's edge, at 220, though one digit would start at 201.
    for number in ["{NUM:801F}", "{NUM: 801F}", "{NUM:80 1F}"] {
        assert!(by_width(&format!("{}{number}", "W".repeat(11))).is_ok(), "{number}");
        assert_eq!(
            message(by_width(&format!("{}{number}", "W".repeat(12)))),
            too_wide_with(&format!("{number} showing 3 digits"), 220)
        );
    }

    // Centred by width, a party member's name is also held to the longest a player can type: six
    // letters, here 14 dots each. Seven wide letters, Cress drawn as "ii" and an i are 124 dots and
    // fit; with six typed letters in place of "ii" they are 200 dots, which put the i at 208.
    let narrow = place_names_of(&[("クレス", 5, "ii")]);
    let with_narrow = |text: &str, centring| place_name(text, &extra, &advance, &narrow, centring);
    let typed_in = format!("{}{{クレス}}i", "W".repeat(7));
    assert!(with_narrow(&typed_in, Centring::ByCount).is_ok());
    assert_eq!(
        message(with_narrow(&typed_in, Centring::ByWidth)),
        too_wide_with("a name of 6 letters typed for {クレス}", 208)
    );
    assert!(with_narrow(&format!("{}{{クレス}}i", "W".repeat(6)), Centring::ByWidth).is_ok());
    // A name that ends the line may end on the narrowest letter a player can type, which leaves the
    // last start further right than six wide letters do: with letters of 4 to 14 dots, eight wide
    // letters and five of 14 and one of 4 are 202 dots, from 11, the last at 209; six of 14 would put
    // it at 204.
    let mixed = vec![PlaceNameName { tag: "クレス", record: 5, drawn: "ii", typed: (4, 14) }];
    let ends_on_name = format!("{}{{クレス}}", "W".repeat(8));
    assert_eq!(
        message(place_name(&ends_on_name, &extra, &advance, &mixed, Centring::ByWidth)),
        too_wide_with("a name of 6 letters typed for {クレス}", 209)
    );
    assert!(place_name(&ends_on_name, &extra, &advance, &narrow, Centring::ByWidth).is_ok());

    // `extract` says so under each of the 122 strings, and under nothing else.
    let note = |section: &str, number: u32| Top.note(section, Some(number));
    assert_eq!(
        note("05", 2).unwrap(),
        "The name of a place, shown on one line and centred. Where the pack adds letters, it must fit the line: \
         about 200 dots, some 28 Latin letters, with room for the longest name a player can type in place of a \
         party member's name. With the game's own characters only, 16 characters at most, a party member's name \
         counting as its letters and one more."
    );
    let noted = PLACE_NAMES.iter().filter(|(block, number)| note(&format!("{block:02}"), *number).is_some()).count();
    assert_eq!(noted, 122);
    assert_eq!((note("05", 6), note("03", 1), note("04", 0)), (None, None, None));
    assert!(note("01", 4).unwrap().starts_with("The game shows this string as sprites"));
    assert!(KINDS[0].limits.contains("centred on one line, which holds about 200 dots, some 28 Latin letters"));
}

#[test]
fn a_place_name_is_checked_at_its_worst_without_trying_every_way() {
    // Letters of 4 and 16 dots, the game's own 14; a player can type letters of 4 to 14 dots.
    let extra = Extra::from([('i', 0x700), ('W', 0x701)]);
    let advance = |char: char| {
        Ok(match char {
            'i' => 4,
            'W' => 16,
            _ => ORIGINAL_ADVANCE,
        })
    };
    let narrow = [PlaceNameName { tag: "クレス", record: 5, drawn: "ii", typed: (4, 14) }];
    let wide = [PlaceNameName { tag: "クレス", record: 5, drawn: "WWWWWWWWW", typed: (4, 14) }];

    // The worst found directly is the worst of every way the pieces can be drawn together.
    for names in [&narrow[..], &wide[..]] {
        for text in [
            "WW{クレス}i{NUM:801F}",
            "{クレス}{クレス}",
            "W{NUM:801F}{クレス}",
            "{NUM:811F}{クレス}{NUM:801F}",
            "iiii{クレス}",
            "{クレス}i{NUM:801F}W",
        ] {
            for (centring, count) in [(Centring::ByWidth, 0), (Centring::ByCount, 6)] {
                let pieces = place_pieces(text, names, &advance).unwrap();
                let mut lines = vec![Vec::new()];
                for piece in &pieces {
                    let ways = place_ways(piece, centring, &advance).unwrap();
                    lines = lines
                        .iter()
                        .flat_map(|line| ways.iter().map(move |(way, _)| [line.as_slice(), way].concat()))
                        .collect();
                }
                let brute = lines.iter().filter_map(|line| place_last_start(line, centring, count)).max();
                let found = place_worst_start(&pieces, centring, count, &advance).unwrap().map(|(start, _)| start);
                assert_eq!(found, brute, "{text} {centring:?}");
            }
        }
    }

    // Twelve names and twelve numbers are checked at once, where trying every way they can be drawn
    // together would take 13 to the 12th times 3 to the 11th lines. Drawn as the pack has them they
    // fit, 216 dots from dot 4, the last digit at 206; but a player may type 84 dots for each name and
    // each number may show three digits, so they cannot, nor can any twelve names.
    let names = place_names_of(&[("クレス", 5, "i")]);
    let many = "{クレス}{NUM:801F}".repeat(12);
    let said = ["a name of 6 letters typed for {クレス}", "{NUM:801F} showing 3 digits"].repeat(12).join(" and ");
    assert_eq!(
        message(place_name(&many, &extra, &advance, &names, Centring::ByWidth)),
        format!(
            "the game shows this string as the name of a place, centred on one line, and with {said} its last \
             character would start at dot {}, where the line ends at dot 208: leave room for that, with fewer or \
             narrower letters",
            12 * 84 + 12 * 42 - 14
        )
    );
}

#[test]
fn a_real_place_name_is_held_to_its_box() {
    let Some(rom) = game() else { return };
    let refused = |pack: &Pack| message(built(&rom, pack, &BuildOptions::default()));
    let named = |pairs: &[(&str, u32, &str)], names: &[(&str, &str)]| {
        let mut pack = pack_of(pairs);
        pack.names = names_of(names).names;
        pack
    };

    // As the English pack had it, with seventeen characters, and with sixteen: its letters bring the
    // renderer, which centres by width, so only the line holds a name back. Names of the English pack
    // as it was before it shortened them for the game's count fit too, but not the longest of them.
    plainly_built(&rom, &pack_of(&[("05", 2, "St. Lenios Church")]));
    plainly_built(&rom, &pack_of(&[("05", 2, "St Lenios Church")]));
    plainly_built(&rom, &pack_of(&[("73", 1, "Arlee, Town of Everlasting Dark")]));
    assert_eq!(
        refused(&pack_of(&[("17", 3, "Euclid National Academy of Science")])),
        "string 17.003: the game shows this string as the name of a place, centred on one line, and its last \
         character would start at dot 215, where the line ends at dot 208: use fewer or narrower letters"
    );
    // A place name is never broken over lines, which would leave its box with the first only: breaking
    // dialogue counts a number as four digits, and would have put this one on two lines.
    let zodiac = "The Zodiac Tower, Floor {NUM:801F}F";
    let (_, image) = plainly_built(&rom, &pack_of(&[("03", 0, zodiac)]));
    let blocks = read_blocks(&image).unwrap();
    let string = split_strings(&blocks[3]).unwrap()[0];
    assert!(!string.contains(&0x00) && string.ends_with(&END), "{string:02x?}");
    // In the game's own characters, without the renderer, the game's count centres the name, and holds
    // it to sixteen (sixteen of them would already be too wide for the line).
    plainly_built(&rom, &pack_of(&[("05", 2, &"あ".repeat(15))]));
    assert_eq!(
        refused(&pack_of(&[("05", 2, &"あ".repeat(17))])),
        "string 05.002: the game shows this string as the name of a place, on one line of at most 16 characters, \
         and it has 17: a party member's name counts as the letters the menus show for it, and one more"
    );
    // A name counts as the name in the character's record, which the menus show, and one more: six
    // for チェスター, and eight for a new name of seven that the menus can take. Centred by width,
    // neither count matters.
    plainly_built(&rom, &pack_of(&[("05", 1, "{チェスター}'s Houses")]));
    let longer = [("チェスター", "チェスタークン")];
    plainly_built(&rom, &named(&[("05", 1, "{チェスター}'s House")], &longer));
    plainly_built(&rom, &named(&[("05", 1, "{チェスター}'s Houses")], &longer));

    // A name longer than the menus hold leaves them the game's names, so the game counts クレス, three,
    // and dialogue draws fifteen letters of 14 dots. The game's own string with it is left alone, as
    // is every string a pack gives as the game has it; changed, it is held to the line. A renamed
    // character brings the renderer, which centres by width: 252 dots start at the window's edge.
    let wide = [("クレス", "ＷＷＷＷＷＷＷＷＷＷＷＷＷＷＷ")];
    assert_eq!(extract(&rom).unwrap().texts["05"][&0], "{クレス}の家");
    plainly_built(&rom, &named(&[("05", 0, "{クレス}の家")], &wide));
    assert_eq!(
        refused(&named(&[("05", 0, "{クレス}のいえ")], &wide)),
        "string 05.000: the game shows this string as the name of a place, centred on one line, and its last \
         character would start at dot 238, where the line ends at dot 208: use fewer or narrower letters"
    );
}
