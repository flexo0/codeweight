# The CLI

```
codeweight scan [<path>...] [<one>|--short] [--json]                       files and folders (default `.`)
codeweight commit [<rev>|<A>..<B>] [-C <repo>] [<one>|--short] [--json]    a commit (default `HEAD`) or a range
codeweight pre-commit [-C <repo>] [--staged] [<one>|--short] [--json]      the net delta of the uncommitted changes
codeweight init [<file>|-]         writes the default weights file (default ./codeweight.toml), - to stdout

<one>: --code|--tests|--docs [-v] [--top <N>] [--by file|function]
```

`scan`, `commit` and `pre-commit` take `--config <file>`, before the command or after; `init` doesn't (it reads no
weights file, so a `--config` there is a usage error, not ignored). The three are each the general command, the three
numbers, or with `--code`, `--tests` or `--docs` (at most one) the specific one, that number alone and its report;
`--verbose` (`-v`), `--top <N>` (the N heaviest functions) and `--by file|function` (what the report lists) are the
specific one's alone. `--json` prints JSON instead of text; `--short`, the general one's alone, one line.

- `scan`'s `<path>...` are files and folders, absolute or relative, `.` when left out; `commit`'s `<rev>`, `HEAD`
  (as `rg` / `tokei` walk `.` and `git show` shows `HEAD`), so the common case needs no argument.
- `<rev>` is any git revision: a hash, a short hash, `HEAD~2`, a tag. `-C` is any path inside the repo, relative or
  absolute (default `.`); `git -C` finds the repo from it, and the weights file is looked up at its root.
- A single `<rev>` is compared against its first parent (a merge commit too); the root commit against the empty tree.
- A range `A..B` diffs the two trees directly, so a change made and undone inside it nets to zero.
- `pre-commit`: the uncommitted changes, `HEAD` against the working tree, untracked files too
  (the commit `git add -A && git commit` would make), so the weight of a commit can be seen before it is made.
  `--staged` (git's name for it, as in `git diff --staged`): `HEAD` against the index, the commit `git commit` would
  make, for a commit staged part by part (`git add -p`) or with other changes left in the working tree.
- `commit` and `pre-commit` weigh the code, the docs and the tests of a delta as `scan` does of the files: each
  side of a change kept as `scan` keeps a file, for `--code`, `--tests` or `--docs` as each keeps one.
- Exit codes: 0 done, 1 an error (a bad path, revision or weights file), 2 a usage error, as is the convention, so a
  script tells a typo in an option from a missing file. `main` prints the error with its context chain
  (`error: ...`) to stderr and ends with 1; the usage errors are clap's own, exit 2. A closed stdout or stderr
  (`| head` quit early) is no error: the writes return `cli::Closed` and `main` ends with 0, silent.

Built (`cli`, `report`): every command and option. A command of several words is kebab-case (`pre-commit`), as
git's and cargo's are and as the options are. `init` never overwrites a file (it is created new, else an error);
`init -` prints the defaults to stdout instead, as `-` means for a file argument. `--by` defaults to `function`;
`--top` keeps the report's first N lines, whichever `--by` lists (so "the N heaviest functions", or files).

- The general command (no `--code`, `--tests` or `--docs`) prints three lines, the code's, the docs' and the
  tests' number in that order, each signed as a delta is, `scan`'s too, a zero without a sign (`code +15.5`,
  `docs 0`, `tests -5`), so a script or a person reads all three at once. `--short` prints them on one line,
  `c+15 d0 t-5`: all three always there in that order, each rounded to a whole number (from the two decimals the
  text shows, so `0.5` is `+1`), a zero `d0`. `--json` prints them as one object (below). The input is read once:
  one walk of the paths, or one `git diff-tree` / `git cat-file --batch` pass, each file weighed once and split
  into its parts (a Rust file with tests counts its code toward the code and its tests toward the tests).
- A specific command (`--code`, `--tests` or `--docs`) prints that number alone (`scan` a number, a delta
  signed, `+42.5`); `--verbose`, `--top` or `--by` print its whole report (a list was asked for). They need one of
  the three flags, and `--short` none: a list of three kinds in one would sum what never sums, and one number is
  short already. The three flags exclude each other, `--short` and `--json` too, all usage errors (exit 2).
  `--code` weighs the code without its tests, `--tests` the tests alone, `--docs` the docs alone, the same for a
  delta: the code, the tests and the docs are always three numbers, never summed into one, so none hides another (a
  word of prose and a statement are no one scale). Each weighs at its full weight; there is no factor that mixes
  them.
  The tests are the test files (by path) and the test code inside a source file (Rust's, see [languages.md](languages.md)). The docs
  are the Markdown files, those the docs globs match (by path, `[files] docs_globs`, see [config.md](config.md)),
  each weighed by its words and symbol runs ([weight.md](weight.md)); a file they match is docs alone, though it is
  under `tests/` or the test globs match it, and `--code` and `--tests` never see it. A Markdown file
  is one line of the report, its `(file)` (`--by function` too: it has no functions), and never has a warning.
  With the numbers alone (the three, `--short`, or a specific one's total), the warnings (syntax errors, unreadable
  folders) go to stderr. A specific command's JSON is its whole report, `--verbose` or not: a machine reads the
  fields it wants.
- `scan`'s text with `--verbose`: the total, then one line per function or class (`<weight>  <path>  <name>`, a class as
  `Name (class)`, a file's code outside any class as `(file)`) or per file with `--by file`, heaviest first, then by
  path and the shown name; then the warnings (syntax errors, then the folders that couldn't be read: `<path> can't be read (<why>), left
  out`). A line that weighs 0 (at two
  decimals) is left out. A file in no language codeweight reads isn't listed, in the text or the JSON: the
  report is of the code weighed.
- `commit`'s text with `--verbose`: the total delta (`total +42.5`), then `<delta>  <status>  <path>  <name>` per line, a renamed
  file's path `old -> new`. A commit has no "heaviest": its lines sort by the delta's size, a gain before a loss of
  the same size, then by path and name, so `--top` keeps the largest changes either way. A line whose delta is 0 (at
  two decimals) is left out, a pure rename's too: the report is of weight, not of files moved.
- The JSON is the machine contract, its fields in the README. The general command's is `{code, docs, tests,
  warnings}`, the numbers rounded to two decimals, a commit's `from` / `to` first, as below; the warnings as the
  report's.
- A specific command's JSON: one object with the text's lines as `rows`, in its
  order, `--top` and `--by` acting alike, so the two formats can't disagree (one row list and one sort feed both).
  A row by file is `{path, weight}`, by function `{path, kind, name, line, weight}`, `kind` `function`, `class` or
  `file`; a commit's row has `before`, `after`, `delta`, `status` and `old_path` instead of `weight`, and the object
  `from` / `to` (full hashes, `from` null for the empty tree, `to` null for `pre-commit`'s uncommitted changes). Every field is
  always there, null where it has no value; the numbers rounded to two decimals as the text's, so a float's last bits never show. `total` is the whole
  total, not the rows'. Warnings are `{path, kind: "syntax_errors", side}`, `side` a commit's `before` / `after`, and a scan's
  `{path, kind: "unreadable", side: null}`.
