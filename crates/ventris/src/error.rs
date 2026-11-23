//! What can go wrong, by who has to act on it.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Text cannot be converted to or from the game's encoding, or a pack asks for something the
    /// game cannot take. The message is written for a translator.
    #[error("{0}")]
    Text(String),
    /// A script file is not in the expected format.
    #[error("{0}")]
    Script(String),
    /// A font cannot supply what was asked of it.
    #[error("{0}")]
    Font(String),
    /// Compressed data is not what its scheme says it should be.
    #[error("{0}")]
    Codec(String),
    /// Code that a build adds to a game cannot be assembled.
    #[error("{0}")]
    Assembly(String),
    /// An image is not one a platform can read, or has no such file or address.
    #[error("{0}")]
    Image(String),
    /// No adapter matches the file.
    #[error("{} is not a game ventris knows how to read yet", .0.display())]
    UnknownGame(PathBuf),
    /// An adapter does not do what the contract says.
    #[error("{0}")]
    Conformance(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A `Result` that fails with a message for a translator.
pub fn text_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Text(message.into()))
}

/// Puts the file's name before what the system said went wrong with it, for `map_err`: the
/// system's own message ("cannot find the file specified") does not say which file it was.
pub fn at(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |error| Error::Io(std::io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}
