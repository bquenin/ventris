//! Putting changed data back into a game image.
//!
//! Every game has the same question when a string, a block or a font grows: it goes where it was if
//! it still fits, and somewhere new if not, in which case the game has to be told where. Where
//! "somewhere new" is and how the game is told differ; the rest is here.

use crate::error::{Error, Result, text_error};
use crate::platforms::snes::BankAllocator;

/// The step that turns an image into the larger one that has the free room.
type Grow = Box<dyn Fn(&[u8]) -> Result<Vec<u8>>>;

/// Free room added to an image, used from its start, and the step that adds it.
///
/// `grow` turns the original image into the larger one; it runs the first time the room is needed,
/// so a build that fits in the original image leaves its size alone.
pub struct Expansion {
    pub allocator: BankAllocator,
    pub end: usize,
    grow: Grow,
}

impl Expansion {
    pub fn new(start: usize, end: usize, grow: impl Fn(&[u8]) -> Result<Vec<u8>> + 'static) -> Self {
        Expansion { allocator: BankAllocator::new(start, end), end, grow: Box::new(grow) }
    }

    /// Make sure `out` is the larger image.
    pub fn grow(&self, out: &mut Vec<u8>) -> Result<()> {
        if out.len() < self.end {
            *out = (self.grow)(out)?;
        }
        Ok(())
    }

    /// Write `data` in free room, never across a bank boundary, and return its file offset.
    pub fn store(&mut self, out: &mut Vec<u8>, data: &[u8]) -> Result<usize> {
        let Ok(target) = self.allocator.allocate(data.len()) else {
            return text_error("the expanded area is full");
        };
        self.grow(out)?;
        let Some(room) = out.get_mut(target..target + data.len()) else {
            return Err(Error::Image("the image did not grow to hold its expanded area".into()));
        };
        room.copy_from_slice(data);
        Ok(target)
    }
}

/// Write `data` over the `size` bytes at `start` if it fits, and elsewhere if it does not.
///
/// When it fits, the rest of the old place is filled with `fill` if that is given, and None is
/// returned. When it does not, the data goes to the expanded area and its offset there is returned;
/// the old place is left for the caller, who has to make the game look at the new one.
pub fn place(
    out: &mut Vec<u8>,
    start: usize,
    size: usize,
    data: &[u8],
    expansion: &mut Expansion,
    fill: Option<u8>,
) -> Result<Option<usize>> {
    if data.len() > size {
        return expansion.store(out, data).map(Some);
    }
    out[start..start + data.len()].copy_from_slice(data);
    if let Some(fill) = fill {
        out[start + data.len()..start + size].fill(fill);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image of 0x100 bytes that grows to 0x300, with free room from 0x200.
    fn expansion() -> Expansion {
        Expansion::new(0x200, 0x300, |image| Ok([image, &[0xFF; 0x200]].concat()))
    }

    #[test]
    fn what_fits_stays_where_it_was() {
        let mut out = vec![1; 0x100];
        let mut expansion = expansion();
        assert_eq!(place(&mut out, 0x10, 4, b"ab", &mut expansion, None).unwrap(), None);
        assert_eq!(out[0x10..0x14], [b'a', b'b', 1, 1]);
        assert_eq!(place(&mut out, 0x20, 4, b"ab", &mut expansion, Some(0)).unwrap(), None);
        assert_eq!(out[0x20..0x25], [b'a', b'b', 0, 0, 1]);
        assert_eq!(out.len(), 0x100);
    }

    #[test]
    fn what_outgrows_its_place_goes_to_the_expanded_area() {
        let mut out = vec![1; 0x100];
        let mut expansion = expansion();
        assert_eq!(place(&mut out, 0x10, 2, b"abc", &mut expansion, Some(0)).unwrap(), Some(0x200));
        assert_eq!(place(&mut out, 0x20, 2, b"def", &mut expansion, None).unwrap(), Some(0x203));
        // The image grew once, the data is in the new room, and the old places are left as they were.
        assert_eq!(out.len(), 0x300);
        assert_eq!(out[0x200..0x207], *b"abcdef\xff");
        assert!(out[..0x100].iter().all(|byte| *byte == 1));
        let error = expansion.store(&mut out, &[0; 0x100]).unwrap_err();
        assert!(matches!(error, Error::Text(_)));
        assert_eq!(error.to_string(), "the expanded area is full");
    }
}
