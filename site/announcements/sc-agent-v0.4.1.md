# sc-agent v0.4.1 — configured models, and `jev` now means `jev-latest`

**Status:** prepared on `main` (`a01ccbea`) via PR #432 and #433 — **publication pending owner go**. No `v0.4.1` tag, crates.io/PyPI wheel, Homebrew, Scoop or winget artifact exists yet; v0.4.0 remains the latest published release.

**Released:** pending

**Install (once published):** `pip install sc-decidr==0.4.1`, `cargo install decidr`, `brew install randlee/tap/decidr`

[README](https://github.com/randlee/sc-agent/blob/main/README.md) · [CHANGELOG](https://github.com/randlee/sc-agent/blob/main/CHANGELOG.md) · [Release v0.4.0](https://github.com/randlee/sc-agent/releases/tag/v0.4.0)

v0.4.1 is a small release about one question: what does a model name mean? Until now the built-in table was the only answer. Now a repository can configure one.

## The change

- **A `models` configuration setting** maps a model name to a built-in `provider` and its wire `id`. A configured name wins over the built-in table.
- **A configured name is not a new model.** It is an alias for a built-in one, which keeps the provider and the wire id together instead of letting a caller pair a model with the wrong provider.
- **The built-in `jev` now resolves to `jev-latest`.** `jev` is still the name the client accepts; the client sends the row's wire id (`jev-latest` since 0.4.1), never the literal string `jev`.

## Why it matters

The old behaviour answered "what is jev?" with a hard-coded string. A repository that needs to pin, or that wants to call the same model by its own name, had no way to say so without forking the table. Configuration now carries that decision, and the default moves with the service rather than with the crate.

## Compatibility

`decidr send --model jev`, `sc_agent.send`, `decidr doctor` and the Python surface are unchanged. The only behaviour a caller can observe without configuring anything is that the wire id sent for `jev` is `jev-latest` instead of `jev`.

## Status detail

Verified at `main` `a01ccbea`, workspace version `0.4.1`, CHANGELOG entry present. Publication is a separate, owner-gated step: this page will be the announcement of record once the tag exists, and it is not rewritten retroactively.
