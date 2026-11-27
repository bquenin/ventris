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
    // then only if there is room for it, so it does not count.
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[155]);
    let large = area(0x248D8, 0x3_9400, &[(0x3000, 0x4_0890), (0x3390, 0x2_0448)], &[177]);
    let areas = BTreeMap::from([(177, world), (155, large)]);
    let (shortfalls, least) = check(&areas, &BTreeSet::from([155]));
    assert!(shortfalls.is_empty());
    let read = block(0x149C0) + block(0x4_0000) + block(0x248D8);
    assert_eq!(least, Some((room - read, "area 155, going into it from area 177".to_string())));
    // From a small area into a larger one, the new code cannot go where the old was: three copies,
    // and the rest, which there was room to read.
    let small = area(0x8C14, 0x1_9CCC, &[(0x2A00, 0x1_0224)], &[154]);
    let next = area(0x2_0D88, 0x3_9894, &[(0x25F8, 0x4_0890)], &[]);
    let areas = BTreeMap::from([(153, small.clone()), (154, next.clone())]);
    let (_, least) = check(&areas, &BTreeSet::from([154]));
    let three = block(0x8C14) + block(0x2A00) + 3 * block(0x2_0D88) + block(0x3_9894);
    let free = room - three;
    assert_eq!(least, Some((free, "area 154, going into it from area 153".to_string())));
    // Grown so that the copies come closer to the end of the heap than `SPARE`: three times the growth.
    let grow = (free - SPARE) / 3 / 4 * 4 + 4;
    let grown = BTreeMap::from([(153, small), (154, area(0x2_0D88 + grow, 0x3_9894, &next.maps, &[]))]);
    let (shortfalls, _) = check(&grown, &BTreeSet::from([154]));
    let over = 3 * grow - (free - SPARE);
    assert_eq!(
        shortfalls,
        [Shortfall { area: 154, other: Some(153), moment: "going into it from area 153".into(), over }]
    );
    // Only the moments of the areas that changed are checked.
    let (shortfalls, least) = check(&grown, &BTreeSet::new());
    assert!(shortfalls.is_empty() && least.is_none());
}

#[test]
fn an_area_the_test_room_alone_leads_to_is_checked_as_if_from_the_worst_area() {
    let world = area(0x149C0, 0x1E2FC, &[(0x4_0000, 0x4_0890)], &[]);
    let test_room = area(0x2_5270, 0x2_C794, &[(0x738, 0)], &[123]);
    let hidden = area(0x1_0000, 0x2_0000, &[(0x1000, 0)], &[]);
    let areas = BTreeMap::from([(177, world), (TEST_ROOM, test_room), (123, hidden)]);
    let (_, least) = check(&areas, &BTreeSet::from([123]));
    assert_eq!(least.unwrap().1, "area 123, going into it from area 177");
}

// --- with the game ---------------------------------------------------------

fn game() -> Option<Image> {
    Some(Image::Ps1Disc(Disc::open(&crate::testing::game("god-ps1")?).unwrap()))
}

#[test]
fn the_game_leaves_itself_the_least_room_going_into_area_105_from_area_107() {
    let Some(image) = game() else { return };
    let disc = image.ps1_disc().unwrap();
    let areas = super::super::insert::footprints(disc).unwrap();
    assert_eq!(areas.len(), 93);
    // As measured in the emulator, the three copies of area 105's code going into it from area 107
    // leave 19,636 bytes free; every other moment of every area leaves more.
    let every: BTreeSet<usize> = areas.keys().copied().collect();
    let (shortfalls, least) = check(&areas, &every);
    assert!(shortfalls.is_empty(), "{shortfalls:?}");
    assert_eq!(least, Some((SPARE, "area 105, going into it from area 107".to_string())));
    assert_eq!(areas[&177].maps, [(0x4_0000, 0x4_0890)]);
    assert_eq!((areas[&155].code, areas[&155].rest), (0x2_48D8, 0x3_9400));
    // The doors of area 096: within it, to area 097 and to the test room.
    assert_eq!(areas[&96].doors, BTreeSet::from([97, TEST_ROOM]));
    assert!(areas[&177].doors.len() > 50);
}
