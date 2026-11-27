//! Byte pairs: two characters written with one byte, for a game whose text has bytes to spare.
//!
//! A game that writes a character a byte often has byte values its text never uses, more so in a
//! translation into an alphabet of a few dozen letters. Each such byte can stand for two characters
//! that often follow one another, which the game's code then draws one after the other: "th", "e ",
//! "in". The pairs are chosen from the text a build writes (`choose`), so each pack gets its own,
//! and a string is then written with as few bytes as they allow (`write`). Pairs are never nested:
//! a byte is one character or two.

use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;

/// What one byte of a written sequence stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit<T> {
    One(T),
    Two(T, T),
}

/// How often a pair occurs, and where the occurrence counted last ended: its sequence and place.
type Count = (usize, Option<(usize, usize)>);

/// Up to `most` pairs that save the most bytes in `sequences`, the best first. Each round takes the
/// pair that occurs most often where neither of its characters is already part of a pair taken
/// before, counting overlapping occurrences once ("eee" has one "ee"), and marks those occurrences
/// from the left; ties go to the lower pair. A pair is taken while it occurs at least once, as a
/// byte left over saves nothing. The same sequences always give the same pairs, in the same order.
pub fn choose<T: Copy + Ord + Hash>(sequences: &[Vec<T>], most: usize) -> Vec<(T, T)> {
    // Whether each character of each sequence is already part of a pair.
    let mut taken: Vec<Vec<bool>> = sequences.iter().map(|sequence| vec![false; sequence.len()]).collect();
    let mut chosen = Vec::new();
    while chosen.len() < most {
        let mut counts: HashMap<(T, T), Count> = HashMap::new();
        for (index, sequence) in sequences.iter().enumerate() {
            for at in 1..sequence.len() {
                if taken[index][at - 1] || taken[index][at] {
                    continue;
                }
                let (count, last) = counts.entry((sequence[at - 1], sequence[at])).or_insert((0, None));
                // The same pair overlapping the one counted just before ("eee") is not counted again.
                if *last != Some((index, at - 1)) {
                    *count += 1;
                    *last = Some((index, at));
                }
            }
        }
        let best = counts.into_iter().max_by(|(pair, (count, _)), (other, (other_count, _))| {
            count.cmp(other_count).then_with(|| other.cmp(pair))
        });
        let Some((pair, _)) = best else { break };
        for (index, sequence) in sequences.iter().enumerate() {
            let mut at = 1;
            while at < sequence.len() {
                if !taken[index][at - 1] && !taken[index][at] && (sequence[at - 1], sequence[at]) == pair {
                    (taken[index][at - 1], taken[index][at]) = (true, true);
                    at += 2;
                } else {
                    at += 1;
                }
            }
        }
        chosen.push(pair);
    }
    chosen
}

/// A sequence written with as few bytes as `pairs` allow: the fewest units, and of two ways that
/// take as many, the one that pairs the earlier characters.
pub fn write<T: Copy + Ord>(sequence: &[T], pairs: &BTreeSet<(T, T)>) -> Vec<Unit<T>> {
    // fewest[at]: the fewest units that write the sequence from `at` on.
    let length = sequence.len();
    let mut fewest = vec![0usize; length + 1];
    for at in (0..length).rev() {
        fewest[at] = fewest[at + 1] + 1;
        if at + 1 < length && pairs.contains(&(sequence[at], sequence[at + 1])) {
            fewest[at] = fewest[at].min(fewest[at + 2] + 1);
        }
    }
    let mut out = Vec::with_capacity(fewest[0]);
    let mut at = 0;
    while at < length {
        let paired = at + 1 < length && pairs.contains(&(sequence[at], sequence[at + 1]));
        if paired && fewest[at + 2] + 1 == fewest[at] {
            out.push(Unit::Two(sequence[at], sequence[at + 1]));
            at += 2;
        } else {
            out.push(Unit::One(sequence[at]));
            at += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn the_pairs_that_occur_most_are_chosen_first_and_never_overlap() {
        let sequences = [chars("that that"), chars("this")];
        // "th" three times, then "at" twice; "ha", twice too, has lost its "h" to "th".
        let pairs = choose(&sequences, 3);
        assert_eq!(pairs, [('t', 'h'), ('a', 't'), ('i', 's')]);
        // The same text gives the same pairs.
        assert_eq!(choose(&sequences, 3), pairs);
    }

    #[test]
    fn overlapping_occurrences_count_once() {
        // "eeee" holds two "ee" that do not overlap, "abab" two "ab"; ties go to the lower pair.
        let pairs = choose(&[chars("eeee"), chars("abab")], 1);
        assert_eq!(pairs, [('a', 'b')]);
        let pairs = choose(&[chars("eeeee"), chars("abab")], 1);
        assert_eq!(pairs, [('a', 'b')]);
        let pairs = choose(&[chars("eeeeee"), chars("abab")], 1);
        assert_eq!(pairs, [('e', 'e')]);
    }

    #[test]
    fn choosing_stops_when_no_pair_is_left() {
        assert_eq!(choose(&[chars("ab")], 10), [('a', 'b')]);
        assert!(choose(&[chars("a"), Vec::new()], 10).is_empty());
        assert!(choose(&[chars("abc")], 0).is_empty());
    }

    #[test]
    fn a_sequence_is_written_in_the_fewest_bytes() {
        let pairs = BTreeSet::from([('a', 'b'), ('b', 'c'), ('c', 'd')]);
        // Pairing "bc" first would leave "a" and "d" alone: three bytes where two do.
        assert_eq!(write(&chars("abcd"), &pairs), [Unit::Two('a', 'b'), Unit::Two('c', 'd')]);
        // Of two ways as short, the earlier characters are paired.
        assert_eq!(write(&chars("abc"), &pairs), [Unit::Two('a', 'b'), Unit::One('c')]);
        assert_eq!(write(&chars("xbcx"), &pairs), [Unit::One('x'), Unit::Two('b', 'c'), Unit::One('x')]);
        assert!(write(&[] as &[char], &pairs).is_empty());
    }
}
