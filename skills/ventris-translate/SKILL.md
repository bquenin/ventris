---
name: ventris-translate
description: Translate a game's text with ventris, or improve an existing ventris translation pack, by command line. Checks that the game has an adapter, extracts its script with `ventris extract`, writes a sparse pack of UTF-8 strings with a glossary, builds the game with `ventris insert` after each batch, and fixes the strings a build refuses. Use when the user wants to translate a game that ventris supports into any language, start, review or correct a pack for the ventris-packs repository, or fix a line in one. Not for adding support for a new game (use ventris-adapter).
---

# Translating a game with ventris

You are helping your user translate a game that ventris already supports, or improve a pack, which is a
translation kept as a folder of plain text files. The ventris repository is
<https://github.com/bquenin/ventris>; the packs are in a separate one,
<https://github.com/bquenin/ventris-packs>. Paths below are paths in the ventris repository.

The people who use ventris work in its app, which you cannot drive. You work with the `ventris` command
instead: it runs on the same engine, builds the same game from the same pack, and refuses the same packs
with the same words.

## Where the facts are

Read these before you start, and go back to them rather than guessing. This file gives the procedure and
its guardrails; the facts are there, and they win where the two differ.

- `docs/translating.md`: the whole job for a person, fixing a line, reviewing a pack, starting one.
- `docs/packs.md`: the pack format in full, `pack.toml`, `names.txt`, fonts, and notes on each game.
- `docs/commands.md`: every command, how to get the program, and the translator's route by command.
- The packs repository's `CONTRIBUTING.md`: the rules a pack kept there follows, `NOTES.md` among them.
- `README.md`, under Scope: what a translation does and does not change.

## Guardrails

- **The game comes from the user.** Ask for the path to their copy. Never download a game, a disc image or
  a patch, and never suggest where to get one.
- **The game stays out.** The game, the extracted script, a built image, a font picture and anything made
  from them never go into a pack, a commit, an issue or a pull request. Keep them in a working folder
  outside any repository, or under `work/` in a copy of the ventris repository, which git ignores.
- **Translate from the game's own text**, not from another translation. An existing fan translation is an
  answer key for facts (what a name refers to, where a line is), never a source of wording; check its
  licence before taking more than facts. Translating from another ventris pack is allowed under that pack's
  licence, with its authors credited (`docs/translating.md`, "Translating from another pack").
- **Text only.** No reworking of layouts, name-entry screens, pictures with text in them, or how the game
  behaves. If a line does not fit, the fix is a shorter wording.
- **The user's account is the user's.** Opening an issue or a pull request, or commenting on one, is done
  under their name: ask before each, and show them what you will post.
- **Say what was seen and what was only built.** Never call a string done because it built.

## Before you start

1. Get the program, from a release or built from source (`docs/commands.md`, "Getting it"), and run
   `ventris games`. If the game is not listed, it has no adapter and cannot be translated yet; the
   ventris-adapter skill is for adding one.
2. Find the user's copy: `ventris find FOLDER` on the folder they keep their games in says whether it is
   the release the adapter reads. When it is not, `find` says which files there are that game and why each
   is not the file needed; tell the user. A game with a fan patch already applied is not recognised.
3. Look in the packs repository for a folder `<game id>/<language>` (such as `top-sfc/en`) and in its open
   issues for a pack someone has claimed. There is one pack per game and language: improving the pack that
   is there is better than starting another.
4. A new pack is claimed with the packs repository's form for a new pack
   (<https://github.com/bquenin/ventris-packs/issues/new/choose>). Ask the user before you open it.
5. Read the limits of each kind of text in `ventris games`, and the notes on the game in `docs/packs.md`.

## Set up a working folder

Extract the script into the working folder, never into the pack:

```bash
ventris extract GAME -o work/game/ja
```

It writes one UTF-8 file per section and checks that the files encode back to the game exactly. The
pack is a different folder. To improve an existing pack, work in a clone or fork of the packs repository,
in the pack's folder, and read its `NOTES.md` first; the working folder with the script and the built
game stays outside that clone.

## Fix the glossary first

Before translating at volume, read through the script and list the names of characters, places, skills,
items and the terms that recur. Agree the main ones with the user, write them in the pack's `NOTES.md`, and
keep to them in every file: a player who meets two spellings thinks they are two things. Where the game
can rename its characters, `names.txt` gives the names (`docs/packs.md`, "Names"). Changing an established
name means changing it in every file of the pack and in `NOTES.md` in the same change.

## Write the pack

- Copy only the strings you change into a file of the same name as their section (`31.txt`), header and
  all, and replace their text. A pack is sparse: whatever it leaves out keeps the game's own text.
- Never change a header. A string's address, `section.number`, never changes; a pack can replace strings
  but cannot add any.
- Every string ends with a blank line, the last in a file too, or the pack is refused.
- Keep each string's `{TAGS}`, where they make sense in the sentence. Never drop a tag, add one, or edit
  its numbers without knowing what it does; `docs/packs.md`, "Get the game's script", says how tags are
  written.
- Read the `# ` comments the extracted script carries at the head of a file and under some headers: they
  state limits that the text does not show.
- Add `pack.toml` (`docs/packs.md`, "Say what the pack is"). A pack for the packs repository also says how
  it was made and what has been seen on screen, and carries that repository's licence.

## Build often

Check the pack, then build it into a new file, after every section or batch of strings:

```bash
ventris describe PACK
ventris insert GAME PACK -o work/game/built.sfc
```

Give the output the ending of the game's own image (`.sfc`, `.iso`). A disc game is built as a whole new
image and needs as much free space as the disc is large.

When the build refuses, nothing is written and the reason is given in words, such as
`string menu.004 is 36 characters; the longest a menu string can be is 26`. That is the feedback: fix the
string it names. Do not work around a refusal: do not move text into another string, drop tags to save
room, turn on one of the game's build options to make it pass, or change the adapter. If a limit seems
wrong, tell the user and leave it for an issue.

One refusal has a second fix: a character that none of the build's fonts has can come from a font that
has it, named in `pack.toml` (`docs/packs.md`, "Fonts"). That supplies a missing letter; it gives no
string more room. A built-in font is the first choice; a font file in the pack needs a licence that lets
it be passed on, and the packs repository takes no font files. Agree the font with the user. When the
refusal is that the font has no room for more characters, a font does not help: change the text.

A build that succeeds says only that the strings fit the room the game gives. A line can still run past
the edge of its box.

## Look at it on screen

Seeing it on screen is part of done. If you can run an emulator, open the built game and look at the
strings you changed, starting from power-on when the pack adds characters the game's font did not have: a
saved state holds the old font. If you cannot, say so, and give the user a list of the changed strings by
address with where in the game each is shown, so that they can look.

Report plainly: which strings were seen on screen, which were only built, and which were not built.

## Record how it was made

- In `NOTES.md`: the glossary, how strings are written, what has been seen and what has not, and how the
  translation was made: machine translated or not, by what, and reviewed by whom, or by nobody.
- In `pack.toml`, `authors` or `description` says it was machine translated, when it was.
- Tell the user that a person who reads both languages should play it before it is presented as more than
  a machine translation.

## Sending it

Only when the user asks. A change to the packs repository is a pull request from their fork, under their
account; `docs/translating.md`, "Send it", says how. Before any commit, run `git status` and check that
only the pack's own files are in it. The pull request says where in the game the strings are and which
were seen on screen.
