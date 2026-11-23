//! Zip archives, as far as a collection of games needs them.
//!
//! Collections of cartridge games are mostly kept one game to a zip file. Two things are asked of
//! such a file. The first is what it holds, with each file's size: that is the list at the end of
//! the archive, read without unpacking anything, so that a folder of thousands can be gone through
//! in a moment. The second is the bytes of one file in it.
//!
//! Only what such archives use is read: files stored as they are or deflated, in an archive
//! small enough not to need the 64-bit extension. Anything else is refused in words.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use flate2::Crc;
use flate2::read::DeflateDecoder;

use crate::error::{Error, Result, at};

/// One file in an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its path inside the archive, with `/` between folders.
    pub name: String,
    /// Its size once unpacked, in bytes.
    pub size: u64,
    packed_size: u64,
    crc32: u32,
    /// 0 when it is stored as it is, 8 when it is deflated.
    method: u16,
    encrypted: bool,
    /// Where in the archive the file's own header is.
    header: u64,
}

const END_OF_LIST: [u8; 4] = [b'P', b'K', 5, 6];
const LISTED_FILE: [u8; 4] = [b'P', b'K', 1, 2];
const FILE_HEADER: [u8; 4] = [b'P', b'K', 3, 4];
/// The record that ends an archive is this long, before a comment of up to 65,535 bytes.
const END_LENGTH: u64 = 22;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// Whether a file is a zip archive by its name. Its contents are only looked at when it is read.
pub fn is_zip(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

fn refused<T>(path: &Path, why: &str) -> Result<T> {
    Err(Error::Image(format!("{}: {why}", path.display())))
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// The files in an archive, in the order its list gives them. Folders are left out.
pub fn entries(path: &Path) -> Result<Vec<Entry>> {
    let mut file = File::open(path).map_err(at(path))?;
    let length = file.metadata().map_err(at(path))?.len();
    if length < END_LENGTH {
        return refused(path, "it is not a zip archive");
    }
    // The record that ends the archive is the last thing in it, but for a comment: look for it
    // from the end backwards.
    let tail_length = length.min(END_LENGTH + 0xFFFF);
    let mut tail = vec![0; tail_length as usize];
    file.seek(SeekFrom::Start(length - tail_length)).map_err(at(path))?;
    file.read_exact(&mut tail).map_err(at(path))?;
    let Some(end) = (0..=tail.len() - END_LENGTH as usize).rev().find(|&at| tail[at..at + 4] == END_OF_LIST) else {
        return refused(path, "it is not a zip archive");
    };
    let end = &tail[end..];
    let (count, list_size, list_start) = (u16_at(end, 10), u32_at(end, 12), u32_at(end, 16));
    if count == 0xFFFF || list_size == 0xFFFF_FFFF || list_start == 0xFFFF_FFFF {
        return refused(path, "it is a zip archive of the 64-bit kind, which is not read");
    }
    if u64::from(list_start) + u64::from(list_size) > length {
        return refused(path, "its list of files is cut short");
    }
    let mut list = vec![0; list_size as usize];
    file.seek(SeekFrom::Start(u64::from(list_start))).map_err(at(path))?;
    file.read_exact(&mut list).map_err(at(path))?;

    let mut found = Vec::new();
    let mut at_byte = 0;
    for _ in 0..count {
        if list.len() < at_byte + 46 || list[at_byte..at_byte + 4] != LISTED_FILE {
            return refused(path, "its list of files is damaged");
        }
        let record = &list[at_byte..];
        let (name_length, extra, comment) =
            (usize::from(u16_at(record, 28)), usize::from(u16_at(record, 30)), usize::from(u16_at(record, 32)));
        if record.len() < 46 + name_length {
            return refused(path, "its list of files is damaged");
        }
        let name = String::from_utf8_lossy(&record[46..46 + name_length]).replace('\\', "/");
        if !name.ends_with('/') {
            found.push(Entry {
                name,
                size: u64::from(u32_at(record, 24)),
                packed_size: u64::from(u32_at(record, 20)),
                crc32: u32_at(record, 16),
                method: u16_at(record, 10),
                encrypted: u16_at(record, 8) & 1 != 0,
                header: u64::from(u32_at(record, 42)),
            });
        }
        at_byte += 46 + name_length + extra + comment;
    }
    Ok(found)
}

/// The bytes of one file of an archive, unpacked and checked against the checksum the archive
/// keeps for it.
pub fn read(path: &Path, entry: &Entry) -> Result<Vec<u8>> {
    if entry.encrypted {
        return refused(path, &format!("{} is locked with a password", entry.name));
    }
    if entry.size == 0xFFFF_FFFF || entry.packed_size == 0xFFFF_FFFF {
        return refused(path, &format!("{} is too large to be read from an archive", entry.name));
    }
    let mut file = File::open(path).map_err(at(path))?;
    let mut header = [0; 30];
    file.seek(SeekFrom::Start(entry.header)).map_err(at(path))?;
    file.read_exact(&mut header).map_err(at(path))?;
    if header[..4] != FILE_HEADER {
        return refused(path, &format!("{} is not where the archive says it is", entry.name));
    }
    // The file's own header repeats its name and may add more: its bytes come after both.
    let skipped = u64::from(u16_at(&header, 26)) + u64::from(u16_at(&header, 28));
    file.seek(SeekFrom::Current(skipped as i64)).map_err(at(path))?;
    let mut packed = file.take(entry.packed_size);
    let mut data = Vec::with_capacity(entry.size as usize);
    let unpacked = match entry.method {
        STORED => packed.read_to_end(&mut data),
        // One byte more than is expected is asked for, so that a file longer than it says is noticed.
        DEFLATED => DeflateDecoder::new(packed).take(entry.size + 1).read_to_end(&mut data),
        _ => return refused(path, &format!("{} is packed in a way that is not read", entry.name)),
    };
    if unpacked.is_err() || data.len() as u64 != entry.size {
        return refused(path, &format!("{} is damaged", entry.name));
    }
    let mut crc = Crc::new();
    crc.update(&data);
    if crc.sum() != entry.crc32 {
        return refused(path, &format!("{} is damaged: its checksum is not the one the archive keeps", entry.name));
    }
    Ok(data)
}

/// A zip archive of the given files, each deflated or stored as it is: what a test needs to make
/// an archive, and nothing a user does.
#[doc(hidden)]
pub fn make(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    use std::io::Write;

    let mut archive = Vec::new();
    let mut list = Vec::new();
    for (name, data, deflate) in files {
        let mut crc = Crc::new();
        crc.update(data);
        let packed = if *deflate {
            let mut packer = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            packer.write_all(data).unwrap();
            packer.finish().unwrap()
        } else {
            data.to_vec()
        };
        let method = if *deflate { DEFLATED } else { STORED };
        // What a file's own header and its line in the list have in common.
        let mut shared = [20u16.to_le_bytes(), 0u16.to_le_bytes(), method.to_le_bytes(), [0; 2], [0; 2]].concat();
        shared.extend(crc.sum().to_le_bytes());
        shared.extend((packed.len() as u32).to_le_bytes());
        shared.extend((data.len() as u32).to_le_bytes());
        shared.extend((name.len() as u16).to_le_bytes());
        shared.extend(0u16.to_le_bytes());
        let header = archive.len() as u32;
        archive.extend(FILE_HEADER);
        archive.extend(&shared);
        archive.extend(name.as_bytes());
        archive.extend(&packed);
        list.extend(LISTED_FILE);
        list.extend(20u16.to_le_bytes());
        list.extend(&shared);
        list.extend([0; 10]);
        list.extend(header.to_le_bytes());
        list.extend(name.as_bytes());
    }
    let (count, list_start) = (files.len() as u16, archive.len() as u32);
    archive.extend(&list);
    archive.extend(END_OF_LIST);
    archive.extend([0; 4]);
    archive.extend(count.to_le_bytes());
    archive.extend(count.to_le_bytes());
    archive.extend((list.len() as u32).to_le_bytes());
    archive.extend(list_start.to_le_bytes());
    archive.extend(0u16.to_le_bytes());
    archive
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn message<T>(result: Result<T>) -> String {
        result.err().map(|error| error.to_string()).unwrap_or_default()
    }

    #[test]
    fn an_archive_is_listed_without_unpacking_and_its_files_are_read() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("games.ZIP");
        let game: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(
            &path,
            make(&[("Game (Japan).sfc", &game, true), ("notes/", b"", false), ("notes/read me.txt", b"hello", false)]),
        )
        .unwrap();
        assert!(is_zip(&path) && !is_zip(&folder.path().join("game.sfc")));

        let listed = entries(&path).unwrap();
        let names: Vec<(&str, u64)> = listed.iter().map(|entry| (entry.name.as_str(), entry.size)).collect();
        assert_eq!(names, [("Game (Japan).sfc", 70_000), ("notes/read me.txt", 5)]);
        assert!(listed[0].packed_size < 70_000);
        assert!(read(&path, &listed[0]).unwrap() == game);
        assert_eq!(read(&path, &listed[1]).unwrap(), b"hello");

        // A comment after the list, as some tools add, does not hide it.
        let mut commented = fs::read(&path).unwrap();
        let last = commented.len();
        commented[last - 2..].copy_from_slice(&9u16.to_le_bytes());
        commented.extend(b"a comment");
        fs::write(&path, &commented).unwrap();
        assert_eq!(entries(&path).unwrap(), listed);
    }

    #[test]
    fn what_is_not_a_sound_archive_is_refused_in_words() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("games.zip");
        let said = |why: &str| format!("{}: {why}", path.display());

        fs::write(&path, b"not an archive at all, only some text").unwrap();
        assert_eq!(message(entries(&path)), said("it is not a zip archive"));
        fs::write(&path, b"short").unwrap();
        assert_eq!(message(entries(&path)), said("it is not a zip archive"));

        let whole = make(&[("game.sfc", &[7; 4000], true)]);
        // A byte of the packed game changed: it no longer unpacks to what the archive says.
        let mut damaged = whole.clone();
        damaged[40] ^= 0xFF;
        fs::write(&path, &damaged).unwrap();
        let listed = entries(&path).unwrap();
        assert!(message(read(&path, &listed[0])).starts_with(&said("game.sfc is damaged")));

        // The list says the file is somewhere it is not.
        let mut misplaced = whole.clone();
        let list = whole.len() - 22 - 46 - "game.sfc".len();
        misplaced[list + 42..list + 46].copy_from_slice(&5u32.to_le_bytes());
        fs::write(&path, &misplaced).unwrap();
        let listed = entries(&path).unwrap();
        assert_eq!(message(read(&path, &listed[0])), said("game.sfc is not where the archive says it is"));

        // Locked with a password, or packed some other way.
        let mut locked = entries_of(&whole, &path);
        locked.encrypted = true;
        assert_eq!(message(read(&path, &locked)), said("game.sfc is locked with a password"));
        let mut other = entries_of(&whole, &path);
        other.method = 14;
        assert_eq!(message(read(&path, &other)), said("game.sfc is packed in a way that is not read"));

        // The 64-bit kind marks its counts as elsewhere.
        let mut large = whole.clone();
        let end = large.len() - 22;
        large[end + 10..end + 12].copy_from_slice(&0xFFFFu16.to_le_bytes());
        fs::write(&path, &large).unwrap();
        assert_eq!(message(entries(&path)), said("it is a zip archive of the 64-bit kind, which is not read"));
    }

    /// The one entry of an archive written to `path`.
    fn entries_of(archive: &[u8], path: &Path) -> Entry {
        fs::write(path, archive).unwrap();
        entries(path).unwrap().remove(0)
    }
}
