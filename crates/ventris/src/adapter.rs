//! The contract between the engine and the code for one game.
//!
//! An adapter is everything ventris knows about one game: where its text is, how it is encoded,
//! and how to put a changed script back. It is written once, by someone who has taken the game
//! apart, and it is compiled into ventris: a new game arrives as a pull request that adds a
//! module under `adapters` and one line to its list. A translation pack is the other half: plain
//! text that names strings by their address, written by anyone, for any language, with no code in
//! it. The two meet only here.
//!
//! So that a pack written today still works with a later ventris, an adapter keeps three
//! promises:
//!
//! - **Addresses are permanent.** A string is `section.number`. Once an adapter is released, a
//!   string keeps its address for good: text found later gets new sections, it is never numbered
//!   in between. Where text is found by what points at it, and more may be found later in the same
//!   place, a kind numbers its strings by where they are (`numbered_by_place`), so that a string's
//!   number does not depend on what else is known.
//! - **The script is lossless.** What `extract` gives encodes back to exactly the game's bytes,
//!   and a pack that changes nothing builds the image it started from, byte for byte.
//! - **A pack is sparse.** Whatever a pack leaves out keeps the game's own bytes.
//!
//! `conformance` checks what a program can check of this. What it cannot check is the screen: a
//! build is not known to work until someone has seen it in an emulator.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::error::{Result, text_error};
use crate::fonts::{self, Font};
use crate::platform::{Image, Platform};
use crate::script::{Pack, Script, parse_name};
use crate::text::quoted;

/// One body of text in a game, and what a pack can do with it.
///
/// Dialogue, menus and item names are often separate systems inside one game, with separate
/// limits, so an adapter says what holds for each. An adapter that can only extract so far is a
/// valid one: everything below `sections` is then left as it is.
pub struct TextKind {
    /// One word, as a user would call it: "dialogue", "menu".
    pub name: &'static str,
    /// Whether a section, by its name, is part of this body of text.
    pub sections: fn(&str) -> bool,
    /// A pack can change its strings.
    pub insert: bool,
    /// A changed string may take more room than the game's own.
    pub grow: bool,
    /// Characters the game's font lacks can be added, from a font the user gives.
    pub new_characters: bool,
    /// Lines are fitted to the screen, unless the user asks for them as written.
    pub wrap: bool,
    /// A string's number is where it is in its section, such as its offset in a file, so the
    /// numbers have gaps. Otherwise a section's strings are numbered from 0 up.
    pub numbered_by_place: bool,
    /// The most characters a string may have, when there is such a limit.
    pub max_length: Option<usize>,
    /// What else limits a string, in a sentence a translator can act on.
    pub limits: &'static str,
    /// For the checks, when `insert` is set: the address of a string that is safe to change, and
    /// that of another whose text is put there. When `grow` is set the second must be the longer.
    pub trial: Option<(&'static str, &'static str)>,
}

impl TextKind {
    /// A body of text that can only be extracted. Set the other fields over this one:
    /// `TextKind { insert: true, ..TextKind::new("dialogue", is_dialogue) }`.
    pub const fn new(name: &'static str, sections: fn(&str) -> bool) -> Self {
        TextKind {
            name,
            sections,
            insert: false,
            grow: false,
            new_characters: false,
            wrap: false,
            numbered_by_place: false,
            max_length: None,
            limits: "",
            trial: None,
        }
    }
}

/// One image of a game that an adapter was worked out on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    /// The image's SHA-1, in lowercase hexadecimal, which is what says a file is this one.
    pub sha1: &'static str,
    /// Which release it is, as shown to a user: "Japan, version 1.01".
    pub name: &'static str,
    /// Its size in bytes, without anything a dumping tool added. A folder of games is searched by
    /// size first, so that only a file of this size is read to see whether it is the game.
    pub size: u64,
    /// What the image says it is, as its platform reads it back (`Platform::label`): a disc's
    /// serial, "SLPS-25172", or a cartridge's title. With `version`, it lets a file that is refused
    /// be told whether it is this release with other contents, or another release.
    pub label: &'static str,
    /// The version the image gives with its label, as its platform shows it: "1.01".
    pub version: &'static str,
    /// Where a catalogue of dumps lists the image, when one does. Players, their download sets and
    /// tools such as an emulator's own check know a dump by its name there, so it is shown wherever
    /// the release is.
    pub catalogue: Option<CatalogueEntry>,
}

/// An image as a catalogue of dumps lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogueEntry {
    pub catalogue: &'static Catalogue,
    /// The name it has there: "Tales of Destiny 2 (Japan, Asia)".
    pub name: &'static str,
    /// The address of its record there, given only where the record has an address of its own (a
    /// record number, not a search). The catalogues host no games: a record is facts about a dump,
    /// its hashes and sizes, and this is never to be a link to anything that offers a game.
    pub page: Option<&'static str>,
}

/// A catalogue of dumps that players and their tools name game files by: Redump for discs,
/// No-Intro for cartridges. Each console's module has the one for its games.
#[derive(Debug, PartialEq, Eq)]
pub struct Catalogue {
    /// "Redump"
    pub name: &'static str,
    /// What it is, in a few words for someone who has never heard of it: "the catalogue of disc dumps".
    pub about: &'static str,
}

/// A file of the game, or one that says it is, that the adapter does not read and can name: another
/// release of the game, or one of its releases with a known patch applied. Listing one lets a
/// refusal say what the file is. It is matched by what the file says it is (`Platform::label`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The label it gives: "SLPS-72502".
    pub label: &'static str,
    /// The version it gives with it, or `None` where any version is that one.
    pub version: Option<&'static str>,
    /// What it is, in a few words that follow "it is": "the Mega Hits! reissue, in Japanese". For a
    /// patch, what the patch is: "DeJap's English translation".
    pub about: &'static str,
    /// It is one of the adapter's releases with a patch applied, not another release.
    pub patched: bool,
}

/// What a user can ask of a build, besides the pack.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    /// To draw characters the game's font lacks, tried in the order given. With none, a build
    /// that needs a font draws from the adapter's own (`Adapter::fonts`).
    pub fonts: Vec<Font>,
    /// The same for text drawn in small tiles, where a game has such text.
    pub menu_font: Option<Font>,
    /// Fit lines to the screen; off keeps line breaks exactly as the pack has them.
    pub wrap: bool,
    /// The adapter's own switches that are turned on.
    pub switches: BTreeSet<String>,
    /// Labels to draw somewhere else than the game does: a label's name, as the adapter gives it,
    /// and how far to move it, in columns to the right and rows down. A translation's word is
    /// often wider than the place the game gives it, and the game has no room to spare; moving a
    /// label, or its neighbours, makes the room.
    pub layout: BTreeMap<String, (i32, i32)>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            fonts: Vec::new(),
            menu_font: None,
            wrap: true,
            switches: BTreeSet::new(),
            layout: BTreeMap::new(),
        }
    }
}

/// What a build did, as lines to show: what changed, what moved, what was added to the font.
pub struct Built {
    pub summary: Vec<String>,
}

/// One game. A module under `adapters` holds one type that implements this, and `adapters::ALL`
/// lists it.
pub trait Adapter: Sync {
    /// Short and permanent, lowercase with hyphens, the console last: "tod2-ps2". Packs name it.
    fn id(&self) -> &'static str;
    /// As shown to a user: "Tales of Destiny 2 (PlayStation 2, Japan)".
    fn name(&self) -> &'static str;
    fn platform(&self) -> &'static dyn Platform;
    /// Each image the adapter was worked out on.
    fn images(&self) -> &'static [Release];
    /// The files of its game it knows of and does not read: other releases, and its releases with a
    /// known patch applied. They are not read; they are named, so that a refused file is told why.
    fn refuses(&self) -> &'static [Refused] {
        &[]
    }
    /// The language of the game's own text, as an IETF tag: "ja".
    fn source_language(&self) -> &'static str;
    fn kinds(&self) -> &'static [TextKind];
    /// The adapter's own build switches: name, and what it does.
    fn switches(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
    /// Whether a pack's `names.txt` can rename the game's characters.
    fn sets_names(&self) -> bool {
        false
    }
    /// What limits a name a pack gives in `names.txt`, as a short phrase in the manner of a kind's
    /// `limits`: "a name is 1 to 15 characters, without tags". Empty when nothing is to be said, and
    /// always when the game's names cannot be set.
    fn name_limits(&self) -> &'static str {
        ""
    }
    /// Whether some of its text is drawn from a second, smaller font that can be added to.
    fn takes_menu_font(&self) -> bool {
        false
    }
    /// Whether a pack can move some of the game's labels on the screen (`BuildOptions::layout`).
    /// The adapter says which, and under what names, in the notes `extract` writes.
    fn moves_labels(&self) -> bool {
        false
    }
    /// The built-in fonts, by name, that new characters are drawn from when a build is given no
    /// font, tried in this order. They are the ones that were looked at in this game and found to
    /// suit it, so that a pack need bring none.
    fn fonts(&self) -> &'static [&'static str] {
        &[]
    }
    /// The same for the text drawn from the second, smaller font.
    fn menu_font(&self) -> Option<&'static str> {
        None
    }

    /// The game's whole script, and its characters' names if a pack can change them.
    fn extract(&self, image: &Image) -> Result<Pack>;

    /// What a translator should know while writing in a section's file, in sentences they can act
    /// on: with no number, about the whole file, and with one, about that string. `extract` writes
    /// it as a comment, at the head of the file or under the string's header. It is for what the
    /// text itself does not show: a string drawn some other way than its neighbours, a width the
    /// screen gives it. Most strings have none.
    fn note(&self, _section: &str, _number: Option<u32>) -> Option<String> {
        None
    }

    /// Fail with `Error::Text` unless an extracted script encodes back to exactly the game's bytes.
    fn verify(&self, image: &Image, texts: &Script) -> Result<()>;

    /// The adapter's part of `build`, which is what callers use. An adapter whose text can be
    /// changed provides it: write to `output` the game with the strings the pack gives, the rest
    /// keeping the game's own. It fails with `Error::Text`, in words a translator can act on, when
    /// the pack asks for something the game cannot take: a string too long for where it must go, a
    /// character with no font to draw it from, a section or a string that does not exist.
    fn write(&self, _image: &Image, _pack: &Pack, _output: &Path, _options: &BuildOptions) -> Result<Built> {
        text_error(format!("{} can be extracted but not rebuilt yet", self.name()))
    }

    /// A glyph of the main font is drawn in a square this many pixels wide.
    fn glyph_size(&self) -> usize;
    fn glyph_count(&self) -> usize;
    /// How bright each pixel value of a glyph is shown, 0 to 255.
    fn glyph_shades(&self) -> &'static [u8] {
        &[0, 255]
    }
    /// A glyph of the game's main font: `glyph_size` rows of pixels, each an index into `glyph_shades`.
    fn glyph(&self, image: &Image, index: usize) -> Result<Vec<Vec<u8>>>;
    /// The bytes that select a glyph in the script, as one number.
    fn code_at(&self, index: usize) -> u32;

    /// The codes in a string of the script that stand for characters with no table entry yet.
    fn unmapped(&self, text: &str) -> Vec<String>;

    fn can_insert(&self) -> bool {
        self.kinds().iter().any(|kind| kind.insert)
    }

    /// The body of text a section belongs to.
    fn kind_of(&self, section: &str) -> Option<&'static TextKind> {
        self.kinds().iter().find(|kind| (kind.sections)(section))
    }

    /// Fail with `Error::Text` if the pack or the options ask for something this game has not got,
    /// as far as can be told without the game: a build option, names for its characters, a font
    /// for its menus.
    fn accepts(&self, pack: &Pack, options: &BuildOptions) -> Result<()> {
        if !self.can_insert() {
            return text_error(format!("{} can be extracted but not rebuilt yet", self.name()));
        }
        let known: Vec<&str> = self.switches().iter().map(|(name, _)| *name).collect();
        let unknown: Vec<&str> =
            options.switches.iter().map(String::as_str).filter(|name| !known.contains(name)).collect();
        if !unknown.is_empty() {
            let known = if known.is_empty() { "none".to_string() } else { known.join(", ") };
            return text_error(format!("{} has no option named {} (it has: {known})", self.name(), unknown.join(", ")));
        }
        if !pack.names.is_empty() && !self.sets_names() {
            return text_error(format!(
                "the pack gives names for characters, and {} has none a pack can set yet",
                self.name()
            ));
        }
        if options.menu_font.is_some() && !self.takes_menu_font() {
            return text_error(format!("{} has no text that a menu font is used for", self.name()));
        }
        if !options.layout.is_empty() && !self.moves_labels() {
            return text_error(format!("{} has no labels that a pack can move yet", self.name()));
        }
        Ok(())
    }

    /// Write to `output` the game with the strings the pack gives; the rest keeps the game's own.
    /// This checks what can be checked without the game, then hands over to the adapter's `write`.
    fn build(&self, image: &Image, pack: &Pack, output: &Path, options: &BuildOptions) -> Result<Built> {
        self.accepts(pack, options)?;
        self.write(image, pack, output, options)
    }
}

/// Whether two adapters are the same one. They are told apart by id, not by address: an adapter is
/// an empty type, and the statics of empty types may all share one address.
pub fn same_adapter(one: &dyn Adapter, other: &dyn Adapter) -> bool {
    one.id() == other.id()
}

/// What is wrong with what an adapter says about itself, without looking at any game.
pub fn declaration_faults(adapter: &dyn Adapter) -> Vec<String> {
    let mut faults = Vec::new();
    let id = adapter.id();
    let words: Vec<&str> = id.split('-').collect();
    let word = |word: &&str| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    if words.len() < 2 || !words.iter().all(word) {
        faults.push(format!("its id, {}, is not lowercase words joined by hyphens", quoted(id)));
    }
    let sha1 = |sha1: &str| sha1.len() == 40 && sha1.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if adapter.images().is_empty() || !adapter.images().iter().all(|image| sha1(image.sha1)) {
        faults.push("its images are not named by SHA-1, in lowercase hexadecimal".into());
    }
    if adapter.images().iter().any(|image| image.size == 0) {
        faults.push("it does not say how large its images are".into());
    }
    // A program cannot check a catalogue's name against the catalogue itself, which is not here; it
    // can only see that the name is given in full.
    let partial = |entry: &CatalogueEntry| {
        entry.catalogue.name.is_empty() || entry.catalogue.about.is_empty() || entry.name.is_empty()
    };
    if adapter.images().iter().any(|image| image.catalogue.as_ref().is_some_and(partial)) {
        faults.push("the name a catalogue gives one of its images is not given in full".into());
    }
    let web = |page: &str| page.starts_with("http://") || page.starts_with("https://");
    let pages = adapter.images().iter().filter_map(|image| image.catalogue.and_then(|entry| entry.page));
    if pages.into_iter().any(|page| !web(page)) {
        faults.push("the page of a catalogue's record it gives is not a web address".into());
    }
    for refused in adapter.refuses() {
        let reads = |image: &Release| refused.version.is_none_or(|version| version == image.version);
        if adapter.images().iter().any(|image| image.label == refused.label && reads(image)) {
            faults.push(format!("it both reads and refuses the files that say they are {}", refused.label));
        }
        if refused.label.is_empty() || refused.about.is_empty() {
            faults.push("a file it refuses is not given a label and a few words about it".into());
        }
    }
    let kinds = adapter.kinds();
    if kinds.is_empty() {
        faults.push("it names no kind of text".into());
    }
    let names: BTreeSet<&str> = kinds.iter().map(|kind| kind.name).collect();
    if names.len() != kinds.len() {
        faults.push("two of its kinds of text have the same name".into());
    }
    for font in adapter.fonts().iter().chain(&adapter.menu_font()) {
        if fonts::built_in(font).is_none() {
            faults.push(format!("the font it draws from, {}, is not one built into the program", quoted(font)));
        }
    }
    if adapter.menu_font().is_some() && !adapter.takes_menu_font() {
        faults.push("it names a font for its menus and has no text that one is used for".into());
    }
    if !adapter.name_limits().is_empty() && !adapter.sets_names() {
        faults.push("it gives limits on its characters' names, and a pack cannot set any".into());
    }
    for kind in kinds {
        if !kind.insert && (kind.grow || kind.new_characters || kind.wrap || kind.trial.is_some()) {
            faults.push(format!("{} cannot be changed, yet says how a changed string is treated", kind.name));
        }
        if !kind.insert {
            continue;
        }
        let trial = kind.trial.and_then(|(to, from)| Some((parse_name(to).ok()?, parse_name(from).ok()?, to != from)));
        match trial {
            None => faults.push(format!("{} can be changed and names no two strings to try it on", kind.name)),
            Some(((to, _), (from, _), different)) => {
                let own = |section: &str| adapter.kind_of(section).is_some_and(|found| std::ptr::eq(found, kind));
                if !different || !own(&to) || !own(&from) {
                    faults.push(format!("the strings {} is tried on are not two of its own", kind.name));
                }
            }
        }
    }
    faults
}
