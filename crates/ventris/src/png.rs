//! Minimal greyscale PNG writer, for pictures of a game's font.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};

/// One piece of a PNG file: its length, its kind, its data, and a checksum over the last two.
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut crc = Crc::new();
    crc.update(kind);
    crc.update(data);
    [&(data.len() as u32).to_be_bytes(), kind.as_slice(), data, &crc.sum().to_be_bytes()].concat()
}

/// Encode rows of 8-bit greyscale pixels as a PNG file. Every row is as long as the first.
pub fn encode_greyscale(rows: &[Vec<u8>]) -> Vec<u8> {
    let height = rows.len() as u32;
    let width = rows.first().map_or(0, Vec::len) as u32;
    // Each row is preceded by the filter it was put through: 0, none.
    let mut raw = Vec::with_capacity(rows.len() * (width as usize + 1));
    for row in rows {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut packer = ZlibEncoder::new(Vec::new(), Compression::new(6));
    packer.write_all(&raw).expect("writing to memory cannot fail");
    let packed = packer.finish().expect("writing to memory cannot fail");
    // 8 bits per pixel, greyscale, and the only compression, filtering and interlacing there are.
    let header = [&width.to_be_bytes(), &height.to_be_bytes(), [8, 0, 0, 0, 0].as_slice()].concat();
    [b"\x89PNG\r\n\x1a\n".as_slice(), &chunk(b"IHDR", &header), &chunk(b"IDAT", &packed), &chunk(b"IEND", &[])].concat()
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::ZlibDecoder;

    use super::*;

    #[test]
    fn a_picture_is_a_header_the_packed_rows_and_an_end() {
        let png = encode_greyscale(&[vec![0, 64, 255], vec![1, 2, 3]]);
        assert_eq!(png[..8], *b"\x89PNG\r\n\x1a\n");
        // IHDR: 13 bytes, 3 pixels wide, 2 high, 8 bits, greyscale. Its checksum is the one any PNG
        // of this size and kind has.
        assert_eq!(png[8..16], *b"\x00\x00\x00\x0dIHDR");
        assert_eq!(png[16..29], [0, 0, 0, 3, 0, 0, 0, 2, 8, 0, 0, 0, 0]);
        assert_eq!(png[29..33], 0xB81F_39C6u32.to_be_bytes());
        let length = u32::from_be_bytes(png[33..37].try_into().unwrap()) as usize;
        assert_eq!(png[37..41], *b"IDAT");
        let mut raw = Vec::new();
        ZlibDecoder::new(&png[41..41 + length]).read_to_end(&mut raw).unwrap();
        assert_eq!(raw, [0, 0, 64, 255, 0, 1, 2, 3]);
        assert_eq!(png[41 + length + 4..], *b"\x00\x00\x00\x00IEND\xae\x42\x60\x82");
    }

    #[test]
    fn a_picture_of_nothing_is_still_a_file() {
        let png = encode_greyscale(&[]);
        assert_eq!(png[16..24], [0; 8]);
        assert!(png.ends_with(b"IEND\xae\x42\x60\x82"));
    }
}
