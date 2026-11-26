# Notes for coding agents

This repository is ventris, a game translation patch engine: it takes a game's script out as UTF-8 text
and puts a translation in any alphabet back in. `README.md` says what it is and how it is laid out.

## Which skill for which job

- **Translating a game that has an adapter, or improving a pack**:
  [`skills/ventris-translate/SKILL.md`](skills/ventris-translate/SKILL.md). Packs live in the
  [ventris-packs](https://github.com/bquenin/ventris-packs) repository, never in this one.
- **Adding a game, which is writing an adapter**:
  [`skills/ventris-adapter/SKILL.md`](skills/ventris-adapter/SKILL.md).
- **Anything else in the engine, the app or the workbench**: [`CONTRIBUTING.md`](CONTRIBUTING.md).

Each skill is a procedure with its guardrails, and points to the docs that hold the facts. Read the one
that fits before you start.

## Ground rules

The rules are in [`CONTRIBUTING.md`](CONTRIBUTING.md#ground-rules), and they hold for you as for anyone.
In short: no game files, ever (keep them under `roms/` and `work/`, which git ignores, and check
`git status` before you commit); translate the content, not the game; look at the screen; say what was
verified and what was not.

Two more, for an agent: commit, push, and open issues or pull requests only when your user asks, since
they are made under your user's account; and never download a game.

## Checks

A pull request passes these, from the root of the repository:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
uv run ruff check
uv run ruff format --check
uv run pytest
```

A change to the app, or to the engine's interface to it, also passes the app's checks in
[`CONTRIBUTING.md`](CONTRIBUTING.md#checks). Tests that need a game pass without checking anything when it
is missing; the [README](README.md#development) says how to point them at one.
