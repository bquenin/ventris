//! A PlayStation program, in the PS-X EXE format, and putting a patch's code into one.
//!
//! The file starts with a header of $800 bytes: `PS-X EXE`, then, from offset $10, the address the
//! program starts at, its global pointer, the address its code and data are loaded at and their
//! size, which is what follows the header in the file. The console's BIOS loads that much and no
//! more, and clears nothing; the program clears its own uninitialised data when it starts.
//!
//! Code a patch adds needs a place in memory that the game never uses, and a way to get there: the
//! file itself cannot grow on a disc whose next file starts right after it, and its bytes are its
//! code and data. `install` takes a run of zero bytes in the file that the game only ever uses as
//! memory to work in, a buffer or variables it clears or sets before reading them, and one call in
//! the game's code made before that memory is first used (a call early in its start-up). The call is
//! sent to a loader kept in the zero bytes, which copies the patch's code to its place, sets the
//! zero bytes back to zero, has the BIOS flush the instruction cache, and makes the call the game
//! made. So the game finds its memory as it left it, and the code is where the patch put it.
//! Where that place is, and which bytes are free, are measured for each game.

use std::ops::Range;

use crate::error::{Error, Result};
use crate::platforms::asm_mips::assemble;

/// The header before what the console loads.
pub const HEADER: usize = 0x800;
const MAGIC: &[u8] = b"PS-X EXE";
/// The loader's room at the start of the zero bytes, and the part of the copied code that finishes
/// the loading, before the patch's own.
pub const LOADER_SIZE: u32 = 0x40;
pub const FINISH_SIZE: u32 = 0x50;

fn exe_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Image(message.into()))
}

fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Where a program's code and data are loaded, and how many bytes.
pub fn loaded(exe: &[u8]) -> Result<Range<u32>> {
    if !exe.starts_with(MAGIC) {
        return exe_error("the program is not a PS-X EXE");
    }
    let (Some(address), Some(size)) = (word(exe, 0x18), word(exe, 0x1C)) else {
        return exe_error("the program's header is cut short");
    };
    if HEADER + size as usize > exe.len() {
        return exe_error("the program's file is shorter than its header says");
    }
    match address.checked_add(size) {
        Some(end) => Ok(address..end),
        None => exe_error("the program's header loads it past the end of memory"),
    }
}

/// The offset in the file of the byte loaded at `address`, through `length` bytes.
pub fn offset(exe: &[u8], address: u32, length: usize) -> Result<usize> {
    let loaded = loaded(exe)?;
    let end = u64::from(address) + length as u64;
    if !loaded.contains(&address) || end > u64::from(loaded.end) {
        return exe_error(format!("${address:08X} is not in what the program loads"));
    }
    Ok(HEADER + (address - loaded.start) as usize)
}

/// Assembled code as changes to the program's file: each segment at the offset of its address.
pub fn code_patches(exe: &[u8], segments: &[(u32, Vec<u8>)]) -> Result<Vec<(usize, Vec<u8>)>> {
    segments.iter().map(|(address, code)| Ok((offset(exe, *address, code.len())?, code.clone()))).collect()
}

/// Where a patch's code is kept in the file, where it goes in memory, and when it is put there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The address of a `jal` in the game's start-up, made before `room` is used: the loader takes
    /// its place, and makes the call itself once it is done.
    pub hook: u32,
    /// Zero bytes in the file, by the addresses they are loaded at, which hold the loader and the
    /// code until the loader runs, and are zero again after.
    pub room: Range<u32>,
    /// Where in memory the code goes: memory the game never uses.
    pub destination: u32,
}

impl Placement {
    /// Where the patch's own code starts in memory: it is to be assembled for that address.
    pub fn code_address(&self) -> u32 {
        self.destination + FINISH_SIZE
    }

    /// The most bytes of code the room keeps.
    pub fn capacity(&self) -> u32 {
        (self.room.end - self.room.start).saturating_sub(LOADER_SIZE + FINISH_SIZE)
    }
}

/// The changes to the program's file that install `code`, assembled for `placement.code_address()`
/// (see the module's comment): the hook sent to the loader, and the loader and the code in the room.
pub fn install(exe: &[u8], placement: &Placement, code: &[u8]) -> Result<Vec<(usize, Vec<u8>)>> {
    let Placement { hook, room, destination } = placement;
    let hook_at = offset(exe, *hook, 4)?;
    let call = word(exe, hook_at).unwrap_or_default();
    if call >> 26 != 3 {
        return exe_error(format!("the program has no call at ${hook:08X} for the loader to take the place of"));
    }
    let called = (hook & 0xF000_0000) | (call & 0x03FF_FFFF) << 2;
    if room.start % 4 != 0 || room.end % 4 != 0 || room.end <= room.start {
        return exe_error("the room for the code is not a run of whole words");
    }
    let room_at = offset(exe, room.start, (room.end - room.start) as usize)?;
    if exe[room_at..room_at + (room.end - room.start) as usize].iter().any(|byte| *byte != 0) {
        return exe_error(format!("the program's bytes at ${:08X} to ${:08X} are not zero", room.start, room.end));
    }
    if room.end - room.start < LOADER_SIZE + FINISH_SIZE {
        return exe_error(format!(
            "the room at ${:08X} keeps {} bytes, and the loader and the end of the loading take {}",
            room.start,
            room.end - room.start,
            LOADER_SIZE + FINISH_SIZE
        ));
    }
    if code.len() as u32 > placement.capacity() {
        return exe_error(format!(
            "the code takes {} bytes, and the room at ${:08X} keeps {}",
            code.len(),
            room.start,
            placement.capacity()
        ));
    }
    let finish = assemble(
        &format!(
            "org {destination}
                la    t0, {start}
                la    t1, {end}
            .zero:
                sw    zero, 0(t0)
                addiu t0, t0, 4
                bne   t0, t1, .zero
                nop
                addiu sp, sp, -8
                sw    ra, 4(sp)
                li    t2, $A0           ; the BIOS's table A, whose function $44 flushes the cache
                jalr  t2
                li    t1, $44
                lw    ra, 4(sp)
                addiu sp, sp, 8
                j     {called}
                nop",
            start = room.start,
            end = room.end,
        ),
        &[],
    )?;
    let mut payload = finish.segments[0].1.clone();
    assert!(payload.len() as u32 <= FINISH_SIZE, "the end of the loading outgrew its room");
    payload.resize(FINISH_SIZE as usize, 0);
    payload.extend(code);
    payload.resize(payload.len().next_multiple_of(4), 0);
    let loader = assemble(
        &format!(
            "org {start}
                la    t0, {source}
                la    t1, {destination}
                li    t2, {words}
            .copy:
                lw    t3, 0(t0)
                addiu t0, t0, 4
                addiu t2, t2, -1
                sw    t3, 0(t1)
                bne   t2, zero, .copy
                addiu t1, t1, 4
                j     {destination}
                nop",
            start = room.start,
            source = room.start + LOADER_SIZE,
            words = payload.len() / 4,
        ),
        &[],
    )?;
    let mut kept = loader.segments[0].1.clone();
    assert!(kept.len() as u32 <= LOADER_SIZE, "the loader outgrew its room");
    kept.resize(LOADER_SIZE as usize, 0);
    kept.extend(payload);
    assert!(kept.len() as u32 <= room.end - room.start, "what the room keeps outgrew it");
    let jal = (3u32 << 26 | (room.start >> 2 & 0x03FF_FFFF)).to_le_bytes().to_vec();
    Ok(vec![(hook_at, jal), (room_at, kept)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program loaded at $80010000, with `size` bytes of zeros, a `jal $80010100` at $80010010.
    fn program(size: usize) -> Vec<u8> {
        let mut exe = vec![0; HEADER + size];
        exe[..8].copy_from_slice(MAGIC);
        exe[0x18..0x1C].copy_from_slice(&0x8001_0000u32.to_le_bytes());
        exe[0x1C..0x20].copy_from_slice(&(size as u32).to_le_bytes());
        exe[HEADER + 0x10..HEADER + 0x14].copy_from_slice(&0x0C00_4040u32.to_le_bytes());
        exe
    }

    fn words(data: &[u8]) -> Vec<u32> {
        data.chunks(4).map(|word| u32::from_le_bytes(word.try_into().unwrap())).collect()
    }

    #[test]
    fn an_address_is_found_in_the_file_past_the_header() {
        let exe = program(0x1000);
        assert_eq!(loaded(&exe).unwrap(), 0x8001_0000..0x8001_1000);
        assert_eq!(offset(&exe, 0x8001_0010, 4).unwrap(), 0x810);
        assert_eq!(offset(&exe, 0x8001_0FFC, 4).unwrap(), 0x17FC);
        assert_eq!(offset(&exe, 0x8001_0FFE, 4).unwrap_err().to_string(), "$80010FFE is not in what the program loads");
        assert!(offset(&exe, 0x8000_FFFC, 4).is_err());
        assert_eq!(code_patches(&exe, &[(0x8001_0020, vec![1, 2])]).unwrap(), [(0x820, vec![1, 2])]);
        assert_eq!(loaded(b"PS-X EXF").unwrap_err().to_string(), "the program is not a PS-X EXE");
        // A header whose load would end past the last address is refused.
        let mut past = program(0x1000);
        past[0x18..0x1C].copy_from_slice(&0xFFFF_F800u32.to_le_bytes());
        assert_eq!(loaded(&past).unwrap_err().to_string(), "the program's header loads it past the end of memory");
    }

    #[test]
    fn the_loader_copies_the_code_clears_its_room_and_makes_the_call_it_took() {
        let exe = program(0x1000);
        let placement = Placement { hook: 0x8001_0010, room: 0x8001_0800..0x8001_0900, destination: 0x801F_8000 };
        assert_eq!(placement.code_address(), 0x801F_8050);
        let code = [0xAA; 8];
        let patches = install(&exe, &placement, &code).unwrap();
        assert_eq!(patches[0], (0x810, 0x0C00_4200u32.to_le_bytes().to_vec())); // jal $80010800
        let (at, kept) = &patches[1];
        assert_eq!(*at, 0x1000);
        assert_eq!(kept.len(), (LOADER_SIZE + FINISH_SIZE) as usize + 8);
        let loader = words(&kept[..LOADER_SIZE as usize]);
        // It copies 22 words from $80010840 to $801F8000, then jumps there.
        assert_eq!(&loader[..5], [0x3C08_8001, 0x2508_0840, 0x3C09_8020, 0x2529_8000, 0x240A_0016]);
        assert_eq!(loader[11], 0x0807_E000);
        let finish = words(&kept[LOADER_SIZE as usize..(LOADER_SIZE + FINISH_SIZE) as usize]);
        // It clears $80010800 to $80010900, then calls the BIOS and jumps to $80010100.
        assert_eq!(&finish[..4], [0x3C08_8001, 0x2508_0800, 0x3C09_8001, 0x2529_0900]);
        assert_eq!(finish[15], 0x0800_4040);
        assert_eq!(kept[(LOADER_SIZE + FINISH_SIZE) as usize..], code);
    }

    #[test]
    fn what_cannot_be_installed_is_refused() {
        let mut exe = program(0x1000);
        let placement = Placement { hook: 0x8001_0010, room: 0x8001_0800..0x8001_0900, destination: 0x801F_8000 };
        assert_eq!(
            install(&exe, &placement, &[0; 0x80]).unwrap_err().to_string(),
            "the code takes 128 bytes, and the room at $80010800 keeps 112"
        );
        // A room too small for the loader and the end of the loading, even with no code.
        let small = Placement { room: 0x8001_0800..0x8001_0840, ..placement.clone() };
        assert_eq!(
            install(&exe, &small, &[]).unwrap_err().to_string(),
            "the room at $80010800 keeps 64 bytes, and the loader and the end of the loading take 144"
        );
        let elsewhere = Placement { hook: 0x8001_0014, ..placement.clone() };
        assert_eq!(
            install(&exe, &elsewhere, &[]).unwrap_err().to_string(),
            "the program has no call at $80010014 for the loader to take the place of"
        );
        exe[HEADER + 0x880] = 1;
        assert_eq!(
            install(&exe, &placement, &[]).unwrap_err().to_string(),
            "the program's bytes at $80010800 to $80010900 are not zero"
        );
    }
}
