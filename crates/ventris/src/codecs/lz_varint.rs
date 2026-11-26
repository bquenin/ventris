//! LZ77 with variable-length numbers and counts packed in nibbles, as used by Super Robot Wars Z
//! (PlayStation 2) for nearly every file of its disc.
//!
//! Numbers are written a byte at a time, the most significant first: `value = value << 7 | byte`,
//! until the value's lowest bit is set; the number is then `value >> 1`. A block is a header of
//! such numbers followed by the stream:
//!
//! ```text
//! size        the number of bytes the block expands to
//! flags       bits 1-4: the window is 1 << (bits + 8) bytes
//! extra       present only when bit 6 of the flags is set, and either the window is smaller than
//!             the size or the flags' bits 0 and 5 are not exactly 1 and 0
//! reserved    a number; 0 in every block seen
//! ```
//!
//! The stream is a run of groups until the output is full. A group starts with a byte whose low
//! nibble is how many literal bytes follow and whose high nibble is how many matches follow them; a
//! nibble of 0 means a number follows instead. Then come the literals, then the matches. A match is
//! a byte `c`: its distance is `c & 0xF` shifted right once when bit 0 of that is set, and
//! otherwise a number whose first bits are that nibble and which goes on in the next bytes; its
//! length is `c >> 4`, or a number when that is 0. Both are one more than written. A match copies
//! byte by byte from that far back, so it may overlap what it writes. The stream ends as soon as
//! the output is full, even in the middle of a group.
//!
//! This is what the game's decompressor does (0x1C6C40 reads the header and 0x1C6D70 the stream, in
//! the program of version 1.04). Only expanding is written so far.

use crate::error::{Error, Result};

fn codec_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Codec(message.into()))
}

/// The number that starts at `position`, continuing from `value` (0 for a number of its own), and
/// where the next one starts.
fn number(data: &[u8], mut position: usize, mut value: u64) -> Result<(usize, usize)> {
    loop {
        let Some(&byte) = data.get(position) else {
            return codec_error("a number runs past the end of the block");
        };
        position += 1;
        value = value << 7 | u64::from(byte);
        if value & 1 == 1 {
            let value = usize::try_from(value >> 1).map_err(|_| Error::Codec("a number is too large".into()))?;
            return Ok((value, position));
        }
        if value >> 56 != 0 {
            return codec_error("a number is too large");
        }
    }
}

/// What a block's header says, and where its stream starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// How many bytes the block expands to.
    pub size: usize,
    pub flags: usize,
    /// The window, in bytes, that matches reach back into.
    pub window: usize,
    /// Where the stream starts, counted from the start of the block.
    pub stream: usize,
}

/// The header of the block that starts at `start` of `data`.
pub fn header(data: &[u8], start: usize) -> Result<Header> {
    let (size, position) = number(data, start, 0)?;
    let (flags, mut position) = number(data, position, 0)?;
    let window = 1usize << ((flags >> 1 & 0xF) + 8);
    if (window < size || flags & 0x21 != 1) && flags & 0x40 != 0 {
        position = number(data, position, 0)?.1;
    }
    let position = number(data, position, 0)?.1;
    Ok(Header { size, flags, window, stream: position - start })
}

/// Expand the block that starts at `start` of `data`. Returns what it expands to and where in `data`
/// the block ends, which is where the next one may start.
pub fn decompress(data: &[u8], start: usize) -> Result<(Vec<u8>, usize)> {
    let header = header(data, start)?;
    let size = header.size;
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut position = start + header.stream;
    while out.len() < size {
        let Some(&group) = data.get(position) else {
            return codec_error("the block ends before its output is full");
        };
        position += 1;
        let mut literals = usize::from(group & 0xF);
        if literals == 0 {
            (literals, position) = number(data, position, 0)?;
        }
        let mut matches = usize::from(group >> 4);
        if matches == 0 {
            (matches, position) = number(data, position, 0)?;
        }
        let Some(bytes) = position.checked_add(literals).and_then(|end| data.get(position..end)) else {
            return codec_error("literal bytes run past the end of the block");
        };
        out.extend_from_slice(bytes);
        position += literals;
        if out.len() >= size {
            break;
        }
        for _ in 0..matches {
            let Some(&code) = data.get(position) else {
                return codec_error("a match runs past the end of the block");
            };
            position += 1;
            let nibble = usize::from(code & 0xF);
            let distance = if nibble & 1 == 1 {
                nibble >> 1
            } else {
                let (distance, next) = number(data, position, nibble as u64)?;
                position = next;
                distance
            } + 1;
            let length = match usize::from(code >> 4) {
                0 => {
                    let (length, next) = number(data, position, 0)?;
                    position = next;
                    length
                }
                length => length,
            } + 1;
            if distance > out.len() {
                return codec_error(format!("a match reaches {distance} bytes back with {} written", out.len()));
            }
            let length = length.min(size - out.len());
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
            if out.len() >= size {
                break;
            }
        }
    }
    out.truncate(size);
    Ok((out, position))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A number as the scheme writes it: seven bits in the high bits of each byte, the last byte
    /// marked by its low bit.
    fn written(mut value: usize) -> Vec<u8> {
        let mut bytes = vec![((value & 0x7F) << 1 | 1) as u8];
        value >>= 7;
        while value != 0 {
            bytes.push(((value & 0x7F) << 1) as u8);
            value >>= 7;
        }
        bytes.reverse();
        bytes
    }

    #[test]
    fn numbers_are_read_most_significant_first_until_the_low_bit_is_set() {
        assert_eq!(number(&[0x03], 0, 0).unwrap(), (1, 1));
        assert_eq!(number(&[0x7F], 0, 0).unwrap(), (63, 1));
        // 0x02 then 0x01: (0x02 << 7 | 0x01) >> 1 = 128.
        assert_eq!(number(&[0x02, 0x01], 0, 0).unwrap(), (128, 2));
        for value in [0, 1, 63, 64, 127, 128, 300, 4096, 524_032, 1 << 30] {
            let bytes = written(value);
            assert_eq!(number(&bytes, 0, 0).unwrap(), (value, bytes.len()), "{value}");
        }
        assert!(number(&[0x02], 0, 0).is_err());
    }

    #[test]
    fn a_block_of_literals_alone_expands_to_them() {
        // size 5, flags 0x11 (a window of 256 bytes), reserved 0; one group of five literals.
        let block = [written(5), written(0x11), written(0), vec![0x15], b"hello".to_vec()].concat();
        let (out, end) = decompress(&block, 0).unwrap();
        assert_eq!((out.as_slice(), end), (&b"hello"[..], block.len()));
        // The group's match count is never read once the output is full.
        let header = header(&block, 0).unwrap();
        assert_eq!((header.size, header.window, header.stream), (5, 256 << 8, 3));
    }

    #[test]
    fn a_match_copies_from_behind_and_may_overlap_what_it_writes() {
        // "ab", then one match: distance from the nibble 3 (odd, so 3 >> 1 = 1, plus 1 = 2), length
        // 5 + 1 = 6: "ababab". Then a group of one literal and no more matches.
        let stream = [vec![0x12], b"ab".to_vec(), vec![0x53], vec![0x11], b"!".to_vec()].concat();
        let block = [written(9), written(0x11), written(0), stream].concat();
        assert_eq!(decompress(&block, 0).unwrap().0, b"abababab!");
        // An even nibble starts a number for the distance: nibble 0 then 0x05 gives (0 << 7 | 5) >> 1
        // = 2, plus 1 = 3; length 2 + 1 = 3.
        let stream = [vec![0x13], b"xyz".to_vec(), vec![0x20, 0x05]].concat();
        let block = [written(6), written(0x11), written(0), stream].concat();
        assert_eq!(decompress(&block, 0).unwrap().0, b"xyzxyz");
    }

    #[test]
    fn a_block_that_lies_is_refused() {
        // A match that reaches back before the start.
        let block = [written(4), written(0x11), written(0), vec![0x11], b"a".to_vec(), vec![0x15]].concat();
        assert!(decompress(&block, 0).is_err());
        // Literals that run past the end.
        let block = [written(4), written(0x11), written(0), vec![0x14], b"ab".to_vec()].concat();
        assert!(decompress(&block, 0).is_err());
        // A block that stops before its output is full.
        let block = [written(4), written(0x11), written(0), vec![0x12], b"ab".to_vec()].concat();
        assert!(decompress(&block, 0).is_err());
    }
}
