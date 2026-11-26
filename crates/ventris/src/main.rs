//! Command-line interface.

use std::fs;
use std::io;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};

use ventris::adapter::{Adapter, BuildOptions, same_adapter};
use ventris::error::at;
use ventris::fonts::{self, Font};
use ventris::tasks::{self, open_game, shown};
use ventris::{Result, adapters, conformance, png, refusal};

#[derive(Parser)]
#[command(name = "ventris", version, about = "Game translation patch engine.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract a game's script to editable text files
    Extract {
        rom: PathBuf,
        /// Directory for the script files
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Build a game file from the original and an edited script
    Insert {
        /// The original game file
        rom: PathBuf,
        /// The pack: a folder of script files, whose pack.toml, if it has one, says what else the
        /// build needs; strings it leaves out keep the game's own text
        script: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Font to draw characters the game's own font lacks, in place of the ones the pack names
        /// or the game's own: a built-in font's name, a Unifont .hex file, or for some games a BDF
        /// file; several may be given, and are tried in that order
        #[arg(long, value_name = "FONT")]
        font: Vec<String>,
        /// Font to draw characters the menu font lacks, in place of the one the pack names or the
        /// game's own: a built-in font's name, or an 8x8 .hex file such as Unscii-8
        #[arg(long, value_name = "FONT")]
        menu_font: Option<String>,
        /// Turn on one of the game's own build options; the games command lists them
        #[arg(long, value_name = "NAME")]
        option: Vec<String>,
        /// Keep line breaks exactly as written instead of wrapping at spaces
        #[arg(long)]
        no_wrap: bool,
    },
    /// Check that a game's adapter keeps the contract: round trip, identical rebuild, changed strings
    Check {
        /// The original game file
        rom: PathBuf,
        /// Folder to build in, made if it is not there; it needs room for a copy of the game
        #[arg(long)]
        work: Option<PathBuf>,
    },
    /// List the games that have an adapter, and what a pack can change in each
    Games,
    /// Look through a folder of games, and the folders in it, for the ones that have an adapter
    Find {
        /// The folder the games are kept in, loose or one to a zip archive
        folder: PathBuf,
    },
    /// Say what a pack is, and check it as far as can be done without the game
    Describe {
        /// The pack's folder, which holds its pack.toml
        folder: PathBuf,
    },
    /// List the fonts built into the program, with the terms each is passed on under
    Licenses,
    /// Render a game's font as a PNG sheet
    Font {
        rom: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u32).range(1..))]
        per_row: u32,
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u32).range(1..))]
        scale: u32,
    },
}

fn extract(rom: &Path, output: &Path) -> Result<()> {
    let extracted = tasks::extract(rom, output)?;
    for line in extracted.lines() {
        println!("{line}");
    }
    println!("wrote {} files to {} and verified that they rebuild the original text", extracted.files, shown(output));
    Ok(())
}

fn insert(rom: &Path, script: &Path, output: &Path, options: &BuildOptions) -> Result<()> {
    let report = tasks::build(rom, script, output, options)?;
    println!("game:      {}", report.adapter.name());
    if let Some(manifest) = &report.manifest {
        println!("{} {}", tasks::label("pack"), manifest.title());
    }
    for line in report.summary.iter().chain(&report.changes) {
        println!("{line}");
    }
    println!("identical: {}, sha1 {}", if report.identical { "yes" } else { "no" }, report.sha1);
    println!("wrote {}", shown(output));
    Ok(())
}

fn describe(folder: &Path) -> Result<()> {
    let pack = tasks::describe(folder)?;
    let manifest = &pack.manifest;
    println!("{} {}", tasks::label("pack"), manifest.title());
    println!("game:      {}", pack.adapter.name());
    println!("{} {}", tasks::label("authors"), manifest.authors.join(", "));
    println!("{} {}", tasks::label("license"), manifest.license.as_deref().unwrap_or("none stated"));
    for count in pack.kinds.iter().filter(|count| count.sections > 0) {
        println!("{}", count.line());
    }
    if pack.names > 0 {
        println!("names:     {} characters renamed", pack.names);
    }
    // The fonts a build of it draws from: the ones it names, or else its game's own.
    let own = |own: bool| if own { ", the game's own" } else { "" };
    if let Some((fonts, game_own)) = pack.fonts() {
        println!("fonts:     {}{}", fonts.join(", "), own(game_own));
    }
    if let Some((font, game_own)) = pack.menu_font() {
        println!("menu font: {font}{}", own(game_own));
    }
    println!("the pack is well formed; whether each string fits its place is known when it is built");
    Ok(())
}

/// A folder to build in, removed with everything in it when it is no longer needed.
struct Scratch(PathBuf);

impl Scratch {
    /// A new folder inside `parent`, or inside the system's folder for temporary files. A `parent`
    /// that does not exist yet is made, with the folders above it; it is left in place afterwards.
    fn new(parent: Option<&Path>) -> Result<Scratch> {
        let parent = parent.map_or_else(std::env::temp_dir, Path::to_path_buf);
        fs::create_dir_all(&parent).map_err(at(&parent))?;
        for attempt in 0..100 {
            let moment = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.subsec_nanos());
            let path = parent.join(format!("ventris-check-{}-{moment:09}-{attempt}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Scratch(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "no name is free for a folder to build in").into())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn check(rom: &Path, work: Option<&Path>) -> Result<()> {
    let (image, adapter, sha1) = open_game(rom)?;
    let folder = Scratch::new(work)?;
    println!("game:      {}", adapter.name());
    for line in conformance::check(adapter, &image, &sha1, &folder.0)? {
        println!("{line}");
    }
    println!("the adapter {} keeps the contract, as far as a program can tell", adapter.id());
    Ok(())
}

/// The games in a folder that there is an adapter for, and where each is.
fn find(folder: &Path) -> Result<()> {
    let survey = tasks::survey(folder)?;
    let (found, read) = tasks::identify_noting(&survey.candidates);
    for game in &found {
        println!("{}", game.adapter.name());
        println!("  id         {}", game.adapter.id());
        println!("  release    {}", game.release.name);
        println!("  file       {}", shown(&game.path));
    }
    // A game that is not found, and of which the folder holds a wrong copy, is named with why each
    // copy is not the file needed, and with the file that is.
    let is_found = |adapter: &&dyn Adapter| found.iter().any(|game| same_adapter(game.adapter, *adapter));
    let missing: Vec<&'static dyn Adapter> =
        adapters::ALL.iter().copied().filter(|adapter| !is_found(adapter)).collect();
    for missed in survey.missed(&missing, &read).iter().filter(|missed| !missed.is_empty()) {
        println!("{}", missed.adapter.name());
        println!("  id         {}", missed.adapter.id());
        println!("  not found  {}", refusal::needed(missed.adapter));
        for line in missed.lines(folder) {
            println!("  passed by  {line}");
        }
    }
    let known = adapters::ALL.len();
    println!(
        "found {} of the {known} games there are adapters for; {} files had the size of one",
        found.len(),
        survey.candidates.len()
    );
    Ok(())
}

fn games() {
    for adapter in adapters::ALL {
        println!("{}", adapter.name());
        println!("  id         {}, for {}, in {}", adapter.id(), adapter.platform().name(), adapter.source_language());
        for image in adapter.images() {
            println!("  release    {}", image.name);
            println!("             {}", refusal::release_facts(adapter.platform(), image));
            if let Some(words) = refusal::catalogue_words(image) {
                println!("             {words}");
            }
            if let Some(page) = image.catalogue.and_then(|entry| entry.page) {
                println!("             {page}");
            }
        }
        for kind in adapter.kinds() {
            let mut can = vec!["extract".to_string()];
            if kind.insert {
                let longer = match kind.max_length {
                    Some(most) if most > 0 => format!(", up to {most} characters"),
                    _ => String::new(),
                };
                can.push(if kind.grow {
                    format!("insert, longer strings{longer}")
                } else {
                    "insert, no longer than the game's own".to_string()
                });
            }
            if kind.new_characters {
                can.push("new characters".to_string());
            }
            if kind.wrap {
                can.push("lines fitted to the screen".to_string());
            }
            println!("  {:<10} {}", kind.name, can.join("; "));
            if !kind.limits.is_empty() {
                println!("             limits: {}", kind.limits);
            }
        }
        if adapter.sets_names() {
            println!("  names      a pack can rename the characters");
            if !adapter.name_limits().is_empty() {
                println!("             limits: {}", adapter.name_limits());
            }
        }
        if !adapter.fonts().is_empty() {
            println!("  fonts      new characters are drawn from {}", adapter.fonts().join(", then "));
        }
        if let Some(font) = adapter.menu_font() {
            println!("  menu font  new characters of the menus are drawn from {font}");
        }
        for (name, purpose) in adapter.switches() {
            println!("  option     {name}: {purpose}");
        }
        println!();
    }
}

/// The fonts built into the program, and then the terms of each in full: they ask to be kept with
/// every copy, and a copy of the program is a copy of the fonts.
fn licenses() {
    println!("The fonts built into ventris, each under its own terms:");
    println!();
    for font in &fonts::BUILT_IN {
        println!("  {}", font.name);
        println!("      {}", font.about);
        println!("      terms: {}", font.license);
    }
    for font in &fonts::BUILT_IN {
        println!();
        println!("==== {} ====", font.name);
        println!();
        println!("{}", font.terms.trim_end());
    }
}

fn font(rom: &Path, output: &Path, per_row: usize, scale: usize) -> Result<()> {
    let (image, adapter, _) = open_game(rom)?;
    let (size, shades, count) = (adapter.glyph_size(), adapter.glyph_shades(), adapter.glyph_count());
    // Each glyph in a square of its own, with a line of grey between it and the next.
    let cell = size + 1;
    let mut pixels = vec![vec![64u8; per_row * cell]; count.div_ceil(per_row) * cell];
    for index in 0..count {
        let (left, top) = ((index % per_row) * cell, (index / per_row) * cell);
        for (y, row) in adapter.glyph(&image, index)?.iter().enumerate() {
            for x in 0..size {
                pixels[top + y][left + x] = shades[usize::from(row[x])];
            }
        }
    }
    let widened = |row: &Vec<u8>| -> Vec<u8> { row.iter().flat_map(|value| repeat_n(*value, scale)).collect() };
    let scaled: Vec<Vec<u8>> = pixels.iter().flat_map(|row| repeat_n(widened(row), scale)).collect();
    fs::write(output, png::encode_greyscale(&scaled)).map_err(at(output))?;
    let (first, last) = (adapter.code_at(0), adapter.code_at(count.saturating_sub(1)));
    println!("wrote {}: {count} glyphs, {per_row} per row, codes 0x{first:02x} to 0x{last:04x}", shown(output));
    Ok(())
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Extract { rom, output } => extract(&rom, &output),
        Command::Insert { rom, script, output, font, menu_font, option, no_wrap } => {
            let options = BuildOptions {
                fonts: font.iter().map(|font| Font::named(font)).collect(),
                menu_font: menu_font.as_deref().map(Font::named),
                wrap: !no_wrap,
                switches: option.into_iter().collect(),
                ..BuildOptions::default()
            };
            insert(&rom, &script, &output, &options)
        }
        Command::Check { rom, work } => check(&rom, work.as_deref()),
        Command::Games => {
            games();
            Ok(())
        }
        Command::Find { folder } => find(&folder),
        Command::Describe { folder } => describe(&folder),
        Command::Licenses => {
            licenses();
            Ok(())
        }
        Command::Font { rom, output, per_row, scale } => font(&rom, &output, per_row as usize, scale as usize),
    }
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_to_build_in_is_made_when_it_is_not_there_and_used_when_it_is() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("not").join("there");
        let scratch = Scratch::new(Some(&parent)).unwrap();
        assert!(scratch.0.is_dir());
        assert_eq!(scratch.0.parent(), Some(parent.as_path()));
        // The same folder again, now that it exists: no complaint, and a folder of its own.
        let again = Scratch::new(Some(&parent)).unwrap();
        assert!(again.0.is_dir());
        assert_ne!(again.0, scratch.0);
        let (first, second) = (scratch.0.clone(), again.0.clone());
        drop((scratch, again));
        assert!(!first.exists() && !second.exists());
        assert!(parent.is_dir());
    }
}
