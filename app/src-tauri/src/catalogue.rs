//! The packs the app can offer: every folder under one folder that holds a `pack.toml`.
//!
//! The folder is laid out like the packs repository, `game/language/`, but nothing here depends
//! on that: a pack says in its own manifest which game it is for, and is found wherever it is.
//!
//! The packs come from two places. The first is the copy of the ventris packs fetched from GitHub
//! (`fetch`), which every player has. The second, which only some have, is a folder the user chose
//! on one game's page: a pack they are writing for that game, or one they got elsewhere. Only that
//! game's packs are taken from it. Both are listed together (`listing`), and the page puts the
//! user's own first.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ventris::manifest::{self, MANIFEST_FILE};
use ventris::tasks;

/// How many strings of one body of text a pack changes.
#[derive(Debug, PartialEq, Serialize)]
pub struct Count {
    /// The body of text, as the game's adapter calls it: "dialogue", "menu".
    pub kind: String,
    pub strings: usize,
}

/// A pack that can be offered: what its manifest says, and what it changes.
#[derive(Debug, Serialize)]
pub struct PackInfo {
    /// Where the pack is, for building from it.
    pub folder: String,
    /// The same from the folder that was searched, with `/` between folders, for showing.
    pub place: String,
    /// The id of the adapter of the game it is for.
    pub game: String,
    pub language: String,
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub changes: Vec<Count>,
    /// How many of the game's characters it renames.
    pub names: usize,
    /// Whether it is in a folder the user chose, and not in the copy fetched from GitHub.
    pub from_folder: bool,
}

/// A folder that has a `pack.toml` and is not a pack that can be built, and why.
#[derive(Debug, Serialize)]
pub struct Fault {
    pub place: String,
    /// The id of the game it is for, when its `pack.toml` can be read far enough to say.
    pub game: Option<String>,
    /// Why, in the words `ventris describe` gives.
    pub message: String,
    /// Whether it is in a folder the user chose, and not in the copy fetched from GitHub.
    pub from_folder: bool,
}

/// The packs under one folder.
#[derive(Debug, Serialize)]
pub struct Catalogue {
    /// The folder that was searched.
    pub folder: String,
    pub packs: Vec<PackInfo>,
    pub faults: Vec<Fault>,
}

/// How many folders deep a pack is looked for. The packs repository needs two.
const DEPTH: usize = 4;

/// The folders at or under `folder` that hold a manifest, in the order of their names. A pack's
/// own folders are not searched, and neither are hidden ones such as `.git`.
fn pack_folders(folder: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if folder.join(MANIFEST_FILE).is_file() {
        found.push(folder.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(folder) else { return };
    let mut inside: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    inside.sort();
    for path in inside {
        let hidden = path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if !hidden {
            pack_folders(&path, depth - 1, found);
        }
    }
}

/// Where a pack is from the folder that was searched: "tod2-ps2/en".
fn place(root: &Path, folder: &Path) -> String {
    let inside = folder.strip_prefix(root).unwrap_or(folder);
    let parts: Vec<String> = inside.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.is_empty() { ".".to_string() } else { parts.join("/") }
}

/// Every pack under a folder, described and checked as far as can be done without the games.
pub fn read(root: &Path) -> Result<Catalogue, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    let mut folders = Vec::new();
    pack_folders(root, DEPTH, &mut folders);
    let (mut packs, mut faults) = (Vec::new(), Vec::new());
    for folder in folders {
        let place = place(root, &folder);
        match tasks::describe(&folder) {
            Ok(pack) => {
                let changed = pack.kinds.iter().filter(|count| count.strings > 0);
                let manifest = pack.manifest;
                packs.push(PackInfo {
                    folder: folder.display().to_string(),
                    place,
                    game: manifest.game,
                    language: manifest.language,
                    name: manifest.name,
                    version: manifest.version,
                    authors: manifest.authors,
                    license: manifest.license,
                    description: manifest.description,
                    homepage: manifest.homepage,
                    changes: changed
                        .map(|count| Count { kind: count.kind.name.to_string(), strings: count.strings })
                        .collect(),
                    names: pack.names,
                    from_folder: false,
                });
            }
            Err(error) => {
                // The game is named in the manifest; when that cannot be read, nobody knows.
                let game = manifest::read(&folder).ok().flatten().map(|manifest| manifest.game);
                faults.push(Fault { place, game, message: error.to_string(), from_folder: false });
            }
        }
    }
    Ok(Catalogue { folder: root.display().to_string(), packs, faults })
}

/// A folder of packs of the user's own for one game, chosen on that game's page: a pack they are
/// writing, or one they got elsewhere.
#[derive(Debug, Serialize)]
pub struct OwnFolder {
    /// The id of the adapter of the game it was chosen for.
    pub game: String,
    pub folder: String,
    /// Why the folder could not be read, as a sentence to show under it: "It is no longer there."
    pub trouble: Option<String>,
    /// Its packs for that game. Packs in it for other games are left out.
    pub packs: Vec<PackInfo>,
    /// Its faulty packs for that game, and those whose `pack.toml` cannot be read far enough to say
    /// which game they are for: the user chose the folder for this game, so they are taken as its.
    pub faults: Vec<Fault>,
}

/// Every pack the app can offer, for the page.
#[derive(Debug, Serialize)]
pub struct Listing {
    /// The folders of packs of the user's own, one for each game they chose one for.
    pub own: Vec<OwnFolder>,
    /// Whether there is a copy of the packs fetched from GitHub.
    pub fetched: bool,
    /// When that copy was last found to be up to date, in seconds since 1970; 0 when not known.
    pub checked: u64,
    /// The packs of that copy, for every game.
    pub packs: Vec<PackInfo>,
    pub faults: Vec<Fault>,
}

/// Whether a pack that names the game `its`, or none, is taken as one for `game` from a folder the
/// user chose for that game. One whose game cannot be told is: the folder was chosen for this game.
fn belongs(its: Option<&str>, game: &str) -> bool {
    its.is_none_or(|its| its == game)
}

/// Of the packs under a folder chosen for one game, those for that game, marked as the user's own.
fn for_game(catalogue: Catalogue, game: &str) -> (Vec<PackInfo>, Vec<Fault>) {
    let packs = catalogue
        .packs
        .into_iter()
        .filter(|pack| pack.game == game)
        .map(|pack| PackInfo { from_folder: true, ..pack })
        .collect();
    let faults = catalogue
        .faults
        .into_iter()
        .filter(|fault| belongs(fault.game.as_deref(), game))
        .map(|fault| Fault { from_folder: true, ..fault })
        .collect();
    (packs, faults)
}

/// Read the folder the user chose for a game's own packs. One that cannot be read is said to be so,
/// in a sentence shown under the folder's path, so the path is not said again when it is simply gone.
pub fn own_folder(game: &str, folder: &Path) -> OwnFolder {
    let (packs, faults, trouble) = match read(folder) {
        Ok(catalogue) => {
            let (packs, faults) = for_game(catalogue, game);
            (packs, faults, None)
        }
        Err(_) if !folder.exists() => (Vec::new(), Vec::new(), Some("It is no longer there.".to_string())),
        Err(error) => (Vec::new(), Vec::new(), Some(format!("It cannot be read: {error}."))),
    };
    OwnFolder { game: game.to_string(), folder: folder.display().to_string(), trouble, packs, faults }
}

/// A game's name as the page shows it, without its release: "Tales of Phantasia". A game this
/// version has no adapter for is named by the id its packs give.
fn title(game: &str) -> String {
    match ventris::adapters::by_id(game) {
        Some(adapter) => adapter.name().split(" (").next().unwrap_or(adapter.name()).to_string(),
        None => game.to_string(),
    }
}

/// Names in a sentence: "A", "A and B", "A, B and C".
fn joined(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Check that a folder can be taken as the user's own packs folder for a game: it can be read, and
/// holds a pack for that game, faulty or not. When it holds none, the error says so in words, and
/// names the games the packs it does hold are for, so that a folder chosen on the wrong game's page
/// is understood.
pub fn check_own(game: &str, folder: &Path) -> Result<(), String> {
    let catalogue = read(folder)?;
    let packs = catalogue.packs.iter().map(|pack| Some(pack.game.as_str()));
    let games: Vec<Option<&str>> = packs.chain(catalogue.faults.iter().map(|fault| fault.game.as_deref())).collect();
    if games.iter().any(|its| belongs(*its, game)) {
        return Ok(());
    }
    let mut others: Vec<String> = Vec::new();
    for name in games.iter().flatten().map(|other| title(other)) {
        if !others.contains(&name) {
            others.push(name);
        }
    }
    let missing = format!("There is no pack for {} in {}.", title(game), folder.display());
    Err(match games.len() {
        0 => format!("{missing} No folder in it has a pack.toml, the file that makes a folder a pack."),
        1 => format!("{missing} The pack in it is for {}.", joined(&others)),
        _ => format!("{missing} The packs in it are for {}.", joined(&others)),
    })
}

/// List the packs in the folders the user chose for their games, by game id, and in the copy fetched
/// from GitHub, if there is one. A folder that cannot be read is said to be so, and the rest is
/// still listed.
pub fn listing(own: &BTreeMap<String, String>, fetched: Option<&Path>, checked: u64) -> Listing {
    let own = own.iter().map(|(game, folder)| own_folder(game, Path::new(folder))).collect();
    let mut listed = Listing { own, fetched: false, checked: 0, packs: Vec::new(), faults: Vec::new() };
    // The copy is only missing for a moment while a new one takes its place; it is then as if
    // there were none yet.
    if let Some(Ok(copy)) = fetched.map(read) {
        listed.fetched = true;
        listed.checked = checked;
        listed.packs = copy.packs;
        listed.faults = copy.faults;
    }
    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(folder: &Path, name: &str, content: &str) {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join(name), content).unwrap();
    }

    fn manifest(game: &str, language: &str) -> String {
        format!(
            "format = 1\ngame = \"{game}\"\nlanguage = \"{language}\"\nname = \"Trial\"\nversion = \"0.1\"\n\
             authors = [\"Ana\", \"Rui\"]\ndescription = \"Six strings.\"\n"
        )
    }

    /// A pack whose `pack.toml` is right, so that its game is known, and whose one file names a
    /// part of the script no game has.
    fn faulty(folder: &Path, game: &str, language: &str) {
        write(folder, "pack.toml", &manifest(game, language));
        write(folder, "99999.txt", "[99999.000]\nNowhere\n\n");
    }

    #[test]
    fn packs_are_found_wherever_they_are_and_faulty_ones_are_set_apart() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let english = root.join("tod2-ps2").join("en");
        write(&english, "pack.toml", &manifest("tod2-ps2", "en"));
        write(&english, "06816.txt", "[06816.4136]\nStart game\n\n[06816.4224]\nChain casting\n\n");
        // A pack's own folders are not searched for more packs.
        write(&english.join("notes"), "pack.toml", "not a manifest");
        let french = root.join("top-sfc").join("fr");
        write(&french, "pack.toml", &manifest("top-sfc", "fr"));
        write(&french, "01.txt", "[01.000]\nBonjour\n\n");
        // A game this program has no adapter for, a hidden folder, and a folder that is no pack.
        write(&root.join("other").join("en"), "pack.toml", &manifest("no-such-game", "en"));
        write(&root.join(".git").join("en"), "pack.toml", &manifest("top-sfc", "en"));
        write(&root.join("tod2-ps2"), "README.md", "not a pack");

        let catalogue = read(root).unwrap();
        let found: Vec<(&str, &str, &str)> = catalogue
            .packs
            .iter()
            .map(|pack| (pack.place.as_str(), pack.game.as_str(), pack.language.as_str()))
            .collect();
        assert_eq!(found, [("tod2-ps2/en", "tod2-ps2", "en"), ("top-sfc/fr", "top-sfc", "fr")]);
        let english = &catalogue.packs[0];
        assert_eq!(english.changes, [Count { kind: "menus".to_string(), strings: 2 }]);
        assert_eq!((english.authors.len(), english.names), (2, 0));
        assert_eq!(english.description.as_deref(), Some("Six strings."));
        assert!(Path::new(&english.folder).join("pack.toml").is_file());

        assert_eq!(catalogue.faults.len(), 1);
        assert_eq!(catalogue.faults[0].place, "other/en");
        assert_eq!(catalogue.faults[0].game.as_deref(), Some("no-such-game"));
        assert!(catalogue.faults[0].message.contains("a game this version of ventris does not know"));
    }

    /// A translator's pack with a mistake in it is not left out: it is listed with the reason
    /// `ventris describe` gives, and with its game whenever its `pack.toml` says which.
    #[test]
    fn a_faulty_pack_is_listed_with_its_reason_and_its_game() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let mistyped = root.join("mine").join("fr");
        write(&mistyped, "pack.toml", &manifest("top-sfc", "fr"));
        write(
            &mistyped,
            "99.txt",
            "[999.000]
Bonjour

",
        );
        let broken = root.join("mine").join("pt");
        write(
            &broken,
            "pack.toml",
            "format = 1
game = \"top-sfc\"
",
        );

        let catalogue = read(root).unwrap();
        assert!(catalogue.packs.is_empty());
        let faults: Vec<(&str, Option<&str>)> =
            catalogue.faults.iter().map(|fault| (fault.place.as_str(), fault.game.as_deref())).collect();
        assert_eq!(faults, [("mine/fr", Some("top-sfc")), ("mine/pt", None)]);
        for (fault, folder) in catalogue.faults.iter().zip([&mistyped, &broken]) {
            let described = ventris::tasks::describe(folder).err().unwrap().to_string();
            assert_eq!(fault.message, described);
        }
    }

    #[test]
    fn a_single_pack_and_a_missing_folder() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "pack.toml", &manifest("top-sfc", "en"));
        let catalogue = read(root.path()).unwrap();
        assert_eq!(catalogue.packs.len(), 1);
        assert_eq!(catalogue.packs[0].place, ".");
        assert!(catalogue.packs[0].changes.is_empty());

        let missing = root.path().join("nowhere");
        assert_eq!(read(&missing).unwrap_err(), format!("{} is not a folder", missing.display()));
    }

    /// A folder chosen for one game gives that game's packs only, marked as the user's own, and the
    /// fetched packs are listed beside it. A folder that has gone away is said to be so, and the
    /// fetched packs are still listed.
    #[test]
    fn a_games_own_folder_gives_its_packs_and_the_fetched_ones_are_listed_beside() {
        let root = tempfile::tempdir().unwrap();
        let fetched = root.path().join("fetched");
        write(&fetched.join("top-sfc").join("en"), "pack.toml", &manifest("top-sfc", "en"));
        write(&fetched.join("tod2-ps2").join("en"), "pack.toml", &manifest("tod2-ps2", "en"));
        write(&fetched.join("new-game").join("en"), "pack.toml", &manifest("new-game", "en"));
        // A folder with packs for both games, and two faulty ones: one for a game this version does
        // not know, and one whose pack.toml cannot be read far enough to say which game it is for.
        let own = root.path().join("mine");
        write(&own.join("fr"), "pack.toml", &manifest("top-sfc", "fr"));
        write(&own.join("pt"), "pack.toml", &manifest("tod2-ps2", "pt"));
        write(&own.join("de"), "pack.toml", "format = 1\n");
        write(&own.join("es"), "pack.toml", &manifest("new-game", "es"));
        let gone = root.path().join("gone");
        let chosen = BTreeMap::from([
            ("top-sfc".to_string(), own.display().to_string()),
            ("tod2-ps2".to_string(), gone.display().to_string()),
        ]);

        let listed = listing(&chosen, Some(&fetched), 1_790_000_000);
        assert_eq!((listed.fetched, listed.checked), (true, 1_790_000_000));
        let fetched_packs: Vec<(&str, bool)> =
            listed.packs.iter().map(|pack| (pack.place.as_str(), pack.from_folder)).collect();
        assert_eq!(fetched_packs, [("tod2-ps2/en", false), ("top-sfc/en", false)]);
        assert_eq!(listed.faults.iter().map(|fault| fault.place.as_str()).collect::<Vec<_>>(), ["new-game/en"]);

        let [destiny, phantasia] = &listed.own[..] else { panic!("{:?}", listed.own) };
        assert_eq!(
            (phantasia.game.as_str(), phantasia.folder.as_str(), &phantasia.trouble),
            ("top-sfc", own.to_str().unwrap(), &None)
        );
        let taken: Vec<(&str, &str, bool)> =
            phantasia.packs.iter().map(|pack| (pack.place.as_str(), pack.game.as_str(), pack.from_folder)).collect();
        assert_eq!(taken, [("fr", "top-sfc", true)]);
        let faults: Vec<(&str, Option<&str>, bool)> = phantasia
            .faults
            .iter()
            .map(|fault| (fault.place.as_str(), fault.game.as_deref(), fault.from_folder))
            .collect();
        assert_eq!(faults, [("de", None, true)]);

        assert_eq!(destiny.game, "tod2-ps2");
        assert_eq!(destiny.trouble.as_deref(), Some("It is no longer there."));
        // A file where the folder was can be told apart.
        fs::write(&gone, "not a folder").unwrap();
        let file_there = own_folder("tod2-ps2", &gone);
        assert_eq!(file_there.trouble, Some(format!("It cannot be read: {} is not a folder.", gone.display())));
        assert!(destiny.packs.is_empty() && destiny.faults.is_empty());

        // No folder of the user's and no copy fetched yet.
        let nothing = listing(&BTreeMap::new(), Some(&root.path().join("not-fetched")), 5);
        assert!(!nothing.fetched && nothing.checked == 0 && nothing.packs.is_empty() && nothing.own.is_empty());
    }

    /// A folder is taken for a game only when it holds a pack for that game, faulty or not. When it
    /// holds none, the reason says which games its packs are for.
    #[test]
    fn a_folder_with_no_pack_for_the_game_is_refused_in_words() {
        let root = tempfile::tempdir().unwrap();
        let phantasia = root.path().join("top-sfc").join("en");
        write(&phantasia, "pack.toml", &manifest("top-sfc", "en"));
        assert_eq!(check_own("top-sfc", &phantasia), Ok(()));
        assert_eq!(
            check_own("tod2-ps2", &phantasia),
            Err(format!(
                "There is no pack for Tales of Destiny 2 in {}. The pack in it is for Tales of Phantasia.",
                phantasia.display()
            ))
        );

        // Several packs, for two games, one of them unknown and one faulty.
        write(&root.path().join("top-sfc").join("fr"), "pack.toml", &manifest("top-sfc", "fr"));
        write(&root.path().join("other").join("en"), "pack.toml", &manifest("new-game", "en"));
        faulty(&root.path().join("top-sfc").join("pt"), "top-sfc", "pt");
        assert_eq!(
            check_own("tod2-ps2", root.path()),
            Err(format!(
                "There is no pack for Tales of Destiny 2 in {}. The packs in it are for Tales of Phantasia and new-game.",
                root.path().display()
            ))
        );
        // A faulty pack for the game is taken: its author will want to see why it cannot be used.
        let mine = root.path().join("mine");
        faulty(&mine.join("fr"), "top-sfc", "fr");
        let found = read(&mine).unwrap();
        assert!(found.packs.is_empty() && found.faults[0].game.as_deref() == Some("top-sfc"));
        assert_eq!(check_own("top-sfc", &mine), Ok(()));
        // So is one whose pack.toml cannot be read far enough to say which game it is for.
        let unreadable = root.path().join("unreadable");
        write(&unreadable, "pack.toml", "not toml at all [");
        assert_eq!(check_own("tod2-ps2", &unreadable), Ok(()));

        let empty = root.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(
            check_own("top-sfc", &empty),
            Err(format!(
                "There is no pack for Tales of Phantasia in {}. No folder in it has a pack.toml, the file that makes a \
                 folder a pack.",
                empty.display()
            ))
        );
        let missing = root.path().join("nowhere");
        assert_eq!(check_own("top-sfc", &missing), Err(format!("{} is not a folder", missing.display())));
    }
}
