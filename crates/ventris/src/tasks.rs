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
use crate::platform::{Image, Platform, same_platform};
use crate::script::{Script, read_pack, write_pack_with};
use crate::{adapters, platforms, refusal};

/// A path as a report shows it: with the separators of the system it is on, and without the parts
/// that change nothing (`./`, a doubled or a final separator). The Python reference prints paths
/// this way, and the two print the same reports.
pub fn shown(path: &Path) -> String {
    let tidy: PathBuf = path.components().filter(|part| !matches!(part, Component::CurDir)).collect();
    if tidy.as_os_str().is_empty() { ".".to_string() } else { tidy.display().to_string() }
}

/// Open a game file and find its adapter: the image as the adapter takes it, the adapter, the SHA-1.
///
/// Which console the file is for, and so how it is opened, is for the platforms to say. A file no
/// adapter reads is refused with what it is and what is needed instead, where that can be told
/// (`refusal`).
pub fn open_game(path: &Path) -> Result<(Image, &'static dyn Adapter, String)> {
    let Some(platform) = platforms::recognise(path)? else {
        return Err(refusal::unrecognised(path));
    };
    let (image, sha1) = platform.open(path)?;
    let adapter = adapters::find(&sha1).filter(|adapter| same_platform(adapter.platform(), platform));
    match adapter {
        Some(adapter) => Ok((image, adapter, sha1)),
        None => Err(refusal::refused(path, platform, &image)),
    }
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

/// A file in a folder of games that is not the file a game's adapter reads, and is that game or says
/// it is: another release, one with a known patch applied, or the release by what it says with other
/// contents. A search names these for a game it does not find, so that a wrong copy is not passed
/// over in silence.
#[derive(Clone)]
pub struct Passed {
    pub path: PathBuf,
    pub adapter: &'static dyn Adapter,
    /// Why it is not read, to follow the file's name: "is Tales of Destiny 2, but not the release
    /// ventris reads: ...". It is the reason a file chosen by hand is refused with.
    pub reason: String,
}

/// What reading a candidate found: the game, or what the file says it is when it is not the game.
enum Examined {
    Game(Found),
    Not(Option<Passed>),
}

impl Candidate {
    /// Read the file and say which game it is, or `None` when it only happens to have a game's size.
    pub fn identify(&self) -> Result<Option<Found>> {
        Ok(match self.examine()? {
            Examined::Game(found) => Some(found),
            Examined::Not(_) => None,
        })
    }

    /// Read the file: the game it is, or, when it is not, whether it says it is a game there is an
    /// adapter for. What it says is read from the image that was opened to hash it, so it costs
    /// nothing more.
    fn examine(&self) -> Result<Examined> {
        let (image, sha1) = self.platform.open(&self.path)?;
        let releases = self.could_be.iter().flat_map(|adapter| adapter.images().iter().map(|image| (*adapter, image)));
        if let Some((adapter, release)) = releases.into_iter().find(|(_, release)| release.sha1 == sha1) {
            return Ok(Examined::Game(Found { adapter, release, path: self.path.clone() }));
        }
        let passed = self.platform.label(&image).and_then(|label| {
            let (adapter, reason) = refusal::near_miss(self.platform, &label, image.size().ok()?)?;
            Some(Passed { path: self.path.clone(), adapter, reason })
        });
        Ok(Examined::Not(passed))
    }
}

/// How many folders deep a folder of games is searched.
const SEARCH_DEPTH: usize = 8;

/// The endings of files that may be a compressed disc image, whose first bytes are read to tell.
const DISC_ENDINGS: [&str; 6] = ["chd", "cso", "zso", "iso", "bin", "img"];

/// A file larger than this is no cartridge, and its first bytes are read too, whatever its ending:
/// a compressed disc image is sometimes given a name that does not say so.
const LARGER_THAN_A_CARTRIDGE: u64 = 16 << 20;

/// The games on a console that have an image of this size.
fn games_of_size(platform: &'static dyn Platform, size: u64) -> Vec<&'static dyn Adapter> {
    let on_it = |adapter: &&'static dyn Adapter| same_platform(adapter.platform(), platform);
    let of_size = |adapter: &&'static dyn Adapter| adapter.images().iter().any(|image| image.size == size);
    adapters::ALL.iter().copied().filter(on_it).filter(of_size).collect()
}

/// A file a search may look at more closely when a game is not found: one that could be an image
/// for a console, of no game's size, whose few bytes that say what it is can be read
/// (`Platform::peek`); or a compressed disc image.
enum Glimpse {
    Peek { path: PathBuf, platform: &'static dyn Platform, size: u64 },
    CompressedDisc(PathBuf),
}

/// What a folder of games holds, sized up: the candidates, and the files to look at more closely
/// for a game that turns out not to be there.
pub struct Survey {
    pub folder: PathBuf,
    pub candidates: Vec<Candidate>,
    glimpses: Vec<Glimpse>,
}

fn search(folder: &Path, depth: usize, survey: &mut Survey) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let hidden = path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'));
        let Ok(about) = fs::metadata(&path) else { continue };
        if about.is_dir() {
            if depth > 0 && !hidden {
                search(&path, depth - 1, survey);
            }
            continue;
        }
        let ending = path.extension().map(|ending| ending.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if (DISC_ENDINGS.contains(&ending.as_str()) || about.len() > LARGER_THAN_A_CARTRIDGE)
            && refusal::is_compressed_disc(&path)
        {
            survey.glimpses.push(Glimpse::CompressedDisc(path));
            continue;
        }
        let own_name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let mut glimpse = None;
        for platform in platforms::ALL.iter().copied() {
            // A file that cannot be sized up, such as a damaged archive, is passed over.
            let Ok(Some(glance)) = platform.glance(&path) else { continue };
            let could_be = games_of_size(platform, glance.size);
            if !could_be.is_empty() {
                let (file_size, modified) = (about.len(), about.modified().ok());
                survey.candidates.push(Candidate { path: path.clone(), file_size, modified, platform, could_be });
                glimpse = None;
                break;
            }
            // An image inside an archive is not looked at: that would mean unpacking it.
            if glimpse.is_none() && glance.name == own_name {
                glimpse = Some(Glimpse::Peek { path: path.clone(), platform, size: glance.size });
            }
        }
        survey.glimpses.extend(glimpse);
    }
}

/// What a folder of games, and the folders inside it, holds: the files that are the size of a game
/// there is an adapter for, in the order of their names, and the files to look at more closely for
/// a game that is not found.
///
/// This is the quick half of finding someone's games. No file is read but for a few bytes: its name
/// and its size, for an archive the list of what it holds, and for a file with a disc image's ending
/// or larger than any cartridge its first 8 bytes, which say whether it is a compressed disc image.
/// A collection of thousands of games takes a moment. What is left is the few files to read whole,
/// and a caller that has read one before can tell it is the same file and not read it again.
pub fn survey(folder: &Path) -> Result<Survey> {
    if !folder.is_dir() {
        return Err(Error::Image(format!("{} is not a folder", shown(folder))));
    }
    let mut survey = Survey { folder: folder.to_path_buf(), candidates: Vec::new(), glimpses: Vec::new() };
    search(folder, SEARCH_DEPTH, &mut survey);
    Ok(survey)
}

/// The files in a folder of games that are the size of a game there is an adapter for (`survey`).
pub fn candidates(folder: &Path) -> Result<Vec<Candidate>> {
    Ok(survey(folder)?.candidates)
}

/// The games there are adapters for that are in a folder: each game once, as the first file found
/// to be it. A file that cannot be read is passed over.
pub fn find_games(folder: &Path) -> Result<Vec<Found>> {
    Ok(identify(&candidates(folder)?))
}

/// The games among some candidates: each game once, as the first file found to be it. Once a game
/// is found, the other files of its size are not read.
pub fn identify(candidates: &[Candidate]) -> Vec<Found> {
    identify_noting(candidates).0
}

/// The same, and the candidates that were read and are not the game they have the size of, but say
/// they are a game there is an adapter for: a near miss, read at no cost beyond the reading.
pub fn identify_noting<'a>(candidates: impl IntoIterator<Item = &'a Candidate>) -> (Vec<Found>, Vec<Passed>) {
    let mut games: Vec<Found> = Vec::new();
    let mut passed = Vec::new();
    for candidate in candidates {
        let wanted = |adapter: &&dyn Adapter| !games.iter().any(|found| found.adapter.id() == adapter.id());
        if !candidate.could_be.iter().any(wanted) {
            continue;
        }
        match candidate.examine() {
            Ok(Examined::Game(found)) if !games.iter().any(|known| known.adapter.id() == found.adapter.id()) => {
                games.push(found)
            }
            Ok(Examined::Not(Some(near))) => passed.push(near),
            _ => {}
        }
    }
    (games, passed)
}

/// What a folder holds of a game that is not found in it: the files that are that game, or say they
/// are, and are not the file needed; and, for a disc game, the compressed disc images, which could
/// be any disc game.
pub struct Missed {
    pub adapter: &'static dyn Adapter,
    pub passed: Vec<Passed>,
    /// The compressed disc images, by their path from the folder searched.
    pub compressed: Vec<String>,
}

impl Missed {
    /// What it holds of the game as sentences, each starting with a file's path from the folder
    /// searched: the files passed over, then the compressed disc images, a few of them by name.
    pub fn lines(&self, folder: &Path) -> Vec<String> {
        let mut lines: Vec<String> = self
            .passed
            .iter()
            .map(|passed| format!("{} {}", from_folder(folder, &passed.path), passed.reason))
            .collect();
        lines.extend(refusal::compressed_note(&self.compressed));
        lines
    }

    pub fn is_empty(&self) -> bool {
        self.passed.is_empty() && self.compressed.is_empty()
    }
}

/// A path as it is shown in a list of what a folder holds: from that folder.
fn from_folder(folder: &Path, path: &Path) -> String {
    shown(path.strip_prefix(folder).unwrap_or(path))
}

impl Survey {
    /// What the folder holds of each game in `missing`, which were not found in it: the candidates
    /// that were read and are near misses (`identify_noting`), and the files looked at more closely
    /// now. Those are read only for the consoles of the games that are missing, a few bytes each;
    /// nothing is hashed.
    pub fn missed(&self, missing: &[&'static dyn Adapter], read: &[Passed]) -> Vec<Missed> {
        let is_missing = |adapter: &dyn Adapter| missing.iter().any(|wanted| wanted.id() == adapter.id());
        let wanted_on =
            |platform: &dyn Platform| missing.iter().any(|adapter| same_platform(adapter.platform(), platform));
        let mut passed: Vec<Passed> = read.iter().filter(|near| is_missing(near.adapter)).cloned().collect();
        let mut compressed = Vec::new();
        for glimpse in &self.glimpses {
            match glimpse {
                Glimpse::Peek { path, platform, size } if wanted_on(*platform) => {
                    let near = platform.peek(path).and_then(|label| refusal::near_miss(*platform, &label, *size));
                    if let Some((adapter, reason)) = near.filter(|(adapter, _)| is_missing(*adapter)) {
                        passed.push(Passed { path: path.clone(), adapter, reason });
                    }
                }
                Glimpse::CompressedDisc(path) => compressed.push(from_folder(&self.folder, path)),
                Glimpse::Peek { .. } => {}
            }
        }
        passed.sort_by(|one, other| one.path.cmp(&other.path));
        missing
            .iter()
            .map(|adapter| Missed {
                adapter: *adapter,
                passed: passed.iter().filter(|near| near.adapter.id() == adapter.id()).cloned().collect(),
                compressed: if adapter.platform().disc() { compressed.clone() } else { Vec::new() },
            })
            .collect()
    }
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

    /// A disc whose SYSTEM.CNF gives this program and version, in `folder` under `name`.
    fn disc(folder: &Path, name: &str, program: &str, version: &str) -> PathBuf {
        let config = format!("BOOT2 = cdrom0:\\{program};1\r\nVER = {version}\r\n");
        crate::platforms::ps2::tests::disc_with(folder, name, config.as_bytes())
    }

    #[test]
    fn a_game_that_is_not_found_is_told_which_files_are_it_and_why_they_are_not_read() {
        use crate::platforms::snes::Mapping;
        use crate::platforms::snes::tests::titled;

        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        fs::create_dir_all(root.join("ps2")).unwrap();
        fs::create_dir_all(root.join("snes")).unwrap();
        // Discs: another release, the release by its label with other contents, a game with no adapter.
        disc(&root.join("ps2"), "Best.iso", "SLPS_732.19", "2.01");
        let changed = disc(&root.join("ps2"), "Changed.iso", "SLPS_251.72", "1.01");
        disc(&root.join("ps2"), "Other game.iso", "SLPM_660.01", "1.00");
        // Compressed disc images, one with a name that does not say so, and archives of unknown content.
        write(root, "ps2/Game.chd", b"MComprHD\x00\x00\x00\x7c and the rest");
        write(root, "ps2/Renamed.iso", b"CISO\x18\x00\x00\x00 and the rest");
        write(root, "ps2/Packed.7z", b"7z\xBC\xAF\x27\x1C\x00\x04 and the rest");
        write(root, "ps2/Packed.zip", &archive::make(&[("game.iso", b"a disc", true)]));
        // Cartridges: DeJap's translation applied, plain and in an archive (which is not looked into),
        // and a file with the game's size and title that is not the game, which is read and hashed.
        let dejap = titled(Mapping::HiRom, 0x100000, b"ToP English (C) DeJap", 0);
        write(root, "snes/DeJap.sfc", &dejap);
        write(root, "snes/DeJap.zip", &archive::make(&[("DeJap.sfc", &dejap, true)]));
        write(root, "snes/Look-alike.sfc", &titled(Mapping::ExHiRom, 0x600000, b"TALES OF PHANTASIA", 0));
        write(root, "snes/Another game.sfc", &titled(Mapping::LoRom, 0x80000, b"ANOTHER GAME", 0));

        let survey = survey(root).unwrap();
        assert_eq!(survey.candidates.len(), 1);
        let (found, read) = identify_noting(&survey.candidates);
        assert!(found.is_empty());
        let missed = survey.missed(adapters::ALL, &read);
        let lines = |id: &str| {
            let missed = missed.iter().find(|missed| missed.adapter.id() == id).unwrap();
            missed.lines(root)
        };
        assert_eq!(fs::metadata(&changed).unwrap().len(), 53_248);
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            lines("tod2-ps2"),
            [
                format!(
                    "ps2{sep}Best.iso is Tales of Destiny 2, but not the release ventris reads: it is the PlayStation 2 \
                     the Best reissue, in Japanese (serial SLPS-73219, version 2.01)."
                ),
                format!(
                    "ps2{sep}Changed.iso says it is the release of Tales of Destiny 2 that ventris reads (serial \
                     SLPS-25172, version 1.01), but its contents are not the original's: it has been changed, by a \
                     patch such as an undub or a translation, or it is a damaged or cut-short copy. It is 53,248 bytes, \
                     and the original is 3,234,004,992."
                ),
                format!(
                    "ps2{sep}Game.chd and ps2{sep}Renamed.iso are compressed disc images (CHD, CSO or ZSO): ventris \
                     cannot read them, or tell which game they hold. If one is this game, convert it back to a plain \
                     disc image first."
                ),
            ]
        );
        assert_eq!(
            lines("top-sfc"),
            [
                format!(
                    "snes{sep}DeJap.sfc is Tales of Phantasia with DeJap's English translation already applied (title \
                     \"ToP English (C) DeJap\", version 1.0). ventris reads the game as it was released, so it needs a \
                     copy without the patch."
                ),
                format!(
                    "snes{sep}Look-alike.sfc says it is the release of Tales of Phantasia that ventris reads (title \
                     \"TALES OF PHANTASIA\", version 1.0), but its contents are not the original's: it has been \
                     changed, by a patch such as an undub or a translation, or it is a damaged or cut-short copy."
                ),
            ]
        );
        // A game that is found is not asked about, and nothing is said of a folder with nothing of it.
        let only = survey.missed(&[&crate::adapters::top_sfc::ADAPTER], &read);
        assert_eq!(only.len(), 1);
        let empty = tempfile::tempdir().unwrap();
        let nothing = super::survey(empty.path()).unwrap().missed(adapters::ALL, &[]);
        assert!(nothing.iter().all(Missed::is_empty));
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
