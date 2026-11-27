# Adding a game

This page is for a programmer who wants ventris to support a game it does not support yet. The work is
reverse engineering: finding where the game keeps its text, how it is encoded, and how it is drawn, then
writing that down as code. If you can read assembly, or are willing to learn on the way, you can do it.

If you would like a game supported and are not going to write the adapter yourself,
[ask for it](#asking-for-a-game) instead.

Questions and findings along the way go in the
[Adapters](https://github.com/bquenin/ventris/discussions/categories/adapters) category of Discussions, where
two posts tell how the existing adapters were made:
[Tales of Phantasia](https://github.com/bquenin/ventris/discussions/2) and
[Tales of Destiny 2](https://github.com/bquenin/ventris/discussions/3).

## What an adapter is

An **adapter** is the code for one game: one Rust module, a folder of its own under
`crates/ventris/src/adapters/`, holding what has been worked out about the game. It says which game it is,
which bodies of text it has and what a pack can do with each, and it extracts the script, checks that it
encodes back exactly, and builds the game with a pack's strings in it.

A **pack** is the other half: a translation, written as plain text by anyone, with no code in it. The two
meet only in the contract, [`crates/ventris/src/adapter.rs`](../crates/ventris/src/adapter.rs). An adapter
written once lets anyone translate the game into any language; [Translating a game](translating.md) is
what a pack's author sees.

What an adapter is not:

- **It is not a translation.** It holds no translated text, and its tests do not depend on one.
- **It is not the game.** It holds what was learned about the game, not the game's bytes. A table of the
  game's characters, the addresses of its routines, the layout of its archive: those are knowledge and go
  in. Its script, its font, its code, its pictures: those are the game and stay out.
- **It is not loaded from elsewhere.** Adapters are compiled into ventris. A new game arrives as a pull
  request that adds a module and one line to the list in `adapters/mod.rs`, and the maintainer reviews it.
- **It does not change the game beyond its text.** See [Scope](#scope).

An adapter that only extracts the script is a whole contribution. It lets translators read the game and
start work, and the build can come later, from you or from someone else.

## What you need

- **The game**, the release you mean to support, as a clean image. Your own copy; it never goes into the
  repository.
- **An emulator with a debugger**: Mesen for the Super Famicom, PCSX-Redux for the PlayStation, PCSX2 for
  the PlayStation 2, DeSmuME for the Nintendo DS. The workbench drives all four, once they are
  [set up](#setting-up-the-emulators) for it.
- **The workbench**, the Python package in `src/ventris_workbench`: tools for taking a game apart. Install
  [uv](https://docs.astral.sh/uv/) and Python 3.11 or later; `uv run ventris-workbench --help` lists the
  tools. Nothing in it is part of what a translator runs.
- **Rust** (1.89 or later) to build ventris and run its tests.
- A decompilation or a commented disassembly of the game, if there is one. Look before you start: it takes
  minutes, and where one exists it has already written down, with names, what you would otherwise trace in an
  emulator: where the text is, its encoding, what points at it, the compression, the routine that draws it.
  Many are of another release than yours, often the American one: the formats carry over, but the addresses
  have to be found again. Like a fan translation, it is an answer key for facts. Decompiled code is the game's
  code in another form, so none of it goes into an adapter; check its licence before taking more than facts.
- An existing fan translation of the game, if there is one. It is an answer key for facts: comparing it
  with the original shows where the text is and what had to change to move it. Check its licence before
  taking more than facts.

The engine keeps its rules in [CONTRIBUTING.md](../CONTRIBUTING.md). Read it before you start.

### Working with a coding agent

If you work with a coding agent, such as Claude Code or Codex, give it the skill
[`skills/ventris-adapter`](../skills/ventris-adapter/SKILL.md): a file of instructions in the
[Agent Skills](https://agentskills.io/) format that holds it to this page and to the rules. Copy that
folder into the folder your agent takes skills from: `~/.claude/skills/` for Claude Code,
`~/.agents/skills/` for Codex; other agents have their own place, which their documentation gives. Any
agent can also be asked to read the file. [AGENTS.md](../AGENTS.md), at the top of the repository, says
in short which skill is for which job, the rules and the checks; Codex and many other agents read it by
themselves in a copy of the repository.

You stay responsible for the pull request: read the code, and say what was seen on screen and what was
not.

## Setting up the emulators

You need the emulator for your game's console, set up once. The last step for each is a command that
proves the workbench can drive it. All of this has been done on Windows. The emulators run on macOS and
Linux too, but the workbench's commands have not been tried with them there.

Two things are yours to supply, and neither ever goes into the repository: the game, your own copy, and
for the PlayStation and the PlayStation 2 a BIOS dumped from your own console. PCSX-Redux and PCSX2 cannot
start a game without one. DeSmuME needs none: it stands in for the Nintendo DS's own.

### Mesen, for the Super Famicom

Use [Mesen Community Edition](https://github.com/nesdev-org/MesenCE) in a build newer than its 2.2.1
release: a development build, linked from its README, or one you build from source with its own
[instructions](https://github.com/nesdev-org/MesenCE/blob/master/COMPILING.md). The workbench has been
used with a build of commit `4b8a196` (October 2026). Release 2.2.1 is too old for `probe`: when a script
watches memory other than what the processor sees, such as the cartridge (`snesPrgRom`), 2.2.1 gives it
the processor's address instead of the address in that memory, so `--reads` and `--writes` would report
the wrong span. Mesen fixed that after 2.2.1.

1. Run Mesen once and go through its setup wizard. Where it asks where to keep its data, choose **Store
   data in the same folder as the application**. The workbench knows Mesen is set up by finding its
   `settings.json` beside the binary; with the data in your user profile it keeps saying Mesen has not
   been set up.
2. Set the environment variable `MESEN` to the path of the binary (`Mesen.exe` on Windows).

Nothing else in Mesen needs changing. The workbench starts it for each run, headless, with no window, and
gives it on the command line what it needs: Mesen's test runner, and file access for the script that runs
inside it and writes what it records.

To check, run the game for 600 frames, ten seconds of play, and take a screenshot at the end:

```bash
uv run ventris-workbench probe roms/game.sfc -o work/game/first --frames 600 --shot 600
```

It takes a few seconds and prints the path of the one file it wrote, `work/game/first-shot-600.png`: the
game ten seconds after power-on, usually its opening or title screen. An error that names `MESEN` or the setup
wizard sends you back to the two steps above.

### PCSX-Redux, for the PlayStation

[PCSX-Redux](https://github.com/grumpycoders/pcsx-redux) has a Lua scripting interface of its own, which
is all the workbench needs: no patch. Build it from source; the workbench has been used with a build of
commit `dbda684a4` (November 2026). On Windows that takes Visual Studio 2026 with its C++ tools and two more
components, `Microsoft.VisualStudio.Component.VC.Llvm.Clang` and
`Microsoft.VisualStudio.Component.VC.Llvm.ClangToolset`, because the configuration to build is
`ReleaseWithClangCL`. The plain `Release` configuration, with Microsoft's own compiler, fails: the Lua
files it builds into the program are longer than that compiler allows a string to be. From a Developer
Command Prompt, in the folder of the source:

```bash
MSBuild vsprojects/pcsx-redux.sln -t:restore -p:RestorePackagesConfig=true
MSBuild vsprojects/pcsx-redux.sln -p:Configuration=ReleaseWithClangCL -p:Platform=x64 -p:PreferredToolArchitecture=x64 -m:4
```

The first command fetches the packages the build needs. In the second, `PreferredToolArchitecture=x64`
matters: without it the build links with the 32-bit linker, which runs out of memory writing the debugging
information. The program is `vsprojects/x64/ReleaseWithClangCL/pcsx-redux.exe`. On macOS and Linux,
PCSX-Redux's own README says how to build it.

1. Set the environment variable `PCSX_REDUX` to the path of the binary.
2. Set `PCSX_REDUX_BIOS` to the path of a BIOS dumped from your own console, such as `scph5500.bin` for a
   Japanese one. Use a BIOS of the game's region.

Like Mesen, PCSX-Redux is started by the workbench for each run, with no window, and given on the command
line the game, the BIOS and the script to run; it quits when the script is done. Its settings and memory
cards go to a new empty folder for each run, so a run never depends on what an earlier one saved
(`--portable` names a folder to keep them in instead).

To check, run the game from power-on for 600 frames and take a screenshot at the end:

```bash
uv run ventris-workbench ps1-probe roms/game.cue -o work/game/first --frames 600 --shot 600
```

It takes a few seconds and prints the path of the screenshot it wrote, `work/game/first-shot-600.png`: ten
seconds after power-on, which is still the console's own start-up screen, the Sony logo. An error that names `PCSX_REDUX` or `PCSX_REDUX_BIOS` sends you back to the two steps
above.

### PCSX2, for the PlayStation 2

The workbench drives PCSX2 through its control socket, PINE, with commands PCSX2 does not have: pause,
advance frame by frame, take a screenshot, press buttons, record which code touches memory. A patch in
[`tools/pcsx2`](../tools/pcsx2/README.md) adds them, so a released PCSX2 will not do: build it from
source with the patch applied. That page says how. In short: apply `pine-automation.patch` to
[PCSX2's source](https://github.com/PCSX2/pcsx2) at commit `c8081bf` (November 2026), which it was
written against and used with; build the way PCSX2's own CI does; and change three settings so that it
starts without its first-run wizard, with the control socket on and your BIOS in the `bios` folder beside
the binary. A later PCSX2 may need the patch brought up to date.

Unlike Mesen, PCSX2 is not started by the workbench. Start it with your game, for example
`pcsx2-qt -fastboot -- game.iso` (`-fastboot` skips the console's start-up screen), and `ps2-probe`
connects to it. On Windows PCSX2 listens on port 28011 of the same machine, which is where the workbench
looks. On macOS and Linux it listens on a socket file instead, `pcsx2.sock` in the folder that `TMPDIR`
(macOS) or `XDG_RUNTIME_DIR` (Linux) names, and the workbench does not look there by itself: set
`PCSX2_PINE` to that file's path. `PCSX2_PINE` also takes a `HOST:PORT`.

To check, with the game running:

```bash
uv run ventris-workbench ps2-probe -o work/game/first --frames 60 --shot 60
```

It pauses the game, runs it 60 frames on from wherever it was, as fast as the machine allows, and prints
the path of the screenshot it wrote, `work/game/first-shot-60.png`. The game is left paused. An error
saying `cannot reach PCSX2` means the control socket is off or not where the workbench looks; one saying
`PCSX2 refused command` means no game is running, or the build lacks the patch.

### DeSmuME, for the Nintendo DS

The workbench runs DeSmuME as a library, the interface frontend that DeSmuME builds as a DLL on Windows,
and needs commands that library does not have: which processor and which instruction touched memory,
reading memory without setting off a watch, and where the game's battery save goes. A patch in
[`tools/desmume`](../tools/desmume/README.md) adds them, so a released DeSmuME will not do: build the
library from source with the patch applied. That page says how. In short: apply `interface-hooks.patch`
to [DeSmuME's source](https://github.com/TASEmulators/desmume) at commit `b549f71` (November 2026), which
it was written against and used with; build `DeSmuME_Interface.sln` in its Release configuration for
x64; put `SDL2.dll` beside the library it makes; and set the environment variable `DESMUME` to the
library's path. A later DeSmuME may need the patch brought up to date.

Like Mesen, DeSmuME is started by the workbench, for each run, with no window, in a process of its own,
so a crash inside the emulator ends the run and not the workbench.

To check, run the game for 600 frames, ten seconds of play, and take a screenshot at the end:

```bash
uv run ventris-workbench ds-probe roms/game.nds -o work/game/first --frames 600 --shot 600
```

It takes a few seconds and prints the path of the one file it wrote, `work/game/first-shot-600.png`: both
screens, the top one above the bottom one, ten seconds after power-on, usually the game's opening or title
screen. An error that names `DESMUME` says the variable is not set or does not lead to a file; one saying
the library is not built with `interface-hooks.patch` means it is DeSmuME's own, or another file.

## From a line on screen to where it is stored

Every game is different, but the road is the same: get a line of text on screen, find it in memory, work
out its encoding from the bytes and the font, and trace it back to where it is stored and what points at
it. The two adapters' module comments are what that road found in two games; read them for the shape of
the answers.

### Know the file

```bash
uv run ventris-workbench identify roms/game.sfc
```

hashes the file and says what it is: for a cartridge, its internal header and mapping. The SHA-1 is what
the adapter will recognise the game by. The adapter also gives, for each release, what the image says it is
(a disc's serial and version, a cartridge's title and version byte), which `ventris check` confirms, and
the name the catalogue of dumps for its console gives it, matched on the hash, with the address of the
record where the record has one of its own. It can list the game's other releases it does not read
(`refuses`), so that a player who has one is told so.

If there is a fan translation, apply it to a copy and compare the two. The regions that differ are where
the text and the code that draws it are:

```bash
uv run ventris-workbench apply roms/game.sfc roms/translation.ips -o roms/game-translated.sfc
uv run ventris-workbench diff roms/game.sfc roms/game-translated.sfc --sort size
uv run ventris-workbench make-patch roms/game.sfc roms/game-translated.sfc -o roms/translation.bps
```

`apply` takes an IPS or BPS patch, and `make-patch` writes a BPS patch from an original and a modified file.
Keep all of these under `roms/` or `work/`, which git ignores.

### Watch the game run (Super Famicom)

`probe` runs a game headless in [Mesen](#mesen-for-the-super-famicom) with scripted input and records what
it does: screenshots, memory dumps, which code reads a range of memory, and a disassembly of the
instructions that actually ran.

```bash
# play to a point, then see which code reads the font and what the text routine executes
uv run ventris-workbench probe roms/game.sfc -o work/game/run --input 0:mash:start,a \
    --trigger C8E555@6000 --frames 300 --shot 240 \
    --reads snesPrgRom:300000-30C81F --exec C8E500-C8E6FF
```

Input steps are `FRAME:KIND:BUTTONS`, where the kind is `mash`, `press`, `hold` or `none`. With `--trigger`,
frame numbers count from the moment the given address first executes.

Other things it records: `--dump` (memory at a frame, such as `snesWorkRam@300`), `--writes` (which code
writes a range), `--registers` (the video registers, to see which layer text is on), and
`--log ADDRESS:MEMORY:START-END`, which adds a line every time one instruction runs, with the registers,
the top of the stack and a few bytes of memory. The log is how to find where a routine is called from and
with what, such as every script a game starts and its address. `--state` starts from a saved state and
`--save` saves one.

Two more tools read the cartridge itself:

```bash
uv run ventris-workbench addr c0ffc0 --mapping HiROM --to pc
uv run ventris-workbench disasm roms/game.sfc C8E550 --count 40
```

`addr` converts between the console's addresses and offsets in the file, for LoROM, HiROM and ExHiROM
cartridges; `disasm` disassembles 65816 code at an address.

A useful order: find the font in the file (it is often uncompressed, and Mesen's tile viewer shows it),
record which code reads it while a line is drawn, follow that code back to where it fetches the next
character, and from there to where the string is and what points at it. The bytes of a string you can
read on screen, set beside the glyphs they draw, give the encoding.

### Watch the game run (PlayStation)

`ps1-probe` does for [PCSX-Redux](#pcsx-redux-for-the-playstation) what `probe` does for Mesen: it runs the
game from power-on, or from a saved state, presses buttons at given frames, takes screenshots, saves memory,
records which code reads or writes a range of memory and what the registers hold each time an address is
run, and saves a state on the way. It runs as fast as the machine allows; `--real-time` slows it to the
game's own speed.

```bash
# from power-on to the title screen, then start a new game and save a state there
uv run ventris-workbench ps1-probe roms/game.cue -o work/game/start --frames 3000     --input 2400:press:circle --shot 2400 3000 --save 3000:work/game/start.state

# from that state, see which code writes a buffer and dump all of RAM
uv run ventris-workbench ps1-probe roms/game.cue -o work/game/run --state work/game/start.state     --frames 120 --writes 800DB600-800DB700 --dump ram@60 vram@60 --shot 0 120
```

Frame 0 is the first frame after power-on or, from a state, the moment it is loaded, which shows the
picture the state was saved with. Input steps are `FRAME:KIND:BUTTONS` as for the other probes, with the
PlayStation's buttons: `cross`, `circle`, `square`, `triangle`, `start`, `select`, `up`, `down`, `left`,
`right`, `l1`, `r1`, `l2`, `r2`. Japanese games confirm with circle, not cross.

Addresses are in hexadecimal. RAM is 2 MB, which the game's code sees at `80000000` to `801FFFFF`; an
address of RAM may be given without the `80`, and the files always show it with. What it writes, all named
after `-o`:

- `-shot-FRAME.png`, the picture on screen.
- `-ram-FRAME.bin`, all of RAM; `-vram-FRAME.bin` and `.png`, the video memory, 1024 by 512 pixels of 16
  bits, where the font and the other pictures are; `-scratch-FRAME.bin`, the 1 KB scratchpad; and
  `-dump-ADDRESS-FRAME.bin` for `--dump ADDRESS:LENGTH@FRAME`.
- `-reads.txt` and `-writes.txt`, a line for every access to the ranges given: the frame, the instruction's
  address (`pc`) and code (`op`), the address touched, the width, the value read or written, and a few
  registers. `-reads-summary.txt` and `-writes-summary.txt` sum them up, a line per instruction.
- `-exec.txt`, a line with every register each time an address given to `--exec` runs, and
  `-registers-FRAME.txt` for `--registers`.

`--trace-from FRAME` waits until that frame to start watching. Watching memory or addresses runs the
emulator's interpreter with its debugger, which is slower: from power-on to a title screen 2400 frames in,
forty seconds at the game's own speed, took four seconds without a watch and fifteen with one.
`--state` also takes a state saved from PCSX-Redux's own window.

The workbench package also holds a reader for PlayStation disc images, `ventris_workbench.platforms.ps1`:
it opens a `.cue` or a `.bin`, lists the files on the disc with their first sector and size, reads them, and
reads whole sectors, which movies and XA audio need. Scripts written to study one game are scratch; keep
them under `work/`.

### Watch the game run (PlayStation 2)

With the [patched PCSX2](#pcsx2-for-the-playstation-2) running the game, `ps2-probe` does what `probe`
does for Mesen: load a state, press buttons at given frames, take screenshots, save memory, record which
code reads or writes a range and what the registers hold each time an address is run, and save a state on
the way. [`tools/pcsx2`](../tools/pcsx2/README.md) says what was measured, such as what watching memory
costs.

```bash
uv run ventris-workbench ps2-probe -o work/game/run --state work/game/town.p2s \
    --input 0:press:cross --shot 60 --reads 1F2000-1F2400 --frames 120
```

The workbench package also holds what a script written to study one game needs: reading the files inside
a PlayStation 2 disc image, the compression schemes met so far, and the client for the emulator's control
socket. Scripts written to study one game are scratch; keep them under `work/`.

### Watch the game run (Nintendo DS)

`ds-probe` runs a game headless in [DeSmuME](#desmume-for-the-nintendo-ds) and does what `probe` does for
Mesen: press buttons at given frames, take screenshots of both screens, save memory and the registers,
record which code reads or writes a range and which instructions run in another, and save a state on the
way. [`tools/desmume`](../tools/desmume/README.md) says what was measured, such as what a watch costs.

```bash
# from a state in a scene with text, see which code writes the main engine's sprites, where many games
# draw their text
uv run ventris-workbench ds-probe roms/game.nds -o work/game/run --state work/game/scene.dst \
    --input 0:press:a --touch 60:128,96 --shot 120 --writes 06400000-0643FFFF --frames 120
```

Input steps are the same as for `ps2-probe`: `mash` presses its buttons together, a few frames on and a
few off. `--touch FRAME:X,Y` touches the bottom screen, at a point counted from its top left, for a few
frames, or for as many as a third number gives (`60:128,96:30`). The console has two processors, and a watch sees both: each line of `-reads.txt`,
`-writes.txt` and `-exec.txt` says which one, the ARM9, which runs the game, or the ARM7, which looks after
sound, touch and the wireless, and the address of the instruction. `--dump` takes a memory by name, such
as `main@300` for the 4 MiB of main memory, `bgb@300` for the sub engine's video memory, or an address and
a length as the ARM9 sees them. Each run starts the game afresh, from power-on or from `--state`, and the
same run gives the same result, so a run can be repeated with a watch added. Use `--trace-from` to set the
watches only from the frame where they are wanted: they slow the game, `--exec` most of all.

## Two adapters, two situations

Each adapter's module comment describes what was found, for a reader who has not seen the game. They are
the best guide to what an adapter has to work out.

**Tales of Phantasia, Super Famicom ([`top_sfc/mod.rs`](../crates/ventris/src/adapters/top_sfc/mod.rs)).** A game
that draws its dialogue at one fixed width, 14 pixels a glyph. Its script is compressed, one pair of blocks
per area of the game, and an area's events and dialogue must fit together in a fixed piece of memory. To
put a translation in, a variable-width renderer is injected (assembly in the adapter's folder, in the `.asm`
files), dialogue that no longer fits in memory is left on the cartridge and read one string at a time, and
the menus, with their own 8x8 font, and the captions, a second text system with its own font and pool, are
handled each in their own way. It shows what it takes when the game's own way of drawing text cannot carry
a translation.

**Tales of Destiny 2, PlayStation 2 ([`tod2_ps2/mod.rs`](../crates/ventris/src/adapters/tod2_ps2/mod.rs)).** A disc
with one large archive and many bodies of text inside it, each found and rebuilt its own way: story
scripts and skits in compressed containers, menus as program modules where strings are found by what
points at them, lists in the main program's data, enemy names in records, and the scripts of the games
within the game. A string that grows moves, the references to it are rewritten, the member is compressed
again and the archive and its table of contents are written out again. It shows how an adapter grows one
body of text at a time, and how strings found by what points at them are numbered by where they are, so
that their addresses hold when more is found later.

## Writing the adapter

The contract is [`crates/ventris/src/adapter.rs`](../crates/ventris/src/adapter.rs). Its module comment
states the three promises an adapter keeps; the comments on `Adapter` and `TextKind` say what each part
means.

1. Add the folder `crates/ventris/src/adapters/<game>_<console>/` and in it `mod.rs`. Its module comment
   describes what you found, for a reader who has not seen the game: where the text is, how it is encoded,
   how the game draws it, and how a changed script is put back.
2. In it, implement `Adapter` on one empty type and give a `pub static ADAPTER` of that type. The table at
   the top of [`adapters/mod.rs`](../crates/ventris/src/adapters/mod.rs) lists what each part says.
3. Add the module to `adapters/mod.rs` and the adapter to `ALL`.
4. Use what is shared. The second table in `adapters/mod.rs` lists the modules an adapter is expected to
   reuse rather than write again: the tag language and character tables, the round trip, glyphs from
   fonts, line breaking, placing text that outgrew its room, compression, and what is about a console
   (under `platforms/`). Something that is about a console and not about your game goes there too.
5. Write tests in `tests.rs` in the same folder, declared in `mod.rs` as `#[cfg(test)] mod tests;`. A
   test that needs the game finds it with `crate::testing` and checks nothing when it is not there, so the
   tests pass on a machine without the game.

How a longer line is made to fit differs in every game and is the adapter's own business. What the
contract fixes is that it either works or is refused, in words a translator can act on: which string, what
the limit is, and what to do.

Write `limits` on each kind of text for a translator, in a sentence. They see it on the game's page in the
app, and in `ventris games`. Where
a string is limited in a way its text does not show, `note` puts a comment in the extracted script.

### Addresses never change

A string is named by its address, `section.number`, and packs depend on it. Once an adapter is released, a
string keeps its address for good. Text found later gets new sections; it is never numbered in between.
Where strings are found by what points at them, and more may be found later in the same place, number them
by where they are (`numbered_by_place`), so that a string's number does not depend on what else is known.
Choose the sections and numbers with this in mind before the first release.

## Checking it

```bash
ventris check roms/game.sfc
```

holds the adapter to the contract as far as a program can: the script survives a round trip, an unchanged
script rebuilds the image byte for byte, each body of text takes a changed string (the `trial` each kind
names), and what a pack must not do is refused.

It cannot look at the screen. A build that `check` passes can still draw garbage, crash in a scene the
check never reached, or cut a line short. That takes an emulator and a person.

### Look at the screen

A change to what the game shows is not done until it has been seen in a screenshot from the emulator.
Build with a few changed strings, in the kinds of text you support, and look at each where the game shows
it. When the font changes, start from power-on: a saved state carries the video memory, and with it the
old font.

## What a pull request shows

- The module, with its module comment, its tests, and its line in `ALL`.
- The output of `ventris check` on the game.
- Screenshots from an emulator of changed text, in each kind of text that can be inserted, from power-on
  when the font changed. A screenshot of a few test strings of your own is enough; it shows the game, so
  keep it to the pull request.
- What was verified and what was not: which scenes were seen, which kinds were only built, what is known
  not to work.
- The release the adapter was made for, and where a fan translation, a decompilation or a disassembly was used
  as an answer key.
- The checks in [CONTRIBUTING.md](../CONTRIBUTING.md) passing.

No game file goes in: no image, no extracted script, no font sheet, no saved state, no patch made from a
game. A character table is knowledge and goes in; so is a table of the game's addresses.

## Scope

What ventris translates is what a game says. It does not turn the game into one made for another language.

In scope is the text an adapter can take out and put back, such as dialogue, menus, names and lists, along
with what it takes to draw a translation's alphabet and to give a longer string room. Out of scope is
changing the game beyond its text: a screen where the player enters a name from a grid of the original's
letters, boxes laid out again so that longer text fits, pictures with text drawn into them, anything in how
the game behaves. An adapter leaves those as the game has them, and what fits on screen is up to a pack's
author. A build option that changes something besides the text, such as how fast a line is typed, is off
unless a pack asks for it.

## Asking for a game

Open an issue with [the form for a game](https://github.com/bquenin/ventris/issues/new/choose). Say which
game, which release and region, and which console; what is known about how it stores its text, if
anything; and whether a fan translation, a decompilation or a disassembly exists that could serve as an
answer key. A request is also the place to say you are starting on an adapter, so that two people do not
take the same game apart at once.
