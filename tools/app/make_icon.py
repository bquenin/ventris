"""Draw the app's icon: the kana あ and the letter A, in the pixels of GNU Unifont, on a dark square.

    python tools/app/make_icon.py app/icon.png
    cd app && npx tauri icon icon.png      # writes every size under src-tauri/icons

The glyphs come from the Unifont built into the engine, so the icon is the same idea as the
program: a game's own writing, and the letters a translation puts beside it.
"""

import gzip
import pathlib
import struct
import sys
import zlib

SIZE = 1024
SCALE = 36  # pixels of the icon to one pixel of a glyph
BACKGROUND = (27, 26, 35)
KANA = (154, 152, 171)
LETTER = (242, 184, 75)
ROUNDING = 180

FONT = pathlib.Path(__file__).parents[2] / "crates/ventris/fonts/unifont_all-18.0.01.hex.gz"


def glyph(character: str) -> list[list[bool]]:
    """A character's 16 rows of pixels, 8 or 16 wide."""
    wanted = f"{ord(character):04X}:"
    with gzip.open(FONT, "rt", encoding="ascii") as lines:
        for line in lines:
            if line.startswith(wanted):
                digits = line.strip().split(":")[1]
                per_row = len(digits) // 16
                rows = [int(digits[row * per_row : (row + 1) * per_row], 16) for row in range(16)]
                bits = per_row * 4
                return [[bool(value >> (bits - 1 - x) & 1) for x in range(bits)] for value in rows]
    raise SystemExit(f"the font has no {character}")


def inside_rounded_square(x: int, y: int) -> bool:
    nearest_x = min(max(x, ROUNDING), SIZE - 1 - ROUNDING)
    nearest_y = min(max(y, ROUNDING), SIZE - 1 - ROUNDING)
    return (x - nearest_x) ** 2 + (y - nearest_y) ** 2 <= ROUNDING**2


def main(output: str) -> None:
    pixels = [
        [(*BACKGROUND, 255) if inside_rounded_square(x, y) else (0, 0, 0, 0) for x in range(SIZE)] for y in range(SIZE)
    ]
    parts = [(glyph("あ"), KANA), (glyph("A"), LETTER)]
    width = sum(len(rows[0]) for rows, _ in parts) + 1  # one empty column between the two
    left = (SIZE - width * SCALE) // 2
    top = (SIZE - 16 * SCALE) // 2
    for rows, colour in parts:
        for y, row in enumerate(rows):
            for x, lit in enumerate(row):
                if lit:
                    for dy in range(SCALE):
                        line = pixels[top + y * SCALE + dy]
                        start = left + x * SCALE
                        line[start : start + SCALE] = [(*colour, 255)] * SCALE
        left += (len(rows[0]) + 1) * SCALE

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    raw = b"".join(b"\0" + bytes(value for pixel in row for value in pixel) for row in pixels)
    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    pathlib.Path(output).write_bytes(png)


if __name__ == "__main__":
    main(sys.argv[1])
