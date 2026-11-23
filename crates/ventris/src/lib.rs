//! ventris: a game translation patch engine.
//!
//! It extracts a game's script as UTF-8 text, and puts a translation in any alphabet back into the
//! game. `adapter` is the contract every game's code follows; `adapters` holds one module per game.
//! `platform` is the contract for a console, and `platforms` holds one module per console. The
//! rest is what adapters share: scripts as files, a pack's manifest, the tag language, fonts and
//! the few that are built in, wrapping, placement, and under `codecs` the compression schemes games use.
//! `tasks` is what a user does with a game and a pack, for the command line and the app to share,
//! and `archive` reads a game out of the zip file a collection keeps it in.
//!
//! The engine was first written in Python and ported here, and comments call that first version
//! "the Python reference". The port was held to building the same image from the same pack, byte
//! for byte, which is why some choices here are made exactly as the Python made them: the order a
//! compressor tries its options in, how a sum is rounded, how a message quotes a string. Changing
//! one of those changes the images that are built, so it is a change to make on purpose and say so.

pub mod adapter;
pub mod adapters;
pub mod archive;
pub mod codecs;
pub mod conformance;
pub mod error;
pub mod fonts;
pub mod layout;
pub mod manifest;
pub mod placement;
pub mod platform;
pub mod platforms;
pub mod png;
pub mod script;
pub mod shift_jis;
pub mod tasks;
#[doc(hidden)]
pub mod testing;
pub mod text;

pub use error::{Error, Result};
