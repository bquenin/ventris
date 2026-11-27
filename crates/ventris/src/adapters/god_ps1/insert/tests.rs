use std::fs;

use super::*;
use crate::adapters::god_ps1::{AREAS, entries, extract, glyph};
use crate::platform::Image;
use crate::platforms::ps1::exe;
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
    let none = HashMap::new();
    let writing = Writing { codes: &codes, drawn: &none, renderer: None };
    fit(&encode_text(&text, &writing)?, &writing, wrapping)
}

/// A changed string's text broken at spaces to the window, a character to a cell.
fn wrap(text: &str) -> Result<String> {
    wrap_to(text, COLUMNS)
}

/// The same, to lines of `columns` cells.
fn wrap_to(text: &str, columns: usize) -> Result<String> {
    let (codes, none) = (codes(), HashMap::new());
    let writing = Writing { codes: &codes, drawn: &none, renderer: None };
    super::wrap_to(text, columns * vwf::LARGE, LINE_CELLS, &writing, Face::Large)
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
    assert!(long.starts_with("its line 1 is wider than the window"), "{long}");
    let spoken = written(&format!("{{SAY}}AA\n{}{{/SAY}}", "A".repeat(16)), false).unwrap_err().to_string();
    assert!(spoken.starts_with("its line 2 is wider"), "{spoken}");
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

/// A changed string of the program's text at `offset` as it is written, the game's own being Ａ.
fn program_written(text: &str, offset: u32, wrap: bool) -> std::result::Result<Vec<u8>, String> {
    let (codes, none) = (codes(), HashMap::new());
    let writing = Writing { codes: &codes, drawn: &none, renderer: None };
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

#[test]
fn areas_that_grew_go_in_the_sectors_moved_areas_leave_before_the_free_room() {
    // Three areas that grew, the first two side by side, and the disc's free room far past them.
    let grown = [(0, 10, 12), (10, 10, 11), (30, 5, 6)];
    let (places, room) = place_grown(&grown, 100..120);
    // Together the first two leave a piece of 20 sectors: the largest area takes its start, the
    // next has no piece left large enough and goes to the free room, and the smallest fits in
    // what is left of the 20.
    assert_eq!(places, [Some(0), Some(100), Some(12)]);
    assert_eq!(room, 25);
    // The free room is used one area after another; an area it has no room left for has no place.
    let (places, _) = place_grown(&[(0, 4, 5), (20, 4, 6), (40, 4, 7)], 100..112);
    assert_eq!(places, [Some(100), Some(105), None]);
    // Nothing grew: nothing to place.
    assert_eq!(place_grown(&[], 100..120), (vec![], 0));
}

#[test]
fn a_door_short_on_the_heap_is_said_once_and_never_dropped() {
    let short = |area, other, mutual, moment: &str| Shortfall { area, other, mutual, moment: moment.into(), over: 8 };
    let changed = BTreeSet::from([153]);
    // A door the events show between a changed area and one that is not: both sides count it, and
    // the build says it from the changed side alone.
    let both = vec![
        short(153, Some(154), true, "going from it into area 154"),
        short(154, Some(153), true, "going into it from area 153"),
    ];
    let said = heap_refusals(both, &changed);
    assert_eq!(said.len(), 1);
    assert!(said[0].starts_with("area 153: going from it into area 154,"), "{said:?}");
    assert!(said[0].ends_with(": shorten some of its changed lines"), "{said:?}");
    // A door assumed into an unchanged area from the changed one, which has no door there: no moment
    // of the changed area counts it, so it is said from the unchanged one, with the lines to shorten.
    let assumed = vec![short(123, Some(153), false, "going into it from area 153")];
    let said = heap_refusals(assumed, &changed);
    assert_eq!(said.len(), 1);
    assert!(said[0].starts_with("area 123: going into it from area 153,"), "{said:?}");
    assert!(said[0].ends_with(": shorten some of the changed lines of area 153"), "{said:?}");
    // Between two changed areas, each side says it; an unchanged area alone is short for the grids.
    let said = heap_refusals(
        vec![short(154, Some(153), true, "going into it from area 153"), short(96, None, false, "in a battle")],
        &BTreeSet::from([153, 154]),
    );
    assert!(said[0].ends_with("(or those of area 153)"), "{said:?}");
    assert!(said[1].contains(FULL_WIDTH), "{said:?}");
}

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
    // A line long enough that the area takes more sectors: alone, it moves to the disc's free room,
    // and every entry that names it follows.
    let long = format!("{{SAY}}{}{{/SAY}}", "élan ".repeat(1200));
    let built = build(disc, &pack_of(&[(109, &long)]), &out, &full_width()).unwrap();
    assert_eq!(
        built.summary[1],
        format!(
            "moved:     area 096 grew past its sectors, and is now in the sectors such areas left in {ARCHIVE} or \
             in the disc's free room ({SPARE_FILE}), of which 933 sectors of 16384 are used"
        )
    );
    let old = program(disc).unwrap().contents.clone();
    {
        let copy = Disc::open(&out).unwrap();
        let contents = &program(&copy).unwrap().contents;
        let spare = copy.first_sector(SPARE_FILE).unwrap() - copy.first_sector(ARCHIVE).unwrap();
        assert_eq!(contents[96].0, spare);
        assert!(contents[189..=195].iter().all(|entry| *entry == contents[96]));
        assert_eq!(&contents[..96], &old[..96]);
        let text = extract(&copy).unwrap();
        assert_eq!(text["096"][&110], original["096"][&110]);
        assert!(text["096"][&109].len() > 6000);
    }
    // With area 097, the one after it, grown too, the two leave one piece of sectors: area 096, the
    // larger, takes its start, over the sectors 097 had, and 097 goes to the free room.
    let pack = Pack::of(Script::from([
        ("096".to_string(), BTreeMap::from([(109, long.clone())])),
        ("097".to_string(), BTreeMap::from([(4, long.clone())])),
    ]));
    let built = build(disc, &pack, &out, &full_width()).unwrap();
    assert!(built.summary[1].starts_with("moved:     areas 096, 097 grew past their sectors"), "{}", built.summary[1]);
    {
        let copy = Disc::open(&out).unwrap();
        let contents = &program(&copy).unwrap().contents;
        let spare = copy.first_sector(SPARE_FILE).unwrap() - copy.first_sector(ARCHIVE).unwrap();
        assert_eq!(contents[96].0, old[96].0);
        assert!(contents[96].1 as usize > (old[97].0 - old[96].0) as usize * SECTOR);
        assert_eq!(contents[97].0, spare);
        assert!(contents[189..=195].iter().all(|entry| *entry == contents[96]));
        let text = extract(&copy).unwrap();
        assert!(text["096"][&109].len() > 6000 && text["097"][&4].len() > 6000);
        assert_eq!((&text["096"][&110], &text["097"][&3]), (&original["096"][&110], &original["097"][&3]));
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
            "program:   6 strings changed, 5 of them moved in the room they share, taking 26 bytes of its 39, 1 of \
             them the same text as another and written once",
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
        // Without the renderer and with no area changed, the heap is as the game has it: not checked,
        // and the game still reads a new area's rest in advance.
        assert_eq!(program_word(&copy, heap::REST_IN_ADVANCE), program_word(disc, heap::REST_IN_ADVANCE));
        assert_eq!(file[687398..687460], program_text::program_file(disc).unwrap()[687398..687460]);
    }
    // With the renderer, which a program's string in Latin letters puts in, every area is checked, and
    // the game made to read the rest after.
    fs::remove_file(&out).unwrap();
    let built = build(disc, &program_pack(&[(657748, "Welcome!")], &[]), &out, &BuildOptions::default()).unwrap();
    assert!(built.summary.iter().any(|line| line.starts_with("room:      105200 bytes")), "{:?}", built.summary);
    assert_eq!(program_word(&Disc::open(&out).unwrap(), heap::REST_IN_ADVANCE), 0x0000_1021);
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
        "string program.701312: broken at spaces, it still has a line 96 pixels wide, and its box, as wide as the \
         game's own text, has room for 60 (5 cells of 12 pixels): shorten its longest word"
    );
    assert!(lines[3].starts_with("string program.713338: broken at spaces, it still has a line 96 pixels wide"));
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
fn the_shop_s_labels_go_in_their_slots_and_a_format_keeps_its_conversions() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // The shop's commands, its count of what the party has, its yes and no, in the read-only data
    // before the program's code; and the count beside them, a format.
    let labels = [(5576, "Buy"), (5580, "Sel"), (5584, "Leave"), (5592, "Owned"), (5620, "Yes No"), (863884, "%8d")];
    let built = build(disc, &program_pack(&[], &labels), &out, &BuildOptions::default()).unwrap();
    assert_eq!(built.summary[1..], ["menu:      6 labels changed, each in its slot"]);
    let file = program_text::program_file(&Disc::open(&out).unwrap()).unwrap();
    assert_eq!(file[0x15C8..0x15E4], *b"BUY\0SEL\0LEAVE\0\0\0OWNED\0\0\0\0\0\0\0");
    assert_eq!(file[0x15E4..0x15F4], program_text::program_file(disc).unwrap()[0x15E4..0x15F4]);
    assert_eq!(file[0x15F4..0x1600], *b"YES NO\0\0\0\0\0\0");
    assert_eq!(file[863884..863892], *b"%8d\0\0\0\0\0");
    // A label must fill in what the game's own does, in the same order, and nothing else; and the
    // commands' slots are small.
    fs::remove_file(&out).unwrap();
    let labels = [(863884, "%8d %s"), (3328, "TURNS"), (5580, "Sell")];
    let refused = build(disc, &program_pack(&[], &labels), &out, &BuildOptions::default()).err().unwrap().to_string();
    let lines: Vec<&str> = refused.lines().collect();
    assert_eq!(lines.len(), 3, "{refused}");
    assert!(lines[0].starts_with("string menu.3328: it fills in nothing where the game's own label fills in %d"));
    assert!(lines[1].starts_with("string menu.5580: it takes 4 bytes, and its slot has room for 3"), "{refused}");
    assert!(lines[2].starts_with("string menu.863884: it fills in %d %s where the game's own label fills in %d"));
    assert!(!out.exists());
}

/// The word at an address in the program's file.
fn program_word(disc: &Disc, address: u32) -> u32 {
    let file = program_text::program_file(disc).unwrap();
    let at = exe::offset(&file, address, 4).unwrap();
    u32::from_le_bytes(file[at..at + 4].try_into().unwrap())
}

#[test]
fn area_105_grows_as_far_as_the_three_copies_from_area_107_leave_room() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    let pack = |kanji: usize| {
        let text = format!("{{SAY}}{}{{/SAY}}", "日".repeat(kanji));
        Pack::of(Script::from([("105".to_string(), BTreeMap::from([(4, text)]))]))
    };
    // Going from area 107 into area 105, the original game leaves only `SPARE` bytes of its heap free,
    // as it has read 105's rest in advance; the build makes it read the rest after, which leaves
    // 249,048 bytes free there. The three copies of 105's code take three times what it grows.
    let built = build(disc, &pack(30_000), &out, &BuildOptions::default()).unwrap();
    let copy = Disc::open(&out).unwrap();
    let grown = footprints(&copy).unwrap()[&105].code - footprints(disc).unwrap()[&105].code;
    assert!(grown > 60_000, "{grown}");
    assert_eq!(
        built.summary[2],
        format!(
            "room:      {} bytes of the heap free at the tightest moment, area 105, going into it from area 107 \
             (the game leaves 19636 at its own)",
            SPARE + 229_412 - 3 * grown
        )
    );
    assert_eq!(program_word(&copy, heap::REST_IN_ADVANCE), 0x0000_1021);
    assert_eq!(program_word(disc, heap::REST_IN_ADVANCE), 0x0C00_AD89);
    // Grown by more than a third of what is past `SPARE` there, it no longer fits.
    fs::remove_file(&out).unwrap();
    let refused = build(disc, &pack(40_000), &out, &BuildOptions::default()).err().unwrap().to_string();
    assert!(refused.starts_with("area 105: going into it from area 107, the game would need "), "{refused}");
    assert!(refused.ends_with(" bytes more of its heap than it has room for: shorten some of its changed lines"));
    assert!(!out.exists());
}

#[test]
fn an_insert_written_by_its_byte_is_measured_as_under_its_name() {
    // In a shop's line of 13 cells, a word the game fills in takes 8, however it is written.
    assert_eq!(wrap_to("AAAA AAAA {WORD}", 13).unwrap(), "AAAA AAAA\n{WORD}");
    assert_eq!(wrap_to("AAAA AAAA {0C:12}", 13).unwrap(), "AAAA AAAA\n{0C:12}");
    assert_eq!(wrap_to("AAAA AAAA {0C:0D}", 13).unwrap(), "AAAA AAAA\n{0C:0D}");
    assert_eq!(wrap_to("AAAA AAAA {0C:01}", 13).unwrap(), "AAAA AAAA\n{0C:01}");
}

// --- characters at their own widths ----------------------------------------

/// The renderer a build would make for every ASCII character but the braces, from Unifont, in
/// cells from $0200.
fn renderer() -> Renderer {
    let characters: Vec<char> = (' '..='~').filter(|char| !matches!(char, '{' | '}')).collect();
    let glyphs = fonts::narrow_glyphs(&Font::built_ins(FONTS), &characters).unwrap();
    let free: Vec<u32> = (0x200..0x300).collect();
    Renderer::new(&glyphs, &free).unwrap()
}

/// The width of some text written with `renderer`, every character its own.
fn width_of(text: &str, renderer: &Renderer) -> usize {
    text.chars().map(|char| renderer.width(char).unwrap()).sum()
}

#[test]
fn the_renderer_s_characters_are_written_a_byte_each_in_runs() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let (a, b, space) = (renderer.byte('a').unwrap(), renderer.byte('b').unwrap(), renderer.byte(' ').unwrap());
    let encoded = |text: &str| encode_text(text, &writing).unwrap();
    assert_eq!(encoded("ab a"), [0x0A, a, b, space, a]);
    // A character of the game's font after a run ends it with the same byte; a byte under $10, a
    // line break, a tag, a code of two bytes, ends it anyway.
    assert_eq!(encoded("aあa"), [0x0A, a, 0x0A, 0xA2, 0x0A, a]);
    assert_eq!(encoded("a\na"), [0x0A, a, 0x0D, 0x0A, a]);
    assert_eq!(encoded("a{WAIT}\na"), [0x0A, a, 0x0B, 0x0A, a]);
    assert_eq!(encoded("{NAME:19}a{PAUSE:14}b"), [0x0C, 0x19, 0x0A, a, 0x0F, 0x00, 0x14, 0x0A, b]);
    assert_eq!(encoded("a{W:0234}"), [0x0A, a, 0x02, 0x34]);
    // Read back, each character is as wide as the renderer draws it, a glyph of the game's 14 pixels.
    let wide = |text: &str| line_widths(&encoded(text), &writing, Face::Large);
    assert_eq!(wide("ab a"), [width_of("ab a", &renderer)]);
    assert_eq!(wide("aあa\nb"), [2 * width_of("a", &renderer) + 14, width_of("b", &renderer)]);
    assert_eq!(line_widths(&encoded("aあ"), &writing, Face::Small), [width_of("a", &renderer) + 12]);
    // A built string reads back as text, its runs as tags, and writes back to the same bytes.
    let data = encoded("ab\nあ");
    let text = decode(&data, &codes);
    assert_eq!(text, format!("{{RUN}}{{B:{a:02X}}}{{B:{b:02X}}}\nあ"));
    assert_eq!(super::super::encode(&text, &codes).unwrap(), data);
    assert_eq!(super::super::read_text(&[data.as_slice(), &[0]].concat(), &codes), Some((data.len(), 3)));
    // Without the renderer nothing changes: the font's full-width forms, a cell each.
    let full = Writing { renderer: None, ..writing };
    assert_eq!(encode_text("Aa", &full).unwrap(), hex("a3 a4"));
    // A run the pack writes itself needs the renderer, and a character after each {RUN}; a byte
    // written by itself does not.
    let refused = encode_text("{RUN}{B:10}", &full).unwrap_err().to_string();
    assert!(refused.contains("this build draws none"), "{refused}");
    assert_eq!(encode_text("{B:10}", &full).unwrap(), [0x10]);
    assert_eq!(encode_text("{RUN}{B:10}{RUN}あ", &writing).unwrap(), [0x0A, 0x10, 0x0A, 0xA2]);
    // A letter after a run the pack starts itself goes on in it, and one after a run it ends starts
    // another.
    assert_eq!(encode_text("{RUN}{B:10}a", &writing).unwrap(), [0x0A, 0x10, a]);
    assert_eq!(encode_text("あ{RUN}a", &writing).unwrap(), [0xA2, 0x0A, a]);
    assert_eq!(encode_text("{RUN}{B:10}{RUN}あa", &writing).unwrap(), [0x0A, 0x10, 0x0A, 0xA2, 0x0A, a]);
    // In a run, a byte is read by the renderer's table, not the font's: one it has no character for is
    // refused, though the font has one.
    assert!(renderer.byte_code(0xA2).is_none() && codes.code(0xA2) != 0);
    let refused = encode_text("{RUN}{B:A2}", &writing).unwrap_err().to_string();
    assert!(refused.contains("the byte $A2 in a run"), "{refused}");
    assert_eq!(encode_text("{B:A2}", &writing).unwrap(), [0xA2]);
    for unfollowed in ["{RUN}{B:10}{RUN}", "{RUN}{B:10}{RUN}\nあ", "{RUN}{RUN}{B:10}", "{RUN}{WAIT}"] {
        let refused = encode_text(unfollowed, &writing).unwrap_err().to_string();
        assert!(refused.contains("not followed by a character"), "{unfollowed}: {refused}");
    }
}

/// A string's bytes with each byte of a run that stands for a pair written as its two characters'.
fn unpaired(data: &[u8], writing: &Writing, pairs: &Pairs) -> Vec<u8> {
    let found = pieces(data, writing);
    let mut out = Vec::new();
    for ((piece, bytes), run) in found.iter().zip(run_states(&found)) {
        match pairs.split(bytes[0]) {
            Some((first, second)) if run && *piece == Piece::Silent => out.extend([first, second]),
            _ => out.extend(bytes),
        }
    }
    out
}

#[test]
fn pairs_are_put_in_the_runs_of_a_string_laid_out_a_byte_a_character() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let byte = |char: char| renderer.byte(char).unwrap();
    // A line that ends at the window's edge, whose line break is left out, so its run goes on into
    // the next line; a name, which ends a run; a kana, which a run's end comes before.
    let width = |char: char| renderer.width(char).unwrap();
    let letter = ('a'..='z').find(|char| width(*char) >= 4 && WINDOW_WIDTH.is_multiple_of(width(*char))).unwrap();
    let full = letter.to_string().repeat(WINDOW_WIDTH / width(letter));
    let text = format!("{full}\nthe then{{NAME:19}}the あ the");
    let data = fit(&encode_text(&text, &writing).unwrap(), &writing, true).unwrap();
    let runs: Vec<Vec<u8>> = run_stretches(&data, &writing).into_iter().map(|run| data[run].to_vec()).collect();
    let the = [byte('t'), byte('h'), byte('e')];
    assert_eq!(runs.len(), 3, "{runs:02X?}");
    assert_eq!(runs[0][runs[0].len() - 8..], [&the[..], &[byte(' ')], &the[..], &[byte('n')]].concat());
    assert_eq!(runs[1], [&the[..], &[byte(' ')]].concat());
    assert_eq!(runs[2], [&[byte(' ')][..], &the[..]].concat());
    let pairs = Pairs::choose(&renderer, &runs);
    let written = paired(&data, &writing, &pairs);
    assert!(written.len() < data.len(), "{written:02X?}");
    // Everything but the runs' bytes is as it was, and each pair stands for its two characters.
    assert_eq!(unpaired(&written, &writing, &pairs), data);
    assert_eq!(written.iter().filter(|byte| **byte < 0x10).count(), data.iter().filter(|byte| **byte < 0x10).count());
    // With no pairs, nothing changes.
    assert_eq!(paired(&data, &writing, &Pairs::default()), data);
}

#[test]
fn a_program_string_read_from_inside_is_not_paired() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    // Two program strings of the same letters; the program also reads the second from its line
    // break on, so its bytes before it may change only to as many bytes, and the rest not at all.
    let letters = encode_text("the then the", &writing).unwrap();
    let kept = vec![0x0D, 0xA2];
    let inside = [&letters[..], &kept].concat();
    let room = |inside| program_text::Room { size: 64, tail: Vec::new(), reached: None, inside };
    let rooms = BTreeMap::from([(0x100, room(None)), (0x200, room(Some((letters.len(), kept.clone()))))]);
    // Pairs in the second would move what the program reads from inside it.
    let pairs = Pairs::choose(&renderer, &[letters[1..].to_vec()]);
    let moved = BTreeMap::from([(0x200, paired(&inside, &writing, &pairs))]);
    let refused = program_text::write_strings(&[], &rooms, &moved).unwrap_err();
    assert!(refused.contains("the rest must stay as it is"), "{refused}");
    // So only the first is paired, and both are written.
    let mut changed = BTreeMap::from([(0x100, letters.clone()), (0x200, inside.clone())]);
    let pairs = put_pairs(&mut BTreeMap::new(), &mut changed, &rooms, &writing, &renderer);
    assert!(!pairs.is_empty());
    assert!(changed[&0x100].len() < letters.len(), "{:02X?}", changed[&0x100]);
    assert_eq!(changed[&0x200], inside);
    assert!(program_text::write_strings(&[], &rooms, &changed).is_ok());
    // Its runs are not counted for the pairs either: alone, it gives none.
    let mut alone = BTreeMap::from([(0x200, inside.clone())]);
    assert_eq!(put_pairs(&mut BTreeMap::new(), &mut alone, &rooms, &writing, &renderer).len(), 0);
    assert_eq!(alone[&0x200], inside);
}

#[test]
fn a_line_break_left_out_after_a_run_does_not_end_it() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    // A line of letters that ends at the window's edge exactly, after which the game starts the next
    // line itself, so the line break is left out; it ended the run too.
    let width = |char: char| renderer.width(char).unwrap();
    let letter = ('a'..='z').find(|char| width(*char) >= 4 && WINDOW_WIDTH.is_multiple_of(width(*char))).unwrap();
    let count = WINDOW_WIDTH / width(letter);
    let full = letter.to_string().repeat(count);
    let (run, a) =
        ([&[vwf::RUN][..], &vec![renderer.byte(letter).unwrap(); count]].concat(), renderer.byte('a').unwrap());
    for wrapping in [true, false] {
        let laid = |text: &str| fit(&encode_text(text, &writing).unwrap(), &writing, wrapping).unwrap();
        // The next line starts with a letter: the run goes on, and the letter is read in it.
        let data = laid(&format!("{full}\na"));
        assert_eq!(data, [&run[..], &[a]].concat());
        assert_eq!(pieces(&data, &writing).last().unwrap().0, Piece::Run(renderer.code('a').unwrap()));
        // With a character of the game's font, the run is ended before it.
        assert_eq!(laid(&format!("{full}\nあ")), [&run[..], &[vwf::RUN, 0xA2]].concat());
        // With a tag, which ends a run anyway, nothing is added.
        assert_eq!(laid(&format!("{full}\n{{PAUSE:14}}a")), [&run[..], &[0x0F, 0x00, 0x14, vwf::RUN, a]].concat());
    }
}

#[test]
fn a_wait_put_inside_a_run_starts_the_run_again_after_it() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    // Five lines of letters with no line break to turn into a wait: one is put before the letter that
    // would scroll the window, inside the run, and every letter is still read as the renderer's.
    let width = |char: char| renderer.width(char).unwrap();
    let letter = ('a'..='z').find(|char| WINDOW_WIDTH.is_multiple_of(width(*char))).unwrap();
    let text = letter.to_string().repeat(WINDOW_WIDTH / width(letter) * (ROWS + 1));
    let data = fit(&encode_text(&text, &writing).unwrap(), &writing, true).unwrap();
    assert!(data.contains(&0x0B), "{data:02X?}");
    let code = renderer.code(letter).unwrap();
    let read = pieces(&data, &writing);
    assert!(
        read.iter().all(|(piece, _)| matches!(piece, Piece::Run(found) if *found == code)
            || matches!(piece, Piece::Wait | Piece::Silent)),
        "{read:?}"
    );
    assert_eq!(read.iter().filter(|(piece, _)| *piece == Piece::Run(code)).count(), text.len());
}

#[test]
fn a_wait_put_before_what_is_not_a_letter_of_a_run_starts_no_run() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    // Four lines of letters, the last ending a 「 short of the window's edge: the 「 that `{SAY}` draws
    // fills it, and the wait goes before it, where no run is started again, as 0E ends one anyway.
    let width = |char: char| renderer.width(char).unwrap();
    let full = ('a'..='z').find(|char| WINDOW_WIDTH.is_multiple_of(width(*char))).unwrap();
    let short = ('a'..='z').find(|char| (WINDOW_WIDTH - vwf::LARGE).is_multiple_of(width(*char))).unwrap();
    let text = format!(
        "{}{}{{SAY}}a{{/SAY}}",
        full.to_string().repeat(WINDOW_WIDTH / width(full) * (ROWS - 1)),
        short.to_string().repeat((WINDOW_WIDTH - vwf::LARGE) / width(short))
    );
    let data = fit(&encode_text(&text, &writing).unwrap(), &writing, true).unwrap();
    let wait = data.iter().position(|byte| *byte == 0x0B).unwrap();
    assert_eq!(data[wait + 1], 0x0E, "{data:02X?}");
    // The letter after it starts a run of its own.
    assert_eq!(&data[wait + 2..], [vwf::RUN, renderer.byte('a').unwrap(), 0x0E]);
}

#[test]
fn lines_are_broken_where_the_next_word_would_run_past_the_window() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let text = "Oh my, are you still half asleep? You were so excited you couldn't sleep, weren't you?";
    for (wrapped, limit) in [(text.to_string(), WINDOW_WIDTH), (format!("{{SAY}}{text}{{/SAY}}"), WINDOW_WIDTH - 14)] {
        let broken = super::wrap(&wrapped, &writing).unwrap();
        let lines: Vec<&str> = broken.trim_start_matches("{SAY}").trim_end_matches("{/SAY}").split('\n').collect();
        assert!(lines.len() > 1, "{broken}");
        for (line, next) in lines.iter().zip(&lines[1..]) {
            // Each line fits, and would not with the next line's first word.
            assert!(width_of(line, &renderer) <= limit, "{line}");
            let first = next.split(' ').next().unwrap();
            assert!(width_of(&format!("{line} {first}"), &renderer) > limit, "{line} | {first}");
        }
        assert!(fit(&encode_text(&broken, &writing).unwrap(), &writing, true).is_ok());
    }
    // A line holds about 35 letters, where it held 16 characters a cell each.
    let line = super::wrap(text, &writing).unwrap();
    assert!(line.split('\n').next().unwrap().chars().count() > 30, "{line}");
    // A word wider than the window is refused, as no line break helps it.
    // (Of a letter whose width does not divide the window's: a line that ends at its edge exactly
    // is one the game starts the next of itself, mid-word, as it does with its own text.)
    let letter = ('a'..='z').find(|char| !WINDOW_WIDTH.is_multiple_of(renderer.width(*char).unwrap())).unwrap();
    let long = letter.to_string().repeat(60);
    let refused = fit(&encode_text(&long, &writing).unwrap(), &writing, true).unwrap_err().to_string();
    assert!(refused.starts_with("broken at spaces, its line 1 is still wider than the window, 224 pixels"));
    // A spoken line that starts after other text indents its next lines under its first character.
    let late = super::wrap(&format!("　{{SAY}}{text}{{/SAY}}"), &writing).unwrap();
    let second = late.split('\n').nth(1).unwrap();
    assert!(width_of(second, &renderer) <= WINDOW_WIDTH - 28, "{late}");
}

#[test]
fn a_line_is_broken_where_its_cells_run_out_and_a_spoken_line_moved_down_is_measured_again() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    // The narrowest letters fill a line's cells before its pixels: the line is broken at the space
    // before the letter that would take a cell past them, as the game would start a new line there.
    let narrow =
        (' '..='~').filter(|char| !matches!(char, ' ' | '{' | '}')).min_by_key(|char| renderer.width(*char)).unwrap();
    assert!(renderer.width(narrow).unwrap() * (LINE_CELLS + 1) + renderer.width(' ').unwrap() <= WINDOW_WIDTH);
    let (first, second) = (narrow.to_string().repeat(LINE_CELLS / 2 + 1), narrow.to_string().repeat(LINE_CELLS / 2));
    let broken = super::wrap(&format!("{first} {second}"), &writing).unwrap();
    assert_eq!(broken, format!("{first}\n{second}"));
    assert!(fit(&encode_text(&broken, &writing).unwrap(), &writing, true).is_ok());
    // A spoken line that starts after the space the line is broken at draws its 「 at the start of
    // the next line, and its own lines start under its first character from there.
    let text = "AAAAAAAAAA {SAY}AAAAAA AAAAAAA{/SAY}";
    assert_eq!(wrap(text).unwrap(), "AAAAAAAAAA\n{SAY}AAAAAA AAAAAAA{/SAY}");
    // A 「 that would run past the window's edge starts the next line too, whatever follows it.
    let kana = "あ".repeat(WINDOW_WIDTH / vwf::LARGE - 1);
    let broken = super::wrap(&format!("{kana} {{SAY}} b{{/SAY}}"), &writing).unwrap();
    assert_eq!(broken, format!("{kana}\n{{SAY}} b{{/SAY}}"));
    assert!(fit(&encode_text(&broken, &writing).unwrap(), &writing, true).is_ok());
}

#[test]
fn a_name_of_the_party_is_never_drawn_at_its_own_width() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let names = program_text::NAMES;
    assert_eq!(program_string("AA", names, &hex("a3"), &writing, true).unwrap(), hex("a3 a3"));
    // A word of the status screen, whose part has lines of 7 cells, 84 pixels in the small font, is.
    let status = 0xA_B4AB;
    let written = program_string("Strength", status, &hex("a3"), &writing, true).unwrap();
    assert_eq!(written.len(), 9);
    assert_eq!(written[0], 0x0A);
    let full = Writing { renderer: None, ..writing };
    let refused = program_string("Strength", status, &hex("a3"), &full, true).unwrap_err().to_string();
    assert!(refused.contains("a line 96 pixels wide") && refused.contains("room for 84 (7 cells of 12 pixels)"));
}

#[test]
fn a_monster_s_name_leaves_room_for_the_letter_after_it() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let monster = program_text::MONSTER_NAMES.start + 3;
    // A run and 16 letters fit, with the space, the letter and the zero byte, in a record of 20: narrow
    // letters, so that the line is not too wide first.
    let (sixteen, seventeen) = ("il".repeat(8), format!("{}i", "il".repeat(8)));
    assert_eq!(program_string(&sixteen, monster, &hex("a3"), &writing, false).unwrap().len(), 17);
    let refused = program_string(&seventeen, monster, &hex("a3"), &writing, false).unwrap_err();
    assert!(refused.to_string().starts_with("it takes 18 bytes, and a battle copies a monster's name"), "{refused}");
    // Without the renderer the game puts its own letter alone after the name, as it does its own names.
    let full = Writing { renderer: None, ..writing };
    assert!(program_string("ABCDEFGHI", monster, &hex("a3"), &full, false).is_ok());
}

#[test]
fn a_place_s_name_fits_the_caption_s_copy_of_it() {
    let (codes, none, renderer) = (codes(), HashMap::new(), renderer());
    let writing = Writing { codes: &codes, drawn: &none, renderer: Some(&renderer) };
    let place = program_text::PLACE_NAMES.start + 0x10;
    // A run and 30 characters fit, with the zero byte, in the caption's 32 bytes: narrow ones, so that
    // the line is not too wide first.
    let (thirty, thirty_one) = ("|".repeat(30), "|".repeat(31));
    assert!(width_of(&thirty_one, &renderer) <= 96);
    assert_eq!(program_string(&thirty, place, &hex("a3"), &writing, false).unwrap().len(), 31);
    let refused = program_string(&thirty_one, place, &hex("a3"), &writing, false).unwrap_err();
    assert!(refused.to_string().starts_with("it takes 32 bytes, and the caption"), "{refused}");
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

#[test]
fn the_cells_the_renderer_takes_are_kanji_no_string_left_as_it_is_uses() {
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
    assert_eq!(unused.len(), 238);
    let kanji = |code: &u32| TABLE.characters.get(code).is_some_and(|char| fonts::is_kanji(*char));
    assert!(unused.iter().all(|code| kanji(code) && *code < SMALL_END));
    // None in the last column or the last row of a picture of the font, which its draw routines read
    // a pixel off: the 37 kanji there no text uses are left out.
    let last = |code: &u32| (code - FIRST_CODE) & 0xF == 0xF || (code - FIRST_CODE) & 0xF0 == 0xF0;
    assert!(!unused.iter().any(last));
    // Nor a code whose low byte is zero, which the renderer's two-byte codes for names and numbers
    // would turn into a string's end.
    assert!(unused.iter().all(|code| code & 0xFF != 0));
    assert!(drawn_true(0x29E) && !drawn_true(0x29F) && !drawn_true(0x2F0) && !drawn_true(0x5FF) && drawn_true(0x600));
    // Every string of area 096 changed frees the kanji only they used, which come first.
    let area = areas.iter().find(|area| area.entry == 96).unwrap();
    let all: BTreeSet<(&str, u32)> = area.strings().keys().map(|number| ("096", *number)).collect();
    let freed = free_cells(&file, &kept(all.clone()), &[], codes);
    let more = freed.len() - unused.len();
    assert!(more > 0 && freed[more..] == unused[..], "{more}");
    assert!(freed.iter().all(|code| code & 0xFF != 0 && drawn_true(*code)));
    // A code the pack's own text writes stays.
    let written = format!("{{K:{:04X}}}", freed[0]);
    assert!(!free_cells(&file, &kept(all), &[written.as_str()], codes).contains(&freed[0]));
}

#[test]
fn build_draws_each_character_at_its_own_width_and_installs_the_renderer() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    let text = "{SAY}Wake up, {NAME:19}! It's morning, élan.{/SAY}";
    let built = build(disc, &pack_of(&[(109, text)]), &out, &BuildOptions::default()).unwrap();
    // The string's characters, and every digit and capital, which a name or a number the game puts in
    // is drawn with.
    let characters = plain_characters("Wake up,! It's morning, élan.0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ").len();
    // With the renderer, every area's moments are checked, its grids counted at each, in the game
    // made to read a new area's rest after a door.
    assert_eq!(
        built.summary[1],
        "room:      105200 bytes of the heap free at the tightest moment, area 177, in a battle (the game leaves \
         19636 at its own)"
    );
    let pairs: usize = built.summary[2].split(", and ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!(pairs > 0);
    assert_eq!(
        built.summary[2..],
        [format!(
            "renderer:  {characters} characters drawn at their own widths, in cells of kanji no unchanged string \
             uses, of 238, and {pairs} pairs of them written a byte each; its code at $801F8000, loaded at start-up \
             from the program's file"
        )]
    );
    let copy = Disc::open(&out).unwrap();
    let new_areas = areas(&copy).unwrap();
    let strings = new_areas.iter().find(|area| area.entry == 96).unwrap().strings();
    // A byte a character, in runs, and no code of two bytes: 「, a run, the name, a run, a line break
    // where a space was and a run again, 「 again.
    let line = strings[&109];
    assert_eq!(line[..2], [0x0E, 0x0A]);
    assert!(!line.iter().any(|byte| (0x01..=0x09).contains(byte)));
    let runs = line.iter().filter(|byte| **byte == 0x0A).count();
    assert_eq!(runs, 2 + line.iter().filter(|byte| **byte == 0x0D).count());
    // Pairs of them a byte each, as the renderer's table in the program says: each byte of a run
    // stands for a character's code, or two.
    let (old, new) = (program_text::program_file(disc).unwrap(), program_text::program_file(&copy).unwrap());
    let code = vwf::PLACEMENT.code_address();
    let run_codes = (vwf::assemble_renderer().unwrap().label("run_codes").unwrap() - code) as usize;
    let table = exe::offset(&new, vwf::PLACEMENT.room.start, 4).unwrap()
        + (exe::LOADER_SIZE + exe::FINISH_SIZE) as usize
        + run_codes;
    let stands_for = |byte: u8| {
        let at = table + 4 * usize::from(byte);
        let second = u16::from_le_bytes([new[at + 2], new[at + 3]]);
        1 + usize::from(second != 0)
    };
    let (mut in_run, mut drawn_in_runs, mut run_bytes) = (false, 0, 0);
    for (at, byte) in line.iter().enumerate() {
        match *byte {
            0x0A => in_run = !in_run,
            byte if byte < 0x10 => in_run = false,
            _ if line[at - 1] == 0x0C => {}
            byte if in_run => (drawn_in_runs, run_bytes) = (drawn_in_runs + stands_for(byte), run_bytes + 1),
            _ => {}
        }
    }
    // A space became the line break.
    let letters = "Wake up, ".chars().count() + "! It's morning, élan.".chars().count();
    let breaks = line.iter().filter(|byte| **byte == 0x0D).count();
    assert_eq!(drawn_in_runs + breaks, letters);
    assert!(run_bytes < drawn_in_runs);
    assert_eq!(line.len(), 1 + runs + run_bytes + breaks + 2 + 1);
    // The next line is the game's own.
    let own = areas(disc).unwrap();
    assert_eq!(strings[&110], own.iter().find(|area| area.entry == 96).unwrap().strings()[&110]);
    // The program calls the loader from its start-up, and the loader and the code are in its room.
    let at = |address: u32| exe::offset(&new, address, 4).unwrap();
    let word = |data: &[u8], address: u32| u32::from_le_bytes(data[at(address)..at(address) + 4].try_into().unwrap());
    let placement = &vwf::PLACEMENT;
    assert_eq!(word(&new, placement.hook), 0x0C00_0000 | placement.room.start >> 2 & 0x03FF_FFFF);
    assert_ne!(word(&new, placement.room.start), 0);
    assert_eq!(word(&old, placement.room.start), 0);
    assert_eq!(word(&new, 0x8002_5DD0) >> 26, 3);
    // Each character's cells, kanji's that no string uses, are drawn in both fonts.
    let drawn: Vec<u32> = (FIRST_CODE..SMALL_END)
        .filter(|code| cell_pixels(&copy, large_cell(*code)) != cell_pixels(disc, large_cell(*code)))
        .collect();
    assert_eq!(drawn.len(), characters);
    for code in &drawn {
        assert!(TABLE.characters.get(code).is_some_and(|char| fonts::is_kanji(*char)), "{code:04X}");
        let small = small_cell(*code).unwrap();
        assert_ne!(cell_pixels(&copy, small), cell_pixels(disc, small), "{code:04X}");
    }
}

#[test]
fn a_wide_character_goes_in_a_blank_cell_and_the_narrow_ones_at_their_widths() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // 한 is 16 pixels wide in Unifont, so it is drawn into a blank cell; é is drawn at its own width,
    // with a space, the digits and the capitals, which the game's names and numbers are drawn with.
    let built = build(disc, &pack_of(&[(109, "{SAY}한éé{/SAY}")]), &out, &BuildOptions::default()).unwrap();
    assert_eq!(built.summary[2], "font:      1 new characters drawn into its blank cells, of 26: 한");
    assert!(built.summary[3].starts_with("renderer:  38 characters drawn at their own widths"), "{:?}", built.summary);
}

#[test]
fn more_characters_than_a_run_has_bytes_for_are_refused() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let out = folder.path().join("built.bin");
    // Latin letters with marks, Greek and Cyrillic, all 8 pixels wide in Unifont: 300 characters.
    let many: String =
        ('\u{0100}'..='\u{017F}').chain('\u{0391}'..='\u{03A1}').chain('\u{0410}'..='\u{047F}').collect();
    let refused = build(disc, &pack_of(&[(109, &many)]), &out, &BuildOptions::default()).err().unwrap().to_string();
    assert!(refused.contains("a run of them has a byte for 240"), "{refused}");
    assert!(!out.exists());
}
