//! Room on the game's heap: how large an area's code may grow.
//!
//! The game loads an area's code onto its heap, from $8011D9D8 (the first block, past the heap's own
//! record) to $801F8000. A block takes its size rounded up to a multiple of 4, and $18 bytes more for
//! its header; the allocator ($8002B1E0) takes the first free block that is large enough, and when
//! none is, it prints a message and gives nothing, which the game does not check. Four blocks of
//! $27640 bytes in all are at the start of the heap all along (`FIXED`). What else is there, and when,
//! was measured in PCSX-Redux by following every allocation:
//!
//! - in the field of an area, its code, then the block of its map, which is 12 bytes for each tile
//!   of the map, or 2 bytes for each and $20000 more for a map that scrolls freely (a world map);
//! - while a map is loaded (an area's first, another map of the same area, or after a battle), a
//!   second block for its picture, whose size the area's header gives with the map, or, after a
//!   battle, the area's other data (the rest, see `insert`), each freed once used;
//! - in a battle, besides the area's code and its map's block, $44448 bytes at most (`BATTLE`): the
//!   same in a battle of each of the game's 255 groups of enemies, at its start, and less while it is
//!   fought;
//! - going through a door to another area, the new area's code is read while the old area's code
//!   and map are still there, and so is its rest if there is room for it; if there is not, the game
//!   reads the rest later, once the old area is gone, and nothing is lost. Then the old code and map
//!   are freed, and the new code is put where the old was if it fits there. If it does not, the game
//!   takes two more blocks of the new code's size, to move it down through them, and needs room for
//!   three copies of it, as well as its rest if that was read. Running out of room for the new code
//!   or for those copies is the end of the game: the allocator's nothing is not checked, and the game
//!   goes on with a block it never copied the code into ($8002A228).
//!
//! The doors are in the areas' events: the event that sends the player to another area pushes the
//! area's number (counted from 1, for entry 96), the map's, a place and a direction, each put in a
//! local value just before, and runs $80 (see `doors`). A door whose numbers are not put there that
//! way (33 of them, which use values the game keeps, such as the way back to a world map) is not
//! found. Area 178 is a room for testing the game, whose doors lead everywhere; its doors are left
//! out, and an area that only it leads to is checked as if the player came from the area whose door
//! into it would need the most room. Which of an area's maps a door is in is not known, so each is
//! supposed in turn.
//!
//! The game leaves itself little room in places. Going from area 107 into area 105, whose code does
//! not fit where 107's was, the three copies leave 19,636 bytes of the heap free; that is the least of
//! any moment of the game's at which running out would end it (`SPARE`). A build must leave at least
//! as much at every moment it changes, so that nothing it builds comes closer to the end of the heap
//! than the game itself does. So area 105 cannot grow, a few areas can grow by a few thousand bytes,
//! and most by tens of thousands or more (see `docs/packs.md`). All of this was seen in the emulator:
//! area 105 grown by 5,896 bytes went in from 107 with 1,948 bytes to spare, as this module counts, and
//! grown by 7,016 bytes the game stopped there; area 155 grown by 188,772 bytes went in from the world
//! map and fought a battle.

use std::collections::{BTreeMap, BTreeSet};

use super::{AREA_HEADER, half, word};

/// The heap's first block and its end.
pub const HEAP_START: usize = 0x8011_D9D8;
pub const HEAP_END: usize = 0x801F_8000;
/// The blocks at the start of the heap all along, headers included.
pub const FIXED: usize = 0x2_7640;
/// The header of a block.
pub const BLOCK_HEADER: usize = 0x18;
/// The most a battle takes, headers included, measured at the start of a battle in area 155.
pub const BATTLE: usize = 0x4_4448;
/// The least of the heap the game leaves free at a moment where running out would end it, going from
/// area 107 into area 105; a build must leave as much at every moment.
pub const SPARE: usize = 0x4CB4;
/// What an area may use of the heap at its busiest moment.
pub const ROOM: usize = HEAP_END - HEAP_START - FIXED - SPARE;
/// The room for testing the game, whose doors are left out.
pub const TEST_ROOM: usize = 178;
/// The first area, entry 96 of the table of contents, is area 1 in a door.
const FIRST_AREA: usize = 96;
/// Where the header's entries start that give each map's size and its picture's, 16 bytes each.
const MAP_ENTRIES: usize = 0x29C;
const MAP_ENTRY_COUNT: usize = 33;
/// A map with one of these flags scrolls freely, and its block is laid out otherwise.
const FREE_SCROLLING: usize = 0xC000;
/// The events' instructions: push a number, push a local value's reference, store, and go to an area.
const PUSH: i16 = 22;
const LOCAL: i16 = 23;
const STORE: i16 = 54;
const DOOR: i16 = 0x80;

/// The room a block of `size` bytes takes on the heap.
fn block(size: usize) -> usize {
    if size == 0 { 0 } else { size.next_multiple_of(4) + BLOCK_HEADER }
}

/// What an area puts on the heap, and where its doors lead.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Footprint {
    /// The size of its code (word 0 of its header), and of the rest of its data (word 10).
    pub code: usize,
    pub rest: usize,
    /// Each map's block and its picture's.
    pub maps: Vec<(usize, usize)>,
    /// The entries of the areas its doors lead to.
    pub doors: BTreeSet<usize>,
}

impl Footprint {
    /// Read from the start of an area's member, its header and its code; None where it is not laid
    /// out as an area is.
    pub fn of(member: &[u8]) -> Option<Footprint> {
        let header = |number: usize| word(member, number * 4);
        let code = header(0)?;
        let code_data = member.get(AREA_HEADER..AREA_HEADER.checked_add(code)?)?;
        // The maps: part 3 of the code starts past parts 1 and 2 (words 1 and 2), word 5 is how many
        // maps there are, and from word 11 is where each one's record is in part 3. A record's second
        // number is its entry in the header (none at 0), and its sixth its flags.
        let maps_at = header(1)?.checked_add(header(2)?)?;
        let count = header(5)?;
        if count > MAP_ENTRY_COUNT * 2 {
            return None;
        }
        let mut maps = Vec::new();
        for number in 0..count {
            let record = maps_at.checked_add(header(11 + number)?)?;
            let entry = half(code_data, record + 2)? as i16;
            let flags = half(code_data, record + 10)?;
            if entry <= 0 {
                maps.push((0, 0));
                continue;
            }
            let entry = entry as usize;
            if entry >= MAP_ENTRY_COUNT {
                return None;
            }
            let at = MAP_ENTRIES + entry * 16;
            let tiles = half(member, at)? * half(member, at + 2)?;
            let map = if flags & FREE_SCROLLING != 0 { 2 * tiles + 0x2_0000 } else { 12 * tiles };
            maps.push((map, (word(member, at + 8)? >> 8) * 4));
        }
        let doors = doors(code_data)?.into_iter().map(|(area, _)| area).collect();
        Some(Footprint { code, rest: header(10)?, maps, doors })
    }

    /// Its largest map block and its smallest.
    fn map_blocks(&self) -> (usize, usize) {
        let blocks = || self.maps.iter().map(|(map, _)| block(*map));
        (blocks().max().unwrap_or_default(), blocks().min().unwrap_or_default())
    }
}

/// The doors of an area's events, from its code: the entry of the area each leads to and the map.
/// A door to another map of the same area (area 0) is left out. None where the code is not laid out
/// as an area's is.
pub fn doors(code: &[u8]) -> Option<Vec<(usize, usize)>> {
    // Part 1 is a count and as many 16-bit numbers, padded to a multiple of 4; the events follow, a
    // count and as many 16-bit instructions.
    let events = 4 + (word(code, 0)?.checked_mul(2)?).next_multiple_of(4);
    let count = word(code, events)?;
    let at = events + 4;
    let instructions: Vec<i16> = code
        .get(at..at.checked_add(count.checked_mul(2)?)?)?
        .chunks(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut found = Vec::new();
    for (place, instruction) in instructions.iter().enumerate() {
        if *instruction != DOOR || place < 35 {
            continue;
        }
        // Five times, from the last: PUSH value, LOCAL n, STORE, LOCAL n.
        let numbers: Option<Vec<i16>> = (0..5)
            .map(|back| {
                let end = place - 7 * back;
                let part = &instructions[end - 7..end];
                (part[0] == PUSH && part[2] == LOCAL && part[4] == STORE && part[5] == LOCAL && part[3] == part[6])
                    .then_some(part[1])
            })
            .collect();
        let Some(numbers) = numbers else { continue };
        // The last pushed is the direction; before it the place, the map and, first, the area.
        let (area, map) = (numbers[4], numbers[3]);
        if area > 0 && map >= 0 {
            found.push((area as usize + FIRST_AREA - 1, map as usize));
        }
    }
    Some(found)
}

/// A moment at which a build would leave the game less of its heap than `SPARE`.
#[derive(Debug, PartialEq, Eq)]
pub struct Shortfall {
    /// The area whose code is too large for it, by entry, and the other area, for a door.
    pub area: usize,
    pub other: Option<usize>,
    /// What the moment is, in words, and how many bytes too many it needs.
    pub moment: String,
    pub over: usize,
}

/// The heap an area needs at each of its moments: going through each door into it and out of it,
/// loading each of its maps, and fighting there. Each comes with what it is, the other area for a
/// door, and the bytes it takes. `areas` is every area, by entry.
fn moments(areas: &BTreeMap<usize, Footprint>, entry: usize) -> Vec<(String, Option<usize>, usize)> {
    let here = &areas[&entry];
    let (largest_map, _) = here.map_blocks();
    let mut found = Vec::new();
    // Going from `from`, from any of its maps, to `to`: the old code and map and the new code, with the
    // new rest where there is room for it; and three copies of the new code, with the rest if it was
    // read, where the new code does not fit where the old was.
    let door = |from: &Footprint, to: &Footprint| {
        let maps = from.maps.iter().map(|(map, _)| block(*map)).chain(from.maps.is_empty().then_some(0));
        maps.map(|map| {
            let hole = block(from.code) + map;
            let read = hole + block(to.code);
            if block(to.code) <= hole {
                return read;
            }
            let rest = if read + block(to.rest) <= ROOM + SPARE { block(to.rest) } else { 0 };
            read.max(hole + 3 * block(to.code) + rest)
        })
        .max()
        .unwrap_or_default()
    };
    let testing = entry == TEST_ROOM;
    let mut from: Vec<usize> = areas
        .iter()
        .filter(|(other, footprint)| {
            !testing && **other != entry && **other != TEST_ROOM && footprint.doors.contains(&entry)
        })
        .map(|(other, _)| *other)
        .collect();
    if from.is_empty() && !testing {
        // Only the test room leads here, or no door the events show: as if from the area whose door
        // into it needs the most. That is not always the area that leaves the most behind it: a
        // smaller one can leave a hole the new code does not fit in, and so need the three copies.
        let worst =
            areas.iter().filter(|(other, _)| **other != entry).max_by_key(|(_, footprint)| door(footprint, here));
        from.extend(worst.map(|(other, _)| *other));
    }
    for other in from {
        found.push((format!("going into it from area {other:03}"), Some(other), door(&areas[&other], here)));
    }
    for other in &here.doors {
        if !testing && *other != entry && *other != TEST_ROOM && areas.contains_key(other) {
            found.push((format!("going from it into area {other:03}"), Some(*other), door(here, &areas[other])));
        }
    }
    for (number, (map, picture)) in here.maps.iter().enumerate() {
        let used = block(here.code) + block(*map) + block(*picture).max(block(here.rest));
        found.push((format!("loading its map {number}"), None, used));
    }
    found.push(("in a battle".into(), None, block(here.code) + largest_map + BATTLE));
    found
}

/// The moments of the areas in `changed` at which they would leave less of the heap free than
/// `SPARE`, and the least that any of their moments leaves free, with what that moment is. `areas`
/// is every area, by entry, as the build would have them; `reserve` is what the build takes of the
/// heap at every moment besides, such as the larger grids of the windows' text (see `vwf`).
pub fn check(
    areas: &BTreeMap<usize, Footprint>,
    changed: &BTreeSet<usize>,
    reserve: usize,
) -> (Vec<Shortfall>, Option<(usize, String)>) {
    let mut shortfalls = Vec::new();
    let mut least: Option<(usize, String)> = None;
    for &entry in changed {
        if !areas.contains_key(&entry) {
            continue;
        }
        for (moment, other, used) in moments(areas, entry) {
            let used = used + reserve;
            let free = (ROOM + SPARE).saturating_sub(used);
            if least.as_ref().is_none_or(|(least, _)| free < *least) {
                least = Some((free, format!("area {entry:03}, {moment}")));
            }
            if used > ROOM {
                shortfalls.push(Shortfall { area: entry, other, moment, over: used - ROOM });
            }
        }
    }
    (shortfalls, least)
}

#[cfg(test)]
mod tests;
