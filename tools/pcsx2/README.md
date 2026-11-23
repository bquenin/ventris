# PCSX2 with automation commands

`pine-automation.patch` adds fourteen commands to PCSX2's control socket (`pcsx2/PINE.cpp`), and
`ventris_workbench.pcsx2` is the client:

- pause, advance N frames, screenshot to a path, set a controller input, read and write memory in
  bulk, and read the frame counter;
- watch a range of memory, which records every read or write of it with where the processor was
  and its registers, and lets the game run on; trace an address, which records the same each time
  it is run; collect what was recorded; and clear both;
- run as fast as the machine can, or at the game's own speed; and save and load a state by its
  file's name, where PCSX2's own commands only know numbered slots.

It also changes PCSX2's own two commands for numbered slots: they now reply once the state is
saved or loaded, where they replied at once and a script went on before the state was there.

`ventris-workbench ps2-probe` puts them together in one command, as `probe` does for Mesen: load a
state, press buttons at given frames, take screenshots, save memory, record which code reads or
writes a range and what the registers hold each time an address is run, save a state on the way.
It talks to an emulator that is already running the game.

`--poke ADDRESS:FILE[:OFFSET:LENGTH][@FRAME]` writes bytes of a file into the console's memory: at a
frame of the probe, or with no frame right after the state is loaded, before the game runs a frame.
The file can be a disc image, with the offset of what is wanted inside it. This is how a state made on
one build of a game is played on another: a state carries what the game had read from its disc, such
as a table of where its files are, and that is written over with the new build's own. What the game
made of its disc once and keeps elsewhere, such as a font sent to video memory, stays as the state has
it. A probe without `--state` goes on from wherever the emulator is; it is not from power-on.

The watch and the trace are how a game's code is found by what it does: which code reads this
string, which writes this position. PCSX2's own debugger has breakpoints and memory checks that
can log and carry on, in its fast mode too, but only writes them to its log. The patch keeps each
hit in a small recorder (`pcsx2/DebugTools/Breakpoints.cpp` and `.h`) for the socket to hand over.

What was measured on Tales of Destiny 2:

- **Speed.** Flat out, the opening film runs at about 215 frames a second and a field in the game
  at about 400, against 60 at the game's own speed. A build with link-time optimisation was 2 to
  4% faster, one with AVX2 as well no faster than that, and the renderer that draws nothing no
  faster at all: one thread runs the game's processor as code PCSX2 makes on the fly, and
  neither the compiler nor the drawing touches it. Each figure was timed by loading a state, then
  asking for a run of frames with the advance command and measuring how long it took, the first of
  several runs left out because it fills the emulator's caches.
- **A watch is not free.** While any watch is set, every read and write the game makes is
  checked, and a run takes about three times as long: 1,200 frames of the film took 19 seconds
  watched and 7 unwatched. So `ps2-probe --trace-from` sets the watches only from the frame where
  they are wanted. A trace of one address costs nothing that was measured.
- **A hit says in which frame it happened**, so it does not matter how often they are collected.
- **A watch does not report an access made by an instruction that is itself traced**: PCSX2 gives
  an instruction its breakpoint or its memory check, not both. Watch first, then trace what it
  finds.
- **A screenshot is written on a thread of the emulator's own**, a moment after it is asked for,
  and there is none to take in the frame a state is loaded in. `ps2-probe` runs two frames after
  a load before it starts counting, and waits for each picture to be whole.

The patch was written against PCSX2 commit `c8081bf` (November 2026).

The patch is a change to PCSX2's own source files, so it comes under PCSX2's licence, not under this
repository's MIT licence. PCSX2 is distributed under the GNU General Public License, version 3 or later
(`COPYING.GPLv3` in its repository); the files the patch changes say `GPL-3.0+`, or `GPL-2.0+` for
`Breakpoints.h`, in their headers.

Build PCSX2 from source with the recipe in its own CI workflows, after `git apply`ing the patch:

- **Windows**: download `pcsx2-windows-dependencies.7z` from the `PCSX2/pcsx2-windows-dependencies`
  releases into `deps/`, then configure with CMake and Ninja under `vcvars64.bat`
  (`.github/workflows/windows_build_qt.yml`, the CMake configuration).
- **macOS**: `.github/workflows/scripts/macos/build-dependencies.sh deps`, which compiles Qt and takes
  about an hour, then CMake with `-DCMAKE_OSX_ARCHITECTURES=x86_64`. The build is x86-64 and runs under
  Rosetta, because the emulator's recompilers are x86-only. Xcode's Metal toolchain is needed
  (`xcodebuild -downloadComponent MetalToolchain`).

To run without the first-run wizard: create `portable.txt` beside the executable, run it once with
`-testconfig` to write default settings, then in `inis/PCSX2.ini` set `SetupWizardIncomplete = false`,
`EnablePINE = true` and `BIOS = <file in bios/>`.

Keep the sound off: under `[SPU2/Output]` in `inis/PCSX2.ini`, `OutputMuted = true`, `StandardVolume = 0`
and `FastForwardVolume = 0`. The emulator reads them when it starts and writes the file itself, so
change it only while the emulator is closed. The automation patch does not touch sound; the speed
command switches between the two volumes, which is why both are nought.

On Windows the emulator has to start on the logged-in desktop. Over SSH that means a scheduled task
created with `/it`, started with `schtasks /run`. Its control port only listens locally, so forward it:
`ssh -N -L 28011:127.0.0.1:28011 machine`.
