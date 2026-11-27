use std::fs;

use super::*;
use crate::adapters::god_ps1::{AREAS, entries, extract, glyph};
use crate::platform::Image;
use crate::script::Script;
use crate::text::from_hex;

fn hex(bytes: &str) -> Vec<u8> {
    from_hex(bytes).unwrap()
}

/// A table of bytes like the game's: $20 the blank between words, $A1 。, $A2 あ, $A3 Ａ, $A4 ａ, and
/// the rest drawing nothing.
fn codes() -> ByteCodes {
    let mut codes = [0u16; 256];
    codes[0x20] = 0x04D7;
    codes[0xA1] = 0x0101;
    codes[0xA2] = 0x0136;
    codes[0xA3] = 0x011A;
    codes[0xA4] = 0x0780;
    ByteCodes::new(codes)
}

/// A string of the pack as it is written a character to a cell: wrapped or not, then fitted.
fn written(text: &str, wrapping: bool) -> Result<Vec<u8>> {
    let codes = codes();
    let text = if wrapping { wrap(text)? } else { text.to_string() };
    let data = encode_with(&text, &codes, &|char| pack_code(char, &codes, &HashMap::new()))?;
    fit(&data, &codes, wrapping)
}

/// A changed string's text broken at spaces to the window, a character to a cell.
fn wrap(text: &str) -> Result<String> {
    wrap_to(text, COLUMNS)
}

/// The same, to lines of `columns` cells.
fn wrap_to(text: &str, columns: usize) -> Result<String> {
    let (codes, none) = (codes(), HashMap::new());
    super::wrap_to(text, columns, &Writing { codes: &codes, drawn: &none, pairs: None })
}

/// What the game would show of a string written for it, as lines: `|` where it waits, `>` where it
/// waits along a line. A `b` stands for any character of two bytes.
fn shown(data: &[u8]) -> String {
    let mut out = String::new();
    let mut at = 0;
    while at < data.len() {
        let (text, length) = match data[at] {
            0x01..=0x0A => ("b", 2),
            0x0B => ("|", 1),
            0x0C => ("N", 2),
            0x0D => ("/", 1),
            0x0E => ("「", 1),
            0x0F if data.get(at + 1) == Some(&1) => (">", 2),
            0x0F => ("~", 2),
            0x20 => (" ", 1),
            0xA3 => ("A", 1),
            0xA4 => ("a", 1),
            _ => ("?", 1),
        };
        out.push_str(text);
        at += length;
    }
    out
}

#[test]
fn ascii_is_written_with_the_font_s_full_width_forms() {
    let codes = codes();
    let none = HashMap::new();
    assert_eq!(stand_in(' '), Some('　'));
    assert_eq!(stand_in('A'), Some('Ａ'));
    assert_eq!(stand_in('!'), Some('！'));
    assert_eq!(stand_in('~'), Some('〜'));
    assert_eq!(stand_in('é'), None);
    // One byte where the game has one, two where not; what the font has not got has no code.
    assert_eq!(pack_code('A', &codes, &none), Some(0x011A));
    assert_eq!(pack_code('?', &codes, &none), Some(0x0103));
    assert_eq!(pack_code('.', &codes, &none), Some(0x04C9));
    assert_eq!(pack_code(',', &codes, &none), None);
    assert_eq!(pack_code('é', &codes, &HashMap::from([('é', 0x0134)])), Some(0x0134));
    let data = encode_with("Aa ?", &codes, &|char| pack_code(char, &codes, &none)).unwrap();
    assert_eq!(data, hex("a3 a4 20 01 03"));
}

#[test]
fn a_line_that_fills_the_window_needs_no_line_break() {
    // Sixteen characters fill a line, and the game starts the next itself.
    let full = "A".repeat(16);
    assert_eq!(shown(&written(&format!("{full}\nAA"), false).unwrap()), format!("{full}AA"));
    // In a spoken line 「 takes the first cell, and the lines after it start one cell in.
    let spoken = "A".repeat(15);
    assert_eq!(shown(&written(&format!("{{SAY}}{spoken}\nA{{/SAY}}"), false).unwrap()), format!("「{spoken}A「"));
    // A wait after a full line waits along the new one.
    assert_eq!(shown(&written(&format!("{full}{{WAIT}}\nAA"), false).unwrap()), format!("{full}>AA"));
    // A shorter line keeps its break, and so does a second break after a full line.
    assert_eq!(shown(&written("AA\nAA", false).unwrap()), "AA/AA");
    assert_eq!(shown(&written(&format!("{full}\n\nA"), false).unwrap()), format!("{full}/A"));
}

#[test]
fn without_wrapping_a_line_longer_than_the_window_is_refused() {
    let long = written(&"A".repeat(17), false).unwrap_err().to_string();
    assert!(long.starts_with("its line 1 is longer than the window"), "{long}");
    let spoken = written(&format!("{{SAY}}AA\n{}{{/SAY}}", "A".repeat(16)), false).unwrap_err().to_string();
    assert!(spoken.starts_with("its line 2 is longer"), "{spoken}");
    // Without wrapping, nothing is moved: five lines scroll as written.
    assert_eq!(shown(&written("A\nA\nA\nA\nA", false).unwrap()), "A/A/A/A/A");
}

#[test]
fn wrapping_breaks_lines_at_spaces_and_waits_before_the_window_scrolls() {
    // Lines of 16 in narration, 15 in a spoken line, a name counted as 6.
    assert_eq!(wrap("aaaa bbbb cccc dddd").unwrap(), "aaaa bbbb cccc\ndddd");
    assert_eq!(wrap("{SAY}aaaa bbbb ccccc d{/SAY}").unwrap(), "{SAY}aaaa bbbb ccccc\nd{/SAY}");
    assert_eq!(wrap("{SAY}aaaa {NAME:19} aaa{/SAY}").unwrap(), "{SAY}aaaa {NAME:19} aaa{/SAY}");
    assert_eq!(wrap("{SAY}aaaaa {NAME:19} aaa{/SAY}").unwrap(), "{SAY}aaaaa {NAME:19}\naaa{/SAY}");
    // A name, a number or a word that would run past the line's end goes to the next line whole.
    assert_eq!(wrap("{SAY}abcdefghijklm {NAME:19}{/SAY}").unwrap(), "{SAY}abcdefghijklm\n{NAME:19}{/SAY}");
    assert_eq!(wrap("you pay {NUM} G here").unwrap(), "you pay {NUM} G\nhere");
    assert_eq!(wrap("you must now pay {NUM} G").unwrap(), "you must now pay\n{NUM} G");
    assert_eq!(wrap("aaaa\nbbbb cccc{WAIT}\ndddd eeee").unwrap(), "aaaa\nbbbb cccc{WAIT}\ndddd eeee");
    // A fifth line would scroll the first away unread: a wait goes in place of the break before it.
    assert_eq!(shown(&written("A A A A A A", true).unwrap()), "A A A A A A");
    let five = ["AAAAAAAAAA"; 5].join(" ");
    assert_eq!(shown(&written(&five, true).unwrap()), "AAAAAAAAAA/AAAAAAAAAA/AAAAAAAAAA/AAAAAAAAAA|AAAAAAAAAA");
    // A fourth line that is full would start a fifth: the page ends a line sooner.
    let full = "A".repeat(16);
    let text = format!("A\nA\nA\n{full}");
    assert_eq!(shown(&written(&text, true).unwrap()), format!("A/A/A|{full}"));
    // A page with no line break in it waits before the character that would fill its last line.
    let unbroken = "A".repeat(70);
    let expected = format!("{}|{}", "A".repeat(63), "A".repeat(7));
    assert_eq!(shown(&written(&unbroken, true).unwrap()), expected);
    // A wait of the pack's own starts a page.
    assert_eq!(shown(&written("A\nA\nA{WAIT}\nA\nA\nA", true).unwrap()), "A/A/A|A/A/A");
}

#[test]
fn a_string_with_the_character_that_stands_for_a_tag_s_cells_is_refused() {
    // Without the check, the line breaking took it for a tag's cells and found no tag behind it.
    let refused = wrap("A\u{E000}A").unwrap_err().to_string();
    assert!(refused.contains("U+E000"), "{refused}");
    assert!(wrap("A {NAME:19}\u{E000}").is_err());
}

/// A changed string of the program's text at `offset` as it is written, the game's own being Ａ.
fn program_written(text: &str, offset: u32, wrap: bool) -> std::result::Result<Vec<u8>, String> {
    let (codes, none) = (codes(), HashMap::new());
    let writing = Writing { codes: &codes, drawn: &none, pairs: None };
    program_string(text, offset, &hex("a3"), &writing, wrap).map_err(|error| error.to_string())
}

#[test]
fn a_name_of_the_party_has_characters_alone() {
    let names = program_text::NAMES;
    assert_eq!(program_written("AA", names, true).unwrap(), hex("a3 a3"));
    // A kanji, or a mark the menus' font has not got, is refused, though the dialogue's font has it.
    for (text, char) in [("A日", "日"), ("A~", "~"), ("Ab", "b")] {
        let refused = program_written(text, names, true).unwrap_err();
        assert!(refused.starts_with(&format!("it has '{char}', and a name of the party")), "{refused}");
    }
    assert_eq!(program_written("ＡＢ", names, true).unwrap(), hex("a3 01 1b"));
    let refused = program_written("A{HOLD}A", names, true).unwrap_err();
    assert!(refused.starts_with("it has the tag {HOLD}, and a name of the party"), "{refused}");
    assert!(refused.ends_with("write the name in characters alone"), "{refused}");
}

#[test]
fn a_string_with_too_many_lines_is_told_what_to_do() {
    // A shopkeeper's line has 2 lines of 13 characters.
    let shop = program_text::SHOP_LINES[0].0;
    assert!(program_written("AAAA AAAA AAAA AAAA", shop, true).is_ok());
    let refused = program_written("AAAA AAAA AAAA AAAA AAAA", shop, true).unwrap_err();
    assert_eq!(
        refused,
        "it has 3 lines, and a string of this part has room for 2: shorten it until, broken at spaces, it has no \
         more than 2"
    );
    let refused = program_written("AAAA\nAAAA\nAAAA", shop, false).unwrap_err();
    assert!(refused.ends_with(": join its lines, or shorten it, until it has no more than 2"), "{refused}");
}

/// A text block like the game's: entry 0 a string, entry 1 the same, entry 2 a list of numbers,
/// entry 3 a string with a byte left over after its zero byte, entry 4 a string, and four zero bytes.
fn block() -> Vec<u8> {
    let mut text = Vec::new();
    for offset in [20u32, 20, 24, 30, 34] {
        text.extend(offset.to_le_bytes());
    }
    text.extend(hex("a2 a1 00 00")); // 20: あ。
    text.extend(hex("02 00 07 00 08 00")); // 24: two numbers
    text.extend(hex("a2 a2 00 77")); // 30: ああ, and a byte left over
    text.extend(hex("a1 00 00 00 00 00")); // 34: 。, then the block's end
    text
}

fn area(text: Vec<u8>) -> Area {
    let entries = entries(&text, &codes()).unwrap();
    Area { entry: 96, text, entries }
}

#[test]
fn a_text_block_is_written_again_in_the_order_of_its_table() {
    let old = area(block());
    assert_eq!(old.entries[1], Entry::Same(0));
    assert_eq!(old.entries[2], Entry::Numbers);
    // Nothing changed: the same bytes.
    assert_eq!(text_block_with(&old, &BTreeMap::new()), old.text);
    // Entry 0 two characters longer: the list of numbers keeps its place in 4 bytes, entry 1 follows
    // entry 0, and the rest is copied.
    let new = text_block_with(&old, &BTreeMap::from([(0, hex("a2 a2 a2 a1"))]));
    let read = area(new.clone());
    assert_eq!(read.entries[1], Entry::Same(0));
    assert_eq!(read.entries[2], Entry::Numbers);
    assert_eq!(read.strings()[&0], hex("a2 a2 a2 a1"));
    assert_eq!(read.strings()[&3], hex("a2 a2"));
    assert_eq!(word(&new, 8), Some(28));
    assert_eq!(&new[28..34], &old.text[24..30]);
    // The last string changed: four zero bytes still end the block.
    let new = text_block_with(&old, &BTreeMap::from([(4, hex("a2 a2 a2"))]));
    assert_eq!(&new[34..], hex("a2 a2 a2 00 00 00 00 00"));
    assert_eq!(area(new).strings()[&4], hex("a2 a2 a2"));
}

/// An area's member like the game's: a header, the code (events, then the text block, then three
/// parts of 8, 4 and 4 bytes), the rest of the data at the next multiple of $800, and one entry at
/// $29C that gives two places in the rest.
fn member_with(text: &[u8]) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend(1u32.to_le_bytes());
    code.extend(hex("05 00 00 00"));
    code.extend(1u32.to_le_bytes());
    code.extend(hex("06 00 00 00"));
    code.extend((text.len() as u32).to_le_bytes());
    code.extend(text);
    code.resize(code.len().next_multiple_of(4), 0);
    let after = code.len();
    code.extend(b"PART2...PT3.PT4.");
    let mut member = vec![0; AREA_HEADER];
    member.extend(&code);
    let rest = member.len().next_multiple_of(REST_ALIGN);
    member.resize(rest, 0);
    member.extend(b"THE REST OF THE AREA'S DATA");
    let words = [code.len(), after, 8, 1, 4, 0, 4, 0, 0, rest, 27];
    for (number, value) in words.iter().enumerate() {
        put_word(&mut member, number * 4, *value);
    }
    let entry = [0x0014_0020, rest + 4, 0x0102_2404, rest + 8];
    for (index, value) in entry.iter().enumerate() {
        put_word(&mut member, HEADER_ENTRIES + 16 + index * 4, *value);
    }
    member
}

#[test]
fn a_member_is_rebuilt_around_its_new_text_block() {
    let old = member_with(&block());
    assert!(layout_holds(&old));
    assert_eq!(rebuild_member(&old, &block()).unwrap(), old);
    // A block 3000 bytes larger moves the parts after it, the rest to the next multiple of $800
    // past them, and the entry's places with it.
    let mut large = block();
    large.extend(vec![0; 3000]);
    let new = rebuild_member(&old, &large).unwrap();
    assert_eq!(new, member_with(&large));
    assert_eq!((member_word(&old, 9), member_word(&new, 9)), (0x1000, 0x1800));
    assert_eq!(word(&new, HEADER_ENTRIES + 16 + 4), Some(member_word(&new, 9) + 4));
    // And back again.
    assert_eq!(rebuild_member(&new, &block()).unwrap(), old);
    // A header that does not add up is refused.
    let mut wrong = old.clone();
    put_word(&mut wrong, 8, 9);
    assert!(rebuild_member(&wrong, &block()).is_err());
}

// --- with the game ---------------------------------------------------------

/// The game's disc, opened, or None where it is not there to test with.
fn game() -> Option<Image> {
    Some(Image::Ps1Disc(Disc::open(&crate::testing::game("god-ps1")?).unwrap()))
}

#[test]
fn every_area_rebuilds_to_its_own_bytes() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let areas = areas(disc).unwrap();
    for area in areas.iter() {
        let old = member(disc, area.entry).unwrap();
        assert!(layout_holds(&old), "area {}", area.section());
        let text = text_block_with(area, &BTreeMap::new());
        assert!(text == area.text, "area {}", area.section());
        assert!(rebuild_member(&old, &text).unwrap() == old, "area {}", area.section());
    }
    assert!(areas.iter().all(|area| AREAS.contains(&area.entry)));
}

#[test]
fn the_cells_new_characters_go_in_are_blank() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    for code in BLANK_CODES {
        let glyph = super::super::glyph(disc, (code - FIRST_CODE) as usize).unwrap();
        assert!(glyph.iter().flatten().all(|pixel| *pixel == 0), "{code:04X}");
    }
}

/// The options of a build that writes every character a cell, as the game's own text does.
fn full_width() -> BuildOptions {
    BuildOptions { switches: [FULL_WIDTH.to_string()].into(), ..BuildOptions::default() }
}

/// A pack of strings of area 096.
fn pack_of(strings: &[(u32, &str)]) -> Pack {
    let strings = strings.iter().map(|(number, text)| (*number, text.to_string())).collect();
    Pack::of(Script::from([("096".to_string(), strings)]))
}

#[test]
fn build_puts_a_longer_line_in_and_moves_its_area_when_it_grows() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    let original = extract(disc).unwrap();
    let pack = pack_of(&[(109, "{SAY}Wake up, {NAME:19}! It's morning, élan.{/SAY}")]);
    let built = build(disc, &pack, &out, &full_width()).unwrap();
    assert_eq!(
        built.summary,
        [
            "dialogue:  1 strings changed in 1 areas",
            "room:      388168 bytes of the heap free at the tightest moment, area 096, in a battle (the game leaves \
             19636 at its own)",
            "font:      3 new characters drawn into its blank cells, of 26: ,'é",
        ]
    );
    {
        let copy = Disc::open(&out).unwrap();
        let text = extract(&copy).unwrap();
        // Broken at spaces to 15 characters, the name counted as 6; the comma, the apostrophe and é
        // in the first three blank cells, which the table of characters has nothing for.
        let expected = "{SAY}Ｗａｋｅ　ｕｐ{K:0134}
{NAME:19}！　Ｉｔ{K:04D3}ｓ
ｍｏｒｎｉｎｇ{K:0134}　{K:04D4}ｌａｎ．{/SAY}";
        assert_eq!(text["096"][&109], expected);
        // The comma is drawn in its cell's font; the three other fonts in the same pixels are as they were.
        let comma = glyph(&copy, 0x34).unwrap();
        assert!(comma.iter().flatten().any(|pixel| *pixel == 1) && comma[15].iter().all(|pixel| *pixel == 0));
        for index in [0x134, 0x234, 0x334] {
            assert_eq!(glyph(&copy, index).unwrap(), glyph(disc, index).unwrap());
        }
        assert_eq!(text["096"][&110], original["096"][&110]);
        assert_eq!(program(&copy).unwrap().contents, program(disc).unwrap().contents);
    }
    // A line long enough that the area takes more sectors: it moves to the disc's free room, and
    // every entry that names it follows.
    let long = format!("{{SAY}}{}{{/SAY}}", "élan ".repeat(1200));
    let built = build(disc, &pack_of(&[(109, &long)]), &out, &full_width()).unwrap();
    assert_eq!(
        built.summary[1],
        format!("moved:     area 096 grew past its sectors and is now in the disc's free room ({SPARE_FILE})")
    );
    {
        let copy = Disc::open(&out).unwrap();
        let contents = &program(&copy).unwrap().contents;
        let spare = copy.first_sector(SPARE_FILE).unwrap() - copy.first_sector(ARCHIVE).unwrap();
        assert_eq!(contents[96].0, spare);
        assert!(contents[189..=195].iter().all(|entry| *entry == contents[96]));
        assert_eq!(&contents[..96], &program(disc).unwrap().contents[..96]);
        let text = extract(&copy).unwrap();
        assert_eq!(text["096"][&110], original["096"][&110]);
        assert!(text["096"][&109].len() > 6000);
    }
    // Too large for the heap, and the bosses' speeches in battle: refused, and nothing written.
    fs::remove_file(&out).unwrap();
    let huge = format!("{{SAY}}{}{{/SAY}}", "日".repeat(200_000));
    let refused = build(disc, &pack_of(&[(109, &huge)]), &out, &BuildOptions::default()).err().unwrap().to_string();
    assert!(refused.starts_with("area 096: going into it from area 097, the game would need "), "{refused}");
    assert!(refused.ends_with(" bytes more of its heap than it has room for: shorten some of its changed lines"));
    let speech = Pack::of(Script::from([("battle".to_string(), BTreeMap::from([(0x9_812A, "x".to_string())]))]));
    let refused = build(disc, &speech, &out, &BuildOptions::default()).err().unwrap().to_string();
    assert!(refused.starts_with("the pack changes what the bosses say in battle (battle.622890)"), "{refused}");
    assert!(!out.exists());
}

/// A pack of strings of the program's text and of the menus' labels.
fn program_pack(strings: &[(u32, &str)], labels: &[(u32, &str)]) -> Pack {
    let section = |strings: &[(u32, &str)]| strings.iter().map(|(number, text)| (*number, text.to_string())).collect();
    Pack::of(Script::from([("program".to_string(), section(strings)), ("menu".to_string(), section(labels))]))
}

/// The string of the program's text a 32-bit word of the program points at, as script text.
fn pointed_at(disc: &Disc, word: usize) -> String {
    let file = program_text::program_file(disc).unwrap();
    let at = (crate::platforms::mips::number(&file, word).unwrap() - program_text::PROGRAM_BASE) as usize;
    let length = file[at..].iter().position(|byte| *byte == 0).unwrap();
    decode(&file[at..at + length], &program(disc).unwrap().codes)
}

#[test]
fn build_puts_the_program_s_text_and_the_menus_labels_in() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // Three names of the party, the first longer than its room and the others shorter; the main
    // menu's first label; a word of the status screen, which a table points at, longer than its room,
    // and two more given the same text.
    let pack = program_pack(
        &[
            (701264, "GENNAI"),
            (701287, "DAN"),
            (701278, "B"),
            (701611, "Power"),
            (701640, "Attack"),
            (701716, "Attack"),
        ],
        &[(687388, "Items")],
    );
    let built = build(disc, &pack, &out, &full_width()).unwrap();
    assert_eq!(
        built.summary[1..],
        [
            "program:   6 strings changed, 5 of them moved in the room they share, 1 of them the same text as another \
             and written once",
            "menu:      1 labels changed, each in its slot",
        ]
    );
    {
        let copy = Disc::open(&out).unwrap();
        // The table of the status screen's words, in the program's code, points at the new ones.
        assert_eq!(pointed_at(&copy, 0x1A40), "Ｐｏｗｅｒ");
        assert_eq!(pointed_at(&copy, 0x1A40 + 4), pointed_at(disc, 0x1A44));
        let text = extract(&copy).unwrap();
        let strings: Vec<&String> = text["program"].values().collect();
        assert!(strings.contains(&&"ＧＥＮＮＡＩ".to_string()) && strings.contains(&&"ＤＡＮ".to_string()));
        assert_eq!(strings.iter().filter(|text| ***text == "Ａｔｔａｃｋ").count(), 1);
        // A lowercase letter is written as the capital the menus' font draws, and the rest of the slot
        // is zero bytes.
        let file = program_text::program_file(&copy).unwrap();
        assert_eq!(file[687388..687398], *b"ITEMS\0\0\0\0\0");
        assert_eq!(file[687398..687460], program_text::program_file(disc).unwrap()[687398..687460]);
    }
    // What does not fit the screen or the room is refused, every string at once, and nothing written.
    fs::remove_file(&out).unwrap();
    let pack = program_pack(
        &[
            (701264, "Gennai"),                               // a name in lowercase letters
            (701312, "New game\nContinue"),                   // the title screen's box is 5 characters wide
            (657748, "Welcome to my humble shop, traveller"), // three lines of 13 where a shopkeeper has two
            (713338, "Baseball cap"),                         // an item's name has 6 characters
        ],
        &[(687398, "Equipment"), (688708, "MESSAGE SPEED")],
    );
    let refused = build(disc, &pack, &out, &full_width()).err().unwrap().to_string();
    let lines: Vec<&str> = refused.lines().collect();
    assert_eq!(lines.len(), 6, "{refused}");
    assert!(lines[0].starts_with("string program.657748: it has 3 lines, and a string of this part has room for 2"));
    assert!(lines[1].starts_with("string program.701264: it has 'e', and a name of the party"), "{refused}");
    assert!(lines[1].ends_with("write it in capitals, digits and kana"));
    assert_eq!(
        lines[2],
        "string program.701312: broken at spaces, it still has a line of 8 cells, and its box, as wide as the \
         game's own text, has room for 5: shorten its longest word"
    );
    assert!(lines[3].starts_with("string program.713338: broken at spaces, it still has a line of 8 cells"));
    assert_eq!(
        lines[4],
        "string menu.687398: it draws 9 characters, and the labels of its screen have room for 8, the most the \
         game's own take there"
    );
    assert!(
        lines[5]
            .starts_with("string menu.688708: it draws 13 characters, and the labels of its screen have room for 11")
    );
    assert!(!out.exists());
}

#[test]
fn an_area_the_game_leaves_no_room_for_cannot_grow() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    let pack = |text: &str| Pack::of(Script::from([("105".to_string(), BTreeMap::from([(4, text.to_string())]))]));
    // Going from area 107 into area 105, the game leaves only `SPARE` bytes of its heap free: area 105
    // cannot take a word more, and the three copies of its code need three times that.
    let refused = build(
        disc,
        &pack("{SAY}おやおや、{NAME:17}さんは\nマヒしてますね。\n{NUM}Ｇかかりますが\nなおしますか？いいえ{/SAY}"),
        &out,
        &BuildOptions::default(),
    );
    assert_eq!(
        refused.err().unwrap().to_string(),
        "area 105: going into it from area 107, the game would need 12 bytes more of its heap than it has room for: \
         shorten some of its changed lines"
    );
    assert!(!out.exists());
    // A line as long as the game's own goes in.
    let same = "{SAY}おやおや、{NAME:17}さんは\nマヒしてますね。\n{NUM}Ｇかかりますが\nなおすのですか{/SAY}";
    let built = build(disc, &pack(same), &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[1],
        "room:      19636 bytes of the heap free at the tightest moment, area 105, going into it from area 107 (the \
         game leaves 19636 at its own)"
    );
}

#[test]
fn an_insert_written_by_its_byte_is_measured_as_under_its_name() {
    // In a shop's line of 13 cells, a word the game fills in takes 8, however it is written.
    assert_eq!(wrap_to("AAAA AAAA {WORD}", 13).unwrap(), "AAAA AAAA\n{WORD}");
    assert_eq!(wrap_to("AAAA AAAA {0C:12}", 13).unwrap(), "AAAA AAAA\n{0C:12}");
    assert_eq!(wrap_to("AAAA AAAA {0C:0D}", 13).unwrap(), "AAAA AAAA\n{0C:0D}");
    assert_eq!(wrap_to("AAAA AAAA {0C:01}", 13).unwrap(), "AAAA AAAA\n{0C:01}");
}

// --- two characters to a cell ----------------------------------------------

/// Pairs of every ASCII character but the braces, counted as if each had a cell of its own.
fn counting() -> Pairs {
    Pairs { narrow: (' '..='~').filter(|char| !matches!(char, '{' | '}')).collect(), ..Pairs::default() }
}

/// A changed string's text broken at spaces to lines of `columns` cells, two characters to a cell.
fn wrap_paired(text: &str, columns: usize) -> Result<String> {
    let (codes, none, pairs) = (codes(), HashMap::new(), counting());
    super::wrap_to(text, columns, &Writing { codes: &codes, drawn: &none, pairs: Some(&pairs) })
}

#[test]
fn narrow_characters_are_paired_from_the_start_of_each_run() {
    use Unit::{Pair, Text};
    let (codes, none, pairs) = (codes(), HashMap::new(), counting());
    let writing = Writing { codes: &codes, drawn: &none, pairs: Some(&pairs) };
    assert_eq!(units("Hello", &writing), [Pair('H', Some('e')), Pair('l', Some('l')), Pair('o', None)]);
    // A tag, a line break or a character of the game's own font ends a run.
    assert_eq!(
        units("ab{NAME:19}c\nde。f", &writing),
        [
            Pair('a', Some('b')),
            Text("{NAME:19}"),
            Pair('c', None),
            Text("\n"),
            Pair('d', Some('e')),
            Text("。"),
            Pair('f', None)
        ]
    );
    // A pause does not: it is written after the cell of the character before it.
    assert_eq!(
        units("Sum{PAUSE:14}mer{0F:03}", &writing),
        [Pair('S', Some('u')), Pair('m', Some('m')), Text("{PAUSE:14}"), Pair('e', Some('r')), Text("{0F:03}")]
    );
    assert_eq!(units("「a{HOLD}b", &writing), [Text("「"), Pair('a', None), Text("{HOLD}"), Pair('b', None)]);
}

#[test]
fn a_pair_is_written_with_its_cell_s_code_or_else_a_character_to_a_cell() {
    let (codes, none) = (codes(), HashMap::new());
    let cells = HashMap::from([(('A', 'a'), 0x0384), (('a', ' '), 0x0385)]);
    let pairs = Pairs { cells: Some(cells), ..counting() };
    let writing = Writing { codes: &codes, drawn: &none, pairs: Some(&pairs) };
    let written = |text: &str| encode_text(text, &writing).unwrap();
    // Two bytes for a pair, as for a kanji; one alone takes the cell of it and a space.
    assert_eq!(written("Aa"), hex("03 84"));
    assert_eq!(written("Aaa"), hex("03 84 03 85"));
    // Two spaces are the game's own blank.
    assert_eq!(written("Aa  Aa"), hex("03 84 20 03 84"));
    // A pair with no cell is written a character to a cell, with the font's full-width forms.
    assert_eq!(written("aA"), hex("a4 a3"));
    assert_eq!(written("AaA"), hex("03 84 a3"));
    assert_eq!(pairs.counts.borrow()[&('A', 'a')], 5);
    // While the pairs are counted, every one is taken to have a cell.
    let counting = counting();
    let writing = Writing { codes: &codes, drawn: &none, pairs: Some(&counting) };
    assert_eq!(encode_text("aA", &writing).unwrap(), hex("08 ff"));
}

#[test]
fn a_pause_inside_a_pair_with_no_cell_stays_between_its_characters() {
    use Unit::{Apart, Pair, Text};
    let (codes, none) = (codes(), HashMap::new());
    let cells = HashMap::from([(('A', 'a'), 0x0384)]);
    let pairs = Pairs { cells: Some(cells), ..counting() };
    let writing = Writing { codes: &codes, drawn: &none, pairs: Some(&pairs) };
    let pause = encode_with("{PAUSE:14}", &codes, &|_| None).unwrap();
    // A pair with a cell is drawn whole, and the pause comes after it.
    assert_eq!(units("A{PAUSE:14}a", &writing), [Pair('A', Some('a')), Text("{PAUSE:14}")]);
    assert_eq!(encode_text("A{PAUSE:14}a", &writing).unwrap(), [hex("03 84"), pause.clone()].concat());
    // A pair with none is written a character to a cell, the pause between them as the pack has it.
    assert_eq!(units("a{PAUSE:14}A{0F:03}", &writing), [Apart('a', vec!["{PAUSE:14}"], 'A'), Text("{0F:03}")]);
    assert_eq!(encode_text("a{PAUSE:14}A", &writing).unwrap(), [hex("a4"), pause.clone(), hex("a3")].concat());
    // A pause after the second character of such a pair is after both.
    assert_eq!(encode_text("aA{PAUSE:14}", &writing).unwrap(), [hex("a4 a3"), pause].concat());
    assert_eq!(pairs.counts.borrow()[&('a', 'A')], 2);
}

#[test]
fn with_pairs_a_line_holds_twice_as_many_narrow_characters() {
    // 16 cells hold 32 characters, 15 in a spoken line 30.
    let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh";
    assert_eq!(wrap_paired(text, COLUMNS).unwrap(), "aaaa bbbb cccc dddd eeee ffff\ngggg hhhh");
    let spoken = "{SAY}aaaa bbbb cccc dddd eeee fff gggg{/SAY}";
    assert_eq!(wrap_paired(spoken, COLUMNS).unwrap(), "{SAY}aaaa bbbb cccc dddd eeee fff\ngggg{/SAY}");
    // A name the game fills in keeps its 6 cells, and a run of narrow characters with an odd count
    // before it leaves half a cell empty: counted in half cells this line fits, drawn it would not.
    let named = "aaa{NAME:19}aaa{NAME:19} b";
    assert_eq!(wrap_paired(named, COLUMNS).unwrap(), "aaa{NAME:19}aaa{NAME:19}\nb");
    // Without pairs the same text takes a cell a character.
    assert_eq!(wrap(text).unwrap(), "aaaa bbbb cccc\ndddd eeee ffff\ngggg hhhh");
}

#[test]
fn a_name_of_the_party_is_never_paired() {
    let (codes, none, pairs) = (codes(), HashMap::new(), counting());
    let writing = Writing { codes: &codes, drawn: &none, pairs: Some(&pairs) };
    let names = program_text::NAMES;
    assert_eq!(program_string("AA", names, &hex("a3"), &writing, true).unwrap(), hex("a3 a3"));
    // A word of the status screen, whose part has lines of 7 cells, is: 8 letters take 4 cells.
    let status = 0xA_B4AB;
    assert_eq!(program_string("Strength", status, &hex("a3"), &writing, true).unwrap(), hex("08 ff").repeat(4));
    let full = Writing { pairs: None, ..writing };
    assert!(program_string("Strength", status, &hex("a3"), &full, true).is_err());
}

#[test]
fn a_code_has_a_cell_in_each_of_the_two_fonts() {
    let large = large_cell(0x5A7);
    assert_eq!((large.page, large.plane, large.x, large.y, large.width), (1, 0, 7 * 16, 10 * 16, 16));
    // In the small font, 483 glyphs to a bit, 21 cells of 12 by 11 pixels to a row.
    let small = small_cell(0x5A7).unwrap();
    assert_eq!(
        (small.page, small.plane, small.x, small.y, small.width, small.height),
        (2, 2, 15 * 12, 10 * 11, 12, 11)
    );
    let last = small_cell(SMALL_END - 1).unwrap();
    assert_eq!((last.plane, last.x, last.y), (3, 20 * 12, 22 * 11));
    assert_eq!(small_cell(SMALL_END), None);
}

#[test]
fn a_glyph_is_brought_down_to_eleven_rows_keeping_where_letters_stand() {
    let glyphs = fonts::load_unifont(&Font::named("unifont"), &['A', 'a', 'g', 't', 'E'], false).unwrap();
    let ink = |char: char| {
        let rows = eleven_rows(&fonts::narrowed(&glyphs[&char], SMALL_WIDTH / 2));
        let inked: Vec<usize> = (0..SMALL_HEIGHT).filter(|y| rows[*y] != 0).collect();
        (inked[0], *inked.last().unwrap(), rows)
    };
    // Capitals from the top row, small letters from the third, on the ninth row; descenders below.
    assert_eq!((ink('A').0, ink('A').1), (0, 8));
    assert_eq!((ink('a').0, ink('a').1), (2, 8));
    assert_eq!(ink('g').1, 10);
    // The bars of E stay three.
    let bars = ink('E').2.iter().filter(|row| row.count_ones() > 2).count();
    assert_eq!(bars, 3);
}

/// The rows of pixels of a cell of the font, as a disc has it.
fn cell_pixels(disc: &Disc, cell: Cell) -> Vec<u16> {
    let picture = tim::read(&member(disc, FONT_MEMBERS[cell.page]).unwrap()).unwrap();
    let bit = |x: usize, y: usize| u16::from(picture.pixel(cell.x + x, cell.y + y).unwrap() >> cell.plane & 1);
    (0..cell.height).map(|y| (0..cell.width).fold(0, |row, x| row | bit(x, y) << (CELL - 1 - x))).collect()
}

/// The codes of two bytes a string's bytes hold.
fn two_byte_codes(data: &[u8]) -> Vec<u32> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < data.len() {
        match data[at] {
            high @ 0x01..=0x0A => {
                found.push(u32::from(high) << 8 | u32::from(data[at + 1]));
                at += 2;
            }
            0x0C | 0x0F => at += 2,
            _ => at += 1,
        }
    }
    found
}

#[test]
fn the_cells_pairs_take_are_kanji_no_string_left_as_it_is_uses() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let codes = &program(disc).unwrap().codes;
    let (areas, own_text, battle) =
        (areas(disc).unwrap(), program_text::program_text(disc).unwrap(), battle_text(disc).unwrap());
    let file = program_text::program_file(disc).unwrap();
    let kept = |dialogue: BTreeSet<(&'static str, u32)>| Kept {
        areas: &areas,
        program: &own_text,
        battle: &battle,
        dialogue,
        program_strings: BTreeSet::new(),
    };
    // With nothing changed, the kanji no text of the game uses: none of its other characters.
    let unused = free_cells(&file, &kept(BTreeSet::new()), &[], codes);
    assert_eq!(unused.len(), 275);
    let kanji = |code: &u32| TABLE.characters.get(code).is_some_and(|char| fonts::is_kanji(*char));
    assert!(unused.iter().all(|code| kanji(code) && *code < SMALL_END));
    // Every string of area 096 changed frees the kanji only they used, which come first.
    let area = areas.iter().find(|area| area.entry == 96).unwrap();
    let all: BTreeSet<(&str, u32)> = area.strings().keys().map(|number| ("096", *number)).collect();
    let freed = free_cells(&file, &kept(all.clone()), &[], codes);
    let more = freed.len() - unused.len();
    assert!(more > 0 && freed[more..] == unused[..], "{more}");
    // A code the pack's own text writes stays.
    let written = format!("{{K:{:04X}}}", freed[0]);
    assert!(!free_cells(&file, &kept(all), &[written.as_str()], codes).contains(&freed[0]));
}

#[test]
fn build_draws_two_characters_to_a_cell_in_both_fonts() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    let text = "{SAY}Wake up, {NAME:19}! It's morning, élan.{/SAY}";
    let built = build(disc, &pack_of(&[(109, text)]), &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[2..],
        ["pairs:     14 pairs of characters drawn two to a cell, in the cells of kanji no unchanged string uses, of \
          275"]
    );
    let copy = Disc::open(&out).unwrap();
    let new_areas = areas(&copy).unwrap();
    let strings = new_areas.iter().find(|area| area.entry == 96).unwrap().strings();
    // Every character of the line is in a pair, written in two bytes; each pair's cell is a kanji's
    // that no string uses, drawn over in both fonts, the other fonts in the same pixels as they were.
    let pairs = two_byte_codes(strings[&109]);
    assert_eq!(pairs.len(), 14);
    let index = |code: u32| (code - FIRST_CODE) as usize;
    for code in &pairs {
        assert!(TABLE.characters.get(code).is_some_and(|char| fonts::is_kanji(*char)), "{code:04X}");
        let large = |plane: usize| FIRST_CODE + (index(*code) & !0x300 | plane << 8) as u32;
        let small = |plane: usize| FIRST_CODE + (plane * SMALL_PER_PLANE + index(*code) % SMALL_PER_PLANE) as u32;
        for (cell, code_of_plane) in
            [(large_cell(*code), &large as &dyn Fn(usize) -> u32), (small_cell(*code).unwrap(), &small)]
        {
            assert_ne!(cell_pixels(&copy, cell), cell_pixels(disc, cell), "{code:04X}");
            for plane in (0..4).filter(|plane| !pairs.contains(&code_of_plane(*plane))) {
                let other = Cell { plane, ..cell };
                assert_eq!(cell_pixels(&copy, other), cell_pixels(disc, other), "{code:04X} {plane}");
            }
        }
    }
    // The next line is the game's own, and so are the glyphs it draws.
    let own = areas(disc).unwrap();
    let own_110 = own.iter().find(|area| area.entry == 96).unwrap().strings()[&110].to_vec();
    assert_eq!(strings[&110], own_110.as_slice());
    for code in two_byte_codes(&own_110) {
        assert_eq!(cell_pixels(&copy, large_cell(code)), cell_pixels(disc, large_cell(code)));
    }
    // A pack that needs more pairs than there are cells: those written most get cells, the others
    // are written a character to a cell, and it builds.
    let letters: Vec<char> = ('a'..='z').collect();
    let every: String = letters.iter().flat_map(|one| letters.iter().flat_map(move |other| [*one, *other])).collect();
    let built = build(disc, &pack_of(&[(109, &every)]), &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[2],
        "pairs:     275 pairs of characters drawn two to a cell, in the cells of kanji no unchanged string uses, of \
         275; 401 more, those written least, are written a character to a cell"
    );
}

#[test]
fn a_character_drawn_whole_does_not_stop_the_pairs_after_it_from_being_counted() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // 한 is 16 pixels wide in unifont, so it is drawn into a blank cell; the two é after it are a pair,
    // counted although 한 has no code while the pairs are counted.
    let built = build(disc, &pack_of(&[(109, "{SAY}한éé{/SAY}")]), &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[2..],
        [
            "font:      1 new characters drawn into its blank cells, of 26: 한",
            "pairs:     1 pairs of characters drawn two to a cell, in the cells of kanji no unchanged string uses, of \
             275"
        ]
    );
}

/// The first `count` pairs of two capitals, each written twice, in one run: "AAAAABAB…".
fn capital_pairs(count: usize) -> String {
    let capitals: Vec<char> = ('A'..='Z').collect();
    let keys = capitals.iter().flat_map(|one| capitals.iter().map(move |other| (*one, *other)));
    keys.take(count).flat_map(|(one, other)| [one, other, one, other]).collect()
}

#[test]
fn the_pairs_left_without_a_cell_are_counted_as_the_lines_are_broken() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // 300 pairs of capitals, written twice, take every cell. The pairs of the second string, written
    // once each, have none. Counted as if each had a cell, its words fit on one line and pair as "ab",
    // " c", "d ", "ef" and so on, 15 pairs; written a character to a cell, they take two lines, the
    // second starting at "kl", and pair as 14: the 25 pairs of capitals left over and those 14 are
    // written a character to a cell.
    let pack = pack_of(&[(109, &capital_pairs(300)), (110, "ab cd ef gh ij kl mn op qr st")]);
    let built = build(disc, &pack, &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[2],
        "pairs:     275 pairs of characters drawn two to a cell, in the cells of kanji no unchanged string uses, of \
         275; 39 more, those written least, are written a character to a cell"
    );
    let copy = Disc::open(&out).unwrap();
    let new_areas = areas(&copy).unwrap();
    let strings = new_areas.iter().find(|area| area.entry == 96).unwrap().strings();
    assert_eq!(strings[&110].iter().filter(|byte| **byte == 0x0D).count(), 1);
}

#[test]
fn a_character_whose_pairs_all_have_cells_takes_no_blank_cell() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // 27 accented letters, each in a pair written three times, take cells first, and pairs of capitals,
    // written twice, the rest; those left over are written with the font's full-width capitals. No
    // accented letter is written alone, so none needs one of the 26 blank cells.
    let accented: String = "àáâãäåæçèéêëìíîïñòóôõöøùúûü".chars().flat_map(|char| [char; 6]).collect();
    assert_eq!(accented.chars().count(), 27 * 6);
    let pack = pack_of(&[(109, &capital_pairs(300)), (110, &accented)]);
    let built = build(disc, &pack, &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary[2..],
        ["pairs:     275 pairs of characters drawn two to a cell, in the cells of kanji no unchanged string uses, of \
          275; 52 more, those written least, are written a character to a cell"]
    );
}
