# Playing a translated game

This page is for someone who has a game and wants to play it translated. You need three things: the
ventris app, your own copy of the game, and an emulator to play it in. The app gets the translation packs
by itself. None of this needs programming, and no terminal is needed.

Read [Before you start](#before-you-start), then follow [The app](#the-app) to the end. If you would
rather type commands in a terminal, the command-line program does the same job through the same engine;
[If you prefer a terminal](#if-you-prefer-a-terminal) points to it.

The program never comes with a game and never downloads one. You bring your own copy; it is read, and a
translated copy is written where you say. Your file is not changed. When the app starts it contacts GitHub,
and only for this: to get the translation packs, which are text.

## Before you start

### Which games

Today there are five games, all Japanese releases: Tales of Phantasia for the Super Famicom, G.O.D pure
for the PlayStation, Tales of Destiny 2 and Super Robot Wars Z for the PlayStation 2, and Ganbare Goemon:
Tōkai Dōchū Ōedo Tengurigaeshi no Maki for the Nintendo DS. Each has an adapter, the code that lets
ventris read and rebuild that one game. Super Robot Wars Z can only have its script read so far: a
translated copy of it cannot be built yet.

### Your copy of the game

A game is recognised by the exact contents of its file, so only the release its adapter was made for will
do. The app states that release in full on the game's page when it does not find your copy: what the file
says it is (a disc's serial, a cartridge's title) and its version, its size in bytes, its SHA-1, and its name
in the catalogue of dumps that lists it, which is the name download sets and tools know it by, so that you
can check a file against it. `ventris games` prints the same.

A Super Famicom game can be kept as its image (`.sfc`, `.smc`, `.swc` or `.fig`, with or without a
copier's header) or as a zip archive that holds one image. A PlayStation game is its disc image, the
`.bin` file of its data track, with or without the `.cue` sheet kept beside it. A PlayStation 2 game is its
disc image (a file ending in `.iso`, `.bin` or `.img`). A Nintendo DS game is its cartridge image (`.nds`)
or a zip archive that holds one.

If your game is not found, or a file you choose is refused, your file is not the exact one the adapter
knows. When you choose a file, ventris says what it is, as far as it can tell, and what is needed instead;
when a copy in your games folder is the game but not the file needed, the game's page names it, with why.
Usually it is for one of these reasons:

- **It is compressed or in an archive**, and has to be unpacked or converted back to the game's plain file
  first.
- **It is a different release**: another region, another version of the same region, or a re-release.
  Others need an adapter of their own.
- **It already has a patch applied**, such as a fan translation or a fix. A patched file is a different
  file. Use a clean copy of the game.

A damaged or incomplete copy is not recognised either.

### The packs

A translation pack is a folder of text files: the translated strings of a game and a file, `pack.toml`,
that says what the pack is. The packs are kept in the
[ventris-packs](https://github.com/bquenin/ventris-packs) repository, one folder per game and language:
`top-sfc/en` is Tales of Phantasia in English.

There is nothing to download: each time the app starts, it gets the packs from that repository on GitHub,
as one archive, and keeps a copy on your computer. That is the only time the app uses the network, and
GitHub the only place it contacts. When GitHub cannot be reached, the app uses the copy it kept and says
from when that copy is. A pack of your own for a game, such as one you are writing or one you got from
someone, can be listed as well; [3. The translations](#3-the-translations) says how.

**The packs there now are machine translated and have not been reviewed by a human translator yet.** They
are working samples, and only parts of each have been seen on screen, so expect lines that read badly, and
some that do not fit their box. Each pack's `NOTES.md` says what has been seen. If you spot something wrong
while playing, [Translating a game](translating.md#fix-a-line) says how to report it or fix it yourself.

## The app

### 1. Install it

Download the installer for your system from the
[Releases page](https://github.com/bquenin/ventris/releases):

| System | File |
| --- | --- |
| Windows | `ventris-app-0.4.0-windows-x86_64-setup.exe` |
| macOS, Apple silicon (M1 and later) | `ventris-app-0.4.0-macos-aarch64.dmg` |
| macOS, Intel | `ventris-app-0.4.0-macos-x86_64.dmg` |
| Linux | `ventris-app-0.4.0-linux-x86_64.AppImage`, or `ventris-app-0.4.0-linux-x86_64.deb` for Debian and Ubuntu |

- **Windows**: run the installer. It installs for your own account, without administrator rights. Then
  start ventris from the Start menu.
- **macOS**: open the `.dmg` and drag ventris into Applications. Then open it from there.
- **Linux**: make the AppImage executable and run it:

  ```bash
  chmod +x ventris-app-0.4.0-linux-x86_64.AppImage
  ./ventris-app-0.4.0-linux-x86_64.AppImage
  ```

  Or install the `.deb` with `sudo apt install ./ventris-app-0.4.0-linux-x86_64.deb` and start ventris from
  your applications menu.

The app is not signed yet, so Windows and macOS warn the first time it is opened.
[First launch: security warnings](#first-launch-security-warnings) says what you will see and what to do.
So far the app has been used on Windows only; if it does not work on yours, please
[say so](https://github.com/bquenin/ventris/issues).

### 2. Tell it where your games are

The window lists the games on the left. Choose one, and its page has two steps. Under **Your copy of the
game**, choose **Choose your games folder…** and pick the folder you keep your games in. The app looks
through it, and the folders inside it, for the games it can translate; a tick beside a game in the list
means it found your copy, and the page says **Found in your games folder**.

The app remembers the folder and which file each game is, and looks through the folder again each time it
starts. The folder is shown at the bottom left under **Your games**, with **Change** to pick another and
**Look again** to search it now.

If your copy is kept somewhere else, or the search did not find it, choose its file instead: **or choose
this game's file**, or, once a games folder is set, **Choose the game's file…**. If it is still not
recognised, see [Your copy of the game](#your-copy-of-the-game).

### 3. The translations

There is nothing to do. At the bottom left, under **Translations**, the app says that it is getting them
from the ventris packs on GitHub, and then that they are up to date, with the date and time. The first
start takes a moment; after that, the copy kept from before is shown at once while the app looks for new
ones. **Look again** gets them again now.

If they cannot be got, the app says so in words, with the reason. With a copy from before, it lists that
copy and says from when it is. With none, the game's page says so; if GitHub could not be reached, check
that the computer is online, then choose **Look again**.

**A pack of your own** is optional: one you are writing, or one you got from someone. It belongs to one
game, and is chosen on the other side of the window: choose the game, then **Make a translation** at the
top, and under **Your pack**, **Choose your pack's folder…**, and pick the folder it is in. Every folder
under it, up to four levels down, that has a `pack.toml` is taken as a pack, and only the packs for this
game are listed. If the folder holds no pack for this game, the app says so, names the game its packs are
for, and does not keep it. The app remembers the folder for this game alone. Back on **Play a
translation**, its packs are listed first, each marked **From your computer**, and the ventris packs after
them. **Look again** under **Your pack** reads the folder again, **Change** picks another, and **Stop using
it** goes back to the ventris packs alone.

A pack that has a mistake in it is listed too, marked **Cannot be used**, with the reason in words. With the
ventris packs that should not happen; if it does, please
[report it](https://github.com/bquenin/ventris-packs/issues). The one exception is a pack made for a newer
version of ventris, which says that this version cannot use it.

### 4. Pick a pack and build

Each pack for the game is shown with its language, name and version, what it says about itself, who made
it, how many strings it changes and its licence; a pack of your own is marked **From your computer**.
Choose **Build the game…** on the one you want. The button waits until your copy of the game has
been found.

The app asks where to save the translated game, and offers a name beside your own file, such as
`Tales of Phantasia (Japan) (Tales of Phantasia in English 0.9, en).sfc`. A disc game is written as a
whole new disc image (for a PlayStation game, a `.bin` with a `.cue` beside it), so it needs as much free
space as the disc is large, and takes longer than a cartridge game.

When it is done the app says **The translated game is ready**, with the file it wrote, **What was changed**
to see the details, and **Show in folder**. If the build is refused, it says **The game could not be
built**, gives the reason in words, and writes nothing. With a pack from the packs repository and a
recognised game that should not happen; if it does, please
[report it](https://github.com/bquenin/ventris/issues).

Then [play it](#play-it).

The other side of the window, **Make a translation**, has the translator's steps for the same game: the
game's script to translate from, the folder of your pack, and a build to try it.
[Translating a game](translating.md) goes through them.

## Play it

Open the file you built in an emulator, as you would the original. The built games have been looked at in
[Mesen](https://github.com/nesdev-org/MesenCE) for the Super Famicom,
[PCSX-Redux](https://github.com/grumpycoders/pcsx-redux) for the PlayStation, [PCSX2](https://pcsx2.net/)
for the PlayStation 2 and [DeSmuME](https://github.com/TASEmulators/desmume) for the Nintendo DS. Other
emulators have not been tried.

Start the translated game from power-on. A saved state made with the original game holds what was in the
console's memory at the time, the original font included, and loading it into the translated game mixes
the two.

## If you prefer a terminal

The app is all you need to play. If you would rather work in a terminal, or want to build a game from a
script, the command-line program, `ventris`, builds the same file from the same game and pack.
[Commands](commands.md#for-a-player) says how to get it and gives the steps of this page as commands.

## First launch: security warnings

The app and the command-line program are not signed with a paid certificate yet. A signature is how
Windows and macOS know who made a program, so without one they warn the first time it is opened. That is
expected, and the steps below get past it. Download ventris only from the project's
[Releases page](https://github.com/bquenin/ventris/releases), and never from a copy someone else hosts.

### Check the download

The Releases page has a file `SHA256SUMS` beside the downloads, with a checksum for each. To check a
download, compute its checksum and compare it with the line for that file:

| System | Command |
| --- | --- |
| Windows | `certutil -hashfile FILE SHA256` |
| macOS | `shasum -a 256 FILE` |
| Linux | `sha256sum FILE` |

`FILE` is the name of the file you downloaded. If the two do not match, do not run it: download it again,
and if they still differ, please [report it](https://github.com/bquenin/ventris/issues).

### Windows

**The app's installer.** You may see a blue screen, "Windows protected your PC". Choose **More info**,
check that the name of the file is the one you downloaded, then choose **Run anyway**. On a computer
managed by an employer or a school, this may be blocked, and only whoever manages it can allow it.

**The command-line program.** Unpacked and run from a terminal, it usually brings no warning. Opened with
a double click, the same screen may appear, with the same way past.

**Smart App Control.** Where Windows 11's Smart App Control is on, it blocks programs that are not signed,
and it has no way to allow a single program. The only choices then are to turn Smart App Control off for
the whole computer, in Windows Security, or to wait for a signed version of ventris.

### macOS

**The app.** Open it once. macOS says that it cannot check the app, and does not open it. Then open
**System Settings**, go to **Privacy & Security**, and under **Security** choose **Open Anyway**, which is
offered for about an hour after the first try. Confirm with your password. After that the app opens like
any other. Control-clicking the app and choosing Open no longer gets past this on macOS 15 and later. The
app is signed ad hoc, without a certificate, which Apple silicon needs for a program to run at all.

**The command-line program.** You may see the same refusal the first time you run it. Allow it the same
way, in **Privacy & Security**, or remove the mark macOS puts on a downloaded file, in the folder you
unpacked it into:

```bash
xattr -d com.apple.quarantine ./ventris
```

### Linux

There is no warning.

## Building from source

To build either program yourself, install [Rust](https://www.rust-lang.org/tools/install) (1.89 or later)
and, in a copy of this repository, run `cargo build --release`, which puts `ventris` in `target/release/`;
the [README](../README.md#getting-it) says more.

The app also needs [Node.js](https://nodejs.org/) and what [Tauri](https://tauri.app/start/prerequisites/)
needs on your system. Then:

```bash
cd app
npm install
npm run tauri build -- --no-bundle    # target/release/ventris-app, without an installer
```
