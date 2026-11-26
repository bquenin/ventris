//! SNES / Super Famicom cartridge layout: copier headers, internal header, address mapping.

use std::fmt;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha1::{Digest, Sha1};

use crate::adapter::Catalogue;
use crate::archive;
use crate::error::{Error, Result, at};
use crate::platform::{Glance, Image, Label, Platform};

/// The catalogue players know a cartridge dump by.
pub const NO_INTRO: Catalogue = Catalogue { name: "No-Intro", about: "the catalogue of cartridge dumps" };

pub const COPIER_HEADER_SIZE: usize = 512;
/// 8 MiB: the most a cartridge mapping reaches.
pub const LARGEST_ROM: usize = 0x800000;
/// The score from which `detect_header` is usually taken at its word.
pub const LIKELY_HEADER: i32 = 5;

/// How a cartridge's ROM appears in the console's address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapping {
    LoRom,
    HiRom,
    ExHiRom,
}

impl Mapping {
    /// In the order their headers are looked for.
    pub const ALL: [Mapping; 3] = [Mapping::LoRom, Mapping::HiRom, Mapping::ExHiRom];

    /// As it is usually written: "LoROM", "HiROM", "ExHiROM".
    pub fn name(self) -> &'static str {
        match self {
            Mapping::LoRom => "LoROM",
            Mapping::HiRom => "HiROM",
            Mapping::ExHiRom => "ExHiROM",
        }
    }

    /// File offset of the internal header.
    pub fn header_offset(self) -> usize {
        match self {
            Mapping::LoRom => 0x7FC0,
            Mapping::HiRom => 0xFFC0,
            Mapping::ExHiRom => 0x40FFC0,
        }
    }

    /// Whether the low nibble of a map-mode byte is one expected for this mapping.
    fn has_map_mode(self, nibble: u8) -> bool {
        match self {
            Mapping::LoRom => matches!(nibble, 0x0 | 0x2 | 0x3),
            Mapping::HiRom => nibble == 0x1,
            Mapping::ExHiRom => nibble == 0x5,
        }
    }
}

impl fmt::Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

fn region_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0x00 => "Japan",
        0x01 => "North America",
        0x02 => "Europe",
        0x03 => "Sweden/Scandinavia",
        0x04 => "Finland",
        0x05 => "Denmark",
        0x06 => "France",
        0x07 => "Netherlands",
        0x08 => "Spain",
        0x09 => "Germany",
        0x0A => "Italy",
        0x0B => "China",
        0x0D => "Korea",
        0x0F => "Canada",
        0x10 => "Brazil",
        0x11 => "Australia",
        _ => return None,
    })
}

/// An address that has no equivalent in the other address space, or an image that cannot be
/// treated as asked.
fn image_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

/// What a cartridge says about itself, in the 64 bytes of its internal header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnesHeader {
    pub mapping: Mapping,
    pub offset: usize,
    pub title: String,
    pub map_mode: u8,
    pub cartridge_type: u8,
    pub rom_size_kb: u32,
    pub ram_size_kb: u32,
    pub region_code: u8,
    pub developer_id: u8,
    pub version: u8,
    pub checksum: u16,
    pub checksum_complement: u16,
    pub reset_vector: u16,
    /// How much the bytes at `offset` look like a header: the higher, the surer.
    pub score: i32,
}

impl SnesHeader {
    pub fn region(&self) -> String {
        region_name(self.region_code)
            .map_or_else(|| format!("unknown (0x{:02x})", self.region_code), |name| name.to_string())
    }

    pub fn fast_rom(&self) -> bool {
        self.map_mode & 0x10 != 0
    }
}

/// Copier headers add 512 bytes to an image whose size is otherwise a multiple of 1 KiB.
pub fn has_copier_header(size: usize) -> bool {
    size % 1024 == COPIER_HEADER_SIZE
}

pub fn strip_copier_header(data: &[u8]) -> &[u8] {
    if has_copier_header(data.len()) { &data[COPIER_HEADER_SIZE..] } else { data }
}

/// A title as text. Titles are in the single-byte part of Shift-JIS: ASCII and half-width katakana.
/// A byte that starts a two-byte character is shown as U+FFFD, since the table that names those
/// characters is not here.
fn title_text(bytes: &[u8]) -> String {
    let text: String = bytes
        .iter()
        .map(|byte| match byte {
            0x00..=0x7F => *byte as char,
            0xA1..=0xDF => char::from_u32(0xFF61 + u32::from(byte - 0xA1)).unwrap_or(char::REPLACEMENT_CHARACTER),
            _ => char::REPLACEMENT_CHARACTER,
        })
        .collect();
    // The separators $1C-$1F count as spaces here, as they do for the Python reference.
    text.trim_end_matches(|char: char| char.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&char)).to_string()
}

fn parse_header(rom: &[u8], mapping: Mapping) -> Option<SnesHeader> {
    let offset = mapping.header_offset();
    header_from(rom.get(offset..offset + 0x40)?, mapping)
}

/// The 64 bytes of a header for `mapping`, read.
fn header_from(raw: &[u8], mapping: Mapping) -> Option<SnesHeader> {
    let offset = mapping.header_offset();
    let raw = raw.get(..0x40)?;
    let word = |at: usize| u16::from_le_bytes([raw[at], raw[at + 1]]);
    let map_mode = raw[0x15];
    let checksum_complement = word(0x1C);
    let checksum = word(0x1E);
    let reset_vector = word(0x3C);
    let title_bytes = &raw[..0x15];

    let mut score = 0;
    if u32::from(checksum) + u32::from(checksum_complement) == 0xFFFF && checksum != 0 {
        score += 4;
    }
    if mapping.has_map_mode(map_mode & 0x0F) {
        score += 2;
    }
    if map_mode & 0xE0 == 0x20 {
        score += 1;
    }
    // The reset vector must point into ROM, which is only mapped at $8000 and above in bank 0.
    score += if reset_vector >= 0x8000 { 2 } else { -4 };
    if title_bytes.iter().all(|byte| matches!(byte, 0x20..=0x7E | 0xA1..=0xDF)) {
        score += 1;
    }
    if (0x07..=0x0D).contains(&raw[0x17]) {
        score += 1;
    }

    Some(SnesHeader {
        mapping,
        offset,
        title: title_text(title_bytes),
        map_mode,
        cartridge_type: raw[0x16],
        rom_size_kb: if raw[0x17] < 0x10 { 1 << raw[0x17] } else { 0 },
        ram_size_kb: if 0 < raw[0x18] && raw[0x18] < 0x10 { 1 << raw[0x18] } else { 0 },
        region_code: raw[0x19],
        developer_id: raw[0x1A],
        version: raw[0x1B],
        checksum,
        checksum_complement,
        reset_vector,
        score,
    })
}

/// Find the internal header of a ROM image that has no copier header.
///
/// Returns None when no candidate location scores `min_score` or more; `LIKELY_HEADER` is the
/// score to ask for when the file may not be a SNES game at all, and 0 takes the best there is.
pub fn detect_header(rom: &[u8], min_score: i32) -> Option<SnesHeader> {
    best_header(Mapping::ALL.iter().filter_map(|mapping| parse_header(rom, *mapping)), min_score)
}

/// The header that scores best, if it scores `min_score` or more. Of two that score the same, the
/// first one looked for is kept.
fn best_header(headers: impl Iterator<Item = SnesHeader>, min_score: i32) -> Option<SnesHeader> {
    let mut best: Option<SnesHeader> = None;
    for header in headers {
        if best.as_ref().is_none_or(|best| header.score > best.score) {
            best = Some(header);
        }
    }
    best.filter(|best| best.score >= min_score)
}

/// The internal header of the cartridge image at `path`, read where each mapping keeps it, past a
/// copier's header when the file has one: three reads of 64 bytes, and not the whole file.
fn header_in_file(path: &Path) -> Option<SnesHeader> {
    let mut file = fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len() as usize;
    let skip = if has_copier_header(size) { COPIER_HEADER_SIZE } else { 0 };
    let headers = Mapping::ALL.iter().filter_map(|mapping| {
        let mut raw = [0; 0x40];
        file.seek(SeekFrom::Start((skip + mapping.header_offset()) as u64)).ok()?;
        file.read_exact(&mut raw).ok()?;
        header_from(&raw, *mapping)
    });
    best_header(headers.collect::<Vec<_>>().into_iter(), LIKELY_HEADER)
}

/// A header's title and version byte as a label: the version byte is shown as the console's tools
/// show it, 0 as "1.0" and 1 as "1.1".
fn label_of(header: SnesHeader) -> Label {
    Label { name: header.title, version: format!("1.{}", header.version) }
}

/// Cartridge checksum: the byte sum with the last part mirrored up to a power of two.
pub fn compute_checksum(rom: &[u8]) -> u16 {
    fn mirror_sum(rom: &[u8], start: usize, length: usize, mut mask: usize) -> u64 {
        while mask != 0 && length & mask == 0 {
            mask >>= 1;
        }
        if mask == 0 {
            return 0;
        }
        let part1: u64 = rom[start..start + mask].iter().map(|byte| u64::from(*byte)).sum();
        let mut part2 = 0;
        let mut rest = length - mask;
        if rest != 0 {
            part2 = mirror_sum(rom, start + mask, rest, mask >> 1);
            while rest < mask {
                rest += rest;
                part2 += part2;
            }
        }
        part1 + part2
    }

    (mirror_sum(rom, 0, rom.len(), 0x800000) & 0xFFFF) as u16
}

/// Return `rom` with its internal checksum and complement recomputed.
pub fn update_checksum(rom: &[u8]) -> Result<Vec<u8>> {
    let Some(header) = detect_header(rom, 0) else {
        return image_error("no SNES header found");
    };
    let mut out = rom.to_vec();
    let main = header.offset + 0x1C;
    let mut places = vec![main];
    // An ExHiROM image made from a HiROM one keeps a second copy of the header where it used to be,
    // checksum included. A game made as ExHiROM may have its title there and no checksum; that is left
    // as it is.
    let low = LOW_HEADER + 0x1C;
    if header.mapping == Mapping::ExHiRom && out[LOW_HEADER..low + 4] == out[header.offset..main + 4] {
        places.push(low);
    }
    // The checksum covers its own four bytes, which always add up to 0x1FE once they are consistent.
    for fields in &places {
        out[*fields..fields + 4].copy_from_slice(&[0xFF, 0xFF, 0x00, 0x00]);
    }
    let checksum = compute_checksum(&out);
    for fields in &places {
        out[*fields..fields + 2].copy_from_slice(&(checksum ^ 0xFFFF).to_le_bytes());
        out[fields + 2..fields + 4].copy_from_slice(&checksum.to_le_bytes());
    }
    Ok(out)
}

/// Pad `rom` with $FF to `size` bytes and record the new size in its header.
pub fn expand(rom: &[u8], size: usize) -> Result<Vec<u8>> {
    if size < rom.len() {
        return image_error("a ROM cannot be expanded to a smaller size");
    }
    let Some(header) = detect_header(rom, 0) else {
        return image_error("no SNES header found");
    };
    let mut out = rom.to_vec();
    out.resize(size, 0xFF);
    let bits = usize::BITS - (size - 1).leading_zeros(); // log2 of the size, rounded up
    out[header.offset + 0x17] = bits.saturating_sub(10) as u8; // and of the size in KiB
    Ok(out)
}

pub const HIROM_SIZE: usize = 0x400000;
pub const EXHIROM_SIZE: usize = 0x800000;
const LOW_HEADER: usize = 0xFFC0;

/// Turn a 4 MiB HiROM image into an 8 MiB ExHiROM one, which adds banks $40-$7D of free space.
///
/// The first 4 MiB stay where the game expects them, in banks $80-$FF. The catch is banks $00-$3F:
/// in ExHiROM their upper halves show the new 4 MiB, and the console starts in bank $00. So the top
/// half of the first bank, which holds the header, the vectors and the start-up code, is copied to
/// where bank $00 now looks (file 0x408000). Code that runs in banks $01-$3F would need the same;
/// games made for fast ROM run in $80 and up, and the copy of bank $00 is enough for them.
pub fn to_exhirom(rom: &[u8]) -> Result<Vec<u8>> {
    let hirom = detect_header(rom, 0).is_some_and(|header| header.mapping == Mapping::HiRom);
    if !hirom || rom.len() != HIROM_SIZE {
        return image_error("only a 4 MiB HiROM image can be turned into ExHiROM");
    }
    let mut out = rom.to_vec();
    out.resize(EXHIROM_SIZE, 0xFF);
    out[HIROM_SIZE + 0x8000..HIROM_SIZE + 0x10000].copy_from_slice(&rom[0x8000..0x10000]);
    for offset in [LOW_HEADER, Mapping::ExHiRom.header_offset()] {
        out[offset + 0x15] = (out[offset + 0x15] & 0xF0) | 0x05;
        out[offset + 0x17] = 0x0D; // 8 MiB
    }
    Ok(out)
}

/// Write assembled code into `rom`; each segment is a CPU address and the bytes that go there.
pub fn write_code(rom: &[u8], segments: &[(u32, Vec<u8>)], mapping: Mapping) -> Result<Vec<u8>> {
    let mut out = rom.to_vec();
    for (address, code) in segments {
        let offset = snes_to_pc(*address, mapping)?;
        if offset + code.len() > out.len() {
            return image_error(format!("${address:06x} is past the end of the ROM"));
        }
        out[offset..offset + code.len()].copy_from_slice(code);
    }
    Ok(out)
}

/// Hands out free space in a file range, never letting a piece cross a 64 KiB bank boundary.
///
/// Games index data with 16-bit registers inside one bank, so data that straddles two banks is read
/// wrongly.
#[derive(Clone, Debug)]
pub struct BankAllocator {
    pub position: usize,
    pub end: usize,
}

impl BankAllocator {
    pub fn new(start: usize, end: usize) -> Self {
        BankAllocator { position: start, end }
    }

    /// Return the file offset of `size` free bytes.
    pub fn allocate(&mut self, size: usize) -> Result<usize> {
        if size > 0x10000 {
            return image_error(format!("{size} bytes cannot fit inside one bank"));
        }
        let mut position = self.position;
        if (position & 0xFFFF) + size > 0x10000 {
            position = (position | 0xFFFF) + 1;
        }
        if position + size > self.end {
            return image_error(format!("no room left for {size} bytes"));
        }
        self.position = position + size;
        Ok(position)
    }
}

/// Convert a file offset (no copier header) to a CPU address.
pub fn pc_to_snes(offset: usize, mapping: Mapping) -> Result<u32> {
    let address = match mapping {
        Mapping::LoRom | Mapping::HiRom if offset >= 0x400000 => {
            return image_error(format!("offset 0x{offset:06x} is outside {mapping} space"));
        }
        Mapping::LoRom => ((offset >> 15) | 0x80) << 16 | (offset & 0x7FFF) | 0x8000,
        Mapping::HiRom => 0xC00000 | offset,
        Mapping::ExHiRom if offset < 0x400000 => 0xC00000 | offset,
        Mapping::ExHiRom if offset < 0x7E0000 => offset,
        // Banks $7E-$7F are work RAM, so the last 128 KiB is only visible in the upper half of $3E-$3F.
        Mapping::ExHiRom if offset < 0x800000 && offset & 0x8000 != 0 => offset - 0x400000,
        Mapping::ExHiRom => return image_error(format!("offset 0x{offset:06x} is not mapped in ExHiROM")),
    };
    Ok(address as u32)
}

/// Convert a CPU address to a file offset (no copier header).
pub fn snes_to_pc(address: u32, mapping: Mapping) -> Result<usize> {
    let bank = (address >> 16) & 0xFF;
    let low = address & 0xFFFF;
    if bank == 0x7E || bank == 0x7F {
        return image_error(format!("${address:06x} is work RAM"));
    }
    let offset = match mapping {
        Mapping::LoRom => {
            if low < 0x8000 {
                return image_error(format!("${address:06x} is not ROM in LoROM"));
            }
            ((bank & 0x7F) << 15) | (low & 0x7FFF)
        }
        Mapping::HiRom | Mapping::ExHiRom => {
            if bank & 0x7F < 0x40 && low < 0x8000 {
                return image_error(format!("${address:06x} is not ROM in {mapping}"));
            }
            let offset = address & 0x3FFFFF;
            if mapping == Mapping::ExHiRom && bank < 0x80 { offset + 0x400000 } else { offset }
        }
    };
    Ok(offset as usize)
}

/// Write a built image to `path` with its internal checksum brought up to date; returns what was written.
pub fn write_rom(path: &Path, rom: &[u8]) -> Result<Vec<u8>> {
    let built = update_checksum(rom)?;
    fs::write(path, &built).map_err(at(path))?;
    Ok(built)
}

/// A cartridge image is small enough to hold whole: adapters are given its bytes, copier header removed.
pub struct Snes;

pub static PLATFORM: Snes = Snes;

/// The endings a cartridge image's name has, whichever tool dumped it.
const CARTRIDGE_ENDINGS: [&str; 4] = ["sfc", "smc", "swc", "fig"];

fn is_cartridge_name(name: &str) -> bool {
    let ending = name.rsplit_once('.').map_or("", |(_, ending)| ending);
    CARTRIDGE_ENDINGS.iter().any(|known| ending.eq_ignore_ascii_case(known))
}

/// The cartridge image in a zip archive, when it holds exactly one: a collection keeps one game to
/// an archive, sometimes with a note beside it. With none, or several, the archive is not taken as
/// a game, since nothing says which was meant.
fn cartridge_in(path: &Path) -> Result<Option<archive::Entry>> {
    let mut images: Vec<archive::Entry> =
        archive::entries(path)?.into_iter().filter(|entry| is_cartridge_name(&entry.name)).collect();
    Ok(if images.len() == 1 { images.pop() } else { None })
}

/// The bytes of a cartridge image, from the file itself or from the archive it is kept in.
fn read_file(path: &Path) -> Result<Vec<u8>> {
    if !archive::is_zip(path) {
        return fs::read(path).map_err(at(path));
    }
    match cartridge_in(path)? {
        Some(entry) => archive::read(path, &entry),
        None => Err(Error::Image(format!("{} does not hold one cartridge image", path.display()))),
    }
}

impl Platform for Snes {
    fn name(&self) -> &'static str {
        "Super Famicom / SNES"
    }

    fn recognises(&self, path: &Path) -> Result<bool> {
        let largest = (LARGEST_ROM + COPIER_HEADER_SIZE) as u64;
        if archive::is_zip(path) {
            // An archive that is not a sound one, or holds no cartridge, is simply not this console's.
            let Ok(Some(entry)) = cartridge_in(path) else { return Ok(false) };
            if entry.size > largest {
                return Ok(false);
            }
        } else if fs::metadata(path).map_err(at(path))?.len() > largest {
            return Ok(false);
        }
        let data = read_file(path)?;
        Ok(detect_header(strip_copier_header(&data), LIKELY_HEADER).is_some())
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
        let header = if has_copier_header(size as usize) { COPIER_HEADER_SIZE as u64 } else { 0 };
        Ok((size <= (LARGEST_ROM + COPIER_HEADER_SIZE) as u64).then_some(Glance { size: size - header, name }))
    }

    fn open(&self, path: &Path) -> Result<(Image, String)> {
        let mut rom = read_file(path)?;
        if has_copier_header(rom.len()) {
            rom.drain(..COPIER_HEADER_SIZE);
        }
        let sha1 = Sha1::digest(&rom).iter().map(|byte| format!("{byte:02x}")).collect();
        Ok((Image::Cartridge(rom), sha1))
    }

    fn changes(&self, original: &Image, built: &Image) -> Vec<String> {
        let (Ok(original), Ok(built)) = (original.cartridge(), built.cartridge()) else {
            return Vec::new();
        };
        if built.len() == original.len() {
            return Vec::new();
        }
        vec![format!("size:      expanded from {} to {} MiB", original.len() / 0x100000, built.len() / 0x100000)]
    }

    /// The title and version of the cartridge's header. A copier's header is gone by now: `open`
    /// took it off.
    fn label(&self, image: &Image) -> Option<Label> {
        detect_header(image.cartridge().ok()?, LIKELY_HEADER).map(label_of)
    }

    /// The same, from a plain cartridge file. A cartridge in an archive is not looked into: its
    /// header is past most of the image, which would have to be unpacked to reach it.
    fn peek(&self, path: &Path) -> Option<Label> {
        if archive::is_zip(path) {
            return None;
        }
        header_in_file(path).map(label_of)
    }

    fn label_kind(&self) -> &'static str {
        "title"
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A made-up game of `size` bytes with a sound header for `mapping`.
    fn make_rom(mapping: Mapping, size: usize) -> Vec<u8> {
        titled(mapping, size, b"TEST GAME", 0)
    }

    /// The same with this title and version byte in its header: other tests make one to say which
    /// game a cartridge claims to be.
    pub(crate) fn titled(mapping: Mapping, size: usize, title: &[u8], version: u8) -> Vec<u8> {
        let mode = match mapping {
            Mapping::LoRom => 0x20,
            Mapping::HiRom => 0x21,
            Mapping::ExHiRom => 0x35,
        };
        let mut rom: Vec<u8> = (0..size).map(|i| ((i * 7 + (i >> 8)) & 0xFF) as u8).collect();
        let base = mapping.header_offset();
        rom[base..base + 0x40].fill(0);
        rom[base..base + 0x15].fill(b' ');
        rom[base..base + title.len()].copy_from_slice(title);
        rom[base + 0x15] = mode;
        rom[base + 0x16] = 0x02;
        rom[base + 0x17] = 0x0C;
        rom[base + 0x18] = 0x03;
        rom[base + 0x19] = 0x00;
        rom[base + 0x1B] = version;
        rom[base + 0x3C..base + 0x3E].copy_from_slice(&0x8000u16.to_le_bytes());
        // The checksum covers its own fields, which always sum to 0x1FE once they are consistent.
        rom[base + 0x1C..base + 0x20].copy_from_slice(&[0xFF, 0xFF, 0x00, 0x00]);
        let checksum = compute_checksum(&rom);
        rom[base + 0x1C..base + 0x1E].copy_from_slice(&(checksum ^ 0xFFFF).to_le_bytes());
        rom[base + 0x1E..base + 0x20].copy_from_slice(&checksum.to_le_bytes());
        rom
    }

    #[test]
    fn a_header_is_found_for_each_mapping() {
        for (mapping, size) in [(Mapping::LoRom, 0x80000), (Mapping::HiRom, 0x100000), (Mapping::ExHiRom, 0x600000)] {
            let rom = make_rom(mapping, size);
            let header = detect_header(&rom, LIKELY_HEADER).unwrap();
            assert_eq!(header.mapping, mapping);
            assert_eq!(header.title, "TEST GAME");
            assert_eq!(header.region(), "Japan");
            assert_eq!(header.rom_size_kb, 4096);
            assert_eq!(header.ram_size_kb, 8);
            assert_eq!(header.checksum, compute_checksum(&rom));
        }
    }

    #[test]
    fn noise_is_not_a_header() {
        assert_eq!(detect_header(&[0; 0x100000], LIKELY_HEADER), None);
        assert_eq!(detect_header(b"too small", LIKELY_HEADER), None);
    }

    #[test]
    fn a_title_is_read_as_single_byte_shift_jis() {
        assert_eq!(title_text(b"A GAME TITLE         "), "A GAME TITLE");
        assert_eq!(title_text(&[0xCC, 0xA7, 0xB2, 0xC5, 0xD9, 0x20, 0x20]), "ﾌｧｲﾅﾙ");
        assert_eq!(title_text(&[0x83, 0x41, 0x1F]), "\u{FFFD}A");
        let mut header = detect_header(&make_rom(Mapping::LoRom, 0x80000), 0).unwrap();
        header.region_code = 0x0C;
        assert_eq!(header.region(), "unknown (0x0c)");
        assert!(!header.fast_rom());
    }

    #[test]
    fn a_copier_header_is_told_by_the_size() {
        let rom = make_rom(Mapping::LoRom, 0x80000);
        let headered = [&[0; 512], rom.as_slice()].concat();
        assert!(has_copier_header(headered.len()));
        assert!(!has_copier_header(rom.len()));
        assert_eq!(strip_copier_header(&headered), rom);
        assert_eq!(strip_copier_header(&rom), rom);
    }

    #[test]
    fn the_checksum_mirrors_a_size_that_is_not_a_power_of_two() {
        // 6 units: the trailing 2 are mirrored to fill 4, so they count twice.
        let unit = 0x1000;
        let rom = [vec![1; 4 * unit], vec![2; 2 * unit]].concat();
        assert_eq!(compute_checksum(&rom), ((4 * unit + 2 * (2 * unit * 2)) & 0xFFFF) as u16);
        assert_eq!(compute_checksum(&[3; 0x8000]), ((3 * 0x8000) & 0xFFFF) as u16);
    }

    #[test]
    fn addresses_convert_both_ways() {
        for (mapping, offset, address) in [
            (Mapping::LoRom, 0x000000, 0x808000),
            (Mapping::LoRom, 0x007FC0, 0x80FFC0),
            (Mapping::LoRom, 0x018123, 0x838123),
            (Mapping::HiRom, 0x00FFC0, 0xC0FFC0),
            (Mapping::HiRom, 0x3FFFFF, 0xFFFFFF),
            (Mapping::ExHiRom, 0x00FFC0, 0xC0FFC0),
            (Mapping::ExHiRom, 0x40FFC0, 0x40FFC0),
            (Mapping::ExHiRom, 0x5ABCDE, 0x5ABCDE),
            (Mapping::ExHiRom, 0x7E8000, 0x3E8000),
        ] {
            assert_eq!(pc_to_snes(offset, mapping).unwrap(), address);
            assert_eq!(snes_to_pc(address, mapping).unwrap(), offset);
        }
    }

    #[test]
    fn mirrors_convert_and_what_is_not_rom_is_refused() {
        assert_eq!(snes_to_pc(0x008000, Mapping::LoRom).unwrap(), 0);
        assert_eq!(snes_to_pc(0x00FFC0, Mapping::HiRom).unwrap(), 0x00FFC0);
        assert_eq!(snes_to_pc(0x00FFC0, Mapping::ExHiRom).unwrap(), 0x40FFC0);
        assert_eq!(snes_to_pc(0x80FFC0, Mapping::ExHiRom).unwrap(), 0x00FFC0);
        assert_eq!(snes_to_pc(0x7E0000, Mapping::HiRom).unwrap_err().to_string(), "$7e0000 is work RAM");
        assert_eq!(snes_to_pc(0x001234, Mapping::LoRom).unwrap_err().to_string(), "$001234 is not ROM in LoROM");
        assert_eq!(snes_to_pc(0x001234, Mapping::ExHiRom).unwrap_err().to_string(), "$001234 is not ROM in ExHiROM");
        assert_eq!(
            pc_to_snes(0x7E0000, Mapping::ExHiRom).unwrap_err().to_string(),
            "offset 0x7e0000 is not mapped in ExHiROM"
        );
        assert_eq!(
            pc_to_snes(0x400000, Mapping::HiRom).unwrap_err().to_string(),
            "offset 0x400000 is outside HiROM space"
        );
        assert!(matches!(pc_to_snes(0x400000, Mapping::LoRom), Err(Error::Image(_))));
    }

    #[test]
    fn a_rom_is_expanded_and_its_checksum_brought_up_to_date() {
        let rom = make_rom(Mapping::HiRom, 0x100000);
        let grown = expand(&rom, 0x200000).unwrap();
        assert_eq!(grown.len(), 0x200000);
        assert_eq!(grown[..0xFFC0 + 0x17], rom[..0xFFC0 + 0x17]);
        assert!(grown[0x100000..].iter().all(|byte| *byte == 0xFF));
        assert_eq!(detect_header(&grown, 0).unwrap().rom_size_kb, 2048);
        let fixed = update_checksum(&grown).unwrap();
        let header = detect_header(&fixed, LIKELY_HEADER).unwrap();
        assert_eq!(header.checksum, compute_checksum(&fixed));
        assert_eq!(header.checksum + header.checksum_complement, 0xFFFF);
        assert_eq!(expand(&rom, 0x80000).unwrap_err().to_string(), "a ROM cannot be expanded to a smaller size");
        assert_eq!(update_checksum(b"not a game").unwrap_err().to_string(), "no SNES header found");
    }

    #[test]
    fn hirom_becomes_exhirom_with_bank_zero_copied() {
        let mut rom = make_rom(Mapping::HiRom, 0x400000);
        rom[0x8000..0x8004].copy_from_slice(b"BOOT");
        let grown = to_exhirom(&rom).unwrap();
        assert_eq!(grown.len(), 0x800000);
        assert_eq!(grown[..0xFFC0], rom[..0xFFC0]);
        // Bank $00 now shows the second half of the file: the start-up code, header and vectors are there too.
        assert_eq!(grown[0x408000..0x408004], *b"BOOT");
        assert_eq!(grown[0x40FFC0..0x40FFD5], grown[0xFFC0..0xFFD5]);
        let header = detect_header(&grown, 0).unwrap();
        assert_eq!((header.mapping, header.rom_size_kb, header.map_mode & 0x0F), (Mapping::ExHiRom, 8192, 5));
        // Both copies of the header get the checksum.
        let fixed = update_checksum(&grown).unwrap();
        assert_eq!(detect_header(&fixed, LIKELY_HEADER).unwrap().checksum, compute_checksum(&fixed));
        assert_eq!(fixed[0x40FFDC..0x40FFE0], fixed[0xFFDC..0xFFE0]);
        assert_eq!(to_exhirom(&grown).unwrap_err().to_string(), "only a 4 MiB HiROM image can be turned into ExHiROM");
    }

    #[test]
    fn the_bank_allocator_never_crosses_a_bank() {
        let mut space = BankAllocator::new(0x60FF00, 0x620000);
        assert_eq!(space.allocate(0x80).unwrap(), 0x60FF00);
        assert_eq!(space.allocate(0x80).unwrap(), 0x60FF80);
        assert_eq!(space.allocate(1).unwrap(), 0x610000);
        let mut space = BankAllocator::new(0x60FFF0, 0x620000);
        assert_eq!(space.allocate(0x20).unwrap(), 0x610000); // skips the 16 bytes left in the first bank
        assert_eq!(space.allocate(0xFFE0).unwrap(), 0x610020);
        assert_eq!(space.allocate(1).unwrap_err().to_string(), "no room left for 1 bytes");
        assert_eq!(
            BankAllocator::new(0, 0x100000).allocate(0x10001).unwrap_err().to_string(),
            "65537 bytes cannot fit inside one bank"
        );
    }

    #[test]
    fn code_is_written_at_its_cpu_address() {
        let rom = make_rom(Mapping::HiRom, 0x100000);
        let patched =
            write_code(&rom, &[(0xC01234, b"CODE".to_vec()), (0xC0FFFC, vec![1, 2])], Mapping::HiRom).unwrap();
        assert_eq!(patched[0x1234..0x1238], *b"CODE");
        assert_eq!(patched[0xFFFC..0xFFFE], [1, 2]);
        assert_eq!(patched[..0x1234], rom[..0x1234]);
        assert_eq!(
            write_code(&rom, &[(0xD0FFFF, vec![1, 2])], Mapping::HiRom).unwrap_err().to_string(),
            "$d0ffff is past the end of the ROM"
        );
    }

    #[test]
    fn the_platform_opens_a_file_with_or_without_a_copier_header() {
        let folder = tempfile::tempdir().unwrap();
        let rom = make_rom(Mapping::HiRom, 0x100000);
        let (plain, headered, other) =
            (folder.path().join("game.sfc"), folder.path().join("game.smc"), folder.path().join("notes.txt"));
        fs::write(&plain, &rom).unwrap();
        fs::write(&headered, [&[0; 512], rom.as_slice()].concat()).unwrap();
        fs::write(&other, "not a game").unwrap();
        assert!(PLATFORM.recognises(&plain).unwrap() && PLATFORM.recognises(&headered).unwrap());
        assert!(!PLATFORM.recognises(&other).unwrap());
        assert!(matches!(PLATFORM.recognises(&folder.path().join("missing.sfc")), Err(Error::Io(_))));
        let (image, sha1) = PLATFORM.open(&plain).unwrap();
        let (stripped, stripped_sha1) = PLATFORM.open(&headered).unwrap();
        assert_eq!(image.cartridge().unwrap(), rom);
        assert_eq!(stripped.cartridge().unwrap(), rom);
        assert_eq!(sha1, stripped_sha1);
        assert!(sha1.len() == 40 && sha1.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));

        assert!(PLATFORM.changes(&image, &stripped).is_empty());
        let grown = Image::Cartridge(expand(&make_rom(Mapping::HiRom, 0x400000), 0x800000).unwrap());
        let small = Image::Cartridge(make_rom(Mapping::HiRom, 0x400000));
        assert_eq!(PLATFORM.changes(&small, &grown), ["size:      expanded from 4 to 8 MiB"]);
    }

    #[test]
    fn a_cartridge_says_its_title_and_version() {
        let mut rom = make_rom(Mapping::HiRom, 0x100000);
        rom[0xFFC0 + 0x1B] = 1;
        let label = PLATFORM.label(&Image::Cartridge(rom)).unwrap();
        assert_eq!((label.name.as_str(), label.version.as_str()), ("TEST GAME", "1.1"));
        let words = crate::platform::label_words(&PLATFORM, &label.name, &label.version);
        assert_eq!(words, "title \"TEST GAME\", version 1.1");
        assert_eq!(PLATFORM.label(&Image::Cartridge(vec![0; 0x100000])), None);
    }

    #[test]
    fn a_plain_cartridge_file_says_what_it_is_without_being_read_whole() {
        let folder = tempfile::tempdir().unwrap();
        let rom = titled(Mapping::HiRom, 0x100000, b"PEEKED", 2);
        let expected = Some(Label { name: "PEEKED".into(), version: "1.2".into() });
        for (name, data) in [("plain.sfc", rom.clone()), ("copier.smc", [vec![0; 512], rom.clone()].concat())] {
            let path = folder.path().join(name);
            fs::write(&path, data).unwrap();
            assert_eq!(PLATFORM.peek(&path), expected, "{name}");
        }
        // Not looked into: an archive, a file with no header, a file that is not there.
        let zipped = folder.path().join("game.zip");
        fs::write(&zipped, archive::make(&[("game.sfc", &rom, true)])).unwrap();
        let noise = folder.path().join("noise.sfc");
        fs::write(&noise, vec![0; 0x100000]).unwrap();
        for path in [zipped, noise, folder.path().join("missing.sfc")] {
            assert_eq!(PLATFORM.peek(&path), None);
        }
    }
}
