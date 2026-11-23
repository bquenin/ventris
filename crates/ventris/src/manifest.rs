//! A pack's manifest: `pack.toml`, the file that says what a pack is.
//!
//! A folder of script files is enough to build from, and that is how a pack starts. To be passed
//! on it has to say more: which game it is for, what language it is in, who made it, and what a
//! build of it needs, so that whoever receives it gives the game's file and nothing else.
//!
//! ```toml
//! format = 1
//! game = "tod2-ps2"
//! language = "en"
//! name = "Tales of Destiny 2 in English"
//! version = "0.3"
//! authors = ["Someone"]
//! license = "CC-BY-SA-4.0"
//!
//! ```
//!
//! `format` is the version of this layout. Whatever the layout gains raises it, and it is read
//! before anything else: a pack written in a later layout is refused as that, and not for a key
//! this code does not know. Format 2 added `[layout]`; a pack in format 1 is read as before.
//! `game` is the id of the game's adapter, which is permanent.
//!
//! A pack need name no font: a build draws new characters from the fonts its game's adapter names,
//! which are built into the program. One that wants others says so under `[build]`:
//!
//! ```toml
//! [build]
//! fonts = ["fonts/letters.bdf", "unifont"]
//! ```
//!
//! Each is the name of a built-in font, or a file inside the pack, named from its folder: a pack is
//! read from one folder and can name no file outside it, since it may come from anyone.
//!
//! A game gives each label a place of fixed width, and a translation's word is often wider. Where
//! the game's adapter can move labels, a pack says which and how far under `[layout]`, by the
//! name the extracted script gives each, in columns to the right and rows down:
//!
//! ```toml
//! [layout]
//! "menu.659#3" = { columns = -2 }
//! ```
//!
//! A key that is not known is refused, so that a misspelt one is not silently ignored.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use serde::Deserialize;

use crate::adapter::{Adapter, BuildOptions};
use crate::adapters;
use crate::error::{Error, Result, at};
use crate::fonts::{self, Font};
use crate::text::quoted;

pub const MANIFEST_FILE: &str = "pack.toml";
/// The version of the manifest's layout that this code reads.
pub const FORMAT: u32 = 2;
/// The format that first had `[layout]`.
const LAYOUT_FORMAT: u32 = 2;

/// What `pack.toml` holds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The version of this layout.
    pub format: u32,
    /// The id of the adapter of the game the pack is for: "tod2-ps2".
    pub game: String,
    /// The language the pack is in, as an IETF tag: "en", "pt-BR".
    pub language: String,
    /// What the pack is called, as shown to someone choosing one.
    pub name: String,
    /// Which state of the pack this is; its authors decide what it looks like.
    pub version: String,
    pub authors: Vec<String>,
    /// The terms the pack is passed on under, as an SPDX name where there is one.
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Where the pack's authors can be found.
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub build: Build,
    /// Labels to draw somewhere else than the game does, by the name the game's adapter gives each.
    #[serde(default)]
    pub layout: BTreeMap<String, Move>,
}

/// How far to move a label: columns to the right and rows down. Either may be negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Move {
    #[serde(default)]
    pub columns: i32,
    #[serde(default)]
    pub rows: i32,
}

/// What a build of the pack needs besides the game: what a user would otherwise have to pass.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// Fonts for the characters the game's own font lacks, tried in this order, in place of the
    /// ones the game's adapter names: each a built-in font's name, or a file inside the pack.
    #[serde(default)]
    pub fonts: Vec<String>,
    /// The same for text drawn in small tiles, where the game has such text.
    #[serde(default)]
    pub menu_font: Option<String>,
    /// The game's own build options that the pack needs turned on.
    #[serde(default)]
    pub options: Vec<String>,
    /// False when the pack's line breaks are to be kept exactly as written.
    #[serde(default)]
    pub wrap: Option<bool>,
}

fn manifest_error<T>(message: impl AsRef<str>) -> Result<T> {
    Err(Error::Script(format!("{MANIFEST_FILE}: {}", message.as_ref())))
}

/// Whether a string has the shape of an IETF language tag: a language of two or three lowercase
/// letters, then any number of parts of up to eight letters or digits, joined by hyphens.
fn is_language(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    (2..=3).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_lowercase())
        && parts.all(|part| (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Whether a file is named the way a pack names its own: from the pack's folder, with `/` between
/// folders, and never leaving it.
fn is_inside(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains(':')
        && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}

/// What the parser refuses, in one line with the number of the line it is on: its own report
/// quotes the file over several lines, and a script file's errors are shown in one.
fn refused_by_the_parser(content: &str, error: &toml::de::Error) -> Error {
    let line = error.span().map(|span| content[..span.start.min(content.len())].matches('\n').count() + 1);
    let place = line.map_or(String::new(), |line| format!("line {line}: "));
    Error::Script(format!("{MANIFEST_FILE}: {place}{}", error.message()))
}

/// Read a manifest from its text.
pub fn parse(content: &str) -> Result<Manifest> {
    // The format is read first and on its own, with every key allowed. A later format may have keys
    // this code does not know, and a pack written in it is to be told so, not that a key is wrong.
    let keys: toml::Table = toml::from_str(content).map_err(|error| refused_by_the_parser(content, &error))?;
    if let Some(format) = keys.get("format").and_then(toml::Value::as_integer)
        && !(1..=i64::from(FORMAT)).contains(&format)
    {
        return manifest_error(format!(
            "it is written in format {format}, and this version of ventris reads up to format {FORMAT}"
        ));
    }
    let manifest: Manifest = toml::from_str(content).map_err(|error| refused_by_the_parser(content, &error))?;
    // An older ventris must be able to tell that it cannot build this pack, which it does by
    // the format. So a pack that uses what a format added has to say that format.
    if !manifest.layout.is_empty() && manifest.format < LAYOUT_FORMAT {
        return manifest_error(format!(
            "it moves labels under [layout], which a pack can do from format {LAYOUT_FORMAT}: write format = \
             {LAYOUT_FORMAT}"
        ));
    }
    if !is_language(&manifest.language) {
        return manifest_error(format!("{} is not a language tag such as en or pt-BR", quoted(&manifest.language)));
    }
    for (key, value) in [("game", &manifest.game), ("name", &manifest.name), ("version", &manifest.version)] {
        if value.trim().is_empty() {
            return manifest_error(format!("{key} is empty"));
        }
    }
    if manifest.authors.is_empty() || manifest.authors.iter().any(|author| author.trim().is_empty()) {
        return manifest_error("authors must name at least one author");
    }
    for font in manifest.build.fonts.iter().chain(&manifest.build.menu_font) {
        if fonts::built_in(font).is_none() && !is_inside(font) {
            return manifest_error(format!(
                "the font {} must be a built-in font or a file inside the pack, named from the pack's folder",
                quoted(font)
            ));
        }
    }
    Ok(manifest)
}

/// Read a pack's manifest, if it has one.
pub fn read(folder: &Path) -> Result<Option<Manifest>> {
    let path = folder.join(MANIFEST_FILE);
    if !path.exists() {
        return Ok(None);
    }
    parse(&fs::read_to_string(&path).map_err(at(&path))?).map(Some)
}

impl Manifest {
    /// The adapter of the game the pack is for.
    pub fn adapter(&self) -> Result<&'static dyn Adapter> {
        adapters::by_id(&self.game).ok_or_else(|| {
            Error::Script(format!(
                "{MANIFEST_FILE}: the pack is for {}, a game this version of ventris does not know",
                quoted(&self.game)
            ))
        })
    }

    /// Fail unless the pack is for this adapter's game.
    pub fn check_game(&self, adapter: &dyn Adapter) -> Result<()> {
        if self.game == adapter.id() {
            return Ok(());
        }
        let own = adapters::by_id(&self.game).map_or_else(|| quoted(&self.game), |own| own.name().to_string());
        Err(Error::Text(format!("the pack is for {own}, and the game given is {}", adapter.name())))
    }

    /// A font the manifest names: the built-in font of that name, or else one of the pack's own
    /// files, which must be there.
    fn font(&self, folder: &Path, name: &str) -> Result<Font> {
        if let Some(font) = fonts::built_in(name) {
            return Ok(Font::BuiltIn(font));
        }
        let path = folder.join(name);
        if !path.is_file() {
            return manifest_error(format!(
                "the font {} is neither a built-in font nor a file in the pack",
                quoted(name)
            ));
        }
        Ok(Font::File(path))
    }

    /// What to build the pack with: what its manifest says, under what the user asked for.
    ///
    /// A font the user gives takes the place of the pack's, since they said which to use. An option
    /// is on when either turns it on, and lines are kept as written when either says so.
    pub fn build_options(&self, folder: &Path, asked: &BuildOptions) -> Result<BuildOptions> {
        let mut options = asked.clone();
        if options.fonts.is_empty() {
            options.fonts = self.build.fonts.iter().map(|font| self.font(folder, font)).collect::<Result<_>>()?;
        }
        if options.menu_font.is_none() {
            options.menu_font = self.build.menu_font.as_ref().map(|font| self.font(folder, font)).transpose()?;
        }
        options.switches.extend(self.build.options.iter().cloned());
        for (label, by) in &self.layout {
            options.layout.entry(label.clone()).or_insert((by.columns, by.rows));
        }
        options.wrap = asked.wrap && self.build.wrap.unwrap_or(true);
        Ok(options)
    }

    /// The pack in a few words, for a report: its name, its version and its language.
    pub fn title(&self) -> String {
        format!("{} {} ({})", self.name, self.version, self.language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: &str = r#"
format = 1
game = "tod2-ps2"
language = "pt-BR"
name = "Tales of Destiny 2 em português"
version = "0.3"
authors = ["Ana", "Rui"]
license = "CC-BY-SA-4.0"
description = "The story and the menus."
homepage = "https://example.org/pack"

[build]
fonts = ["fonts/letters.bdf", "fonts/unifont.hex.gz"]
menu_font = "fonts/small.hex"
options = ["renderer"]
wrap = false
"#;
    const LEAST: &str = "format = 1\ngame = \"top-sfc\"\nlanguage = \"en\"\nname = \"Trial\"\nversion = \"1\"\n\
                         authors = [\"Ana\"]\n";

    fn message(content: &str) -> String {
        parse(content).unwrap_err().to_string()
    }

    #[test]
    fn a_manifest_is_read() {
        let manifest = parse(WHOLE).unwrap();
        assert_eq!((manifest.game.as_str(), manifest.language.as_str()), ("tod2-ps2", "pt-BR"));
        assert_eq!(manifest.authors, ["Ana", "Rui"]);
        assert_eq!(manifest.license.as_deref(), Some("CC-BY-SA-4.0"));
        assert_eq!(manifest.build.fonts, ["fonts/letters.bdf", "fonts/unifont.hex.gz"]);
        assert_eq!(manifest.build.wrap, Some(false));
        assert_eq!(manifest.title(), "Tales of Destiny 2 em português 0.3 (pt-BR)");

        let least = parse(LEAST).unwrap();
        assert_eq!(least.build, Build::default());
        assert!(least.layout.is_empty());

        // Labels to move, each by columns to the right and rows down; what is left out is nought.
        let moved = format!(
            "{}[layout]\n\"menu.659#3\" = {{ columns = -2 }}\n\"menu.008#2\" = {{ columns = 1, rows = -1 }}\n",
            LEAST.replace("format = 1", "format = 2")
        );
        let manifest = parse(&moved).unwrap();
        assert_eq!(manifest.layout["menu.659#3"], Move { columns: -2, rows: 0 });
        assert_eq!(manifest.layout["menu.008#2"], Move { columns: 1, rows: -1 });
        let options = manifest.build_options(Path::new("."), &BuildOptions::default()).unwrap();
        assert_eq!(options.layout["menu.659#3"], (-2, 0));
        assert!(message(&moved.replace("columns = -2", "colums = -2")).contains("unknown field `colums`"));
        assert!(least.license.is_none() && least.description.is_none() && least.homepage.is_none());
    }

    #[test]
    fn a_manifest_that_is_wrong_is_refused_in_words() {
        for (content, expected) in [
            (
                LEAST.replace("format = 1", "format = 3"),
                "it is written in format 3, and this version of ventris reads up to format 2",
            ),
            // A later format is told as that even where it has a key this code does not know.
            (
                format!("{}subtitle = \"x\"\n[extras]\nsize = 2\n", LEAST.replace("format = 1", "format = 4")),
                "it is written in format 4, and this version of ventris reads up to format 2",
            ),
            // A pack that uses what format 2 added has to say format 2.
            (
                format!("{LEAST}[layout]\n\"menu.659#3\" = {{ columns = -2 }}\n"),
                "it moves labels under [layout], which a pack can do from format 2: write format = 2",
            ),
            (LEAST.replace("\"en\"", "\"English\""), "'English' is not a language tag such as en or pt-BR"),
            (LEAST.replace("\"Trial\"", "\" \""), "name is empty"),
            (LEAST.replace("[\"Ana\"]", "[]"), "authors must name at least one author"),
            (
                format!("{LEAST}[build]\nfonts = [\"../font.hex\"]\n"),
                "the font '../font.hex' must be a built-in font or a file inside the pack, named from the pack's \
                 folder",
            ),
            (
                format!("{LEAST}[build]\nmenu_font = \"C:/fonts/small.hex\"\n"),
                "the font 'C:/fonts/small.hex' must be a built-in font or a file inside the pack, named from the \
                 pack's folder",
            ),
        ] {
            assert_eq!(message(&content), format!("pack.toml: {expected}"));
        }
        // A format that is not a number is the parser's to refuse, as a key that is missing is.
        assert!(message(&LEAST.replace("format = 1", "format = \"1\"")).starts_with("pack.toml: line 1: "));
        // What the parser itself refuses is told with the line it is on.
        assert!(message(&LEAST.replace("game = \"top-sfc\"\n", "")).contains("missing field `game`"));
        let misspelt = message(&format!("{LEAST}lisence = \"MIT\"\n"));
        assert!(misspelt.starts_with("pack.toml: line 7: unknown field `lisence`"), "{misspelt}");
        assert!(message("format = \n").starts_with("pack.toml: line 1: "));
    }

    #[test]
    fn a_file_a_pack_names_stays_inside_it() {
        for inside in ["font.hex", "fonts/letters.bdf", "fonts/a/b.hex.gz"] {
            assert!(is_inside(inside), "{inside}");
        }
        for outside in
            ["", "/font.hex", "../font.hex", "fonts/../../font.hex", "./font.hex", "fonts\\a.hex", "C:font.hex"]
        {
            assert!(!is_inside(outside), "{outside}");
        }
        for tag in ["en", "ja", "fil", "pt-BR", "zh-Hant", "sr-Latn-RS"] {
            assert!(is_language(tag), "{tag}");
        }
        for tag in ["", "e", "EN", "english", "en-", "en_US", "en-toolongsubtag"] {
            assert!(!is_language(tag), "{tag}");
        }
    }

    #[test]
    fn a_pack_is_for_one_game() {
        let manifest = parse(LEAST).unwrap();
        let own = manifest.adapter().unwrap();
        assert_eq!(own.id(), "top-sfc");
        manifest.check_game(own).unwrap();
        let other = adapters::by_id("tod2-ps2").unwrap();
        assert_eq!(
            manifest.check_game(other).unwrap_err().to_string(),
            format!("the pack is for {}, and the game given is {}", own.name(), other.name())
        );
        let unknown = parse(&LEAST.replace("top-sfc", "no-such-game")).unwrap();
        assert_eq!(
            unknown.adapter().err().unwrap().to_string(),
            "pack.toml: the pack is for 'no-such-game', a game this version of ventris does not know"
        );
    }

    #[test]
    fn a_build_takes_what_the_manifest_says_under_what_the_user_asked() {
        let folder = tempfile::tempdir().unwrap();
        let pack = folder.path();
        fs::write(pack.join(MANIFEST_FILE), WHOLE).unwrap();
        let manifest = read(pack).unwrap().unwrap();
        // A font the manifest names has to be there.
        let missing = manifest.build_options(pack, &BuildOptions::default()).unwrap_err().to_string();
        assert_eq!(
            missing,
            "pack.toml: the font 'fonts/letters.bdf' is neither a built-in font nor a file in the pack"
        );
        fs::create_dir(pack.join("fonts")).unwrap();
        for font in ["letters.bdf", "unifont.hex.gz", "small.hex"] {
            fs::write(pack.join("fonts").join(font), "").unwrap();
        }

        let options = manifest.build_options(pack, &BuildOptions::default()).unwrap();
        let file = |name: &str| Font::File(pack.join(name));
        assert_eq!(options.fonts, [file("fonts/letters.bdf"), file("fonts/unifont.hex.gz")]);
        assert_eq!(options.menu_font, Some(file("fonts/small.hex")));
        assert!(options.switches.contains("renderer") && !options.wrap);

        // What the user gives takes the place of the pack's fonts, and adds to its options.
        let asked = BuildOptions {
            fonts: vec![Font::named("mine.hex")],
            menu_font: Some(Font::named("small.hex")),
            switches: ["fast".to_string()].into(),
            wrap: true,
            ..BuildOptions::default()
        };
        let options = manifest.build_options(pack, &asked).unwrap();
        assert_eq!((options.fonts, options.menu_font), (asked.fonts.clone(), asked.menu_font.clone()));
        assert_eq!(options.switches.iter().collect::<Vec<_>>(), ["fast", "renderer"]);

        // A font the program has built in is named by its name, and needs no file in the pack.
        let built_in = parse(&format!("{LEAST}[build]\nfonts = [\"fonts/letters.bdf\", \"unifont\"]\n")).unwrap();
        let options = built_in.build_options(pack, &BuildOptions::default()).unwrap();
        assert_eq!(options.fonts, [file("fonts/letters.bdf"), Font::named("unifont")]);
        assert!(matches!(options.fonts[1], Font::BuiltIn(_)));

        // A folder with no manifest is still a pack, and one that asks for nothing changes nothing.
        assert!(read(&pack.join("fonts")).unwrap().is_none());
        let options = parse(LEAST).unwrap().build_options(pack, &BuildOptions::default()).unwrap();
        assert!(options.fonts.is_empty() && options.menu_font.is_none() && options.switches.is_empty() && options.wrap);
    }
}
