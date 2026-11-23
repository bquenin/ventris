## What this changes

<!-- In a sentence or two. Link the issue, if there is one. -->

## What was verified

<!-- What you checked and how: the commands you ran, the scenes you looked at on screen. Say also what
you did not check. "Built, not seen on screen" is a fine answer. -->

## Checklist

- [ ] No game file is in this pull request: no image, extracted script, font sheet, saved state, or patch made from a game.
- [ ] The change stays within the scope: it is about a game's text, not about changing the game beyond it.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] If the workbench changed: `uv run ruff check`, `uv run ruff format --check` and `uv run pytest` pass.
- [ ] If the app changed: `cd app && npm test && npm run build` passes.
- [ ] If what a game shows changed: a screenshot from an emulator is attached, from power-on if the font changed.
- [ ] If this is an adapter: the output of `ventris check` is included, and no string's address changed.
