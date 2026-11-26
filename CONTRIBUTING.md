# Contributing

This project is meant to be built by the people who use it: those who take games apart, and those who
translate them. There are three ways to help, each with a page that goes from nothing to done:

- **Translate**: fix a line you saw while playing, review a machine translated pack, or start a pack for a
  new language. No programming needed, and no terminal: the app saves the game's script and builds a pack.
  See [Translating a game](docs/translating.md). Packs live in the
  [ventris-packs](https://github.com/bquenin/ventris-packs) repository, not here.
- **Add a game**: write an adapter, the code that lets ventris read and rebuild one game. This is work
  for the command line, which [Commands](docs/commands.md) describes. See
  [Adding a game](docs/adding-a-game.md). You can also [ask for a game](docs/adding-a-game.md#asking-for-a-game).
- **Improve ventris itself**: report a bug, or fix one, in the engine, the app or the workbench. Use the
  [issue forms](https://github.com/bquenin/ventris/issues/new/choose).

To find others or ask a question, there is [Discussions](https://github.com/bquenin/ventris/discussions): the
[Translations](https://github.com/bquenin/ventris/discussions/categories/translations) category for packs, the
[Adapters](https://github.com/bquenin/ventris/discussions/categories/adapters) category for taking a game apart.
A pack you are starting is claimed in the packs repository instead, with
[the form for a new pack](https://github.com/bquenin/ventris-packs/issues/new/choose), so that two people do
not begin the same one.

If you work with a coding agent, [AGENTS.md](AGENTS.md) and the skills in [`skills/`](skills/) hold it to
these rules; you stay responsible for what it sends.

## Ground rules

- **No game files, ever.** No image, no extracted script, no font sheet, no saved state, no patch made from
  a game, in a commit or attached to an issue. Keep them under `roms/` and `work/`, which git ignores, or
  anywhere outside the repository. Check `git status` before you commit. What was learned about a game
  goes in (a character table, the addresses of its routines); the game does not.
- **Translate the content, not the game.** ventris puts a translation of a game's text into it. It does
  not rework a name-entry screen, lay boxes out again so that longer text fits, redraw pictures with text
  in them, or change how the game behaves. See [Scope](README.md#scope).
- **Look at the screen.** A change to what a game shows is not done until it has been seen in a
  screenshot from an emulator. Saved states carry video memory, so test a change to a font from power-on.
- **Say what was verified and what was not.** If something fails, or was skipped, say so with what you
  saw. "Built, not seen on screen" is a fine thing to write.
- **Comments in plain words.** Comments and docstrings say what the code does and why, for a reader who has
  not seen the game.
- **An existing fan translation is an answer key**, used for facts: where things are, what changed. Check
  its licence before taking more than facts.

## Checks

A pull request passes these. The engine and the workbench:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
uv run ruff check
uv run ruff format --check
uv run pytest
```

The app, when the change touches it or the engine's interface to it:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm test && npm run build
```

Rust is formatted at width 120, Python at line length 120. Tests that need a game look for it by what it
is in the folder `VENTRIS_ROMS` names, and pass without checking anything when it is not there; the
[README](README.md#development) says how to set it.

## Adapters

Adapters are built into ventris, not loaded from elsewhere, so a new game is a pull request to this
repository, and the maintainer reviews every one. A pull request for an adapter shows:

- the module, with a module comment that describes what was found for a reader who has not seen the game,
  its tests, and its line in `ALL`;
- the output of `ventris check` on the game;
- screenshots from an emulator of changed text, in each kind of text that can be inserted, from power-on
  when the font changed;
- what was verified and what was not, and which release of the game it is for.

An adapter that only extracts is a whole contribution. A string's address, `section.number`, never changes
once an adapter is released; packs depend on it. [Adding a game](docs/adding-a-game.md) has the detail.

The engine was ported from Python and was held to building the same images byte for byte. A change that
alters what a valid pack builds is made on purpose, and the pull request says so.

## Licence

The engine, the app and the workbench are under the [MIT licence](LICENSE). By sending a contribution here,
you agree that it is passed on under the same terms.
