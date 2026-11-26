//! What the engine asks of a console, so that the command line never has to know which one it is.
//!
//! A platform recognises an image file, opens it as whatever its adapters work on, and says which
//! image it is by its SHA-1. It can also size a file up at a glance, without reading it, which is
//! what lets a whole folder of games be searched quickly, and read what an image says it is (a
//! disc's serial, a cartridge's title), which is what lets a file that is refused be told what it
//! is. That is all the engine needs. Everything else a console's module offers (address mapping,
//! checksums, rebuilding a disc) is a library for the adapters of that console's games, and no
//! part of this contract.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result, at};
use crate::platforms::ps1;
use crate::platforms::ps2::Disc;

/// What an adapter is given to read the game from. Each console adds the form its games take.
pub enum Image {
    /// A cartridge, small enough to hold whole: its bytes, without anything a dumping tool added.
    Cartridge(Vec<u8>),
    /// A disc, too large to hold whole: it stays open, and the files inside it are read as they
    /// are asked for. It is closed when the image is dropped. This is a PlayStation 2 disc, an
    /// image of 2048-byte sectors.
    Disc(Disc),
    /// A PlayStation disc, held the same way: an image of whole 2352-byte CD sectors.
    Ps1Disc(ps1::Disc),
}

impl Image {
    /// The bytes of a cartridge image. An adapter for a cartridge game starts here.
    pub fn cartridge(&self) -> Result<&[u8]> {
        match self {
            Image::Cartridge(rom) => Ok(rom),
            _ => Err(Error::Image("the image is not a cartridge".into())),
        }
    }

    /// The open disc. An adapter for a game on a disc starts here.
    pub fn disc(&self) -> Result<&Disc> {
        match self {
            Image::Disc(disc) => Ok(disc),
            _ => Err(Error::Image("the image is not a disc".into())),
        }
    }

    /// The open PlayStation disc. An adapter for a PlayStation game starts here.
    pub fn ps1_disc(&self) -> Result<&ps1::Disc> {
        match self {
            Image::Ps1Disc(disc) => Ok(disc),
            _ => Err(Error::Image("the image is not a PlayStation disc".into())),
        }
    }
}

impl Image {
    /// How many bytes the image is: the size an adapter gives for it (`Release::size`).
    pub fn size(&self) -> Result<u64> {
        match self {
            Image::Cartridge(rom) => Ok(rom.len() as u64),
            Image::Disc(disc) => Ok(fs::metadata(&disc.path).map_err(at(&disc.path))?.len()),
            Image::Ps1Disc(disc) => Ok(fs::metadata(&disc.path).map_err(at(&disc.path))?.len()),
        }
    }
}

/// What a file looks to hold, told from its name and its size alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glance {
    /// The size in bytes of the game `open` would give, without anything a dumping tool added.
    pub size: u64,
    /// The name of the image itself: the file's own, or that of the image inside an archive.
    pub name: String,
}

/// What an image says it is, in the few bytes where a console's games describe themselves: for a
/// PlayStation 2 disc the serial and version its `SYSTEM.CNF` gives, for a cartridge the title and
/// version byte of its header. Anyone can write these, so they say what a file claims to be, and
/// only its SHA-1 says what it is; they are for telling a user why a file is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// What names the game: "SLPS-25172", "TALES OF PHANTASIA".
    pub name: String,
    /// "1.01"; empty when the image gives none.
    pub version: String,
}

/// A label as a user is shown it: "serial SLPS-25172, version 1.01". A name with spaces in it, such
/// as a cartridge's title, is put in quotes, so that it does not run into the sentence around it.
pub fn label_words(platform: &dyn Platform, name: &str, version: &str) -> String {
    let name = if name.contains(' ') { format!("\"{name}\"") } else { name.to_string() };
    let mut words = format!("{} {name}", platform.label_kind());
    if !version.is_empty() {
        words.push_str(&format!(", version {version}"));
    }
    words
}

/// Whether two consoles are the same one. They are told apart by name, not by where they are in
/// memory: each is a value that takes no room, and the compiler may put two such values at one
/// address, which made two consoles one in builds where it did.
pub fn same_platform(one: &dyn Platform, other: &dyn Platform) -> bool {
    one.name() == other.name()
}

/// One console.
pub trait Platform: Sync {
    /// As shown to a user: "Super Famicom / SNES".
    fn name(&self) -> &'static str;

    /// Whether the file at `path` is an image for this console. It must not read a large file whole.
    fn recognises(&self, path: &Path) -> Result<bool>;

    /// What the file at `path` would hold if it is an image for this console, going by its name
    /// and its size; `None` when by those it cannot be one. It reads nothing of the file but, for an
    /// archive, the list of what is in it. A folder of thousands of games is searched by asking
    /// this of every file, and opening only those of the size of a game that is looked for.
    fn glance(&self, path: &Path) -> Result<Option<Glance>>;

    /// The image as this console's adapters take it, and the SHA-1 that identifies it, in lowercase
    /// hexadecimal. The SHA-1 is taken over the game alone, without anything a dumping tool added.
    fn open(&self, path: &Path) -> Result<(Image, String)>;

    /// What an open image says it is, or `None` when it says nothing this console's games say.
    fn label(&self, _image: &Image) -> Option<Label> {
        None
    }

    /// What the name in a label is, as a word a user knows it by: "serial", "title".
    fn label_kind(&self) -> &'static str {
        "name"
    }

    /// What the file at `path` says it is, read from the few bytes where it says so, without opening
    /// it as an image or hashing it: what lets a folder of games be told which files are a game that
    /// is looked for, in another release, without reading them. `None` when it says nothing, and
    /// where finding out would cost more than a few small reads, as for a cartridge in an archive.
    fn peek(&self, _path: &Path) -> Option<Label> {
        None
    }

    /// Whether this console's games are discs, so that a compressed disc image may be one of them.
    fn disc(&self) -> bool {
        false
    }

    /// What a build changed about the image as a whole, as lines to show; an adapter reports the rest.
    fn changes(&self, _original: &Image, _built: &Image) -> Vec<String> {
        Vec::new()
    }
}
