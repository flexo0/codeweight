# Changelog

codeweight's versions, newest first. The version is semver: while 0.x, a break (a JSON field, a command, an option
or a weights-file key renamed, removed or changed) or the default weights' numbers moving is a minor bump, an
addition or a fix a patch; from 1.0, a break is a major bump. The whole policy and how a release is made:
[docs/releasing.md](https://github.com/flexo0/codeweight/blob/master/docs/releasing.md).

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
