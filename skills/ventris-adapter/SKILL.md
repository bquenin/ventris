---
name: ventris-adapter
description: Add support for a new game to ventris by writing an adapter, the Rust module that extracts a game's script as UTF-8 text and builds the game with a translation in it. Covers finding what already exists about the game, reverse engineering where its text is and how it is encoded, compressed and drawn, the adapter contract, tests, `ventris check`, looking at the screen, and the pull request. Use when the user wants ventris to read or translate a game it does not support yet, is taking a game's text apart for ventris, or is preparing an adapter pull request to bquenin/ventris. Not for translating a game that already has an adapter (use ventris-translate).
---

# Adding a game to ventris

You are helping your user write an adapter: the code for one game, a Rust module compiled into ventris,
that says where the game's text is, extracts it, and builds the game with a translation's strings in it.
The ventris repository is <https://github.com/bquenin/ventris>. Paths below are paths in it.

## Where the facts are

Read these before you study the game, and go back to them rather than guessing. This file gives the
procedure and its guardrails; the facts are there, and they win where the two differ.

- `CONTRIBUTING.md`: the ground rules, the checks a pull request passes, what an adapter's pull request
  shows.
- `docs/adding-a-game.md`: the guide, in the order the work is done, with the workbench's tools, the
  emulators, the two existing adapters as worked examples, and Scope.
- `crates/ventris/src/adapter.rs`: the contract. Its module comment states the three promises an adapter
  keeps; the comments on `Adapter` and `TextKind` say what each part means.
- The top of `crates/ventris/src/adapters/mod.rs`: what each part of an adapter says, and the shared
  modules an adapter reuses rather than writes again.
- The module comments of the existing adapters, under `crates/ventris/src/adapters/`: what the road found
  in two games.
- `docs/commands.md` and `docs/packs.md`: the commands, and what a translator does with your adapter.

## Guardrails

- **What is learned about a game goes in; the game does not.** A character table, the addresses of its
  routines and the layout of its archive are knowledge. Its script, its font, its code, its pictures, a
  saved state and a patch made from it are the game. Study scripts, extracted text, dumps and screenshots
  stay under `work/` or `roms/`, which git ignores, or outside the repository.
- **The game comes from the user.** Ask for the path to their copy, the release they mean to support.
  Never download a game or a BIOS. A fan translation's patch may be fetched to use as an answer key, from
  its authors' own page, never a copy of the game with it applied; what it may be used for is in "An
  answer key is for facts" below.
- **Text only.** An adapter does not change the game beyond its text: no name-entry screens, no boxes laid
  out again, no pictures redrawn, nothing in how the game behaves (`docs/adding-a-game.md`, "Scope"). An
  adapter that only extracts is a whole contribution.
- **An answer key is for facts.** A fan translation, a decompilation or a disassembly tells you where
  things are and what changed. None of its code or text goes into the adapter; check its licence before
  taking more than facts.
- **The user's account is the user's.** Commit, push, open an issue or a pull request, or comment, only
  when the user asks, and show them what you will post.
- **Say what was verified and what was not**, with the evidence, to the user and in the pull request.

## 1. Find out what already exists, and tell the user

Before any study, look for:

- **A translation of the game in any language**, finished or partial. One web search is not enough:
  search in the game's own language and script and in the languages of the likely translators (English,
  Chinese, Korean, Spanish, Portuguese, Russian and others), on GitHub, in the databases of fan
  translations and ROM hacks, and in forums. Patches are often kept where an English search does not
  reach. Days of work have been spent on a game that turned out to have patches published this way.
- **A translation of another version of the game**: the original on another console, a port or a remake.
  It is an answer key for what the script says and the names and terms it uses; it says nothing about
  where anything is in the release you are working on, whose program and formats are its own.
- **A decompilation or a commented disassembly**, of any release: the formats carry over even when the
  addresses do not.
- **Documented formats**: notes on the game's text, compression, archive or font.
- **Whether someone has claimed the game** in the ventris issues or Discussions.

Tell the user what you found, with links, and what you looked for and did not find. A translation into
the language they want may change whether the work is worth doing; that is their decision. To claim the
game, the issue form for a game (`docs/adding-a-game.md`, "Asking for a game") says what to give; ask
before you open it.

## 2. Read the contract and the guide

Read `CONTRIBUTING.md`, `docs/adding-a-game.md` in full, the module comment of `adapter.rs` and the
comments on `Adapter` and `TextKind`, the top of `adapters/mod.rs`, and the module comment of the existing
adapter nearest your game. Then follow the order of work `docs/adding-a-game.md` gives: set up the
emulator, know the file, follow a line on screen back to where it is stored, write the adapter, check it,
look at the screen.

## 3. Work in that order

- Build and run with the tools the guide names: `ventris` for `check`, `extract` and `font`, and the
  Python workbench (`uv run ventris-workbench --help`) for identifying the file, probing the emulator,
  disassembling and comparing files.
- When the text is in no standard encoding, as on most consoles, find a line by its shape, not its bytes:
  get one line on screen, save memory at that moment, and search it for the gaps between the codes of the
  line's kana, taken in the order of the syllabary, in steps of one and two bytes. Most games number kana in
  that order, whatever the numbers are. What it finds is often a buffer the line was copied into; record
  which code writes the buffer, and it leads to the routine that reads the script and to how it is coded.
- A font that looks like noise in a viewer may be several 1-bit fonts stacked in one 4-bit texture, the
  palette choosing which bit is drawn. Split the bits before reading the glyphs; the routine that draws a
  character says which bit, page and cell a code selects.
- Get extraction right first: `extract` and `verify`, with the script encoding back to the game's bytes.
- Before writing the build, settle its design on facts: how the game finds the files it reads (by name or
  by a fixed place, and whether anything checks a size), where it puts what it loads in memory and how much
  room there is, and what else must move when text grows. Then prove the design on one image changed by
  hand, with a longer line run in the emulator, before any of it becomes code.
  The build can come later, from the user or from someone else.
- Reuse the shared modules the top of `adapters/mod.rs` lists. What is about a console and not about the
  game goes under `platforms/`.
- **A console ventris has no platform for** comes first: a module under `platforms/` on the contract in
  `platform.rs` (recognise an image, open it, its SHA-1, its label) and the library its adapters will
  need (a file system and rebuilding it, headers and checksums), tested on a made-up image, and a line in
  `platforms::ALL`. Its emulator is built from source like the others, and what the workbench needs that
  it lacks (screenshots, input, memory dumps, watching memory with the processor and the address of the
  instruction, saved states, running without a window) goes in a patch under `tools/<emulator>/`.
- **A font with no standard encoding**, its glyphs in the order the game's makers first used them, needs
  its table read by eye. Check every kanji against the furigana the script gives it and read decoded lines
  through; matching glyphs against the system's Japanese fonts gave poor candidates for a game's own font.
  Glyphs the script never uses are the weakest part of such a table: say so in its header.
- **Latin letters are narrow and even, always.** A translation in a Latin alphabet is drawn in narrow
  letters, not the game's full-width ones, and
  never a mix of the two: no fallback that draws some letters wide when the font runs short of room. When
  there is not room for every letter narrow, draw the leftovers as single narrow letters or refuse the
  build, saying how much room it needed and had. A build option that makes every letter full-width is
  allowed, as the pack's own choice. Check it on screen twice: with a pack that changes little, when the
  font has least free room, and with one that changes much.
- **A variable-width renderer comes first.** Changing how the game draws text is in scope when a
  translation needs it: hook the routine that draws a character so that each letter is drawn from a glyph
  of its own at its own width (Tales of Phantasia's `vwf.asm`). It needs a cell for each character, not
  for each pair, and gives proportional text. Put what is about the console and not the game (encoding
  its instructions, placing code in its executable, hooking a routine) under `platforms/`, so the next
  game on that console reuses it. Schemes that pack letters into the game's fixed cells come after, when
  a renderer is shown not to be possible.
- **No room is not a reason to give up a renderer.** When the game has no room for its code, its glyphs
  or its buffers, give it new room rather than squeezing: on the Super Famicom, a part of the ROM added by
  the build (Tales of Phantasia's renderer lives there); on a disc console, memory the game never uses,
  the executable made longer where the memory after it is free, or a small hook that loads the code from
  the disc's free room. Choose on measured facts (how much memory the game uses at its busiest), say in
  the module comment where it went and why, and keep the placing under `platforms/` for the next game.
- Choose sections and numbers before the first release: a string's address, `section.number`, never
  changes once released, and packs depend on it (`docs/adding-a-game.md`, "Addresses never change").
- A refusal names the string, the limit and what to do, in words a translator can act on. Write each kind
  of text's `limits` for a translator, in a sentence.
- Keep the module comment up to date with what was found, for a reader who has not seen the game.

## 4. Check it

- A pack that changes nothing must rebuild the original image byte for byte, and whatever a pack leaves
  out keeps the game's own bytes. `ventris check GAME` holds the adapter to the contract as far as a
  program can; run it and keep its output for the pull request.
- Tests go in `tests.rs` beside the module. A test that needs the game finds it with `crate::testing` and
  passes without checking anything when it is missing. No test fixture is cut from the game.
- So a test that passes may not have run. `VENTRIS_ROMS` is set in `.cargo/config.toml`, which each
  machine keeps for itself and a fresh checkout or worktree lacks: run the game's tests with
  `-- --nocapture` and see that no `skipped:` line names your game.
- A new adapter changes what other tests count: the list in `adapters::ALL`'s test, the number of games in
  `ventris find`'s output (`crates/ventris/tests/cli.rs`, `docs/commands.md`), the app's list of games
  (`app/src-tauri/src/lib.rs`), and the table of games in the README.
- Run the checks in `CONTRIBUTING.md`, "Checks".

## 5. Look at the screen

A change to what the game shows is not done until it has been seen in a screenshot from an emulator.
Build with a few changed strings of your own in each kind of text that can be inserted, and look at each
where the game shows it. When the font changed, start from power-on: a saved state carries the video
memory, and with it the old font. If you cannot run the emulator, say so and tell the user what to look
at and where.

## 6. The pull request

Only when the user asks. Before any commit, run `git status` and check that no game file, extracted text,
dump, saved state or screenshot is in it. `docs/adding-a-game.md`, "What a pull request shows", lists
what goes in the description: the module, the output of `ventris check`, screenshots of changed text (in
the pull request, not the repository), what was verified and what was not, the release, and which answer
keys were used.
