//! PlayStation disc images: finding and reading the files inside one, and writing a changed copy.
//!
//! A PlayStation game is on a CD, and an image of one keeps every sector whole: 2352 bytes, of which
//! the game's own data is only a part. Redump, the catalogue of disc dumps, gives each track as a
//! `.bin` file of such sectors with a `.cue` sheet beside it that lists the tracks, and its SHA-1 is
//! that of the `.bin` that holds the data track. That `.bin` is what an image is identified by.
//!
//! The data track is in the CD-ROM XA format, "Mode 2". Each sector starts with 12 bytes of sync
//! (00, ten FF, 00), then a header of 4 bytes (where the sector is, as minutes, seconds and frames in
//! binary-coded decimal, counted from 00:02:00 at the first sector, and the mode, 2), then a
//! subheader of 8 bytes (file, channel, submode, coding, and the same 4 again). The submode says
//! which of two forms the sector takes:
//!
//! - Form 1, for data: 2048 bytes at offset 24, an EDC of 4 bytes (a 32-bit checksum of the
//!   subheader and the data), then 276 bytes of ECC (Reed-Solomon parity that lets a drive repair a
//!   damaged sector).
//! - Form 2, for movies and voices, where speed matters more than every byte: 2324 bytes at offset
//!   24 and an EDC of 4 bytes, which a disc may leave as zero.
//!
//! The file system, ISO 9660 as on a PlayStation 2 disc, is in the Form 1 data: its volume
//! descriptor is in sector 16, and each file is a run of sectors whose 2048 bytes of data follow on
//! from each other. `Disc` reads files that way. Movies (`.STR`) and streamed sound (XA) mix Form 1
//! and Form 2 sectors in one file; an adapter that needs them reads whole sectors with `Disc::raw`.
//!
//! Either file may be given. A `.cue` is read for the name of its data track's `.bin` (its first
//! track, which must be `MODE2/2352`), and that `.bin` is what is opened and hashed. A `.bin` given
//! alone is taken to be the data track. Other tracks, such as CD audio in files of their own, are not
//! read. A search of a folder of games looks only at `.bin` and `.img` files, so that a game kept
//! with its `.cue` is found once, by its `.bin`.
//!
//! `write_image` writes a changed copy: bytes replaced inside files, which keep their size, written
//! to a new `.bin` with a `.cue` beside it. Each sector that changed is given a new EDC and ECC;
//! every other sector is copied as it is, so that a copy with no change is the original byte for
//! byte. A file cannot be made larger yet: that would mean moving the files after it and rewriting
//! the folders that say where they are, and a change that does not fit in a file is refused.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sha1::{Digest, Sha1};

use crate::error::{Error, Result, at};
use crate::platform::{Glance, Image, Label, Platform};
use crate::platforms::ps2::{
    DEEPEST_FOLDER, DIRECTORY, RECORD_HEAD, ROOT_RECORD, VOLUME_MAGIC, VOLUME_SECTOR, ascii, extent, is_blank,
    serial_of, version_in,
};

pub use crate::platforms::ps2::REDUMP;

/// A whole sector, as an image keeps it.
pub const RAW_SECTOR: usize = 2352;
/// The data a Form 1 sector carries, and so the size of a sector to the file system.
pub const SECTOR: u64 = 2048;
/// The data a Form 2 sector carries.
pub const FORM2_DATA: usize = 2324;
/// Where a sector's data starts: after the sync, the header and the subheader.
pub const DATA: usize = 24;
/// The 12 bytes every sector starts with.
const SYNC: [u8; 12] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
/// Where the header is: minute, second, frame, mode.
const HEADER: usize = 12;
/// Where the subheader is: file, channel, submode, coding, then the same again.
const SUBHEADER: usize = 16;
/// The bit of the submode that marks a Form 2 sector.
const FORM2: u8 = 0x20;
/// Where a Form 1 sector's EDC is, after its data; its ECC follows.
const FORM1_EDC: usize = DATA + SECTOR as usize;
/// Where a Form 1 sector's ECC starts: P parity, 172 bytes, then Q parity, 104 bytes.
const P_PARITY: usize = FORM1_EDC + 4;
const Q_PARITY: usize = P_PARITY + 172;
/// Where a Form 2 sector's EDC is, after its data.
const FORM2_EDC: usize = DATA + FORM2_DATA;
/// How many sectors are read or copied at a time: about 2.4 MB.
const BATCH: usize = 1024;

fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

/// The table the EDC is worked out with: CRC-32 over the polynomial 0x8001801B, taken bit-reversed
/// (0xD8018001) as the CD-ROM standard has it, one entry per byte value.
const EDC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ if value & 1 != 0 { 0xD801_8001 } else { 0 };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
};

/// The EDC of some bytes: a CRC-32 that starts from zero and is not inverted at the end. A sector
/// stores it little-endian after its data.
pub fn edc(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0, |edc, byte| (edc >> 8) ^ EDC_TABLE[((edc ^ u32::from(*byte)) & 0xFF) as usize])
}

/// Multiplication by 2 in the Galois field of 256 elements the ECC is worked out in, whose
/// polynomial is x⁸ + x⁴ + x³ + x² + 1 (0x11D): `ECC_DOUBLE[x]` is 2·x.
const ECC_DOUBLE: [u8; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        let doubled = (index << 1) ^ if index & 0x80 != 0 { 0x11D } else { 0 };
        table[index] = doubled as u8;
        index += 1;
    }
    table
};

/// Division by 3 in the same field: `ECC_THIRD[x ^ 2·x]` is x, since x ^ 2·x is 3·x.
const ECC_THIRD: [u8; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        table[index ^ ECC_DOUBLE[index] as usize] = index as u8;
        index += 1;
    }
    table
};

/// One set of Reed-Solomon parity, P or Q, over `source`, written to `out`.
///
/// The CD-ROM standard lays the bytes out as a grid of 16-bit words and takes parity along its
/// columns (P) and its diagonals (Q). Here that is said with steps through the bytes: there are
/// `majors` codewords, each of `minors` bytes; codeword `major` starts at
/// `(major / 2) * major_step + major % 2` (the two bytes of a word belong to two codewords) and moves
/// on by `minor_step`, wrapping round at the end of the grid. Each codeword gives two parity bytes,
/// at `major` and at `major + majors`.
fn parity(source: &[u8], majors: usize, minors: usize, major_step: usize, minor_step: usize, out: &mut [u8]) {
    let size = majors * minors;
    for major in 0..majors {
        let mut index = (major >> 1) * major_step + (major & 1);
        let (mut weighted, mut sum) = (0u8, 0u8);
        for _ in 0..minors {
            let byte = source[index];
            index += minor_step;
            if index >= size {
                index -= size;
            }
            weighted ^= byte;
            sum ^= byte;
            weighted = ECC_DOUBLE[usize::from(weighted)];
        }
        weighted = ECC_THIRD[usize::from(ECC_DOUBLE[usize::from(weighted)] ^ sum)];
        out[major] = weighted;
        out[major + majors] = weighted ^ sum;
    }
}

/// Write the ECC of a Mode 2 Form 1 sector: P parity over the header to the EDC, then Q parity over
/// the same and P. The CD-ROM XA standard has the header taken as zero for this, so that a sector
/// can be moved without its parity changing; it is put back afterwards.
pub fn ecc(sector: &mut [u8]) {
    let header: [u8; 4] = sector[HEADER..SUBHEADER].try_into().expect("a header is 4 bytes");
    sector[HEADER..SUBHEADER].fill(0);
    let mut p = [0; 172];
    parity(&sector[HEADER..P_PARITY], 86, 24, 2, 86, &mut p);
    sector[P_PARITY..Q_PARITY].copy_from_slice(&p);
    let mut q = [0; 104];
    parity(&sector[HEADER..Q_PARITY], 52, 43, 86, 88, &mut q);
    sector[Q_PARITY..RAW_SECTOR].copy_from_slice(&q);
    sector[HEADER..SUBHEADER].copy_from_slice(&header);
}

/// Whether a whole sector is a Form 2 one, by its subheader's submode.
pub fn is_form2(sector: &[u8]) -> bool {
    sector[SUBHEADER + 2] & FORM2 != 0
}

/// Write the check bytes of a whole Mode 2 sector to match what is in it: for Form 1, its EDC and
/// its ECC; for Form 2, its EDC. The sync, the header and the subheader are left as they are.
pub fn seal(sector: &mut [u8]) {
    if is_form2(sector) {
        let sum = edc(&sector[SUBHEADER..FORM2_EDC]);
        sector[FORM2_EDC..RAW_SECTOR].copy_from_slice(&sum.to_le_bytes());
    } else {
        let sum = edc(&sector[SUBHEADER..FORM1_EDC]);
        sector[FORM1_EDC..P_PARITY].copy_from_slice(&sum.to_le_bytes());
        ecc(sector);
    }
}

/// Whether a whole sector starts as a Mode 2 sector does: the sync, then mode 2 in its header.
fn is_mode2(sector: &[u8]) -> bool {
    sector.len() >= RAW_SECTOR && sector[..HEADER] == SYNC && sector[HEADER + 3] == 2
}

/// Fill `buffer` from `file` as far as the file goes; how many bytes were read.
fn read_full(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Whole sectors from `first` on, up to `count`; fewer when the image ends first.
fn read_raw(file: &mut File, first: u64, count: u64) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(first * RAW_SECTOR as u64))?;
    let mut data = Vec::new();
    file.take(count * RAW_SECTOR as u64).read_to_end(&mut data)?;
    data.truncate(data.len() / RAW_SECTOR * RAW_SECTOR);
    Ok(data)
}

/// Up to `length` bytes of the Form 1 data that runs on from sector `first`, starting `skip` bytes
/// into it: the 2048 bytes of each sector, one after the other. Fewer when the image ends first.
fn read_data(file: &mut File, first: u64, skip: u64, length: u64) -> io::Result<Vec<u8>> {
    let mut sector = first + skip / SECTOR;
    let mut skip = (skip % SECTOR) as usize;
    let mut left = length;
    let mut out = Vec::with_capacity(length.min((BATCH as u64) * SECTOR) as usize);
    let mut raw = vec![0; BATCH * RAW_SECTOR];
    while left > 0 {
        let wanted = (skip as u64 + left).div_ceil(SECTOR).min(BATCH as u64) as usize;
        file.seek(SeekFrom::Start(sector * RAW_SECTOR as u64))?;
        let whole = read_full(file, &mut raw[..wanted * RAW_SECTOR])? / RAW_SECTOR;
        for one in raw[..whole * RAW_SECTOR].as_chunks::<RAW_SECTOR>().0.iter() {
            let data = &one[DATA + skip..FORM1_EDC];
            let taken = data.len().min(usize::try_from(left).unwrap_or(usize::MAX));
            out.extend_from_slice(&data[..taken]);
            left -= taken as u64;
            skip = 0;
        }
        if whole < wanted {
            break;
        }
        sector += whole as u64;
    }
    Ok(out)
}

/// Whether the file at `path` is a raw image of a PlayStation disc's data track: sector 16, read as
/// a whole Mode 2 sector, holds the ISO 9660 volume descriptor.
pub fn is_raw_disc(path: &Path) -> bool {
    let sector = File::open(path).and_then(|mut image| read_raw(&mut image, VOLUME_SECTOR, 1));
    sector.is_ok_and(|sector| is_mode2(&sector) && sector[DATA..].starts_with(VOLUME_MAGIC))
}

/// Whether a path names a cue sheet, by its ending.
fn is_cue(path: &Path) -> bool {
    path.extension().is_some_and(|ending| ending.eq_ignore_ascii_case("cue"))
}

/// One `FILE` of a cue sheet: the name it gives, and its tracks, each as its number and its mode
/// ("MODE2/2352", "AUDIO").
#[derive(Debug, PartialEq)]
struct CueFile {
    name: String,
    tracks: Vec<(u32, String)>,
}

/// The name a `FILE` line gives, which may be in quotes, and what follows it.
fn cue_name(rest: &str) -> Option<(&str, &str)> {
    let rest = rest.trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        quoted.split_once('"')
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some((&rest[..end], &rest[end..]))
    }
    .filter(|(name, _)| !name.is_empty())
}

/// The files and tracks of a cue sheet. Lines it does not need, such as `INDEX`, are passed over.
fn parse_cue(text: &str) -> Vec<CueFile> {
    let mut files: Vec<CueFile> = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next().map(str::to_ascii_uppercase).as_deref() {
            Some("FILE") => {
                let rest = line.trim_start()[4..].to_string();
                if let Some((name, _)) = cue_name(&rest) {
                    files.push(CueFile { name: name.to_string(), tracks: Vec::new() });
                }
            }
            Some("TRACK") => {
                let number = words.next().and_then(|number| number.parse().ok());
                let mode = words.next().map(str::to_ascii_uppercase);
                if let (Some(file), Some(number), Some(mode)) = (files.last_mut(), number, mode) {
                    file.tracks.push((number, mode));
                }
            }
            _ => {}
        }
    }
    files
}

/// The `.bin` a cue sheet's data track is in: the file that holds track 1, which must be a Mode 2
/// track of whole sectors. A name in the sheet is taken from the sheet's own folder.
fn data_track(cue: &Path) -> Result<PathBuf> {
    // A cue sheet is a few lines; a large file with its ending is not read whole to find out.
    if fs::metadata(cue).map_err(at(cue))?.len() > 1 << 16 {
        return image_error(format!("{} is too large to be a cue sheet", cue.display()));
    }
    let text = fs::read(cue).map_err(at(cue))?;
    let files = parse_cue(&String::from_utf8_lossy(&text));
    let first = files.iter().find(|file| file.tracks.first().is_some_and(|(number, _)| *number == 1));
    match first {
        Some(file) if file.tracks[0].1 == "MODE2/2352" => Ok(cue.parent().unwrap_or(Path::new("")).join(&file.name)),
        Some(file) => image_error(format!(
            "{}: its first track is {}, not a PlayStation data track (MODE2/2352)",
            cue.display(),
            file.tracks[0].1
        )),
        None => image_error(format!("{} names no first track", cue.display())),
    }
}

/// An open disc image. It is closed when it is dropped. Until then Windows refuses to delete the
/// file or to write another in its place, so an image that was built is dropped before it is removed.
pub struct Disc {
    /// The `.bin` of the data track: what is read, and what the SHA-1 is of.
    pub path: PathBuf,
    /// The cue sheet that goes with it: the one that was opened, or one with the same name beside the
    /// `.bin` that names it. A copy that is written has its own, made from this one.
    pub cue: Option<PathBuf>,
    image: Mutex<File>,
    /// The name the disc gives itself.
    pub label: String,
    pub publisher: String,
    /// Each file's path, such as `MOVIE/OPENNING.STR`, and where it is: its first sector and its
    /// size in bytes, as its directory record gives them.
    pub files: BTreeMap<String, (u32, u32)>,
    /// Where each file's directory record is, as a place in the Form 1 data: the sector it is in
    /// times 2048, plus where in that sector's data it starts.
    pub records: BTreeMap<String, u64>,
    /// What `keep` holds.
    kept: Mutex<Kept>,
}

/// What has been worked out from an image and is kept with it, by name.
type Kept = HashMap<&'static str, Arc<dyn Any + Send + Sync>>;

impl Disc {
    /// Open a disc image from its `.bin` or its `.cue`.
    pub fn open(path: &Path) -> Result<Disc> {
        let (bin, cue) = if is_cue(path) {
            (data_track(path)?, Some(path.to_path_buf()))
        } else {
            // A cue sheet beside the .bin, under the same name, is kept with it if it names it.
            let beside = path.with_extension("cue");
            let names_it = |cue: &Path| data_track(cue).ok().and_then(|bin| fs::canonicalize(bin).ok());
            let cue = names_it(&beside).filter(|bin| fs::canonicalize(path).is_ok_and(|own| own == *bin));
            (path.to_path_buf(), cue.map(|_| beside))
        };
        let image = File::open(&bin).map_err(at(&bin))?;
        let mut disc = Disc {
            path: bin,
            cue,
            image: Mutex::new(image),
            label: String::new(),
            publisher: String::new(),
            files: BTreeMap::new(),
            records: BTreeMap::new(),
            kept: Mutex::new(HashMap::new()),
        };
        let first = disc.raw(VOLUME_SECTOR as u32, 1)?;
        if !is_mode2(&first) || !first[DATA..].starts_with(VOLUME_MAGIC) {
            return image_error(format!(
                "{} is not the data track of a PlayStation disc: an image of whole 2352-byte Mode 2 sectors",
                disc.path.display()
            ));
        }
        let volume = disc.data(VOLUME_SECTOR as u32, 0, SECTOR)?;
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

    /// How many sectors the image holds.
    pub fn sector_count(&self) -> Result<u32> {
        let size = fs::metadata(&self.path).map_err(at(&self.path))?.len();
        Ok((size / RAW_SECTOR as u64) as u32)
    }

    /// Up to `count` whole sectors of 2352 bytes from `first` on, as the image keeps them; fewer
    /// when the image ends first. For what the file system does not give: Form 2 sectors, and what
    /// a sector's header and subheader say.
    pub fn raw(&self, first: u32, count: u32) -> Result<Vec<u8>> {
        read_raw(&mut self.image(), u64::from(first), u64::from(count)).map_err(at(&self.path))
    }

    /// Up to `length` bytes of the Form 1 data that runs on from sector `first`, starting `skip`
    /// bytes into it, as the console reads the disc by sector number. Fewer when the image ends first.
    pub fn data(&self, first: u32, skip: u64, length: u64) -> Result<Vec<u8>> {
        read_data(&mut self.image(), u64::from(first), skip, length).map_err(at(&self.path))
    }

    /// The files of a directory and of the directories inside it, added to `found`: each one's
    /// path, where it is, and where its directory record is.
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
        let data = self.data(sector, 0, u64::from(size))?;
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
            // After the name comes what CD-ROM XA adds (who owns the file, whether it holds Form 2
            // sectors), which is not needed to find it.
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

    /// The sector a file starts in.
    pub fn first_sector(&self, name: &str) -> Result<u32> {
        Ok(self.entry(name)?.0)
    }

    /// `length` bytes of a file from `start`.
    pub fn read(&self, name: &str, start: u64, length: u64) -> Result<Vec<u8>> {
        let (sector, size) = self.entry(name)?;
        self.span(name, sector, u64::from(size), start, start.saturating_add(length))
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
        self.data(sector, start, end - start)
    }

    /// The name of the program the console starts, from `SYSTEM.CNF`.
    pub fn boot_name(&self) -> Result<String> {
        match boot_name_in(&ascii(&self.read_from("SYSTEM.CNF", 0)?)) {
            Some(name) => Ok(name),
            None => image_error(format!("{}: SYSTEM.CNF does not name a program to start", self.path.display())),
        }
    }

    /// The game's serial number as it is printed on the case: `SLPS-00944` for `SLPS_009.44`.
    pub fn serial(&self) -> Result<String> {
        Ok(serial_of(&self.boot_name()?))
    }

    /// The version `SYSTEM.CNF` gives on a line `VER = 1.01`, or `None`; few PlayStation discs have one.
    pub fn version(&self) -> Result<Option<String>> {
        Ok(version_in(&ascii(&self.read_from("SYSTEM.CNF", 0)?)))
    }

    /// SHA-1 of the whole `.bin`, read in pieces, in lowercase hexadecimal.
    pub fn sha1(&self) -> Result<String> {
        let mut image = self.image();
        let mut digest = Sha1::new();
        let mut chunk = vec![0; BATCH * RAW_SECTOR];
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

/// The program a PlayStation `SYSTEM.CNF` names, from a line such as `BOOT = cdrom:\SLPS_009.44;1`:
/// the name runs up to the semicolon. The backslash is left out on some discs. A PlayStation 2
/// disc's line is `BOOT2`, which is not this one.
fn boot_name_in(config: &str) -> Option<String> {
    config.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if !key.trim_matches(is_blank).eq_ignore_ascii_case("BOOT") {
            return None;
        }
        let value = value.trim_start_matches(is_blank);
        let rest = value.get(..6).filter(|start| start.eq_ignore_ascii_case("cdrom:")).map(|_| &value[6..])?;
        let rest = rest.trim_start_matches('\\');
        let name = rest.split(|char| char == ';' || is_blank(char)).next().filter(|name| !name.is_empty())?;
        Some(name.to_string())
    })
}

/// What a disc image says it is, read without opening it as a `Disc`: the volume descriptor, the
/// root folder and `SYSTEM.CNF`, three small reads, where opening it lists every folder on the disc.
/// `None` when it is no PlayStation image or names no PlayStation program.
fn peek_label(path: &Path) -> Option<Label> {
    let bin = if is_cue(path) { data_track(path).ok()? } else { path.to_path_buf() };
    let mut image = File::open(bin).ok()?;
    let first = read_raw(&mut image, VOLUME_SECTOR, 1).ok()?;
    if !is_mode2(&first) || !first[DATA..].starts_with(VOLUME_MAGIC) {
        return None;
    }
    let volume = &first[DATA..FORM1_EDC];
    let (first, size) = extent(&volume[ROOT_RECORD..]);
    // A root folder is a few sectors; a larger one is not believed.
    let root = read_data(&mut image, u64::from(first), 0, u64::from(size).min(16 * SECTOR)).ok()?;
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
    let config = ascii(&read_data(&mut image, u64::from(sector), 0, u64::from(size).min(SECTOR)).ok()?);
    let name = serial_of(&boot_name_in(&config)?);
    Some(Label { name, version: version_in(&config).unwrap_or_default() })
}

/// Where a copy's `.bin` and `.cue` go when `output` is asked for: `output` is either, and the other
/// is beside it under the same name.
fn outputs(output: &Path) -> (PathBuf, PathBuf) {
    let bin = if is_cue(output) { output.with_extension("bin") } else { output.to_path_buf() };
    let cue = bin.with_extension("cue");
    (bin, cue)
}

/// The cue sheet of a copy whose `.bin` is `bin`. Made from the original's sheet where there is one,
/// with its data track's file renamed and every line else kept; a file of another track (CD audio)
/// is named by its full path when the copy is in another folder, so that it is still found.
/// Without one, a sheet of the one data track.
fn cue_for(disc: &Disc, bin: &Path) -> Result<String> {
    let bin_name = bin.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(cue) = &disc.cue else {
        return Ok(format!("FILE \"{bin_name}\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n"));
    };
    let text = String::from_utf8_lossy(&fs::read(cue).map_err(at(cue))?).into_owned();
    let from = cue.parent().unwrap_or(Path::new(""));
    let to = bin.parent().unwrap_or(Path::new(""));
    let same_folder = match (fs::canonicalize(from), fs::canonicalize(to)) {
        (Ok(from), Ok(to)) => from == to,
        _ => from == to,
    };
    let data = fs::canonicalize(&disc.path).ok();
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let ending = &line[body.len()..];
        let indent = &body[..body.len() - body.trim_start().len()];
        let words = body.trim_start();
        let named = words.get(..4).filter(|word| word.eq_ignore_ascii_case("FILE")).and_then(|_| cue_name(&words[4..]));
        let Some((name, kind)) = named else {
            out.push_str(line);
            continue;
        };
        let source = from.join(name);
        let new_name = if fs::canonicalize(&source).ok() == data && data.is_some() {
            bin_name.clone()
        } else if same_folder {
            name.to_string()
        } else {
            std::path::absolute(&source).unwrap_or(source).display().to_string()
        };
        out.push_str(&format!("{indent}FILE \"{new_name}\"{kind}{ending}"));
    }
    Ok(out)
}

/// Write a copy of the image to `output` with some of its files changed: a `.bin`, and a `.cue`
/// beside it under the same name. `output` may name either.
///
/// `patches` are (file name, offset in the file, bytes) to write over what is there, in order, so
/// that a later one wins where two overlap. Each sector they fall in is given a new EDC and ECC.
/// A file keeps its size: a patch that runs past a file's end is refused, since a file cannot be
/// made larger on a PlayStation image yet. A patch that falls in a Form 2 sector is refused too:
/// the file system's view of a file is its Form 1 data, which such a sector does not have.
///
/// What can be refused is refused before anything is written. If writing fails part-way, what was
/// written is removed.
pub fn write_image(disc: &Disc, output: &Path, patches: &[(&str, u64, Vec<u8>)]) -> Result<()> {
    // The patches, sector by sector: each changed sector's place in the data, and what goes there.
    let mut edits: BTreeMap<u32, Vec<(usize, &[u8])>> = BTreeMap::new();
    for (name, offset, data) in patches {
        let (first, size) = disc.entry(name)?;
        let end = offset.saturating_add(data.len() as u64);
        if end > u64::from(size) {
            return image_error(format!(
                "{name} has no bytes {offset:#x} to {end:#x}: it is {size:#x} long, and a file cannot be made \
                 larger on a PlayStation disc image"
            ));
        }
        let mut done = 0;
        while done < data.len() {
            let at = offset + done as u64;
            let sector = first + (at / SECTOR) as u32;
            let within = (at % SECTOR) as usize;
            let taken = (SECTOR as usize - within).min(data.len() - done);
            edits.entry(sector).or_default().push((within, &data[done..done + taken]));
            done += taken;
        }
    }
    // Each changed sector, read, changed and sealed now, so that one that cannot be is refused first.
    let mut changed: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for (sector, pieces) in &edits {
        let mut whole = disc.raw(*sector, 1)?;
        if !is_mode2(&whole) {
            return image_error(format!("sector {sector} of {} is not a Mode 2 sector", disc.path.display()));
        }
        if is_form2(&whole) {
            return image_error(format!(
                "sector {sector} of {} is a Form 2 sector (a movie's or a sound's), which has no file data to change",
                disc.path.display()
            ));
        }
        for (within, bytes) in pieces {
            whole[DATA + within..][..bytes.len()].copy_from_slice(bytes);
        }
        seal(&mut whole);
        changed.insert(*sector, whole);
    }
    let (bin, cue) = outputs(output);
    // The image is still open for reading, and opening it for writing would empty it.
    for (source, target) in [(Some(&disc.path), &bin), (disc.cue.as_ref(), &cue)] {
        if let (Some(Ok(source)), Ok(target)) = (source.map(fs::canonicalize), fs::canonicalize(target))
            && source == target
        {
            return image_error(format!(
                "{} is the image being read: a copy cannot be written over it",
                target.display()
            ));
        }
    }
    let sheet = cue_for(disc, &bin)?;

    // Both files are written under names of their own beside the outputs and only put in place once
    // both are whole, so that a build that fails leaves whatever was there before. The names carry the
    // process's number and are made new, never opened if something has them already; a build removes
    // only the ones it made.
    let partial = |path: &Path| {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}.part", std::process::id()));
        path.with_file_name(name)
    };
    let (bin_part, cue_part) = (partial(&bin), partial(&cue));
    let make = |path: &Path| OpenOptions::new().write(true).create_new(true).open(path).map_err(at(path));
    let (mut bin_made, mut cue_made) = (false, false);
    let written = (|| -> Result<()> {
        let mut source = File::open(&disc.path).map_err(at(&disc.path))?;
        let mut out = make(&bin_part)?;
        bin_made = true;
        let mut buffer = vec![0; BATCH * RAW_SECTOR];
        let mut sector = 0u64;
        loop {
            let count = read_full(&mut source, &mut buffer).map_err(at(&disc.path))?;
            if count == 0 {
                break;
            }
            let end = sector + (count / RAW_SECTOR) as u64;
            let wanted = u32::try_from(sector).unwrap_or(u32::MAX)..u32::try_from(end).unwrap_or(u32::MAX);
            for (number, whole) in changed.range(wanted) {
                let place = (u64::from(*number) - sector) as usize * RAW_SECTOR;
                buffer[place..place + RAW_SECTOR].copy_from_slice(whole);
            }
            out.write_all(&buffer[..count]).map_err(at(&bin_part))?;
            sector = end;
        }
        out.sync_all().map_err(at(&bin_part))?;
        drop(out);
        let mut sheet_out = make(&cue_part)?;
        cue_made = true;
        sheet_out.write_all(sheet.as_bytes()).map_err(at(&cue_part))?;
        drop(sheet_out);
        fs::rename(&bin_part, &bin).map_err(at(&bin))?;
        bin_made = false;
        fs::rename(&cue_part, &cue).map_err(at(&cue))?;
        cue_made = false;
        Ok(())
    })();
    if written.is_err() {
        if bin_made {
            let _ = fs::remove_file(&bin_part);
        }
        if cue_made {
            let _ = fs::remove_file(&cue_part);
        }
    }
    written
}

/// A PlayStation disc image is too large to hold whole: adapters are given a `Disc`, which stays open.
pub struct Ps1;

pub static PLATFORM: Ps1 = Ps1;

impl Platform for Ps1 {
    fn name(&self) -> &'static str {
        "PlayStation"
    }

    /// A `.cue` whose first track is a Mode 2 data track, or a `.bin` that is one. A cue sheet whose
    /// `.bin` is missing is still taken, so that opening it says which file is missing.
    fn recognises(&self, path: &Path) -> Result<bool> {
        Ok(if is_cue(path) { data_track(path).is_ok() } else { is_raw_disc(path) })
    }

    fn glance(&self, path: &Path) -> Result<Option<Glance>> {
        // The .bin of the data track, under one of the endings it is kept with; a .cue is passed over,
        // so that a game is not found twice. Its size is a number of whole sectors.
        let ending = path.extension().map(|ending| ending.to_string_lossy().to_ascii_lowercase());
        if !matches!(ending.as_deref(), Some("bin" | "img")) {
            return Ok(None);
        }
        let size = fs::metadata(path).map_err(at(path))?.len();
        if !size.is_multiple_of(RAW_SECTOR as u64) {
            return Ok(None);
        }
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Some(Glance { size, name }))
    }

    fn open(&self, path: &Path) -> Result<(Image, String)> {
        let disc = Disc::open(path)?;
        let sha1 = disc.sha1()?;
        Ok((Image::Ps1Disc(disc), sha1))
    }

    /// The serial `SYSTEM.CNF` gives, and a version where it gives one, which few discs do. A disc
    /// without one names no program the console starts, as a PlayStation 2 game on a CD does, whose
    /// line is `BOOT2`.
    fn label(&self, image: &Image) -> Option<Label> {
        let disc = image.ps1_disc().ok()?;
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

pub mod tim;

#[cfg(test)]
mod tests;
