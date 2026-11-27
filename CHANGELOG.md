# Changelog

codeweight's versions, newest first. The version is semver: while 0.x, a break (a JSON field, a command, an option
or a weights-file key renamed, removed or changed) or the default weights' numbers moving is a minor bump, an
addition or a fix a patch; from 1.0, a break is a major bump. The whole policy and how a release is made:
[docs/releasing.md](https://github.com/flexo0/codeweight/blob/master/docs/releasing.md).

## 0.2.0 - 2026-11-27

### Added

- One-line installers, `install.sh` (Linux, macOS) and `install.ps1` (Windows), attached to each release set to its
  tag; each checks the archive against the `.sha256` each archive now gets, and the release runs each from it.

### Fixed

- `scan` weighs a file named in two spellings once (relative and absolute, through `..` or a symlink), deduped on its
  canonical path and shown as first found, so `scan s $PWD/s` totals one file's weight, not twice it.
- A file under a folder scanned that can't be read (mode 000, gone since the walk) is left out with an `unreadable`
  warning and the rest weighed, instead of ending `scan` with exit 1; a file named itself stays an error.
- A folder given is walked though it or a folder above it is excluded, the excludes matched from it down, so
  `scan build/proj` (or `scan` run in it) weighs its files instead of a silent total 0.
- The text rounds a weight as the JSON does, a tie at the third decimal away from zero, so `statement = 0.625` shows
  0.63 in `scan --code` as in `--json`, not 0.62.
- A shebang is a comment kind in JavaScript, TypeScript and Rust (`hash_bang_line`, `shebang`), so a license header
  after `#!/usr/bin/env node` is still the file's header and weighs nothing in the comments, as in Python.
- The comments' weight leaves out test code's entries, so `show --comments -v` on a Rust file with a test and a
  non-test entry of one name (`impl A` beside `#[cfg(test)] impl A`) no longer panics.
- A deeply nested broken JavaScript file no longer aborts the scan with a stack overflow.
- Rust: a test attribute is told by its path, so `#[tokio::test(flavor = "multi_thread")]` marks test code as
  `#[tokio::test]` does, whatever its arguments.
- Java: `case null, default` (whose default the grammar parses as an identifier) weighs nothing under `free_default`,
  as `default:` and `default ->` do.

## 0.1.0 - 2026-11-27

The first release.

- `codeweight scan [<path>...]` weighs files and folders (default `.`), `show [<rev>|<A>..<B>]` the net delta of a
  commit or a range (default `HEAD`, `-C` the repo), `diff` the uncommitted changes' (`--staged`: the staged ones
  alone), `init` writes the default weights file to tune (`-`: to stdout).
- Java, TypeScript, TSX, JavaScript, Python, Rust, Go and C, parsed by tree-sitter; a file that doesn't parse cleanly
  is still weighed, with a warning.
- Every element has a weight from the weights file, heavier when nested in branches, loops, `try`, lambdas and
  nested functions; the defaults are tuned on 1,400+ pairs of a snippet and its simpler counterpart.
- Four numbers, never one sum: the code's, its comments' (comments and docstrings), the docs' (Markdown,
  reStructuredText and AsciiDoc files, each by its own markup's rules) and the tests' (the test files and Rust's
  `#[cfg(test)]` items); `--short` on one line (`c+15 m+3 d+17 t-5`), `--code`, `--comments`, `--docs` or `--tests`
  one alone, with `-v` its report by function or file (`--by`, `--top`).
- `--json`, the machine contract ([docs/json.md](https://github.com/flexo0/codeweight/blob/master/docs/json.md)).
- The weights file (`--config`, else `./codeweight.toml`, else the repo root's) merges over the defaults, an unknown
  key an error; its `[files] exclude` and `--exclude` leave paths out, each a `.gitignore` line, and `scan` honours
  a git repo's `.gitignore`s.
- Prebuilt binaries for Linux (x86_64, static), macOS (x86_64 and Apple silicon) and Windows (x86_64), and a skill
  for coding agents (`skills/codeweight/`).
- Exit 0 on success, 1 on an error (a bad path, revision or weights file), 2 on a usage error.
