"""Draw an outline font (TrueType, OpenType) as a large bitmap font, for a game whose cells are small.

A game's font cell is a dozen pixels across. A letter drawn straight at that size by a pixel font has
hard edges and thin strokes; the game's own letters were drawn large and reduced, which is what gives
them smooth edges. This tool does the first half: it draws each character several times the size of
the cell, in one bit per pixel, and writes the result as a BDF file. ``ventris insert`` does
the second half when it is given that file as ``--font``: a glyph that is a whole number of times the
size of the game's cell is reduced to it, and the share of each pixel that was ink becomes its level.

The engine itself has no dependencies; this tool needs Pillow, so run it with

    uv run --with pillow python tools/fonts/outline_to_bdf.py FONT.ttf -o work/fonts/NAME.bdf

Every character is drawn with its capitals ``--cap-height`` cells' rows tall and its baseline
``--baseline`` rows down the cell, centred, and squeezed sideways if its ink is wider than
``--ink-width`` pixels of the cell. The defaults are for Tales of Destiny 2's half-width font.
"""

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# Latin with its accented letters, Greek, Cyrillic, and the punctuation a translation is likely to use.
DEFAULT_RANGES = "21-7E,A1-17F,370-3FF,400-45F,2010-2027,2030-203A,20AC"


def parse_ranges(text: str) -> list[int]:
    codes: list[int] = []
    for part in text.split(","):
        first, _, last = part.partition("-")
        codes += range(int(first, 16), int(last or first, 16) + 1)
    return codes


def sized(path: Path, cap_height: int) -> ImageFont.FreeTypeFont:
    """The font at the size where a capital H is ``cap_height`` pixels tall."""
    size = cap_height
    while True:
        font = ImageFont.truetype(str(path), size)
        _, top, _, bottom = font.getbbox("H", anchor="ls")
        if bottom - top >= cap_height:
            return font
        size += 1


def draw(font: ImageFont.FreeTypeFont, char: str, width: int, height: int, baseline: int, ink_width: int):
    """A character in a cell of ``width`` by ``height`` pixels, or None if it has no ink there."""
    canvas = Image.new("L", (width * 6, height * 3), 0)
    ImageDraw.Draw(canvas).text((width * 2, height + baseline), char, font=font, fill=255, anchor="ls")
    box = canvas.getbbox()
    if box is None:
        return None
    left, _, right, _ = box
    strip = canvas.crop((left, height, right, height * 2))  # the rows of the cell, the ink's columns
    if strip.width > ink_width:
        strip = strip.resize((ink_width, height), Image.LANCZOS)
    cell = Image.new("L", (width, height), 0)
    cell.paste(strip, ((width - strip.width) // 2, 0))
    return cell.point(lambda value: 255 if value >= 128 else 0)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("font", type=Path)
    parser.add_argument("-o", "--output", type=Path, required=True)
    parser.add_argument("--cell", default="12x16", help="the game's cell, WIDTHxHEIGHT")
    parser.add_argument("--scale", type=int, default=8, help="how many times the cell's size to draw at")
    parser.add_argument("--cap-height", type=float, default=12, help="rows of the cell a capital takes")
    parser.add_argument("--baseline", type=int, default=13, help="rows of the cell above the baseline")
    parser.add_argument("--ink-width", type=float, default=10, help="the widest a character's ink may be, in the cell")
    parser.add_argument("--ranges", default=DEFAULT_RANGES, help="characters to draw, as hexadecimal ranges")
    arguments = parser.parse_args()

    cell_width, cell_height = (int(number) for number in arguments.cell.split("x"))
    scale = arguments.scale
    width, height, baseline = cell_width * scale, cell_height * scale, arguments.baseline * scale
    font = sized(arguments.font, round(arguments.cap_height * scale))
    missing = draw(font, "\U0010fffe", width, height, baseline, width)  # what the font draws for a character it lacks
    blank = missing.tobytes() if missing else b""

    glyphs = []
    for code in parse_ranges(arguments.ranges):
        cell = draw(font, chr(code), width, height, baseline, round(arguments.ink_width * scale))
        if cell is not None and cell.getbbox() is not None and cell.tobytes() != blank:
            glyphs.append((code, cell))

    descent = height - baseline
    name = f"{font.getname()[0]} {font.getname()[1]}".replace('"', "")
    lines = [
        "STARTFONT 2.1",
        f"FONT -ventris-{name.replace(' ', '')}-{height}",
        f"SIZE {height} 72 72",
        f"FONTBOUNDINGBOX {width} {height} 0 {-descent}",
        "STARTPROPERTIES 3",
        f'FAMILY_NAME "{name}"',
        f"FONT_ASCENT {baseline}",
        f"FONT_DESCENT {descent}",
        "ENDPROPERTIES",
        f"CHARS {len(glyphs)}",
    ]
    row_bytes = -(-width // 8)
    for code, cell in glyphs:
        lines += [f"STARTCHAR U+{code:04X}", f"ENCODING {code}", "SWIDTH 1000 0", f"DWIDTH {width} 0"]
        lines += [f"BBX {width} {height} 0 {-descent}", "BITMAP"]
        bits = cell.convert("1").tobytes()  # rows of whole bytes, the leftmost pixel in the top bit
        lines += [bits[row * row_bytes : (row + 1) * row_bytes].hex().upper() for row in range(height)]
        lines.append("ENDCHAR")
    lines.append("ENDFONT")
    arguments.output.write_text("\n".join(lines) + "\n", encoding="ascii")
    print(
        f"wrote {arguments.output}: {len(glyphs)} characters of {name}, each {width}x{height} for a {cell_width}x{cell_height} cell"
    )


if __name__ == "__main__":
    main()
