//! LZSS over a ring of 4096 bytes that starts out filled, as used by Tales of Destiny 2 (PlayStation 2).
//!
//! A block is a 9-byte header followed by the payload:
//!
//! ```text
//! byte 0      mode: 0 stored, 1 LZSS, 3 LZSS with runs
//! bytes 1-4   payload size, little-endian
//! bytes 5-8   size once expanded, little-endian
//! ```
//!
//! The payload is a sequence of flag bytes, each followed by eight tokens, read from the low bit up.
//! A set bit is a literal byte. A clear bit is a two-byte token `b1 b2`:
//!
//! ```text
//! reference    place in the ring = b1 | (b2 & 0xF0) << 4,  length = (b2 & 0x0F) + 3
//! short run    mode 3, b2 & 0x0F == 0x0F, b2 >> 4 != 0:  (b2 >> 4) + 3 copies of b1
//! long run     mode 3, b2 == 0x0F:                        b1 + 19 copies of the next byte
//! ```
//!
//! A reference names a place in a ring buffer, not a distance back, and the ring holds a fixed
//! pattern before anything is written, so that even the first bytes of a file can be copied from
//! it. Every byte that comes out is also written to the ring. Writing starts at 0xFEE in mode 1 and
//! at 0xFEF in mode 3, and a reference may read what it is writing.
//!
//! The compressor here never copies from the pattern the ring starts with, only from what it has
//! written, so its output is valid but not byte-identical to the original compressor's.

use std::collections::HashMap;

use crate::error::{Error, Result};

pub const HEADER_SIZE: usize = 9;
pub const RING_SIZE: usize = 0x1000;

pub const MODE_STORED: u8 = 0;
pub const MODE_LZSS: u8 = 1;
pub const MODE_LZSS_RLE: u8 = 3;

/// The longest run one token can stand for.
const LONGEST_RUN: usize = 274;

fn codec_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Codec(message.into()))
}

/// Where in the ring the first byte that comes out is written, for the modes that have a ring.
fn first_write(mode: u8) -> Option<usize> {
    match mode {
        MODE_LZSS => Some(0xFEE),
        MODE_LZSS_RLE => Some(0xFEF),
        _ => None,
    }
}

/// The ring before anything is written: two tables of every byte value, then zeros.
pub fn initial_ring() -> [u8; RING_SIZE] {
    let mut ring = [0; RING_SIZE];
    let mut position = 0;
    for value in 0..=0xFF {
        // value, 0, four times over
        ring[position..position + 8].copy_from_slice(&[value, 0, value, 0, value, 0, value, 0]);
        position += 8;
    }
    for value in 0..=0xFF {
        // value, 0xFF, three times over, and the value once more
        ring[position..position + 7].copy_from_slice(&[value, 0xFF, value, 0xFF, value, 0xFF, value]);
        position += 7;
    }
    ring
}

/// The mode, the payload size and the expanded size of the block at `pos`.
pub fn read_header(data: &[u8], pos: usize) -> Result<(u8, usize, usize)> {
    let Some(header) = pos.checked_add(HEADER_SIZE).and_then(|end| data.get(pos..end)) else {
        return codec_error("truncated block header");
    };
    let number = |at: usize| u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]) as usize;
    Ok((header[0], number(1), number(5)))
}

/// Whether `data` is exactly one block: a known mode, and a payload size that accounts for all of it.
pub fn is_block(data: &[u8]) -> bool {
    match read_header(data, 0) {
        Ok((mode, payload_size, _)) => {
            matches!(mode, MODE_STORED | MODE_LZSS | MODE_LZSS_RLE) && HEADER_SIZE + payload_size == data.len()
        }
        Err(_) => false,
    }
}

/// Expand the block that starts at `pos`.
pub fn decompress_block(data: &[u8], pos: usize) -> Result<Vec<u8>> {
    let (mode, payload_size, size) = read_header(data, pos)?;
    let start = pos + HEADER_SIZE;
    let Some(payload) = start.checked_add(payload_size).and_then(|end| data.get(start..end)) else {
        return codec_error("truncated block payload");
    };
    decompress(payload, mode, size)
}

/// The first `most` bytes of what a block expands to, or all of it when it has fewer. `data` starts
/// with the block's header and may stop short of its end: a large block need not be read whole to
/// see how it begins. It fails if `data` stops before that many bytes are reached.
pub fn decompress_start(data: &[u8], most: usize) -> Result<Vec<u8>> {
    let (mode, payload_size, size) = read_header(data, 0)?;
    let payload = &data[HEADER_SIZE..data.len().min(HEADER_SIZE.saturating_add(payload_size))];
    let wanted = most.min(size);
    if mode == MODE_STORED {
        return match payload.get(..wanted) {
            Some(start) => Ok(start.to_vec()),
            None => codec_error("block payload ends before the expanded size is reached"),
        };
    }
    expand(payload, mode, wanted, false)
}

/// How many bytes of a block, header included, are enough to expand its first `most` bytes: the
/// most a compressor spends is a flag byte for every eight bytes it leaves as they are.
pub fn enough_for(most: usize) -> usize {
    HEADER_SIZE + most + most.div_ceil(8) + 4
}

/// Expand a payload of `mode` to `size` bytes.
pub fn decompress(payload: &[u8], mode: u8, size: usize) -> Result<Vec<u8>> {
    if mode == MODE_STORED {
        if payload.len() != size {
            return codec_error("stored block size does not match its header");
        }
        return Ok(payload.to_vec());
    }
    expand(payload, mode, size, true)
}

/// Expand a compressed payload to `size` bytes. With `whole`, the payload must end where the last
/// of them is reached; without, it may go on.
fn expand(payload: &[u8], mode: u8, size: usize, whole: bool) -> Result<Vec<u8>> {
    let Some(mut write) = first_write(mode) else {
        return codec_error(format!("unsupported compression mode {mode}"));
    };

    let mut ring = initial_ring();
    // The size comes from a header, which may say anything: room is only set aside for what a
    // payload of this length could give.
    let mut out = Vec::with_capacity(size.min(payload.len().saturating_mul(LONGEST_RUN)));
    let mut pos = 0;
    let next = |pos: &mut usize| -> Result<u8> {
        let Some(byte) = payload.get(*pos) else {
            return codec_error("block payload ends before the expanded size is reached");
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
                let byte = next(&mut pos)?;
                ring[write] = byte;
                write = (write + 1) % RING_SIZE;
                out.push(byte);
                continue;
            }
            let (b1, b2) = (next(&mut pos)?, next(&mut pos)?);
            if mode == MODE_LZSS_RLE && b2 & 0x0F == 0x0F {
                let (byte, count) =
                    if b2 >> 4 != 0 { (b1, usize::from(b2 >> 4) + 3) } else { (next(&mut pos)?, usize::from(b1) + 19) };
                for _ in 0..count {
                    ring[write] = byte;
                    write = (write + 1) % RING_SIZE;
                }
                out.resize(out.len() + count, byte);
            } else {
                // Byte by byte, because the place read may be the place being written.
                let mut read = usize::from(b1) | usize::from(b2 & 0xF0) << 4;
                for _ in 0..(b2 & 0x0F) + 3 {
                    let byte = ring[read];
                    ring[write] = byte;
                    out.push(byte);
                    read = (read + 1) % RING_SIZE;
                    write = (write + 1) % RING_SIZE;
                }
            }
        }
    }
    if whole && pos != payload.len() {
        return codec_error(format!("block payload has {} unused bytes", payload.len() - pos));
    }
    // The original compressor lets the last token run past the end; the excess is dropped.
    out.truncate(size);
    Ok(out)
}

/// Compress `data` into a block, header included.
pub fn compress_block(data: &[u8], mode: u8) -> Result<Vec<u8>> {
    let payload = compress(data, mode)?;
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
/// The cheapest sequence of tokens for the whole block is chosen: for each position, working back
/// from the end, the token that leaves the fewest bits to the end. Built images are compared with
/// those of the Python reference, so every choice between tokens of equal cost is made as it makes it.
pub fn compress(data: &[u8], mode: u8) -> Result<Vec<u8>> {
    if mode == MODE_STORED {
        return Ok(data.to_vec());
    }
    let Some(first) = first_write(mode) else {
        return codec_error(format!("unsupported compression mode {mode}"));
    };
    let rle = mode == MODE_LZSS_RLE;
    let max_match = if rle { 17 } else { 18 }; // in mode 3 a length nibble of 0xF marks a run instead
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
            if distance >= RING_SIZE {
                break; // by then the ring has been written over
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
                    out.extend([data[pos], ((length - 3) << 4 | 0x0F) as u8]);
                } else {
                    out.extend([(length - 19) as u8, 0x0F, data[pos]]);
                }
                length
            }
            Token::Reference(length, distance) => {
                let place = (first + pos - distance) % RING_SIZE;
                out.extend([(place & 0xFF) as u8, (place >> 4 & 0xF0 | (length - 3)) as u8]);
                length
            }
        };
        bit += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use crate::text::from_hex;

    use super::*;

    fn hex(bytes: &str) -> Vec<u8> {
        from_hex(bytes).unwrap()
    }

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

    #[test]
    fn literals_only() {
        assert_eq!(decompress(b"\xffABCDEFGH\x01I", MODE_LZSS, 9).unwrap(), b"ABCDEFGHI");
    }

    #[test]
    fn the_start_of_a_block_can_be_read_without_the_rest() {
        let data: Vec<u8> = (0..5000u32).map(|n| (n * n / 7 % 251) as u8).collect();
        for mode in [MODE_STORED, MODE_LZSS, MODE_LZSS_RLE] {
            let whole = compress_block(&data, mode).unwrap();
            // Given no more of the block than is sure to be enough.
            let cut = &whole[..whole.len().min(enough_for(100))];
            assert_eq!(decompress_start(cut, 100).unwrap(), data[..100]);
            // Asking for more than there is gives all of it.
            assert_eq!(decompress_start(&whole, 1 << 20).unwrap(), data);
            // A block cut before what is asked for is refused.
            assert_eq!(
                fault(decompress_start(&whole[..HEADER_SIZE + 20], 100)),
                "block payload ends before the expanded size is reached"
            );
        }
        assert_eq!(fault(decompress_start(&[1, 2, 3], 4)), "truncated block header");
    }

    #[test]
    fn the_ring_starts_out_filled() {
        let ring = initial_ring();
        assert_eq!(ring.len(), 0x1000);
        assert_eq!(ring[0x480..0x488], hex("90 00 90 00 90 00 90 00")); // value, 0, four times
        assert_eq!(ring[0x800..0x807], hex("00 ff 00 ff 00 ff 00")); // value, FF, three times, value
        assert_eq!(ring[0x807..0x80E], hex("01 ff 01 ff 01 ff 01"));
        assert!(ring[0xF00..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn a_reference_can_read_what_was_never_written() {
        // Place 0x485 in the untouched ring holds 00 90 00: a file can start by copying from it.
        assert_eq!(decompress(&[0b0, 0x85, 0x40], MODE_LZSS, 3).unwrap(), hex("00 90 00"));
    }

    #[test]
    fn a_reference_names_a_place_not_a_distance() {
        // Mode 1 writes from 0xFEE: after "abc", place 0xFEE holds "a". Five bytes from there overlap the copy.
        let payload = [&[0b0111], b"abc".as_slice(), &[0xEE, 0xF2]].concat();
        assert_eq!(decompress(&payload, MODE_LZSS, 8).unwrap(), b"abcabcab");
    }

    #[test]
    fn the_two_modes_start_writing_in_different_places() {
        let payload = [0b01, b'x', 0xEE, 0xF0]; // one literal, then three bytes from 0xFEE
        // The literal went to 0xFEE, and the copy follows itself.
        assert_eq!(decompress(&payload, MODE_LZSS, 4).unwrap(), b"xxxx");
        // Here the literal went to 0xFEF.
        assert_eq!(decompress(&payload, MODE_LZSS_RLE, 4).unwrap(), b"x\x00x\x00");
    }

    #[test]
    fn runs() {
        assert_eq!(decompress(&[0b0, 0x41, 0x2F], MODE_LZSS_RLE, 5).unwrap(), b"A".repeat(5)); // short: 2 + 3
        assert_eq!(decompress(&[0b0, 0x01, 0x0F, 0x42], MODE_LZSS_RLE, 20).unwrap(), b"B".repeat(20)); // long: 1 + 19
        // A run goes into the ring like anything else.
        assert_eq!(decompress(&[0b00, 0x41, 0x1F, 0xEF, 0xF1], MODE_LZSS_RLE, 8).unwrap(), b"A".repeat(8));
    }

    #[test]
    fn mode_1_has_no_runs() {
        // The same two bytes are a reference of 18 bytes there, to place 0x041: the table entry for 8.
        assert_eq!(
            decompress(&[0b0, 0x41, 0x0F], MODE_LZSS, 18).unwrap(),
            hex("00 08 00 08 00 08 00 09 00 09 00 09 00 09 00 0a 00 0a")
        );
    }

    #[test]
    fn the_last_token_may_run_past_the_end() {
        assert_eq!(decompress(&[0b0, 0x41, 0x2F], MODE_LZSS_RLE, 2).unwrap(), b"AA");
    }

    #[test]
    fn blocks() {
        let packed = block(MODE_LZSS_RLE, &[0b01, b'Z', 0x5A, 0x1F], 5);
        assert!(is_block(&packed));
        assert_eq!(read_header(&packed, 0).unwrap(), (MODE_LZSS_RLE, 4, 5));
        assert_eq!(decompress_block(&packed, 0).unwrap(), b"ZZZZZ");
        assert_eq!(decompress_block(&[b"??".as_slice(), &packed].concat(), 2).unwrap(), b"ZZZZZ");
        assert_eq!(decompress_block(&block(MODE_STORED, b"plain", 5), 0).unwrap(), b"plain");
        assert!(!is_block(&[packed.as_slice(), &[0]].concat())); // the payload size must account for everything
        assert!(!is_block(&[b"SCPK".as_slice(), &[0; 12]].concat()));
        assert!(!is_block(&[0x03]));
    }

    #[test]
    fn errors() {
        assert!(fault(decompress(b"\xffAB", MODE_LZSS, 8)).contains("ends before"));
        // A token cut short in its middle, and a long run without the byte it repeats.
        assert!(fault(decompress(&[0b0, 0x41], MODE_LZSS, 8)).contains("ends before"));
        assert!(fault(decompress(&[0b0, 0x01, 0x0F], MODE_LZSS_RLE, 20)).contains("ends before"));
        assert_eq!(fault(decompress(b"\xffABCDEFGH\x00", MODE_LZSS, 8)), "block payload has 1 unused bytes");
        assert_eq!(fault(decompress(b"", 2, 0)), "unsupported compression mode 2");
        assert_eq!(fault(decompress(b"abc", MODE_STORED, 4)), "stored block size does not match its header");
        assert_eq!(fault(read_header(&[0x03, 0x00], 0)), "truncated block header");
        let packed = block(MODE_LZSS, b"\xffABCDEFGH", 8);
        assert_eq!(fault(decompress_block(&packed[..packed.len() - 2], 0)), "truncated block payload");
        // A header that promises far more than its payload can give is refused, not given room.
        assert!(fault(decompress_block(&block(MODE_LZSS, b"\xffAB", u32::MAX), 0)).contains("ends before"));
    }

    /// Made-up bytes, the same on every run: each is drawn from `choices` by a small generator.
    fn made_up(seed: u32, size: usize, choices: &[u8]) -> Vec<u8> {
        let mut state = seed;
        let mut draw = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            choices[(state >> 16) as usize % choices.len()]
        };
        (0..size).map(|_| draw()).collect()
    }

    #[test]
    fn compress_round_trips() {
        let every_byte: Vec<u8> = (0..=255).collect();
        let mut samples = vec![
            Vec::new(),
            b"a".to_vec(),
            b"ab".repeat(40),
            vec![0; 600],
            every_byte.repeat(20),
            [b"abc".as_slice(), &[0; 300], &b"abc".repeat(9)].concat(),
        ];
        samples.extend([5, 300, 9000].map(|size| made_up(1, size, b"ab\x00")));
        samples.push(made_up(2, 2000, &every_byte));
        for data in &samples {
            for mode in [MODE_LZSS, MODE_LZSS_RLE, MODE_STORED] {
                assert_eq!(&decompress(&compress(data, mode).unwrap(), mode, data.len()).unwrap(), data);
                assert_eq!(&decompress_block(&compress_block(data, mode).unwrap(), 0).unwrap(), data);
            }
        }
    }

    #[test]
    fn compress_uses_runs_and_references() {
        assert_eq!(compress(&b"A".repeat(5), MODE_LZSS_RLE).unwrap(), [0b0, 0x41, 0x2F]);
        assert_eq!(compress(&b"B".repeat(20), MODE_LZSS_RLE).unwrap(), [0b0, 0x01, 0x0F, 0x42]);
        // "abc" again, three bytes back: in mode 3 the first byte went to 0xFEF.
        assert_eq!(compress(b"abcabc", MODE_LZSS_RLE).unwrap(), [0b0111, b'a', b'b', b'c', 0xEF, 0xF0]);
        assert_eq!(compress(b"abcabc", MODE_LZSS).unwrap(), [0b0111, b'a', b'b', b'c', 0xEE, 0xF0]);
        let long = [vec![0; 4000], b"xyz".repeat(500)].concat();
        assert!(compress(&long, MODE_LZSS_RLE).unwrap().len() < 300);
        assert_eq!(
            compress_block(&b"A".repeat(5), MODE_LZSS_RLE).unwrap(),
            [3, 3, 0, 0, 0, 5, 0, 0, 0, 0b0, 0x41, 0x2F]
        );
        assert_eq!(fault(compress(b"abc", 2)), "unsupported compression mode 2");
    }
}
