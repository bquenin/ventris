use super::super::vwf;
use super::*;
use crate::platform::Image;
use crate::platforms::ps1::Disc;

/// Instructions as the events have them, little-endian.
fn events(entries: &[u16], instructions: &[i16]) -> Vec<u8> {
    let mut out = (entries.len() as u32).to_le_bytes().to_vec();
    out.extend(entries.iter().flat_map(|entry| entry.to_le_bytes()));
    out.resize(out.len().next_multiple_of(4), 0);
    out.extend((instructions.len() as u32).to_le_bytes());
    out.extend(instructions.iter().flat_map(|instruction| instruction.to_le_bytes()));
    out
}

/// The instructions of a door to an area (counted from 1) and a map, at a place, facing a way.
fn door(area: i16, map: i16) -> Vec<i16> {
    let mut out = Vec::new();
    for (local, value) in [area, map, 10, 12, 2].into_iter().enumerate() {
        let local = local as i16 + 3;
        out.extend([PUSH, value, LOCAL, local, STORE, LOCAL, local]);
    }
    out.push(DOOR);
    out
}

#[test]
fn doors_are_read_from_the_events() {
    // A door to area 60 (entry 155), map 0, one to another map of the same area, and an instruction
    // $80 that is a number pushed.
    let mut instructions = vec![15, 2];
    instructions.extend(door(60, 0));
    instructions.extend(door(0, 3));
    instructions.extend([PUSH, DOOR, 24]);
    instructions.extend(door(82, 1));
    assert_eq!(doors(&events(&[2, 9, 30], &instructions)), Some(vec![(155, 0), (177, 1)]));
    // A door whose area is a value the game keeps is not one the events show.
    let mut kept = door(60, 0);
    kept[0..2].copy_from_slice(&[LOCAL, 40]);
    assert_eq!(doors(&events(&[], &kept)), Some(vec![]));
    assert_eq!(doors(&[1, 0, 0, 0]), None);
}

fn area(code: usize, rest: usize, maps: &[(usize, usize)], doors: &[usize]) -> Footprint {
    Footprint { code, rest, maps: maps.to_vec(), doors: doors.iter().copied().collect() }
}

#[test]
fn a_door_needs_both_areas_and_three_copies_of_a_code_that_does_not_fit_where_the_old_was() {
    let room = ROOM + SPARE;
    // From a world map into a large area: the old code and map and the new code. The new rest is read
    // after, once the old area is gone, so it does not count.
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[155]);
    let large = area(0x248D8, 0x3_9400, &[(0x3000, 0x4_0890), (0x3390, 0x2_0448)], &[177]);
    let areas = BTreeMap::from([(177, world), (155, large)]);
    let (shortfalls, least) = check(&areas, &BTreeSet::from([155]), 0);
    assert!(shortfalls.is_empty());
    let read = block(0x149C0) + block(0x4_0000) + block(0x248D8);
    assert_eq!(least, Some((room - read, "area 155, going into it from area 177".to_string())));
    // From a small area into a larger one, the new code cannot go where the old was: three copies,
    // and still not the rest.
    let small = area(0x8C14, 0x1_9CCC, &[(0x2A00, 0x1_0224)], &[154]);
    let next = area(0x2_0D88, 0x3_9894, &[(0x25F8, 0x4_0890)], &[]);
    let areas = BTreeMap::from([(153, small.clone()), (154, next.clone())]);
    let (_, least) = check(&areas, &BTreeSet::from([154]), 0);
    let three = block(0x8C14) + block(0x2A00) + 3 * block(0x2_0D88);
    let free = room - three;
    assert_eq!(least, Some((free, "area 154, going into it from area 153".to_string())));
    // Grown so that the copies come closer to the end of the heap than `SPARE`: three times the growth.
    let grow = (free - SPARE) / 3 / 4 * 4 + 4;
    let grown = BTreeMap::from([(153, small), (154, area(0x2_0D88 + grow, 0x3_9894, &next.maps, &[]))]);
    let (shortfalls, _) = check(&grown, &BTreeSet::from([154]), 0);
    let over = 3 * grow - (free - SPARE);
    assert_eq!(
        shortfalls,
        [Shortfall { area: 154, other: Some(153), moment: "going into it from area 153".into(), over }]
    );
    // Only the moments of the areas checked are.
    let (shortfalls, least) = check(&grown, &BTreeSet::new(), 0);
    assert!(shortfalls.is_empty() && least.is_none());
    // What the build takes of the heap besides counts at every moment, once.
    let reserve = free - SPARE + 4;
    let (shortfalls, least) = check(&areas, &BTreeSet::from([154]), reserve);
    assert_eq!(least, Some((SPARE - 4, "area 154, going into it from area 153".to_string())));
    assert_eq!(
        shortfalls,
        [Shortfall { area: 154, other: Some(153), moment: "going into it from area 153".into(), over: 4 }]
    );
}

#[test]
fn the_rest_is_read_after_a_door_beside_the_new_code_alone() {
    // An area of little code, no map and a large rest: its door leaves much free, and reading its rest
    // after, beside its code, is its busiest moment.
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[123]);
    let hidden = area(0x1_0000, 0x9_0000, &[], &[]);
    let areas = BTreeMap::from([(177, world), (123, hidden)]);
    let (shortfalls, least) = check(&areas, &BTreeSet::from([123]), 0);
    let used = block(0x1_0000) + block(0x9_0000);
    assert_eq!(least, Some((ROOM + SPARE - used, "area 123, reading its rest after a door into it".to_string())));
    assert!(shortfalls.is_empty());
}

#[test]
fn an_area_the_test_room_alone_leads_to_is_checked_as_if_from_the_worst_area() {
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[]);
    let test_room = area(0x2_5270, 0x2_C794, &[(0x738, 0)], &[123]);
    let hidden = area(0x1_0000, 0x2_0000, &[(0x1000, 0)], &[]);
    let areas = BTreeMap::from([(177, world), (TEST_ROOM, test_room), (123, hidden)]);
    let (_, least) = check(&areas, &BTreeSet::from([123]), 0);
    assert_eq!(least.unwrap().1, "area 123, going into it from area 177");
}

#[test]
fn with_no_door_known_into_an_area_the_worst_door_is_not_always_from_the_largest_area() {
    // The world map leaves a hole the new code fits in; the small area leaves one it does not, so
    // going from there needs the three copies, which is more than the heap has room for.
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[]);
    let small = area(0x8C14, 0x1_9CCC, &[(0x2A00, 0x1_0224)], &[]);
    let hidden = area(0x4_0000, 0x1000, &[(0x1000, 0)], &[]);
    let areas = BTreeMap::from([(177, world), (153, small), (123, hidden)]);
    let (shortfalls, _) = check(&areas, &BTreeSet::from([123]), 0);
    let used = block(0x8C14) + block(0x2A00) + 3 * block(0x4_0000);
    assert_eq!(
        shortfalls,
        [Shortfall { area: 123, other: Some(153), moment: "going into it from area 153".into(), over: used - ROOM }]
    );
}

// --- with the game ---------------------------------------------------------

fn game() -> Option<Image> {
    Some(Image::Ps1Disc(Disc::open(&crate::testing::game("god-ps1")?).unwrap()))
}

/// How far an area's code can grow, by multiples of 4, before a moment of it leaves less than
/// `SPARE` free, with `reserve` taken at every moment, if no other area grows.
fn growth(areas: &BTreeMap<usize, Footprint>, entry: usize, reserve: usize) -> usize {
    let fits = |grow: usize| {
        let mut grown = areas.clone();
        grown.get_mut(&entry).unwrap().code += grow;
        check(&grown, &BTreeSet::from([entry]), reserve).0.is_empty()
    };
    let (mut fit, mut over) = (0, 0x10_0000);
    while over - fit > 4 {
        let middle = (fit + over) / 8 * 4;
        if fits(middle) { fit = middle } else { over = middle }
    }
    fit
}

#[test]
fn with_the_rest_read_after_a_door_every_area_has_room_to_grow() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let areas = super::super::insert::footprints(disc).unwrap();
    assert_eq!(areas.len(), 93);
    assert_eq!(areas[&177].maps, [(0x4_0000, 0x4_0890)]);
    assert_eq!((areas[&155].code, areas[&155].rest), (0x2_48D8, 0x3_9400));
    // The doors of area 096: within it, to area 097 and to the test room.
    assert_eq!(areas[&96].doors, BTreeSet::from([97, TEST_ROOM]));
    assert!(areas[&177].doors.len() > 50);
    // As measured in the emulator, the three copies of area 105's code going into it from area 107
    // leave 19,636 bytes free in the original game, which reads its rest of 229,412 bytes in advance:
    // read after, they leave both.
    assert_eq!(block(areas[&105].rest), 229_412);
    let door = moments(&areas, 105).into_iter().find(|(moment, ..)| moment == "going into it from area 107");
    assert_eq!(door.map(|(.., used)| ROOM + SPARE - used), Some(SPARE + 229_412));
    // Every moment of every area leaves more than `SPARE` free, with the renderer's grids too.
    let every: BTreeSet<usize> = areas.keys().copied().collect();
    let (shortfalls, least) = check(&areas, &every, vwf::GRIDS);
    assert!(shortfalls.is_empty(), "{shortfalls:?}");
    assert_eq!(least, Some((105_200, "area 177, in a battle".to_string())));
    // How far the areas that could not grow, or barely, can grow now, the renderer's grids counted,
    // and the least any area can.
    let grows: Vec<usize> = [105, 107, 131, 179].iter().map(|entry| growth(&areas, *entry, vwf::GRIDS)).collect();
    assert_eq!(grows, [75_892, 121_164, 106_520, 77_944]);
    let least = every
        .iter()
        .filter(|entry| **entry != TEST_ROOM)
        .map(|entry| (growth(&areas, *entry, vwf::GRIDS), *entry))
        .min();
    assert_eq!(least, Some((52_324, 157)));
}

#[test]
fn the_door_patch_makes_the_call_give_no_block_and_refuses_another_file() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let mut program = super::super::program::program_file(disc).unwrap().to_vec();
    assert_eq!(door_patch(&program).unwrap(), (0x1A738, vec![0x21, 0x10, 0, 0]));
    program[0x1A738] ^= 1;
    let refused = door_patch(&program).err().unwrap().to_string();
    assert!(refused.contains("$80029F38"), "{refused}");
}
