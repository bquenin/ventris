//! The command line, run the way a user runs it.
//!
//! The `games` command and the refusals that need no game are tested everywhere. The rest needs a
//! game's image, the cartridge of Tales of Phantasia or Tales of Destiny 2's disc, which is looked
//! for as `ventris::testing` says; some of it also needs a font under
//! `work/fonts/`, GNU Unifont or Unscii. A test whose file is missing says so and passes.
//!
//! A test that builds a disc game writes an image as large as the disc, one at a time, in a folder
//! for temporary files that is removed with it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, PoisonError};

use ventris::adapters;
use ventris::platforms::snes;
use ventris::script::read_script;

/// What a run of the command line gave: its exit code, what it printed, and what it complained of.
struct Run {
    code: Option<i32>,
    out: String,
    err: String,
}

fn ventris(arguments: &[&str]) -> Run {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_ventris")).args(arguments).output().unwrap();
    Run {
        code: output.status.code(),
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
    }
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// A file that git does not keep, if it is there; a word about it if not.
fn local_file(path: &str, what: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    if !path.exists() {
        eprintln!("skipped: {what} is not available at {}", path.display());
        return None;
    }
    Some(path)
}

fn unifont() -> Option<PathBuf> {
    local_file("work/fonts/unifont_all-18.0.01.hex", "GNU Unifont")
}

/// The cartridge of Tales of Phantasia.
fn phantasia() -> Option<PathBuf> {
    ventris::testing::game("top-sfc")
}

/// An 8x8 font, for menu text.
fn unscii() -> Option<PathBuf> {
    local_file("work/fonts/unscii-8.hex", "Unscii")
}

const PHANTASIA: &str = "Tales of Phantasia (Super Famicom, Japan)";
const PHANTASIA_SHA1: &str = "fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0";

/// The disc of Tales of Destiny 2.
fn disc() -> Option<PathBuf> {
    ventris::testing::game("tod2-ps2")
}

const DISC_GAME: &str = "Tales of Destiny 2 (PlayStation 2, Japan)";
const DISC_SHA1: &str = "3b2be72b751607a7a2a56d62feec71584b4b8c72";

/// A pack of one file, in a folder of its own.
fn pack(folder: &Path, name: &str, file: &str, content: &str) -> PathBuf {
    let pack = folder.join(name);
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join(file), content).unwrap();
    pack
}

/// Run `insert` on a pack that must be refused, and return what the user is told.
fn refused(rom: &Path, pack: &Path, more: &[&str]) -> String {
    let output = pack.with_extension("sfc");
    let run = ventris(&[&["insert", text(rom), text(pack), "-o", text(&output)], more].concat());
    assert_eq!((run.code, run.out.as_str()), (Some(1), ""), "{}", run.err);
    assert!(!output.exists(), "a refused build left a file behind");
    run.err
}

#[test]
fn the_games_command_lists_every_adapter() {
    let run = ventris(&["games"]);
    assert_eq!(run.code, Some(0));
    for adapter in adapters::ALL {
        assert!(run.out.contains(adapter.name()) && run.out.contains(adapter.id()));
    }
    // The games come in the order the Python reference lists them, this one first.
    let entry = "Tales of Phantasia (Super Famicom, Japan)\n\
        \x20 id         top-sfc, for Super Famicom / SNES, in ja\n\
        \x20 image      fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0  Japan\n\
        \x20 dialogue   extract; insert, longer strings; new characters; lines fitted to the screen\n\
        \x20            limits: an area's longest string and its events must fit together in the 16,480 bytes of \
        memory the game has for them; the name of a place, shown on entering it, is centred on one line, which \
        holds about 200 dots, some 28 Latin letters, where the pack adds letters, with room for the longest name a \
        player can type, and 16 characters in the game's own, a party member's name counting as its letters and one \
        more\n\
        \x20 menu       extract; insert, longer strings, up to 26 characters; new characters\n\
        \x20            limits: a new character takes the tile of a kana that no menu string uses any more\n\
        \x20 captions   extract; insert, longer strings; new characters\n\
        \x20            limits: battle messages, the names of skills in kanji and the descriptions of items, in cells \
        16 dots wide, about 15 to a line; a character the game's font lacks is drawn from Unifont, two narrow ones to \
        a cell, so a piece that the game joins to another needs an even number of them; a caption that takes more \
        cells than the game's own may run into what is beside it\n\
        \x20 names      a pack can rename the characters\n\
        \x20            limits: a name is 1 to 15 characters, without tags; the menus hold names of up to 7, and \
        keep the game's own names when one is longer\n\
        \x20 fonts      new characters are drawn from unifont\n\
        \x20 menu font  new characters of the menus are drawn from unscii-8\n\
        \x20 option     renderer: install the variable-width renderer even if no new character needs it\n\
        \x20 option     cartridge: read the dialogue of every area the pack changes from the cartridge, even where \
        it would fit in memory\n\n\
        Tales of Destiny 2 (PlayStation 2, Japan)\n";
    assert!(run.out.replace("\r\n", "\n").starts_with(entry), "{}", run.out);
    let titles: Vec<&str> = run.out.lines().filter(|line| !line.is_empty() && !line.starts_with(' ')).collect();
    assert_eq!(titles, adapters::ALL.iter().map(|adapter| adapter.name()).collect::<Vec<_>>());
    let entry = "Tales of Destiny 2 (PlayStation 2, Japan)\n\
        \x20 id         tod2-ps2, for PlayStation 2, in ja\n\
        \x20 image      3b2be72b751607a7a2a56d62feec71584b4b8c72  Japan, version 1.01\n\
        \x20 dialogue   extract; insert, longer strings; new characters; lines fitted to the screen\n\
        \x20            limits: the text of one script cannot pass 65,535 bytes, and a string that nothing in its \
        script's code refers to cannot take more bytes than the game's own; the strings of program-script share the \
        588 bytes its text has, strings that are the same kept once\n\
        \x20 skits      extract; insert, longer strings; new characters\n\
        \x20            limits: a line is shown on one row, about 54 narrow letters or 27 wide ones, and a longer \
        one scrolls sideways while it is typed; the text of one skit cannot pass 65,535 bytes, and a string that \
        nothing in its script's code refers to cannot take more bytes than the game's own\n\
        \x20 menus      extract; insert, longer strings; new characters\n\
        \x20            limits: the strings a pack changes in one menu share the room they had between them, and a \
        string that finds no place there goes to the end of the menu's module, which may grow until it has 63,736 \
        bytes, as the largest has already; the strings of the quiz's, the battles' and the field's modules stay \
        where they are, each with its own bytes and the few unused ones after it; a wide character takes two bytes \
        and a narrow one takes one\n\
        \x20 lists      extract; insert, longer strings; new characters\n\
        \x20            limits: the strings a pack changes share the room they had between them, so one may take \
        more bytes where others take fewer, and a pack that changes one string alone can give it no more than its \
        own bytes and the few unused ones after it; 18 strings the program reaches in ways that cannot be followed \
        stay where they are, each with its own bytes; a wide character takes two bytes and a narrow one takes one\n\
        \x20 enemies    extract; insert, no longer than the game's own; new characters\n\
        \x20            limits: a name takes at most 22 bytes: 11 wide characters, or 22 narrow ones\n\
        \x20 minigames  extract; insert, no longer than the game's own; new characters\n\
        \x20            limits: a string stays where it is and the next one follows it at once, so it cannot take \
        more bytes than the game's own; a wide character takes two bytes and a narrow one takes one\n\
        \x20 fonts      new characters are drawn from dejavu-condensed-bold, then unifont\n\
        \x20 option     skit-pace: type each changed line of a skit fast enough to be read whole before the game \
        takes it away, and refuse a line too long for that\n\
        \x20 option     narrow-spell-names: draw a spell's name in the grids of artes (the Artes and Enchant screens) \
        in the narrow font a skill's is drawn in, with room for 17 letters where the game's large letters leave 9\n\n";
    assert!(run.out.replace("\r\n", "\n").contains(entry), "{}", run.out);
}

#[test]
fn a_file_that_is_no_game_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let notes = folder.path().join("notes.txt");
    fs::write(&notes, "not a game").unwrap();
    let run = ventris(&["extract", text(&notes), "-o", text(&folder.path().join("out"))]);
    assert_eq!((run.code, run.out.as_str()), (Some(1), ""));
    assert!(
        run.err.starts_with("error: ")
            && run.err.trim_end().ends_with("notes.txt is not a game ventris knows how to read yet")
    );
    assert!(!folder.path().join("out").exists());

    let run = ventris(&["font", text(&folder.path().join("missing.sfc")), "-o", text(&folder.path().join("font.png"))]);
    assert_eq!(run.code, Some(1));
    assert!(run.err.starts_with("error: ") && run.err.contains("missing.sfc"), "{}", run.err);
}

#[test]
fn a_game_with_menus_and_names_is_extracted_and_put_back() {
    let Some(rom) = phantasia() else { return };
    let folder = tempfile::tempdir().unwrap();
    let ja = folder.path().join("ja");
    let run = ventris(&["extract", text(&rom), "-o", text(&ja)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[..5],
        [
            format!("game:      {PHANTASIA}").as_str(),
            "dialogue:  3201 strings in 50 sections",
            "menu:      929 strings",
            "captions:  622 strings",
            // The captions use kanji that the dialogue never does, and the table does not have them yet.
            "unmapped:  178 characters using 86 codes that have no table entry yet",
        ]
    );
    assert!(
        lines[5].starts_with("wrote 52 files to ")
            && lines[5].ends_with(" and verified that they rebuild the original text")
    );
    let script = read_script(&ja).unwrap();
    assert_eq!(script.keys().filter(|section| section.len() == 2).count(), 50);
    assert_eq!(script["menu"][&4], "こうげき");
    // The characters' own names come out too, under a word about what the file is for.
    let names = fs::read_to_string(ja.join("names.txt")).unwrap();
    assert!(names.starts_with("# On the left, a character's name in the game"));
    assert!(names.ends_with(
        "クレス = クレス\nミント = ミント\nアーチェ = アーチェ\nチェスター = チェスター\nクラース = クラース\n"
    ));
    // The files have the same line ends on every system.
    assert!(!names.contains('\r') && !fs::read(ja.join("01.txt")).unwrap().contains(&b'\r'));

    // The script as extracted, names included, gives back the game it came from.
    let output = folder.path().join("rebuilt.sfc");
    let run = ventris(&["insert", text(&rom), text(&ja), "-o", text(&output)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[..3],
        [
            format!("game:      {PHANTASIA}").as_str(),
            "changed:   nothing, the script matches the game",
            format!("identical: yes, sha1 {PHANTASIA_SHA1}").as_str(),
        ]
    );
    assert!(fs::read(&output).unwrap() == ventris::testing::cartridge("top-sfc").unwrap());
}

#[test]
fn the_font_sheet_of_a_game_with_one_bit_glyphs_can_be_drawn() {
    let Some(rom) = phantasia() else { return };
    let folder = tempfile::tempdir().unwrap();
    let sheet = folder.path().join("font.png");
    let run = ventris(&["font", text(&rom), "-o", text(&sheet)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(run.out.trim_end().ends_with("font.png: 1601 glyphs, 32 per row, codes 0x05 to 0x0cc5"), "{}", run.out);
    let png = fs::read(&sheet).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    // 32 glyphs across and 51 down, each 16 pixels and a line of grey, drawn at twice the size.
    assert_eq!(png[16..24], [0, 0, 0x04, 0x40, 0, 0, 0x06, 0xC6]);
}

#[test]
fn insert_reports_dialogue_menus_names_and_the_renderer() {
    let Some(rom) = phantasia() else { return };
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    let output = path.join("out.sfc");

    // The renderer alone, which needs no font.
    let empty = path.join("empty");
    fs::create_dir(&empty).unwrap();
    let run = ventris(&["insert", text(&rom), text(&empty), "-o", text(&output), "--option", "renderer"]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[..5],
        [
            format!("game:      {PHANTASIA}").as_str(),
            "changed:   nothing, the script matches the game",
            "renderer:  variable-width renderer installed",
            "size:      expanded from 6 to 8 MiB",
            // No longer the image the Python reference built: the renderer now also centres the names
            // of places by their width in dots, and draws a number at the end of one whole.
            "identical: no, sha1 607de0e55d97a403f459a8cf97109ef8a03c5fb6",
        ]
    );
    let built = fs::read(&output).unwrap();
    let header = snes::detect_header(&built, snes::LIKELY_HEADER).unwrap();
    assert_eq!(header.checksum, snes::compute_checksum(&built));

    // Menu text that fits where it was, and a name in the game's own letters: no font is asked for.
    pack(path, "kana", "menu.txt", "[menu.004]\nぼうぎょ\n\n[menu.005]\nかいひ\n\n");
    let kana = pack(path, "kana", "names.txt", "# a line of comment\nクレス = クレスト\nミント = ミント\n");
    let run = ventris(&["insert", text(&rom), text(&kana), "-o", text(&output)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[1..7],
        [
            "changed:   nothing, the script matches the game",
            "menu:      3 menu strings changed",
            "names:     1 characters renamed (クレス)",
            "renderer:  variable-width renderer installed",
            "size:      expanded from 6 to 8 MiB",
            "identical: no, sha1 316588bda6f9074208d4cd9df72db2a06e6a1ff3",
        ]
    );

    // New letters in dialogue, as written and fitted to the window.
    let Some(font) = unifont() else { return };
    let line = "{クレス}\n「Dad, did you call me? Привет, мир! A sentence long enough to be broken in two.\n\n";
    let letters = pack(path, "letters", "05.txt", &format!("[05.045]\n{line}"));
    let arguments = ["insert", text(&rom), text(&letters), "-o", text(&output), "--font", text(&font)];
    let run = ventris(&arguments);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[1..7],
        [
            "changed:   1 dialogue blocks (05)",
            "moved:     1 outgrew their space and went to the expanded area (05)",
            "font:      33 characters the game's font lacks:  !,.?ADabcdeghiklmnorstuwyПвеимрт",
            "renderer:  variable-width renderer installed",
            "size:      expanded from 6 to 8 MiB",
            "identical: no, sha1 61103c2be6c9633cda032a56253b8b182900f06a",
        ]
    );
    let run = ventris(&[&arguments[..], &["--no-wrap"]].concat());
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(run.out.contains("identical: no, sha1 e9b439ae3046fa89a91f36d96e9a4463a705780b"), "{}", run.out);

    // A scratch pack kept outside the repository, under `work/top-sfc/menu-en`, builds one known
    // image; without it, this last check is skipped. The image is no longer the one the Python
    // reference built: the letters its menus gain now also go into the captions' font, so that an
    // item's name reads in letters where the game draws it large, and the letters its dialogue gains
    // go into a font for the lines the game shows as sprites. The hashes above moved for that second
    // reason too, and all of them for the renderer's handling of place names. And it names a character
    // in seven letters, one more than the game's longest name, so the status screen counts seven
    // letters of a name before it places the family name.
    let (Some(menu_font), Some(english)) = (unscii(), local_file("work/top-sfc/menu-en", "the trial pack")) else {
        return;
    };
    let fonts = ["--font", text(&font), "--menu-font", text(&menu_font)];
    let run = ventris(&[&["insert", text(&rom), text(&english), "-o", text(&output)], &fonts[..]].concat());
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(run.out.contains("identical: no, sha1 5a0c7a68df7882213dc32dc16780697aac5da099"), "{}", run.out);
}

#[test]
fn what_a_pack_of_dialogue_menus_and_names_must_not_do_is_refused_in_words() {
    let Some(rom) = phantasia() else { return };
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    let empty = path.join("empty");
    fs::create_dir(&empty).unwrap();
    let long_menu = format!("[menu.004]\n{}\n\n", "あ".repeat(27));
    let long_name = format!("クレス = {}\n", "C".repeat(16));
    let large = format!("[01.000]\n{}\n\n", "あ".repeat(20000));
    for (pack, more, message) in [
        (pack(path, "section", "shop.txt", "[shop.000]\nあ\n\n"), vec![], "shop is not a part of this game's script"),
        (pack(path, "events", "02.txt", "[02.000]\nあ\n\n"), vec![], "block 02 is not a dialogue block"),
        (
            pack(path, "past", "01.txt", "[01.9999]\nおまけ\n\n"),
            vec![],
            "string 01.9999 does not exist: block 01 has 14",
        ),
        (
            pack(path, "menu-past", "menu.txt", "[menu.9999]\nあ\n\n"),
            vec![],
            "string menu.9999 does not exist: there are 929",
        ),
        (
            pack(path, "tag", "01.txt", "[01.002]\nあ{BOGUS:01}\n\n"),
            vec![],
            "string 01.002: unknown or malformed tag {BOGUS:01}",
        ),
        (
            // The game's own font for its menus is drawn from; what stops this pack is that no tile is free.
            pack(path, "menu-letters", "menu.txt", "[menu.004]\nAtk\n\n"),
            vec![],
            "the menu font has room for 0 new characters and the pack needs 2 (kt); a tile only comes free once \
             every menu string that uses its kana is replaced",
        ),
        (
            pack(path, "menu-long", "menu.txt", &long_menu),
            vec![],
            "string menu.004 is 27 characters; the longest a menu string can be is 26",
        ),
        (
            pack(path, "stranger", "names.txt", "Dhaos = Dhaos\n"),
            vec![],
            "Dhaos is not a character whose name can be set (クレス, ミント, アーチェ, チェスター, クラース)",
        ),
        (
            pack(path, "name-long", "names.txt", &long_name),
            vec![],
            "the name for クレス must be 1 to 15 characters, without tags",
        ),
        (
            pack(path, "large", "01.txt", &large),
            vec![],
            "string 01.000 is 5576 bytes too long for the memory the game has left beside the events of its area",
        ),
        (
            empty,
            vec!["--option", "fast"],
            "Tales of Phantasia (Super Famicom, Japan) has no option named fast (it has: renderer, cartridge)",
        ),
        (
            pack(path, "broken", "01.txt", "[01.002]\nあ\n"),
            vec![],
            "01.txt: string 002 must be followed by a blank line",
        ),
    ] {
        assert_eq!(refused(&rom, &pack, &more).trim_end(), format!("error: {message}"));
    }
    let missing = refused(&rom, &path.join("no-such-pack"), &[]);
    assert!(
        missing.starts_with("error: ") && missing.trim_end().ends_with("no-such-pack is not a folder of script files")
    );
}

#[test]
fn the_adapter_of_a_game_with_menus_and_names_keeps_the_contract() {
    let Some(rom) = phantasia() else { return };
    let folder = tempfile::tempdir().unwrap();
    let run = ventris(&["check", text(&rom), "--work", text(folder.path())]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(
        run.out.lines().collect::<Vec<_>>(),
        [
            format!("game:      {PHANTASIA}").as_str(),
            "extract:   3201 strings of dialogue, 929 strings of menu, 622 strings of captions; they encode back to the \
             game's bytes",
            "unchanged: an empty pack and the whole script both rebuild the image byte for byte",
            "dialogue:  01.001 changed to a longer one; a string that does not exist is refused",
            "menu:      menu.004 changed to a longer one; a string that does not exist is refused",
            "captions:  captions.004 changed to a longer one; a string that does not exist is refused",
            "screen:    not checked; that takes an emulator and a person",
            "the adapter top-sfc keeps the contract, as far as a program can tell",
        ]
    );
    // Nothing of the game is left in the folder it was built in.
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);
}

/// Held by a test while it builds a disc game, so that only one image as large as a disc is being
/// written at any time.
static BUILDING_A_DISC: Mutex<()> = Mutex::new(());

#[test]
fn a_disc_game_is_extracted_and_its_script_put_back() {
    let Some(rom) = disc() else { return };
    let folder = tempfile::tempdir().unwrap();
    let ja = folder.path().join("ja");
    let run = ventris(&["extract", text(&rom), "-o", text(&ja)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[..8],
        [
            format!("game:      {DISC_GAME}").as_str(),
            "dialogue:  39946 strings in 462 sections",
            "skits:     3022 strings in 609 sections",
            "menus:     716 strings in 22 sections",
            "lists:     3259 strings",
            "enemies:   933 strings in 550 sections",
            "minigames: 5732 strings in 96 sections",
            "unmapped:  0 characters using 0 codes that have no table entry yet",
        ]
    );
    assert!(lines[8].starts_with("wrote 1740 files to "));
    assert_eq!(read_script(&ja).unwrap()["06307"][&4], "はやく、はやく！");
    assert!(!fs::read(ja.join("06307.txt")).unwrap().contains(&b'\r'));

    // The whole script put back gives the image it came from.
    let _one_at_a_time = BUILDING_A_DISC.lock().unwrap_or_else(PoisonError::into_inner);
    let output = folder.path().join("out.iso");
    let run = ventris(&["insert", text(&rom), text(&ja), "-o", text(&output)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines[..3],
        [
            format!("game:      {DISC_GAME}").as_str(),
            "dialogue:  0 strings changed in 0 scripts, 0 of them moved",
            format!("identical: yes, sha1 {DISC_SHA1}").as_str(),
        ]
    );
    assert_eq!(fs::metadata(&output).unwrap().len(), fs::metadata(&rom).unwrap().len());
}

#[test]
fn a_disc_game_s_font_sheet_can_be_drawn() {
    let Some(rom) = disc() else { return };
    let folder = tempfile::tempdir().unwrap();
    let sheet = folder.path().join("font.png");
    let run = ventris(&["font", text(&rom), "-o", text(&sheet)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(run.out.trim_end().ends_with("font.png: 2560 glyphs, 32 per row, codes 0x9940 to 0xe6c2"), "{}", run.out);
    // 32 glyphs across and 80 down, each 24 pixels and a line of grey, drawn at twice the size.
    assert_eq!(fs::read(&sheet).unwrap()[16..24], [0, 0, 0x06, 0x40, 0, 0, 0x0F, 0xA0]);
}

#[test]
fn what_a_disc_game_s_pack_must_not_do_is_refused_in_words() {
    let Some(rom) = disc() else { return };
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    let plain = pack(path, "plain", "06307.txt", "[06307.004]\nはやく！\n\n");
    pack(path, "named", "06307.txt", "[06307.004]\nはやく！\n\n");
    let named = pack(path, "named", "names.txt", "カイル = Kyle\n");
    let no_option = format!("{DISC_GAME} has no option named fast (it has: skit-pace, narrow-spell-names)");
    let no_names = format!("the pack gives names for characters, and {DISC_GAME} has none a pack can set yet");
    let no_menu_font = format!("{DISC_GAME} has no text that a menu font is used for");
    for (pack, more, message) in [
        (pack(path, "section", "99999.txt", "[99999.000]\nx\n\n"), vec![], "99999 is not a part of this game's script"),
        (
            pack(path, "past", "06307.txt", "[06307.091]\nあ\n\n"),
            vec![],
            "string 06307.091 does not exist: 06307 has 91 strings",
        ),
        (
            // An empty string that no code refers to: it has nowhere to go once it holds text.
            pack(path, "unmoved", "06307.txt", "[06307.069]\nあ\n\n"),
            vec![],
            "string 06307.069 takes 2 bytes where the game's has 0, and it cannot move: nothing in the script's code \
             refers to it",
        ),
        (
            pack(path, "tag", "06307.txt", "[06307.004]\n{NAME:01}\n\n"),
            vec![],
            "string 06307.004: {NAME:01} needs 4 parameter bytes",
        ),
        (plain.clone(), vec!["--option", "fast"], no_option.as_str()),
        (named, vec![], no_names.as_str()),
        (plain, vec!["--menu-font", "no-such-font.hex"], no_menu_font.as_str()),
    ] {
        assert_eq!(refused(&rom, &pack, &more).trim_end(), format!("error: {message}"));
    }
}

#[test]
fn the_adapter_of_a_disc_game_keeps_the_contract() {
    let Some(rom) = disc() else { return };
    let _one_at_a_time = BUILDING_A_DISC.lock().unwrap_or_else(PoisonError::into_inner);
    let folder = tempfile::tempdir().unwrap();
    let run = ventris(&["check", text(&rom), "--work", text(folder.path())]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(
        run.out.lines().collect::<Vec<_>>(),
        [
            format!("game:      {DISC_GAME}").as_str(),
            "extract:   39946 strings of dialogue, 3022 strings of skits, 716 strings of menus, 3259 strings of \
             lists, 933 strings of enemies, 5732 strings of minigames; they encode back to the game's bytes",
            "unchanged: an empty pack and the whole script both rebuild the image byte for byte",
            "dialogue:  06307.004 changed to a longer one; a string that does not exist is refused",
            "skits:     00058.001 changed to a longer one; a string that does not exist is refused",
            "menus:     06816.4136 changed to a longer one; a string that does not exist is refused",
            "lists:     program.1094168 changed to a longer one; a string that does not exist is refused",
            "enemies:   08211.001 changed; a string that does not exist is refused",
            "minigames: 06178.17786 changed; a string that does not exist is refused",
            "screen:    not checked; that takes an emulator and a person",
            "the adapter tod2-ps2 keeps the contract, as far as a program can tell",
        ]
    );
    // Nothing of the game is left in the folder it was built in: each image was closed before it was removed.
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);
}

/// What a pack's `pack.toml` holds for a pack of Tales of Phantasia, with whatever follows `more`.
fn manifest(more: &str) -> String {
    format!(
        "format = 1\ngame = \"top-sfc\"\nlanguage = \"en\"\nname = \"Trial\"\nversion = \"0.1\"\n\
         authors = [\"Ana\", \"Rui\"]\n{more}"
    )
}

#[test]
fn a_pack_is_described_and_checked_without_the_game() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    let whole = pack(path, "whole", "31.txt", "[31.001]\nHello!\n\n[31.004]\nBye!\n\n");
    fs::write(whole.join("33.txt"), "[33.000]\nGo!\n\n").unwrap();
    fs::create_dir(whole.join("fonts")).unwrap();
    fs::write(whole.join("fonts/letters.hex"), "").unwrap();
    fs::write(whole.join("pack.toml"), manifest("license = \"CC0-1.0\"\n[build]\nfonts = [\"fonts/letters.hex\"]\n"))
        .unwrap();
    let run = ventris(&["describe", text(&whole)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(
        run.out.lines().collect::<Vec<_>>(),
        [
            "pack:      Trial 0.1 (en)",
            format!("game:      {PHANTASIA}").as_str(),
            "authors:   Ana, Rui",
            "license:   CC0-1.0",
            "dialogue:  3 strings in 2 sections",
            "fonts:     fonts/letters.hex",
            "menu font: unscii-8, the game's own",
            "the pack is well formed; whether each string fits its place is known when it is built",
        ]
    );

    // A pack that names no font is drawn from its game's own, and one may name a built-in font.
    let plain = pack(path, "plain", "31.txt", "[31.001]\nHello!\n\n");
    for (build, fonts) in
        [("", "fonts:     unifont, the game's own"), ("[build]\nfonts = [\"unifont\"]\n", "fonts:     unifont")]
    {
        fs::write(plain.join("pack.toml"), manifest(build)).unwrap();
        let run = ventris(&["describe", text(&plain)]);
        assert_eq!(run.code, Some(0), "{}", run.err);
        assert_eq!(run.out.lines().nth(5), Some(fonts), "{}", run.out);
    }

    let refused = |name: &str, files: &[(&str, &str)]| {
        for (file, content) in files {
            pack(path, name, file, content);
        }
        let run = ventris(&["describe", text(&path.join(name))]);
        assert_eq!((run.code, run.out.as_str()), (Some(1), ""), "{}", run.err);
        run.err.trim_end().to_string()
    };
    let line = ("31.txt", "[31.001]\nGO!\n\n");
    let bare = refused("bare", &[line]);
    assert!(bare.ends_with("bare has no pack.toml, the file that says what the pack is"), "{bare}");
    for (name, files, message) in [
        (
            "elsewhere",
            vec![("pack.toml", manifest("").replace("top-sfc", "no-such-game"))],
            "pack.toml: the pack is for 'no-such-game', a game this version of ventris does not know",
        ),
        (
            "font",
            vec![("pack.toml", manifest("[build]\nfonts = [\"letters.hex\"]\n"))],
            "pack.toml: the font 'letters.hex' is neither a built-in font nor a file in the pack",
        ),
        (
            "option",
            vec![("pack.toml", manifest("[build]\noptions = [\"fast\"]\n"))],
            "Tales of Phantasia (Super Famicom, Japan) has no option named fast (it has: renderer, cartridge)",
        ),
        (
            // Tales of Destiny 2 is a game whose characters a pack cannot rename.
            "named",
            vec![
                ("pack.toml", manifest("").replace("top-sfc", "tod2-ps2")),
                ("names.txt", "カイル = Kyle\n".to_string()),
            ],
            "the pack gives names for characters, and Tales of Destiny 2 (PlayStation 2, Japan) has none a pack \
             can set yet",
        ),
        (
            "section",
            vec![("pack.toml", manifest("")), ("shop.txt", "[shop.000]\nx\n\n".to_string())],
            "shop is not a part of this game's script",
        ),
        (
            // A block of Tales of Phantasia's events, which holds no text.
            "events",
            vec![("pack.toml", manifest("")), ("02.txt", "[02.000]\nx\n\n".to_string())],
            "02 is not a part of this game's script",
        ),
        (
            // Past the last member of Tales of Destiny 2's archive.
            "past-the-archive",
            vec![
                ("pack.toml", manifest("").replace("top-sfc", "tod2-ps2")),
                ("99999.txt", "[99999.000]\nx\n\n".to_string()),
            ],
            "99999 is not a part of this game's script",
        ),
        (
            "broken",
            vec![("pack.toml", manifest("")), ("31.txt", "[31.001]\nGO!\n".to_string())],
            "31.txt: string 001 must be followed by a blank line",
        ),
        (
            "misspelt",
            vec![("pack.toml", manifest("lisence = \"MIT\"\n"))],
            "pack.toml: line 7: unknown field `lisence`",
        ),
    ] {
        let files: Vec<(&str, &str)> = files.iter().map(|(file, content)| (*file, content.as_str())).collect();
        let told = refused(name, &files);
        assert!(told.starts_with(&format!("error: {message}")), "{told}");
    }
    let missing = ventris(&["describe", text(&path.join("no-such-pack"))]);
    assert!(missing.err.trim_end().ends_with("no-such-pack is not a folder of script files"), "{}", missing.err);
}

#[test]
fn insert_builds_a_pack_with_what_its_manifest_says() {
    let (Some(rom), Some(font)) = (phantasia(), unifont()) else { return };
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    let line = "[05.045]\n{クレス}\n「Hello!\n\n";
    // The pack carries its font, so the build is given the game and the pack and nothing else.
    let hello = pack(path, "pack", "05.txt", line);
    fs::create_dir(hello.join("fonts")).unwrap();
    fs::copy(&font, hello.join("fonts/unifont.hex")).unwrap();
    fs::write(hello.join("pack.toml"), manifest("[build]\nfonts = [\"fonts/unifont.hex\"]\n")).unwrap();
    let output = path.join("out.sfc");
    let run = ventris(&["insert", text(&rom), text(&hello), "-o", text(&output)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(
        run.out.lines().take(3).collect::<Vec<_>>(),
        [format!("game:      {PHANTASIA}").as_str(), "pack:      Trial 0.1 (en)", "changed:   1 dialogue blocks (05)"]
    );
    // It is the image the same strings build when the font is given by hand.
    let plain = pack(path, "plain", "05.txt", line);
    let by_hand = path.join("by-hand.sfc");
    let run = ventris(&["insert", text(&rom), text(&plain), "-o", text(&by_hand), "--font", text(&font)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(fs::read(&output).unwrap() == fs::read(&by_hand).unwrap());

    // With no font named by the pack or the user, the game's own is drawn from, and it is the same font.
    let unaided = path.join("unaided.sfc");
    let run = ventris(&["insert", text(&rom), text(&plain), "-o", text(&unaided)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(fs::read(&unaided).unwrap() == fs::read(&by_hand).unwrap());
    // A built-in font can be given by its name.
    let by_name = path.join("by-name.sfc");
    let run = ventris(&["insert", text(&rom), text(&plain), "-o", text(&by_name), "--font", "unifont"]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert!(fs::read(&by_name).unwrap() == fs::read(&by_hand).unwrap());

    // A pack for another game is refused before anything is built.
    let other = pack(path, "other", "05.txt", line);
    fs::write(other.join("pack.toml"), manifest("").replace("top-sfc", "tod2-ps2")).unwrap();
    assert_eq!(
        refused(&rom, &other, &[]).trim_end(),
        format!("error: the pack is for {DISC_GAME}, and the game given is {PHANTASIA}")
    );
}

#[test]
fn the_built_in_fonts_are_listed_with_their_terms() {
    let run = ventris(&["licenses"]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let out = run.out.replace("\r\n", "\n");
    assert!(out.starts_with("The fonts built into ventris, each under its own terms:\n\n  unifont\n"), "{out}");
    for (name, words) in [
        ("unifont", "SIL OPEN FONT LICENSE Version 1.1"),
        ("dejavu-condensed-bold", "Bitstream Vera Fonts Copyright"),
        ("unscii-8", "in the public domain"),
    ] {
        let terms = out.split(&format!("==== {name} ====\n")).nth(1).unwrap_or_default();
        let terms = terms.split("\n==== ").next().unwrap_or_default();
        assert!(terms.contains(words), "{name}: {terms}");
    }
}

#[test]
fn a_folder_of_games_is_searched_for_the_ones_that_have_an_adapter() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path();
    // A file with a game's size that is not the game is read and passed over.
    fs::create_dir_all(path.join("snes")).unwrap();
    fs::write(path.join("snes").join("look-alike.sfc"), vec![9u8; 0x600000]).unwrap();
    fs::write(path.join("snes").join("too small.sfc"), vec![9u8; 0x8000]).unwrap();
    let run = ventris(&["find", text(path)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(run.out.trim_end(), "found 0 of the 2 games there are adapters for; 1 files had the size of one");

    let missing = path.join("nowhere");
    let run = ventris(&["find", text(&missing)]);
    assert_eq!(run.code, Some(1));
    assert_eq!(run.err.trim_end(), format!("error: {} is not a folder", missing.display()));

    // With the game at hand: it is found in an archive, in a folder inside the one given.
    let Some(rom) = ventris::testing::cartridge("top-sfc") else { return };
    let zipped = path.join("snes").join("Tales of Phantasia (Japan).zip");
    fs::write(&zipped, ventris::archive::make(&[("Tales of Phantasia (Japan).sfc", &rom, true)])).unwrap();
    let run = ventris(&["find", text(path)]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(
        lines,
        [
            PHANTASIA,
            "  id         top-sfc",
            "  release    Japan",
            format!("  file       {}", zipped.display()).as_str(),
            "found 1 of the 2 games there are adapters for; 2 files had the size of one",
        ]
    );
    // The archive is taken wherever a game's file is.
    let run = ventris(&["extract", text(&zipped), "-o", text(&path.join("script"))]);
    assert_eq!(run.code, Some(0), "{}", run.err);
    assert_eq!(run.out.lines().next(), Some(format!("game:      {PHANTASIA}").as_str()));
}
