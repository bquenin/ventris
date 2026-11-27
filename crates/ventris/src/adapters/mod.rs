//! One adapter per game: where its script lives, how it is encoded, and how to put it back.
//!
//! An adapter is the code for one game. It is compiled into ventris: there is no loading one
//! from elsewhere, and a new game arrives as a pull request that adds a module here, a folder of its
//! own, and one line to `ALL` below. A translation pack is not code and lives apart: once a game's adapter is in, anyone
//! can write packs for it. `crate::adapter` is the contract, and says what an adapter promises to
//! those packs.
//!
//! # Writing an adapter for a new game
//!
//! `docs/adding-a-game.md` is the guide: how to take a game apart, the two adapters here as worked
//! examples, how to check an adapter and what its pull request shows. In short, add the folder
//! `adapters/<game>_<console>/`: a `mod.rs` whose module comment describes what was found for a
//! reader who has not seen the game, with one empty type that implements `Adapter` and a
//! `pub static ADAPTER` of that type; its tests in `tests.rs`; and beside them any assembly or table
//! the module includes. Add it to `ALL`; run `ventris check <image>`; and look at the screen. What each
//! part of the contract says:
//!
//! | What | Says |
//! | --- | --- |
//! | `id`, `name`, `platform`, `source_language` | which game this is |
//! | `images` | each release it reads: its name, SHA-1 and size, and what the image says it is (`label`, `version`: a disc's serial, a cartridge's title), which `ventris check` confirms, and its name in the catalogue of dumps for its console (`catalogue`) |
//! | `refuses` | the files of its game it knows and does not read, other releases and releases with a known patch applied, so that a refused file is told what it is |
//! | `kinds` | its bodies of text, and what a pack can do with each |
//! | `extract` | every string, by section and number |
//! | `verify` | that the extracted script encodes back to the game's bytes |
//! | `note` | what a translator should know about a file or a string that its text does not show |
//! | `write` | the game with the pack's strings in it |
//! | `glyph_size`, `glyph_count`, `glyph`, `code_at` | the font, for drawing it as a picture |
//! | `unmapped` | which codes in a string stand for characters with no table entry |
//!
//! An adapter that only extracts is a whole one: leave `write` out, and `insert` off its kinds.
//!
//! # What an adapter reuses
//!
//! What an adapter has to work out is what differs from game to game. The rest is shared, and an
//! adapter is expected to use it and not write it again:
//!
//! | Module | Holds |
//! | --- | --- |
//! | `text` | the tag language (`{NAME:AA BB}`), character tables, control codes with parameters |
//! | `script` | which strings a pack changes, and the round-trip check |
//! | `fonts` | glyphs for characters the game's font lacks, and places in the font for them |
//! | `layout` | breaking lines at spaces to a width in pixels |
//! | `placement` | "where it was if it fits, otherwise the expanded area": the caller only redirects |
//! | `codecs` | compression schemes, when the text is compressed |
//! | `platforms` | what is about a console: mapping, headers and checksums, expansion, an assembler for injected code, reading and rebuilding a disc image |
//!
//! Game files never go into the repository: an adapter holds what was learned about a game, not
//! the game. A table of the game's characters is knowledge; its script, its font and its code are
//! not.

use crate::adapter::Adapter;

pub mod god_ps1;
pub mod goemon_nds;
pub mod srwz_ps2;
pub mod tod2_ps2;
pub mod top_sfc;

/// Every game ventris knows.
pub static ALL: &[&dyn Adapter] =
    &[&top_sfc::ADAPTER, &tod2_ps2::ADAPTER, &srwz_ps2::ADAPTER, &goemon_nds::ADAPTER, &god_ps1::ADAPTER];

/// The adapter for the game whose image has this SHA-1.
pub fn find(sha1: &str) -> Option<&'static dyn Adapter> {
    ALL.iter().copied().find(|adapter| adapter.images().iter().any(|image| image.sha1 == sha1))
}

pub fn by_id(id: &str) -> Option<&'static dyn Adapter> {
    ALL.iter().copied().find(|adapter| adapter.id() == id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::adapter::declaration_faults;

    use super::*;

    #[test]
    fn the_registry_holds_adapters_that_declare_themselves_properly() {
        assert_eq!(
            ALL.iter().map(|adapter| adapter.id()).collect::<Vec<_>>(),
            ["top-sfc", "tod2-ps2", "srwz-ps2", "goemon-nds", "god-ps1"]
        );
        let same = |found: Option<&dyn Adapter>, adapter: &dyn Adapter| {
            found.is_some_and(|found| crate::adapter::same_adapter(found, adapter))
        };
        for adapter in ALL.iter().copied() {
            assert_eq!(declaration_faults(adapter), Vec::<String>::new());
            assert!(same(by_id(adapter.id()), adapter));
            assert!(adapter.images().iter().all(|image| same(find(image.sha1), adapter)));
        }
        assert!(find(&"0".repeat(40)).is_none() && by_id("no-such-game").is_none());
        // No two adapters share an id, or claim the same image.
        let ids: BTreeSet<&str> = ALL.iter().map(|adapter| adapter.id()).collect();
        let images: BTreeSet<&str> = ALL.iter().flat_map(|adapter| adapter.images()).map(|image| image.sha1).collect();
        assert_eq!(ids.len(), ALL.len());
        assert_eq!(images.len(), ALL.iter().map(|adapter| adapter.images().len()).sum::<usize>());
    }
}
