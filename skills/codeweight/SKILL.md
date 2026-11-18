---
name: codeweight
description: Measure how heavy code is with the `codeweight` CLI.
---

# codeweight

Every element of code (statement, call, branch, loop, lambda, function, class, ...) has a weight; nesting makes it
heavier. A function's, file's or folder's weight is the sum. The numbers are relative: compare code under the same
weights file, never against a fixed threshold.

## Commands

| Command | Prints |
|---|---|
| `codeweight scan <path>...` | one number: the code's weight, tests left out |
| `codeweight scan-tests <path>...` | one number: the tests' weight (test files, Rust `#[cfg(test)]` items) |
| `codeweight scan <path>... -v` | `total N`, then `<weight>  <path>  <name>` per function/class, heaviest first |
| `codeweight commit <rev>` / `A..B` `[-C <repo>]` | one signed number: the net delta of the code (`+42.5`, `-17`), tests left out |
| `codeweight commit-tests <rev>` / `A..B` `[-C <repo>]` | one signed number: the net delta of the tests alone |
| `codeweight pre-commit [-C <repo>]` | the code's delta of the uncommitted changes: `HEAD` against the working tree, staged or not, untracked files too, ignored ones left out |
| `codeweight pre-commit-tests [-C <repo>]` | the tests' delta of the uncommitted changes |
| `codeweight commit <rev> -v` (any of the four) | `total +N`, then `<delta>  <status>  <path>  <name>` per changed function (`added`, `removed`, `heavier`, `lighter`) |

The code and the tests are always two numbers, never summed: report them apart.

Options: `--top N` (only the first N lines), `--by file` (files instead of functions), `--format json` (the whole
report as JSON: `total`, `rows`, `warnings`; a commit's rows have `before`, `after`, `delta`, `status`, and its
`from` / `to` the hashes, `to` null for `pre-commit`'s uncommitted changes).
`--top` and `--by` imply `-v`.

## Recipes

- Heaviest functions: `codeweight scan src --top 10`.
- Did my change make it lighter? Not committed yet: `codeweight pre-commit` and `codeweight pre-commit-tests` (they
  leave the index alone). Committed: `codeweight commit HEAD` and `codeweight commit-tests HEAD`, or `main..HEAD` for
  a branch. `-v` for which functions changed.
- Parsing the output in a script: `--format json`, read `.total` / `.rows`.

## Notes

- The weights file: `--config <file>`, else `./codeweight.toml`, else `codeweight.toml` at the repo root, else the
  built-in defaults. Don't create or edit it unless asked.
- Files in other languages are left out, unlisted. A file with syntax errors is still weighed, with a warning
  (on stderr in one-number mode).
- Exit 0 on success, 2 on an error (`error: ...` on stderr).
- `codeweight: command not found`: install it, `cargo install --git https://github.com/flexo0/codeweight` (or
  `cargo install --path .` in a clone).
