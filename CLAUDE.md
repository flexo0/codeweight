# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`codeweight`: a CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a lambda, a
function, a class, ...) has a weight, set in a weights file; an element nested in branches, loops, lambdas and nested
functions weighs more. The weight of a file, a folder or a function is the sum of its elements' weights; the weight of
a git commit (or a range) is its net delta, after minus before, positive or negative. It reads Java, TypeScript (TS,
TSX) and Python, parsed by tree-sitter.

Milestones 0-4 are built (the CLI, the config, the engine, Java, TypeScript, TSX, Python, folders); the design and
its milestones are `docs/design.md`. Build the rest in the milestones' order, and keep that doc in step with what is
built (a decision made while building goes into it, in the same commit).

## Checking a change

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, then the tests: `cargo test` (only the ones of the
  change by default, `cargo test <filter>`; the whole suite when the change spans modules).
- The engine's tests are small snippets of each language with their weights worked out by hand in the test, under a
  test weights file of their own (round numbers), never the defaults: tuning the defaults must not break them.
- The `commit` tests make throwaway git repos in a temp dir (`tempfile`); never run against a real repo.
- Rust 1.100 (`rust-toolchain.toml`, `rust-version` in `Cargo.toml`), edition 2024. Few dependencies: `tree-sitter`
  and its grammars (`tree-sitter-java`, `tree-sitter-typescript`, `tree-sitter-python`), `clap`, `serde`, `toml`,
  `serde_json`, `ignore` and its `globset` (the `[files]` globs), `rayon`, `anyhow`; `tempfile` for the tests. A new
  one needs a reason.
- External tool: `git` (the commit mode runs it, `git diff-tree` and `git cat-file --batch`; no `git2` / `gix`).

## Invariants

- The engine is pure: text and a language in, weights out. It knows nothing of files, folders or git; `source`
  turns every input into `(path, language, text)` items.
- Every weight and factor comes from the weights file. The defaults are a TOML file in the repo
  (`src/default.toml`), embedded with `include_str!` and written out by `codeweight init`: no weight is a literal in
  the code. A user's file is merged over the defaults; an unknown key, category or node kind is an error, not ignored.
- Nesting is the depth of nesting constructs only (branch, loop, `try`, lambda, nested function or class), never of
  plain tree levels; `else if` chains and `switch` / `match` cases are siblings, not deeper.
- The output is deterministic: sorted by weight, then by path and name, whatever order `rayon` finished in.
- The JSON output is the machine contract (other tools read it): a change to its fields changes the README's
  description of it, in the same commit.
- A file that doesn't parse cleanly is still weighed (tree-sitter recovers) and gets a warning in the report; an
  unsupported file is skipped and listed, never an error.
- Errors carry context (`anyhow::Context`) and end the CLI with exit 2; no `unwrap` / `expect` outside tests but on
  invariants that can't fail.

## Where to change what

- A language: a module in `src/lang/` (its grammar, extensions, node kind → category table, nesting kinds, how a
  function is named), its default weights' overrides in `src/default.toml` if any, its tests.
- A category: `src/default.toml` (its default weight), the languages' tables, the README's list.
- A command or option: `src/cli.rs`, the README's usage.

## Style

- rustfmt defaults, clippy clean. Doc comments say what a thing is for, not how.
- Commit messages: one line, `<area>: <what changed and how it behaves>`, the area a module (`engine`, `lang/java`,
  `config`, `git`, `report`, `cli`) or `docs`, `tests`, `ci`.
