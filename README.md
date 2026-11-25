# codeweight

[![crates.io](https://img.shields.io/crates/v/codeweight.svg)](https://crates.io/crates/codeweight)
[![CI](https://github.com/flexo0/codeweight/actions/workflows/ci.yml/badge.svg)](https://github.com/flexo0/codeweight/actions/workflows/ci.yml)

A CLI that weighs code. Our ambition is to weigh the code's cognitive volume. 
Every element of a file (a statement, a call, a branch, a loop, a function, ...) has a weight; 
an element nested in branches, loops and lambdas weighs more,
a long argument list weighs more, a long dereference chain weighs more, etc. A file's or a function's weight is the
sum of its elements' weights; a commit's is its net delta.

Designed for autonomous agentic pipelines, for automatic decision-making.
Use it to see how much module/feature/service/project/you-name-it weights; what delta a commit or uncommitted changes yield;
to validate amount of docs matches amount of code; to check that refactor reduced weight; etc.

It reads Java, TypeScript (TSX too), JavaScript, Python, Rust, Go and C, each parsed by tree-sitter. Markdown files,
not to be left out, receive a docs weight of their own.

The default coefficients are calibrated on 1,400+ pairs, each a bad snippet beside its simpler good
counterpart with the same behaviour, from linters' rule examples, fixers' tests and real refactoring commits
([code-snippet-pairs](https://github.com/flexo0/code-snippet-pairs)). The docs-weight coefficients are calibrated
on snippets of code beside their descriptions, each description presumed to carry the very same cognitive volume as
its code, so that a doc weighs precisely what the code it describes does.

## Example

codeweight run on its own repo, and on what a commit added or took away, as four numbers: code, comments, docs
and tests:

```
$ codeweight scan .
code +11765.16
comments +6531.52
docs +17383.9
tests +5198.5
$ codeweight show f4de918 --short
c+112 m+43 d+33 t+65
$ codeweight show f4de918 --code --top 2
total +112.46
+30.06  added    src/cli.rs  write_all
+24.18  heavier  src/cli.rs  scan
```

## Install

A binary for Linux (x86_64, static), macOS (x86_64 and Apple silicon) or Windows (x86_64) from
[the releases](https://github.com/flexo0/codeweight/releases/latest): unpack the archive for your platform and put
`codeweight` (`codeweight.exe` on Windows) on your `PATH`.

Or from source, with Rust 1.100 or later and a C compiler (the tree-sitter grammars are C, built from source with the
crate):

```
cargo install codeweight
```

or the latest from GitHub, `cargo install --git https://github.com/flexo0/codeweight`.

For a coding agent that reads `SKILL.md` skills, link or copy `skills/codeweight/` into the
agent's skills folder, e.g. from a clone: `ln -s "$PWD/skills/codeweight" ~/.claude/skills/codeweight`.

## Quick start

```
codeweight scan [<path>...]     weigh files and folders (default `.`)
codeweight show [<rev>]         weigh the net delta of a commit (default `HEAD`), or of a range `A..B`
codeweight diff                 weigh the net delta of the uncommitted changes (`--staged`: the staged ones alone)
codeweight init                 write the default weights file to ./codeweight.toml, for you to tune
```

Each prints four numbers: code, comments, docs and tests (`--short`: the four on one line). `--code`,
`--tests`, `--docs` or `--comments` prints that number alone, and with `-v` the functions behind it, heaviest first; `--json` the same as JSON.

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

Rust 1.100 (pinned in `rust-toolchain.toml`, so rustup picks it up) and a C compiler for the grammars:
`cargo build --release`; checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## Licence

MIT, see [LICENSE](https://github.com/flexo0/codeweight/blob/master/LICENSE).
