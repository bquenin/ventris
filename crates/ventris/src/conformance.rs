//! Checks that an adapter keeps the contract in `adapter`, run against the game itself.
//!
//! These are the promises a program can check: the script survives a round trip, an unchanged
//! script rebuilds the image byte for byte, each kind of text that can be changed takes a changed
//! string, and what a pack must not do is refused. They are the same for every game, which is the
//! point: whoever adds an adapter does not write them.
//!
//! What they cannot tell is whether the changed text reads properly on screen. That takes a person
//! and an emulator.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::adapter::{Adapter, BuildOptions, declaration_faults};
use crate::error::{Error, Result};
use crate::platform::Image;
use crate::script::{Pack, Script, parse_name, read_pack, string_name, write_pack};

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition { Ok(()) } else { Err(Error::Conformance(message.into())) }
}

/// Check that a build was turned down with `Error::Text`, the error a translator is shown.
fn refuses(built: Result<String>, what: &str) -> Result<()> {
    match built {
        Err(Error::Text(_)) => Ok(()),
        Err(other) => Err(other),
        Ok(_) => Err(Error::Conformance(format!("a pack with {what} was built instead of being refused"))),
    }
}

/// A string of the script that a kind of text is tried on.
fn trial_string<'a>(texts: &'a Script, section: &str, number: u32) -> Result<&'a String> {
    let found = texts.get(section).and_then(|items| items.get(&number));
    found.ok_or_else(|| Error::Conformance(format!("the script has no string {} to try", string_name(section, number))))
}

/// A pack that holds one string.
fn one_string(section: &str, number: u32, text: &str) -> Pack {
    Pack::of(Script::from([(section.to_string(), BTreeMap::from([(number, text.to_string())]))]))
}

/// Run every check on an open image; `work` is a folder to build in. Returns what was checked, as
/// lines to show, and fails with `Error::Conformance` at the first thing that does not hold.
pub fn check(adapter: &dyn Adapter, image: &Image, sha1: &str, work: &Path) -> Result<Vec<String>> {
    let mut done = Vec::new();
    let faults = declaration_faults(adapter);
    require(faults.is_empty(), faults.join("; "))?;
    let known = adapter.images().iter().find(|known| known.sha1 == sha1);
    require(known.is_some(), format!("the image, {sha1}, is not one {} was worked out on", adapter.id()))?;
    // A folder of games is searched by size: an image whose size is given wrong would never be found.
    let (given, size) = (known.map_or(0, |release| release.size), image.size()?);
    require(given == size, format!("the adapter gives its image as {given} bytes, and it is {size}"))?;

    // --- extracting
    let game = adapter.extract(image)?;
    let texts = &game.texts;
    require(!texts.is_empty(), "nothing was extracted")?;
    let kinds = adapter.kinds();
    let mut counts = vec![0; kinds.len()];
    for (section, items) in texts {
        let kind = adapter.kind_of(section).and_then(|kind| kinds.iter().position(|known| std::ptr::eq(known, kind)));
        let Some(kind) = kind else {
            let message = format!("section {section} belongs to none of the adapter's kinds of text");
            return Err(Error::Conformance(message));
        };
        require(
            kinds[kind].numbered_by_place || items.keys().copied().eq(0..items.len() as u32),
            format!("the strings of section {section} are not numbered from 0 up"),
        )?;
        counts[kind] += items.len();
    }
    for (kind, count) in kinds.iter().zip(&counts) {
        require(*count > 0, format!("no section of the script is {}", kind.name))?;
    }
    require(
        game.names.is_empty() || adapter.sets_names(),
        "names were extracted, and the adapter says a pack cannot set any",
    )?;
    adapter.verify(image, texts)?;
    // Through the files and back, since that is the way a pack reaches a build.
    let files = work.join("script");
    write_pack(&files, &game)?;
    let read_back = read_pack(&files)?;
    require(read_back == game, "the script is not the same once written to files and read back")?;
    adapter.verify(image, &read_back.texts)?;
    let shown: Vec<String> =
        kinds.iter().zip(&counts).map(|(kind, count)| format!("{count} strings of {}", kind.name)).collect();
    done.push(format!("extract:   {}; they encode back to the game's bytes", shown.join(", ")));

    if !adapter.can_insert() {
        done.push("insert:    not checked, the adapter only extracts".to_string());
        return Ok(done);
    }

    // --- building
    let output = work.join("built");
    let checked = building(adapter, image, sha1, &read_back, &output, &mut done);
    // The image that was built is a copy of the game: it is not left behind, whatever happened.
    let _ = fs::remove_file(&output);
    checked?;
    done.push("screen:    not checked; that takes an emulator and a person".to_string());
    Ok(done)
}

/// The checks that build: `game` is the whole script, as read back from its files, and `output` the
/// file every build is written to. What was checked is added to `done`.
fn building(
    adapter: &dyn Adapter,
    image: &Image,
    sha1: &str,
    game: &Pack,
    output: &Path,
    done: &mut Vec<String>,
) -> Result<()> {
    let texts = &game.texts;
    // So that what is built is the pack's text and nothing else.
    let as_written = BuildOptions { wrap: false, ..BuildOptions::default() };
    // Build a pack and return the SHA-1 of the image it gave.
    let built = |pack: &Pack, options: &BuildOptions| -> Result<String> {
        adapter.build(image, pack, output, options)?;
        Ok(adapter.platform().open(output)?.1)
    };

    require(
        built(&Pack::default(), &as_written)? == sha1,
        "a pack that holds nothing does not rebuild the image it started from",
    )?;
    require(built(game, &BuildOptions::default())? == sha1, "the game's own script does not rebuild the image")?;
    done.push("unchanged: an empty pack and the whole script both rebuild the image byte for byte".to_string());

    for kind in adapter.kinds() {
        let Some((to, from)) = kind.trial.filter(|_| kind.insert) else { continue };
        let (section, number) = parse_name(to)?;
        let (from_section, from_number) = parse_name(from)?;
        let old = trial_string(texts, &section, number)?;
        let new = trial_string(texts, &from_section, from_number)?;
        require(new != old, format!("the two strings {} is tried on hold the same text", kind.name))?;
        if kind.grow {
            require(
                new.chars().count() > old.chars().count(),
                format!("{} can grow, and the string tried on it is not a longer one", kind.name),
            )?;
        }
        if let Some(most) = kind.max_length {
            require(new.chars().count() <= most, format!("the string tried on {} is over its own limit", kind.name))?;
        }
        require(
            built(&one_string(&section, number, new), &as_written)? != sha1,
            format!("changing {to} left the image as it was"),
        )?;
        // The number after the last one: no string has it, however the section is numbered.
        let past_the_end = texts[&section].keys().next_back().map_or(0, |last| last + 1);
        refuses(
            built(&one_string(&section, past_the_end, new), &as_written),
            &format!("a string past the end of section {section}"),
        )?;
        let grew = if kind.grow { " to a longer one" } else { "" };
        done.push(format!(
            "{:<10} {to} changed{grew}; a string that does not exist is refused",
            format!("{}:", kind.name)
        ));
    }

    refuses(built(&one_string("no-such-section", 0, "x"), &as_written), "a section the game does not have")?;
    let unknown = BuildOptions { switches: ["no-such-option".to_string()].into(), ..as_written.clone() };
    refuses(built(&Pack::default(), &unknown), "an unknown option")?;
    if !adapter.sets_names() {
        let named = Pack { texts: Script::new(), names: vec![("x".to_string(), "y".to_string())] };
        refuses(built(&named, &as_written), "names the game cannot set")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! The adapter contract: what every game's adapter declares, and the checks that hold it to that.
    //!
    //! A game made up here, a few lines of text in a file, stands in for a real one, so that the
    //! contract and its checks are tested without any game file.

    use sha1::{Digest, Sha1};

    use crate::adapter::{Built, Release, TextKind};
    use crate::error::text_error;
    use crate::platform::{Glance, Platform};
    use crate::script::changes;

    use super::*;

    /// A made-up console whose games are small files.
    struct Files;

    static FILES: Files = Files;

    fn sha1_of(data: &[u8]) -> String {
        Sha1::digest(data).iter().map(|byte| format!("{byte:02x}")).collect()
    }

    impl Platform for Files {
        fn name(&self) -> &'static str {
            "made up"
        }

        fn recognises(&self, path: &Path) -> Result<bool> {
            Ok(path.extension().is_some_and(|extension| extension == "toy"))
        }

        fn glance(&self, path: &Path) -> Result<Option<Glance>> {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(self.recognises(path)?.then_some(Glance { size: fs::metadata(path)?.len(), name }))
        }

        fn open(&self, path: &Path) -> Result<(Image, String)> {
            let data = fs::read(path)?;
            let sha1 = sha1_of(&data);
            Ok((Image::Cartridge(data), sha1))
        }
    }

    const TOY: &[u8] = b"one\ntwo words\nthree words here\n";
    const TOY_SHA1: &str = "0422f0e11acbd5151582d71f25fcf03853526f32";
    const TOY_SIZE: u64 = TOY.len() as u64;
    /// The last line is the menu; the ones before it are dialogue.
    const MENU_AT: usize = 2;

    fn is_talk(section: &str) -> bool {
        section == "talk"
    }

    fn is_menu(section: &str) -> bool {
        section == "menu"
    }

    fn is_either(section: &str) -> bool {
        is_talk(section) || is_menu(section)
    }

    static SOUND: [TextKind; 2] = [
        TextKind {
            insert: true,
            grow: true,
            trial: Some(("talk.000", "talk.001")),
            ..TextKind::new("dialogue", is_talk)
        },
        TextKind {
            insert: true,
            max_length: Some(16),
            trial: Some(("menu.000", "menu.001")),
            ..TextKind::new("menu", is_menu)
        },
    ];
    /// The same two kinds, the dialogue numbered by where its lines are, which leaves gaps.
    static BY_PLACE: [TextKind; 2] = [
        TextKind {
            insert: true,
            grow: true,
            numbered_by_place: true,
            trial: Some(("talk.000", "talk.005")),
            ..TextKind::new("dialogue", is_talk)
        },
        TextKind {
            insert: true,
            max_length: Some(16),
            trial: Some(("menu.000", "menu.001")),
            ..TextKind::new("menu", is_menu)
        },
    ];
    static READ_ONLY: [TextKind; 2] = [TextKind::new("dialogue", is_talk), TextKind::new("menu", is_menu)];
    static NO_TRIAL: [TextKind; 1] = [TextKind { insert: true, ..TextKind::new("dialogue", is_either) }];
    static TRIAL_ELSEWHERE: [TextKind; 2] = [
        TextKind { insert: true, trial: Some(("talk.000", "menu.001")), ..TextKind::new("dialogue", is_talk) },
        TextKind::new("menu", is_menu),
    ];
    static CLAIMS_TOO_MUCH: [TextKind; 1] = [TextKind { grow: true, ..TextKind::new("dialogue", is_either) }];
    static WRONG_TRIAL: [TextKind; 2] = [
        TextKind {
            insert: true,
            grow: true,
            trial: Some(("talk.001", "talk.000")),
            ..TextKind::new("dialogue", is_talk)
        },
        TextKind::new("menu", is_menu),
    ];

    /// A promise the made-up game's adapter can be made to break.
    #[derive(Clone, Copy, PartialEq)]
    enum Broken {
        Nothing,
        Lossy,
        StraySection,
        Gaps,
        TouchesEverything,
        IgnoresThePack,
        TakesAnything,
    }

    /// A game that is lines of text: the first two are dialogue and may grow, the last is a menu and
    /// may not.
    struct Toy {
        id: &'static str,
        images: &'static [Release],
        kinds: &'static [TextKind],
        fonts: &'static [&'static str],
        name_limits: &'static str,
        broken: Broken,
    }

    const SOUND_TOY: Toy = Toy {
        id: "toy-files",
        images: &[Release { sha1: TOY_SHA1, name: "the only one", size: TOY_SIZE }],
        kinds: &SOUND,
        fonts: &[],
        name_limits: "",
        broken: Broken::Nothing,
    };

    impl Toy {
        fn sections(image: &Image) -> Result<BTreeMap<String, Vec<String>>> {
            let text = String::from_utf8_lossy(image.cartridge()?).into_owned();
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            let menu = lines[MENU_AT..].iter().cloned().chain(["last".to_string()]).collect();
            Ok(BTreeMap::from([("talk".to_string(), lines[..MENU_AT].to_vec()), ("menu".to_string(), menu)]))
        }
    }

    impl Adapter for Toy {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            "A made-up game"
        }

        fn platform(&self) -> &'static dyn Platform {
            &FILES
        }

        fn images(&self) -> &'static [Release] {
            self.images
        }

        fn source_language(&self) -> &'static str {
            "en"
        }

        fn kinds(&self) -> &'static [TextKind] {
            self.kinds
        }

        fn fonts(&self) -> &'static [&'static str] {
            self.fonts
        }

        fn name_limits(&self) -> &'static str {
            self.name_limits
        }

        fn extract(&self, image: &Image) -> Result<Pack> {
            let mut texts = Script::new();
            for (section, lines) in Toy::sections(image)? {
                texts.insert(
                    section,
                    lines.into_iter().enumerate().map(|(number, line)| (number as u32, line)).collect(),
                );
            }
            match self.broken {
                Broken::StraySection => {
                    texts.insert("credits".to_string(), BTreeMap::from([(0, "x".to_string())]));
                }
                Broken::Gaps => {
                    let talk = texts.get_mut("talk").expect("the game has dialogue");
                    let second = talk.remove(&1).expect("the game has two lines of dialogue");
                    talk.insert(5, second);
                }
                _ => {}
            }
            Ok(Pack::of(texts))
        }

        fn verify(&self, image: &Image, texts: &Script) -> Result<()> {
            if self.broken == Broken::Lossy {
                return text_error("string talk.001 does not survive a round trip");
            }
            if *texts != self.extract(image)?.texts {
                return text_error("the script does not survive a round trip");
            }
            Ok(())
        }

        fn write(&self, image: &Image, pack: &Pack, output: &Path, _options: &BuildOptions) -> Result<Built> {
            let mut sections = Toy::sections(image)?;
            let mut texts = pack.texts.clone();
            if self.broken == Broken::TakesAnything {
                for (section, items) in &mut texts {
                    let known = sections.get(section).map_or(0, Vec::len);
                    items.retain(|number, _| (*number as usize) < known);
                }
            }
            if self.broken == Broken::Gaps {
                // The line it numbers 5 is the second one.
                let talk = texts.get_mut("talk");
                if let Some((talk, text)) = talk.and_then(|talk| talk.remove(&5).map(|text| (talk, text))) {
                    talk.insert(1, text);
                }
            }
            let found = changes(&texts, |section| sections.get(section).cloned())?;
            if self.broken == Broken::IgnoresThePack {
                fs::write(output, image.cartridge()?)?;
                return Ok(Built { summary: vec!["changed:   0 strings".to_string()] });
            }
            for (section, number, text) in &found {
                let own = &mut sections.get_mut(section).expect("changes() knows the section")[*number as usize];
                if section == "menu" && text.chars().count() > own.chars().count() {
                    return text_error(format!("menu.{number:03} cannot take more characters than the game's own"));
                }
                *own = text.clone();
            }
            let menu = &sections["menu"];
            let lines = sections["talk"].iter().chain(&menu[..menu.len() - 1]);
            let mut written: String = lines.map(|line| format!("{line}\n")).collect();
            if self.broken == Broken::TouchesEverything {
                written.push('\n');
            }
            fs::write(output, written)?;
            Ok(Built { summary: vec![format!("changed:   {} strings", found.len())] })
        }

        fn glyph_size(&self) -> usize {
            1
        }

        fn glyph_count(&self) -> usize {
            1
        }

        fn glyph(&self, _image: &Image, _index: usize) -> Result<Vec<Vec<u8>>> {
            Ok(vec![vec![1]])
        }

        fn code_at(&self, index: usize) -> u32 {
            index as u32
        }

        fn unmapped(&self, _text: &str) -> Vec<String> {
            Vec::new()
        }
    }

    fn checked(adapter: &Toy, image: &[u8]) -> Result<Vec<String>> {
        let work = tempfile::tempdir().unwrap();
        let lines = check(adapter, &Image::Cartridge(image.to_vec()), &sha1_of(image), work.path());
        assert!(!work.path().join("built").exists());
        lines
    }

    #[test]
    fn the_made_up_game_is_named_by_its_sha1() {
        assert_eq!(sha1_of(TOY), TOY_SHA1);
    }

    #[test]
    fn a_kind_numbered_by_place_may_have_gaps() {
        // The dialogue's two lines are numbered 0 and 5. That is refused of a kind numbered from 0
        // up, and is what a kind numbered by place is for.
        let by_place = Toy { kinds: &BY_PLACE, broken: Broken::Gaps, ..SOUND_TOY };
        assert_eq!(declaration_faults(&by_place), Vec::<String>::new());
        let lines = checked(&by_place, TOY).unwrap();
        assert_eq!(
            lines[0],
            "extract:   2 strings of dialogue, 2 strings of menu; they encode back to the game's bytes"
        );
        // The string tried is put where another's text is, and the number after the last is refused.
        assert_eq!(lines[2], "dialogue:  talk.000 changed to a longer one; a string that does not exist is refused");
    }

    #[test]
    fn a_sound_adapter_passes_the_checks() {
        assert_eq!(declaration_faults(&SOUND_TOY), Vec::<String>::new());
        let lines = checked(&SOUND_TOY, TOY).unwrap();
        assert_eq!(
            lines,
            [
                "extract:   2 strings of dialogue, 2 strings of menu; they encode back to the game's bytes",
                "unchanged: an empty pack and the whole script both rebuild the image byte for byte",
                "dialogue:  talk.000 changed to a longer one; a string that does not exist is refused",
                "menu:      menu.000 changed; a string that does not exist is refused",
                "screen:    not checked; that takes an emulator and a person",
            ]
        );
    }

    #[test]
    fn an_adapter_that_only_extracts_is_a_whole_one() {
        let read_only = Toy { kinds: &READ_ONLY, ..SOUND_TOY };
        assert_eq!(declaration_faults(&read_only), Vec::<String>::new());
        let lines = checked(&read_only, TOY).unwrap();
        assert_eq!(lines.last().unwrap(), "insert:    not checked, the adapter only extracts");
        let folder = tempfile::tempdir().unwrap();
        let image = Image::Cartridge(TOY.to_vec());
        let refused = read_only.build(&image, &Pack::default(), &folder.path().join("out"), &BuildOptions::default());
        let Err(Error::Text(message)) = refused else { panic!("it was built") };
        assert_eq!(message, "A made-up game can be extracted but not rebuilt yet");
    }

    #[test]
    fn what_an_adapter_says_about_itself_is_checked() {
        for (broken, fault) in [
            (Toy { id: "Toy Game", ..SOUND_TOY }, "is not lowercase words joined by hyphens"),
            (
                Toy { images: &[Release { sha1: "abc", name: "not a SHA-1", size: 1 }], ..SOUND_TOY },
                "its images are not named by SHA-1",
            ),
            (
                Toy { images: &[Release { sha1: TOY_SHA1, name: "no size", size: 0 }], ..SOUND_TOY },
                "it does not say how large its images are",
            ),
            (Toy { kinds: &NO_TRIAL, ..SOUND_TOY }, "names no two strings to try it on"),
            (Toy { kinds: &TRIAL_ELSEWHERE, ..SOUND_TOY }, "are not two of its own"),
            (
                Toy { fonts: &["unifont", "no-such-font"], ..SOUND_TOY },
                "the font it draws from, 'no-such-font', is not one built into the program",
            ),
            (
                Toy { kinds: &CLAIMS_TOO_MUCH, ..SOUND_TOY },
                "cannot be changed, yet says how a changed string is treated",
            ),
            (
                Toy { name_limits: "a name is short", ..SOUND_TOY },
                "it gives limits on its characters' names, and a pack cannot set any",
            ),
        ] {
            let faults = declaration_faults(&broken);
            assert!(faults.iter().any(|found| found.contains(fault)), "{fault}: {faults:?}");
            // And the checks stop there, before looking at the game.
            assert!(matches!(checked(&broken, TOY), Err(Error::Conformance(message)) if message.contains(fault)));
        }
    }

    #[test]
    fn the_checks_catch_an_adapter_that_breaks_a_promise() {
        let lossy = checked(&Toy { broken: Broken::Lossy, ..SOUND_TOY }, TOY).unwrap_err();
        assert!(matches!(lossy, Error::Text(_)));
        assert_eq!(lossy.to_string(), "string talk.001 does not survive a round trip");

        for (broken, fault) in [
            (
                Toy { broken: Broken::StraySection, ..SOUND_TOY },
                "section credits belongs to none of the adapter's kinds of text",
            ),
            (Toy { broken: Broken::Gaps, ..SOUND_TOY }, "the strings of section talk are not numbered from 0 up"),
            (
                Toy { broken: Broken::TouchesEverything, ..SOUND_TOY },
                "a pack that holds nothing does not rebuild the image it started from",
            ),
            (Toy { broken: Broken::IgnoresThePack, ..SOUND_TOY }, "changing talk.000 left the image as it was"),
            (
                Toy { broken: Broken::TakesAnything, ..SOUND_TOY },
                "a pack with a string past the end of section talk was built instead of being refused",
            ),
            (
                Toy { kinds: &WRONG_TRIAL, ..SOUND_TOY },
                "dialogue can grow, and the string tried on it is not a longer one",
            ),
        ] {
            let error = checked(&broken, TOY).unwrap_err();
            assert!(matches!(error, Error::Conformance(_)), "{fault}: {error:?}");
            assert_eq!(error.to_string(), fault);
        }

        let patched = [TOY, b"patched\n"].concat();
        assert_eq!(
            checked(&SOUND_TOY, &patched).unwrap_err().to_string(),
            "the image, 7922239fe5ada5fc6a9914d85ff79235d91d7ca2, is not one toy-files was worked out on"
        );
    }

    #[test]
    fn a_build_refuses_what_the_adapter_does_not_offer() {
        let folder = tempfile::tempdir().unwrap();
        let out = folder.path().join("out");
        let image = Image::Cartridge(TOY.to_vec());
        let options = BuildOptions::default();
        let built = SOUND_TOY.build(&image, &one_string("talk", 0, "ONE"), &out, &options).unwrap();
        assert_eq!(built.summary, ["changed:   1 strings"]);
        assert_eq!(fs::read(&out).unwrap(), b"ONE\ntwo words\nthree words here\n");

        let refused = |pack: &Pack, options: &BuildOptions| match SOUND_TOY.build(&image, pack, &out, options) {
            Err(Error::Text(message)) => message,
            _ => panic!("it was not refused in words for a translator"),
        };
        let fast = BuildOptions { switches: ["fast".to_string()].into(), ..BuildOptions::default() };
        assert_eq!(refused(&Pack::default(), &fast), "A made-up game has no option named fast (it has: none)");
        let named = Pack { texts: Script::new(), names: vec![("Hero".to_string(), "Héros".to_string())] };
        assert_eq!(
            refused(&named, &options),
            "the pack gives names for characters, and A made-up game has none a pack can set yet"
        );
        let small = BuildOptions { menu_font: Some(folder.path().join("small.hex").into()), ..BuildOptions::default() };
        assert_eq!(refused(&Pack::default(), &small), "A made-up game has no text that a menu font is used for");
        assert_eq!(
            refused(&one_string("menu", 0, "three words here and more"), &options),
            "menu.000 cannot take more characters than the game's own"
        );
    }
}
