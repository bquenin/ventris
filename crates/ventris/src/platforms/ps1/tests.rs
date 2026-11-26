use std::path::{Path, PathBuf};

use super::*;

/// Where a sector is, as its header gives it: minutes, seconds and frames in binary-coded decimal,
/// counted from 00:02:00 at sector 0, then the mode.
fn header(sector: u32) -> [u8; 4] {
    let frames = sector + 150;
    let bcd = |number: u32| (((number / 10) << 4) | (number % 10)) as u8;
    [bcd(frames / 4500), bcd(frames / 75 % 60), bcd(frames % 75), 2]
}

/// A whole Mode 2 sector holding `data`, sealed: Form 1 for data, Form 2 for a movie's or a sound's.
fn whole(sector: u32, form2: bool, data: &[u8]) -> Vec<u8> {
    let mut whole = vec![0; RAW_SECTOR];
    whole[..12].copy_from_slice(&SYNC);
    whole[12..16].copy_from_slice(&header(sector));
    let submode = if form2 { FORM2 | 0x04 } else { 0x08 };
    whole[16..24].copy_from_slice(&[1, 0, submode, 0, 1, 0, submode, 0]);
    whole[DATA..DATA + data.len()].copy_from_slice(data);
    seal(&mut whole);
    whole
}

/// One entry of an ISO 9660 directory, with the 14 bytes CD-ROM XA adds after the name.
fn record(name: &[u8], sector: u32, size: u32, directory: bool) -> Vec<u8> {
    let mut body = [sector.to_le_bytes(), sector.to_be_bytes(), size.to_le_bytes(), size.to_be_bytes()].concat();
    body.extend([0; 7]);
    body.extend([if directory { DIRECTORY } else { 0 }, 0, 0, 1, 0, 0, 1, name.len() as u8]);
    body.extend(name);
    // A byte of padding puts what follows the name at an even place in the record.
    if !body.len().is_multiple_of(2) {
        body.push(0);
    }
    let attributes: u16 = if directory { 0x8D55 } else { 0x0D55 };
    body.extend([0, 0, 0, 0]);
    body.extend(attributes.to_be_bytes());
    body.extend(b"XA");
    body.extend([0; 6]);
    [&[body.len() as u8 + 2, 0], body.as_slice()].concat()
}

const CONFIG: &[u8] = b"BOOT = cdrom:\\SLPS_123.45;1\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801FFFF0\r\n";
const NOTE: &[u8] = b"in a folder";

/// The contents of a file of a sector and a half.
fn data() -> Vec<u8> {
    (0..=255).collect::<Vec<u8>>().repeat(12)
}

/// The data of a disc's sectors in order, as the file system sees them, and which of them are Form 2:
/// two files in the root, one in a folder, and a movie of a Form 1 and a Form 2 sector.
fn sectors(config: &[u8]) -> Vec<(Vec<u8>, bool)> {
    let whole_sector = SECTOR as u32;
    let root = [
        record(&[0], 18, whole_sector, true),
        record(&[1], 18, whole_sector, true),
        record(b"SYSTEM.CNF;1", 20, config.len() as u32, false),
        record(b"DATA.BIN;1", 21, data().len() as u32, false),
        record(b"MOVIE.STR;1", 24, 2 * whole_sector, false),
        record(b"SUB", 19, whole_sector, true),
    ]
    .concat();
    let sub = [
        record(&[0], 19, whole_sector, true),
        record(&[1], 18, whole_sector, true),
        record(b"NOTE.TXT;1", 23, NOTE.len() as u32, false),
    ]
    .concat();
    let mut volume = vec![0; SECTOR as usize];
    volume[..7].copy_from_slice(b"\x01CD001\x01");
    volume[40..72].copy_from_slice(format!("{:<32}", "TESTDISC").as_bytes());
    volume[318..446].copy_from_slice(format!("{:<128}", "A PUBLISHER").as_bytes());
    let root_record = record(&[0], 18, whole_sector, true);
    volume[156..156 + 34].copy_from_slice(&root_record[..34]);
    let mut sectors = vec![(Vec::new(), false); 16];
    sectors.push((volume, false));
    sectors.push((Vec::new(), false));
    sectors.push((root, false));
    sectors.push((sub, false));
    sectors.push((config.to_vec(), false));
    sectors.push((data()[..2048].to_vec(), false));
    sectors.push((data()[2048..].to_vec(), false));
    sectors.push((NOTE.to_vec(), false));
    sectors.push((b"the movie's header".to_vec(), false));
    sectors.push((vec![0x5A; FORM2_DATA], true));
    sectors.push((b"after the last file".to_vec(), false));
    sectors
}

/// A disc image written to `folder` as `name`, with a cue sheet beside it.
fn disc_with(folder: &Path, name: &str, config: &[u8]) -> PathBuf {
    let image: Vec<u8> = sectors(config)
        .iter()
        .enumerate()
        .flat_map(|(number, (data, form2))| whole(number as u32, *form2, data))
        .collect();
    let path = folder.join(name);
    fs::write(&path, image).unwrap();
    let sheet = format!("FILE \"{name}\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n");
    fs::write(path.with_extension("cue"), sheet).unwrap();
    path
}

fn image(folder: &Path) -> PathBuf {
    disc_with(folder, "disc.bin", CONFIG)
}

/// What is wrong with an image, or with what was asked of it, in the words of the error.
fn fault<T>(result: Result<T>) -> String {
    match result {
        Err(error @ Error::Image(_)) => error.to_string(),
        Err(other) => panic!("refused, but not as something an image cannot give: {other:?}"),
        Ok(_) => panic!("it was not refused"),
    }
}

/// Multiplication in the field the ECC is worked out in, done bit by bit, apart from the tables.
fn times(mut a: u8, mut b: u8) -> u8 {
    let mut product = 0;
    while b != 0 {
        if b & 1 != 0 {
            product ^= a;
        }
        a = if a & 0x80 != 0 { (a << 1) ^ 0x1D } else { a << 1 };
        b >>= 1;
    }
    product
}

/// Whether a codeword, its data bytes then its two parity bytes, is one the Reed-Solomon code allows:
/// the sum of its bytes is zero, and so is the sum of each byte times 2 to the power of how far it
/// is from the end.
fn is_codeword(bytes: &[u8]) -> bool {
    let plain = bytes.iter().fold(0, |sum, byte| sum ^ byte);
    let weighted = bytes.iter().fold(0, |sum, byte| times(sum, 2) ^ byte);
    plain == 0 && weighted == 0
}

/// Whether every P and Q codeword of a Form 1 sector checks, with its header taken as zero.
fn ecc_checks(sector: &[u8]) -> bool {
    let mut grid = sector[HEADER..].to_vec();
    grid[..4].fill(0);
    // P: 86 columns of 24 bytes, each followed by its two parity bytes.
    let p = (0..86).all(|column| {
        let mut word: Vec<u8> = (0..24).map(|row| grid[column + 86 * row]).collect();
        word.extend([grid[2064 + column], grid[2064 + 86 + column]]);
        is_codeword(&word)
    });
    // Q: 52 diagonals of 43 bytes through the data and P, each followed by its two parity bytes.
    let q = (0..52).all(|diagonal| {
        let mut index = (diagonal / 2) * 86 + diagonal % 2;
        let mut word = Vec::new();
        for _ in 0..43 {
            word.push(grid[index]);
            index = (index + 88) % 2236;
        }
        word.extend([grid[2236 + diagonal], grid[2236 + 52 + diagonal]]);
        is_codeword(&word)
    });
    p && q
}

#[test]
fn the_edc_is_the_cd_rom_crc() {
    // The check value of CRC-32/CD-ROM-EDC, and a longer one worked out bit by bit apart from this code.
    assert_eq!(edc(b"123456789"), 0x6EC2_EDC4);
    assert_eq!(edc(&(0..=255).collect::<Vec<u8>>().repeat(8)), 0xCD51_84E2);
    assert_eq!(edc(&[0; 2056]), 0);
}

#[test]
fn a_sealed_sector_has_parity_the_code_allows() {
    // An empty sector has all its check bytes zero, whatever its header.
    let mut empty = whole(1234, false, &[]);
    empty[16..24].fill(0);
    seal(&mut empty);
    assert!(empty[FORM1_EDC..].iter().all(|byte| *byte == 0));
    // A sector of varied data: every codeword checks, and the header does not count.
    let data: Vec<u8> = (0..2048u32).map(|index| (index * 7 + index / 13) as u8).collect();
    let sector = whole(16, false, &data);
    assert!(ecc_checks(&sector));
    assert_eq!(sector[FORM1_EDC..P_PARITY], edc(&sector[16..FORM1_EDC]).to_le_bytes());
    let elsewhere = whole(200_000, false, &data);
    assert_eq!(sector[FORM1_EDC..], elsewhere[FORM1_EDC..]);
    // One byte changed and the sector sealed again: it checks, and its parity is not what it was.
    let mut changed = sector.clone();
    changed[DATA + 100] ^= 0x40;
    assert!(!ecc_checks(&changed));
    seal(&mut changed);
    assert!(ecc_checks(&changed));
    assert_ne!(changed[P_PARITY..], sector[P_PARITY..]);
    // A Form 2 sector has an EDC over its subheader and its 2324 bytes, and no ECC.
    let movie = whole(30, true, &[0x5A; FORM2_DATA]);
    assert!(is_form2(&movie));
    assert_eq!(movie[FORM2_EDC..], edc(&movie[16..FORM2_EDC]).to_le_bytes());
}

#[test]
fn a_sector_header_counts_from_two_seconds() {
    assert_eq!(header(0), [0x00, 0x02, 0x00, 2]);
    assert_eq!(header(16), [0x00, 0x02, 0x16, 2]);
    assert_eq!(header(208_298), [0x46, 0x19, 0x23, 2]);
}

#[test]
fn lists_files_in_folders() {
    let folder = tempfile::tempdir().unwrap();
    let disc = Disc::open(&image(folder.path())).unwrap();
    let expected = [
        ("DATA.BIN", (21, 3072)),
        ("MOVIE.STR", (24, 4096)),
        ("SUB/NOTE.TXT", (23, 11)),
        ("SYSTEM.CNF", (20, CONFIG.len() as u32)),
    ];
    assert_eq!(disc.files, BTreeMap::from(expected.map(|(name, place)| (name.to_string(), place))));
    assert_eq!(disc.records.keys().collect::<Vec<_>>(), disc.files.keys().collect::<Vec<_>>());
    // The note's record is the third in its folder, after two of 48 bytes.
    assert_eq!(disc.records["SUB/NOTE.TXT"], 19 * SECTOR + 96);
    assert_eq!(disc.label, "TESTDISC");
    assert_eq!(disc.publisher, "A PUBLISHER");
    assert_eq!(disc.sector_count().unwrap(), 27);
    assert_eq!(disc.cue, Some(folder.path().join("disc.cue")));
}

#[test]
fn reads_a_file_or_a_part_of_one() {
    let folder = tempfile::tempdir().unwrap();
    let disc = Disc::open(&image(folder.path())).unwrap();
    assert_eq!(disc.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
    assert_eq!(disc.read_from("DATA.BIN", 0).unwrap(), data());
    assert_eq!(disc.read("DATA.BIN", 0x7FE, 4).unwrap(), [0xFE, 0xFF, 0x00, 0x01]); // across two sectors
    assert_eq!(disc.read_from("DATA.BIN", 3072).unwrap(), b"");
    assert_eq!(disc.read_from("DATA.BIN", 0x900).unwrap(), data()[0x900..]);
    assert_eq!(disc.size("DATA.BIN").unwrap(), 3072);
    assert_eq!(disc.first_sector("DATA.BIN").unwrap(), 21);
    // By sector number, as the console reads, and whole sectors.
    assert_eq!(disc.data(21, 0x7FE, 4).unwrap(), [0xFE, 0xFF, 0x00, 0x01]);
    assert_eq!(disc.data(26, 0, 19).unwrap(), b"after the last file");
    assert_eq!(disc.data(26, 0, 1 << 20).unwrap().len(), 2048); // the image ends
    let movie = disc.raw(25, 1).unwrap();
    assert!(is_form2(&movie));
    assert_eq!(movie[DATA..FORM2_EDC], [0x5A; FORM2_DATA]);
    assert_eq!(disc.raw(25, 5).unwrap().len(), 2 * RAW_SECTOR);
    assert_eq!(fault(disc.read("SUB/NOTE.TXT", 8, 8)), "SUB/NOTE.TXT has no bytes 0x8 to 0x10: it is 0xb long");
    assert!(fault(disc.read_from("MISSING.BIN", 0)).ends_with("has no file named MISSING.BIN"));
}

#[test]
fn names_the_program_the_console_starts() {
    let folder = tempfile::tempdir().unwrap();
    let disc = Disc::open(&image(folder.path())).unwrap();
    assert_eq!(disc.boot_name().unwrap(), "SLPS_123.45");
    assert_eq!(disc.serial().unwrap(), "SLPS-12345");
    assert_eq!(disc.version().unwrap(), None);
    // The ways discs write the line.
    assert_eq!(boot_name_in("BOOT=cdrom:SCPS_100.01;1\r\n").as_deref(), Some("SCPS_100.01"));
    assert_eq!(boot_name_in("boot = CDROM:\\SLUS_000.67;1").as_deref(), Some("SLUS_000.67"));
    assert_eq!(boot_name_in("BOOT2 = cdrom0:\\SLPS_251.72;1\r\n"), None);
    assert_eq!(boot_name_in("BOOT = cdrom:\\;1"), None);
}

#[test]
fn a_disc_says_its_serial_and_version() {
    let folder = tempfile::tempdir().unwrap();
    let path = image(folder.path());
    let (image, _) = PLATFORM.open(&path).unwrap();
    let label = PLATFORM.label(&image).unwrap();
    assert_eq!(label, Label { name: "SLPS-12345".into(), version: String::new() });
    assert_eq!(crate::platform::label_words(&PLATFORM, &label.name, &label.version), "serial SLPS-12345");
    assert_eq!(PLATFORM.peek(&path), Some(label.clone()));
    assert_eq!(PLATFORM.peek(&path.with_extension("cue")), Some(label));
    let versioned = disc_with(folder.path(), "versioned.bin", b"BOOT = cdrom:\\SLPM_860.01;1\r\nVER = 1.1\r\n");
    let (versioned, _) = PLATFORM.open(&versioned).unwrap();
    assert_eq!(PLATFORM.label(&versioned), Some(Label { name: "SLPM-86001".into(), version: "1.1".into() }));
    // A PlayStation 2 game on a CD names its program on a BOOT2 line: nothing to name it by here.
    let ps2 = disc_with(folder.path(), "ps2.bin", b"BOOT2 = cdrom0:\\SLPS_200.01;1\r\nVER = 1.00\r\n");
    let (ps2, _) = PLATFORM.open(&ps2).unwrap();
    assert_eq!(PLATFORM.label(&ps2), None);
    assert_eq!(PLATFORM.label(&Image::Cartridge(Vec::new())), None);
}

#[test]
fn a_cue_sheet_names_the_data_track() {
    let folder = tempfile::tempdir().unwrap();
    let bin = image(folder.path());
    let disc = Disc::open(&bin.with_extension("cue")).unwrap();
    assert_eq!(disc.path, bin);
    assert_eq!(disc.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
    let sheet = "REM a comment\nFILE track1.bin BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n\
                 FILE \"Track 2 (audio).bin\" BINARY\n  TRACK 02 AUDIO\n    INDEX 00 00:00:00\n";
    let files = parse_cue(sheet);
    assert_eq!(
        files,
        [
            CueFile { name: "track1.bin".into(), tracks: vec![(1, "MODE2/2352".into())] },
            CueFile { name: "Track 2 (audio).bin".into(), tracks: vec![(2, "AUDIO".into())] },
        ]
    );
    // A sheet whose first track is audio, or one with no tracks, is no PlayStation disc's.
    let music = folder.path().join("music.cue");
    fs::write(&music, "FILE \"music.bin\" BINARY\n  TRACK 01 AUDIO\n").unwrap();
    assert!(fault(Disc::open(&music)).contains("its first track is AUDIO"));
    assert!(!PLATFORM.recognises(&music).unwrap());
    // One whose .bin is missing is taken, so that opening it says what is missing.
    let missing = folder.path().join("missing.cue");
    fs::write(&missing, "FILE \"gone.bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();
    assert!(PLATFORM.recognises(&missing).unwrap());
    assert!(matches!(Disc::open(&missing), Err(Error::Io(_))));
    // A .bin alone has no sheet; one beside it that names another .bin is not its own.
    let alone = folder.path().join("alone.bin");
    fs::copy(&bin, &alone).unwrap();
    assert_eq!(Disc::open(&alone).unwrap().cue, None);
    fs::write(alone.with_extension("cue"), "FILE \"disc.bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();
    assert_eq!(Disc::open(&alone).unwrap().cue, None);
}

#[test]
fn write_image_with_no_change_is_the_original() {
    let folder = tempfile::tempdir().unwrap();
    let image = image(folder.path());
    let disc = Disc::open(&image).unwrap();
    let out = folder.path().join("out").join("copy.bin");
    fs::create_dir(out.parent().unwrap()).unwrap();
    write_image(&disc, &out, &[]).unwrap();
    assert!(fs::read(&out).unwrap() == fs::read(&image).unwrap());
    let sheet = fs::read_to_string(out.with_extension("cue")).unwrap();
    assert_eq!(sheet, "FILE \"copy.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n");
    // Asked for by its .cue, the copy is the same, its .bin beside it.
    let by_cue = folder.path().join("out").join("other.cue");
    write_image(&disc, &by_cue, &[]).unwrap();
    assert!(fs::read(by_cue.with_extension("bin")).unwrap() == fs::read(&image).unwrap());
    assert_eq!(Disc::open(&by_cue).unwrap().read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
}

#[test]
fn write_image_changes_bytes_inside_files_and_seals_their_sectors() {
    let folder = tempfile::tempdir().unwrap();
    let image = image(folder.path());
    let disc = Disc::open(&image).unwrap();
    let out = folder.path().join("out.bin");
    // Across the two sectors of DATA.BIN, and at the start of SYSTEM.CNF.
    let patches = [("DATA.BIN", 0x7FE, b"WXYZ".to_vec()), ("SYSTEM.CNF", 0, b"boot".to_vec())];
    write_image(&disc, &out, &patches).unwrap();
    let (before, after) = (fs::read(&image).unwrap(), fs::read(&out).unwrap());
    assert_eq!(before.len(), after.len());
    let differ: Vec<usize> = (0..before.len() / RAW_SECTOR)
        .filter(|sector| before[sector * RAW_SECTOR..][..RAW_SECTOR] != after[sector * RAW_SECTOR..][..RAW_SECTOR])
        .collect();
    assert_eq!(differ, [20, 21, 22]);
    for sector in differ {
        let whole = &after[sector * RAW_SECTOR..][..RAW_SECTOR];
        assert_eq!(whole[..24], before[sector * RAW_SECTOR..][..24]); // sync, header and subheader kept
        assert_eq!(whole[FORM1_EDC..P_PARITY], edc(&whole[16..FORM1_EDC]).to_le_bytes());
        assert!(ecc_checks(whole));
    }
    let built = Disc::open(&out).unwrap();
    let patched = [&[0xFC, 0xFD], b"WXYZ".as_slice(), &[0x02, 0x03]].concat();
    assert_eq!(built.read("DATA.BIN", 0x7FC, 8).unwrap(), patched);
    assert_eq!(built.read("SYSTEM.CNF", 0, 8).unwrap(), b"boot = c");
    assert_eq!(built.read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
}

#[test]
fn write_image_refuses_what_it_cannot_do_before_writing() {
    let folder = tempfile::tempdir().unwrap();
    let image = image(folder.path());
    let disc = Disc::open(&image).unwrap();
    let bad = folder.path().join("bad.bin");
    let too_long = [("SUB/NOTE.TXT", 8, b"too long".to_vec())];
    assert!(fault(write_image(&disc, &bad, &too_long)).starts_with("SUB/NOTE.TXT has no bytes 0x8 to 0x10"));
    assert!(fault(write_image(&disc, &bad, &[("MISSING.BIN", 0, vec![1])])).contains("no file named"));
    let in_the_movie = [("MOVIE.STR", 2048, vec![0])];
    assert!(fault(write_image(&disc, &bad, &in_the_movie)).contains("is a Form 2 sector"));
    assert!(!bad.exists() && !bad.with_extension("cue").exists());
    // The image that is open is never written over, by its .bin or by its .cue.
    let before = fs::read(&image).unwrap();
    assert!(fault(write_image(&disc, &image, &[])).contains("is the image being read"));
    let by_cue = Disc::open(&image.with_extension("cue")).unwrap();
    assert!(fault(write_image(&by_cue, &folder.path().join("disc.cue"), &[])).contains("is the image being read"));
    assert!(fs::read(&image).unwrap() == before);
    // The first sector of the movie is a Form 1 one, and can be changed.
    write_image(&disc, &bad, &[("MOVIE.STR", 0, b"THE".to_vec())]).unwrap();
    assert_eq!(Disc::open(&bad).unwrap().read("MOVIE.STR", 0, 6).unwrap(), b"THE mo");
}

#[test]
fn a_copy_keeps_the_other_tracks_of_its_cue_sheet() {
    let folder = tempfile::tempdir().unwrap();
    let bin = image(folder.path());
    let cue = bin.with_extension("cue");
    fs::write(
        &cue,
        "FILE \"disc.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n\
         FILE \"disc (Track 2).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n    INDEX 01 00:00:00\r\n",
    )
    .unwrap();
    let disc = Disc::open(&cue).unwrap();
    // In the same folder, the audio track is named as it was.
    write_image(&disc, &folder.path().join("copy.bin"), &[]).unwrap();
    let sheet = fs::read_to_string(folder.path().join("copy.cue")).unwrap();
    assert!(sheet.starts_with("FILE \"copy.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n"));
    assert!(sheet.contains("\r\nFILE \"disc (Track 2).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n"));
    // Elsewhere, by its full path.
    let elsewhere = folder.path().join("out");
    fs::create_dir(&elsewhere).unwrap();
    write_image(&disc, &elsewhere.join("copy.bin"), &[]).unwrap();
    let sheet = fs::read_to_string(elsewhere.join("copy.cue")).unwrap();
    let audio = std::path::absolute(folder.path().join("disc (Track 2).bin")).unwrap();
    assert!(sheet.contains(&format!("FILE \"{}\" BINARY\r\n", audio.display())));
}

#[test]
fn what_is_kept_for_an_image_is_worked_out_once() {
    let folder = tempfile::tempdir().unwrap();
    let disc = Disc::open(&image(folder.path())).unwrap();
    let mut made = 0;
    let mut size = || {
        disc.keep("the size of the note", || {
            made += 1;
            disc.size("SUB/NOTE.TXT")
        })
    };
    assert_eq!((*size().unwrap(), *size().unwrap()), (11, 11));
    assert_eq!(made, 1);
    assert_eq!(fault(disc.keep("nothing", || image_error::<u64>("not yet"))), "not yet");
    assert_eq!(*disc.keep("nothing", || Ok("now".to_string())).unwrap(), "now");
}

#[test]
fn the_platform_opens_a_disc_and_names_it_by_the_sha1_of_its_bin() {
    let folder = tempfile::tempdir().unwrap();
    let path = image(folder.path());
    let other = folder.path().join("game.sfc");
    fs::write(&other, vec![0; 0x10000]).unwrap();
    assert_eq!(PLATFORM.name(), "PlayStation");
    assert!(PLATFORM.recognises(&path).unwrap());
    assert!(PLATFORM.recognises(&path.with_extension("cue")).unwrap());
    assert!(!PLATFORM.recognises(&other).unwrap());
    assert!(!PLATFORM.recognises(&folder.path().join("absent.bin")).unwrap());
    assert!(fault(Disc::open(&other)).contains("is not the data track of a PlayStation disc"));
    let whole = Sha1::digest(fs::read(&path).unwrap());
    let expected: String = whole.iter().map(|byte| format!("{byte:02x}")).collect();
    for given in [path.clone(), path.with_extension("cue")] {
        let (image, sha1) = PLATFORM.open(&given).unwrap();
        assert_eq!(sha1, expected);
        assert_eq!(image.size().unwrap(), 27 * RAW_SECTOR as u64);
        assert_eq!(image.ps1_disc().unwrap().read_from("SUB/NOTE.TXT", 0).unwrap(), NOTE);
        assert!(image.disc().is_err());
    }
    // A search finds the game by its .bin, never by its .cue.
    let glance = PLATFORM.glance(&path).unwrap().unwrap();
    assert_eq!(glance, Glance { size: 27 * RAW_SECTOR as u64, name: "disc.bin".into() });
    assert_eq!(PLATFORM.glance(&path.with_extension("cue")).unwrap(), None);
}

#[test]
fn a_playstation_2_image_and_a_playstation_image_are_each_their_own_consoles() {
    let folder = tempfile::tempdir().unwrap();
    let bin = image(folder.path());
    let iso = crate::platforms::ps2::tests::disc_with(folder.path(), "ps2.iso", b"BOOT2 = cdrom0:\\SLPS_251.72;1\r\n");
    assert_eq!(crate::platforms::recognise(&bin).unwrap().map(|platform| platform.name()), Some("PlayStation"));
    assert_eq!(
        crate::platforms::recognise(&bin.with_extension("cue")).unwrap().map(|platform| platform.name()),
        Some("PlayStation")
    );
    assert_eq!(crate::platforms::recognise(&iso).unwrap().map(|platform| platform.name()), Some("PlayStation 2"));
    assert!(!PLATFORM.recognises(&iso).unwrap());
    assert!(!crate::platforms::ps2::PLATFORM.recognises(&bin).unwrap());
    // By size alone: a .bin of 2352-byte sectors is no PlayStation 2 image.
    assert_eq!(crate::platforms::ps2::PLATFORM.glance(&bin).unwrap(), None);
}

/// G.O.D pure (Japan), SLPS-00944, as Redump lists it: one Mode 2 track.
const GOD_SIZE: u64 = 489_919_248;
const GOD_SHA1: &str = "a771b5f362751845086e02f53b2c0dd7f302361a";

/// The real disc, where it is there to test with.
fn god() -> Option<PathBuf> {
    crate::testing::image_file(GOD_SIZE, GOD_SHA1)
}

#[test]
fn reads_the_files_of_a_real_disc() {
    let Some(path) = god() else { return };
    let disc = Disc::open(&path).unwrap();
    for name in ["SYSTEM.CNF", "SLPS_009.44", "GOD.DAT", "MOVIE/OPENNING.STR", "DATA/OUT3MIN.DAT"] {
        assert!(disc.files.contains_key(name), "{name} is not on the disc");
    }
    assert_eq!(disc.files["SLPS_009.44"], (24, 866_304));
    assert_eq!(disc.files["GOD.DAT"], (447, 148_281_344));
    assert_eq!(disc.serial().unwrap(), "SLPS-00944");
    assert_eq!(disc.sector_count().unwrap() as u64 * RAW_SECTOR as u64, GOD_SIZE);
    // The program starts with the header every PlayStation program has.
    assert_eq!(disc.read("SLPS_009.44", 0, 8).unwrap(), b"PS-X EXE");
    let (image, sha1) = PLATFORM.open(&path.with_extension("cue")).unwrap();
    assert_eq!(sha1, GOD_SHA1);
    let label = PLATFORM.label(&image).unwrap();
    assert_eq!(label.name, "SLPS-00944");
    assert_eq!(PLATFORM.peek(&path), Some(label));
}

/// The proof that the EDC and ECC are worked out right: each sector of a real disc, sealed again,
/// is what the disc has. A Form 2 sector whose EDC the disc left as zero is passed over.
#[test]
fn sealing_every_sector_of_a_real_disc_gives_it_back() {
    let Some(path) = god() else { return };
    let disc = Disc::open(&path).unwrap();
    let count = disc.sector_count().unwrap();
    let (mut form1, mut form2, mut form2_unsummed, mut wrong) = (0u32, 0u32, 0u32, Vec::new());
    let mut first = 0;
    while first < count {
        let batch = disc.raw(first, BATCH as u32).unwrap();
        for (index, stored) in batch.as_chunks::<RAW_SECTOR>().0.iter().enumerate() {
            let number = first + index as u32;
            assert!(is_mode2(stored), "sector {number} is not a Mode 2 sector");
            assert_eq!(stored[12..15], header(number)[..3], "sector {number} says it is elsewhere");
            if is_form2(stored) {
                if stored[FORM2_EDC..] == [0; 4] {
                    form2_unsummed += 1;
                    continue;
                }
                form2 += 1;
            } else {
                form1 += 1;
            }
            let mut sealed = stored.to_vec();
            seal(&mut sealed);
            if sealed != stored {
                wrong.push(number);
            }
        }
        first += BATCH as u32;
    }
    eprintln!(
        "{form1} Form 1 sectors and {form2} Form 2 sectors sealed again; {form2_unsummed} Form 2 sectors with no EDC; \
         {} differ",
        wrong.len()
    );
    assert!(wrong.is_empty(), "sectors whose check bytes are not what sealing gives: {wrong:?}");
    assert_eq!(form1 + form2 + form2_unsummed, count);
    assert!(form1 > 0 && form2 + form2_unsummed > 0);
}

#[test]
fn writes_a_real_disc_back() {
    let Some(path) = god() else { return };
    let disc = Disc::open(&path.with_extension("cue")).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let same = folder.path().join("same.bin");
    write_image(&disc, &same, &[]).unwrap();
    assert_eq!(Disc::open(&same).unwrap().sha1().unwrap(), GOD_SHA1);
    let sheet = fs::read_to_string(folder.path().join("same.cue")).unwrap();
    assert!(sheet.starts_with("FILE \"same.bin\" BINARY"));
    let _ = fs::remove_file(&same);

    // One byte changed in the second sector of a file: that sector alone differs, and it checks.
    let name = "DATA/OUT3MIN.DAT";
    let offset = 0x900;
    let old = disc.read(name, offset, 1).unwrap()[0];
    let changed = folder.path().join("changed.bin");
    write_image(&disc, &changed, &[(name, offset, vec![old ^ 0xFF])]).unwrap();
    assert_eq!(fs::metadata(&changed).unwrap().len(), GOD_SIZE);
    let target = disc.first_sector(name).unwrap() + 1;
    let (mut original, mut copy) = (File::open(&path).unwrap(), File::open(&changed).unwrap());
    let mut differ = Vec::new();
    let mut first = 0u64;
    loop {
        let (a, b) =
            (read_raw(&mut original, first, BATCH as u64).unwrap(), read_raw(&mut copy, first, BATCH as u64).unwrap());
        assert_eq!(a.len(), b.len());
        if a.is_empty() {
            break;
        }
        for (index, (x, y)) in
            a.as_chunks::<RAW_SECTOR>().0.iter().zip(b.as_chunks::<RAW_SECTOR>().0.iter()).enumerate()
        {
            if x != y {
                differ.push(first as u32 + index as u32);
            }
        }
        first += BATCH as u64;
    }
    assert_eq!(differ, [target]);
    let built = Disc::open(&changed).unwrap();
    let sector = built.raw(target, 1).unwrap();
    assert!(!is_form2(&sector));
    assert!(ecc_checks(&sector));
    assert_eq!(sector[FORM1_EDC..P_PARITY], edc(&sector[16..FORM1_EDC]).to_le_bytes());
    assert_eq!(built.read(name, offset, 1).unwrap(), [old ^ 0xFF]);
}
