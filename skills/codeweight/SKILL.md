---
name: codeweight
description: Measure how heavy code is with the `codeweight` CLI.
---

# codeweight

Every element of code (statement, call, branch, loop, lambda, function, class, ...) has a weight; nesting makes it
heavier. A function's, file's or folder's weight is the sum. A Markdown file has a docs weight instead: its words
and symbol runs (`## Setup` is 2). The numbers are relative: compare code under the same weights file, never against
a fixed threshold.

## Commands

| Command | Prints |
|---|---|
| `codeweight scan [<path>...]` (default `.`) | three lines, the weights of the code (tests left out), the docs and the tests: `code +9488.09`, `docs +31321`, `tests +3886.37` |
| `codeweight show [<rev>]` (default `HEAD`) / `A..B` `[-C <path>]` | the three net deltas of a commit or a range: `code +15.5`, `docs 0`, `tests -5` (a zero has no sign) |
| `codeweight diff [-C <path>]` | the three deltas of the uncommitted changes: `HEAD` against the working tree, staged or not, untracked files too, ignored ones left out |
| `codeweight diff --staged` | only the staged changes: `HEAD` against the index, what `git commit` would commit |
| any of the three `--short` | the three on one line, each rounded to a whole number: `c+15 d0 t-5` |
| any of the three `--code` / `--tests` / `--docs` | that one number alone: `9488.09`, a delta signed (`+42.5`, `-17`) |
| `codeweight scan [<path>...] --code -v` | `total N`, then `<weight>  <path>  <name>` per function/class, heaviest first |
| `codeweight show [<rev>] --code -v` (or `diff`, `--tests`, `--docs`) | `total +N`, then `<delta>  <status>  <path>  <name>` per changed function (`added`, `removed`, `heavier`, `lighter`); a Markdown file is one line, `(file)` |

The tests are the test files and Rust `#[cfg(test)]` items; the docs the Markdown files (`*.md`, `*.markdown`, under
`tests/` too). The code, the tests and the docs are always three numbers, never summed: report them apart.

Options: `--json` (JSON; without a flag `{code, docs, tests, warnings}`, a commit's with `from` / `to`; with
`--code`, `--tests` or `--docs` the whole report: `total`, `rows`, `warnings`, a commit's rows with `before`,
`after`, `delta`, `status`, and its `from` / `to` the hashes, `to` null for `diff`'s uncommitted changes).
`--exclude <pattern>` (`scan`, `show`, `diff`; repeatable): leave out more besides the weights file's excludes,
a folder ending in `/` (`gen/` at any depth, `src/gen/` from the repo's root) or a glob (`**/*.gen.ts`).
`-v`: the warnings (syntax errors, unreadable folders) on stderr; with `--code`, `--tests` or `--docs`, the whole
report. With `--code`, `--tests` or `--docs` only: `--top N` (only the first N lines), `--by file`
(files instead of functions); `--top` and `--by` imply `-v`. Without them, these are a usage error, as `--short`
with one of them, two of them, or `--short` with `--json`.

## Recipes

- Heaviest functions: `codeweight scan src --code --top 10` (the tests': `--tests --top 10`).
- Generated or vendored code left out: `codeweight scan --exclude gen/ --exclude src/legacy/`.
- Did my change make it lighter? Not committed yet: `codeweight diff` (it leaves the index alone). Committed:
  `codeweight show` (`HEAD`), or `main..HEAD` for a branch. For which functions changed: `--code -v` (or `--tests
  -v`, `--docs -v`).
- The commit about to be made, other changes left in the working tree: stage it, then `codeweight diff --staged`.
- A commit's weight in one short line: `codeweight diff --staged --short`.
- Parsing the output in a script: `--json`, read `.code` / `.docs` / `.tests`; or `--code --json` (or `--tests`,
  `--docs`), read `.total` / `.rows`.

## Notes

- The weights file: `--config <file>`, else `./codeweight.toml`, else `codeweight.toml` at the repo root, else the
  built-in defaults. Don't create or edit it unless asked.
- Files in other languages are left out, unlisted. A file with syntax errors is still weighed, with a warning
  (in the report and the JSON; with the numbers alone only with `-v`, on stderr).
- Exit 0 on success (a pipe to `head` that quits early too, silent), 1 on an error (`error: ...` on stderr), 2 on a
  usage error (a bad command or option, options that don't go together).
- `-C <path>` (`--repo <path>`): any path inside the repo, as `git -C`.
- `codeweight: command not found`: install it, `cargo install --git https://github.com/flexo0/codeweight` (or
  `cargo install --path .` in a clone).
