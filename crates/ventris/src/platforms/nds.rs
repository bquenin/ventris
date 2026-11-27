//! Nintendo DS cartridges: the header, the file system inside the cartridge, and rebuilding it.
//!
//! A DS game is one image of up to 512 MiB. Its first 0x4000 bytes are the header, which says where
//! everything else is: the code for each of the two processors (the ARM9 binary, which runs the game,
//! and the ARM7 one, which runs sound and the hardware), the tables of the code overlays the ARM9
//! binary loads as it needs them, and the file system, which Nintendo's tools call NitroFS. The
//! file system is two tables: the file allocation table (FAT), eight bytes per file, the offsets in
//! the image where it starts and ends; and the file name table (FNT), which gives the folders and
//! the names of the files in them. A file is known to the game by its number, the index of its entry
//! in the FAT; the overlays are files of the same table, with no name.
//!
//! A translation changes files. Whichever way the game reads a file, it goes through its FAT entry,
//! so a file is rebuilt by writing its new bytes where they fit and pointing its entry at them: in
//! its own place when it is no longer than the room it had, which is its own length up to the next
//! file, else in the unused space after the last file. The image keeps its size; files start on
//! 512-byte boundaries, as the tools that made the game put them, and room left over is 0xFF, as
//! the tools fill it. The header says how much of the image is used, and has a checksum of its
//! own, which a console checks before it starts a game; both are brought up to date. A file that is
//! not changed keeps its place and its bytes, so a build that changes nothing is the image itself.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use sha1::{Digest, Sha1};

use crate::archive;
use crate::error::{Error, Result, at};
use crate::platform::{Glance, Image, Label, Platform};

pub use crate::platforms::snes::NO_INTRO;

/// The header takes this much of the image, before anything it points at.
pub const HEADER_SIZE: usize = 0x4000;
/// 512 MiB, the most a cartridge holds.
pub const LARGEST_ROM: u64 = 0x2000_0000;
/// Files start on boundaries of this many bytes.
pub const FILE_ALIGNMENT: usize = 0x200;
/// What fills room that holds nothing.
pub const FILLER: u8 = 0xFF;
/// The checksum of the Nintendo logo in every licensed game's header: a file without it is not a
/// game, whatever its name.
const LOGO_CHECKSUM: u16 = 0xCF56;

/// The fields of the header that tell what a game is and where its parts are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// Up to twelve characters: "DS GOEMON".
    pub title: String,
    /// Four characters that name the game and its region: "A5EJ".
    pub game_code: String,
    /// Starts at 0; a later printing of the same game has a higher one.
    pub version: u8,
    pub arm9_offset: usize,
    pub arm9_address: u32,
    pub arm9_size: usize,
    pub arm7_offset: usize,
    pub arm7_size: usize,
    pub fnt_offset: usize,
    pub fnt_size: usize,
    pub fat_offset: usize,
    pub fat_size: usize,
    pub overlays9_offset: usize,
    pub overlays9_size: usize,
    /// How many bytes of the image are used, from its start.
    pub used: usize,
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().expect("four bytes"))
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// The CRC-16 the header's checksums use (polynomial 0xA001, starting at 0xFFFF).
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

impl Header {
    /// The header of an image, or `None` when the bytes are not a game's header: too short, without
    /// the logo every game has, with a checksum that does not match, or pointing past the image.
    pub fn read(rom: &[u8]) -> Option<Header> {
        if rom.len() < 0x200 || u16_at(rom, 0x15C) != LOGO_CHECKSUM || u16_at(rom, 0x15E) != crc16(&rom[..0x15E]) {
            return None;
        }
        let text = |range: std::ops::Range<usize>| {
            let bytes = &rom[range];
            let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
            bytes[..end]
                .iter()
                .all(|byte| (0x20..0x7F).contains(byte))
                .then(|| String::from_utf8_lossy(&bytes[..end]).into_owned())
        };
        let word = |offset: usize| u32_at(rom, offset) as usize;
        Some(Header {
            title: text(0..12)?,
            game_code: text(12..16)?,
            version: rom[0x1E],
            arm9_offset: word(0x20),
            arm9_address: u32_at(rom, 0x28),
            arm9_size: word(0x2C),
            arm7_offset: word(0x30),
            arm7_size: word(0x3C),
            fnt_offset: word(0x40),
            fnt_size: word(0x44),
            fat_offset: word(0x48),
            fat_size: word(0x4C),
            overlays9_offset: word(0x50),
            overlays9_size: word(0x54),
            used: word(0x80),
        })
    }

    fn label(&self) -> Label {
        Label { name: self.game_code.clone(), version: self.version.to_string() }
    }
}

/// An overlay of the ARM9 binary: code loaded at `address` when the game needs it, from file `file`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub id: u32,
    pub address: u32,
    pub size: u32,
    pub file: u16,
    /// The overlay is stored compressed, which a game's code undoes when it loads it.
    pub compressed: bool,
}

/// The file system of an image, as its tables give it.
#[derive(Clone, Debug)]
pub struct FileSystem {
    pub header: Header,
    /// Each file's start and end in the image, by file number.
    pub files: Vec<(usize, usize)>,
    /// The path of each named file, folders separated by `/`, by file number.
    pub names: BTreeMap<u16, String>,
    pub overlays: Vec<Overlay>,
}

fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

impl FileSystem {
    /// Read the tables of a game's image.
    pub fn read(rom: &[u8]) -> Result<FileSystem> {
        let Some(header) = Header::read(rom) else {
            return image_error("this is not a Nintendo DS game: its header is not one");
        };
        let within = |offset: usize, size: usize| offset.checked_add(size).is_some_and(|end| end <= rom.len());
        if !within(header.fat_offset, header.fat_size) || !within(header.fnt_offset, header.fnt_size) {
            return image_error("the image is cut short: its file tables are past its end");
        }
        let fat = &rom[header.fat_offset..header.fat_offset + header.fat_size];
        let mut files = Vec::with_capacity(fat.len() / 8);
        for entry in fat.as_chunks::<8>().0 {
            let (start, end) = (u32_at(entry, 0) as usize, u32_at(entry, 4) as usize);
            if end < start || end > rom.len() {
                return image_error(format!("file {} is past the end of the image", files.len()));
            }
            files.push((start, end));
        }
        let mut names = BTreeMap::new();
        let fnt = &rom[header.fnt_offset..header.fnt_offset + header.fnt_size];
        read_folder(fnt, 0xF000, "", &mut names, 0)?;
        if let Some((number, _)) = names.iter().find(|(number, _)| usize::from(**number) >= files.len()) {
            return image_error(format!("the name table names file {number}, which the image has not got"));
        }
        let mut overlays = Vec::new();
        if within(header.overlays9_offset, header.overlays9_size) {
            let table = &rom[header.overlays9_offset..header.overlays9_offset + header.overlays9_size];
            for entry in table.as_chunks::<32>().0 {
                let flags = u32_at(entry, 28);
                overlays.push(Overlay {
                    id: u32_at(entry, 0),
                    address: u32_at(entry, 4),
                    size: u32_at(entry, 8),
                    file: u32_at(entry, 24) as u16,
                    compressed: flags >> 24 & 1 != 0,
                });
            }
        }
        Ok(FileSystem { header, files, names, overlays })
    }

    /// The number of the file at `path`.
    pub fn number(&self, path: &str) -> Option<u16> {
        self.names.iter().find(|(_, name)| name.as_str() == path).map(|(number, _)| *number)
    }

    /// The bytes of a file.
    pub fn file<'a>(&self, rom: &'a [u8], number: u16) -> Result<&'a [u8]> {
        match self.files.get(usize::from(number)) {
            Some((start, end)) => Ok(&rom[*start..*end]),
            None => image_error(format!("the image has no file {number}")),
        }
    }

    /// The bytes of the file at `path`.
    pub fn file_at<'a>(&self, rom: &'a [u8], path: &str) -> Result<&'a [u8]> {
        match self.number(path) {
            Some(number) => self.file(rom, number),
            None => image_error(format!("the image has no file {path}")),
        }
    }

    /// The image with these files given new contents, by number; the others keep their places and
    /// their bytes. A file goes where it was when it fits in the room it had, which runs to where the
    /// next file starts, and after the last file otherwise.
    pub fn rebuild(&self, rom: &[u8], replaced: &BTreeMap<u16, Vec<u8>>) -> Result<Vec<u8>> {
        let mut out = rom.to_vec();
        if replaced.is_empty() {
            return Ok(out);
        }
        let mut starts: Vec<usize> =
            self.files.iter().filter(|(start, end)| end > start).map(|(start, _)| *start).collect();
        starts.sort_unstable();
        let room = |start: usize| -> usize {
            let next = starts.iter().find(|other| **other > start).copied().unwrap_or(self.header.used.max(start));
            next.max(start)
        };
        let mut files = self.files.clone();
        let mut free = align(self.header.used.max(files.iter().map(|(_, end)| *end).max().unwrap_or(0)));
        for (number, data) in replaced {
            let Some((start, end)) = files.get(usize::from(*number)).copied() else {
                return image_error(format!("the image has no file {number}"));
            };
            let limit = if end > start { room(start).max(end) } else { end };
            let place = if start + data.len() <= limit {
                out[start..limit].fill(FILLER);
                start
            } else {
                out[start..end].fill(FILLER);
                let place = free;
                if place + data.len() > out.len() {
                    return image_error(format!(
                        "file {} does not fit: the image has {} bytes free after its last file and it needs {}",
                        self.names.get(number).cloned().unwrap_or_else(|| number.to_string()),
                        out.len().saturating_sub(place),
                        data.len()
                    ));
                }
                free = align(place + data.len());
                place
            };
            out[place..place + data.len()].copy_from_slice(data);
            files[usize::from(*number)] = (place, place + data.len());
        }
        for (number, (start, end)) in files.iter().enumerate() {
            let entry = self.header.fat_offset + number * 8;
            put_u32(&mut out, entry, *start as u32);
            put_u32(&mut out, entry + 4, *end as u32);
        }
        let used = files.iter().map(|(_, end)| *end).max().unwrap_or(0).max(self.header.used);
        put_u32(&mut out, 0x80, used as u32);
        let checksum = crc16(&out[..0x15E]);
        out[0x15E..0x160].copy_from_slice(&checksum.to_le_bytes());
        Ok(out)
    }
}

/// The bytes of a file, found by its number in the allocation table alone, without reading the name
/// table: for an adapter that knows the number and reads the file often, such as once per glyph.
pub fn file_by_number(rom: &[u8], number: u16) -> Result<&[u8]> {
    let Some(header) = Header::read(rom) else {
        return image_error("this is not a Nintendo DS game: its header is not one");
    };
    let entry = header.fat_offset + 8 * usize::from(number);
    if 8 * usize::from(number) + 8 > header.fat_size || entry + 8 > rom.len() {
        return image_error(format!("the image has no file {number}"));
    }
    let (start, end) = (u32_at(rom, entry) as usize, u32_at(rom, entry + 4) as usize);
    match rom.get(start..end) {
        Some(data) => Ok(data),
        None => image_error(format!("file {number} is past the end of the image")),
    }
}

fn align(offset: usize) -> usize {
    offset.div_ceil(FILE_ALIGNMENT) * FILE_ALIGNMENT
}

/// The files of one folder of the name table, and of the folders inside it.
fn read_folder(fnt: &[u8], folder: u16, prefix: &str, names: &mut BTreeMap<u16, String>, depth: usize) -> Result<()> {
    let entry = 8 * usize::from(folder & 0xFFF);
    if depth > 64 || entry + 8 > fnt.len() {
        return image_error("the name table of the image is damaged");
    }
    let mut at = u32_at(fnt, entry) as usize;
    let mut number = u16_at(fnt, entry + 4);
    loop {
        let Some(&kind) = fnt.get(at) else { return image_error("the name table of the image is damaged") };
        at += 1;
        if kind == 0 {
            return Ok(());
        }
        let length = usize::from(kind & 0x7F);
        let Some(name) = fnt.get(at..at + length) else { return image_error("the name table of the image is damaged") };
        let name = String::from_utf8_lossy(name).into_owned();
        at += length;
        if kind & 0x80 != 0 {
            let Some(inner) = fnt.get(at..at + 2) else { return image_error("the name table of the image is damaged") };
            at += 2;
            read_folder(fnt, u16::from_le_bytes([inner[0], inner[1]]), &format!("{prefix}{name}/"), names, depth + 1)?;
        } else {
            names.insert(number, format!("{prefix}{name}"));
            number = number.wrapping_add(1);
        }
    }
}

/// Write a built image to `path`.
pub fn write_rom(path: &Path, rom: &[u8]) -> Result<()> {
    fs::write(path, rom).map_err(at(path))
}

pub struct Nds;

pub static PLATFORM: Nds = Nds;

fn is_cartridge_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ending)| ending.eq_ignore_ascii_case("nds"))
}

/// The cartridge image in a zip archive, when it holds exactly one.
fn cartridge_in(path: &Path) -> Result<Option<archive::Entry>> {
    let mut images: Vec<archive::Entry> =
        archive::entries(path)?.into_iter().filter(|entry| is_cartridge_name(&entry.name)).collect();
    Ok(if images.len() == 1 { images.pop() } else { None })
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    if !archive::is_zip(path) {
        return fs::read(path).map_err(at(path));
    }
    match cartridge_in(path)? {
        Some(entry) => archive::read(path, &entry),
        None => Err(Error::Image(format!("{} does not hold one DS cartridge image", path.display()))),
    }
}

/// The first bytes of a plain file, enough for its header.
fn head_of(path: &Path) -> Option<Vec<u8>> {
    let mut head = vec![0; 0x200];
    fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    Some(head)
}

impl Platform for Nds {
    fn name(&self) -> &'static str {
        "Nintendo DS"
    }

    fn recognises(&self, path: &Path) -> Result<bool> {
        if archive::is_zip(path) {
            let Ok(Some(entry)) = cartridge_in(path) else { return Ok(false) };
            if entry.size > LARGEST_ROM {
                return Ok(false);
            }
            return Ok(Header::read(&read_file(path)?).is_some());
        }
        if fs::metadata(path).map_err(at(path))?.len() > LARGEST_ROM {
            return Ok(false);
        }
        Ok(head_of(path).is_some_and(|head| Header::read(&head).is_some()))
    }

    fn glance(&self, path: &Path) -> Result<Option<Glance>> {
        let (name, size) = if archive::is_zip(path) {
            match cartridge_in(path) {
                Ok(Some(entry)) => (entry.name.rsplit('/').next().unwrap_or_default().to_string(), entry.size),
                _ => return Ok(None),
            }
        } else if is_cartridge_name(&path.to_string_lossy()) {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            (name, fs::metadata(path).map_err(at(path))?.len())
        } else {
            return Ok(None);
        };
        Ok((size <= LARGEST_ROM).then_some(Glance { size, name }))
    }

    fn open(&self, path: &Path) -> Result<(Image, String)> {
        let rom = read_file(path)?;
        let sha1 = Sha1::digest(&rom).iter().map(|byte| format!("{byte:02x}")).collect();
        Ok((Image::Cartridge(rom), sha1))
    }

    /// The game code and version of the header.
    fn label(&self, image: &Image) -> Option<Label> {
        Header::read(image.cartridge().ok()?).map(|header| header.label())
    }

    /// The same, from a plain file; one in an archive is not unpacked for it.
    fn peek(&self, path: &Path) -> Option<Label> {
        if archive::is_zip(path) {
            return None;
        }
        Header::read(&head_of(path)?).map(|header| header.label())
    }

    fn label_kind(&self) -> &'static str {
        "game code"
    }

    fn changes(&self, original: &Image, built: &Image) -> Vec<String> {
        let (Ok(original), Ok(built)) = (original.cartridge(), built.cartridge()) else {
            return Vec::new();
        };
        let (Ok(before), Ok(after)) = (FileSystem::read(original), FileSystem::read(built)) else {
            return Vec::new();
        };
        let moved = before.files.iter().zip(&after.files).filter(|(old, new)| old.0 != new.0).count();
        if moved == 0 {
            return Vec::new();
        }
        vec![format!("files:     {moved} moved to the free space after the last file")]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A made-up DS image: a header, a name table with a folder, and these files, 512-byte aligned,
    /// then filler to `size`.
    pub(crate) fn make_rom(files: &[(&str, &[u8])], size: usize) -> Vec<u8> {
        let mut rom = vec![0u8; HEADER_SIZE];
        rom[..9].copy_from_slice(b"TEST GAME");
        rom[12..16].copy_from_slice(b"TEST");
        rom[0x15C..0x15E].copy_from_slice(&LOGO_CHECKSUM.to_le_bytes());
        // Name table: the root folder holds the files at the top, and a folder "sub" holds the rest.
        let (top, inner): (Vec<_>, Vec<_>) = files.iter().partition(|(name, _)| !name.contains('/'));
        let mut fnt = vec![0u8; 16];
        let root_start = fnt.len();
        for (name, _) in &top {
            fnt.push(name.len() as u8);
            fnt.extend_from_slice(name.as_bytes());
        }
        fnt.extend_from_slice(&[0x83, b's', b'u', b'b', 0x01, 0xF0, 0]);
        let sub_start = fnt.len();
        for (name, _) in &inner {
            let name = name.rsplit('/').next().unwrap();
            fnt.push(name.len() as u8);
            fnt.extend_from_slice(name.as_bytes());
        }
        fnt.push(0);
        fnt[0..4].copy_from_slice(&(root_start as u32).to_le_bytes());
        fnt[4..6].copy_from_slice(&0u16.to_le_bytes());
        fnt[6..8].copy_from_slice(&2u16.to_le_bytes());
        fnt[8..12].copy_from_slice(&(sub_start as u32).to_le_bytes());
        fnt[12..14].copy_from_slice(&(top.len() as u16).to_le_bytes());
        fnt[14..16].copy_from_slice(&0xF000u16.to_le_bytes());
        let ordered: Vec<(&str, &[u8])> = top.iter().chain(inner.iter()).copied().collect();
        let fnt_offset = rom.len();
        rom.extend_from_slice(&fnt);
        rom.resize(align(rom.len()), FILLER);
        let fat_offset = rom.len();
        rom.resize(fat_offset + 8 * ordered.len(), 0);
        rom.resize(align(rom.len()), FILLER);
        for (index, (_, data)) in ordered.iter().enumerate() {
            let start = rom.len();
            rom.extend_from_slice(data);
            put_u32(&mut rom, fat_offset + 8 * index, start as u32);
            let end = rom.len() as u32;
            put_u32(&mut rom, fat_offset + 8 * index + 4, end);
            rom.resize(align(rom.len()), FILLER);
        }
        let used = rom.len();
        rom.resize(size, FILLER);
        put_u32(&mut rom, 0x40, fnt_offset as u32);
        put_u32(&mut rom, 0x44, fnt.len() as u32);
        put_u32(&mut rom, 0x48, fat_offset as u32);
        put_u32(&mut rom, 0x4C, (8 * ordered.len()) as u32);
        put_u32(&mut rom, 0x80, used as u32);
        let checksum = crc16(&rom[..0x15E]);
        rom[0x15E..0x160].copy_from_slice(&checksum.to_le_bytes());
        rom
    }

    #[test]
    fn the_crc_is_the_one_the_header_uses() {
        // CRC-16/MODBUS of "123456789".
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn the_file_system_is_read_by_name_and_number() {
        let rom = make_rom(&[("a.dat", b"alpha"), ("b.dat", b"bravo!"), ("sub/c.dat", b"charlie")], 0x8000);
        let fs = FileSystem::read(&rom).unwrap();
        assert_eq!(fs.header.title, "TEST GAME");
        assert_eq!(fs.header.game_code, "TEST");
        assert_eq!(fs.names.values().collect::<Vec<_>>(), ["a.dat", "b.dat", "sub/c.dat"]);
        assert_eq!(fs.file_at(&rom, "sub/c.dat").unwrap(), b"charlie");
        assert_eq!(fs.file(&rom, 1).unwrap(), b"bravo!");
        assert_eq!(file_by_number(&rom, 2).unwrap(), b"charlie");
        assert!(file_by_number(&rom, 3).is_err());
        assert!(fs.file_at(&rom, "missing.dat").is_err());
    }

    #[test]
    fn a_header_that_is_not_a_game_is_not_read() {
        let mut rom = make_rom(&[("a.dat", b"alpha")], 0x8000);
        assert!(Header::read(&rom).is_some());
        rom[0x15E] ^= 1;
        assert!(Header::read(&rom).is_none());
        assert!(FileSystem::read(&rom).is_err());
        assert!(Header::read(&[0; 0x100]).is_none());
    }

    #[test]
    fn a_rebuild_with_nothing_changed_is_the_image_itself() {
        let rom = make_rom(&[("a.dat", b"alpha"), ("b.dat", b"bravo")], 0x8000);
        let fs = FileSystem::read(&rom).unwrap();
        assert_eq!(fs.rebuild(&rom, &BTreeMap::new()).unwrap(), rom);
    }

    #[test]
    fn a_file_stays_in_place_when_it_fits_and_moves_after_the_last_otherwise() {
        let rom = make_rom(&[("a.dat", b"alpha"), ("b.dat", b"bravo")], 0x8000);
        let fs = FileSystem::read(&rom).unwrap();
        let (a_start, _) = fs.files[0];
        // Up to the next file's start is room enough.
        let fits = vec![7u8; FILE_ALIGNMENT];
        let built = fs.rebuild(&rom, &BTreeMap::from([(0, fits.clone())])).unwrap();
        let after = FileSystem::read(&built).unwrap();
        assert_eq!(after.files[0], (a_start, a_start + fits.len()));
        assert_eq!(after.file(&built, 1).unwrap(), b"bravo");
        // One byte more moves it after the last file, on a boundary, and its old place is filled.
        let long = vec![9u8; FILE_ALIGNMENT + 1];
        let built = fs.rebuild(&rom, &BTreeMap::from([(0, long.clone())])).unwrap();
        let after = FileSystem::read(&built).unwrap();
        assert_eq!(after.file(&built, 0).unwrap(), long.as_slice());
        assert!(after.files[0].0 >= fs.header.used && after.files[0].0.is_multiple_of(FILE_ALIGNMENT));
        assert!(built[a_start..a_start + 5].iter().all(|byte| *byte == FILLER));
        assert_eq!(after.header.used, after.files[0].1);
        assert_eq!(after.file(&built, 1).unwrap(), b"bravo");
        assert_eq!(built.len(), rom.len());
        // A file larger than the free space is refused, in words.
        let huge = vec![0u8; 0x8000];
        let error = fs.rebuild(&rom, &BTreeMap::from([(1, huge)])).unwrap_err().to_string();
        assert!(error.starts_with("file b.dat does not fit"), "{error}");
    }

    #[test]
    fn the_platform_knows_a_ds_game_by_its_header() {
        let folder = tempfile::tempdir().unwrap();
        let rom = make_rom(&[("a.dat", b"alpha")], 0x8000);
        let plain = folder.path().join("game.nds");
        fs::write(&plain, &rom).unwrap();
        let other = folder.path().join("other.nds");
        fs::write(&other, vec![0u8; 0x8000]).unwrap();
        assert!(PLATFORM.recognises(&plain).unwrap());
        assert!(!PLATFORM.recognises(&other).unwrap());
        let zipped = folder.path().join("game.zip");
        fs::write(&zipped, archive::make(&[("game.nds", &rom, true)])).unwrap();
        assert!(PLATFORM.recognises(&zipped).unwrap());
        assert_eq!(PLATFORM.glance(&zipped).unwrap(), Some(Glance { size: 0x8000, name: "game.nds".into() }));
        let (image, sha1) = PLATFORM.open(&zipped).unwrap();
        assert_eq!(sha1.len(), 40);
        let label = PLATFORM.label(&image).unwrap();
        assert_eq!((label.name.as_str(), label.version.as_str()), ("TEST", "0"));
        assert_eq!(PLATFORM.peek(&plain), Some(label));
        assert_eq!(PLATFORM.peek(&zipped), None);
    }
}
