# Writing a translation pack

A translation pack is a folder of text files: the strings of a game that you have changed, and a small
file that says what the pack is. It holds no code, no part of the game, and as a rule no font. Anyone who
has the game's own file and your pack can build the translated game, in the app or with one command.

This guide is for whoever writes a pack, and is the reference for the format. Its steps are given as
commands of the `ventris` program ([Commands](commands.md#getting-it) says how to get it), with the game's
file, for adapter authors and for scripts. It needs nothing from the Python workbench. A translator does
not need the commands: the app shows a game's limits, saves its script, says what is wrong with a pack and
builds the game. [Translating a game](translating.md) is the walk-through, with the app and no terminal:
fixing a line, reviewing a pack, starting one, and sending the work.

## 1. See what the game can take

```bash
ventris games
```

lists every game that has an adapter. For each it gives the game's id (`tod2-ps2`), the image the adapter
was made for, and its kinds of text (dialogue, menus, lists and so on) with what a pack can do with each:
whether a string may be longer than the game's own, whether new characters can be added, and the limits
in a sentence. Read the limits of the kinds you mean to translate before you start.

A game is recognised by the SHA-1 of its file. A file that already has a fan patch applied is a different
file and is not recognised. A cartridge game can be given as the zip archive a collection keeps it in.
Given a folder, `ventris find` looks through it for the games that have an adapter, and says which file
each is.

## 2. Get the game's script

```bash
ventris extract roms/game.iso -o work/game/ja
```

writes the whole script as UTF-8 text, one file per section. The extracted script is the game's own text:
keep it under `work/`, which git ignores, and never put it in a pack.

Each string is a header line, its text, and a blank line:

```text
[31.001]
The first line
and the second.{PAGE}
The next page.

[31.004]
...
```

- The header is the string's address in square brackets: its section, a dot, its number. Addresses
  never change, in this version of ventris or a later one. In some kinds of text the numbers have
  gaps; that is as it should be.
- `{TAGS}` stand for whatever is not a character: who speaks, a page break, a character's name, a pause.
  Keep the tags a string has, where they make sense in your sentence. A tag with numbers after a colon,
  such as `{NAME:01 00 00 00}`, is copied as it is.
- `{G:1A2B}` is a character of the game's font that has no entry in the adapter's table yet.
- A line that starts with `# ` (a hash and a space), or that is a hash alone, is a comment. It is no part
  of any string, wherever it stands: at the head of a file, under a header, between two strings. The
  extracted script has some, written by ventris where a string is limited in a way its text does not
  show: at the head of a file for what holds for all of it, and under the header of a string that is
  shown some other way than its neighbours. Read them before translating that string. You can write your
  own in a pack, to yourself or to whoever takes it up after you:

  ```text
  [01.004]
  # Shown as sprites: 15 to a row, 32 letters to a page.
  Don't... defile the tree...
  ```

- A line is a header only when it is exactly an address in square brackets. A line of text that is exactly
  that, or that starts with a hash, is written with a backslash before it: `\#1`, `\[01.004]`. Text that
  merely starts with a bracket, such as `[Cress] nods.`, needs nothing.
- Two near misses are refused when a pack is read, with the file's name, where taking them for text would
  put them in the game or join a string to the one before it: an address with one of its brackets
  missing (`[01.004`), and a line that starts with two hashes (`## 01.004`, which is how a header was
  written before the brackets, or `## check this`).

## 3. Write the pack

Make a new folder, and in it one file per section you change, named after the section (`31.txt`). Copy
the strings you change from the extracted script, header and all, and replace their text with yours.
A script file's name ends in `.txt` in lowercase: one ending in `.TXT` is refused, with its name.

A pack is sparse: it holds only the strings it changes. Whatever it leaves out keeps the game's own text,
so a pack can start with one line and grow. A pack can replace strings but cannot add any.

Write in any language and any alphabet. The files are UTF-8, and you never deal with the game's encoding.

### Names

Where `ventris games` says "a pack can rename the characters", a file `names.txt` in the pack gives
them. Each line is the character's name in the game, which is also its tag in the script, then the name to
show:

```text
クレス = Cress
```

`extract` writes this file with every name in it. A line you leave out keeps the game's own name.

### Fonts

A game's font has the characters of the game's own language. Characters it lacks are drawn from another
font, into room the adapter finds in the game's font. You do not have to bring one: `ventris` has a
few fonts built in, and each game draws from the ones that were tried in it and look right.
They are named for each game by `ventris games`, and `ventris licenses` shows their terms.

| Built-in font | What it is |
| --- | --- |
| `unifont` | [GNU Unifont](https://unifoundry.com/unifont/): 16 pixels tall, with a glyph for nearly every character of Unicode |
| `dejavu-condensed-bold` | DejaVu Sans Condensed Bold, drawn for a cell of 12 by 16 pixels: Latin, Greek and Cyrillic |
| `unscii-8` | [Unscii-8](http://viznut.fi/unscii/): 8 by 8 pixels, for text drawn in small tiles |

A font is used once the game's own lacks a character your text uses. A pack that keeps to the game's
characters is drawn with the game's font alone.

If none of these suits your language, a pack can carry a font of its own and name it in the manifest:

- a Unifont `.hex` file, which may be compressed (`.hex.gz`);
- for Tales of Destiny 2, also a BDF bitmap font; `tools/fonts/outline_to_bdf.py` makes one from an
  outline font;
- for the menus of Tales of Phantasia, an 8x8 `.hex` font, named as the menu font.

Put it in a folder `fonts/` in the pack, with the licence it comes under, and use only a font whose
licence lets you pass it on. A font that serves a whole language well is better proposed for
`ventris` itself, so that every pack in that language has it.

## 4. Say what the pack is: `pack.toml`

```toml
format = 1
game = "tod2-ps2"
language = "en"
name = "Tales of Destiny 2 in English"
version = "0.3"
authors = ["Your name"]
license = "CC-BY-NC-SA-4.0"
description = "The story and the skits. Menus are not done yet."
homepage = "https://example.org/my-pack"
```

| Key | Needed | What it says |
| --- | --- | --- |
| `format` | yes | The version of this layout: `1`, or `2` for a pack that moves labels under `[layout]`. It goes up whenever the layout gains a key, so that an older `ventris` says a pack is newer than it is, and does not call the new key a mistake. |
| `game` | yes | The id of the game's adapter, as `ventris games` shows it. |
| `language` | yes | The language of the pack as an IETF tag: `en`, `fr`, `pt-BR`, `zh-Hant`. |
| `name` | yes | What the pack is called, for someone choosing one. |
| `version` | yes | Which state of the pack this is. You decide what it looks like; change it when you pass on a new state. |
| `authors` | yes | Who made it; at least one name. |
| `license` | no | The terms you pass your translation on under, as an [SPDX](https://spdx.org/licenses/) name where there is one. A pack in the packs repository says `CC-BY-NC-SA-4.0`, that repository's licence; for a pack kept elsewhere, the choice is its authors'. |
| `description` | no | A sentence or two: what is translated and what is not. |
| `homepage` | no | Where the pack's authors can be found. |

Under `[build]`, all optional, is what a build would otherwise have to be told. Most packs need none of
it:

```toml
[build]
fonts = ["fonts/letters.bdf", "unifont"]
```

| Key | What it says |
| --- | --- |
| `fonts` | The fonts for new characters, tried in this order, in place of the game's own: each the name of a built-in font, or a file in the pack. |
| `menu_font` | The same for text drawn in small tiles, in a game that has such text. |
| `options` | The game's own build options to turn on; `ventris games` lists them. |
| `wrap` | `false` keeps your line breaks exactly as written. Left out, lines are fitted to the screen where the game's adapter does that. |

A font file is named from the pack's folder, with `/` between folders. A pack cannot name a file outside
itself. A key that is not in these tables is refused, so that a misspelt one does not go unnoticed.

Under `[layout]`, in a pack of format 2, are the labels to draw somewhere else than the game does, where
the game's adapter can move them. Each is named as the extracted script names it, with how far to move
it, in `columns` to the right and `rows` down; either may be negative, and one left out is nought:

```toml
[layout]
"menu.659#3" = { columns = -2 }
```

A game gives a label a place of fixed width, and a translation's word is often wider. Moving a label, or
its neighbours, makes the room, where no wording fits. The notes on each game say what can be moved.

A folder with no `pack.toml` can still be built from, which is how a pack starts. It needs one before it
is passed on.

## 5. Build it and look at it

```bash
ventris insert roms/game.iso packs/my-pack -o work/game/built.iso
```

builds the game with your strings in it, and says what it did: how many strings changed, which moved,
which characters were added to the font. With a `pack.toml` it takes the options from there, and the
fonts if the pack names any. `--font`, `--menu-font`, `--option` and `--no-wrap` on the command line are
for trying things out: a font given there, by a built-in font's name or as a file, is used in place of
the pack's and the game's own.

When a pack asks for something the game cannot take, nothing is written and the reason is given in words:
a string too long for its place, a character none of the fonts has, a section that does not exist.
Shorten the string or change what is named, and build again.

A build that succeeds is not yet a translation that works. What fits on screen is for you to judge: run
the built game in an emulator and look at the text you changed. Start from power-on, not from a saved
state, when the pack adds characters.

A disc game is built as a whole new image, so it needs as much free space as the disc is large.

## 6. Check it before passing it on

```bash
ventris describe packs/my-pack
```

reads the pack without the game and says what it is: its name, game, authors and licence, and how many
strings of each kind it changes, and the fonts a build of it draws from. It refuses a pack whose manifest
is wrong, whose files are not script files, that names a font that is neither built in nor in the pack,
or that asks for something its game has not got. It cannot
tell whether each string fits; only a build can.

Pass on the pack's folder and nothing else: your strings, `names.txt`, `pack.toml`, and a font of your
own with its licence if the pack carries one. Never the game's file, the extracted script, or a built
image.

A pack for the [packs repository](https://github.com/bquenin/ventris-packs) follows its own rules as well,
which its `CONTRIBUTING.md` sets out: among them, no font files, a `NOTES.md` with the names and terms
chosen, and the licence of that repository, CC BY-NC-SA 4.0.

## Notes on each game

Each game's limits are given by `ventris games`. What follows is what else a pack's author needs.

### Tales of Phantasia (`top-sfc`)

- Dialogue lines are broken at spaces to fit the box.
- The game has 16,480 bytes of memory for an area's events and its dialogue, and a translation of an
  area is often larger than that. The build then keeps that area's dialogue on the cartridge and has
  the game read one string at a time, and says so (`cartridge:` and the areas). What still has to fit is
  the area's longest string beside its events, so a single string of several thousand bytes can be
  refused; break it into two where the game's script allows, or shorten it.
- Eight strings of dialogue are shown as sprites over the scene, not in a box: a voice from nowhere, a
  line of narration. The extracted script says which, in a comment under each. The game draws them with
  its letters wide apart, so they hold far less than a box does:
  - **15 characters to a row.** The build breaks the rows at spaces, as it does for a box.
  - **32 letters to a page**, spaces not counted. The game cannot show more: what comes after is drawn
    as scrambled dots. The build refuses a longer page and names it. Put a `{PAGE}` where the page
    should turn, which waits for a button, or say it in fewer letters. Where the turn falls is yours
    to choose, which is why the build does not choose it.
  - A string with no space in it at all, as in Japanese, is left to the game, which starts a new row
    after every 16th character.
- 122 strings are the name of a place or a shop, which the game shows on entering it, centred in a box
  of one line at the top of the screen. The extracted script says which, in a comment under each. The
  game counts a name's characters to centre it, and puts half of what 16 leaves as spaces in front, 14
  dots each, which is right only for its own characters. A build with the variable-width renderer (one
  that adds characters, renames one, or asks for it with the `renderer` option) centres a name by its
  width instead: it measures in dots what the line will draw, a party member's name and a number
  included as the game shows them, and starts the name half of what the window's 224 dots leave from its
  left edge. A name left in Japanese is centred that way too, so one with an odd number of characters,
  or with a party member's name in it, sits on the middle where the game has it 7 dots to the left. Two
  things leave the box without the name, and the build refuses them, naming the string:
  - **A last character that would start at dot 208 or further**, centring included. With the renderer
    the line holds about 200 dots, some 28 Latin letters or 13 characters 16 dots wide. The build keeps
    room for what the game may draw instead of what the pack says: in place of a party member's name, a
    name a player typed (up to six letters, 14 dots each in the game's own characters); for a number, as
    many digits as it can show (three for a byte, eight otherwise), wherever it stands in the name. The
    message says when it is that room that is missing.
  - **More than 16 characters, without the renderer.** The game's own count then centres the name and
    breaks past 16. A party member's name counts as its letters, as the menus show them, and one more.
    Sixteen of the game's own characters, 14 dots each, are too wide for the line already.

  A place name the pack gives as the game has it is not held to this. The build does not break a place
  name over lines, as it does other dialogue.
- `captions.txt` holds what a battle says, the names of skills written in kanji, the descriptions of
  items and the headings of the menu screens. Some of the game's kanji there have no entry in the
  character table yet and show as `{K:....}`. Four things are particular to captions:
  - **Two letters to a cell.** The game draws captions in cells 16 dots wide, about 15 to a line. A
    character the game's font lacks is drawn from Unifont, and narrow ones, such as Latin, Greek or
    Cyrillic letters, go two to a cell: 30 letters to a line. A run with an odd number of them ends in
    a space.
  - **Pieces need an even number of letters.** The game builds some sentences from pieces: a caption
    may hold `{PHRASE:EA62}`, which is another caption shown in its place. A cell is not shared between
    pieces, so write each piece with an even number of letters, or a gap shows where they meet.
  - **A cell kept blank.** Where the game draws a picture over the text, such as a button, the caption
    has the game's wide space, `　`, in that cell. Keep it there.
  - **No more cells than the game's own.** A screen gives a caption the room the game's own took, and
    a longer one can overwrite what is beside it. The build lists the captions that take more cells
    than the game's. Look at each of those on screen: some have room, a heading has none.
- Menu text (labels, the names of items and skills) is in `menu.txt`. It is a more limited system than
  dialogue: 8x8 tiles, one per character, in a font of 192 tiles. A new character takes the tile of a
  kana that no remaining menu string uses, so a pack has to replace most of the menu text before it has
  room for a new alphabet. A menu string may be up to 26 characters.
- The menus give each string a place of fixed width, and a longer one is cut off or drawn over. Where a
  word a translation needs cannot be had in that width, a pack can move labels to make the room. 164
  of the menus' fixed labels can be moved: those of the main menu, the status, equipment and custom
  screens, the shop, the prompts. In the extracted `menu.txt`, each string the game draws at such a
  place has a comment under its header that names every place, with its column and row and, where it
  is known, the screen. A pack moves one in `pack.toml`, under `[layout]`, by columns to the right and
  rows down:

  ```toml
  format = 2

  [layout]
  "menu.659#3" = { columns = -2 }
  ```

  What is beside a label does not move with it: a number, a name from a list and a picture are placed
  by the screen's own code. One picture can be moved the same way, as `"picture.skills"`: the control
  pad after the skills label on the status screen. The build checks that a move stays on the screen,
  and nothing more: whether a label now sits on something else only the screen shows, so look at it.
  A pack that moves nothing gets the game's own layout.
- The status screen writes a character's full name the Japanese way, `クレス ＝ アルベイン`: the given
  name, the sign `＝`, and the family name, a column apart. The sign is `menu.759`, which the naming
  screen also draws before the family name. To join the two names with a space, write `menu.759` as a
  space and move the family name over it, as `"status.family"`:

  ```toml
  [layout]
  "status.family" = { columns = -2 }
  ```

  The family name follows the given name whatever its length, and this moves it for every character.
  A name of seven characters in the menus is counted to its end, so the family name keeps its distance
  after it too.
- A name from `names.txt` appears wherever dialogue prints it, unless the player has typed one of their
  own. The menus show it too when it is at most 7 characters and the menu font has room for its letters;
  otherwise they keep the game's name.

### Tales of Destiny 2 (`tod2-ps2`)

- The game draws ASCII in a narrow font that has capitals only, and draws some other ASCII as something
  else (`"` `'` `` ` `` as a closing quote, `~` `_` as a dash, `;` as a colon, `< >` and `[ ]` as other
  brackets, `*` as a star, `# $ @ \ ^ |` not at all). Those, lowercase letters and anything else outside
  its own characters are drawn from `dejavu-condensed-bold`, and what that lacks from `unifont`.
  A narrow character takes one byte and a wide one two, which matters wherever a limit is counted in bytes.
- Changed dialogue is broken at spaces to 40 columns, a wide character taking two. A party member's name
  (`{NAME:01 00 00 00}` to `{NAME:06 00 00 00}`) takes the columns of the name a new game gives that
  member: the pack's, where it changes the strings of the program the game takes the names from
  (`program.1064000` for the first, Kyle, and the five that `program.1063976`, `1063960`, `1063944`,
  `1063928` and `1063896` hold), or else the game's own. Any other name is counted as 10 columns, the
  width of the longest of the game's own. The player can rename the party on the Status screen, and a
  name holds at most 12 columns; a bubble is as wide as its longest line, and the screen holds about 56.
  A new game copies each of those six names into a field of the save that has room for 12 bytes, so a
  longer name is refused, with its string and its length.
  A skit's line is one row that scrolls sideways when it is long, and is left as written.
- A skit types each line a letter at a time, and takes it away when the line two after it starts, or
  when the skit ends; the times are set by the voice, and a button does not hold them. English is about
  twice as long as the Japanese, so at the game's pace a long line is taken away before it is whole. The
  option `skit-pace`, which a pack turns on with `options = ["skit-pace"]` under `[build]`, types each
  line the pack changes faster where it must: at the slowest pace that leaves the line whole for a second
  before it is taken away (or for as long as the game leaves its own line whole, if that is less), never
  slower than the game's own pace and never faster than 5 frames a letter. A line too long for that even
  at 5 frames a letter is refused, with its letters and the most that fit; every such line is named at
  once. Letters are counted as the game draws them: each character and space one, a button picture one,
  and a party member's name 6, since the player may rename the party. A line break, a number or an
  item's name in a skit's line is refused with the option on, since no skit of the game has one. Lines
  the pack leaves alone keep the game's pace. It is an option, off unless asked for, because it changes
  how fast the game types, which is more than its text.
- The grids of artes, on the Artes screen (while a slot is set) and the Enchant screen, draw a skill's name in
  the narrow font, with room for 17 letters in its column, and a spell's name in large letters, with room for
  9. The option `narrow-spell-names` draws a spell's name as a skill's is drawn. It changes one instruction
  in each of the two screens' modules and nothing else: not the name, its colour or its place. It is an
  option, off unless asked for, for the same reason as `skit-pace`.
- A list to choose from (the game's own are strings such as `06308.019` and `06308.385`, after a string
  `select`) is left as written: each line is one choice, after the question if there is one, and the game
  picks among the last lines. It may not have more lines than the game's own, nor a line wider than 55
  columns (the game's own keep to 40); a string that does is refused, with the line.
- A bubble shows four lines at a time and goes on to the next four by itself; `{02}` starts a new page
  sooner. A page can show at most 127 characters, counting spaces, each button picture as one and a name
  as the letters it fills in, but not line breaks. So where a page of a changed string would hold more
  than 123 (a little is kept free, and a name is counted as 6 letters, a number as 7 digits and an
  item's name as 16), its lines are broken narrower, down to 24 columns, until every page fits; a string
  that still does not fit is refused, with the page that is too full.
- In the lists and the menus, the strings a pack changes share the room they had between them: one may be
  longer where others are shorter.

### G.O.D pure (`god-ps1`)

- Three kinds of text can be changed: the dialogue, in the sections `096` to `188`, one for each area of the
  game; the program's own text, in `program` (shops, help pages, the names of items, monsters and places,
  battle messages); and the labels of the menus, in `menu`. Section `battle` can be extracted and read, and a
  pack that changes it is refused. It is what the bosses say in battle, and the messages that take the place
  of the battle's own when such a battle starts, is won or is lost; it is shown in the battle's window, 4
  lines of 18 smaller cells, where the game goes on to the next line by itself when a line is full.
- The window shows 4 lines of 224 pixels; a kana, kanji or mark of the game's own font is 14 pixels wide, so
  a line of Japanese holds 16. After `{SAY}`, which starts a spoken line and draws its 「, the lines that
  follow start under its first character, 14 pixels in, so a spoken line has 210. Changed lines are broken at
  spaces to fit, and a `{WAIT}` is put in where the window would fill; the window then scrolls one line at a
  time, so the end of the last page stays in view above the new one. A name the game fills in is counted as
  6 cells of 14 pixels, a number as 6 and a word as 8. A word wider than a line is refused.
- Latin letters, digits, spaces and punctuation are drawn **each at its own width**, by a renderer the build
  puts into the game: Unifont's ink and a pixel more, from 2 to 9 pixels, and 4 for a space, so a line holds
  about 35 letters. Each character a pack writes that the game's font lacks and `unifont` (or the fonts the
  pack names) draws 8 pixels wide, accented letters included, is drawn once into both of the game's fonts:
  the one of 16 pixels the dialogue's window uses, and the one of 12 by 11 the battle's window and the menus'
  screens use, at the same width. This holds for the dialogue and the program's text, but not for the
  party's names, which stay a character to a cell. The characters take the cells of kanji that no string the
  pack leaves as it is uses: 275 when a pack changes little, most of the font's 1,516 kanji in a full
  translation; a pack needs about a hundred, and one that needs more cells than there are, or more than 240
  such characters, is refused. Such a letter takes **one byte**, in a run that a byte starts, so a run takes
  one more; a line break or a tag ends a run. The game's own text, and what the pack leaves Japanese, is
  drawn as before.
- With the build option `full-width` (`--option full-width`, or `options = ["full-width"]` under
  `[build]` in `pack.toml`), every character takes a cell, as in the game's own text: Latin letters,
  digits and punctuation are drawn with the game's full-width glyphs, a space is its blank cell, and
  characters the font lacks (accented letters, the comma, the apostrophe, quotation marks) are drawn from
  `unifont`, or from the fonts the pack names, into the font's 26 free cells, so a pack can use at most 26
  such characters. Without the option, those 26 cells take only the characters drawn 16 pixels wide.
- What English costs in bytes, measured on the pack in progress (areas 096 to 104, 497 strings): its
  English has twice as many characters as the Japanese, and its text blocks came out 1.5 times the
  Japanese ones' bytes (11,796 bytes more over 23,612; two letters to a cell, as before the renderer,
  cost 11,092). A letter is a byte where a kana is one and a kanji two. So an area that cannot grow, such
  as 105 (27,598 bytes of text for 17,258 Japanese characters), takes an English of roughly 1.25 times as
  many characters as its Japanese. That is an estimate: what counts is the bytes, which runs, tags and line
  breaks change too, and the build says whether an area fits.
- An area that grows past its sectors on the disc is moved: into the sectors other areas that moved
  left, or to the disc's free room near its end, 16,384 sectors. Every area of the game can grow and
  move: an area takes a few hundred to a few thousand sectors, and a full translation adds a few to
  each, so the areas that move free nearly all the room they need. The heap limits below are reached
  long before the disc's: every area grown as far as they allow still leaves more than 13,000 sectors
  of the free room unused. The disc stays the size it is. A build that needs more room than there is
  is refused, with the areas that found none.
- An area's code, which holds its text, must fit on the game's heap with everything else that is there
  at the area's busiest moment: going through a door into it or out of it, when the game holds both
  areas for a while, loading one of its maps, or fighting in it. Going through a door into an area
  whose code does not fit where the old area's was, the game needs room for three copies of the new
  code at once, and running out of room there stops the game. The game leaves itself little room in
  places: going from area 107 into area 105, only 19,636 bytes of the heap are free, because it reads
  the new area's pictures (229,412 bytes there) before it goes through the door. A build that changes an
  area, or draws letters at their own widths, makes the game read them after, once the old area is
  gone, as it already does when it has no room for them; the door takes a few frames longer. Every
  moment of every area it changes, and of every area when it draws letters at their own widths, must
  still leave at least 19,636 bytes free, or the build is refused with the area, the moment (the door,
  and the other area) and how many bytes too many it needs. How much an area can grow, if no other area
  does, with letters drawn at their own widths:

  | Area | Text (bytes) | Can grow by (bytes) | Tightest moment |
  | --- | ---: | ---: | --- |
  | 157 | 16,370 | 52,324 | going into it from area 154 |
  | 154 | 20,354 | 53,256 | going into it from area 157 |
  | 105 | 27,598 | 75,892 | going into it from area 107 |
  | 179 | 13,474 | 77,944 | going into it from area 131 |
  | 131 | 12,750 | 106,520 | going into it from area 179 |
  | 107 | 6,088 | 121,164 | going into it from area 105 |

  Every other area can grow by more than 79,000 bytes, and half of them by more than 180,000. Two areas
  that lead to each other share their room: what one takes, the other cannot, and an area whose code
  grows past another's can make the door between them need the three copies.
- The program cannot grow, so the strings of `program` a pack changes share the room they had between them:
  70,402 bytes if every one changes, a kana taking one byte, and a kanji or a cell of two letters two, as
  does the cell of an odd letter at the end of a run, its other half blank.
  One may be longer where others are shorter, and strings given the same text are written once, which
  frees much room, since the game repeats many (the monster book's descriptions above all); but where the
  code that finds such strings cannot reach one place from all of them, each is written apart, and needs
  its own bytes. A pack that
  changes only a few strings has only their room; a string that finds no place is refused, with the bytes
  each needs.
- The note on the first string of each part of `program` gives its width and lines: a line is no wider
  than the widest of the game's own there, a name the game fills in counted as 6 cells, a number as 6 and
  a word as 8, and changed lines are broken at spaces to fit. A string has no more lines than the game's
  own, but a shopkeeper's line may have 2, a description in the monster book 5, and the help pages, the
  chakra lessons and the diary run on. A choice (yes and no, the title screen's menu) stays as wide as the
  game's own, its box being made for that. A party member's name takes at most 6 characters, as the name
  entry allows, and 15 bytes, and is written in capitals, since the menus draw it in a font without
  lowercase letters. The player can still rename the hero, in kana.
- A label of `menu` stays in its slot: it takes no more bytes than the game left for it, and draws no more
  characters than the widest label of its screen, 8 in the main menu; ﾞ and ﾟ take a byte but are drawn
  over the letter before. The menus' font has capitals, digits, punctuation and the kana; lowercase
  letters are written as capitals.
