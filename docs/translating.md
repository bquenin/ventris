# Translating a game

This page is for anyone who wants to translate a game or improve a translation. The work is done in plain
text files, in any language and any alphabet, with no programming and no hacking of the game: the game's
adapter takes the text out and puts yours back in. Knowing the languages, and the game, is what it takes.

Everything here is done in the ventris app, a plain text editor and a web browser. No terminal is needed.
[Playing a translated game](playing.md#the-app) says how to install the app and how it finds your copy of
the game.

There are three sizes of contribution, smallest first:

1. [Fix a line](#fix-a-line): you saw something wrong while playing, and you correct it or report it.
2. [Review a pack](#review-a-pack): you go through a part of a game and correct what you find.
3. [Start a pack](#start-a-pack): a new language, or a game that has an adapter and no pack yet.

## A few words first

- **Pack**: a translation, kept as a folder of plain text files. It holds the strings it changes and a file,
  `pack.toml`, that says what the pack is: which game, which language, who made it. It holds no part of the
  game. The packs are kept in the [ventris-packs](https://github.com/bquenin/ventris-packs) repository, one
  folder per game and language (`top-sfc/en` is Tales of Phantasia in English).
- **Adapter**: the code inside ventris that knows how to read one game and put text back into it. A game
  can be translated only once it has one. The app lists them on the left of its window.
- **String**: one piece of the game's text: a line of dialogue, a menu label, an item's name.
- **Section** and **address**: the game's text is divided into sections, one file each, and every string
  has an address made of its section and its number, written in square brackets: `[31.004]` is string
  4 of section 31, and it is in the file `31.txt`. Addresses never change, so a correction made today still
  lands on the same string with a later version of ventris.
- **Tag**: whatever in a string is not a character is written in curly brackets: `{PAGE}` for a new page,
  a character's name, a pause, a change of colour. Keep the tags a string has, where they make sense in
  your sentence.

**The packs there now are machine translated and have not been reviewed by a human translator.** They are
working samples: they show that ventris carries a whole game, from the dialogue to the menus and lists, and
they are playable as far as has been seen. Anyone is free to improve them, one line or one part at a time,
or to write a pack of their own in any language. This page explains how.

## Fix a line

You saw a line that reads wrong, means the wrong thing, or does not fit its box. You can fix it yourself
with a pull request, or tell us about it in an issue and leave the fix to someone else.

### Just tell us

If you would rather not edit anything, open an issue in the packs repository with
[the form for a wrong line](https://github.com/bquenin/ventris-packs/issues/new/choose). Say:

- which game and which pack;
- where in the game it is: the place, the scene, what you had just done;
- what it says, copied as exactly as you can;
- what it should say, if you know;
- a screenshot, if you have one.

That is enough for someone to find the string and fix it.

### Find the string

Search the pack's files for the text you saw. Use a few words from the middle of the line rather than the
whole of it: the game breaks lines where they fit the screen, not where the file does, and a character's
name may be a tag in the file. Either of these works:

- on GitHub, the search box on the [packs repository](https://github.com/bquenin/ventris-packs) page
  searches the text of its files when you are signed in;
- in a copy on your computer, your text editor's search across a folder.

The search shows a file and a line. Just above that line is the string's header, such as `[31.004]`. That
address is how to name the string in an issue or a pull request.

Each string in a file is a header line, its text, and a blank line:

```text
[31.004]
The first line
and the second.{PAGE}
The next page.

[31.005]
...
```

A line that starts with `# ` is a comment, for whoever reads the file; it never reaches the game. The
full format is in [the pack format](packs.md#2-get-the-games-script).

If you cannot find the text, and it is still in the game's own language on screen, the pack does not
change that string yet. Say so in an issue: someone with the game's script can find it.

### Change it

Change the text under the header, and nothing else: keep the header, keep the tags, and keep the blank
line after the string. Edit with a plain text editor, not a word processor, and keep the file in UTF-8,
which is what most editors use today.

If the line was too long for its box, a shorter wording is the fix: the box will not grow.

### Look at it

If you can, build the game with your change and look at it in an emulator, at the place where you saw the
line. The app gets the packs from GitHub by itself, but to try a change before sending it, make it in a
copy of the packs on your computer and build from that copy:

1. Get the packs repository with **Code**, then **Download ZIP**, and unpack it. It unpacks to a folder
   named `ventris-packs-main`.
2. Make your change in that folder, in the pack's file.
3. In the app, choose the game, then **Make a translation** at the top. Under **Your pack**, choose
   **Choose your pack's folder…** and pick that folder. Only the packs for this game are taken from it.
4. Under **Build the game to try it**, choose **Build the game…** on your copy of the pack, and open the
   game it writes in your emulator.

[Playing a translated game](playing.md#the-app) goes through the app step by step.

If the build is refused, the app says **The game could not be built**, gives the reason in words, and
writes nothing. See [When a string does not fit](#when-a-string-does-not-fit).

If you cannot build or look, send the change anyway and say that you did not see it on screen.

### Send it

The packs are on GitHub, and a change to them is sent as a pull request. You can do it all in the browser,
with nothing to install:

1. Sign in to GitHub and open the file in the [packs repository](https://github.com/bquenin/ventris-packs).
2. Choose the pencil button, **Edit this file**. GitHub makes your own copy of the repository (a fork) for
   you to edit in.
3. Make the change. If you tried it in a copy on your computer first, make the same change here.
4. Choose **Commit changes**, write a line saying what you changed, and choose **Propose changes**.
5. On the page that follows, choose **Create pull request**. In its description, say where in the game the
   string is, and whether you saw your change on screen.

If you use git, fork the repository, change the files in a branch, and open a pull request in the usual
way. Several fixes can go in one pull request.

## Review a pack

Reviewing is going through a part of a game, with knowledge of the language and of the game, and
correcting what you find. A part can be an area, a chapter, the skits, the menus, the item list: whatever
you can play through and judge.

### Before you start

- Open an issue in the packs repository to say which part you are taking, so that two people do not review
  the same scenes.
- Read the pack's `NOTES.md`. It holds the names and terms the pack has chosen (characters, places,
  skills, recurring words), how its strings are written, and what has and has not been seen on screen.
- Get the game's own script, to compare the translation with the original: the app saves it, as
  [Get the game's script](#get-the-games-script) shows. Keep it to yourself and out of the pack. The
  script's file and the pack's file of the same section have the same headers, so you can put them side
  by side.

### Working through it

- Play the part in the built game and read the pack's files beside it. What the screen shows tells you
  what the file cannot: a line that runs off its box, a name cut short, a sentence that reads wrong where
  it lands.
- Correct the meaning where the translation is wrong, and the wording where it is stiff. Someone who reads
  only the translation's language can still fix wording, spelling and what does not fit; meaning takes
  someone who knows the original.
- **Keep a name the same everywhere.** A character, a place or a skill is named in dialogue, in menus, in
  lists and in the notes, and a player who meets two spellings thinks they are two things. If you change
  an established name, change it in every file of the pack and in `NOTES.md`, in the same pull request.
  Search the whole pack for the old name to find every place.
- If a string needs a note for the next person (why a wording was chosen, what it refers to), write it as
  a comment, a line starting with `# `, under its header.
- Build from your copy of the pack as you go, as [Look at it](#look-at-it) shows, to see your corrections
  on screen.

### Say what you saw

Say in the pull request which part of the game you played through and looked at on screen, and which
strings you changed without seeing them. Add the same to the pack's `NOTES.md`, where it says what has
been seen, so that the next reviewer knows where to start. A string that builds is not yet a string that
works: only the screen shows that.

## Start a pack

You want a translation in a language that has no pack yet, or a pack for a game that has an adapter and no
pack. A pack may be in any language and any alphabet, including a revision in the game's own language.

Open an issue in the packs repository first, with
[the form for a new pack](https://github.com/bquenin/ventris-packs/issues/new/choose): there is one pack per
game and language, and a pack someone has already started is better joined than begun again. To find others
to share the work, or to ask whether someone would take up a language, post in the
[Translations](https://github.com/bquenin/ventris/discussions/categories/translations) category of Discussions.

You need the app, with your own copy of the game found by it
([Playing a translated game](playing.md#the-app) says how), an emulator, and a plain text editor.

### See what the game can take

On the game's page in the app, open **What a translation can change in this game**, near the bottom. It
lists the game's kinds of text (dialogue, menus, lists and so on) and what a pack can do with each:
whether a string may be longer than the game's own, whether new characters can be added (for an alphabet
the game does not have), and the limits, in a sentence. Read the limits of the kinds you mean to translate
before you start. [Notes on each game](packs.md#notes-on-each-game) says the rest of what a pack's author
needs to know about each game, and is worth reading in full.

### Get the game's script

In the app, choose the game, then **Make a translation** at the top. Under **Save the game's script**,
choose **Save the game's script…** and pick a folder.
The button waits until your copy of the game has been found. The app makes a folder inside the one you
picked, named after the game, such as `Tales of Phantasia script`, and writes the whole script there as
UTF-8 text, one file per section. It then checks that the files encode back to the game exactly.

When it is done the app says **The game's script is saved**, with the folder it wrote, **What was
written** to see how many strings of each kind there are, and **Show in folder**. If a folder of that name
is already there and has files in it, nothing is written, so a second save never writes over your work;
the app says so, and you can pick another place.

That script is the game's own text: keep it on your computer, never in a pack, and never share it.

### Write the pack

Make a new folder, and in it one file per section you change, named after the section (`31.txt`). Copy
the strings you change from the game's script, header and all, and replace their text with yours. A
pack holds only the strings it changes; whatever it leaves out keeps the game's own text, so a pack can
start with one line and grow.

Then add a `pack.toml` that says what the pack is:

```toml
format = 1
game = "top-sfc"
language = "fr"
name = "Tales of Phantasia en français"
version = "0.1"
authors = ["Your name"]
license = "CC-BY-NC-SA-4.0"
description = "The dialogue of the first two towns. Machine translated, then corrected by hand."
```

`format = 1` is right for a pack that moves no labels; one that does says `format = 2`. The `license` line
is the licence of the packs repository, which every pack kept there carries. For a pack you keep
somewhere else, the licence is yours to choose, or to leave out.

[The pack format](packs.md) describes every key, character names (`names.txt`), fonts, moving labels, and
the format of the script files in full.

### Translating from another pack

You may translate from the original script, or from a pack in another language, say the English one,
when you do not read the original. If you start from another pack, its licence applies to what you make
from it, and its authors are credited in your `pack.toml`. Say in the description which pack you worked
from, since a translation of a translation carries its mistakes along.

### Build it and look at it

In the app, on the game's **Make a translation** tab, under **Your pack**, choose **Choose your pack's
folder…**, or **Change** once one is chosen, and pick the folder your pack is in. You can pick the pack's own folder, or a
folder that holds it; one that also holds the game's script is fine, since a folder without a `pack.toml`
is not taken as a pack. The folder belongs to that game: only its packs for that game are taken, and each
game can have a folder of its own. A folder with no pack for the game is not kept, and the app says which
game the packs in it are for.

The app reads the pack and shows it under **Your pack**, with its language, name, version and what it
changes, or marked **Cannot be used** with the reason (see below). Reading it checks what can be checked
without the game: its `pack.toml`, how its files are written, and that each string it changes is one the
game has. Whether the text fits the game is only known by building it. It is listed on the **Play a
translation** tab too, first, marked **From your computer**.

Under **Build the game to try it**, choose **Build the game…**. The app asks where to save the translated game, builds it, and says **The translated game is ready**, with
**What was changed** to see the details. Run the built game in an emulator and look at the text you
changed. When the pack adds characters the game's font did not have, start from power-on, not from a saved
state: a saved state holds the game's old font.

A build always reads the pack as it is now, so after you change a file, build again. When you change
`pack.toml`, or add a file, choose **Look again** under **Your pack** so that the app shows the pack as it
is. When you are done, **Stop using it** goes back to the ventris packs alone.

### When the app says a pack cannot be used

The app checks every pack in the folder you chose for the game as far as it can without the game. A pack whose `pack.toml`
or files are wrong is still listed under **Choose a translation**, marked **Cannot be used**, with the
reason in words in place of a **Build the game…** button, for example:

```text
31.txt: '[31.004' has one of its brackets missing: a string's header is written '[31.004]'
```

```text
pack.toml: line 1: missing field `language`
```

Correct what it says, then choose **Look again** under **Your pack**. When the `pack.toml` cannot
be read at all, it is not known which game the pack is for; it is shown on the page of the game you chose
the folder for.

If the app does not list your pack at all, check that its folder has a file named exactly `pack.toml`.
Windows hides the endings of file names unless told otherwise, so a file shown as `pack.toml` may really
be `pack.toml.txt`.

## When a string does not fit

Each game gives its text a certain room, and the build checks it. When a pack asks for something the game
cannot take, nothing is written, and the app says **The game could not be built** and what is wrong and
where, in words, for example:

```text
string menu.004 is 34 characters; the longest a menu string can be is 26
```

```text
the menu font has room for 0 new characters and the pack needs 2 (kt); a tile only comes free once every menu string that uses its kana is replaced
```

Shorten the string, or change what it needs, and build again. The first message is a string that is too
long; the second says the game's small menu font has no room for two more letters until more of the
original's menu text is replaced.

Many limits are not something a build can know. A line may build and still run past the edge of its box,
or into the label beside it. Only the screen shows that, which is why a pack's author looks at what they
changed.

## What a pack must never hold

- **The game's own script, or any file of the game**: no saved script, no built game, no patch made
  from one, no picture of the game's font. A pack holds its authors' own words and nothing else.
- **Another translation's text**, such as a fan translation, unless its licence allows it and it is
  credited. An existing translation may be used to check facts, such as what a name refers to; its
  wording stays its authors'.

## Machine translation

A pack may be machine translated, in whole or in part, as long as it says so in its `pack.toml`, in
`authors` or in `description`. The packs there now say it in both. A reader then knows what they are
getting, and a reviewer knows where to look hardest.

## Working with a coding agent

A coding agent, such as Claude Code or Codex, can do this work with you by command line: extract the
script, keep the pack's files and its glossary in order, build the game, and read what a refused build
says. It cannot drive the app, so it uses the commands in [Commands](commands.md#for-a-translator).

This repository has a skill for it, [`skills/ventris-translate`](../skills/ventris-translate/SKILL.md): a
file of instructions in the [Agent Skills](https://agentskills.io/) format that holds the agent to the
rules on this page. To give it to your agent, copy that folder into the folder your agent takes skills
from: `~/.claude/skills/` for Claude Code, `~/.agents/skills/` for Codex. Other agents have their own
place, which their documentation gives. Any agent can also be asked to read the file.

You stay responsible for what you send. What an agent translates is a machine translation and is
declared as one, and only what someone saw on screen counts as seen.

## The licence

Packs in the packs repository are under the Creative Commons Attribution-NonCommercial-ShareAlike 4.0
licence (CC BY-NC-SA 4.0), as its [README](https://github.com/bquenin/ventris-packs#licence) says, and
their `pack.toml` says so with `license = "CC-BY-NC-SA-4.0"`. It covers only what the pack's authors
wrote. Nothing in it grants any right in the game's own
text, which stays with its rights holders.

## Say what you saw

Whatever the size of your contribution, say what you checked on screen and what you did not. A pull
request that says "seen on screen in the first town; the rest only built" is worth more than one that says
nothing, and it tells the next person where to look.

For the same steps by command line, see [Commands](commands.md#for-a-translator).
