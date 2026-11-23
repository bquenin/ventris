//! What is about a console and not about one game.
//!
//! Each console's module is two things. Its `PLATFORM` is the little the engine itself needs, on the
//! contract in `platform`: recognising an image, opening it, naming it by its SHA-1. The rest of
//! the module is a library for the adapters of that console's games: address mapping, headers and
//! checksums, expansion, an assembler for injected code, reading and rebuilding a disc image.

use std::path::Path;

use crate::error::Result;
use crate::platform::Platform;

pub mod asm65816;
pub mod ps2;
pub mod snes;
pub mod w65816;

/// Every console, in the order they are asked whether a file is theirs.
pub static ALL: &[&dyn Platform] = &[&ps2::PLATFORM, &snes::PLATFORM];

/// The console the file at `path` is an image for, if any here knows it.
pub fn recognise(path: &Path) -> Result<Option<&'static dyn Platform>> {
    for platform in ALL.iter().copied() {
        if platform.recognises(path)? {
            return Ok(Some(platform));
        }
    }
    Ok(None)
}
