# Commands

This page is about `ventris`, the command-line program: how to get it, the routes of a player and a
translator by command, and every command, what it takes and what it does. The same is printed in short by
`ventris --help` and `ventris <command> --help`.

A player or a translator does not need it: [the app](playing.md#the-app) does their whole job in a window.
The command line is for two kinds of use:

- **Writing an adapter.** `check`, `extract` and `font` are what someone taking a game apart runs again
  and again; [Adding a game](adding-a-game.md) goes through them.
- **Automation**, such as checking every pack in a repository with `describe` each time it changes, or
  building a game from a script.

It runs on the same engine as the app: the two build the same file from the same game and pack, and
refuse the same packs with the same words.

```bash
ventris games
ventris find ~/games
ventris describe packs/game/en
ventris insert games/game.sfc packs/game/en -o games/game-en.sfc
ventris extract games/game.sfc -o work/game/ja
ventris check games/game.sfc
ventris font games/game.sfc -o work/game/font.png
ventris licenses
```

When building from source, `cargo run --release --` in front of a command runs it without building first.

## Getting it

Download the program for your system from the
[Releases page](https://github.com/bquenin/ventris/releases) and unpack it into a folder:

| System | File |
| --- | --- |
| Windows | `ventris-0.3.0-windows-x86_64.zip` |
| macOS, Apple silicon (M1 and later) | `ventris-0.3.0-macos-aarch64.tar.gz` |
| macOS, Intel | `ventris-0.3.0-macos-x86_64.tar.gz` |
| Linux | `ventris-0.3.0-linux-x86_64.tar.gz` |

It is one program, `ventris` (`ventris.exe` on Windows), with nothing else to install, and the fonts it
needs are built in. It is not signed yet;
[First launch: security warnings](playing.md#first-launch-security-warnings) says what to expect. So far it
has been used on Windows only; if it does not run on yours, please
[say so](https://github.com/bquenin/ventris/issues). To build it from source instead, see
[Building from source](playing.md#building-from-source).

It runs from a terminal: on Windows, open the folder in File Explorer, type `cmd` in the address bar and
press Enter; on macOS and Linux, open a terminal in that folder. Check that it runs:

```bash
ventris games
```

On macOS and Linux, and in PowerShell on Windows, a program in the current folder is run as `./ventris`,
so type `./ventris games`. The examples on this page leave the `./` out.

## For a player

The steps of [Playing a translated game](playing.md), by command. The command line reads the packs from a
folder and fetches nothing, so get them first: on the
[packs repository](https://github.com/bquenin/ventris-packs)'s page, choose **Code**, then
**Download ZIP**, and unpack it, or clone it with git.

To see whether your copy of a game is the release its adapter knows, point `find` at the folder you keep
your games in:

```bash
ventris find C:\path\to\your\games
```

It lists each supported game it found, with its id and the file it is in:

```text
Tales of Phantasia (Super Famicom, Japan)
  id         top-sfc
  release    Japan
  file       C:\path\to\your\games\snes\Tales of Phantasia (Japan).zip
found 1 of the 2 games there are adapters for; 2 files had the size of one
```

If it does not list your game, see [Your copy of the game](playing.md#your-copy-of-the-game). Then see
what a pack is, and build the game with it:

```bash
ventris describe ventris-packs/top-sfc/en
ventris insert "C:\path\to\your\games\Tales of Phantasia (Japan).zip" ventris-packs/top-sfc/en -o top-en.sfc
```

`ventris-packs` is the folder you unpacked the packs into. The first file `insert` takes is your game, the
second is the pack's folder, and `-o` names the file to write. Put quotes around a path that has spaces in
it. Give the new file the same ending as the game's own (`.sfc` for a cartridge, even when your copy is a
zip archive; `.iso` for a disc). Then [play it](playing.md#play-it).

## For a translator

The steps of [Translating a game](translating.md), by command.

```bash
ventris games
ventris extract your-game.sfc -o work/game/ja
ventris insert your-game.sfc my-pack -o work/game/built.sfc
ventris describe my-pack
```

`games` shows what a pack can do with each kind of text and the limits, as the app's
**What a translation can change in this game** does. `extract` writes the game's script, as the app's
**Save the game's script…** does; keep it to yourself and out of the pack. `insert` builds the game from
your copy of a pack, as **Build the game…** does. `describe` reads the pack without the game and refuses
one whose `pack.toml` or files are wrong, with the reason the app shows beside **Cannot be used**.

## The game's file

`extract`, `insert`, `check` and `font` work only on games that have an adapter. They take the game's own
file:

- **A cartridge game** (Super Famicom) as its image, with or without the 512 bytes a copier adds at the
  front, or as the zip archive a collection keeps it in, as long as the archive holds one cartridge image.
- **A PlayStation 2 game** as its disc image (`.iso`). The image is opened, not loaded: the commands read
  the files inside it that they need.

A game is recognised by the SHA-1 of its image, so only the release an adapter was made for is recognised.
An image that already has a fan patch applied is a different file and is not recognised as the game. A
file that is not recognised is refused with what it is, as far as can be told, and what is needed instead:
a compressed or archived file is to be unpacked or converted back first; another release of a game that
has an adapter, or that release with its contents changed, is told the release needed; a game with no
adapter is named by what it says it is. A file that is none of these is refused with
`... is not a game ventris knows how to read yet`.

## `games`

Lists every game that has an adapter. For each it gives the game's id (`tod2-ps2`), the release the adapter
was made for in full (what the image says it is, such as a disc's serial, its version, its size in bytes
and its SHA-1, which is what a file is checked against; its name in the catalogue of dumps that lists it,
and the address of its record there where one is given), and its kinds of text (dialogue, menus, lists and so on) with
what a pack can do with each: whether a string may be longer than the game's own, whether new characters
can be added, whether lines are fitted to the screen, and the limits in a sentence. It also names the
built-in fonts each game draws new characters from, and the game's own build options.

## `find FOLDER`

Looks through a folder of games, and the folders inside it, for the games that have an adapter, and says
which file each is. Games may be kept loose or one to a zip archive.

It does not read every file. An adapter gives the exact size of its game's image, a file's size is known
without reading it (for an archive, from its list of contents), and only a file of a game's size is read
to see whether it is the game. A collection of a few thousand games takes well under a second, plus the
reading of the few files that have the right size. Folders whose names start with a dot are skipped.

For a game it does not find, it says which files in the folder are that game, or say they are, and are
not the file needed, each with the reason `insert` would refuse it with, and gives the release needed.
For this it reads a few bytes more of some files and hashes nothing more: where a disc image or a plain
cartridge image says what it is, and the first bytes of a file with a disc image's ending or larger than
any cartridge, which say whether it is a compressed disc image. A disc game that is not found is also
told of the compressed disc images in the folder, since which game each holds cannot be told. A game
there is no adapter for is never named, and a cartridge in an archive is not looked into.

## `describe FOLDER`

Reads a pack without the game and says what it is: its name, game, authors and licence, how many strings
of each kind it changes, how many characters it renames, and the fonts a build of it draws from. It
refuses a pack whose `pack.toml` is wrong or missing, whose files are not script files, that names a font
that is neither built in nor in the pack, or that asks for something its game has not got. It cannot tell
whether each string fits; only a build can.

## `insert GAME PACK -o OUTPUT`

Builds the game with the pack's strings in it and writes it to `OUTPUT`. The game's own file is only read.
It says what it did: how many strings changed, which moved, which characters were added to the font, and
whether the result is identical to the original.

With a `pack.toml`, the build takes the pack's options, fonts and layout from there. A folder with no
`pack.toml` can be built from as well, which is how a pack starts. These say what a manifest would, and
are there for trying things out:

| Option | What it does |
| --- | --- |
| `--font FONT` | Draws characters the game's font lacks from this font, in place of the ones the pack names or the game's own: a built-in font's name, a Unifont `.hex` file, or for some games a BDF file. Several may be given, and are tried in that order. |
| `--menu-font FONT` | The same for the menu font, in a game that has one: a built-in font's name, or an 8x8 `.hex` file. |
| `--option NAME` | Turns on one of the game's own build options; `games` lists them. |
| `--no-wrap` | Keeps line breaks exactly as written. Without it, changed text is fitted to the screen in the way each game needs. |

When a pack asks for something the game cannot take, nothing is written and the reason is given in words:
a string too long for its place, a character none of the fonts has room for, a section that does not
exist.

A disc game is built as a whole new image, so it needs as much free space as the disc is large.

## `extract GAME -o FOLDER`

Writes the game's whole script as UTF-8 text, one file per section, and checks that the files encode back
to the game's own bytes. The format is described in [the pack format](packs.md#2-get-the-games-script).

The extracted script is the game's own text. Keep it to yourself, under `work/` in a copy of this
repository (git ignores it) or anywhere outside it, and never put it in a pack.

## `check GAME`

Holds the game's adapter to the contract as far as a program can: the script survives a round trip, an
unchanged script rebuilds the image byte for byte, each body of text takes a changed string, and what a
pack must not do is refused. It cannot look at the screen; that takes an emulator and a person.

`--work FOLDER` builds in that folder, and makes it if it is not there. It needs room for a copy of the game.
Without it, a temporary folder is used.

## `font GAME -o FILE`

Draws the game's font as a PNG picture, `--per-row` glyphs to a row (32 unless told) at `--scale` times
its size (2 unless told). It is a picture of the game's own font, so keep it to yourself as you would the
extracted script.

## `licenses`

Lists the fonts built into the program, with the terms each is passed on under. They are kept in
`crates/ventris/fonts`, with their licences.

| Built-in font | What it is |
| --- | --- |
| `unifont` | [GNU Unifont](https://unifoundry.com/unifont/): 16 pixels tall, with a glyph for nearly every character of Unicode |
| `dejavu-condensed-bold` | DejaVu Sans Condensed Bold, drawn for a cell of 12 by 16 pixels: Latin, Greek and Cyrillic |
| `unscii-8` | [Unscii-8](http://viznut.fi/unscii/): 8 by 8 pixels, for text drawn in small tiles |
