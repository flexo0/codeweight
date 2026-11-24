# codeweight

[![crates.io](https://img.shields.io/crates/v/codeweight.svg)](https://crates.io/crates/codeweight)
[![CI](https://github.com/flexo0/codeweight/actions/workflows/ci.yml/badge.svg)](https://github.com/flexo0/codeweight/actions/workflows/ci.yml)

A CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a function, ...) has a
weight, and an element nested in branches, loops and lambdas weighs more. A file's or a function's weight is the sum
of its elements' weights; a commit's is its net delta, after minus before.

Use it to see where the heavy code is, how much a commit or your uncommitted changes add or take away, or to let a
coding agent check its own changes.

It reads Java, TypeScript (TSX too), JavaScript, Python, Rust and Go, parsed by tree-sitter. Markdown files get a docs
weight of their own.

## Example

codeweight run on its own sources, and on what a commit added or took away: its code, its docs and its tests, three
numbers, never one sum:

```
$ codeweight scan src
code +9649.96
docs 0
tests +3036.49
$ codeweight show f4de918 --short
c+112 d+67 t+65
$ codeweight show f4de918 --code -v --top 2
total +112.46
+30.06  added    src/cli.rs  write_all
+24.18  heavier  src/cli.rs  scan
```

## Install

A binary for Linux (x86_64, static), macOS (x86_64 and Apple silicon) or Windows (x86_64) from
[the releases](https://github.com/flexo0/codeweight/releases/latest): unpack the archive for your platform and put
`codeweight` (`codeweight.exe`) on your `PATH`.

Or from source, with Rust 1.100 or later and a C compiler (the tree-sitter grammars are C, built from source with the
crate):

```
cargo install codeweight
```

or the latest from GitHub, `cargo install --git https://github.com/flexo0/codeweight`.

For a coding agent that reads `SKILL.md` skills (Claude Code and others), link or copy `skills/codeweight/` into the
agent's skills, e.g. from a clone `ln -s "$PWD/skills/codeweight" ~/.claude/skills/codeweight`.

## Quick start

```
codeweight scan [<path>...]     weigh files and folders (default `.`)
codeweight show [<rev>]         weigh the net delta of a commit (default `HEAD`), or of a range `A..B`
codeweight diff                 weigh the net delta of the uncommitted changes (`--staged`: the staged ones alone)
codeweight init                 write the default weights file to ./codeweight.toml, to tune
```

Each prints three numbers, code, docs and tests (`--short`: on one line). `--code`, `--tests` or `--docs` prints that
number alone, and with `-v` the functions behind it, heaviest first; `--json` the same as JSON.

## More

- [docs/usage.md](https://github.com/flexo0/codeweight/blob/master/docs/usage.md): every command and option, the text
  report, which files are weighed, how `show` and `diff` compare
- [docs/json.md](https://github.com/flexo0/codeweight/blob/master/docs/json.md): the JSON report's fields
- [docs/weights-file.md](https://github.com/flexo0/codeweight/blob/master/docs/weights-file.md): the weights file, the
  weight formula, and what each category is in each language
- [docs/README.md](https://github.com/flexo0/codeweight/blob/master/docs/README.md): the design, for contributors
- [CHANGELOG.md](https://github.com/flexo0/codeweight/blob/master/CHANGELOG.md): what each version changed, and what
  a version's bump promises

## Building

Rust 1.100 (pinned in `rust-toolchain.toml`, so rustup picks it) and a C compiler for the grammars:
`cargo build --release`; checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## License

MIT, see [LICENSE](https://github.com/flexo0/codeweight/blob/master/LICENSE).
