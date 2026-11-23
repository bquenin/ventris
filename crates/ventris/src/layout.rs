//! Fitting text into a game's text box.

use crate::error::Result;
use crate::text::{Token, tag_at};

/// The tag that starts a new page in most games, for `wrap_lines`.
pub const PAGE_TAGS: &[&str] = &["PAGE"];

/// Break lines that are too wide for the box, at spaces.
///
/// `advance` gives the width of a character in pixels, and may refuse a character the game cannot
/// draw. `tag_width` gives the room a tag's text takes, by the tag's name and what follows its
/// colon, if anything does (which party member a name tag names, say). A tag in `page_tags`
/// starts a new page, which puts the pen back at the left edge.
///
/// A character may start as long as the pen is left of `limit`. A line with no space in it is left
/// alone: games wrap text themselves when the box is full, which suits scripts written without
/// spaces. Line breaks already in the text are kept.
pub fn wrap_lines(
    text: &str,
    advance: &dyn Fn(char) -> Result<usize>,
    limit: usize,
    tag_width: &dyn Fn(&str, Option<&str>) -> usize,
    page_tags: &[&str],
) -> Result<String> {
    let mut pieces = Vec::new();
    for line in text.split('\n') {
        wrap_line(line, advance, limit, tag_width, page_tags, &mut pieces)?;
    }
    Ok(pieces.join("\n"))
}

/// Break one line, adding what it becomes to `pieces`.
fn wrap_line<'a>(
    line: &'a str,
    advance: &dyn Fn(char) -> Result<usize>,
    limit: usize,
    tag_width: &dyn Fn(&str, Option<&str>) -> usize,
    page_tags: &[&str],
    pieces: &mut Vec<&'a str>,
) -> Result<()> {
    let mut start = 0; // where the piece being measured begins
    let mut width = 0; // its width so far
    let mut space = None; // position of its last space, the place it can be broken
    let mut after_space = 0; // width of what follows that space
    let mut position = 0;
    while let Some(char) = line[position..].chars().next() {
        if char == '{'
            && let Some(Token::Tag { raw, name, value }) = tag_at(&line[position..])
        {
            if page_tags.contains(&name) {
                width = 0;
                space = None;
            } else {
                let step = tag_width(name, value);
                width += step;
                after_space += step;
            }
            position += raw.len();
            continue;
        }
        if char == ' ' {
            if width >= limit {
                // the space itself falls off the end: break here
                pieces.push(&line[start..position]);
                start = position + 1;
                width = 0;
                space = None;
            } else {
                space = Some(position);
                after_space = 0;
                width += advance(char)?;
            }
            position += 1;
            continue;
        }
        if width >= limit
            && let Some(at) = space
        {
            pieces.push(&line[start..at]);
            start = at + 1;
            width = after_space;
            space = None;
        }
        let step = advance(char)?;
        width += step;
        after_space += step;
        position += char.len_utf8();
    }
    pieces.push(&line[start..]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten pixels a character and no tag that takes room, unless a test says otherwise.
    fn wrap(text: &str, limit: usize, names: &[(&str, usize)]) -> String {
        let tag_width =
            |tag: &str, _: Option<&str>| names.iter().find(|(name, _)| *name == tag).map_or(0, |(_, width)| *width);
        wrap_lines(text, &|_| Ok(10), limit, &tag_width, PAGE_TAGS).unwrap()
    }

    #[test]
    fn lines_break_at_spaces() {
        // A 50-pixel line: a character may start while the pen is left of 50.
        assert_eq!(wrap("aaa bbb ccc", 50, &[]), "aaa\nbbb\nccc");
        assert_eq!(wrap("aa bb cc dd", 50, &[]), "aa bb\ncc dd");
        assert_eq!(wrap("aaaa b", 50, &[]), "aaaa\nb");
        // Five characters fit exactly; a sixth does not, and with no space to break at the line is left alone.
        assert_eq!(wrap("aaaaa", 50, &[]), "aaaaa");
        assert_eq!(wrap("aaaaaaaaaaaa", 50, &[]), "aaaaaaaaaaaa");
        assert_eq!(wrap("aaaaaaaa bb", 50, &[]), "aaaaaaaa\nbb");
        // Breaks already in the text are kept, and each line is measured on its own.
        assert_eq!(wrap("aa\nbb cc dd ee", 50, &[]), "aa\nbb cc\ndd ee");
        assert_eq!(wrap("", 50, &[]), "");
        assert_eq!(wrap("aa bb\n", 50, &[]), "aa bb\n");
        // Characters of more than one byte are measured one by one, like any other.
        assert_eq!(wrap("ああ いい うう ええ", 50, &[]), "ああ いい\nうう ええ");
    }

    #[test]
    fn tags_are_measured() {
        // Tags with no width are ignored; a page break starts the measuring again.
        assert_eq!(wrap("{SPEED:10}aa bb{COLOR:02} cc", 50, &[]), "{SPEED:10}aa bb{COLOR:02}\ncc");
        assert_eq!(wrap("aa bb{PAGE}cc dd", 50, &[]), "aa bb{PAGE}cc dd");
        assert_eq!(wrap("aa bb{PAGE}\ncc dd ee", 50, &[]), "aa bb{PAGE}\ncc dd\nee");
        // A tag that stands for text takes the room it is given.
        assert_eq!(wrap("{クレス} aa bb", 100, &[("クレス", 50)]), "{クレス} aa\nbb");
        assert_eq!(wrap("{クレス} aa bb", 200, &[("クレス", 50)]), "{クレス} aa bb");
        assert_eq!(wrap("{クレス} aa bb", 100, &[("クレス", 10)]), "{クレス} aa bb");
        // A brace that starts no tag is a character like any other.
        assert_eq!(wrap("{a b} cc dd", 50, &[]), "{a b}\ncc dd");
    }

    #[test]
    fn a_character_the_game_cannot_draw_stops_the_wrapping() {
        let advance = |char: char| if char == 'x' { crate::error::text_error("no x") } else { Ok(10) };
        assert_eq!(wrap_lines("aa x", &advance, 50, &|_, _| 0, PAGE_TAGS).unwrap_err().to_string(), "no x");
    }
}
