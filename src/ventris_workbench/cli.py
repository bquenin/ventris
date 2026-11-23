"""The workbench's command line: tools for taking a game apart. The engine itself is in Rust."""

import argparse
import hashlib
import sys
import zlib
from pathlib import Path

from ventris_workbench import __version__, emulator, pcsx2, ps2probe
from ventris_workbench.bindiff import diff_ranges, merge_ranges
from ventris_workbench.patch import PatchError, apply_patch, create_bps, detect_format
from ventris_workbench.platforms import snes, w65816


class UnknownGameError(Exception):
    """The file is not a game the command can work on."""


def _load_snes(path: Path) -> tuple[bytes, bool, snes.SnesHeader | None]:
    """Read a file and return (image without copier header, had copier header, SNES header)."""
    data = path.read_bytes()
    headered = snes.has_copier_header(len(data))
    rom = snes.strip_copier_header(data)
    return rom, headered, snes.detect_header(rom)


def _cmd_identify(args: argparse.Namespace) -> int:
    data = args.rom.read_bytes()
    rom, headered, header = _load_snes(args.rom)
    print(f"file:           {args.rom}")
    print(f"size:           {len(data)} bytes ({len(data) / 0x20000:g} Mbit)")
    print(f"crc32:          {zlib.crc32(data):08x}")
    print(f"sha1:           {hashlib.sha1(data).hexdigest()}")
    if header is None:
        print("platform:       unknown (no SNES header found)")
        return 0
    print("platform:       SNES")
    if headered:
        print(f"copier header:  yes, {snes.COPIER_HEADER_SIZE} bytes (hashes without it below)")
        print(f"rom crc32:      {zlib.crc32(rom):08x}")
        print(f"rom sha1:       {hashlib.sha1(rom).hexdigest()}")
    else:
        print("copier header:  no")
    computed = snes.compute_checksum(rom)
    print(f"title:          {header.title}")
    print(f"mapping:        {header.mapping}{' (FastROM)' if header.fast_rom else ''}")
    print(f"header offset:  0x{header.offset:06x}")
    print(f"region:         {header.region}")
    print(f"version:        1.{header.version}")
    print(f"cartridge type: 0x{header.cartridge_type:02x}")
    print(f"declared rom:   {header.rom_size_kb} KiB")
    print(f"declared ram:   {header.ram_size_kb} KiB")
    print(f"reset vector:   ${header.reset_vector:04x}")
    status = "ok" if computed == header.checksum else f"mismatch, computed 0x{computed:04x}"
    print(f"checksum:       0x{header.checksum:04x} ({status})")
    return 0


def _cmd_apply(args: argparse.Namespace) -> int:
    source = args.rom.read_bytes()
    patch = args.patch.read_bytes()
    fmt = detect_format(patch)
    add_header = args.copier_header and not snes.has_copier_header(len(source))
    if add_header:
        source = bytes(snes.COPIER_HEADER_SIZE) + source
    result = apply_patch(source, patch)
    if add_header:
        result = result[snes.COPIER_HEADER_SIZE :]
        source = source[snes.COPIER_HEADER_SIZE :]
    args.output.write_bytes(result)
    print(f"applied {fmt.upper()} patch: {len(source)} -> {len(result)} bytes, crc32 {zlib.crc32(result):08x}")
    print(f"wrote {args.output}")
    return 0


def _cmd_make_patch(args: argparse.Namespace) -> int:
    source = args.original.read_bytes()
    target = args.modified.read_bytes()
    patch = create_bps(source, target)
    args.output.write_bytes(patch)
    print(f"wrote {args.output} ({len(patch)} bytes)")
    return 0


def _cmd_diff(args: argparse.Namespace) -> int:
    a, a_headered, a_header = _load_snes(args.a)
    b, b_headered, _ = _load_snes(args.b)
    mapping = a_header.mapping if a_header else None
    for path, headered in ((args.a, a_headered), (args.b, b_headered)):
        if headered:
            print(f"note: ignoring the {snes.COPIER_HEADER_SIZE}-byte copier header of {path}")

    regions = merge_ranges(diff_ranges(a, b), gap=args.gap)
    changed = sum(end - start for start, end in regions)
    common = min(len(a), len(b))
    print(f"a: {len(a)} bytes   b: {len(b)} bytes")
    if len(a) != len(b):
        print(f"size differs by {abs(len(a) - len(b))} bytes; only the first {common} are compared")
    share = changed / common if common else 0
    print(f"{len(regions)} changed regions, {changed} bytes ({share:.1%}), merging gaps up to {args.gap} bytes")
    if not regions:
        return 0

    shown = sorted(regions, key=lambda r: r[1] - r[0], reverse=True) if args.sort == "size" else regions
    if args.limit:
        shown = shown[: args.limit]
    print()
    print(f"{'offset':>10}  {'end':>10}  {'length':>8}  {'cpu address':<11}")
    for start, end in shown:
        address = ""
        if mapping:
            try:
                address = f"${snes.pc_to_snes(start, mapping):06x}"
            except snes.AddressError:
                address = "unmapped"
        print(f"0x{start:08x}  0x{end:08x}  {end - start:>8}  {address:<11}")
    if len(shown) < len(regions):
        print(f"... {len(regions) - len(shown)} more regions (raise --limit to see them)")
    return 0


def _cmd_disasm(args: argparse.Namespace) -> int:
    rom, _, header = _load_snes(args.rom)
    if header is None:
        raise UnknownGameError(f"{args.rom} does not look like a SNES game")
    address = int(args.address.removeprefix("$").removeprefix("0x"), 16)
    offset = snes.snes_to_pc(address, header.mapping)
    listing = w65816.disassemble(rom, offset, address, args.count, m8=not args.m16, x8=not args.x16)
    for instruction in listing:
        print(f"{instruction.address:06X}  {instruction.data.hex(' '):<12} {instruction}")
    return 0


def _cmd_ps2_probe(args: argparse.Namespace) -> int:
    plan = ps2probe.Plan(
        frames=args.frames,
        input=args.input,
        shots=args.shot,
        dumps=args.dump,
        reads=args.reads,
        writes=args.writes,
        log=args.log,
        trace_from=args.trace_from,
        state=args.state,
        save=args.save or "",
        pokes=args.poke,
        fast=not args.real_time,
    )
    with pcsx2.Pcsx2(args.emulator) as running:
        files = ps2probe.run(running, args.output, plan)
    for path in files:
        print(path)
    return 0


def _cmd_probe(args: argparse.Namespace) -> int:
    probe = emulator.Probe(
        frames=args.frames,
        input=" ".join(args.input),
        trigger=args.trigger or "",
        input_after=" ".join(args.after),
        shots=args.shot,
        dumps=args.dump,
        registers=args.registers,
        reads=args.reads,
        writes=args.writes,
        exec_range=args.exec or "",
        log=args.log,
        trace_from=args.trace_from,
        state=args.state,
        save=args.save or "",
    )
    files = emulator.run_probe(args.rom, args.output, probe, timeout=args.timeout)
    trace = Path(f"{args.output.resolve()}-exec.txt")
    if trace.exists():
        rom, _, header = _load_snes(args.rom)
        if header is None:
            raise UnknownGameError(f"{args.rom} does not look like a SNES game")
        text = emulator.listing(rom, header.mapping, emulator.read_executed(trace))
        listing = trace.with_suffix(".asm")
        listing.write_text(text)
        files.append(listing)
    for file in sorted(files):
        print(file)
    return 0


def _cmd_addr(args: argparse.Namespace) -> int:
    value = int(args.value.removeprefix("$").removeprefix("0x"), 16)
    if args.to == "pc":
        print(f"0x{snes.snes_to_pc(value, args.mapping):06x}")
    else:
        print(f"${snes.pc_to_snes(value, args.mapping):06x}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ventris-workbench",
        description="Tools for taking a game apart. The engine that extracts and inserts scripts is the Rust program.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    identify = sub.add_parser("identify", help="hash a game file and show what it is")
    identify.add_argument("rom", type=Path)
    identify.set_defaults(func=_cmd_identify)

    apply = sub.add_parser("apply", help="apply an IPS or BPS patch")
    apply.add_argument("rom", type=Path)
    apply.add_argument("patch", type=Path)
    apply.add_argument("-o", "--output", type=Path, required=True)
    apply.add_argument(
        "--copier-header",
        action="store_true",
        help="the patch was made for a SNES ROM with a 512-byte copier header; "
        "patch as if the ROM had one, and write the result without it",
    )
    apply.set_defaults(func=_cmd_apply)

    make_patch = sub.add_parser("make-patch", help="create a BPS patch from an original and a modified file")
    make_patch.add_argument("original", type=Path)
    make_patch.add_argument("modified", type=Path)
    make_patch.add_argument("-o", "--output", type=Path, required=True)
    make_patch.set_defaults(func=_cmd_make_patch)

    diff = sub.add_parser("diff", help="list the regions that differ between two game files")
    diff.add_argument("a", type=Path)
    diff.add_argument("b", type=Path)
    diff.add_argument("--gap", type=int, default=16, help="merge regions separated by up to this many bytes")
    diff.add_argument("--limit", type=int, default=40, help="maximum regions to list (0 for all)")
    diff.add_argument("--sort", choices=("offset", "size"), default="offset")
    diff.set_defaults(func=_cmd_diff)

    disasm = sub.add_parser("disasm", help="disassemble SNES code at a CPU address")
    disasm.add_argument("rom", type=Path)
    disasm.add_argument("address", help="hexadecimal CPU address, such as C8E550")
    disasm.add_argument("--count", type=int, default=32, help="number of instructions")
    disasm.add_argument("--m16", action="store_true", help="start with a 16-bit accumulator")
    disasm.add_argument("--x16", action="store_true", help="start with 16-bit index registers")
    disasm.set_defaults(func=_cmd_disasm)

    probe = sub.add_parser(
        "probe",
        help="run a game in the Mesen emulator with scripted input and record what it does",
        description="Frame numbers count from the trigger when one is given, otherwise from the start. "
        "Addresses are hexadecimal. Needs the MESEN environment variable to name the Mesen binary.",
    )
    probe.add_argument("rom", type=Path)
    probe.add_argument("-o", "--output", type=Path, required=True, help="path prefix for everything written")
    probe.add_argument("--frames", type=int, default=600, help="frame at which to stop")
    probe.add_argument(
        "--input",
        nargs="*",
        default=[],
        metavar="STEP",
        help="input steps FRAME:KIND:BUTTONS, where KIND is mash, press, hold or none; such as 0:mash:start,a",
    )
    probe.add_argument("--trigger", metavar="ADDRESS@FRAME", help="restart the clock when this address executes")
    probe.add_argument("--after", nargs="*", default=[], metavar="STEP", help="input steps once the trigger fires")
    probe.add_argument("--shot", nargs="*", type=int, default=[], metavar="FRAME", help="take screenshots")
    probe.add_argument(
        "--dump", nargs="*", default=[], metavar="MEMORY@FRAME", help="save memory, e.g. snesWorkRam@300"
    )
    probe.add_argument("--registers", nargs="*", type=int, default=[], metavar="FRAME", help="save the registers")
    probe.add_argument(
        "--reads", nargs="*", default=[], metavar="MEMORY:START-END", help="record which code reads these ranges"
    )
    probe.add_argument(
        "--writes", nargs="*", default=[], metavar="MEMORY:START-END", help="record which code writes these ranges"
    )
    probe.add_argument("--exec", metavar="START-END", help="record and disassemble the instructions run in this range")
    probe.add_argument(
        "--log",
        nargs="*",
        default=[],
        metavar="ADDRESS[:MEMORY:START-END]",
        help="add a line each time this address executes: registers, top of the stack, and that memory",
    )
    probe.add_argument("--trace-from", type=int, default=0, metavar="FRAME", help="start recording at this frame")
    probe.add_argument("--state", type=Path, help="saved state to start from")
    probe.add_argument("--save", metavar="FILE@FRAME", help="save a state at this frame")
    probe.add_argument("--timeout", type=int, default=300, help="seconds before giving up")
    probe.set_defaults(func=_cmd_probe)

    ps2 = sub.add_parser(
        "ps2-probe",
        help="run a game in the PCSX2 emulator with scripted input and record what it does",
        description="Talks to a patched PCSX2 that is already running the game (tools/pcsx2). Frames count from "
        "where the probe starts, after the state is loaded when one is given. Addresses are hexadecimal.",
    )
    ps2.add_argument("-o", "--output", type=Path, required=True, help="path prefix for everything written")
    ps2.add_argument("--frames", type=int, default=600, help="frame at which to stop")
    ps2.add_argument(
        "--input",
        nargs="*",
        default=[],
        metavar="STEP",
        help="input steps FRAME:KIND:BUTTONS, where KIND is mash, press, hold or none; such as 0:mash:cross",
    )
    ps2.add_argument("--shot", nargs="*", type=int, default=[], metavar="FRAME", help="take screenshots")
    ps2.add_argument(
        "--dump", nargs="*", default=[], metavar="ADDRESS:LENGTH@FRAME", help="save memory, e.g. 1F2000:400@300"
    )
    ps2.add_argument("--reads", nargs="*", default=[], metavar="START-END", help="record which code reads this range")
    ps2.add_argument("--writes", nargs="*", default=[], metavar="START-END", help="record which code writes it")
    ps2.add_argument(
        "--log", nargs="*", default=[], metavar="ADDRESS", help="record the registers each time this address is run"
    )
    ps2.add_argument("--trace-from", type=int, default=0, metavar="FRAME", help="start recording at this frame")
    ps2.add_argument("--state", type=Path, help="saved state to start from")
    ps2.add_argument("--save", metavar="FILE@FRAME", help="save a state at this frame")
    ps2.add_argument(
        "--poke",
        nargs="*",
        default=[],
        metavar="ADDRESS:FILE[:OFFSET:LENGTH][@FRAME]",
        help="write bytes of a file into memory: at this frame, or with no frame right after the state is loaded, "
        "before the game runs; e.g. 1DC320:SLPS_251.72:DD320:8FD0",
    )
    ps2.add_argument("--real-time", action="store_true", help="run at the game's own speed, not as fast as possible")
    ps2.add_argument("--emulator", metavar="HOST:PORT", help="where PCSX2 listens, when not the usual place")
    ps2.set_defaults(func=_cmd_ps2_probe)

    addr = sub.add_parser("addr", help="convert between SNES CPU addresses and file offsets")
    addr.add_argument("value", help="hexadecimal address or offset")
    addr.add_argument("--mapping", choices=("LoROM", "HiROM", "ExHiROM"), required=True)
    addr.add_argument("--to", choices=("pc", "snes"), required=True)
    addr.set_defaults(func=_cmd_addr)

    return parser


def main(argv: list[str] | None = None) -> int:
    # A report may name characters the terminal's encoding lacks; show those as escapes rather than fail.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(errors="backslashreplace")
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except (
        PatchError,
        snes.AddressError,
        w65816.DisassemblyError,
        UnknownGameError,
        emulator.EmulatorError,
        pcsx2.Pcsx2Error,
        ps2probe.ProbeError,
        OSError,
    ) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
