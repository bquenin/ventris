//! LZSS with optional run-length encoding, as used by Tales of Phantasia (SNES).
//!
//! A block is a 9-byte header followed by the payload:
//!
//! ```text
//! byte 0      mode: 0x80 stored, 0x81 LZSS, 0x83 LZSS + RLE
//! bytes 1-4   payload size, little-endian
//! bytes 5-8   decompressed size, little-endian
//! ```
//!
//! The payload is a sequence of flag bytes, each followed by eight tokens, read from the low bit up.
//! A set bit is a literal byte. A clear bit is a two-byte token `b1 b2`:
//!
//! ```text
//! back-reference   distance = b1 | (b2 & 0x0F) << 8,  length = (b2 >> 4) + 3
//! short run        mode 0x83, b2 >> 4 == 0xF:  (b2 & 0x0F) + 3 copies of b1
//! long run         mode 0x83, b2 == 0xF0:      b1 + 19 copies of the next byte
//! ```
//!
//! The distance counts back from the current end of the output, and a copy may overlap the bytes it
//! writes.

use std::collections::HashMap;

use crate::error::{Error, Result};

pub const HEADER_SIZE: usize = 9;
pub const MAX_DISTANCE: usize = 0xFFF;

pub const MODE_STORED: u8 = 0x80;
pub const MODE_LZSS: u8 = 0x81;
pub const MODE_LZSS_RLE: u8 = 0x83;

/// The longest run one token can stand for.
const LONGEST_RUN: usize = 274;

fn codec_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Codec(message.into()))
}

/// The mode, the payload size and the decompressed size of the block at `pos`.
pub fn read_header(data: &[u8], pos: usize) -> Result<(u8, usize, usize)> {
    let Some(header) = pos.checked_add(HEADER_SIZE).and_then(|end| data.get(pos..end)) else {
        return codec_error("truncated block header");
    };
    let number = |at: usize| u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]) as usize;
    Ok((header[0], number(1), number(5)))
}

/// Total size of the block at `pos`, header included.
pub fn block_size(data: &[u8], pos: usize) -> Result<usize> {
    Ok(HEADER_SIZE + read_header(data, pos)?.1)
}

/// Decompress the block that starts at `pos`.
pub fn decompress_block(data: &[u8], pos: usize) -> Result<Vec<u8>> {
    let (mode, payload_size, size) = read_header(data, pos)?;
    let start = pos + HEADER_SIZE;
    let Some(payload) = start.checked_add(payload_size).and_then(|end| data.get(start..end)) else {
        return codec_error("truncated block payload");
    };
    decompress(payload, mode, size)
}

/// Decompress a payload of `mode` to `size` bytes.
pub fn decompress(payload: &[u8], mode: u8, size: usize) -> Result<Vec<u8>> {
    if mode == MODE_STORED {
        if payload.len() != size {
            return codec_error("stored block size does not match its header");
        }
        return Ok(payload.to_vec());
    }
    if mode != MODE_LZSS && mode != MODE_LZSS_RLE {
        return codec_error(format!("unsupported compression mode 0x{mode:02x}"));
    }

    // The size comes from a header, which may say anything: room is only set aside for what a
    // payload of this length could give.
    let mut out: Vec<u8> = Vec::with_capacity(size.min(payload.len().saturating_mul(LONGEST_RUN)));
    let mut pos = 0;
    let next = |pos: &mut usize| -> Result<u8> {
        let Some(byte) = payload.get(*pos) else {
            return codec_error("block payload ends before the decompressed size is reached");
        };
        *pos += 1;
        Ok(*byte)
    };
    while out.len() < size {
        let flags = next(&mut pos)?;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags >> bit & 1 == 1 {
                out.push(next(&mut pos)?);
                continue;
            }
            let (b1, b2) = (next(&mut pos)?, next(&mut pos)?);
            let (high, low) = (usize::from(b2 >> 4), usize::from(b2 & 0x0F));
            if mode == MODE_LZSS_RLE && high == 0x0F {
                let (byte, count) = if low != 0 { (b1, low + 3) } else { (next(&mut pos)?, usize::from(b1) + 19) };
                out.resize(out.len() + count, byte);
                continue;
            }
            let distance = usize::from(b1) | low << 8;
            if distance == 0 || distance > out.len() {
                return codec_error(format!(
                    "back-reference distance {distance} reaches before the start of the output"
                ));
            }
            // Byte by byte, because the bytes read may be the ones being written.
            for _ in 0..high + 3 {
                out.push(out[out.len() - distance]);
            }
        }
    }
    if pos != payload.len() {
        return codec_error(format!("block payload has {} unused bytes", payload.len() - pos));
    }
    // The original compressor lets the last token run past the end; the excess is dropped.
    out.truncate(size);
    Ok(out)
}

/// Compress `data` into a block, header included, using whichever mode is smallest.
pub fn compress_block(data: &[u8]) -> Result<Vec<u8>> {
    let (mut mode, mut payload) = (MODE_STORED, data.to_vec());
    for candidate in [MODE_LZSS, MODE_LZSS_RLE] {
        let packed = compress(data, candidate)?;
        if packed.len() < payload.len() {
            (mode, payload) = (candidate, packed);
        }
    }
    let (Ok(payload_size), Ok(size)) = (u32::try_from(payload.len()), u32::try_from(data.len())) else {
        return codec_error("the data is too large for a block, whose sizes are 32 bits");
    };
    Ok([&[mode], payload_size.to_le_bytes().as_slice(), &size.to_le_bytes(), &payload].concat())
}

/// One token of a payload, as the compressor chooses them.
#[derive(Clone, Copy)]
enum Token {
    Literal,
    /// This many copies of one byte.
    Run(usize),
    /// This many bytes, copied from this far back.
    Reference(usize, usize),
}

/// Compress `data` to a payload for `mode`.
///
/// The cheapest sequence of tokens for the whole block is chosen, not the longest match at each
/// step, so the output is usually a little smaller than the original compressor's and never relies
/// on its quirks. It decompresses to the same data but is not byte-identical to the original.
///
/// Built images are compared with those of the Python reference, so every choice between tokens of
/// equal cost is made as it makes it.
pub fn compress(data: &[u8], mode: u8) -> Result<Vec<u8>> {
    if mode == MODE_STORED {
        return Ok(data.to_vec());
    }
    if mode != MODE_LZSS && mode != MODE_LZSS_RLE {
        return codec_error(format!("unsupported compression mode 0x{mode:02x}"));
    }
    let rle = mode == MODE_LZSS_RLE;
    let max_match = if rle { 17 } else { 18 }; // in RLE mode a length nibble of 0xF marks a run instead
    let size = data.len();

    // The longest earlier match at each position, as (length, distance). Any shorter length is
    // available at the same distance. Earlier positions that start with the same three bytes are
    // tried from the nearest back, and a longer match must be strictly longer to be preferred.
    const NONE: usize = usize::MAX;
    let mut longest = vec![(0, 0); size];
    let mut latest: HashMap<[u8; 3], usize> = HashMap::new();
    let mut before = vec![NONE; size]; // the nearest earlier position that starts with the same three bytes
    for pos in 0..size.saturating_sub(2) {
        let limit = max_match.min(size - pos);
        let key = [data[pos], data[pos + 1], data[pos + 2]];
        let (mut best_length, mut best_distance) = (0, 0);
        let mut start = latest.get(&key).copied().unwrap_or(NONE);
        while start != NONE {
            let distance = pos - start;
            if distance > MAX_DISTANCE {
                break;
            }
            let mut length = 3;
            while length < limit && data[start + length] == data[pos + length] {
                length += 1;
            }
            if length > best_length {
                (best_length, best_distance) = (length, distance);
                if length == limit {
                    break;
                }
            }
            start = before[start];
        }
        longest[pos] = (best_length, best_distance);
        before[pos] = latest.insert(key, pos).unwrap_or(NONE);
    }

    // How many times the byte at each position repeats from there on.
    let mut run = vec![1; size];
    for pos in (0..size.saturating_sub(1)).rev() {
        if data[pos] == data[pos + 1] {
            run[pos] = (run[pos + 1] + 1).min(LONGEST_RUN);
        }
    }

    // Cheapest cost in bits from each position to the end: a literal is 9, a two-byte token 17,
    // a long run 25. `step` records the token chosen.
    let mut cost = vec![0u64; size + 1];
    let mut step = vec![Token::Literal; size];
    for pos in (0..size).rev() {
        let (mut best, mut choice) = (cost[pos + 1] + 9, Token::Literal);
        let (length, distance) = longest[pos];
        for take in 3..=length {
            if cost[pos + take] + 17 < best {
                (best, choice) = (cost[pos + take] + 17, Token::Reference(take, distance));
            }
        }
        if rle {
            for take in 4..=run[pos] {
                let price = cost[pos + take] + if take <= 18 { 17 } else { 25 };
                if price < best {
                    (best, choice) = (price, Token::Run(take));
                }
            }
        }
        (cost[pos], step[pos]) = (best, choice);
    }

    let mut out = Vec::new();
    let (mut flag_at, mut bit) = (0, 8); // forces a new flag byte before the first token
    let mut pos = 0;
    while pos < size {
        if bit == 8 {
            (flag_at, bit) = (out.len(), 0);
            out.push(0);
        }
        pos += match step[pos] {
            Token::Literal => {
                out[flag_at] |= 1 << bit;
                out.push(data[pos]);
                1
            }
            Token::Run(length) => {
                if length <= 18 {
                    out.extend([data[pos], 0xF0 | (length - 3) as u8]);
                } else {
                    out.extend([(length - 19) as u8, 0xF0, data[pos]]);
                }
                length
            }
            Token::Reference(length, distance) => {
                out.extend([(distance & 0xFF) as u8, ((length - 3) << 4 | distance >> 8) as u8]);
                length
            }
        };
        bit += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(mode: u8, payload: &[u8], size: u32) -> Vec<u8> {
        [&[mode], (payload.len() as u32).to_le_bytes().as_slice(), &size.to_le_bytes(), payload].concat()
    }

    /// What is wrong with compressed data, in the words of the error.
    fn fault<T>(result: Result<T>) -> String {
        match result {
            Err(error @ Error::Codec(_)) => error.to_string(),
            Err(other) => panic!("refused, but not as compressed data that is wrong: {other:?}"),
            Ok(_) => panic!("it was not refused"),
        }
    }

    /// Numbers that look random and are the same on every run.
    struct Noise(u64);

    impl Noise {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, limit: usize) -> usize {
            (self.next() % limit as u64) as usize
        }

        fn bytes(&mut self, count: usize) -> Vec<u8> {
            (0..count).map(|_| self.next() as u8).collect()
        }
    }

    #[test]
    fn literals_only() {
        assert_eq!(decompress(b"\xffABCDEFGH\x01I", MODE_LZSS, 9).unwrap(), b"ABCDEFGHI");
    }

    #[test]
    fn a_back_reference_counts_from_the_end_of_the_output() {
        // Three literals, then copy 4 bytes from 3 back: the copy overlaps what it writes.
        let payload = [&[0b0111], b"abc".as_slice(), &[0x03, 0x10]].concat();
        assert_eq!(decompress(&payload, MODE_LZSS, 7).unwrap(), b"abcabca");
    }

    #[test]
    fn a_distance_has_twelve_bits() {
        let body: Vec<u8> = (0..512).map(|value| value as u8).collect();
        let mut payload = Vec::new();
        for chunk in body.chunks(8) {
            payload.push(0xFF);
            payload.extend(chunk);
        }
        payload.extend([0x00, 0x00, 0x02]); // distance 0x200, length 3
        assert_eq!(decompress(&payload, MODE_LZSS, body.len() + 3).unwrap(), [&body[..], &body[..3]].concat());
    }

    #[test]
    fn runs_only_exist_in_rle_mode() {
        let short = [0b0, 0x41, 0xF2]; // 5 x 'A'
        assert_eq!(decompress(&short, MODE_LZSS_RLE, 5).unwrap(), b"AAAAA");
        let long = [0b0, 0x01, 0xF0, 0x42]; // 20 x 'B'
        assert_eq!(decompress(&long, MODE_LZSS_RLE, 20).unwrap(), [b'B'; 20]);
        // In plain LZSS mode the same token is a back-reference of length 18.
        let plain = [0b01, b'Z', 0x01, 0xF0];
        assert_eq!(decompress(&plain, MODE_LZSS, 19).unwrap(), [b'Z'; 19]);
    }

    #[test]
    fn the_last_token_may_overrun_the_declared_size() {
        let payload = [0b01, b'x', 0x01, 0x70]; // 1 literal + 10 copies
        assert_eq!(decompress(&payload, MODE_LZSS, 6).unwrap(), [b'x'; 6]);
    }

    #[test]
    fn a_stored_block_and_its_header() {
        let data = [b"prefix".as_slice(), &block(MODE_STORED, b"hello", 5), b"suffix"].concat();
        assert_eq!(read_header(&data, 6).unwrap(), (MODE_STORED, 5, 5));
        assert_eq!(block_size(&data, 6).unwrap(), 14);
        assert_eq!(decompress_block(&data, 6).unwrap(), b"hello");
    }

    #[test]
    fn what_is_not_a_block_is_refused() {
        for (payload, mode, size, message) in [
            (b"\xffab".as_slice(), MODE_LZSS, 5, "block payload ends before the decompressed size is reached"),
            (&[0b0, 0x05, 0x00], MODE_LZSS, 3, "back-reference distance 5 reaches before the start of the output"),
            (&[0b1, b'a', b'b'], MODE_LZSS, 1, "block payload has 1 unused bytes"),
            (b"abc", 0x85, 3, "unsupported compression mode 0x85"),
            (b"abc", MODE_STORED, 4, "stored block size does not match its header"),
        ] {
            assert_eq!(fault(decompress(payload, mode, size)), message);
        }
        assert_eq!(decompress(&[0b1, b'a'], MODE_LZSS, 1).unwrap(), b"a");
        // A token cut short by the end of the payload, in each of its forms.
        for payload in [&[0b0, 0x01][..], &[0b0, 0x01, 0xF0], &[0b0]] {
            assert_eq!(
                fault(decompress(payload, MODE_LZSS_RLE, 30)),
                "block payload ends before the decompressed size is reached"
            );
        }
    }

    #[test]
    fn what_is_compressed_comes_back() {
        for mode in [MODE_STORED, MODE_LZSS, MODE_LZSS_RLE] {
            let mut noise = Noise(0x9E37_79B9_7F4A_7C15 ^ u64::from(mode));
            let words: Vec<Vec<u8>> = (0..40)
                .map(|_| {
                    let length = 2 + noise.below(7);
                    noise.bytes(length)
                })
                .collect();
            let mut text = Vec::new();
            for _ in 0..3000 {
                text.extend(&words[noise.below(words.len())]); // longer than the 4095-byte window
            }
            let samples = [
                Vec::new(),
                b"a".to_vec(),
                b"ab".to_vec(),
                b"abcabcabcabcabcabcabcabcabcabcabcabc".to_vec(),
                vec![0; 3],
                vec![0; 4],
                vec![0; 18],
                vec![0; 19],
                vec![0; 274],
                [vec![0; 1000], b"x".to_vec(), vec![0xFF; 300]].concat(),
                noise.bytes(3000),
                text,
            ];
            for data in samples {
                let payload = compress(&data, mode).unwrap();
                assert!(
                    decompress(&payload, mode, data.len()).unwrap() == data,
                    "mode {mode:#x}, {} bytes",
                    data.len()
                );
            }
        }
    }

    #[test]
    fn the_compressor_makes_the_choices_of_the_python_reference() {
        // Payloads the reference gives for the same data, in each mode.
        let data = b"abcabcabcabcabcabcabcabcabcabcabcabc";
        assert_eq!(compress(data, MODE_LZSS).unwrap(), [0x07, 0x61, 0x62, 0x63, 0x03, 0xC0, 0x03, 0xF0]);
        assert_eq!(compress(data, MODE_LZSS_RLE).unwrap(), [0x07, 0x61, 0x62, 0x63, 0x03, 0xD0, 0x03, 0xE0]);
        // Long runs first, the shorter one that is left over last.
        let data = [vec![0; 1000], b"x".to_vec(), vec![0xFF; 300]].concat();
        #[rustfmt::skip]
        let expected = [
            0x10, 0x9F, 0xF0, 0x00, 0xFF, 0xF0, 0x00, 0xFF, 0xF0, 0x00, 0xFF, 0xF0, 0x00, 0x78, 0x07, 0xF0, 0xFF, 0xFF,
            0xF0, 0xFF,
        ];
        assert_eq!(compress(&data, MODE_LZSS_RLE).unwrap(), expected);
        assert_eq!(compress(&[7; 4], MODE_LZSS_RLE).unwrap(), [0x00, 0x07, 0xF1]);
        assert_eq!(compress(&[7; 4], MODE_LZSS).unwrap(), [0x01, 0x07, 0x01, 0x00]);
        // Runs, literals and references side by side.
        let data = [vec![7; 40], b"abc".to_vec(), vec![7; 20], b"abcabc".to_vec()].concat();
        assert_eq!(
            compress(&data, MODE_LZSS_RLE).unwrap(),
            [0x0E, 0x15, 0xF0, 0x07, 0x61, 0x62, 0x63, 0x14, 0x30, 0x17, 0xE0, 0x03, 0x00]
        );
    }

    #[test]
    fn a_block_takes_the_smallest_mode() {
        let data = [vec![7; 500], b"abc".to_vec()].concat();
        let runs = compress_block(&data).unwrap();
        assert_eq!(runs[0], MODE_LZSS_RLE);
        assert!(runs.len() < 20);
        assert_eq!(decompress_block(&runs, 0).unwrap(), data);
        let noise = Noise(1).bytes(64);
        let stored = compress_block(&noise).unwrap();
        assert_eq!(stored[0], MODE_STORED);
        assert_eq!(decompress_block(&stored, 0).unwrap(), noise);
        assert_eq!(fault(compress(b"abc", 0x85)), "unsupported compression mode 0x85");
    }

    #[test]
    fn a_block_cut_short_is_refused() {
        assert_eq!(fault(read_header(b"\x81\x00", 0)), "truncated block header");
        assert_eq!(fault(read_header(b"\x81\x00", usize::MAX)), "truncated block header");
        let cut = block(MODE_LZSS, b"\xff", 1);
        assert_eq!(fault(decompress_block(&cut[..cut.len() - 1], 0)), "truncated block payload");
    }
}
