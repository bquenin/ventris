//! What the app remembers between runs: where the games are kept, which file each game was found
//! to be, and, for a game whose translator chose one, the folder their own pack is in.
//!
//! It is one small JSON file in the folder the system gives the app for its settings. A file that
//! is missing or cannot be read is the same as one that says nothing: the user is asked again.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A game's own file: one found in the games folder, or one the user chose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameFile {
    pub path: String,
    /// Which release of the game it is, as its adapter calls it.
    pub release: String,
    pub sha1: String,
    /// The file's size and when it was last changed, in seconds. A file found before, that still
    /// has both, is the same file and is not read again.
    #[serde(default)]
    pub file_size: u64,
    #[serde(default)]
    pub modified: u64,
    /// Whether the user chose this file themselves. Such a choice is kept over what a search finds.
    #[serde(default)]
    pub chosen: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The folder the user keeps their games in, which is searched for the ones there are adapters for.
    #[serde(default)]
    pub games_folder: Option<String>,
    /// The folder of a pack of the user's own for a game, such as one they are writing, by the id
    /// of the game's adapter. Only games the user chose one for are here.
    ///
    /// Before this, one folder held the packs for every game, as `packs_folder`. A file that still
    /// has that key reads as if it had not, and the key is gone once the file is saved again.
    #[serde(default)]
    pub own_packs: BTreeMap<String, String>,
    /// Each game's file, by the id of the game's adapter.
    #[serde(default)]
    pub games: BTreeMap<String, GameFile>,
}

pub fn load(file: &Path) -> Settings {
    fs::read_to_string(file).ok().and_then(|content| serde_json::from_str(&content).ok()).unwrap_or_default()
}

/// Write the settings whole to a file beside the settings file, then put it in its place, so that an
/// app stopped while it writes never leaves a file cut short. The file beside is named after this
/// process, so that two copies of the app saving at once do not write into the same one.
pub fn save(file: &Path, settings: &Settings) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("{}: {error}", file.display());
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder).map_err(failed)?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    let mut written = file.as_os_str().to_owned();
    written.push(format!(".{}.part", std::process::id()));
    let written = PathBuf::from(written);
    fs::write(&written, content).map_err(failed)?;
    fs::rename(&written, file).map_err(|error| {
        let _ = fs::remove_file(&written);
        failed(error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_kept_and_a_bad_file_says_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("deeper").join("settings.json");
        assert_eq!(load(&file), Settings::default());

        let mut settings = Settings { games_folder: Some("C:/games".to_string()), ..Settings::default() };
        settings.own_packs.insert("top-sfc".to_string(), "C:/my pack".to_string());
        let game = GameFile {
            path: "C:/games/top.sfc".to_string(),
            release: "Japan".to_string(),
            sha1: "ab".to_string(),
            file_size: 4,
            modified: 5,
            chosen: true,
        };
        settings.games.insert("top-sfc".to_string(), game);
        save(&file, &settings).unwrap();
        assert_eq!(load(&file), settings);
        // Written beside, then put in place: nothing else is left in the folder.
        save(&file, &Settings::default()).unwrap();
        assert_eq!(load(&file), Settings::default());
        let left: Vec<_> =
            fs::read_dir(file.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(left, ["settings.json"]);
        save(&file, &settings).unwrap();

        fs::write(&file, "{ not json").unwrap();
        assert_eq!(load(&file), Settings::default());
        // A file written before a key existed still reads.
        fs::write(&file, r#"{"games": {"top-sfc": {"path": "top.sfc", "release": "Japan", "sha1": "ab"}}}"#).unwrap();
        let older = load(&file);
        assert_eq!(older.games_folder, None);
        assert_eq!((older.games["top-sfc"].file_size, older.games["top-sfc"].chosen), (0, false));
        assert!(older.own_packs.is_empty());
    }

    /// A file from when one packs folder served every game still reads, with all it holds but that
    /// folder, and the old key is gone once the file is saved again.
    #[test]
    fn the_old_single_packs_folder_is_ignored_and_dropped_at_the_next_save() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("settings.json");
        let old = r#"{
  "games_folder": "C:/roms",
  "packs_folder": "C:/code/ventris-packs",
  "games": {"top-sfc": {"path": "top.zip", "release": "Japan", "sha1": "ab", "file_size": 4, "modified": 5}}
}"#;
        fs::write(&file, old).unwrap();
        let read = load(&file);
        assert_eq!(read.games_folder.as_deref(), Some("C:/roms"));
        assert_eq!((read.games["top-sfc"].path.as_str(), read.games["top-sfc"].file_size), ("top.zip", 4));
        assert!(read.own_packs.is_empty());

        save(&file, &read).unwrap();
        let saved = fs::read_to_string(&file).unwrap();
        assert!(!saved.contains("packs_folder") && saved.contains("own_packs"), "{saved}");
        assert_eq!(load(&file), read);
    }
}
