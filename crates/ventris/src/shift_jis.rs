//! Shift-JIS, as far as games need it: which character a two-byte code stands for.
//!
//! Games of Japanese consoles often keep lists of their characters as Shift-JIS, even when their
//! own text is encoded another way. The table beside this file holds the two-byte codes of
//! JIS X 0208 and nothing else: not the single bytes (ASCII and half-width kana), and none of the
//! additions later variants made. It is the mapping of the `shift_jis` codec of the Python
//! reference, taken from that codec code by code, so that both read a game's list the same way.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::text::read_table;

static CHARACTERS: LazyLock<HashMap<u16, char>> = LazyLock::new(|| {
    read_table(include_str!("shift_jis.tbl"))
        .into_iter()
        .map(|(code, text)| {
            let code = u16::from_str_radix(code, 16).unwrap_or_else(|_| panic!("shift_jis.tbl: {code} is no code"));
            let mut letters = text.chars();
            let (Some(char), None) = (letters.next(), letters.next()) else {
                panic!("shift_jis.tbl: code {code:04X} is not one character");
            };
            (code, char)
        })
        .collect()
});

/// The character two bytes stand for together, or None when they are not one two-byte code.
pub fn character(lead: u8, trail: u8) -> Option<char> {
    CHARACTERS.get(&u16::from_be_bytes([lead, trail])).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_bytes_are_one_character_of_jis_x_0208_or_nothing() {
        assert_eq!(CHARACTERS.len(), 6879);
        assert_eq!(
            (character(0x81, 0x40), character(0x82, 0xA0), character(0x8A, 0xBF)),
            (Some('　'), Some('あ'), Some('漢'))
        );
        assert_eq!((character(0x88, 0x9F), character(0xEA, 0xA4)), (Some('亜'), Some('熙'))); // the first and last kanji
        // Where the variants of Shift-JIS disagree, this is the standard's reading.
        assert_eq!((character(0x81, 0x5F), character(0x81, 0x60)), (Some('＼'), Some('〜')));
        // Single bytes, a second byte no code has, and the rows later variants added.
        for (lead, trail) in [(0x41, 0x42), (0xB1, 0xB2), (0x82, 0x7F), (0x82, 0x3F), (0x87, 0x40), (0xED, 0x40)] {
            assert_eq!(character(lead, trail), None, "{lead:02X}{trail:02X}");
        }
    }
}
