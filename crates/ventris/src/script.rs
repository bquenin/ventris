//! Scripts as editable UTF-8 text files.
//!
//! One file per section, named after it: a dialogue block (`05.txt`) or another body of text such as
//! `menu.txt`. Each string is a header line followed by its text and a blank line:
//!
//! ```text
//! [05.000]
//! first line
//! second line{PAGE}
//! next page
//!
//! [05.004]
//! ...
//! ```
//!
//! The header is the string's permanent address: section, then string number. An extracted script
//! holds every string. A pack (a translation, or a revision in the original language) holds only the
//! strings it changes, in any number of files; whatever it leaves out keeps the game's own text.
//!
//! A line that starts with `# ` (a hash and a space), or that is a hash alone, is a comment. It is
//! not part of any string and is left out when the file is read, wherever it stands: at the head of
//! the file, under a header, between two strings. An extracted script carries comments that say
//! what limits a string; a translator can add their own.
//!
//! A line is a header only when it is exactly an address in square brackets. A line of text that is
//! exactly that, or that starts with a hash, is written with a backslash before it (`\#1`), and read
//! back without it. Text that merely starts with a bracket (`[Cress]`) is text as it is.
//!
//! Two near misses are refused rather than taken for text, which would put them in the game or join
//! a string to the one before: an address with one of its brackets missing, and a header as it was
//! written before the brackets, `## 05.004`.
//!
//! Files are written with `\n` line ends on every system, and read with either kind.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result, text_error};
use crate::text::quoted;

/// Section name -> string number -> text.
pub type Script = BTreeMap<String, BTreeMap<u32, String>>;

/// What a folder of script files holds: strings by address, and the characters' names if it gives any.
///
/// It is what a translator writes and what an adapter is handed to build from. It is also what an
/// adapter extracts: the game's own script is the pack that holds every string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pack {
    pub texts: Script,
    /// A character's tag in the script, and the name to show, in the order the file has them.
    pub names: Vec<(String, String)>,
}

impl Pack {
    pub fn of(texts: Script) -> Self {
        Pack { texts, names: Vec::new() }
    }

    /// The name a pack gives the character with this tag, if it gives one.
    pub fn name(&self, tag: &str) -> Option<&str> {
        self.names.iter().find(|(known, _)| known == tag).map(|(_, name)| name.as_str())
    }
}

pub const NAMES_FILE: &str = "names.txt";
pub const NAMES_HEADER: &str = "On the left, a character's name in the game, which is also its tag in the script. On the right, the\n\
name to show. Change the right-hand side. A line you remove keeps the game's own name.";

fn script_error<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Script(message.into()))
}

fn is_section(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A string's address as it is written in a script file.
pub fn string_name(section: &str, number: u32) -> String {
    format!("{section}.{number:03}")
}

/// The section and number of a string's address, as `string_name` writes it.
pub fn parse_name(name: &str) -> Result<(String, u32)> {
    let parsed = name.rsplit_once('.').and_then(|(section, number)| {
        let digits = !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
        (is_section(section) && digits).then(|| Some((section.to_string(), number.parse().ok()?)))?
    });
    parsed.ok_or_else(|| Error::Script(format!("{} is not a string's address", quoted(name))))
}

/// Whether a line of a script file is a comment: a hash and a space, or a hash alone.
fn is_comment(line: &str) -> bool {
    line == "#" || line.starts_with("# ")
}

/// The address a header line names: the line, spaces after it aside, is exactly `[address]`.
fn header_of(line: &str) -> Option<(String, u32)> {
    parse_name(line.trim_end().strip_prefix('[')?.strip_suffix(']')?).ok()
}

/// Whether a line would be read as something other than text: a comment, a header, or one of the
/// near misses that are refused.
fn is_special(line: &str) -> bool {
    line.starts_with('#') || header_of(line).is_some() || near_miss(line).is_some()
}

/// What to say of a line that is nearly a header: an address with one bracket, or a header as it
/// was written before the brackets.
fn near_miss(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    let one_bracket = match (trimmed.strip_prefix('['), trimmed.strip_suffix(']')) {
        (Some(_), Some(_)) => None,
        (Some(rest), None) | (None, Some(rest)) => Some(rest),
        (None, None) => None,
    };
    if let Some(address) = one_bracket.filter(|rest| parse_name(rest).is_ok()) {
        return Some(format!(
            "{} has one of its brackets missing: a string's header is written {}",
            quoted(trimmed),
            quoted(&format!("[{address}]"))
        ));
    }
    let old = trimmed.strip_prefix("## ")?;
    Some(match parse_name(old) {
        Ok(_) => format!(
            "{} is a header as it used to be written: it is now {}",
            quoted(trimmed),
            quoted(&format!("[{old}]"))
        ),
        Err(_) => format!(
            "{} is neither a header nor a comment: a header is an address in square brackets, such as [05.004], \
             and a comment starts with one hash",
            quoted(trimmed)
        ),
    })
}

/// A line of text as it is written in a file: one that would be read as something else, with or
/// without backslashes before it, gets a backslash before it.
fn written_line(line: &str) -> String {
    if is_special(line.trim_start_matches('\\')) { format!("\\{line}") } else { line.to_string() }
}

/// A line of a file as the text it stands for: the backslash `written_line` put there is dropped.
fn read_line(line: &str) -> &str {
    match line.strip_prefix('\\') {
        Some(rest) if is_special(rest.trim_start_matches('\\')) => rest,
        _ => line,
    }
}

/// A note as the comment lines that hold it, each ending in a line break.
fn comment(note: &str) -> String {
    note.lines().map(|line| if line.is_empty() { "#\n".to_string() } else { format!("# {line}\n") }).collect()
}

/// What to say in a file beside its strings: given a string's number, a note on that string, and
/// given none, a note for the head of the file.
pub type Notes<'a> = &'a dyn Fn(Option<u32>) -> Option<String>;

pub fn format_block(section: &str, texts: &BTreeMap<u32, String>) -> String {
    format_block_with(section, texts, &|_| None)
}

/// A section's file, with a comment at its head and under the header of each string that has a note.
pub fn format_block_with(section: &str, texts: &BTreeMap<u32, String>, notes: Notes) -> String {
    let digits = texts.keys().next_back().map_or(1, |last| last.to_string().len()).max(3);
    let mut content = notes(None).map_or_else(String::new, |note| comment(&note) + "\n");
    for (index, text) in texts {
        content += &format!("[{section}.{index:0digits$}]\n");
        content += &notes(Some(*index)).map_or_else(String::new, |note| comment(&note));
        content += &text.split('\n').map(written_line).collect::<Vec<_>>().join("\n");
        content += "\n\n";
    }
    content
}

/// Parse one file. When `section` is given, every header must name that section.
pub fn parse_block(content: &str, section: Option<&str>) -> Result<BTreeMap<u32, String>> {
    let content = content.replace("\r\n", "\n").replace('\r', "\n");
    let mut texts = BTreeMap::new();
    let mut current: Option<(u32, Vec<&str>)> = None;

    // Every string is followed by one blank line; the file's final newline adds one more empty item.
    fn finish(texts: &mut BTreeMap<u32, String>, index: u32, lines: &[&str], last: bool) -> Result<()> {
        let trailing = if last { 2 } else { 1 };
        if lines.len() < trailing + 1 || lines[lines.len() - trailing..].iter().any(|line| !line.is_empty()) {
            return script_error(format!("string {index:03} must be followed by a blank line"));
        }
        let text: Vec<&str> = lines[..lines.len() - trailing].iter().map(|line| read_line(line)).collect();
        texts.insert(index, text.join("\n"));
        Ok(())
    }

    for line in content.split('\n') {
        if is_comment(line) {
            continue;
        }
        // Nearly a header: taken for text it would end up in the game, or leave the string under
        // it joined to the one before.
        if let Some(message) = near_miss(line) {
            return script_error(message);
        }
        if let Some((named, index)) = header_of(line) {
            let address = line.trim_end().trim_start_matches('[').trim_end_matches(']');
            if let Some((previous, lines)) = current.take() {
                finish(&mut texts, previous, &lines, false)?;
            }
            if section.is_some_and(|section| section != named) {
                return script_error(format!(
                    "string {address} does not belong in the file for {}",
                    section.unwrap_or_default()
                ));
            }
            if texts.contains_key(&index) {
                return script_error(format!("string {address} appears twice"));
            }
            current = Some((index, Vec::new()));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        } else if !line.is_empty() {
            return script_error("text found before the first string header");
        }
    }
    if let Some((index, lines)) = current {
        finish(&mut texts, index, &lines, true)?;
    }
    Ok(texts)
}

/// Write the characters' names, one `Tag = name` line each, after an optional comment.
pub fn write_names(directory: &Path, names: &[(String, String)], header: &str) -> Result<()> {
    fs::create_dir_all(directory)?;
    let mut content: String = header.lines().map(|line| format!("# {line}\n")).collect();
    content.extend(names.iter().map(|(tag, name)| format!("{tag} = {name}\n")));
    Ok(fs::write(directory.join(NAMES_FILE), content)?)
}

/// Read `names.txt` if the pack has one. A character it leaves out keeps the game's own name.
pub fn read_names(directory: &Path) -> Result<Vec<(String, String)>> {
    let path = directory.join(NAMES_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<(String, String)> = Vec::new();
    for (number, line) in fs::read_to_string(path)?.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let number = number + 1;
        let pair = line.split_once('=').map(|(tag, name)| (tag.trim(), name.trim()));
        let Some((tag, name)) = pair.filter(|(tag, name)| !tag.is_empty() && !name.is_empty()) else {
            return script_error(format!("{NAMES_FILE} line {number}: expected 'Tag = name'"));
        };
        if names.iter().any(|(known, _)| known == tag) {
            return script_error(format!("{NAMES_FILE} line {number}: {tag} appears twice"));
        }
        names.push((tag.to_string(), name.to_string()));
    }
    Ok(names)
}

pub fn write_script(directory: &Path, script: &Script) -> Result<()> {
    write_script_with(directory, script, &|_, _| None)
}

/// What to say beside the strings of a script: asked for each section's head (no number) and for
/// each string.
pub type ScriptNotes<'a> = &'a dyn Fn(&str, Option<u32>) -> Option<String>;

/// Write a script with comments beside its strings.
pub fn write_script_with(directory: &Path, script: &Script, notes: ScriptNotes) -> Result<()> {
    fs::create_dir_all(directory)?;
    for (section, texts) in script {
        if !is_section(section) {
            return script_error(format!("{} cannot be used as a section name", quoted(section)));
        }
        let content = format_block_with(section, texts, &|number| notes(section, number));
        fs::write(directory.join(format!("{section}.txt")), content)?;
    }
    Ok(())
}

/// Read the script files of a folder: every file whose name ends in `.txt`, but `names.txt`.
///
/// A file whose name ends in `.TXT`, or `.Txt`, is refused rather than passed over: a translator who
/// saved `01.TXT` would otherwise find their strings gone without a word. It is not read as a
/// script file either, since two names that differ only so can both be in one folder on some
/// systems.
pub fn read_script(directory: &Path) -> Result<Script> {
    let mut script = Script::new();
    let mut paths: Vec<_> = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    paths.sort_by_key(|entry| entry.file_name());
    for entry in paths {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let extension = path.extension().map(|extension| extension.to_string_lossy().into_owned());
        match extension.as_deref() {
            Some("txt") => {}
            Some(other) if other.eq_ignore_ascii_case("txt") => {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                return script_error(format!(
                    "{name}: the name of a script file ends in .txt, in lowercase: rename it {stem}.txt"
                ));
            }
            _ => continue,
        }
        if name == NAMES_FILE {
            continue;
        }
        let section = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        if !is_section(&section) {
            return script_error(format!("{name}: the file name is not a section name"));
        }
        let block = parse_block(&fs::read_to_string(&path)?, Some(&section)).map_err(|error| match error {
            Error::Script(message) => Error::Script(format!("{name}: {message}")),
            other => other,
        })?;
        script.insert(section, block);
    }
    Ok(script)
}

pub fn read_pack(directory: &Path) -> Result<Pack> {
    Ok(Pack { texts: read_script(directory)?, names: read_names(directory)? })
}

pub fn write_pack(directory: &Path, pack: &Pack) -> Result<()> {
    write_pack_with(directory, pack, &|_, _| None)
}

/// Write a pack with comments beside its strings (see `write_script_with`).
pub fn write_pack_with(directory: &Path, pack: &Pack, notes: ScriptNotes) -> Result<()> {
    write_script_with(directory, &pack.texts, notes)?;
    if !pack.names.is_empty() {
        write_names(directory, &pack.names, NAMES_HEADER)?;
    }
    Ok(())
}

/// The strings of a pack that differ from the game's own, as (section, number, text), in order.
///
/// `original` gives a section's strings as the game has them, or None when there is no such section.
/// A pack can replace strings but not add any: games refer to them by number.
pub fn changes(pack: &Script, original: impl Fn(&str) -> Option<Vec<String>>) -> Result<Vec<(String, u32, String)>> {
    let mut found = Vec::new();
    for (section, items) in pack {
        let Some(strings) = original(section) else {
            return text_error(format!("{section} is not a part of this game's script"));
        };
        for (number, text) in items {
            let Some(own) = strings.get(*number as usize) else {
                return text_error(format!(
                    "string {} does not exist: {section} has {} strings",
                    string_name(section, *number),
                    strings.len()
                ));
            };
            if text != own {
                found.push((section.clone(), *number, text.clone()));
            }
        }
    }
    Ok(found)
}

/// The same as `changes`, for sections whose strings are numbered by where they are: `original`
/// gives a section's own strings by their numbers.
pub fn changes_by_place(
    pack: &Script,
    original: impl Fn(&str) -> Option<BTreeMap<u32, String>>,
) -> Result<Vec<(String, u32, String)>> {
    let mut found = Vec::new();
    for (section, items) in pack {
        let Some(strings) = original(section) else {
            return text_error(format!("{section} is not a part of this game's script"));
        };
        for (number, text) in items {
            let Some(own) = strings.get(number) else {
                return text_error(format!("string {} does not exist", string_name(section, *number)));
            };
            if text != own {
                found.push((section.clone(), *number, text.clone()));
            }
        }
    }
    Ok(found)
}

/// The same as `verify`, for sections whose strings are numbered by where they are.
pub fn verify_by_place(
    script: &Script,
    original: &BTreeMap<String, BTreeMap<u32, Vec<u8>>>,
    encode: impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<()> {
    for (section, items) in script {
        let none = BTreeMap::new();
        let strings = original.get(section).unwrap_or(&none);
        if !items.keys().eq(strings.keys()) {
            return text_error(format!(
                "section {section} does not hold the strings the game has, each under its number"
            ));
        }
        for (number, data) in strings {
            if &encode(&items[number])? != data {
                return text_error(format!(
                    "string {} does not survive a round trip through its script file",
                    string_name(section, *number)
                ));
            }
        }
    }
    Ok(())
}

/// Check that an extracted script encodes back to exactly the bytes the game has.
pub fn verify(
    script: &Script,
    original: &BTreeMap<String, Vec<Vec<u8>>>,
    encode: impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<()> {
    for (section, items) in script {
        let strings = original.get(section).map(Vec::as_slice).unwrap_or_default();
        if items.len() != strings.len() {
            return text_error(format!(
                "section {section} has {} strings where the game has {}",
                items.len(),
                strings.len()
            ));
        }
        for (number, data) in strings.iter().enumerate() {
            let number = number as u32;
            let same = match items.get(&number) {
                Some(text) => &encode(text)? == data,
                None => false,
            };
            if !same {
                return text_error(format!(
                    "string {} does not survive a round trip through its script file",
                    string_name(section, number)
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(pairs: &[(u32, &str)]) -> BTreeMap<u32, String> {
        pairs.iter().map(|(number, text)| (*number, (*text).to_string())).collect()
    }

    #[test]
    fn a_block_is_written_and_read_back() {
        let texts = block(&[(0, "first line\nsecond{PAGE}"), (4, ""), (5, "\nstarts and ends with a break\n")]);
        let written = format_block("05", &texts);
        assert!(written.starts_with("[05.000]\nfirst line\nsecond{PAGE}\n\n[05.004]\n\n\n[05.005]\n"));
        assert_eq!(parse_block(&written, Some("05")).unwrap(), texts);
        // The same file as another system would have written it.
        assert_eq!(parse_block(&written.replace('\n', "\r\n"), Some("05")).unwrap(), texts);
        assert_eq!(format_block("05", &block(&[(1234, "x")])), "[05.1234]\nx\n\n");
        // Spaces left after a header by an editor do not stop it being one.
        assert_eq!(parse_block("[05.000]  \nx\n\n", Some("05")).unwrap(), block(&[(0, "x")]));
    }

    #[test]
    fn a_comment_is_no_part_of_a_string() {
        let texts = block(&[(0, "first line\n\nafter a blank line"), (4, "another")]);
        // At the head of the file, under a header, inside a string, between two strings, at the end.
        let file = "# About this file.\n#\n# A second paragraph.\n\n[05.000]\n# About this string.\nfirst line\n\n\
                    # in the middle\nafter a blank line\n\n# before the next\n[05.004]\nanother\n\n# the last word\n";
        assert_eq!(parse_block(file, Some("05")).unwrap(), texts);
        // A hash with no space after it is text, and so is a line that only starts with a bracket,
        // or that has brackets round something that is no address.
        let plain = block(&[(0, "#1 on the list\n[Cress] nods.\n[aside]\n#"), (1, "x")]);
        assert_eq!(
            parse_block("[05.000]\n#1 on the list\n[Cress] nods.\n[aside]\n\\#\n\n[05.001]\nx\n\n", None).unwrap(),
            plain
        );
        // Text that would be read as a comment or a header is written with a backslash before it,
        // and so is text that starts with that backslash, so that each reads back as it was.
        let odd = block(&[(0, "# not a comment\n[05.001]\n\\# a backslash first\n\\\\[05.002]\n## 05.003"), (1, "#")]);
        let written = format_block("05", &odd);
        let expected =
            "[05.000]\n\\# not a comment\n\\[05.001]\n\\\\# a backslash first\n\\\\\\[05.002]\n\\## 05.003\n\n";
        assert!(written.starts_with(expected), "{written}");
        assert_eq!(parse_block(&written, Some("05")).unwrap(), odd);
    }

    #[test]
    fn notes_are_written_as_comments() {
        let texts = block(&[(0, "first"), (4, "second")]);
        let notes = |number: Option<u32>| match number {
            None => Some("About the file.\n\nIts second paragraph.".to_string()),
            Some(4) => Some("About string 4.".to_string()),
            Some(_) => None,
        };
        let written = format_block_with("05", &texts, &notes);
        assert_eq!(
            written,
            "# About the file.\n#\n# Its second paragraph.\n\n[05.000]\nfirst\n\n[05.004]\n# About string 4.\nsecond\n\n"
        );
        // They are for the reader: the strings read back as they were.
        assert_eq!(parse_block(&written, Some("05")).unwrap(), texts);
        let folder = tempfile::tempdir().unwrap();
        let script = Script::from([("05".to_string(), texts)]);
        write_script_with(folder.path(), &script, &|section, number| {
            notes(number).map(|note| format!("{section}: {note}"))
        })
        .unwrap();
        assert!(fs::read_to_string(folder.path().join("05.txt")).unwrap().starts_with("# 05: About the file.\n"));
        assert_eq!(read_script(folder.path()).unwrap(), script);
    }

    #[test]
    fn a_file_that_is_not_a_script_is_refused() {
        for (content, message) in [
            ("stray\n[05.000]\nx\n\n", "text found before the first string header"),
            ("[05.000]\nx\n[05.001]\ny\n\n", "string 000 must be followed by a blank line"),
            ("[05.000]\nx\n", "string 000 must be followed by a blank line"),
            ("[05.000]\nx\n\n[05.000]\ny\n\n", "string 05.000 appears twice"),
            ("[08.000]\nx\n\n", "string 08.000 does not belong in the file for 05"),
            // Nearly a header. Taken for text, each would end up in the game, and the string
            // under it would be joined to the one before.
            (
                "[05.000]\nx\n\n[05.001\ny\n\n",
                "'[05.001' has one of its brackets missing: a string's header is written '[05.001]'",
            ),
            (
                "[05.000]\nx\n\n05.001]\ny\n\n",
                "'05.001]' has one of its brackets missing: a string's header is written '[05.001]'",
            ),
            // A file from before the brackets, and a comment written the way a heading is elsewhere.
            ("## 05.000\nx\n\n", "'## 05.000' is a header as it used to be written: it is now '[05.000]'"),
            (
                "[05.000]\nx\n\n## check this\n[05.001]\ny\n\n",
                "'## check this' is neither a header nor a comment: a header is an address in square brackets, such \
                 as [05.004], and a comment starts with one hash",
            ),
        ] {
            assert_eq!(parse_block(content, Some("05")).unwrap_err().to_string(), message);
        }
    }

    #[test]
    fn a_pack_is_a_folder_of_files() {
        let folder = tempfile::tempdir().unwrap();
        let mut texts = Script::new();
        texts.insert("05".into(), block(&[(0, "a"), (7, "b\nc")]));
        texts.insert("menu".into(), block(&[(3, "d")]));
        let pack =
            Pack { texts, names: vec![("クレス".into(), "Cress".into()), ("ミント".into(), "Mint".into())] };
        write_pack(folder.path(), &pack).unwrap();
        assert_eq!(read_pack(folder.path()).unwrap(), pack);
        assert_eq!(pack.name("ミント"), Some("Mint"));
        let names = fs::read_to_string(folder.path().join(NAMES_FILE)).unwrap();
        assert!(names.starts_with("# On the left") && names.ends_with("クレス = Cress\nミント = Mint\n"));

        fs::write(folder.path().join("07.txt"), "[08.000]\nwrong file\n\n").unwrap();
        assert!(read_script(folder.path()).unwrap_err().to_string().starts_with("07.txt: "));
        fs::remove_file(folder.path().join("07.txt")).unwrap();
        // A script file whose name ends in capitals is refused, not passed over, and not read.
        for name in ["01.TXT", "01.Txt", "06.tXt"] {
            fs::write(folder.path().join(name), "[01.000]\nmine\n\n").unwrap();
            let stem = name.split('.').next().unwrap();
            assert_eq!(
                read_script(folder.path()).unwrap_err().to_string(),
                format!("{name}: the name of a script file ends in .txt, in lowercase: rename it {stem}.txt")
            );
            assert!(read_pack(folder.path()).is_err());
            fs::remove_file(folder.path().join(name)).unwrap();
        }
        // Other files a pack holds are not script files, whatever their endings.
        for name in ["NOTES.md", "README", "font.TTF", "pack.toml", "notes.txt.bak"] {
            fs::write(folder.path().join(name), "anything").unwrap();
        }
        assert_eq!(read_pack(folder.path()).unwrap(), pack);
        fs::write(folder.path().join(NAMES_FILE), "クレス = Cress\nクレス = Cless\n").unwrap();
        assert_eq!(read_names(folder.path()).unwrap_err().to_string(), "names.txt line 2: クレス appears twice");
        fs::write(folder.path().join(NAMES_FILE), "no separator\n").unwrap();
        assert_eq!(read_names(folder.path()).unwrap_err().to_string(), "names.txt line 1: expected 'Tag = name'");
    }

    #[test]
    fn addresses_are_written_and_read() {
        assert_eq!(string_name("06307", 4), "06307.004");
        assert_eq!(parse_name("06307.004").unwrap(), ("06307".to_string(), 4));
        assert_eq!(parse_name("menu.1234").unwrap(), ("menu".to_string(), 1234));
        for bad in ["", "menu", "menu.", ".004", "me nu.004", "menu.4a"] {
            assert!(parse_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn only_what_differs_from_the_game_is_a_change() {
        let original = |section: &str| (section == "05").then(|| vec!["a".to_string(), "b".to_string()]);
        let mut pack = Script::new();
        pack.insert("05".into(), block(&[(0, "a"), (1, "B")]));
        assert_eq!(changes(&pack, original).unwrap(), [("05".to_string(), 1, "B".to_string())]);
        pack.insert("05".into(), block(&[(2, "c")]));
        assert_eq!(changes(&pack, original).unwrap_err().to_string(), "string 05.002 does not exist: 05 has 2 strings");
        pack.insert("shop".into(), block(&[(0, "c")]));
        pack.remove("05");
        assert_eq!(changes(&pack, original).unwrap_err().to_string(), "shop is not a part of this game's script");
    }

    #[test]
    fn a_script_must_encode_back_to_the_game_s_bytes() {
        let encode = |text: &str| Ok(text.as_bytes().to_vec());
        let original: BTreeMap<String, Vec<Vec<u8>>> = [("05".to_string(), vec![b"a".to_vec(), b"b".to_vec()])].into();
        let mut script = Script::new();
        script.insert("05".into(), block(&[(0, "a"), (1, "b")]));
        verify(&script, &original, encode).unwrap();
        script.insert("05".into(), block(&[(0, "a"), (1, "x")]));
        assert_eq!(
            verify(&script, &original, encode).unwrap_err().to_string(),
            "string 05.001 does not survive a round trip through its script file"
        );
        script.insert("05".into(), block(&[(0, "a")]));
        assert_eq!(
            verify(&script, &original, encode).unwrap_err().to_string(),
            "section 05 has 1 strings where the game has 2"
        );
    }
}
