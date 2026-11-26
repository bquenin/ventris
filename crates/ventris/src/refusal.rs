//! Why a file is not a game ventris can read, in words a player can act on.
//!
//! A game is recognised by the SHA-1 of its image, and a file whose SHA-1 is no adapter's is
//! refused. That alone says nothing a player can use, so the refusal says what the file is, as far
//! as can be told, and what is needed instead:
//!
//! - a compressed or wrapped file (a CHD, a CSO, an archive), told by its first bytes, which has to
//!   be unpacked or converted back to the game's plain file first;
//! - another release of a game ventris knows, or one of its releases with a known patch applied,
//!   told by what the file says it is (a disc's serial, a cartridge's title) and the files each
//!   adapter lists as refused;
//! - the release ventris reads by what it says it is, with other contents: changed by a patch, or a
//!   damaged or cut-short copy;
//! - a game no adapter is for.
//!
//! What a file says it is can be read by its console's module (`Platform::label`); what an adapter
//! knows of its game's releases is in its `images` and `refuses`. A file that is none of these keeps
//! the plain refusal, `Error::UnknownGame`.
//!
//! A search of a folder of games gives the same reasons for the files it passes over (`near_miss`),
//! without the file's name in front and without what is needed after, which the page or the list
//! states once.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::adapter::{Adapter, Refused, Release};
use crate::adapters;
use crate::error::Error;
use crate::platform::{Image, Label, Platform, label_words};
use crate::tasks::shown;

/// A compressed or wrapped file, told by how it starts.
struct Wrapped {
    magic: &'static [u8],
    /// What the file is, after "is".
    what: &'static str,
    /// How to get the plain file back, as the start of a sentence.
    fix: &'static str,
    /// It holds a disc image, of which game cannot be told.
    disc: bool,
}

/// None of these is read by ventris; a zip archive that holds one cartridge image is, and is
/// recognised as the cartridge before this is asked.
const WRAPPED: &[Wrapped] = &[
    Wrapped {
        magic: b"MComprHD",
        what: "a CHD file, a compressed disc image",
        fix: "Convert it back to a plain disc image first, with chdman, the program CHD files are made with",
        disc: true,
    },
    Wrapped {
        magic: b"CISO",
        what: "a CSO file, a compressed disc image",
        fix: "Convert it back to a plain .iso first, with maxcso for example",
        disc: true,
    },
    Wrapped {
        magic: b"ZISO",
        what: "a ZSO file, a compressed disc image",
        fix: "Convert it back to a plain .iso first, with maxcso for example",
        disc: true,
    },
    Wrapped {
        magic: b"PK\x03\x04",
        what: "a zip archive that does not hold one cartridge image",
        fix: "Unpack it first",
        disc: false,
    },
    Wrapped { magic: b"7z\xBC\xAF\x27\x1C", what: "a 7z archive", fix: "Unpack it first", disc: false },
    Wrapped { magic: b"\x1f\x8b", what: "a gzip-compressed file", fix: "Unpack it first", disc: false },
    Wrapped { magic: b"Rar!", what: "a RAR archive", fix: "Unpack it first", disc: false },
];

/// What the first bytes of the file at `path` say it is wrapped in, if anything.
fn wrapped(path: &Path) -> Option<&'static Wrapped> {
    let mut start = [0; 8];
    let read = File::open(path).and_then(|mut file| file.read(&mut start)).unwrap_or(0);
    WRAPPED.iter().find(|wrapped| start[..read].starts_with(wrapped.magic))
}

/// Whether the file at `path` is a compressed disc image (CHD, CSO or ZSO), by its first bytes.
pub fn is_compressed_disc(path: &Path) -> bool {
    wrapped(path).is_some_and(|wrapped| wrapped.disc)
}

/// Why a file that no console recognises is refused: what it is when its first bytes say, and
/// otherwise the plain refusal.
pub fn unrecognised(path: &Path) -> Error {
    match wrapped(path) {
        Some(Wrapped { what, fix, .. }) => Error::Refused(format!(
            "{} is {what}. ventris reads a game's plain file, such as an .iso for a disc or the cartridge image \
             for a cartridge, and cannot read inside this one. {fix}, then use the file that comes out.",
            shown(path)
        )),
        None => Error::UnknownGame(PathBuf::from(shown(path))),
    }
}

/// Compressed disc images found in a folder of games, as a sentence: up to three names, then how
/// many more. Which game each holds cannot be told, so they are named once, for every disc game that
/// is not found.
pub fn compressed_note(names: &[String]) -> Option<String> {
    let first: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    let more = names.len() - first.len();
    let list = match (first.as_slice(), more) {
        ([], _) => return None,
        ([one], _) => one.to_string(),
        (all, 0) => format!("{} and {}", all[..all.len() - 1].join(", "), all[all.len() - 1]),
        (some, more) => format!("{} and {more} more", some.join(", ")),
    };
    Some(if names.len() == 1 {
        format!(
            "{list} is a compressed disc image (CHD, CSO or ZSO): ventris cannot read it, or tell which game it \
             holds. If it is this game, convert it back to a plain disc image first."
        )
    } else {
        format!(
            "{list} are compressed disc images (CHD, CSO or ZSO): ventris cannot read them, or tell which game they \
             hold. If one is this game, convert it back to a plain disc image first."
        )
    })
}

/// A size in bytes with its thousands apart: "3,234,004,992".
fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A game's title without the release its name gives: "Tales of Destiny 2" for
/// "Tales of Destiny 2 (PlayStation 2, Japan)".
fn title(adapter: &dyn Adapter) -> &'static str {
    let name = adapter.name();
    name.split_once(" (").map_or(name, |(title, _)| title)
}

/// Where a catalogue lists a release, as a phrase: `named "Tales of Destiny 2 (Japan, Asia)" in
/// Redump, the catalogue of disc dumps`. What the catalogue is goes with its name, for a player who
/// has never heard of it. The address of its record is not in it: the command line's `games` and
/// the app's game page give that apart.
pub fn catalogue_words(release: &Release) -> Option<String> {
    let entry = release.catalogue?;
    Some(format!("named \"{}\" in {}, {}", entry.name, entry.catalogue.name, entry.catalogue.about))
}

/// A release in full, as a player checks a file against it: `Japan, version 1.01, named "Tales of
/// Destiny 2 (Japan, Asia)" in Redump, the catalogue of disc dumps: serial SLPS-25172, 3,234,004,992
/// bytes, SHA-1 3b2b...`.
pub fn release_in_full(platform: &dyn Platform, release: &Release) -> String {
    let named = catalogue_words(release).map_or(String::new(), |words| format!(", {words}"));
    format!("{}{named}: {}", release.name, release_facts(platform, release))
}

/// What a release is, without its name or its catalogue: "serial SLPS-25172, 3,234,004,992 bytes,
/// SHA-1 3b2b...". The version is left out where the release's name already gives it, as "Japan,
/// version 1.01" does, so that it is not read twice.
pub fn release_facts(platform: &dyn Platform, release: &Release) -> String {
    let version = if release.name.contains(&format!("version {}", release.version)) { "" } else { release.version };
    let label = if release.label.is_empty() {
        String::new()
    } else {
        format!("{}, ", label_words(platform, release.label, version))
    };
    format!("{label}{} bytes, SHA-1 {}", grouped(release.size), release.sha1)
}

/// The sentence that says which release, or releases, an adapter reads.
pub fn needed(adapter: &dyn Adapter) -> String {
    let platform = adapter.platform();
    let releases: Vec<String> = adapter.images().iter().map(|release| release_in_full(platform, release)).collect();
    match releases.as_slice() {
        [one] => format!("The release ventris reads is {one}."),
        many => format!("The releases ventris reads are {}.", many.join("; and ")),
    }
}

/// What a file that a console knows is, when no adapter reads it.
enum Miss {
    /// It says it is a release that is read, and is not: changed, or damaged. `size` is its own.
    Changed { adapter: &'static dyn Adapter, release: &'static Release, size: u64 },
    /// A file the adapter knows of and refuses.
    Known { adapter: &'static dyn Adapter, refused: &'static Refused },
    /// It names a release that is read, with another version.
    OtherVersion { adapter: &'static dyn Adapter },
    /// A game no adapter is for.
    NoAdapter,
}

/// Which of those a file is, from what it says it is and its size.
fn classify(platform: &'static dyn Platform, label: &Label, size: u64) -> Miss {
    let on_it = |adapter: &&'static dyn Adapter| std::ptr::addr_eq(adapter.platform(), platform);
    let games: Vec<&'static dyn Adapter> = adapters::ALL.iter().copied().filter(on_it).collect();
    for adapter in &games {
        let same =
            adapter.images().iter().find(|release| release.label == label.name && release.version == label.version);
        if let Some(release) = same {
            return Miss::Changed { adapter: *adapter, release, size };
        }
    }
    for adapter in &games {
        let refuses = |refused: &&Refused| {
            refused.label == label.name && refused.version.is_none_or(|version| version == label.version)
        };
        if let Some(refused) = adapter.refuses().iter().find(refuses) {
            return Miss::Known { adapter: *adapter, refused };
        }
    }
    for adapter in &games {
        if adapter.images().iter().any(|release| release.label == label.name) {
            return Miss::OtherVersion { adapter: *adapter };
        }
    }
    Miss::NoAdapter
}

impl Miss {
    /// Why the file is not read, to follow its name: "is Tales of Destiny 2, but not ...".
    fn reason(&self, platform: &dyn Platform, says: &str) -> String {
        match self {
            Miss::Changed { adapter, release, size } => {
                let sizes = if *size == release.size {
                    String::new()
                } else {
                    format!(" It is {} bytes, and the original is {}.", grouped(*size), grouped(release.size))
                };
                format!(
                    "says it is the release of {} that ventris reads ({says}), but its contents are not the \
                     original's: it has been changed, by a patch such as an undub or a translation, or it is a \
                     damaged or cut-short copy.{sizes}",
                    title(*adapter)
                )
            }
            Miss::Known { adapter, refused } if refused.patched => format!(
                "is {} with {} already applied ({says}). ventris reads the game as it was released, so it needs a \
                 copy without the patch.",
                title(*adapter),
                refused.about
            ),
            Miss::Known { adapter, refused } => {
                format!("is {}, but not the release ventris reads: it is {} ({says}).", title(*adapter), refused.about)
            }
            Miss::OtherVersion { adapter } => {
                format!("is {}, but not the release ventris reads: it is another version ({says}).", title(*adapter))
            }
            Miss::NoAdapter => format!(
                "is a {} game ({says}), and ventris has no adapter for it, so it cannot read it.",
                platform.name()
            ),
        }
    }

    /// What is needed instead, as a sentence; empty for a game with no adapter.
    fn advice(&self) -> String {
        match self {
            Miss::Changed { release, .. } => {
                let named = catalogue_words(release).map_or(String::new(), |words| format!(", {words}"));
                format!("Use an unchanged copy{named}: the original's SHA-1 is {}.", release.sha1)
            }
            Miss::Known { adapter, .. } | Miss::OtherVersion { adapter } => needed(*adapter),
            Miss::NoAdapter => String::new(),
        }
    }

    fn adapter(&self) -> Option<&'static dyn Adapter> {
        match self {
            Miss::Changed { adapter, .. } | Miss::Known { adapter, .. } | Miss::OtherVersion { adapter } => {
                Some(*adapter)
            }
            Miss::NoAdapter => None,
        }
    }
}

/// Why an image that its console recognises, and that no adapter reads, is refused. `image` is the
/// file at `path`, open.
pub fn refused(path: &Path, platform: &'static dyn Platform, image: &Image) -> Error {
    let Some(label) = platform.label(image) else {
        return Error::UnknownGame(PathBuf::from(shown(path)));
    };
    let miss = classify(platform, &label, image.size().unwrap_or(0));
    let reason = miss.reason(platform, &label_words(platform, &label.name, &label.version));
    let message = format!("{} {reason} {}", shown(path), miss.advice());
    Error::Refused(message.trim_end().to_string())
}

/// For a file in a folder of games that says it is `label` and holds a game of `size` bytes: the
/// game ventris knows that it is, or says it is, and why it is not read, to follow the file's name.
/// `None` for a file that is no game an adapter is for, which a search does not mention.
pub fn near_miss(platform: &'static dyn Platform, label: &Label, size: u64) -> Option<(&'static dyn Adapter, String)> {
    let miss = classify(platform, label, size);
    let adapter = miss.adapter()?;
    Some((adapter, miss.reason(platform, &label_words(platform, &label.name, &label.version))))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::adapters::{tod2_ps2, top_sfc};
    use crate::platforms::ps2::tests::disc_with;
    use crate::platforms::snes::Mapping;
    use crate::platforms::snes::tests::titled;
    use crate::tasks::open_game;

    /// What `open_game` refuses a file with: the words, and whether it is the plain refusal.
    fn refusal(path: &Path) -> (String, bool) {
        match open_game(path) {
            Err(error @ Error::Refused(_)) => (error.to_string(), false),
            Err(error @ Error::UnknownGame(_)) => (error.to_string(), true),
            Err(other) => panic!("refused, but not as a file that is no game: {other:?}"),
            Ok(_) => panic!("it was not refused"),
        }
    }

    const NEEDED_DISC: &str = "The release ventris reads is Japan, version 1.01, named \"Tales of Destiny 2 (Japan, \
        Asia)\" in Redump, the catalogue of disc dumps: serial SLPS-25172, 3,234,004,992 bytes, SHA-1 \
        3b2be72b751607a7a2a56d62feec71584b4b8c72.";
    const NEEDED_CARTRIDGE: &str = "The release ventris reads is Japan, named \"Tales of Phantasia (Japan)\" in No-Intro, \
        the catalogue of cartridge dumps: title \"TALES OF PHANTASIA\", version 1.0, 6,291,456 bytes, SHA-1 \
        fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0.";

    #[test]
    fn a_compressed_or_wrapped_file_is_told_by_its_first_bytes() {
        let folder = tempfile::tempdir().unwrap();
        let write = |name: &str, data: &[u8]| {
            let path = folder.path().join(name);
            fs::write(&path, data).unwrap();
            path
        };
        let chd = write("game.chd", b"MComprHD\x00\x00\x00\x7c\x00\x00\x00\x05 and the rest");
        let (words, plain) = refusal(&chd);
        assert!(!plain);
        assert_eq!(
            words,
            format!(
                "{} is a CHD file, a compressed disc image. ventris reads a game's plain file, such as an .iso for a \
                 disc or the cartridge image for a cartridge, and cannot read inside this one. Convert it back to a \
                 plain disc image first, with chdman, the program CHD files are made with, then use the file that \
                 comes out.",
                shown(&chd)
            )
        );
        for (name, data, what) in [
            ("game.cso", b"CISO\x18\x00\x00\x00".as_slice(), "is a CSO file, a compressed disc image."),
            ("game.zso", b"ZISO\x18\x00\x00\x00", "is a ZSO file, a compressed disc image."),
            ("game.7z", b"7z\xBC\xAF\x27\x1C\x00\x04", "is a 7z archive."),
            ("game.gz", b"\x1f\x8b\x08\x00", "is a gzip-compressed file."),
            ("game.rar", b"Rar!\x1a\x07\x01\x00", "is a RAR archive."),
            // An archive that holds a disc image is no cartridge's: no console recognises it.
            (
                "game.zip",
                &crate::archive::make(&[("game.iso", b"a disc", true)]),
                "is a zip archive that does not hold one cartridge image.",
            ),
        ] {
            let (words, plain) = refusal(&write(name, data));
            assert!(!plain && words.contains(what), "{words}");
        }
        // Too short to say, or saying nothing: the plain refusal.
        for (name, data) in [("empty.bin", b"".as_slice()), ("one.bin", b"M"), ("notes.txt", b"not a game")] {
            let path = write(name, data);
            assert_eq!(refusal(&path), (format!("{} is not a game ventris knows how to read yet", shown(&path)), true));
        }
    }

    /// A disc whose SYSTEM.CNF gives this serial and version.
    fn disc(folder: &Path, serial: &str, version: Option<&str>) -> PathBuf {
        let program = format!("{}_{}.{}", &serial[..4], &serial[5..8], &serial[8..]);
        let mut config = format!("BOOT2 = cdrom0:\\{program};1\r\n");
        if let Some(version) = version {
            config.push_str(&format!("VER = {version}\r\nVMODE = NTSC\r\n"));
        }
        disc_with(folder, &format!("{serial}.iso"), config.as_bytes())
    }

    #[test]
    fn a_disc_is_told_what_it_is_and_which_release_is_needed() {
        assert_eq!(tod2_ps2::ADAPTER.images()[0].label, "SLPS-25172");
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();

        // Another release of a game ventris knows, and an edition the adapter names.
        let path = disc(root, "SLPS-72502", Some("2.01"));
        assert_eq!(
            refusal(&path),
            (
                format!(
                    "{} is Tales of Destiny 2, but not the release ventris reads: it is the Mega Hits! reissue, in \
                     Japanese (serial SLPS-72502, version 2.01). {NEEDED_DISC}",
                    shown(&path)
                ),
                false
            )
        );
        let (words, _) = refusal(&disc(root, "SCKA-20054", None));
        assert!(words.contains("it is the PlayStation 2 BigHit Series reissue, in Korean (serial SCKA-20054). "));

        // The release that is read, by what it says, with other contents: here, far too small.
        let path = disc(root, "SLPS-25172", Some("1.01"));
        let size = grouped(fs::metadata(&path).unwrap().len());
        assert_eq!(
            refusal(&path),
            (
                format!(
                    "{} says it is the release of Tales of Destiny 2 that ventris reads (serial SLPS-25172, version \
                     1.01), but its contents are not the original's: it has been changed, by a patch such as an undub \
                     or a translation, or it is a damaged or cut-short copy. It is {size} bytes, and the original is \
                     3,234,004,992. Use an unchanged copy, named \"Tales of Destiny 2 (Japan, Asia)\" in Redump, the \
                     catalogue of disc dumps: the original's SHA-1 is 3b2be72b751607a7a2a56d62feec71584b4b8c72.",
                    shown(&path)
                ),
                false
            )
        );

        // The serial of the release that is read, with another version.
        let path = disc(root, "SLPS-25172", Some("1.00"));
        let (words, _) = refusal(&path);
        assert_eq!(
            words,
            format!(
                "{} is Tales of Destiny 2, but not the release ventris reads: it is another version (serial \
                 SLPS-25172, version 1.00). {NEEDED_DISC}",
                shown(&path)
            )
        );

        // A game there is no adapter for.
        let path = disc(root, "SLPM-66001", Some("1.00"));
        assert_eq!(
            refusal(&path),
            (
                format!(
                    "{} is a PlayStation 2 game (serial SLPM-66001, version 1.00), and ventris has no adapter for \
                     it, so it cannot read it.",
                    shown(&path)
                ),
                false
            )
        );

        // A disc that names no PlayStation 2 program: the plain refusal.
        let path = disc_with(root, "other.iso", b"BOOT = cdrom:\\SLPS_000.01;1\r\n");
        assert!(refusal(&path).1);
    }

    #[test]
    fn a_cartridge_is_told_what_it_is_and_which_release_is_needed() {
        assert_eq!(top_sfc::ADAPTER.images()[0].label, "TALES OF PHANTASIA");
        let folder = tempfile::tempdir().unwrap();
        let write = |name: &str, data: &[u8]| {
            let path = folder.path().join(name);
            fs::write(&path, data).unwrap();
            path
        };

        // The title and version of the release that is read, with other contents, and a copier's header
        // in front, which changes nothing: the size given is the game's, without it.
        let rom = titled(Mapping::HiRom, 0x100000, b"TALES OF PHANTASIA", 0);
        let path = write("changed.smc", &[vec![0; 512], rom].concat());
        assert_eq!(
            refusal(&path),
            (
                format!(
                    "{} says it is the release of Tales of Phantasia that ventris reads (title \"TALES OF \
                     PHANTASIA\", version 1.0), but its contents are not the original's: it has been changed, by a \
                     patch such as an undub or a translation, or it is a damaged or cut-short copy. It is 1,048,576 \
                     bytes, and the original is 6,291,456. Use an unchanged copy, named \"Tales of Phantasia (Japan)\" in \
                     No-Intro, the catalogue of cartridge dumps: the original's SHA-1 is \
                     fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0.",
                    shown(&path)
                ),
                false
            )
        );

        // A known patch, which writes its own title.
        let path = write("dejap.sfc", &titled(Mapping::HiRom, 0x100000, b"ToP English (C) DeJap", 0));
        assert_eq!(
            refusal(&path),
            (
                format!(
                    "{} is Tales of Phantasia with DeJap's English translation already applied (title \"ToP English \
                     (C) DeJap\", version 1.0). ventris reads the game as it was released, so it needs a copy without \
                     the patch. {NEEDED_CARTRIDGE}",
                    shown(&path)
                ),
                false
            )
        );

        // Another game, in the zip archive a collection keeps it in.
        let other = titled(Mapping::LoRom, 0x80000, b"SOME OTHER GAME", 1);
        let path = write("other.zip", &crate::archive::make(&[("Other (Japan).sfc", &other, true)]));
        let (words, _) = refusal(&path);
        assert_eq!(
            words,
            format!(
                "{} is a Super Famicom / SNES game (title \"SOME OTHER GAME\", version 1.1), and ventris has no \
                 adapter for it, so it cannot read it.",
                shown(&path)
            )
        );
    }

    #[test]
    fn a_release_is_given_in_full() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(6_291_456), "6,291,456");
        assert_eq!(grouped(3_234_004_992), "3,234,004,992");
        let adapter = &tod2_ps2::ADAPTER;
        assert_eq!(
            release_in_full(adapter.platform(), &adapter.images()[0]),
            "Japan, version 1.01, named \"Tales of Destiny 2 (Japan, Asia)\" in Redump, the catalogue of disc dumps: \
             serial SLPS-25172, 3,234,004,992 bytes, SHA-1 3b2be72b751607a7a2a56d62feec71584b4b8c72"
        );
        // A version the release's name does not give is stated with the label.
        let cartridge = &top_sfc::ADAPTER;
        assert_eq!(
            release_facts(cartridge.platform(), &cartridge.images()[0]),
            "title \"TALES OF PHANTASIA\", version 1.0, 6,291,456 bytes, SHA-1 fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0"
        );
        assert_eq!(compressed_note(&[]), None);
        assert_eq!(
            compressed_note(&["a.chd".into()]).unwrap(),
            "a.chd is a compressed disc image (CHD, CSO or ZSO): ventris cannot read it, or tell which game it holds. \
             If it is this game, convert it back to a plain disc image first."
        );
        let names: Vec<String> = ["a.chd", "b.cso", "c.zso", "d.chd", "e.chd"].map(String::from).to_vec();
        assert!(compressed_note(&names[..2]).unwrap().starts_with("a.chd and b.cso are compressed disc images "));
        assert!(compressed_note(&names[..3]).unwrap().starts_with("a.chd, b.cso and c.zso are "));
        assert!(compressed_note(&names).unwrap().starts_with("a.chd, b.cso, c.zso and 2 more are compressed "));
    }
}
