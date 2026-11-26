//! TIM, the PlayStation's own format for a picture that is to be copied into video memory.
//!
//! A TIM starts with the number 0x10 and a word of flags: the low 3 bits say how many bits a pixel
//! takes (0 for 4, 1 for 8, 2 for 16, 3 for 24), and bit 3 says that a colour table, a CLUT,
//! comes first. The CLUT, when there is one, and then the pixels are each a block: the block's
//! length in bytes (with its own 12 bytes of header), where it goes in video memory as x and y,
//! and its width and height, then the data. Video memory is counted in 16-bit units, so the width
//! of the pixel block is in those: a 4-bit picture 256 pixels wide is 64 units wide. Within a byte
//! of a 4-bit picture, the low half is the pixel on the left.
//!
//! Games keep fonts this way, often several in one picture: a 4-bit pixel holds four 1-bit ones.

use crate::error::{Error, Result};

/// A picture read from a TIM: what an adapter needs to read its pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tim {
    /// Bits a pixel takes: 4, 8, 16 or 24.
    pub depth: usize,
    /// In pixels.
    pub width: usize,
    pub height: usize,
    /// Where the pixels go in video memory, in 16-bit units across and rows down.
    pub x: u16,
    pub y: u16,
    /// The pixel block's data, rows one after another.
    pub pixels: Vec<u8>,
    /// Where that data starts in the TIM's bytes, so that a changed picture can be written back.
    pub pixels_at: usize,
}

fn tim_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

/// A little-endian number of `N` bytes at `at`, if the data is long enough.
fn number<const N: usize>(data: &[u8], at: usize) -> Option<usize> {
    let bytes: [u8; N] = data.get(at..at.checked_add(N)?)?.try_into().ok()?;
    Some(bytes.iter().rev().fold(0, |value, byte| value << 8 | usize::from(*byte)))
}

/// Read a TIM. It fails with `Error::Image` where the data is not one, or is cut short.
pub fn read(data: &[u8]) -> Result<Tim> {
    if number::<4>(data, 0) != Some(0x10) {
        return tim_error("the data is not a TIM picture: it does not start with the number 16");
    }
    let Some(flags) = number::<4>(data, 4) else {
        return tim_error("the TIM picture is cut short");
    };
    let depth = match flags & 7 {
        0 => 4,
        1 => 8,
        2 => 16,
        3 => 24,
        _ => return tim_error(format!("the TIM picture gives an unknown depth of pixel, {}", flags & 7)),
    };
    let mut at = 8;
    if flags & 8 != 0 {
        // The colour table is not needed to read which pixel has which value.
        match number::<4>(data, at) {
            Some(length) if length >= 12 => at += length,
            _ => return tim_error("the TIM picture's colour table is cut short"),
        }
    }
    let header = (number::<4>(data, at), number::<2>(data, at + 4), number::<2>(data, at + 6));
    let size = (number::<2>(data, at + 8), number::<2>(data, at + 10));
    let ((Some(length), Some(x), Some(y)), (Some(units), Some(height))) = (header, size) else {
        return tim_error("the TIM picture's pixels are cut short");
    };
    let bytes = units * 2 * height;
    if length < 12 + bytes || data.len() < at + 12 + bytes {
        return tim_error("the TIM picture's pixels are cut short");
    }
    Ok(Tim {
        depth,
        width: units * 16 / depth,
        height,
        x: x as u16,
        y: y as u16,
        pixels: data[at + 12..at + 12 + bytes].to_vec(),
        pixels_at: at + 12,
    })
}

impl Tim {
    /// The value of the pixel at `x`, `y` in a picture of 4 or 8 bits a pixel, or `None` outside it
    /// or in a picture of another depth.
    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let row = y * self.width * self.depth / 8;
        match self.depth {
            4 => {
                let byte = self.pixels[row + x / 2];
                Some(if x & 1 == 1 { byte >> 4 } else { byte & 0x0F })
            }
            8 => Some(self.pixels[row + x]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TIM of 4 bits a pixel, 4 pixels wide and 2 high, with a colour table of 16 colours.
    fn small() -> Vec<u8> {
        let mut data = vec![0x10, 0, 0, 0, 8, 0, 0, 0];
        data.extend([12 + 32, 0, 0, 0, 0, 0, 0xF0, 0, 16, 0, 1, 0]);
        data.extend([0; 32]);
        data.extend([12 + 4, 0, 0, 0, 0x40, 0, 0x80, 0, 1, 0, 2, 0]);
        data.extend([0x21, 0x43, 0x65, 0x87]);
        data
    }

    #[test]
    fn a_four_bit_picture_reads_left_pixel_first() {
        let tim = read(&small()).unwrap();
        assert_eq!((tim.depth, tim.width, tim.height, tim.x, tim.y), (4, 4, 2, 0x40, 0x80));
        let rows: Vec<Vec<u8>> = (0..2).map(|y| (0..4).map(|x| tim.pixel(x, y).unwrap()).collect()).collect();
        assert_eq!(rows, [[1, 2, 3, 4], [5, 6, 7, 8]]);
        assert_eq!((tim.pixel(4, 0), tim.pixel(0, 2)), (None, None));
    }

    #[test]
    fn what_is_not_a_whole_tim_is_refused() {
        let data = small();
        assert!(read(&data[..data.len() - 1]).is_err());
        assert!(read(&data[..20]).is_err());
        assert!(read(&[0x11, 0, 0, 0, 8, 0, 0, 0]).is_err());
        let mut odd = data.clone();
        odd[4] = 5;
        assert!(read(&odd).is_err());
    }
}
