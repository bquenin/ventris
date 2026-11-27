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

/// A string of the pack as it is written: wrapped or not, then fitted.
fn written(text: &str, wrapping: bool) -> Result<Vec<u8>> {
    let codes = codes();
    let text = if wrapping { wrap(text)? } else { text.to_string() };
    let data = encode_with(&text, &codes, &|char| pack_code(char, &codes, &HashMap::new()))?;
    fit(&data, &codes, wrapping)
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
    program_string(text, offset, &hex("a3"), &codes(), &HashMap::new(), wrap).map_err(|error| error.to_string())
}

#[test]
fn a_name_of_the_party_has_characters_alone() {
    let names = program_text::NAMES;
    assert_eq!(program_written("AA", names, true).unwrap(), hex("a3 a3"));
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
    let built = build(disc, &pack, &out, &BuildOptions::default()).unwrap();
    assert_eq!(
        built.summary,
        [
            "dialogue:  1 strings changed in 1 areas",
            "room:      the largest area changed, 096, has 55576 bytes of code, of the 186832 known to work",
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
    let built = build(disc, &pack_of(&[(109, &long)]), &out, &BuildOptions::default()).unwrap();
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
    // Too large for the room the game is known to have: refused, and nothing written.
    fs::remove_file(&out).unwrap();
    let huge = format!("{{SAY}}{}{{/SAY}}", "日".repeat(70_000));
    let refused = build(disc, &pack_of(&[(109, &huge)]), &out, &BuildOptions::default()).err().unwrap().to_string();
    assert!(refused.starts_with("area 096: its code, which holds its text, would be"), "{refused}");
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
    let built = build(disc, &pack, &out, &BuildOptions::default()).unwrap();
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
    let refused = build(disc, &pack, &out, &BuildOptions::default()).err().unwrap().to_string();
    let lines: Vec<&str> = refused.lines().collect();
    assert_eq!(lines.len(), 6, "{refused}");
    assert!(lines[0].starts_with("string program.657748: it has 3 lines, and a string of this part has room for 2"));
    assert!(lines[1].starts_with("string program.701264: it has 'e', and a name of the party"), "{refused}");
    assert!(lines[1].ends_with("write it in capitals"));
    assert_eq!(
        lines[2],
        "string program.701312: broken at spaces, it still has a line of 8 characters, and its box, as wide as the \
         game's own text, has room for 5: shorten its longest word"
    );
    assert!(lines[3].starts_with("string program.713338: broken at spaces, it still has a line of 8 characters"));
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
