//! PlayStation 2 disc images: finding and reading the files inside one.
//!
//! A PS2 game disc is an ISO 9660 file system. An image of one is several gigabytes, so it is never
//! read whole: `Disc` keeps the image open and reads the parts asked for. `SYSTEM.CNF` in the root
//! names the program the console starts.
//!
//! `write_image` writes a changed copy: bytes replaced inside files that keep their size, and new
//! contents of any size for the file that ends the image, which is where a game's large archive
//! tends to be. A PS2 disc also describes its files a second time, as UDF. That copy is not
//! rewritten, since the console finds files through ISO 9660; only the UDF marker that must sit in
//! the last sector moves.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sha1::{Digest, Sha1};

use crate::adapter::Catalogue;
use crate::error::{Error, Result, at};
use crate::platform::{Glance, Image, Label, Platform};

pub const SECTOR: u64 = 2048;
/// Where the primary volume descriptor is.
pub(crate) const VOLUME_SECTOR: u64 = 16;
/// How an ISO 9660 volume descriptor starts.
pub(crate) const VOLUME_MAGIC: &[u8] = b"\x01CD001";
/// Where the root directory's record is inside the volume descriptor.
pub(crate) const ROOT_RECORD: usize = 156;
/// The flag that marks a directory record.
pub(crate) const DIRECTORY: u8 = 2;
/// A directory record up to the first byte of the name it gives.
pub(crate) const RECORD_HEAD: usize = 33;
/// Folders inside folders: the standard allows eight, and an image that loops would never end.
pub(crate) const DEEPEST_FOLDER: usize = 32;
/// How much of an image is read or copied at a time.
const CHUNK: usize = 1 << 24;
/// In the volume descriptor: the number of sectors, little-endian then big-endian.
const VOLUME_SIZE: u64 = 80;
/// In a directory record: the file's size, little-endian then big-endian.
const RECORD_SIZE: u64 = 10;
/// The kind of UDF descriptor that sits in the image's last sector.
const UDF_ANCHOR: u16 = 2;

/// The catalogue players know a disc dump by.
pub const REDUMP: Catalogue = Catalogue { name: "Redump", about: "the catalogue of disc dumps" };

fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

/// Whether the file at `path` starts like an ISO 9660 image.
pub fn is_disc(path: &Path) -> bool {
    let mut magic = [0; VOLUME_MAGIC.len()];
    let read = File::open(path).and_then(|mut image| {
        image.seek(SeekFrom::Start(VOLUME_SECTOR * SECTOR))?;
        image.read_exact(&mut magic)
    });
    read.is_ok() && magic == VOLUME_MAGIC
}

/// Bytes as text, for the names a disc gives: what is not ASCII is shown as the replacement character.
pub(crate) fn ascii(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| if byte.is_ascii() { char::from(*byte) } else { char::REPLACEMENT_CHARACTER }).collect()
}

/// Whether a character of such text is blank: a space, a tab, a line end or one of the separators.
pub(crate) fn is_blank(char: char) -> bool {
    matches!(char, '\t'..='\r' | '\x1c'..=' ')
}

/// Up to `length` bytes of a file from `offset`; fewer when the file ends first.
fn read_at(file: &mut File, offset: u64, length: u64) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut data = Vec::with_capacity(length.min(CHUNK as u64) as usize);
    file.take(length).read_to_end(&mut data)?;
    Ok(data)
}

/// An open disc image. It is closed when it is dropped. Until then Windows refuses to delete the
/// file or to write another in its place, so an image that was built is dropped before it is removed.
pub struct Disc {
    pub path: PathBuf,
    image: Mutex<File>,
    /// The name the disc gives itself.
    pub label: String,
    pub publisher: String,
    /// Each file's path, such as `VIRX/PADMAN.IRX`, and where it is: its first sector and its size in bytes.
    pub files: BTreeMap<String, (u32, u32)>,
    /// Where in the image each file's directory record is.
    pub records: BTreeMap<String, u64>,
    /// What `keep` holds.
    kept: Mutex<Kept>,
}

/// What has been worked out from an image and is kept with it, by name.
type Kept = HashMap<&'static str, Arc<dyn Any + Send + Sync>>;

impl Disc {
    pub fn open(path: &Path) -> Result<Disc> {
        let image = File::open(path).map_err(at(path))?;
        let mut disc = Disc {
            path: path.to_path_buf(),
            image: Mutex::new(image),
            label: String::new(),
            publisher: String::new(),
            files: BTreeMap::new(),
            records: BTreeMap::new(),
            kept: Mutex::new(HashMap::new()),
        };
        let volume = disc.sectors(VOLUME_SECTOR, SECTOR)?;
        if !volume.starts_with(VOLUME_MAGIC) || volume.len() < SECTOR as usize {
            return image_error(format!("{} is not an ISO 9660 disc image", path.display()));
        }
        disc.label = ascii(&volume[40..72]).trim_matches(is_blank).to_string();
        disc.publisher = ascii(&volume[318..446]).trim_matches(is_blank).to_string();
        let (first, size) = extent(&volume[ROOT_RECORD..]);
        let mut found = Vec::new();
        disc.list(first, size, "", 0, &mut found)?;
        for (name, entry, record) in found {
            disc.files.insert(name.clone(), entry);
            disc.records.insert(name, record);
        }
        Ok(disc)
    }

    /// The open file. A reader that failed part-way leaves nothing behind that the next one relies
    /// on, since each read says where it starts.
    fn image(&self) -> MutexGuard<'_, File> {
        self.image.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Up to `size` bytes of the image from the start of a sector.
    fn sectors(&self, sector: u64, size: u64) -> Result<Vec<u8>> {
        read_at(&mut self.image(), sector * SECTOR, size).map_err(at(&self.path))
    }

    /// The files of a directory and of the directories inside it, added to `found`: each one's
    /// path, where it is, and where in the image its directory record is.
    fn list(
        &self,
        sector: u32,
        size: u32,
        prefix: &str,
        depth: usize,
        found: &mut Vec<(String, (u32, u32), u64)>,
    ) -> Result<()> {
        if depth > DEEPEST_FOLDER {
            return image_error(format!("{} has folders inside folders without end", self.path.display()));
        }
        let data = self.sectors(u64::from(sector), u64::from(size))?;
        let mut position = 0;
        while position < data.len() {
            let length = usize::from(data[position]);
            if length == 0 {
                // A record never crosses a sector; the rest of this one is padding.
                position = (position / SECTOR as usize + 1) * SECTOR as usize;
                continue;
            }
            let record = &data[position..data.len().min(position + length)];
            let place = u64::from(sector) * SECTOR + position as u64;
            position += length;
            if record.len() < RECORD_HEAD {
                return image_error(format!("{} has a directory entry that is cut short", self.path.display()));
            }
            let (first, size) = extent(record);
            let name = &record[RECORD_HEAD..record.len().min(RECORD_HEAD + usize::from(record[32]))];
            if name == [0] || name == [1] {
                continue; // the directory itself and its parent
            }
            let name = ascii(name);
            // ";1" is a version number.
            let path = format!("{prefix}{}", name.split(';').next().unwrap_or_default());
            if record[25] & DIRECTORY != 0 {
                self.list(first, size, &format!("{path}/"), depth + 1, found)?;
            } else {
                found.push((path, (first, size), place));
            }
        }
        Ok(())
    }

    fn entry(&self, name: &str) -> Result<(u32, u32)> {
        match self.files.get(name) {
            Some(entry) => Ok(*entry),
            None => image_error(format!("{} has no file named {name}", self.path.display())),
        }
    }

    /// A file's size in bytes.
    pub fn size(&self, name: &str) -> Result<u64> {
        Ok(u64::from(self.entry(name)?.1))
    }

    /// Where a file starts in the image, in bytes.
    pub fn offset(&self, name: &str) -> Result<u64> {
        Ok(u64::from(self.entry(name)?.0) * SECTOR)
    }

    /// `length` bytes of a file from `start`.
    pub fn read(&self, name: &str, start: u64, length: u64) -> Result<Vec<u8>> {
        let (sector, size) = self.entry(name)?;
        let end = start.saturating_add(length);
        self.span(name, sector, u64::from(size), start, end)
    }

    /// A file's contents from `start` to its end; all of it when `start` is 0.
    pub fn read_from(&self, name: &str, start: u64) -> Result<Vec<u8>> {
        let (sector, size) = self.entry(name)?;
        self.span(name, sector, u64::from(size), start, u64::from(size))
    }

    /// The bytes of a file from `start` up to `end`, which must both be inside it and in that order.
    fn span(&self, name: &str, sector: u32, size: u64, start: u64, end: u64) -> Result<Vec<u8>> {
        if start > end || end > size {
            return image_error(format!("{name} has no bytes {start:#x} to {end:#x}: it is {size:#x} long"));
        }
        read_at(&mut self.image(), u64::from(sector) * SECTOR + start, end - start).map_err(at(&self.path))
    }

    /// The name of the program the console starts, from `SYSTEM.CNF`.
    pub fn boot_name(&self) -> Result<String> {
        match boot_name_in(&ascii(&self.read_from("SYSTEM.CNF", 0)?)) {
            Some(name) => Ok(name),
            None => image_error(format!("{}: SYSTEM.CNF does not name a program to start", self.path.display())),
        }
    }

    /// The game's serial number as it is printed on the case: `SLPS-25172` for `SLPS_251.72`.
    pub fn serial(&self) -> Result<String> {
        Ok(serial_of(&self.boot_name()?))
    }

    /// The version `SYSTEM.CNF` gives on its line `VER = 1.01`, or `None` when it has no such line.
    pub fn version(&self) -> Result<Option<String>> {
        Ok(version_in(&ascii(&self.read_from("SYSTEM.CNF", 0)?)))
    }

    /// SHA-1 of the whole image, read in pieces, in lowercase hexadecimal.
    pub fn sha1(&self) -> Result<String> {
        let mut image = self.image();
        let mut digest = Sha1::new();
        let mut chunk = vec![0; CHUNK];
        image.seek(SeekFrom::Start(0)).map_err(at(&self.path))?;
        loop {
            let count = image.read(&mut chunk).map_err(at(&self.path))?;
            if count == 0 {
                break;
            }
            digest.update(&chunk[..count]);
        }
        Ok(digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// What `make` gives, worked out once for this image and kept under `name` for as long as the
    /// image is open. An adapter keeps here what it reads again and again and what is slow to get
    /// at, such as a table of contents or an expanded font. A failure is not kept.
    pub fn keep<T: Any + Send + Sync>(&self, name: &'static str, make: impl FnOnce() -> Result<T>) -> Result<Arc<T>> {
        let held = self.kept().get(name).cloned();
        if let Some(value) = held.and_then(|value| value.downcast::<T>().ok()) {
            return Ok(value);
        }
        // Made without holding the lock, so that making one thing may ask for another.
        let value = Arc::new(make()?);
        self.kept().insert(name, value.clone());
        Ok(value)
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The program a `SYSTEM.CNF` names, from a line such as `BOOT2 = cdrom0:\SLPS_251.72;1`: the name
/// runs up to the semicolon.
fn boot_name_in(config: &str) -> Option<String> {
    config.match_indices("BOOT2").find_map(|(found, key)| {
        let rest = config[found + key.len()..].trim_start_matches(is_blank).strip_prefix('=')?;
        let rest = rest.trim_start_matches(is_blank).strip_prefix("cdrom0:\\")?;
        let name = rest.split(|char| char == ';' || is_blank(char)).next().filter(|name| !name.is_empty())?;
        Some(name.to_string())
    })
}

/// A program's name as the serial printed on the case: `SLPS-25172` for `SLPS_251.72`.
pub(crate) fn serial_of(boot_name: &str) -> String {
    boot_name.replace('_', "-").replace('.', "")
}

/// The version a `SYSTEM.CNF` gives on its line `VER = 1.01`.
pub(crate) fn version_in(config: &str) -> Option<String> {
    config.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        let value = value.trim_matches(is_blank);
        (key.trim_matches(is_blank) == "VER" && !value.is_empty()).then(|| value.to_string())
    })
}

/// What a disc image says it is, read without opening it as a `Disc`: the volume descriptor, the
/// root folder and `SYSTEM.CNF`, three small reads, where opening it lists every folder on the disc.
/// `None` when it is no ISO 9660 image or names no PlayStation 2 program.
fn peek_label(path: &Path) -> Option<Label> {
    let mut image = File::open(path).ok()?;
    let volume = read_at(&mut image, VOLUME_SECTOR * SECTOR, SECTOR).ok()?;
    if !volume.starts_with(VOLUME_MAGIC) || volume.len() < SECTOR as usize {
        return None;
    }
    let (first, size) = extent(&volume[ROOT_RECORD..]);
    // A root folder is a few sectors; a larger one is not believed.
    let root = read_at(&mut image, u64::from(first) * SECTOR, u64::from(size).min(16 * SECTOR)).ok()?;
    let mut position = 0;
    let mut config = None;
    while position < root.len() {
        let length = usize::from(root[position]);
        if length == 0 {
            position = (position / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        let record = &root[position..root.len().min(position + length)];
        position += length;
        if record.len() < RECORD_HEAD {
            return None;
        }
        let name = &record[RECORD_HEAD..record.len().min(RECORD_HEAD + usize::from(record[32]))];
        if name.split(|byte| *byte == b';').next() == Some(b"SYSTEM.CNF") {
            config = Some(extent(record));
            break;
        }
    }
    let (sector, size) = config?;
    let config = ascii(&read_at(&mut image, u64::from(sector) * SECTOR, u64::from(size).min(SECTOR)).ok()?);
    let name = serial_of(&boot_name_in(&config)?);
    Some(Label { name, version: version_in(&config).unwrap_or_default() })
}

/// Where a directory record says its file or directory is: the first sector, and the size in bytes.
/// Each is stored both little-endian and big-endian; the first is read.
pub(crate) fn extent(record: &[u8]) -> (u32, u32) {
    let number = |at: usize| u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
    (number(2), number(10))
}

/// A 32-bit number as ISO 9660 stores it: little-endian, then big-endian.
fn both_ways(number: u32) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&number.to_le_bytes());
    bytes[4..].copy_from_slice(&number.to_be_bytes());
    bytes
}

/// How many sectors a file of `size` bytes takes.
fn sectors_for(size: u64) -> u64 {
    size.div_ceil(SECTOR)
}

/// New contents for the file that ends an image: its name, and its bytes in pieces.
pub type Tail<'a> = (&'a str, &'a mut dyn Iterator<Item = Result<Vec<u8>>>);

/// A copy of an image while it is being written: the image it is read from and the file it goes to.
struct Copy<'a> {
    source: File,
    source_path: &'a Path,
    out: File,
    out_path: &'a Path,
    buffer: Vec<u8>,
}

impl Copy<'_> {
    /// Copy from where the source is to where the output is, up to `length` bytes or, without a
    /// length, to the end of the source.
    fn copy(&mut self, length: Option<u64>) -> Result<()> {
        let mut left = length.unwrap_or(u64::MAX);
        while left > 0 {
            let wanted = self.buffer.len().min(usize::try_from(left).unwrap_or(usize::MAX));
            let count = self.source.read(&mut self.buffer[..wanted]).map_err(at(self.source_path))?;
            if count == 0 {
                break;
            }
            self.out.write_all(&self.buffer[..count]).map_err(at(self.out_path))?;
            left -= count as u64;
        }
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.out.write_all(data).map_err(at(self.out_path))
    }

    /// Write `data` at `offset` in the output, over what is there.
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        self.out.seek(SeekFrom::Start(offset)).map_err(at(self.out_path))?;
        self.write(data)
    }
}

/// Write a copy of the image to `output` with some of its files changed.
///
/// `patches` are (file name, offset in the file, bytes) to write over what is there. `tail` gives
/// new contents, in pieces and of any size, for the file that ends the image; whatever followed
/// that file follows it again, and the sizes ISO 9660 records are brought up to date.
///
/// What can be refused is refused before anything is written. If writing fails part-way, what was
/// written is removed.
pub fn write_image(disc: &Disc, output: &Path, patches: &[(&str, u64, Vec<u8>)], tail: Option<Tail<'_>>) -> Result<()> {
    let image_size = fs::metadata(&disc.path).map_err(at(&disc.path))?.len();
    // The file that ends the image, when it is given new contents: its first sector and its size.
    let mut ending = None;
    if let Some((name, _)) = &tail {
        let last = disc.files.values().map(|(sector, _)| *sector).max();
        match disc.files.get(*name) {
            Some((first, old_size)) if Some(*first) == last => ending = Some((u64::from(*first), u64::from(*old_size))),
            _ => return image_error(format!("{name} is not the file that ends {}", disc.path.display())),
        }
    }
    // Where in the image each patch goes.
    let mut places = Vec::new();
    for (name, offset, data) in patches {
        let end = offset.saturating_add(data.len() as u64);
        if end > disc.size(name)? {
            return image_error(format!("{name} has no bytes {offset:#x} to {end:#x}"));
        }
        places.push((disc.offset(name)? + offset, data));
    }
    // The image is still open for reading, and opening it for writing would empty it.
    if let (Ok(source), Ok(target)) = (fs::canonicalize(&disc.path), fs::canonicalize(output))
        && source == target
    {
        return image_error(format!("{} is the image being read: a copy cannot be written over it", output.display()));
    }

    let source = File::open(&disc.path).map_err(at(&disc.path))?;
    let out = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(output).map_err(at(output))?;
    let mut copy = Copy { source, source_path: &disc.path, out, out_path: output, buffer: vec![0; CHUNK] };
    let written = (|| -> Result<()> {
        copy.copy(Some(ending.map_or(image_size, |(first, _)| first * SECTOR)))?;
        if let (Some((name, pieces)), Some((first, old_size))) = (tail, ending) {
            let mut new_size = 0;
            for piece in pieces {
                let piece = piece?;
                copy.write(&piece)?;
                new_size += piece.len() as u64;
            }
            let Ok(recorded_size) = u32::try_from(new_size) else {
                return image_error(format!("{name} would be {new_size} bytes, more than a file on a disc can be"));
            };
            copy.write(&vec![0; (new_size.wrapping_neg() % SECTOR) as usize])?;
            let followed = (first + sectors_for(old_size)) * SECTOR;
            copy.source.seek(SeekFrom::Start(followed)).map_err(at(&disc.path))?;
            copy.copy(None)?; // what followed the file
            if new_size != old_size {
                copy.write_at(disc.records[name] + RECORD_SIZE, &both_ways(recorded_size))?;
            }
            if sectors_for(new_size) != sectors_for(old_size) {
                let sectors = image_size / SECTOR + sectors_for(new_size) - sectors_for(old_size);
                let Ok(recorded_sectors) = u32::try_from(sectors) else {
                    return image_error(format!("the image would be {sectors} sectors, more than a disc can be"));
                };
                copy.write_at(VOLUME_SECTOR * SECTOR + VOLUME_SIZE, &both_ways(recorded_sectors))?;
                // The UDF marker in the last sector says which sector it is in, with a checksum over that.
                let last = recorded_sectors.saturating_sub(1);
                let mut tag = read_at(&mut copy.out, u64::from(last) * SECTOR, 16).map_err(at(output))?;
                if tag.len() == 16 && u16::from_le_bytes([tag[0], tag[1]]) == UDF_ANCHOR {
                    tag[12..].copy_from_slice(&last.to_le_bytes());
                    tag[4] = 0;
                    tag[4] = tag.iter().fold(0, |sum: u8, byte| sum.wrapping_add(*byte));
                    copy.write_at(u64::from(last) * SECTOR, &tag)?;
                }
            }
        }
        for (place, data) in places {
            copy.write_at(place, data)?;
        }
        Ok(())
    })();
    drop(copy); // the file is closed before it is removed
    if written.is_err() {
        let _ = fs::remove_file(output);
    }
    written
}

/// A disc image is too large to hold whole: adapters are given a `Disc`, which stays open.
pub struct Ps2;

pub static PLATFORM: Ps2 = Ps2;

impl Platform for Ps2 {
    fn name(&self) -> &'static str {
        "PlayStation 2"
    }

    fn recognises(&self, path: &Path) -> Result<bool> {
        Ok(is_disc(path))
    }

    fn glance(&self, path: &Path) -> Result<Option<Glance>> {
        // A disc image is kept as it is, under one of a few endings; it is too large for an archive.
        let ending = path.extension().map(|ending| ending.to_string_lossy().to_ascii_lowercase());
        if !matches!(ending.as_deref(), Some("iso" | "bin" | "img")) {
            return Ok(None);
        }
        // An image of 2048-byte sectors; a PlayStation disc's .bin, of 2352-byte ones, is not one.
        let size = fs::metadata(path).map_err(at(path))?.len();
        if !size.is_multiple_of(SECTOR) {
            return Ok(None);
        }
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Some(Glance { size, name }))
    }

    fn open(&self, path: &Path) -> Result<(Image, String)> {
        let disc = Disc::open(path)?;
        let sha1 = disc.sha1()?;
        Ok((Image::Disc(disc), sha1))
    }

    /// The serial and version `SYSTEM.CNF` gives. A disc without one is not a PlayStation 2 game
    /// that can be named, such as a PlayStation disc, whose `SYSTEM.CNF` has no `BOOT2` line.
    fn label(&self, image: &Image) -> Option<Label> {
        let disc = image.disc().ok()?;
        let name = disc.serial().ok()?;
        Some(Label { name, version: disc.version().ok().flatten().unwrap_or_default() })
    }

    fn label_kind(&self) -> &'static str {
        "serial"
    }

    fn peek(&self, path: &Path) -> Option<Label> {
        peek_label(path)
    }

    fn disc(&self) -> bool {
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// One entry of an ISO 9660 directory.
    fn record(name: &[u8], sector: u32, size: u32, directory: bool) -> Vec<u8> {
        let mut body = [sector.to_le_bytes(), sector.to_be_bytes(), size.to_le_bytes(), size.to_be_bytes()].concat();
        body.extend([0; 7]);
        body.extend([if directory { DIRECTORY } else { 0 }, 0, 0, 1, 0, 0, 1, name.len() as u8]);
        body.extend(name);
        body.resize(body.len().next_multiple_of(2), 0); // records have an even length
        [&[body.len() as u8 + 2, 0], body.as_slice()].concat()
    }

    /// Data followed by zeros up to the end of its last sector.
    fn sector(data: &[u8]) -> Vec<u8> {
        let mut padded = data.to_vec();
        padded.resize(data.len().next_multiple_of(SECTOR as usize), 0);
        padded
    }

    /// The start of the UDF marker a disc has in its last sector: it names that sector, with a checksum.
    fn anchor(sector: u32) -> Vec<u8> {
        let mut tag = [&[2, 0, 2, 0, 0, 0, 0, 0, 0x34, 0x12, 0xF0, 0x01], sector.to_le_bytes().as_slice()].concat();
        tag[4] = tag.iter().fold(0, |sum: u8, byte| sum.wrapping_add(*byte));
        [tag.as_slice(), b"the rest of the marker"].concat()
    }

    const CONFIG: &[u8] = b"BOOT2 = cdrom0:\\SLPS_123.45;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
    const NOTE: &[u8] = b"in a folder";

    /// The contents of a file of a sector and a half.
    fn data() -> Vec<u8> {
        (0..=255).collect::<Vec<u8>>().repeat(12)
    }

    /// A disc with two files in its root and one in a folder, written to `folder`.
    fn image(folder: &Path) -> PathBuf {
        disc_with(folder, "disc.iso", CONFIG)
    }

    /// The same disc with another `SYSTEM.CNF`, under another name: other tests make one to say a
    /// game's serial and version.
    pub(crate) fn disc_with(folder: &Path, name: &str, config: &[u8]) -> PathBuf {
        let whole = SECTOR as u32;
        let root = [
            record(&[0], 18, whole, true),
            record(&[1], 18, whole, true),
            record(b"SYSTEM.CNF;1", 20, config.len() as u32, false),
            record(b"DATA.BIN;1", 21, data().len() as u32, false),
            record(b"SUB", 19, whole, true),
        ]
        .concat();
        let sub = [
            record(&[0], 19, whole, true),
            record(&[1], 18, whole, true),
            record(b"NOTE.TXT;1", 23, NOTE.len() as u32, false),
        ]
        .concat();
        let mut volume = vec![0; SECTOR as usize];
        volume[..7].copy_from_slice(b"\x01CD001\x01");
        volume[40..72].copy_from_slice(format!("{:<32}", "TESTDISC").as_bytes());
        volume[318..446].copy_from_slice(format!("{:<128}", "A PUBLISHER").as_bytes());
        volume[156..190].copy_from_slice(&record(&[0], 18, whole, true));
        volume[80..88].copy_from_slice(&both_ways(26));
        let path = folder.join(name);
        let sectors = [
            vec![0; 16 * SECTOR as usize],
            volume,
            vec![0; SECTOR as usize],
            sector(&root),
            sector(&sub),
            sector(config),
            sector(&data()),
            sector(NOTE),
            sector(b"after the last file"),
            sector(&anchor(25)),
        ];
        fs::write(&path, sectors.concat()).unwrap();
        path
    }

    /// What is wrong with an image, or with what was asked of it, in the words of the error.
    fn fault<T>(result: Result<T>) -> String {
        match result {
            Err(error @ Error::Image(_)) => error.to_string(),
            Err(other) => panic!("refused, but not as something an image cannot give: {other:?}"),
            Ok(_) => panic!("it was not refused"),
        }
    }

    /// The pieces of a file's new contents, as `write_image` takes them.
    fn pieces(pieces: &[&[u8]]) -> impl Iterator<Item = Result<Vec<u8>>> {
        pieces.iter().map(|piece| Ok(piece.to_vec())).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn write_image_changes_bytes_inside_files() {
        let folder = tempfile::tempdir().unwrap();
        let image = image(folder.path());
        let out = folder.path().join("out.iso");
        let disc = Disc::open(&image).unwrap();
        write_image(&disc, &out, &[], None).unwrap();
        assert!(fs::read(&out).unwrap() == fs::read(&image).unwrap());
        let patches = [("DATA.BIN", 0x7FE, b"WXYZ".to_vec()), ("SYSTEM.CNF", 0, b"boot".to_vec())];
        write_image(&disc, &out, &patches, None).unwrap();
        // A patch that does not fit its file is refused before anything is written.
        let bad = folder.path().join("bad.iso");
        let too_long = [("SUB/NOTE.TXT", 8, b"too long".to_vec())];
        assert_eq!(fault(write_image(&disc, &bad, &too_long, None)), "SUB/NOTE.TXT has no bytes 0x8 to 0x10");
        assert!(fault(write_image(&disc, &bad, &[("MISSING.BIN", 0, vec![1])], None)).contains("no file named"));
        assert!(!bad.exists());
        let built = Disc::open(&out).unwrap();
        let patched = [&[0xFC, 0xFD], b"WXYZ".as_slice(), &[0x02, 0x03]].concat();
        assert_eq!(built.read("DATA.BIN", 0x7FC, 8).unwrap(), patched);
        assert_eq!(built.read("SYSTEM.CNF", 0, 8).unwrap(), b"boot2 = ");
        assert_eq!(built.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
        assert_eq!(fs::metadata(&out).unwrap().len(), fs::metadata(&image).unwrap().len());
    }

    #[test]
    fn write_image_gives_the_last_file_any_size() {
        let folder = tempfile::tempdir().unwrap();
        let image = image(folder.path());
        let out = folder.path().join("out.iso");
        let disc = Disc::open(&image).unwrap();
        // The same size: nothing else changes.
        write_image(&disc, &out, &[], Some(("SUB/NOTE.TXT", &mut pieces(&[b"IN A ", b"FOLDER"])))).unwrap();
        let mut expected = fs::read(&image).unwrap();
        expected[23 * SECTOR as usize..][..NOTE.len()].copy_from_slice(b"IN A FOLDER");
        assert!(fs::read(&out).unwrap() == expected);
        // Three sectors where there was one: what followed still follows, two sectors later.
        let grown = (0..=255).collect::<Vec<u8>>().repeat(20);
        let halves = [&grown[..3000], &grown[3000..]];
        write_image(&disc, &out, &[], Some(("SUB/NOTE.TXT", &mut pieces(&halves)))).unwrap();
        let bad = folder.path().join("bad.iso");
        let not_last = write_image(&disc, &bad, &[], Some(("DATA.BIN", &mut pieces(&[b""]))));
        assert!(fault(not_last).starts_with("DATA.BIN is not the file that ends "));
        assert!(!bad.exists());
        let data = fs::read(&out).unwrap();
        let sector = SECTOR as usize;
        assert_eq!(data.len(), 28 * sector);
        assert!(data[26 * sector..].starts_with(b"after the last file"));
        assert!(data[27 * sector..].starts_with(&anchor(27))); // the marker names its new sector
        assert_eq!(data[16 * sector + 80..][..8], both_ways(28));
        let built = Disc::open(&out).unwrap();
        assert_eq!(built.files["SUB/NOTE.TXT"], (23, grown.len() as u32));
        assert_eq!(built.read_from("SUB/NOTE.TXT", 0).unwrap(), grown);
        assert_eq!(built.read_from("DATA.BIN", 0).unwrap(), self::data());
    }

    #[test]
    fn a_copy_that_cannot_be_finished_is_not_left_behind() {
        let folder = tempfile::tempdir().unwrap();
        let image = image(folder.path());
        let out = folder.path().join("out.iso");
        let disc = Disc::open(&image).unwrap();
        // The contents of the last file stop coming part-way.
        let mut failing = [Ok(b"IN A ".to_vec()), image_error("no more")].into_iter();
        assert_eq!(fault(write_image(&disc, &out, &[], Some(("SUB/NOTE.TXT", &mut failing)))), "no more");
        assert!(!out.exists());
        // The image that is open is never written over.
        let before = fs::read(&image).unwrap();
        assert!(fault(write_image(&disc, &image, &[], None)).contains("is the image being read"));
        assert!(fs::read(&image).unwrap() == before);
    }

    #[test]
    fn lists_files_in_folders() {
        let folder = tempfile::tempdir().unwrap();
        let disc = Disc::open(&image(folder.path())).unwrap();
        let expected = [("DATA.BIN", (21, 3072)), ("SUB/NOTE.TXT", (23, 11)), ("SYSTEM.CNF", (20, 57))];
        assert_eq!(disc.files, BTreeMap::from(expected.map(|(name, place)| (name.to_string(), place))));
        // Each file's record is where its directory has it: the note's is the third in its folder.
        assert_eq!(disc.records["SUB/NOTE.TXT"], 19 * SECTOR + 68);
        assert_eq!(disc.records.keys().collect::<Vec<_>>(), disc.files.keys().collect::<Vec<_>>());
        assert_eq!(disc.label, "TESTDISC");
        assert_eq!(disc.publisher, "A PUBLISHER");
    }

    #[test]
    fn reads_a_file_or_a_part_of_one() {
        let folder = tempfile::tempdir().unwrap();
        let disc = Disc::open(&image(folder.path())).unwrap();
        assert_eq!(disc.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
        assert_eq!(disc.read_from("DATA.BIN", 0).unwrap(), data());
        assert_eq!(disc.read("DATA.BIN", 0x7FE, 4).unwrap(), [0xFE, 0xFF, 0x00, 0x01]); // across two sectors
        assert_eq!(disc.read_from("DATA.BIN", 3072).unwrap(), b"");
        assert_eq!(disc.size("DATA.BIN").unwrap(), 3072);
        assert_eq!(disc.offset("DATA.BIN").unwrap(), 21 * SECTOR);
    }

    #[test]
    fn names_the_program_the_console_starts() {
        let folder = tempfile::tempdir().unwrap();
        let disc = Disc::open(&image(folder.path())).unwrap();
        assert_eq!(disc.boot_name().unwrap(), "SLPS_123.45");
        assert_eq!(disc.serial().unwrap(), "SLPS-12345");
        assert_eq!(disc.version().unwrap().as_deref(), Some("1.00"));
    }

    #[test]
    fn a_disc_says_its_serial_and_version() {
        let folder = tempfile::tempdir().unwrap();
        let (image, _) = PLATFORM.open(&image(folder.path())).unwrap();
        let label = PLATFORM.label(&image).unwrap();
        assert_eq!((label.name.as_str(), label.version.as_str()), ("SLPS-12345", "1.00"));
        assert_eq!(
            crate::platform::label_words(&PLATFORM, &label.name, &label.version),
            "serial SLPS-12345, version 1.00"
        );
        // No version line: the serial alone. No BOOT2 line, as on a PlayStation disc: nothing to name it by.
        let bare = disc_with(folder.path(), "bare.iso", b"BOOT2 = cdrom0:\\SLPM_660.01;1\r\n");
        let (bare, _) = PLATFORM.open(&bare).unwrap();
        assert_eq!(PLATFORM.label(&bare), Some(Label { name: "SLPM-66001".into(), version: String::new() }));
        let first = disc_with(folder.path(), "ps1.iso", b"BOOT = cdrom:\\SLPS_000.01;1\r\nVER = 1.0\r\n");
        let (first, _) = PLATFORM.open(&first).unwrap();
        assert_eq!(PLATFORM.label(&first), None);
        assert_eq!(PLATFORM.label(&Image::Cartridge(Vec::new())), None);
    }

    #[test]
    fn hashes_the_whole_image() {
        let folder = tempfile::tempdir().unwrap();
        let image = image(folder.path());
        let disc = Disc::open(&image).unwrap();
        let whole = Sha1::digest(fs::read(&image).unwrap());
        assert_eq!(disc.sha1().unwrap(), whole.iter().map(|byte| format!("{byte:02x}")).collect::<String>());
        assert_eq!(disc.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE); // reading still works afterwards
    }

    #[test]
    fn errors() {
        let folder = tempfile::tempdir().unwrap();
        let image = image(folder.path());
        let disc = Disc::open(&image).unwrap();
        assert!(fault(disc.read_from("MISSING.BIN", 0)).ends_with("has no file named MISSING.BIN"));
        assert_eq!(fault(disc.read("SUB/NOTE.TXT", 8, 8)), "SUB/NOTE.TXT has no bytes 0x8 to 0x10: it is 0xb long");
        assert_eq!(fault(disc.read_from("SUB/NOTE.TXT", 12)), "SUB/NOTE.TXT has no bytes 0xc to 0xb: it is 0xb long");
        let other = folder.path().join("game.sfc");
        fs::write(&other, vec![0; 0x10000]).unwrap();
        assert!(is_disc(&image));
        assert!(!is_disc(&other));
        assert!(!is_disc(&folder.path().join("absent.iso")));
        assert!(!is_disc(folder.path()));
        assert!(fault(Disc::open(&other)).ends_with("is not an ISO 9660 disc image"));
        assert!(matches!(Disc::open(&folder.path().join("absent.iso")), Err(Error::Io(_))));
    }

    #[test]
    fn what_is_kept_for_an_image_is_worked_out_once() {
        let folder = tempfile::tempdir().unwrap();
        let disc = Disc::open(&image(folder.path())).unwrap();
        let mut made = 0;
        let mut size = || {
            disc.keep("the size of the note", || {
                made += 1;
                disc.size("SUB/NOTE.TXT")
            })
        };
        assert_eq!((*size().unwrap(), *size().unwrap()), (11, 11));
        assert_eq!(made, 1);
        // A failure is not kept, and what is kept under another name is another thing.
        assert_eq!(fault(disc.keep("nothing", || image_error::<u64>("not yet"))), "not yet");
        assert_eq!(*disc.keep("nothing", || Ok("now".to_string())).unwrap(), "now");
    }

    #[test]
    fn the_platform_opens_a_disc_and_names_it_by_its_sha1() {
        let folder = tempfile::tempdir().unwrap();
        let path = image(folder.path());
        let other = folder.path().join("game.sfc");
        fs::write(&other, vec![0; 0x10000]).unwrap();
        assert_eq!(PLATFORM.name(), "PlayStation 2");
        assert!(PLATFORM.recognises(&path).unwrap());
        // A file that is not there is not a disc, and it is for another console to say what it is.
        assert!(!PLATFORM.recognises(&other).unwrap());
        assert!(!PLATFORM.recognises(&folder.path().join("absent.iso")).unwrap());
        let (image, sha1) = PLATFORM.open(&path).unwrap();
        assert_eq!(sha1, image.disc().unwrap().sha1().unwrap());
        assert_eq!(image.disc().unwrap().read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
        assert_eq!(fault(image.cartridge()), "the image is not a cartridge");
        assert_eq!(fault(Image::Cartridge(Vec::new()).disc().map(|_| ())), "the image is not a disc");
        assert!(PLATFORM.changes(&image, &image).is_empty());
    }
}
