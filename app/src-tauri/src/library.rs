//! The user's games: the files in their games folder that are games there is an adapter for.
//!
//! Someone who plays old games keeps them in one folder, often thousands of them, loose or one to
//! a zip archive. They are not asked for each game's file: the folder is searched. The search is
//! the engine's (`ventris::tasks::candidates`), which reads no file to find the few that have
//! a game's size. What is added here is memory: a file that was read before, and has not changed
//! since, is known without being read again, so that looking through the folder each time the app
//! starts costs a moment and not the reading of a whole disc.
//!
//! A file the user chose themselves is the second way, for a game kept somewhere else. It is kept
//! over whatever the search finds, for as long as the file is there.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ventris::adapter::Adapter;
use ventris::adapters;
use ventris::tasks::{self, Candidate, Found};

use crate::settings::GameFile;

fn seconds(time: Option<SystemTime>) -> u64 {
    time.and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |since| since.as_secs())
}

/// A game's file, with what tells it apart from another file at the same place.
pub fn game_file(path: &Path, release: &str, sha1: &str, chosen: bool) -> GameFile {
    let about = fs::metadata(path).ok();
    GameFile {
        path: path.display().to_string(),
        release: release.to_string(),
        sha1: sha1.to_string(),
        file_size: about.as_ref().map_or(0, |about| about.len()),
        modified: seconds(about.and_then(|about| about.modified().ok())),
        chosen,
    }
}

/// Whether a candidate is the very file that was remembered: same place, same size, same time.
fn is_same_file(candidate: &Candidate, file: &GameFile) -> bool {
    candidate.path.display().to_string() == file.path
        && candidate.file_size == file.file_size
        && seconds(candidate.modified) == file.modified
}

/// What a search of the games folder gives.
#[derive(Debug, Default, PartialEq)]
pub struct Looked {
    /// Each game's file, by the id of the game's adapter.
    pub games: BTreeMap<String, GameFile>,
    /// For each game that is not found, what the folder holds of it, as sentences that start with a
    /// file's path from the folder: a wrong copy and why it is not read, and compressed disc images.
    /// A game with nothing to say is left out.
    pub near: BTreeMap<String, Vec<String>>,
}

/// Search the games folder, and give each game's file: what the user chose themselves where that
/// is still there, and otherwise what the folder holds. A game that is in neither is left out, and
/// what the folder holds of it is said (`Looked::near`).
pub fn search(folder: &Path, remembered: &BTreeMap<String, GameFile>) -> Result<Looked, String> {
    let survey = tasks::survey(folder).map_err(|error| error.to_string())?;
    let mut games: BTreeMap<String, GameFile> = BTreeMap::new();
    for (id, file) in remembered {
        if file.chosen && Path::new(&file.path).is_file() {
            games.insert(id.clone(), file.clone());
        }
    }
    // First the files that are known from before, which costs nothing.
    let mut unknown: Vec<&Candidate> = Vec::new();
    for candidate in &survey.candidates {
        match remembered.iter().find(|(_, file)| !file.chosen && is_same_file(candidate, file)) {
            Some((id, file)) => {
                games.entry(id.clone()).or_insert_with(|| file.clone());
            }
            None => unknown.push(candidate),
        }
    }
    // Then the others are read, but only for the games that are still missing.
    unknown.retain(|candidate| candidate.could_be.iter().any(|adapter| !games.contains_key(adapter.id())));
    let (found, read) = tasks::identify_noting(unknown);
    for Found { adapter, release, path } in found {
        games.entry(adapter.id().to_string()).or_insert_with(|| game_file(&path, release.name, release.sha1, false));
    }
    let missing: Vec<&'static dyn Adapter> =
        adapters::ALL.iter().copied().filter(|adapter| !games.contains_key(adapter.id())).collect();
    let near = survey
        .missed(&missing, &read)
        .into_iter()
        .filter(|missed| !missed.is_empty())
        .map(|missed| (missed.adapter.id().to_string(), missed.lines(folder)))
        .collect();
    Ok(Looked { games, near })
}

#[cfg(test)]
mod tests {
    use ventris::{archive, testing};

    use super::*;

    fn remembered(id: &str, file: &GameFile) -> BTreeMap<String, GameFile> {
        BTreeMap::from([(id.to_string(), file.clone())])
    }

    #[test]
    fn a_folder_with_no_game_in_it_gives_none_and_keeps_only_a_choice_that_is_still_there() {
        let folder = tempfile::tempdir().unwrap();
        let look_alike = folder.path().join("look-alike.sfc");
        fs::write(&look_alike, vec![3u8; 0x400000]).unwrap();
        assert!(search(folder.path(), &BTreeMap::new()).unwrap().games.is_empty());

        // A file the user chose is kept while it is there, wherever it is; one that is gone is not.
        let chosen = game_file(&look_alike, "Japan", "ab", true);
        assert!(chosen.chosen && chosen.file_size == 0x400000 && chosen.modified > 0);
        assert_eq!(
            search(folder.path(), &remembered("top-sfc", &chosen)).unwrap().games,
            remembered("top-sfc", &chosen)
        );
        let gone = GameFile { path: folder.path().join("gone.sfc").display().to_string(), ..chosen.clone() };
        assert!(search(folder.path(), &remembered("top-sfc", &gone)).unwrap().games.is_empty());
        // What a search found before, and is no longer in the folder, is forgotten.
        let found_before = GameFile { chosen: false, ..gone };
        assert!(search(folder.path(), &remembered("top-sfc", &found_before)).unwrap().games.is_empty());

        let missing = folder.path().join("nowhere");
        assert_eq!(search(&missing, &BTreeMap::new()).unwrap_err(), format!("{} is not a folder", missing.display()));
    }

    #[test]
    fn what_the_folder_holds_of_a_game_it_is_not_found_in_is_said_for_that_game() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("game.chd"), b"MComprHD and the rest").unwrap();
        let looked = search(folder.path(), &BTreeMap::new()).unwrap();
        assert!(looked.games.is_empty());
        // A compressed disc image could be either disc game, and is no cartridge.
        assert_eq!(looked.near.keys().collect::<Vec<_>>(), ["god-ps1", "tod2-ps2"]);
        for game in ["god-ps1", "tod2-ps2"] {
            assert!(looked.near[game][0].starts_with("game.chd is a compressed disc image (CHD, CSO or ZSO): "));
        }
    }

    /// With the game at hand: it is found in an archive, known again without being read, and
    /// read again once the file has changed.
    #[test]
    fn a_game_is_found_and_then_known_without_being_read_again() {
        let Some(rom) = testing::cartridge("top-sfc") else { return };
        let folder = tempfile::tempdir().unwrap();
        let zipped = folder.path().join("snes").join("Tales of Phantasia (Japan).zip");
        fs::create_dir_all(zipped.parent().unwrap()).unwrap();
        fs::write(&zipped, archive::make(&[("Tales of Phantasia (Japan).sfc", &rom, true)])).unwrap();

        let found = search(folder.path(), &BTreeMap::new()).unwrap().games;
        assert_eq!(found.keys().collect::<Vec<_>>(), ["top-sfc"]);
        let file = &found["top-sfc"];
        assert_eq!(
            (file.path.as_str(), file.release.as_str(), file.chosen),
            (zipped.to_str().unwrap(), "Japan", false)
        );
        assert!(file.sha1.len() == 40 && file.file_size == fs::metadata(&zipped).unwrap().len());

        // Known from before: what was remembered is given back as it is, which shows that the
        // file was not read, since reading it would have put the game's own SHA-1 there.
        let marked = GameFile { sha1: "remembered".to_string(), ..file.clone() };
        assert_eq!(search(folder.path(), &remembered("top-sfc", &marked)).unwrap().games["top-sfc"].sha1, "remembered");
        // A file of another size at the same place is another file, and is read.
        let stale = GameFile { file_size: 1, ..marked };
        assert_eq!(search(folder.path(), &remembered("top-sfc", &stale)).unwrap().games["top-sfc"].sha1, file.sha1);
    }
}
