//! What a user does with a game and a pack, whichever program they do it from.
//!
//! The command line prints what these return, and the app shows it in a window. Neither holds
//! any of the steps itself, so the two cannot come to differ in what they build or refuse.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use crate::adapter::{Adapter, BuildOptions, Release, TextKind};
use crate::error::{Error, Result};
use crate::manifest::{self, Manifest};
use crate::platform::{Image, Platform};
use crate::script::{Script, read_pack, write_pack_with};
use crate::{adapters, platforms};

/// A path as a report shows it: with the separators of the system it is on, and without the parts
/// that change nothing (`./`, a doubled or a final separator). The Python reference prints paths
/// this way, and the two print the same reports.
pub fn shown(path: &Path) -> String {
    let tidy: PathBuf = path.components().filter(|part| !matches!(part, Component::CurDir)).collect();
    if tidy.as_os_str().is_empty() { ".".to_string() } else { tidy.display().to_string() }
}

/// Open a game file and find its adapter: the image as the adapter takes it, the adapter, the SHA-1.
///
/// Which console the file is for, and so how it is opened, is for the platforms to say.
pub fn open_game(path: &Path) -> Result<(Image, &'static dyn Adapter, String)> {
    let unknown = || Error::UnknownGame(PathBuf::from(shown(path)));
    let platform = platforms::recognise(path)?.ok_or_else(unknown)?;
    let (image, sha1) = platform.open(path)?;
    let adapter = adapters::find(&sha1).filter(|adapter| std::ptr::addr_eq(adapter.platform(), platform));
    Ok((image, adapter.ok_or_else(unknown)?, sha1))
}

/// How much of one body of text a script holds.
pub struct KindCount {
    pub kind: &'static TextKind,
    pub strings: usize,
    /// The sections those strings are in.
    pub sections: usize,
}

/// A report's first word and its colon, as wide as the others: "dialogue: ".
pub fn label(name: &str) -> String {
    format!("{:<10}", format!("{name}:"))
}

impl KindCount {
    /// How many strings of this body of text there are, as a report's line: "dialogue: 1215 strings
    /// in 6 sections".
    pub fn line(&self) -> String {
        let spread = if self.sections == 1 { String::new() } else { format!(" in {} sections", self.sections) };
        format!("{} {} strings{spread}", label(self.kind.name), self.strings)
    }
}

/// A script's strings counted by the body of text they belong to, in the adapter's order. A kind
/// the script has nothing of is there with zeros.
pub fn count_by_kind(adapter: &dyn Adapter, texts: &Script) -> Vec<KindCount> {
    let count = |kind: &'static TextKind| {
        let of_this_kind = |section: &str| adapter.kind_of(section).is_some_and(|found| std::ptr::eq(found, kind));
        let sections: Vec<usize> =
            texts.iter().filter(|(section, _)| of_this_kind(section)).map(|(_, items)| items.len()).collect();
        KindCount { kind, strings: sections.iter().sum(), sections: sections.len() }
    };
    adapter.kinds().iter().map(count).collect()
}

/// What an extraction wrote.
pub struct Extracted {
    pub adapter: &'static dyn Adapter,
    /// The strings written, by body of text.
    pub kinds: Vec<KindCount>,
    /// How many characters have no entry in the adapter's table yet, and how many codes they use.
    pub unmapped: usize,
    pub codes: usize,
    /// How many files were written, one per section.
    pub files: usize,
}

impl Extracted {
    /// What was written, as lines to show: the game, the strings of each body of text, and the
    /// characters that have no table entry yet.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!("game:      {}", self.adapter.name())];
        lines.extend(self.kinds.iter().map(KindCount::line));
        lines.push(format!(
            "unmapped:  {} characters using {} codes that have no table entry yet",
            self.unmapped, self.codes
        ));
        lines
    }
}

/// Write a game's script to a folder as UTF-8 text, one file per section, and check it: the files
/// are read back and encoded again, so that a lossy step can never go unnoticed. The folder is
/// made if it is not there.
pub fn extract(game: &Path, output: &Path) -> Result<Extracted> {
    let (image, adapter, _) = open_game(game)?;
    let script = adapter.extract(&image)?;
    write_pack_with(output, &script, &|section, number| adapter.note(section, number))?;
    adapter.verify(&image, &read_pack(output)?.texts)?;

    let strings = script.texts.values().flat_map(|items| items.values());
    let unmapped: Vec<String> = strings.flat_map(|text| adapter.unmapped(text)).collect();
    let codes = unmapped.iter().collect::<BTreeSet<_>>().len();
    let kinds = count_by_kind(adapter, &script.texts);
    Ok(Extracted { adapter, kinds, unmapped: unmapped.len(), codes, files: script.texts.len() })
}

/// Fail unless a pack's folder is there: one that is not is a mistyped path, not a pack that
/// changes nothing.
fn pack_folder(folder: &Path) -> Result<()> {
    if folder.is_dir() {
        Ok(())
    } else {
        Err(Error::Script(format!("{} is not a folder of script files", shown(folder))))
    }
}

/// What a pack is, as far as can be told without the game.
pub struct Described {
    pub manifest: Manifest,
    pub adapter: &'static dyn Adapter,
    /// The strings it changes, by body of text.
    pub kinds: Vec<KindCount>,
    /// How many of the game's characters it renames.
    pub names: usize,
}

impl Described {
    /// The fonts a build of the pack draws new characters from, and whether they are the game's
    /// own and not ones the pack names. `None` when neither names any.
    pub fn fonts(&self) -> Option<(Vec<String>, bool)> {
        let named = &self.manifest.build.fonts;
        if !named.is_empty() {
            return Some((named.clone(), false));
        }
        let own = self.adapter.fonts();
        (!own.is_empty()).then(|| (own.iter().map(|font| font.to_string()).collect(), true))
    }

    /// The same for the text drawn in small tiles.
    pub fn menu_font(&self) -> Option<(String, bool)> {
        match (&self.manifest.build.menu_font, self.adapter.menu_font()) {
            (Some(font), _) => Some((font.clone(), false)),
            (None, Some(font)) => Some((font.to_string(), true)),
            (None, None) => None,
        }
    }
}

/// Say what a pack is, and check what can be checked of it with no game at hand: this is all that
/// can be done where packs are kept, since the games are not kept there.
pub fn describe(folder: &Path) -> Result<Described> {
    pack_folder(folder)?;
    let Some(manifest) = manifest::read(folder)? else {
        let file = manifest::MANIFEST_FILE;
        return Err(Error::Script(format!("{} has no {file}, the file that says what the pack is", shown(folder))));
    };
    let adapter = manifest.adapter()?;
    let pack = read_pack(folder)?;
    let options = manifest.build_options(folder, &BuildOptions::default())?;
    adapter.accepts(&pack, &options)?;
    if let Some(section) = pack.texts.keys().find(|section| adapter.kind_of(section).is_none()) {
        return Err(Error::Text(format!("{section} is not a part of this game's script")));
    }
    let kinds = count_by_kind(adapter, &pack.texts);
    Ok(Described { manifest, adapter, kinds, names: pack.names.len() })
}

/// What a build did.
pub struct Report {
    pub adapter: &'static dyn Adapter,
    /// The pack's manifest, when it has one.
    pub manifest: Option<Manifest>,
    /// What the adapter changed, as lines to show.
    pub summary: Vec<String>,
    /// What changed about the image as a whole, as lines to show.
    pub changes: Vec<String>,
    /// The SHA-1 of the image that was written.
    pub sha1: String,
    /// Whether that image is the same as the game's own, as it is when a pack changes nothing.
    pub identical: bool,
}

/// Build a game file from the game's own and a pack.
///
/// The pack's manifest, when it has one, says what the build needs; `options` is what the user
/// asked for on top of it. The game's own file is never written to.
pub fn build(game: &Path, pack: &Path, output: &Path, options: &BuildOptions) -> Result<Report> {
    pack_folder(pack)?;
    // The manifest is read before the game is: a disc takes a while to recognise.
    let manifest = manifest::read(pack)?;
    if let (Ok(source), Ok(target)) = (fs::canonicalize(game), fs::canonicalize(output))
        && source == target
    {
        return Err(Error::Image(format!(
            "{} is the game's own file, and a build does not write over it",
            shown(output)
        )));
    }
    let (image, adapter, sha1) = open_game(game)?;
    let options = match &manifest {
        Some(manifest) => {
            manifest.check_game(adapter)?;
            manifest.build_options(pack, options)?
        }
        None => options.clone(),
    };
    let summary = adapter.build(&image, &read_pack(pack)?, output, &options)?.summary;
    // The image is read back from the file, so that what is reported is what was written.
    let (built, built_sha1) = adapter.platform().open(output)?;
    let changes = adapter.platform().changes(&image, &built);
    let identical = built_sha1 == sha1;
    Ok(Report { adapter, manifest, summary, changes, sha1: built_sha1, identical })
}

/// A file in a folder of games that is the size of a game there is an adapter for. It has been
/// sized up and not read: whether it is that game is known once `identify` has read it.
pub struct Candidate {
    pub path: PathBuf,
    /// The file's own size and when it was last changed. With its path they say whether it is still
    /// the file it was, so that one read before need not be read again.
    pub file_size: u64,
    pub modified: Option<SystemTime>,
    platform: &'static dyn Platform,
    /// The games it could be, going by its size.
    pub could_be: Vec<&'static dyn Adapter>,
}

/// A game's own file, found.
#[derive(Clone)]
pub struct Found {
    pub adapter: &'static dyn Adapter,
    pub release: &'static Release,
    pub path: PathBuf,
}

impl Candidate {
    /// Read the file and say which game it is, or `None` when it only happens to have a game's size.
    pub fn identify(&self) -> Result<Option<Found>> {
        let (_, sha1) = self.platform.open(&self.path)?;
        let releases = self.could_be.iter().flat_map(|adapter| adapter.images().iter().map(|image| (*adapter, image)));
        let found = releases.into_iter().find(|(_, release)| release.sha1 == sha1);
        Ok(found.map(|(adapter, release)| Found { adapter, release, path: self.path.clone() }))
    }
}

/// How many folders deep a folder of games is searched.
const SEARCH_DEPTH: usize = 8;

/// The games on a console that have an image of this size.
fn games_of_size(platform: &'static dyn Platform, size: u64) -> Vec<&'static dyn Adapter> {
    let on_it = |adapter: &&'static dyn Adapter| std::ptr::addr_eq(adapter.platform(), platform);
    let of_size = |adapter: &&'static dyn Adapter| adapter.images().iter().any(|image| image.size == size);
    adapters::ALL.iter().copied().filter(on_it).filter(of_size).collect()
}

fn search(folder: &Path, depth: usize, found: &mut Vec<Candidate>) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let hidden = path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'));
        let Ok(about) = fs::metadata(&path) else { continue };
        if about.is_dir() {
            if depth > 0 && !hidden {
                search(&path, depth - 1, found);
            }
            continue;
        }
        for platform in platforms::ALL.iter().copied() {
            // A file that cannot be sized up, such as a damaged archive, is passed over.
            let Ok(Some(glance)) = platform.glance(&path) else { continue };
            let could_be = games_of_size(platform, glance.size);
            if !could_be.is_empty() {
                let (file_size, modified) = (about.len(), about.modified().ok());
                found.push(Candidate { path: path.clone(), file_size, modified, platform, could_be });
                break;
            }
        }
    }
}

/// The files in a folder of games, and in the folders inside it, that are the size of a game there
/// is an adapter for, in the order of their names.
///
/// This is the quick half of finding someone's games. No file is read, only its name and its size,
/// and for an archive the list of what it holds; a collection of thousands of games takes a moment.
/// What is left is the few files to read whole, and a caller that has read one before can tell it
/// is the same file and not read it again.
pub fn candidates(folder: &Path) -> Result<Vec<Candidate>> {
    if !folder.is_dir() {
        return Err(Error::Image(format!("{} is not a folder", shown(folder))));
    }
    let mut found = Vec::new();
    search(folder, SEARCH_DEPTH, &mut found);
    Ok(found)
}

/// The games there are adapters for that are in a folder: each game once, as the first file found
/// to be it. A file that cannot be read is passed over.
pub fn find_games(folder: &Path) -> Result<Vec<Found>> {
    Ok(identify(&candidates(folder)?))
}

/// The games among some candidates: each game once, as the first file found to be it. Once a game
/// is found, the other files of its size are not read.
pub fn identify(candidates: &[Candidate]) -> Vec<Found> {
    let mut games: Vec<Found> = Vec::new();
    for candidate in candidates {
        let wanted = |adapter: &&dyn Adapter| !games.iter().any(|found| std::ptr::addr_eq(found.adapter, *adapter));
        if !candidate.could_be.iter().any(wanted) {
            continue;
        }
        if let Ok(Some(found)) = candidate.identify()
            && !games.iter().any(|known| std::ptr::addr_eq(known.adapter, found.adapter))
        {
            games.push(found);
        }
    }
    games
}

/// The name of the image a game's file holds, which is the file's own unless it is an archive.
pub fn image_name(path: &Path) -> Option<String> {
    let glances = platforms::ALL.iter().filter_map(|platform| platform.glance(path).ok().flatten());
    glances.map(|glance| glance.name).next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive;
    use crate::testing;

    /// A file of `size` bytes that no adapter knows: it has a game's size and is not the game.
    fn filler(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 253) as u8).collect()
    }

    fn write(folder: &Path, name: &str, data: &[u8]) -> PathBuf {
        let path = folder.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }

    #[test]
    fn only_files_the_size_of_a_known_game_are_candidates() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        let six = filler(0x600000); // the size of Tales of Phantasia's cartridge
        // Kept as it is, with the bytes a copier adds, and in an archive beside a note.
        write(root, "snes/b plain.sfc", &six);
        write(root, "snes/c copier.smc", &[vec![0; 512], six.clone()].concat());
        write(
            root,
            "snes/a zipped.zip",
            &archive::make(&[("read me.txt", b"hi", false), ("Game (Japan).sfc", &six, true)]),
        );
        // Not candidates: another size, another kind of file, two cartridges in one archive, a
        // damaged archive, and a folder whose name starts with a dot.
        write(root, "snes/other size.sfc", &filler(0x80000));
        write(root, "snes/notes.txt", &six);
        write(root, "snes/two.zip", &archive::make(&[("one.sfc", &six, true), ("two.sfc", &six, true)]));
        write(root, "snes/damaged.zip", b"not an archive");
        write(root, ".hidden/plain.sfc", &six);
        // A disc's ending with a cartridge's size is no disc game's size.
        write(root, "ps2/small.iso", &six);

        let found = candidates(root).unwrap();
        let names: Vec<String> =
            found.iter().map(|candidate| candidate.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a zipped.zip", "b plain.sfc", "c copier.smc"]);
        for candidate in &found {
            let ids: Vec<&str> = candidate.could_be.iter().map(|adapter| adapter.id()).collect();
            assert_eq!(ids, ["top-sfc"]);
            assert!(candidate.modified.is_some());
            // It has the size of the game and is not the game.
            assert!(candidate.identify().unwrap().is_none());
        }
        assert_eq!(found[0].file_size, fs::metadata(&found[0].path).unwrap().len());
        assert!(find_games(root).unwrap().is_empty());

        assert_eq!(image_name(&found[0].path).as_deref(), Some("Game (Japan).sfc"));
        assert_eq!(image_name(&found[2].path).as_deref(), Some("c copier.smc"));
        assert_eq!(image_name(&root.join("snes/notes.txt")), None);

        let missing = root.join("nowhere");
        let refused = candidates(&missing).err().unwrap().to_string();
        assert_eq!(refused, format!("{} is not a folder", shown(&missing)));
    }

    /// With the game at hand: it is found however it is kept, once, and opens to the same image.
    #[test]
    fn a_game_is_found_in_a_folder_however_it_is_kept() {
        let Some(rom) = testing::cartridge("top-sfc") else { return };
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        write(root, "a look-alike.sfc", &filler(rom.len()));
        let zipped = write(
            root,
            "snes/Tales of Phantasia (Japan).zip",
            &archive::make(&[("Tales of Phantasia (Japan).sfc", &rom, true)]),
        );
        write(root, "snes/with a copier's bytes.smc", &[vec![0; 512], rom.clone()].concat());

        assert_eq!(candidates(root).unwrap().len(), 3);
        let games = find_games(root).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!((games[0].adapter.id(), games[0].release.name, &games[0].path), ("top-sfc", "Japan", &zipped));

        // The archive opens as the game does, and the copy with a copier's bytes is the same game.
        let (image, adapter, sha1) = open_game(&zipped).unwrap();
        assert!(image.cartridge().unwrap() == rom && adapter.id() == "top-sfc");
        let (_, _, other) = open_game(&root.join("snes/with a copier's bytes.smc")).unwrap();
        assert_eq!(sha1, other);
    }
}
