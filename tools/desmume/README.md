# DeSmuME as a library, with hooks a probe can use

`ventris-workbench ds-probe` runs a Nintendo DS game in DeSmuME, headless, as `probe` does for Mesen:
start from power-on or a saved state, press buttons and touch the bottom screen at given frames, take
screenshots of both screens, save memory and the registers, record which code reads or writes a range
and which instructions run, save a state on the way. `ventris_workbench.dsprobe` is the driver.

It does not run DeSmuME's program. It loads DeSmuME's interface frontend
(`desmume/src/frontend/interface`), a library with a C interface and no window, the one SkyTemple's
py-desmume uses, and calls it through Python's `ctypes`. DeSmuME's Lua scripting, in its Windows
program, cannot be started from the command line, and its read hooks do not say which processor read.

`interface-hooks.patch` adds to that library what a probe needs. All of it is in the interface
frontend's code or behind its `TARGET_INTERFACE` switch, so DeSmuME's other programs build as before.

- **Who made an access.** Inside a memory hook, the callback can ask which processor made the access
  (`desmume_memory_hook_cpu`: 0 the ARM9, 1 the ARM7), the address of the instruction it was running
  (`desmume_memory_hook_pc`), the kind of access (`desmume_memory_hook_access`: 0 an instruction
  fetch, 1 data, 3 a DMA, for which the instruction has nothing to do with it), and the value written,
  or the instruction run for an exec hook (`desmume_memory_hook_value`). Without them a hook is told
  an address and a size, and nothing else.
- **Reading memory without setting a watch off.** `desmume_memory_read_block` copies memory as either
  processor sees it, as a debugger reads it. The emulator's own debugging reads no longer call the
  hooks at all; before, the sound chip's reads of its samples did.
- **Where the battery save goes.** `desmume_set_battery_dir`, called before a game is opened. By
  default the game's `.dsv` file is written beside the program that loaded the library, which for the
  workbench is `python.exe`. `ds-probe` puts it in a folder of the run's own, emptied first, so that a
  run never starts from another's save.
- **Three fixes to the hooks as they were.** A 16-bit write to main memory never called the write
  hook; a byte written to the ARM9's data memory called it as a read; and finding a byte's hook
  added an empty entry to the table each time it looked, so the table grew with every access.

What was measured on Ganbare Goemon (Japan), with the release build below on a desktop Windows machine:

- **Speed.** Unwatched, the game runs at 145 to 190 frames a second, against 60 at its own speed:
  3,000 frames from power-on took 16 seconds, 600 frames of the opening story 4.1 seconds. The library
  is started afresh for each run, which adds about half a second.
- **The same run gives the same result.** Two runs from power-on with the same input left main
  memory and the screenshot identical, byte for byte, 1,500 frames in; so did two runs from a saved
  state. A frame number in one run is the same moment in the next, so a run can be repeated with a
  watch added and still see what the first one saw.
- **A watch costs what Python does for each access it sees.** The hooks call into Python. A read watch
  over 1.75 MiB of main memory, where the game keeps its heap, made 600 frames take 5.2 seconds
  instead of 4.1; set from frame 540 with `--trace-from`, 4.5. An exec watch over the whole of main
  memory calls Python for every instruction the ARM9 runs there, and 600 frames took 127 seconds: watch
  the range of a routine, not of a program. Setting a watch also costs time of its own: the library
  keeps one entry per byte watched.
- **The hooks are called by the interpreter only.** DeSmuME also has a recompiler (JIT), which skips
  them. It is off unless a frontend turns it on, and the interface library never does.
- **Screenshots** are the two screens, 256 by 192 pixels each, the top one above the bottom one, as the
  console last drew them.

The patch was written against DeSmuME commit `b549f71b83287f265b6d74b9e77ef5d98a617190` (November
2026), from [TASEmulators/desmume](https://github.com/TASEmulators/desmume). DeSmuME keeps some of its
sources with Windows line endings, `src/MMU.h` among them, and the patch carries them as they are
(`.gitattributes` in this folder keeps git from changing them), so apply it with `git apply` from the
root of DeSmuME's repository.

The patch is a change to DeSmuME's own source files, so it comes under DeSmuME's licence, not under
this repository's MIT licence. DeSmuME is distributed under the GNU General Public License, version 2
(`desmume/COPYING` and `license.txt` in its repository), and the four files the patch changes say in
their headers that they may be used under version 2 or, at your option, any later version.

Build on Windows with Visual Studio 2026 and its C++ tools (the Community edition was used), from a
developer command prompt for x64 (`vcvars64.bat`):

```bat
cd desmume\src\frontend\interface\windows
msbuild DeSmuME_Interface.sln /m /v:m /p:Configuration=Release /p:Platform=x64
copy SDL\lib\x64\SDL2.dll __bins\
```

The library is `__bins\DeSmuME Interface-VS2026-x64-Release.dll` (the name follows the toolset), and
the library needs `SDL2.dll` beside it, which the last line puts there. Set `DESMUME` to the library's
path.

DeSmuME's Windows program, built from `desmume\src\frontend\windows\DeSmuME.sln` in its `Dev+`
configuration, has a disassembler, memory, tile and map viewers, and GDB stubs for both processors
(`--arm9gdb PORT`, `--arm7gdb PORT`), which help in looking around by hand. The workbench does not
need it. It is built with the same `msbuild` line and `/p:Configuration=Dev+`; that configuration does
not unpack the DirectX libraries the others unpack first, so run `un7z_and_touch.bat directx\directx.7z
.libs\directx.tag` in that folder before the first build.

On macOS and Linux the interface frontend has a Meson build of its own
(`desmume/src/frontend/interface/meson.build`), which makes `libdesmume`. Neither the patch nor the
workbench has been tried there.
