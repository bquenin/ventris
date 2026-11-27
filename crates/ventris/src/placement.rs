//! Putting changed data back into a game image.
//!
//! Every game has the same question when a string, a block or a font grows: it goes where it was if
//! it still fits, and somewhere new if not, in which case the game has to be told where. Where
//! "somewhere new" is and how the game is told differ; the rest is here.

use std::cmp::Reverse;
use std::ops::Range;

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

/// What `share_room` answers: the stretches, the offset each string is put at, or None for one no
/// stretch had a place for, and the size of the largest piece left unused.
pub type Shared = (Vec<Range<usize>>, Vec<Option<usize>>, usize);

/// Where changed strings that can move are put, when the strings a pack changes share the room they
/// had between them, as a program's own text does when the program cannot grow: `strings` gives
/// each as its offset, the bytes that were its own there, the bytes it needs now, both counting the
/// zero byte that ends it, and the offsets it may be put at. Rooms side by side make one stretch.
/// Each string first goes into its own stretch, in the order they were in and with no gaps, so that
/// what a stretch has left is in one piece at its end; a string its stretch has no place for then
/// goes into the first stretch that has, the longest such string first (see `Free` for where in a
/// stretch).
///
/// `spare` is room that was no string's, such as what a module has past its end. It is one more
/// stretch, the last, and is used when the others have no place.
///
/// A string that needs nothing (the same text as another, written once) is given the place its
/// stretch has reached, and takes no room there.
pub fn share_room(strings: &[(usize, usize, usize, Range<usize>)], spare: Option<Range<usize>>) -> Shared {
    let (order, stretches, stretch_of) = stretches_of(strings, spare);
    let mut free = Free::new(&stretches);
    let mut places = vec![None; strings.len()];
    let mut left = Vec::new();
    for index in &order {
        let (need, may, stretch) = (strings[*index].2, &strings[*index].3, stretch_of[*index]);
        let found = free.starts(stretch, need, may, false).next();
        match found {
            Some((piece, start)) => places[*index] = Some(free.take(stretch, piece, start, need)),
            None => left.push(*index),
        }
    }
    left.sort_by_key(|index| (Reverse(strings[*index].2), strings[*index].0));
    for index in left {
        let (need, may) = (strings[index].2, &strings[index].3);
        let found = [false, true].into_iter().find_map(|gap| {
            (0..stretches.len()).find_map(|stretch| Some((stretch, free.starts(stretch, need, may, gap).next()?)))
        });
        if let Some((stretch, (piece, start))) = found {
            places[index] = Some(free.take(stretch, piece, start, need));
        }
    }
    let largest = free.largest();
    (stretches, places, largest)
}

/// The free room of each stretch, as pieces in the order of the file. A stretch starts as one
/// piece, and a string is put at the start of a piece, so that while strings go in one after
/// another a stretch keeps its free room in one piece at its end. A string that may not start that
/// soon (its address built by code that reaches only so far) can be put further on in a piece, but
/// only where no stretch has a place at the start of one: the bytes it skips stay free, a piece of
/// their own, so that no free byte is lost.
struct Free {
    pieces: Vec<Vec<Range<usize>>>,
    ends: Vec<usize>,
}

impl Free {
    fn new(stretches: &[Range<usize>]) -> Self {
        let pieces = stretches.iter().map(|stretch| vec![stretch.clone()]).collect();
        Free { pieces, ends: stretches.iter().map(|stretch| stretch.end).collect() }
    }

    /// Where in `stretch` a string of `need` bytes that may start in `may` can go: each piece it fits
    /// in, with the offset it would start at there, at the piece's start, or with `gap`, as soon in
    /// the piece as it may start.
    fn starts(
        &self,
        stretch: usize,
        need: usize,
        may: &Range<usize>,
        gap: bool,
    ) -> impl Iterator<Item = (usize, usize)> {
        self.pieces[stretch].iter().enumerate().filter_map(move |(index, piece)| {
            let start = if gap { piece.start.max(may.start) } else { piece.start };
            (may.contains(&start) && start + need <= piece.end).then_some((index, start))
        })
    }

    /// Take the `need` bytes from `start` in a piece of `stretch`, and give `start` back.
    fn take(&mut self, stretch: usize, piece: usize, start: usize, need: usize) -> usize {
        let pieces = &mut self.pieces[stretch];
        let Range { start: first, end } = pieces[piece].clone();
        let kept: Vec<Range<usize>> =
            [first..start, start + need..end].into_iter().filter(|kept| !kept.is_empty()).collect();
        pieces.splice(piece..piece + 1, kept);
        start
    }

    /// Where `stretch` is filled to: the start of its last piece, if that reaches its end.
    fn reached(&self, stretch: usize) -> usize {
        let end = self.ends[stretch];
        self.pieces[stretch].last().filter(|piece| piece.end == end).map_or(end, |piece| piece.start)
    }

    /// The size of the largest piece.
    fn largest(&self) -> usize {
        self.pieces.iter().flatten().map(|piece| piece.len()).max().unwrap_or(0)
    }
}

/// The strings of `share_room` in the order of their offsets, the stretches their rooms make, the
/// spare room last, and the stretch each string's room is in.
fn stretches_of(
    strings: &[(usize, usize, usize, Range<usize>)],
    spare: Option<Range<usize>>,
) -> (Vec<usize>, Vec<Range<usize>>, Vec<usize>) {
    let mut order: Vec<usize> = (0..strings.len()).collect();
    order.sort_by_key(|index| strings[*index].0);
    let mut stretches: Vec<Range<usize>> = Vec::new();
    let mut stretch_of = vec![0; strings.len()];
    for index in &order {
        let (offset, room, ..) = strings[*index];
        match stretches.last_mut() {
            Some(stretch) if stretch.end == offset => stretch.end += room,
            _ => stretches.push(offset..offset + room),
        }
        stretch_of[*index] = stretches.len() - 1;
    }
    stretches.extend(spare.filter(|spare| !spare.is_empty()));
    (order, stretches, stretch_of)
}

/// The same as `share_room`, packed tighter, for when that leaves a string without a place: the
/// longest string first, each goes into the piece it leaves the least room in. The strings no
/// longer keep their order, and a stretch's free room may end up in several pieces, but scattered
/// rooms of a few strings are used far better: three names, of 4, 6 and 7 bytes, give room to
/// names of 7, 4 and 2 bytes this way, and not the other.
pub fn pack_room(strings: &[(usize, usize, usize, Range<usize>)], spare: Option<Range<usize>>) -> Shared {
    let (order, stretches, stretch_of) = stretches_of(strings, spare);
    let mut free = Free::new(&stretches);
    let mut places = vec![None; strings.len()];
    let mut by_need = order.clone();
    by_need.sort_by_key(|index| (Reverse(strings[*index].2), strings[*index].0));
    for index in by_need {
        let (need, may) = (strings[index].2, &strings[index].3);
        if need == 0 {
            places[index] = Some(free.reached(stretch_of[index]));
            continue;
        }
        let best = [false, true].into_iter().find_map(|gap| {
            let starts = (0..stretches.len()).flat_map(|stretch| {
                free.starts(stretch, need, may, gap).map(move |(piece, start)| (stretch, piece, start))
            });
            starts.min_by_key(|(stretch, piece, start)| free.pieces[*stretch][*piece].end - start)
        });
        if let Some((stretch, piece, start)) = best {
            places[index] = Some(free.take(stretch, piece, start, need));
        }
    }
    let largest = free.largest();
    (stretches, places, largest)
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

    const ANY: Range<usize> = 0..usize::MAX;

    #[test]
    fn packed_tighter_scattered_rooms_take_strings_their_own_order_leaves_out() {
        // Three rooms apart, of 4, 6 and 7 bytes, and strings that need 7, 4 and 2.
        let strings = [(100, 4, 7, ANY), (200, 6, 4, ANY), (300, 7, 2, ANY)];
        assert_eq!(share_room(&strings, None).1, [None, Some(200), Some(300)]);
        let (stretches, places, largest) = pack_room(&strings, None);
        assert_eq!(stretches, [100..104, 200..206, 300..307]);
        assert_eq!(places, [Some(300), Some(100), Some(200)]);
        assert_eq!(largest, 4);
        // A string that needs nothing takes no room; one with nowhere to go is left out.
        let strings = [(100, 4, 0, ANY), (200, 6, 8, ANY)];
        assert_eq!(pack_room(&strings, None).1, [Some(100), None]);
        // Where a string may go is kept to.
        let strings = [(100, 8, 3, ANY), (200, 4, 3, 0..150)];
        assert_eq!(pack_room(&strings, None).1, [Some(200), Some(100)]);
    }

    #[test]
    fn a_string_that_may_not_start_where_a_stretch_is_filled_to_goes_further_on_in_it() {
        // One stretch of 16 bytes: the second string may not start before 108, where the first leaves
        // the stretch filled to 104. It goes at 108, and the 4 bytes between are left unused.
        let strings = [(100, 8, 4, ANY), (108, 8, 8, 108..200)];
        assert_eq!(share_room(&strings, None).1, [Some(100), Some(108)]);
        let strings = [(100, 8, 8, 104..200), (108, 8, 4, ANY)];
        assert_eq!(pack_room(&strings, None).1, [Some(104), Some(100)]);
        // A stretch it can go in with no gap comes first.
        let strings = [(100, 8, 4, ANY), (108, 8, 8, 108..300)];
        assert_eq!(share_room(&strings, Some(200..216)).1, [Some(100), Some(200)]);
        // The bytes such a string skips stay free: the third string goes in the 7 bytes between the
        // first and the second, its own room being too small.
        let strings = [(100, 8, 1, ANY), (108, 8, 8, 108..300), (200, 2, 4, ANY)];
        let (_, places, largest) = share_room(&strings, None);
        assert_eq!((places, largest), (vec![Some(100), Some(108), Some(101)], 3));
        assert_eq!(pack_room(&strings, None).1, [Some(200), Some(108), Some(100)]);
    }
}
