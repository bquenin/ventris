# The fonts built into ventris

A translation needs letters the game's own font may not have. These fonts are compiled into the
`ventris` program, so that a pack need bring none. Each game's adapter names the ones it draws from,
and `ventris games` shows them.

A font is here because its terms let it be passed on inside another program, and because it has been
looked at in a game. They are not covered by ventris's own licence: each keeps its own terms, which are
in this folder and which `ventris licenses` prints.

| Name | File | What it is | Terms |
| --- | --- | --- | --- |
| `unifont` | `unifont_all-18.0.01.hex.gz` | [GNU Unifont](https://unifoundry.com/unifont/) 18.0.01: 16 pixels tall, a glyph for nearly every character of Unicode | SIL Open Font License 1.1, or the GNU GPL 2 or later with the font embedding exception: `unifont-LICENSE.txt` |
| `dejavu-condensed-bold` | `dejavu-condensed-bold.bdf.gz` | [DejaVu](https://dejavu-fonts.github.io/) Sans Condensed Bold 2.37, drawn as a bitmap for a cell of 12 by 16 pixels: Latin, Greek and Cyrillic | The Bitstream Vera Fonts licence, with the DejaVu changes in the public domain: `dejavu-LICENSE.txt` |
| `unscii-8` | `unscii-8.hex.gz` | [Unscii-8](http://viznut.fi/unscii/): 8 by 8 pixels, for text drawn in small tiles | Public domain: `unscii-8-LICENSE.txt` |

The files are compressed with gzip, and the two `.hex` files are otherwise as their authors publish them.

The DejaVu file is not the font as published, which is an outline font. It was drawn from
`DejaVuSansCondensed-Bold.ttf` with the tool in this repository, at its defaults, which are those of Tales
of Destiny 2's half-width font:

```bash
uv run --with pillow python tools/fonts/outline_to_bdf.py DejaVuSansCondensed-Bold.ttf -o dejavu-condensed-bold.bdf
```

Each letter is drawn eight times the size of the game's cell, and a build reduces it to the cell.

## Adding one

A font is added when a game needs it: check that its terms allow passing it on inside a program, put the
file and its terms here, list it in `BUILT_IN` in `src/fonts.rs` and in the table above, and look at it in
the game before an adapter names it.
