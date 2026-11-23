//! What every game's adapter shares about text: the tag language of a script, and control codes.
//!
//! In a script file, anything that is not a character the game draws is written as a tag: `{NAME}`,
//! or `{NAME:AA BB}` with parameter bytes in hexadecimal. A newline is a line break. What the tags
//! are is up to each game; how they are written, split and parsed is the same everywhere.

use std::collections::BTreeMap;

use crate::error::{Result, text_error};

/// One piece of a script: a character the game draws (a newline being a line break), or a tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Char(char),
    /// `raw` is the tag as written, braces included; `value` is what follows the colon, if there is one.
    Tag {
        raw: &'a str,
        name: &'a str,
        value: Option<&'a str>,
    },
}

impl Token<'_> {
    /// The token as it is written in a script.
    pub fn text(&self) -> String {
        match self {
            Token::Char(char) => char.to_string(),
            Token::Tag { raw, .. } => (*raw).to_string(),
        }
    }

    /// The name of a tag, or an empty string for a character.
    pub fn name(&self) -> &str {
        match self {
            Token::Char(_) => "",
            Token::Tag { name, .. } => name,
        }
    }
}

/// The tag that starts at the beginning of `text`, if one does: a name, then optionally a colon and
/// hexadecimal digits and spaces, in braces. A `{` that starts no tag is a character like any other.
pub fn tag_at(text: &str) -> Option<Token<'_>> {
    let rest = text.strip_prefix('{')?;
    let name_end = rest.find(|c: char| c.is_whitespace() || matches!(c, '{' | '}' | ':'))?;
    if name_end == 0 {
        return None;
    }
    let name = &rest[..name_end];
    let after = &rest[name_end..];
    if after.starts_with('}') {
        return Some(Token::Tag { raw: &text[..name_end + 2], name, value: None });
    }
    let value_part = after.strip_prefix(':')?;
    let value_end = value_part.find(|c: char| !(c.is_ascii_hexdigit() || c == ' '))?;
    if !value_part[value_end..].starts_with('}') {
        return None;
    }
    Some(Token::Tag { raw: &text[..name_end + value_end + 3], name, value: Some(&value_part[..value_end]) })
}

/// Split a script into tags and single characters, in order.
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(char) = rest.chars().next() {
        let token = if char == '{' { tag_at(rest) } else { None }.unwrap_or(Token::Char(char));
        rest = &rest[match token {
            Token::Char(char) => char.len_utf8(),
            Token::Tag { raw, .. } => raw.len(),
        }..];
        out.push(token);
    }
    out
}

/// The characters of `text` that are drawn, without tags and line breaks, each once, in order.
pub fn plain_characters(text: &str) -> Vec<char> {
    let mut found = Vec::new();
    for token in tokens(text) {
        if let Token::Char(char) = token
            && char != '\n'
            && !found.contains(&char)
        {
            found.push(char);
        }
    }
    found
}

/// Read a character table: one `KEY=character` line each, `#` starting a comment line. What the key
/// means, a code or a glyph number, is up to the game.
pub fn read_table(content: &str) -> Vec<(&str, &str)> {
    content
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split_once('=').unwrap_or((line, "")))
        .collect()
}

/// Bytes as a tag shows them: `0A 1B`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ")
}

/// The bytes a tag's value stands for: pairs of hexadecimal digits, with spaces allowed between pairs.
pub fn from_hex(value: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut digits = value.chars().peekable();
    while let Some(&char) = digits.peek() {
        if char == ' ' {
            digits.next();
            continue;
        }
        let high = digits.next()?.to_digit(16)?;
        let low = digits.next()?.to_digit(16)?;
        out.push((high << 4 | low) as u8);
    }
    Some(out)
}

/// Text in quotes, as a message shows what a translator typed: `'o'`, or `"'"` for the quote itself.
///
/// This is how the Python reference writes a string in its messages, and the two must say the same.
/// Single quotes, unless the text has one and no double quote. A backslash, the quote in use, a
/// control character, any space but the plain one, and a character that cannot be seen are written
/// as escapes (`\n`, `\x7f`, `　`). Telling whether a code point is assigned at all takes the
/// Unicode tables, which are not here: an unassigned one is shown as it is.
pub fn quoted(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for char in text.chars() {
        match char {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if char == quote => {
                out.push('\\');
                out.push(char);
            }
            ' '..='~' => out.push(char),
            _ if char.is_ascii() || !can_be_seen(char) => match char as u32 {
                code @ ..=0xFF => out.push_str(&format!("\\x{code:02x}")),
                code @ ..=0xFFFF => out.push_str(&format!("\\u{code:04x}")),
                code => out.push_str(&format!("\\U{code:08x}")),
            },
            _ => out.push(char),
        }
    }
    out.push(quote);
    out
}

/// Whether a character outside ASCII leaves a mark on screen: not a control or formatting
/// character, not a space or a line separator, and not one set aside for private use.
fn can_be_seen(char: char) -> bool {
    // The formatting characters of Unicode: soft hyphen, zero-width marks, direction marks and such.
    const FORMATTING: [(u32, u32); 21] = [
        (0xAD, 0xAD),
        (0x600, 0x605),
        (0x61C, 0x61C),
        (0x6DD, 0x6DD),
        (0x70F, 0x70F),
        (0x890, 0x891),
        (0x8E2, 0x8E2),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0001, 0xE0001),
        (0xE0020, 0xE007F),
    ];
    const PRIVATE_USE: [(u32, u32); 3] = [(0xE000, 0xF8FF), (0xF0000, 0xFFFFD), (0x100000, 0x10FFFD)];
    let code = char as u32;
    let within = |ranges: &[(u32, u32)]| ranges.iter().any(|(first, last)| (*first..=*last).contains(&code));
    !(char.is_control() || char.is_whitespace() || within(&FORMATTING) || within(&PRIVATE_USE))
}

/// A game's control codes: how many parameter bytes each takes, and the names some of them have.
///
/// A code with a name is written `{NAME}`; one without is written as its number, `{1D}`. Either way
/// its parameters follow a colon: `{1D:10}`. Both forms are accepted when parsing.
pub struct ControlCodes {
    parameters: BTreeMap<u32, usize>,
    names: BTreeMap<u32, &'static str>,
}

impl ControlCodes {
    pub fn new(parameters: &[(u32, usize)], names: &[(u32, &'static str)]) -> Self {
        ControlCodes { parameters: parameters.iter().copied().collect(), names: names.iter().copied().collect() }
    }

    /// How many parameter bytes follow `code`.
    pub fn count(&self, code: u32) -> usize {
        self.parameters.get(&code).copied().unwrap_or(0)
    }

    /// The code a name stands for, if it is one of the named ones.
    pub fn code(&self, name: &str) -> Option<u32> {
        self.names.iter().find(|(_, known)| **known == name).map(|(code, _)| *code)
    }

    pub fn tag(&self, code: u32, parameters: &[u8]) -> String {
        let name = self.names.get(&code).map_or_else(|| format!("{code:02X}"), |name| (*name).to_string());
        if parameters.is_empty() { format!("{{{name}}}") } else { format!("{{{name}:{}}}", hex(parameters)) }
    }

    /// The code and parameter bytes of a tag, given its name and what follows the colon.
    pub fn parse(&self, name: &str, value: &str) -> Result<(u32, Vec<u8>)> {
        let shown = if value.is_empty() { format!("{{{name}}}") } else { format!("{{{name}:{value}}}") };
        let code = self.code(name).or_else(|| u32::from_str_radix(name, 16).ok());
        let (Some(code), Some(parameters)) = (code, from_hex(value)) else {
            return text_error(format!("unknown tag {shown}"));
        };
        if parameters.len() != self.count(code) {
            return text_error(format!("{shown} needs {} parameter bytes", self.count(code)));
        }
        Ok((code, parameters))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_splits_into_characters_and_tags() {
        let found = tokens("あ{NAME}b{1D:10 2f}\n{not a tag}{:x}{X:zz}");
        let shown: Vec<String> = found.iter().map(Token::text).collect();
        assert_eq!(shown[..5], ["あ", "{NAME}", "b", "{1D:10 2f}", "\n"]);
        assert_eq!(found[1], Token::Tag { raw: "{NAME}", name: "NAME", value: None });
        assert_eq!(found[3], Token::Tag { raw: "{1D:10 2f}", name: "1D", value: Some("10 2f") });
        // A brace that starts no tag is a character, and so is everything after it.
        assert_eq!(shown[5..].concat(), "{not a tag}{:x}{X:zz}");
        assert!(found[5..].iter().all(|token| matches!(token, Token::Char(_))));
        assert_eq!(tokens("{A:}")[0], Token::Tag { raw: "{A:}", name: "A", value: Some("") });
    }

    #[test]
    fn plain_characters_leave_out_tags_and_line_breaks() {
        assert_eq!(plain_characters("ab{NAME}a\nc{1D:10}"), ['a', 'b', 'c']);
    }

    #[test]
    fn control_codes_are_written_and_read_back() {
        let codes = ControlCodes::new(&[(0x1D, 1), (0x3F, 2)], &[(0x08, "WAIT"), (0x3F, "FACE")]);
        assert_eq!(codes.tag(0x08, &[]), "{WAIT}");
        assert_eq!(codes.tag(0x1D, &[0x10]), "{1D:10}");
        assert_eq!(codes.tag(0x3F, &[7, 0xAB]), "{FACE:07 AB}");
        assert_eq!(codes.parse("WAIT", "").unwrap(), (0x08, vec![]));
        assert_eq!(codes.parse("3F", "07 ab").unwrap(), (0x3F, vec![7, 0xAB]));
        assert_eq!(codes.parse("FACE", "07AB").unwrap(), (0x3F, vec![7, 0xAB]));
        assert_eq!(codes.parse("BOGUS", "").unwrap_err().to_string(), "unknown tag {BOGUS}");
        assert_eq!(codes.parse("1D", "1 0").unwrap_err().to_string(), "unknown tag {1D:1 0}");
        assert_eq!(codes.parse("1D", "").unwrap_err().to_string(), "{1D} needs 1 parameter bytes");
    }

    #[test]
    fn text_is_quoted_as_the_python_reference_quotes_it() {
        assert_eq!(quoted("o"), "'o'");
        assert_eq!(quoted("П"), "'П'");
        assert_eq!(quoted("'"), "\"'\"");
        assert_eq!(quoted("'\""), "'\\'\"'");
        assert_eq!(quoted("{a\\b\n"), "'{a\\\\b\\n'");
        assert_eq!(quoted("\u{7f}\u{1}\u{a0}\u{3000}\u{200b}\u{1F600}"), "'\\x7f\\x01\\xa0\\u3000\\u200b\u{1F600}'");
        assert_eq!(quoted("\u{E0001}"), "'\\U000e0001'");
    }

    #[test]
    fn a_table_is_keys_and_characters() {
        assert_eq!(read_table("# kana\n010=あ\n\n011==\n"), [("010", "あ"), ("011", "=")]);
    }
}
