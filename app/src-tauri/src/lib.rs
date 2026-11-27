//! The ventris app: a window over the engine, for someone who has a game and wants it translated.
//!
//! The window is a web page (under `app/src`), and this is what it can ask for. Each `command`
//! below is one question or one job: which games there are, which of them the user has, which
//! packs there are, build this pack into that game, save that game's script. The work itself is the
//! engine's, through `ventris::tasks`, so the app builds and extracts exactly what the command line
//! does.
//!
//! The app holds no game and fetches none. The user says where they keep their games, the folder
//! is searched for the ones there are adapters for (`library`), and the built game is written where
//! they say.
//!
//! What the app does fetch is the translation packs, which are text: at start it gets the ventris
//! packs from their repository on GitHub and keeps a copy (`fetch`). A folder the user chose on a
//! game's page, such as one with a pack they are writing for it, adds that game's packs from it
//! (`catalogue`).

pub mod catalogue;
pub mod fetch;
pub mod library;
pub mod settings;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::Manager;
use ventris::adapter::{Adapter, BuildOptions};
use ventris::{adapters, fonts, manifest, tasks};

use catalogue::Listing;
use settings::{GameFile, Settings};

/// One body of text of a game, and what a pack can do with it.
#[derive(Serialize)]
struct Kind {
    name: &'static str,
    /// A pack can change its strings.
    insert: bool,
    /// A changed string may be longer than the game's own.
    grow: bool,
    new_characters: bool,
    max_length: Option<usize>,
    limits: &'static str,
}

/// A release of a game that the engine reads, in full, so that a player can check a file against it.
#[derive(Serialize)]
struct Release {
    sha1: &'static str,
    /// "Japan, version 1.01"
    name: &'static str,
    /// Its size in bytes.
    size: u64,
    /// What the image says it is: its serial for a disc, its title for a cartridge, and its version.
    label: &'static str,
    version: &'static str,
    /// What `label` is, as a word to show before it: "serial", "title".
    label_kind: &'static str,
    /// Where a catalogue of dumps lists it, when one does.
    catalogue: Option<Catalogued>,
}

/// A release as a catalogue of dumps lists it.
#[derive(Serialize)]
struct Catalogued {
    /// The catalogue: "Redump".
    name: &'static str,
    /// What it is, for someone who has never heard of it: "the catalogue of disc dumps".
    about: &'static str,
    /// The release's name there: "Tales of Destiny 2 (Japan, Asia)".
    entry: &'static str,
    /// The address of its record there, which the page opens in the browser. A record is facts about
    /// a dump; the catalogues host no games.
    page: Option<&'static str>,
}

/// A game the engine has an adapter for.
#[derive(Serialize)]
struct Game {
    id: &'static str,
    name: &'static str,
    platform: &'static str,
    source_language: &'static str,
    releases: Vec<Release>,
    kinds: Vec<Kind>,
    /// Whether a pack can be built into it yet, or its script only be extracted.
    can_insert: bool,
    sets_names: bool,
    /// What limits a name a pack gives its characters, in a phrase like a kind's `limits`; empty
    /// when nothing is said.
    name_limits: &'static str,
}

fn game(adapter: &dyn Adapter) -> Game {
    let kind = |kind: &ventris::adapter::TextKind| Kind {
        name: kind.name,
        insert: kind.insert,
        grow: kind.grow,
        new_characters: kind.new_characters,
        max_length: kind.max_length,
        limits: kind.limits,
    };
    Game {
        id: adapter.id(),
        name: adapter.name(),
        platform: adapter.platform().name(),
        source_language: adapter.source_language(),
        releases: adapter
            .images()
            .iter()
            .map(|image| Release {
                sha1: image.sha1,
                name: image.name,
                size: image.size,
                label: image.label,
                version: image.version,
                label_kind: adapter.platform().label_kind(),
                catalogue: image.catalogue.map(|entry| Catalogued {
                    name: entry.catalogue.name,
                    about: entry.catalogue.about,
                    entry: entry.name,
                    page: entry.page,
                }),
            })
            .collect(),
        kinds: adapter.kinds().iter().map(kind).collect(),
        can_insert: adapter.can_insert(),
        sets_names: adapter.sets_names(),
        name_limits: adapter.name_limits(),
    }
}

/// The games the engine has an adapter for.
#[tauri::command]
fn games() -> Vec<Game> {
    adapters::ALL.iter().map(|adapter| game(*adapter)).collect()
}

/// Run work that reads or writes a whole game away from the window, which would otherwise stop
/// answering until it is done.
async fn away<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|error| error.to_string())?
}

/// Held while the settings file is read, changed and written, so that two changes cannot cross.
static SETTINGS: Mutex<()> = Mutex::new(());

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let folder = app.path().app_config_dir().map_err(|error| error.to_string())?;
    Ok(folder.join("settings.json"))
}

/// Change what is remembered, and give back all of it. When the change fails, nothing is written.
fn change_settings(file: &Path, change: impl FnOnce(&mut Settings) -> Result<(), String>) -> Result<Settings, String> {
    let _held = SETTINGS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = settings::load(file);
    change(&mut settings)?;
    settings::save(file, &settings)?;
    Ok(settings)
}

/// Change what is remembered, and give back all of it.
fn remember(file: &Path, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    change_settings(file, |settings| {
        change(settings);
        Ok(())
    })
}

/// What is remembered. A game's file that is no longer where it was is left out.
///
/// This reads nothing but the settings, so that the window has something to show at once;
/// `search_games` then brings it in line with what the games folder holds now.
#[tauri::command]
fn remembered(app: tauri::AppHandle) -> Result<Settings, String> {
    let mut settings = settings::load(&settings_file(&app)?);
    settings.games.retain(|_, game| Path::new(&game.path).is_file());
    Ok(settings)
}

/// What is remembered after a search, and what the games folder holds of each game it was not
/// found in, which is said on that game's page and not remembered.
#[derive(Debug, Serialize)]
struct Searched {
    #[serde(flatten)]
    settings: Settings,
    /// For each game not found, sentences that start with a file's path from the games folder.
    near: BTreeMap<String, Vec<String>>,
}

/// Search the games folder, remember what it holds, and give back all that is remembered. With no
/// games folder chosen, only the files the user chose themselves are kept, while they are there.
///
/// The settings are read, and the result written, while they are held, so that a file the user
/// chooses for a game during the search is not lost when the search writes what it found.
fn search_and_remember(file: &Path, folder: Option<String>) -> Result<Searched, String> {
    let mut near = BTreeMap::new();
    let settings = change_settings(file, |settings| {
        let folder = folder.or(settings.games_folder.take());
        let before = std::mem::take(&mut settings.games);
        settings.games = match &folder {
            Some(folder) => {
                let looked = library::search(Path::new(folder), &before)?;
                near = looked.near;
                looked.games
            }
            None => before.into_iter().filter(|(_, game)| Path::new(&game.path).is_file()).collect(),
        };
        settings.games_folder = folder;
        Ok(())
    })?;
    Ok(Searched { settings, near })
}

/// Look through the games folder again.
#[tauri::command]
async fn search_games(app: tauri::AppHandle) -> Result<Searched, String> {
    let file = settings_file(&app)?;
    away(move || search_and_remember(&file, None)).await
}

/// Take a folder as the place the user keeps their games, and search it.
#[tauri::command]
async fn choose_games_folder(app: tauri::AppHandle, folder: String) -> Result<Searched, String> {
    let file = settings_file(&app)?;
    away(move || search_and_remember(&file, Some(folder))).await
}

/// Where the copy of the packs fetched from GitHub is kept: in the folder the system gives the app
/// for data it can get again.
fn packs_store(app: &tauri::AppHandle) -> Result<fetch::Store, String> {
    let folder = app.path().app_cache_dir().map_err(|error| error.to_string())?;
    Ok(fetch::Store::new(&folder))
}

/// Every pack there is on this computer now: those in the folders the user chose for their games,
/// and the copy fetched from GitHub, if there is one. Nothing is fetched.
fn list_packs(settings_file: &Path, store: &fetch::Store) -> Listing {
    let own = settings::load(settings_file).own_packs;
    let kept = store.kept();
    catalogue::listing(
        &own,
        kept.as_ref().map(|kept| kept.folder.as_path()),
        kept.as_ref().map_or(0, |kept| kept.checked),
    )
}

/// The packs on this computer, at once and without the network.
#[tauri::command]
async fn packs(app: tauri::AppHandle) -> Result<Listing, String> {
    let (file, store) = (settings_file(&app)?, packs_store(&app)?);
    away(move || Ok(list_packs(&file, &store))).await
}

/// Fetch the packs from GitHub, keep them, and give every pack there is after it. On failure the
/// copy kept before is left as it was, and the error says why in words.
#[tauri::command]
async fn refresh_packs(app: tauri::AppHandle) -> Result<Listing, String> {
    let (file, store) = (settings_file(&app)?, packs_store(&app)?);
    away(move || {
        store.refresh(&fetch::packs_source()?)?;
        Ok(list_packs(&file, &store))
    })
    .await
}

/// Take a folder as the one a game's own packs are read from, if it holds a pack for that game, and
/// remember it for that game alone. A folder with no pack for the game is not remembered, and the
/// error says why in words.
fn choose_own(settings_file: &Path, game: &str, folder: String) -> Result<(), String> {
    if adapters::by_id(game).is_none() {
        return Err(format!("{game} is not a game this version of ventris knows"));
    }
    catalogue::check_own(game, Path::new(&folder))?;
    remember(settings_file, |settings| {
        settings.own_packs.insert(game.to_string(), folder);
    })?;
    Ok(())
}

/// Stop reading a game's own packs from the folder chosen for it.
fn forget_own(settings_file: &Path, game: &str) -> Result<(), String> {
    remember(settings_file, |settings| {
        settings.own_packs.remove(game);
    })?;
    Ok(())
}

/// Take a folder as the one a game's own packs are read from, and give every pack there is after it.
#[tauri::command]
async fn choose_own_packs(app: tauri::AppHandle, game: String, folder: String) -> Result<Listing, String> {
    let (file, store) = (settings_file(&app)?, packs_store(&app)?);
    away(move || {
        choose_own(&file, &game, folder)?;
        Ok(list_packs(&file, &store))
    })
    .await
}

/// Stop reading a game's own packs from the folder chosen for it, and give the packs there are after.
#[tauri::command]
async fn forget_own_packs(app: tauri::AppHandle, game: String) -> Result<Listing, String> {
    let (file, store) = (settings_file(&app)?, packs_store(&app)?);
    away(move || {
        forget_own(&file, &game)?;
        Ok(list_packs(&file, &store))
    })
    .await
}

/// A game's file, recognised.
#[derive(Debug, Serialize)]
struct Recognised {
    /// The id of the game's adapter.
    game: String,
    file: GameFile,
}

/// Which game a file is, by its SHA-1. Fails, in words to show, when it is none the engine knows.
fn recognise(path: &Path) -> Result<Recognised, String> {
    let (_, adapter, sha1) = tasks::open_game(path).map_err(|error| error.to_string())?;
    let release = adapter.images().iter().find(|image| image.sha1 == sha1).map_or("", |image| image.name);
    Ok(Recognised { game: adapter.id().to_string(), file: library::game_file(path, release, &sha1, true) })
}

/// Say which game a file the user chose is, and remember it as that game's file. This is the
/// second way to give a game, for one that is not in the games folder.
#[tauri::command]
async fn recognise_game(app: tauri::AppHandle, path: String) -> Result<Recognised, String> {
    let file = settings_file(&app)?;
    away(move || {
        let recognised = recognise(Path::new(&path))?;
        remember(&file, |settings| {
            settings.games.insert(recognised.game.clone(), recognised.file.clone());
        })?;
        Ok(recognised)
    })
    .await
}

/// Forget the file the user chose for a game, and go back to what the games folder holds.
#[tauri::command]
async fn forget_game(app: tauri::AppHandle, game: String) -> Result<Searched, String> {
    let file = settings_file(&app)?;
    away(move || {
        remember(&file, |settings| {
            settings.games.remove(&game);
        })?;
        search_and_remember(&file, None)
    })
    .await
}

/// The characters Unicode classes as format characters (its category Cf): they draw nothing, and
/// some change how the text around them is shown, as U+202E turns what follows right to left, so
/// that a file name could seem to end other than it does.
const FORMAT_CHARACTERS: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0001}', '\u{E0001}'),
    ('\u{E0020}', '\u{E007F}'),
];

fn is_format_character(c: char) -> bool {
    FORMAT_CHARACTERS.iter().any(|(first, last)| (*first..=*last).contains(&c))
}

/// A name as a file or folder can carry it on any system: characters some system refuses become
/// `_`, and control and format characters, which draw nothing, are left out.
fn safe_name(name: &str) -> String {
    let kept = name.chars().filter(|c| !c.is_control() && !is_format_character(*c));
    let clean: String = kept.map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
    clean.trim().to_string()
}

/// The most characters of a pack's label in the name of a built game: a pack can call itself
/// anything, and a file's name has a limit.
const MAX_LABEL: usize = 64;

/// A name for the built game, beside the game's own file and never the same as it:
/// `game (Pack 1.0, en).iso`. The label, from the pack, is made safe and cut to `MAX_LABEL`
/// characters.
///
/// `image` is the name of the image the file holds. It differs from the file's own when the game
/// is kept in an archive, and the built game takes its ending: a game is built, not an archive.
fn output_beside(game: &Path, image: &str, label: &str) -> PathBuf {
    let stem = game.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let label: String = safe_name(label).chars().take(MAX_LABEL).collect();
    let name = format!("{stem} ({})", label.trim_end());
    match Path::new(image).extension() {
        Some(extension) => game.with_file_name(format!("{name}.{}", extension.to_string_lossy())),
        None => game.with_file_name(name),
    }
}

/// Where to offer to write the game built from a pack.
#[tauri::command]
fn suggest_output(game: String, pack: String) -> Result<String, String> {
    let manifest = manifest::read(Path::new(&pack)).map_err(|error| error.to_string())?;
    let label = manifest.map_or("translated".to_string(), |manifest| {
        format!("{} {}, {}", manifest.name, manifest.version, manifest.language)
    });
    let game = Path::new(&game);
    let own_name = || game.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let image = tasks::image_name(game).unwrap_or_else(own_name);
    Ok(output_beside(game, &image, &label).display().to_string())
}

/// What a build did, to show.
#[derive(Debug, Serialize)]
struct Built {
    output: String,
    /// What changed, as the engine reports it.
    lines: Vec<String>,
    sha1: String,
    /// Whether the built game is the same as the game's own, as when a pack changes nothing.
    identical: bool,
}

fn build_into(game: &Path, pack: &Path, output: &Path) -> Result<Built, String> {
    let report = tasks::build(game, pack, output, &BuildOptions::default()).map_err(|error| error.to_string())?;
    let lines = report.summary.into_iter().chain(report.changes).collect();
    Ok(Built { output: output.display().to_string(), lines, sha1: report.sha1, identical: report.identical })
}

/// Build the game with a pack's strings in it, to a file the user chose.
#[tauri::command]
async fn build(game: String, pack: String, output: String) -> Result<Built, String> {
    away(move || build_into(Path::new(&game), Path::new(&pack), Path::new(&output))).await
}

/// What saving a game's script wrote, to show.
#[derive(Debug, Serialize)]
struct Saved {
    /// The folder the script was written to.
    folder: String,
    /// How many files it holds, one per section.
    files: usize,
    /// What was written, as the engine reports it.
    lines: Vec<String>,
}

/// The folder a game's script is saved in, inside the one the user chose: "Tales of Phantasia
/// script". A folder of its own keeps the script's files from mixing with whatever else is there.
fn script_folder(chosen: &Path, game_name: &str) -> PathBuf {
    let title = game_name.split(" (").next().unwrap_or(game_name);
    chosen.join(format!("{} script", safe_name(title)))
}

fn save_script_into(game: &str, file: &Path, chosen: &Path) -> Result<Saved, String> {
    let adapter = adapters::by_id(game).ok_or_else(|| format!("{game} is not a game this version of ventris knows"))?;
    if !chosen.is_dir() {
        return Err(format!("{} is not a folder", chosen.display()));
    }
    let folder = script_folder(chosen, adapter.name());
    // A folder of that name with files in it may be a translation the user is working on, made
    // from a script saved before: it is never written over.
    let in_use = fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_some());
    if in_use {
        return Err(format!(
            "{} is already there and is not empty, so nothing was written. Choose another folder, or move \
             that one away first.",
            folder.display()
        ));
    }
    let extracted = tasks::extract(file, &folder).map_err(|error| error.to_string())?;
    Ok(Saved { folder: folder.display().to_string(), files: extracted.files, lines: extracted.lines() })
}

/// Save a game's own script, as text to translate from, in a folder inside the one the user chose.
/// It is written by the same step as `ventris extract`, and checked the same way.
#[tauri::command]
async fn save_script(game: String, file: String, folder: String) -> Result<Saved, String> {
    away(move || save_script_into(&game, Path::new(&file), Path::new(&folder))).await
}

/// A font that is built into the program, and the terms it is passed on under.
#[derive(Serialize)]
struct FontTerms {
    name: &'static str,
    about: &'static str,
    license: &'static str,
    terms: &'static str,
}

#[derive(Serialize)]
struct About {
    version: &'static str,
    fonts: Vec<FontTerms>,
}

/// What the program is and what it carries: the fonts' terms ask to be kept with every copy.
#[tauri::command]
fn about() -> About {
    let terms = |font: &fonts::BuiltIn| FontTerms {
        name: font.name,
        about: font.about,
        license: font.license,
        terms: font.terms,
    };
    About { version: env!("CARGO_PKG_VERSION"), fonts: fonts::BUILT_IN.iter().map(terms).collect() }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            games,
            remembered,
            search_games,
            choose_games_folder,
            packs,
            refresh_packs,
            choose_own_packs,
            forget_own_packs,
            recognise_game,
            forget_game,
            suggest_output,
            build,
            save_script,
            about,
        ])
        .run(tauri::generate_context!())
        .expect("the app could not start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_game_is_listed_with_what_a_pack_can_change() {
        let listed = games();
        let ids: Vec<&str> = listed.iter().map(|game| game.id).collect();
        assert_eq!(ids, ["top-sfc", "tod2-ps2", "srwz-ps2", "goemon-nds", "god-ps1"]);
        // A game that can only be extracted so far is listed too, with nothing a pack can change.
        let extract_only = &listed[3];
        assert_eq!((extract_only.platform, extract_only.can_insert), ("Nintendo DS", false));
        let last = &listed[1];
        assert_eq!((last.platform, last.source_language, last.can_insert), ("PlayStation 2", "ja", true));
        assert!(last.kinds.iter().any(|kind| kind.name == "menus" && kind.insert && kind.grow));
        assert!(!last.releases.is_empty() && last.releases[0].sha1.len() == 40);
        // The release in full, for the page to state it.
        let release = &last.releases[0];
        assert_eq!(
            (release.label_kind, release.label, release.version, release.size),
            ("serial", "SLPS-25172", "1.01", 3_234_004_992)
        );
        assert_eq!(listed[0].releases[0].label_kind, "title");
        let redump = release.catalogue.as_ref().unwrap();
        assert_eq!((redump.name, redump.entry), ("Redump", "Tales of Destiny 2 (Japan, Asia)"));
        assert_eq!(redump.page, Some("http://redump.org/disc/15921/"));
        assert!(!last.sets_names && last.name_limits.is_empty());
        // The page shows what a name may be beside the row for names.
        let phantasia = &listed[0];
        assert!(phantasia.sets_names && phantasia.name_limits.starts_with("a name is 1 to 15 characters"));
    }

    #[test]
    fn the_built_game_is_named_beside_the_game_and_never_as_it() {
        let game = Path::new("games").join("tod2.iso");
        let beside = |name: &str| Path::new("games").join(name);
        assert_eq!(output_beside(&game, "tod2.iso", "Trial 0.1, en"), beside("tod2 (Trial 0.1, en).iso"));
        // What a file name cannot hold is replaced, so a pack's name cannot send the file elsewhere.
        assert_eq!(output_beside(&game, "tod2.iso", "a/b\\c: \"d\"? "), beside("tod2 (a_b_c_ _d__).iso"));
        assert_eq!(output_beside(Path::new("rom"), "rom", "x"), Path::new("rom (x)"));
        // A game kept in an archive is built as a game: it takes the ending of the image inside.
        let zipped = Path::new("games").join("Game (Japan).zip");
        assert_eq!(output_beside(&zipped, "Game (Japan).sfc", "Trial 1, en"), beside("Game (Japan) (Trial 1, en).sfc"));
    }

    /// A pack's label is cut to a sane length, on a character, and what draws nothing is left out:
    /// a right-to-left mark cannot make the name seem to end in something else.
    #[test]
    fn a_packs_label_in_the_built_games_name_is_short_and_shows_what_it_is() {
        let game = Path::new("games").join("tod2.iso");
        let named =
            |label: &str| output_beside(&game, "tod2.iso", label).file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(named("Trial\u{202E}osi.exe 1, en"), "tod2 (Trialosi.exe 1, en).iso");
        assert_eq!(named("A\u{200B}\u{FEFF}\u{2066}B\u{0007}\u{0085}C\u{E0041}, en"), "tod2 (ABC, en).iso");
        // 70 kana, three bytes each: 64 of them are kept, whole.
        let long = "あ".repeat(70);
        assert_eq!(named(&long), format!("tod2 ({}).iso", "あ".repeat(64)));
        assert_eq!(named(&format!("{} x", "a".repeat(63))), format!("tod2 ({}).iso", "a".repeat(63)));
        assert_eq!(
            named(&format!("{}, version 1.0, en", "Translation ".repeat(10))),
            "tod2 (Translation Translation Translation Translation Translation Tran).iso"
        );
        // The game's own name is not cut: it is the user's file.
        let long_game = Path::new("games").join(format!("{}.iso", "g".repeat(100)));
        let built = output_beside(&long_game, "game.iso", "Trial");
        assert_eq!(built.file_name().unwrap().to_string_lossy(), format!("{} (Trial).iso", "g".repeat(100)));
        assert_eq!(script_folder(Path::new("w"), "A\u{202E}B (X)"), Path::new("w").join("AB script"));
    }

    #[test]
    fn a_script_is_saved_in_a_folder_named_after_the_game() {
        let chosen = Path::new("work");
        let named = |name: &str| script_folder(chosen, name);
        assert_eq!(named("Tales of Phantasia (Super Famicom, Japan)"), chosen.join("Tales of Phantasia script"));
        assert_eq!(named("A/B: C"), chosen.join("A_B_ C script"));
    }

    #[test]
    fn a_script_is_not_saved_over_a_folder_in_use_or_where_there_is_no_folder() {
        let folder = tempfile::tempdir().unwrap();
        let game = folder.path().join("game.sfc");
        // A folder of the script's name that holds something may be someone's translation.
        let in_use = folder.path().join("Tales of Phantasia script");
        fs::create_dir_all(&in_use).unwrap();
        fs::write(in_use.join("01.txt"), "[01.000]\nMy own words\n\n").unwrap();
        let refused = save_script_into("top-sfc", &game, folder.path()).unwrap_err();
        assert!(refused.contains("is already there and is not empty, so nothing was written"), "{refused}");
        assert_eq!(fs::read_to_string(in_use.join("01.txt")).unwrap(), "[01.000]\nMy own words\n\n");

        let missing = folder.path().join("nowhere");
        let refused = save_script_into("top-sfc", &game, &missing).unwrap_err();
        assert_eq!(refused, format!("{} is not a folder", missing.display()));
        let refused = save_script_into("no-such-game", &game, folder.path()).unwrap_err();
        assert_eq!(refused, "no-such-game is not a game this version of ventris knows");
    }

    #[test]
    fn what_is_remembered_is_changed_one_thing_at_a_time() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("settings.json");
        remember(&file, |settings| settings.games_folder = Some("games".to_string())).unwrap();
        let game = library::game_file(Path::new("top.sfc"), "Japan", "ab", true);
        let kept = remember(&file, |settings| {
            settings.games.insert("top-sfc".to_string(), game.clone());
        })
        .unwrap();
        assert_eq!(kept.games_folder.as_deref(), Some("games"));
        assert_eq!(kept.games["top-sfc"], game);
        assert_eq!(settings::load(&file), kept);
    }

    fn pack_manifest(game: &str, language: &str) -> String {
        format!(
            "format = 1\ngame = \"{game}\"\nlanguage = \"{language}\"\nname = \"Trial\"\nversion = \"1\"\n\
             authors = [\"Ana\"]\n"
        )
    }

    fn write_pack(folder: &Path, game: &str, language: &str) {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("pack.toml"), pack_manifest(game, language)).unwrap();
    }

    /// A folder chosen on one game's page is remembered for that game alone, and only that game's
    /// packs are taken from it; the other game is left as it was. Forgetting it leaves the fetched
    /// packs, and a folder that has gone away is said to be so while the fetched packs stay listed.
    #[test]
    fn a_games_own_folder_is_kept_for_that_game_alone_and_forgotten() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("settings.json");
        let store = fetch::Store::new(&folder.path().join("cache"));
        let copy = folder.path().join("cache").join("packs").join("copy-1");
        write_pack(&copy.join("top-sfc").join("en"), "top-sfc", "en");
        write_pack(&copy.join("tod2-ps2").join("en"), "tod2-ps2", "en");
        store.adopt("copy-1", 1);
        // A folder with a pack for each game, as a copy of the packs repository would be.
        let own = folder.path().join("my packs");
        write_pack(&own.join("top-sfc").join("fr"), "top-sfc", "fr");
        write_pack(&own.join("tod2-ps2").join("pt"), "tod2-ps2", "pt");
        remember(&file, |settings| settings.games_folder = Some("games".to_string())).unwrap();

        choose_own(&file, "top-sfc", own.display().to_string()).unwrap();
        let kept = settings::load(&file);
        assert_eq!(kept.own_packs.keys().collect::<Vec<_>>(), ["top-sfc"]);
        assert_eq!(kept.games_folder.as_deref(), Some("games"));
        let listed = list_packs(&file, &store);
        let [phantasia] = &listed.own[..] else { panic!("{:?}", listed.own) };
        let taken: Vec<(&str, &str)> =
            phantasia.packs.iter().map(|pack| (pack.game.as_str(), pack.language.as_str())).collect();
        assert_eq!((phantasia.game.as_str(), taken), ("top-sfc", vec![("top-sfc", "fr")]));
        assert!(phantasia.packs[0].from_folder);
        assert_eq!((listed.fetched, listed.packs.len()), (true, 2));
        assert!(listed.packs.iter().all(|pack| !pack.from_folder));

        // Gone: said so, and the fetched packs are still there.
        let moved = folder.path().join("moved");
        fs::rename(&own, &moved).unwrap();
        let listed = list_packs(&file, &store);
        assert_eq!(listed.own[0].trouble.as_deref(), Some("It is no longer there."));
        assert_eq!(listed.packs.len(), 2);

        forget_own(&file, "top-sfc").unwrap();
        let listed = list_packs(&file, &store);
        assert!(listed.own.is_empty() && listed.packs.len() == 2);
        assert!(settings::load(&file).own_packs.is_empty());
        assert_eq!(
            choose_own(&file, "no-game", moved.display().to_string()).unwrap_err(),
            "no-game is not a game this version of ventris knows"
        );
    }

    /// A folder with no pack for the game it was chosen for is not remembered, and the reason names
    /// the game its packs are for.
    #[test]
    fn a_folder_with_no_pack_for_the_game_is_not_remembered() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("settings.json");
        let phantasia = folder.path().join("top-sfc").join("en");
        write_pack(&phantasia, "top-sfc", "en");
        let refused = choose_own(&file, "tod2-ps2", phantasia.display().to_string()).unwrap_err();
        assert_eq!(
            refused,
            format!(
                "There is no pack for Tales of Destiny 2 in {}. The pack in it is for Tales of Phantasia.",
                phantasia.display()
            )
        );
        assert!(settings::load(&file).own_packs.is_empty());
        // One chosen before is kept when another choice for the same game is refused.
        choose_own(&file, "top-sfc", phantasia.display().to_string()).unwrap();
        assert!(choose_own(&file, "top-sfc", folder.path().join("nowhere").display().to_string()).is_err());
        assert_eq!(settings::load(&file).own_packs["top-sfc"], phantasia.display().to_string());
    }

    /// A settings file from when one packs folder served every game: everything else in it is kept,
    /// that folder is not read for any game, and the old key is gone at the next save.
    #[test]
    fn a_settings_file_with_the_old_packs_folder_still_loads() {
        let folder = tempfile::tempdir().unwrap();
        let own = folder.path().join("my packs");
        write_pack(&own, "top-sfc", "fr");
        let file = folder.path().join("settings.json");
        let game = library::game_file(Path::new("top.zip"), "Japan", "ab", false);
        let saved = serde_json::json!({ "games_folder": "games", "packs_folder": own.display().to_string(), "games": { "top-sfc": game } });
        fs::write(&file, saved.to_string()).unwrap();

        let store = fetch::Store::new(&folder.path().join("cache"));
        let listed = list_packs(&file, &store);
        assert!(listed.own.is_empty() && listed.packs.is_empty());
        let kept = settings::load(&file);
        assert_eq!((kept.games_folder.as_deref(), &kept.games["top-sfc"]), (Some("games"), &game));

        choose_own(&file, "top-sfc", own.display().to_string()).unwrap();
        let written = fs::read_to_string(&file).unwrap();
        assert!(!written.contains("packs_folder") && written.contains("own_packs"), "{written}");
        assert_eq!(settings::load(&file).games["top-sfc"], game);
    }

    #[test]
    fn a_file_that_is_no_game_is_said_to_be_none() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("notes.sfc");
        fs::write(&file, vec![0u8; 0x8000]).unwrap();
        let refused = recognise(&file).unwrap_err();
        assert!(refused.ends_with("is not a game ventris knows how to read yet"), "{refused}");
        // A file that can be told what it is is told in the command line's words.
        let chd = folder.path().join("game.chd");
        fs::write(&chd, b"MComprHD and the rest").unwrap();
        let refused = recognise(&chd).unwrap_err();
        assert_eq!(refused, tasks::open_game(&chd).err().unwrap().to_string());
        assert!(refused.contains("game.chd is a CHD file, a compressed disc image. "), "{refused}");
    }

    /// With the game at hand: the app recognises it and builds the same image as the command line's
    /// own step does, and refuses to write over the game's file. Without the game it checks nothing.
    #[test]
    fn a_pack_is_built_into_a_game_as_the_command_line_builds_it() {
        let Some(rom) = ventris::testing::game("top-sfc") else { return };
        let recognised = recognise(&rom).unwrap();
        assert_eq!((recognised.game.as_str(), recognised.file.sha1.len()), ("top-sfc", 40));
        assert!(!recognised.file.release.is_empty() && recognised.file.chosen && recognised.file.file_size > 0);

        let folder = tempfile::tempdir().unwrap();
        let pack = folder.path().join("pack");
        fs::create_dir_all(&pack).unwrap();
        let manifest = "format = 1\ngame = \"top-sfc\"\nlanguage = \"en\"\nname = \"Trial\"\nversion = \"1\"\n\
                        authors = [\"Ana\"]\n";
        fs::write(pack.join("pack.toml"), manifest).unwrap();
        fs::write(pack.join("01.txt"), "[01.000]\nIt reads: hello.\n\n").unwrap();

        let output = folder.path().join("built.sfc");
        let built = build_into(&rom, &pack, &output).unwrap();
        assert!(!built.identical && !built.lines.is_empty() && built.sha1.len() == 40);
        let by_engine = folder.path().join("by-engine.sfc");
        tasks::build(&rom, &pack, &by_engine, &BuildOptions::default()).unwrap();
        assert!(fs::read(&output).unwrap() == fs::read(&by_engine).unwrap());

        let before = fs::read(&rom).unwrap();
        let refused = build_into(&rom, &pack, &rom).unwrap_err();
        assert!(refused.ends_with("is the game's own file, and a build does not write over it"), "{refused}");
        assert!(fs::read(&rom).unwrap() == before);
    }

    /// The files in a folder, by name, with what each holds.
    fn files(folder: &Path) -> Vec<(String, Vec<u8>)> {
        let mut found: Vec<(String, Vec<u8>)> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .map(|path| (path.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&path).unwrap()))
            .collect();
        found.sort();
        found
    }

    /// With the game at hand: the script the app saves is the one the command line's own step
    /// writes, file for file, and saving it a second time into the same place is refused.
    #[test]
    fn a_games_script_is_saved_as_the_command_line_extracts_it() {
        let Some(rom) = ventris::testing::game("top-sfc") else { return };
        let folder = tempfile::tempdir().unwrap();
        let chosen = folder.path().join("chosen");
        fs::create_dir_all(&chosen).unwrap();
        let saved = save_script_into("top-sfc", &rom, &chosen).unwrap();
        let written = chosen.join("Tales of Phantasia script");
        assert_eq!(saved.folder, written.display().to_string());
        assert!(saved.lines[0].starts_with("game:") && saved.lines.iter().any(|line| line.starts_with("dialogue:")));

        let by_engine = folder.path().join("by-engine");
        let extracted = tasks::extract(&rom, &by_engine).unwrap();
        let (ours, theirs) = (files(&written), files(&by_engine));
        assert!(saved.files == extracted.files && !ours.is_empty());
        assert!(ours == theirs, "the saved script differs from what the command line writes");

        let refused = save_script_into("top-sfc", &rom, &chosen).unwrap_err();
        assert!(refused.contains("is already there and is not empty"), "{refused}");
    }
}

#[cfg(test)]
mod search_tests {
    use ventris::{archive, testing};

    use super::*;

    /// With the game at hand: choosing a games folder finds it, a file the user chose is kept over
    /// what the folder holds, and forgetting that choice goes back to the folder.
    #[test]
    fn the_games_folder_is_searched_and_a_choice_by_hand_comes_first() {
        let Some(rom) = testing::cartridge("top-sfc") else { return };
        let folder = tempfile::tempdir().unwrap();
        let settings = folder.path().join("settings.json");
        let games = folder.path().join("games");
        let zipped = games.join("snes").join("Tales of Phantasia (Japan).zip");
        fs::create_dir_all(zipped.parent().unwrap()).unwrap();
        fs::write(&zipped, archive::make(&[("Tales of Phantasia (Japan).sfc", &rom, true)])).unwrap();
        let elsewhere = folder.path().join("elsewhere.sfc");
        fs::write(&elsewhere, &rom).unwrap();

        // Nothing chosen yet: nothing is known, and nothing fails.
        assert_eq!(search_and_remember(&settings, None).unwrap().settings, Settings::default());

        let found = search_and_remember(&settings, Some(games.display().to_string())).unwrap().settings;
        assert_eq!(found.games_folder.as_deref(), games.to_str());
        assert_eq!(
            (found.games["top-sfc"].path.as_str(), found.games["top-sfc"].chosen),
            (zipped.to_str().unwrap(), false)
        );
        assert_eq!(settings::load(&settings), found);
        // The built game is offered beside the archive, as a cartridge.
        let image = tasks::image_name(&zipped).unwrap();
        let offered = output_beside(&zipped, &image, "Trial 1, en");
        assert_eq!(offered, games.join("snes").join("Tales of Phantasia (Japan) (Trial 1, en).sfc"));

        // A file chosen by hand takes the game's place, and a later search leaves it there.
        let chosen = recognise(&elsewhere).unwrap();
        remember(&settings, |kept| {
            kept.games.insert(chosen.game.clone(), chosen.file.clone());
        })
        .unwrap();
        let searched = search_and_remember(&settings, None).unwrap().settings;
        assert_eq!(searched.games["top-sfc"].path, elsewhere.to_str().unwrap());
        // Once it is gone, the folder's own is back.
        fs::remove_file(&elsewhere).unwrap();
        let searched = search_and_remember(&settings, None).unwrap().settings;
        assert_eq!(searched.games["top-sfc"].path, zipped.to_str().unwrap());
    }
}
