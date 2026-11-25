# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`codeweight`: a CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a lambda, a
function, a class, ...) has a weight, set in a weights file; an element nested in branches, loops, lambdas and nested
functions weighs more. The weight of a file, a folder or a function is the sum of its elements' weights; the weight of
a git commit (or a range, or the uncommitted changes) is its net delta, after minus before, positive or negative. It
reads Java, TypeScript (TS, TSX), JavaScript, Python, Rust, Go and C, parsed by tree-sitter. It weighs docs too: a
Markdown file's docs weight is its words, punctuation and symbol runs, a number of its own, never summed with the code's or the
tests'; and a code file's comments and docstrings, weighed as the docs are, are a fourth, the comments' (test code's
left out): `scan`, `show` and `diff` print the four (`--code`, `--tests`, `--docs` or `--comments` one alone, with
its report; `--short` the four on one line).

The user docs past the README are `docs/usage.md`, `docs/json.md` and `docs/weights-file.md`. The design is the rest
of `docs/`, a file per subject (its index `docs/README.md`). Keep those docs in step with what is built (a decision
made while building goes into its subject's file, in the same commit).

## Checking a change

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, then the tests: `cargo test` (only the ones of the
  change by default, `cargo test <filter>`; the whole suite when the change spans modules).
- The engine's tests are small snippets of each language with their weights worked out by hand in the test, under a
  test weights file of their own (round numbers), never the defaults: tuning the defaults must not break them.
- The `show` and `diff` tests make throwaway git repos in a temp dir (`tempfile`); never run against a real repo.
- Rust 1.100 (`rust-toolchain.toml`, `rust-version` in `Cargo.toml`), edition 2024. Few dependencies: `tree-sitter`
  and its grammars (`tree-sitter-java`, `tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-python`,
  `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-c`), `clap`, `serde`, `toml`, `serde_json`, `ignore` and its `globset` (the `[files]` globs), `rayon`,
  `anyhow`; `tempfile` for the tests. A new one needs a reason.
- External tool: `git` (the commit mode runs it, `git diff-tree` and `git cat-file --batch`, for the uncommitted
  changes `git add -A` into a copy of the index and `git write-tree`; no `git2` / `gix`).

## Invariants

- The engine is pure: text and a language in, weights out. It knows nothing of files, folders or git; `source`
  turns every input into `(path, kind, text)` items, a kind a language's code or Markdown; Markdown's docs weight
  (`markdown`) is as pure.
- Every weight and factor comes from the weights file. The defaults are a TOML file in the repo
  (`src/default.toml`), embedded with `include_str!` and written out by `codeweight init`: no weight is a literal in
  the code. A user's file is merged over the defaults; an unknown key, category or node kind is an error, not ignored.
- Nesting is the depth of nesting constructs only (branch, loop, `try`, lambda, nested function or class), never of
  plain tree levels; `else if` chains and `switch` / `match` cases are siblings, not deeper.
- The output is deterministic: sorted by weight, then by path and name, whatever order `rayon` finished in.
- The JSON output is the machine contract (other tools read it): a change to its fields changes its
  description, `docs/json.md`, in the same commit.
- The agents' skill, `skills/codeweight/SKILL.md`, is how agents run codeweight: a change to a command, an option,
  the text output or the JSON's fields changes it too, in the same commit.
- A file that doesn't parse cleanly is still weighed (tree-sitter recovers) and gets a warning in the report; an
  unsupported file is skipped and listed, never an error.
- Errors carry context (`anyhow::Context`) and end the CLI with exit 1 (2 is a usage error, clap's); no `unwrap` /
  `expect` outside tests but on invariants that can't fail.

## Where to change what

- A language: a module in `src/lang/` (its grammar, extensions, node kind → category table, nesting kinds, how a
  function is named), its default weights' overrides in `src/default.toml` if any, its tests.
- A category: `src/default.toml` (its default weight), the languages' tables, `docs/weights-file.md`'s list.
- A command or option: `src/cli.rs`, `docs/usage.md` (and the README's quick start, if it is there), the skill
  (`skills/codeweight/SKILL.md`).
- The docs weight (what a Markdown file's words, punctuation and symbol runs are): `src/markdown.rs`, its `[docs]` weights in
  `src/default.toml`, `docs/weight.md`; which files are docs: `[files] docs_globs`, `src/source.rs`.
- A release: `docs/releasing.md` (the version's bump, `CHANGELOG.md`'s section, the tag); a commit's subject is its
  changelog line, so no commit adds to `CHANGELOG.md` but a release's.

## Style

- rustfmt defaults, clippy clean. Doc comments say what a thing is for, not how.
- Commit messages: one line, `<area>: <what changed and how it behaves>`, the area a module (`engine`, `lang/java`,
  `config`, `git`, `report`, `cli`) or `docs`, `tests`, `ci`, `release`.
