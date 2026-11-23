//! What the engine asks of a console, so that the command line never has to know which one it is.
//!
//! A platform recognises an image file, opens it as whatever its adapters work on, and says which
//! image it is by its SHA-1. It can also size a file up at a glance, without reading it, which is
//! what lets a whole folder of games be searched quickly. That is all the engine needs. Everything else a console's module
//! offers (address mapping, checksums, rebuilding a disc) is a library for the adapters of that
//! console's games, and no part of this contract.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result, at};
use crate::platforms::ps2::Disc;

/// What an adapter is given to read the game from. Each console adds the form its games take.
pub enum Image {
    /// A cartridge, small enough to hold whole: its bytes, without anything a dumping tool added.
    Cartridge(Vec<u8>),
    /// A disc, too large to hold whole: it stays open, and the files inside it are read as they
    /// are asked for. It is closed when the image is dropped.
    Disc(Disc),
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
}

impl Image {
    /// How many bytes the image is: the size an adapter gives for it (`Release::size`).
    pub fn size(&self) -> Result<u64> {
        match self {
            Image::Cartridge(rom) => Ok(rom.len() as u64),
            Image::Disc(disc) => Ok(fs::metadata(&disc.path).map_err(at(&disc.path))?.len()),
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

    /// What a build changed about the image as a whole, as lines to show; an adapter reports the rest.
    fn changes(&self, _original: &Image, _built: &Image) -> Vec<String> {
        Vec::new()
    }
}
