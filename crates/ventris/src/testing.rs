//! Finding a game on the machine the tests run on.
//!
//! No game is in the repository, so a test that needs one looks for it, and checks nothing where
//! it is not there. It looks by what the game is and not by a file's name: in the folder the
//! environment variable `VENTRIS_ROMS` names, or else in `roms/` at the top of the
//! repository, for a file that is the image the game's adapter was worked out on. Anyone's
//! collection will do, however its files are named and whether or not they are in archives.
//!
//! This is for the tests of the engine, of the command line and of the app, and no part of what a
//! user runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::platform::Image;
use crate::tasks::{self, Candidate};

/// The folder games are looked for in.
pub fn games_folder() -> PathBuf {
    match std::env::var_os("VENTRIS_ROMS") {
        Some(folder) if !folder.is_empty() => PathBuf::from(folder),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("roms"),
    }
}

/// The file of the game with this adapter id, or `None`, with a word about why, where it is not
/// there to test with. The folder is gone through once for all the tests of a program.
pub fn game(id: &str) -> Option<PathBuf> {
    static CANDIDATES: OnceLock<Vec<Candidate>> = OnceLock::new();
    static FOUND: Mutex<BTreeMap<String, Option<PathBuf>>> = Mutex::new(BTreeMap::new());

    let mut found = FOUND.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(known) = found.get(id) {
        return known.clone();
    }
    let candidates = CANDIDATES.get_or_init(|| tasks::candidates(&games_folder()).unwrap_or_default());
    let could_be = |candidate: &&Candidate| candidate.could_be.iter().any(|adapter| adapter.id() == id);
    let is_it = |candidate: &Candidate| match candidate.identify() {
        Ok(Some(game)) if game.adapter.id() == id => Some(game.path),
        _ => None,
    };
    let path = candidates.iter().filter(could_be).find_map(is_it);
    if path.is_none() {
        eprintln!("skipped: the game {id} was not found in {}", games_folder().display());
    }
    found.insert(id.to_string(), path.clone());
    path
}

/// The bytes of a cartridge game, without anything a dumping tool added, or `None` where it is
/// not there to test with.
pub fn cartridge(id: &str) -> Option<Vec<u8>> {
    let (image, _, _) = tasks::open_game(&game(id)?).ok()?;
    match image {
        Image::Cartridge(rom) => Some(rom),
        Image::Disc(_) | Image::Ps1Disc(_) => None,
    }
}

/// The file in the games folder, or a folder inside it, that is the image with this size and SHA-1,
/// for a test of a console's module, which has no adapter to find a game by. Only files of that size
/// are read. `None`, with a word about why, where it is not there to test with.
pub fn image_file(size: u64, sha1: &str) -> Option<PathBuf> {
    use sha1::{Digest, Sha1};
    use std::io::Read;

    fn search(folder: &Path, depth: usize, size: u64, sha1: &str) -> Option<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(folder).ok()?.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            let Ok(about) = std::fs::metadata(&path) else { continue };
            if about.is_dir() {
                if depth > 0
                    && let Some(found) = search(&path, depth - 1, size, sha1)
                {
                    return Some(found);
                }
                continue;
            }
            if about.len() != size {
                continue;
            }
            let Ok(mut file) = std::fs::File::open(&path) else { continue };
            let mut digest = Sha1::new();
            let mut chunk = vec![0; 1 << 22];
            while let Ok(count) = file.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                digest.update(&chunk[..count]);
            }
            let found: String = digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
            if found == sha1 {
                return Some(path);
            }
        }
        None
    }

    let found = search(&games_folder(), 8, size, sha1);
    if found.is_none() {
        eprintln!("skipped: the image with SHA-1 {sha1} was not found in {}", games_folder().display());
    }
    found
}
