# The CLI

```
codeweight scan <path>... [-v]            a file or a folder, absolute or relative; several allowed; tests left out
codeweight scanTests <path>... [-v]       the same, of the tests alone
codeweight commit [<rev>|<A>..<B>] [-C <repo>] [-v]   the net weight delta of a commit, between two revisions,
                                          or of the uncommitted changes (no revision)
codeweight init [<file>]                  writes the default weights file (default ./codeweight.toml)
```

Every command takes `--config <file>`, `--format text|json` (text by default), `--top <N>` (the N heaviest functions)
and `--by file|function` (what the report lists).

- `<rev>` is any git revision: a hash, a short hash, `HEAD~2`, a tag. `-C` is any path inside the repo, relative or
  absolute (default `.`); `git -C` finds the repo from it, and the weights file is looked up at its root.
- A single `<rev>` is compared against its first parent (a merge commit too); the root commit against the empty tree.
- A range `A..B` diffs the two trees directly, so a change made and undone inside it nets to zero.
- No revision: the uncommitted changes, `HEAD` against the working tree, untracked files too (the commit
  `git add -A && git commit` would make), so the weight of a commit can be seen before it is made.
- Exit codes: 0 done, 2 an error (a bad path, revision or weights file). `main` prints the error with its context
  chain (`error: ...`) to stderr; clap's own usage errors exit 2 too.

Built (`cli`, `report`): every command and option. `init` never overwrites a file (it is created new, else an
error). `--by` defaults to `function`; `--top` keeps the report's first N lines, whichever `--by` lists (so "the N
heaviest functions", or files).

- `scan` prints its total alone, a number, so a script or a person reads one value, and `commit` its net delta alone,
  signed (`+42.5`); `--verbose` (`-v`), `--top` or `--by` print the whole report (a list was asked for). `scan` weighs the code without its tests, `scanTests`
  (alias `scan-tests`) the tests alone: two numbers, not one mixed by `test_factor`, so neither hides the other.
  Each weighs at its full weight; `test_factor` is `commit`'s alone (a delta is one number for a change to both).
  The tests are the test files (by path) and the test code inside a source file (Rust's, see [languages.md](languages.md)). With the
  total alone, the warnings (syntax errors, unreadable folders) go to stderr. The JSON is the whole report,
  `--verbose` or not: a machine reads the fields it wants.
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
- The JSON (the machine contract, its fields in the README): one object with the text's lines as `rows`, in its
  order, `--top` and `--by` acting alike, so the two formats can't disagree (one row list and one sort feed both).
  A row by file is `{path, weight}`, by function `{path, kind, name, line, weight}`, `kind` `function`, `class` or
  `file`; a commit's row has `before`, `after`, `delta`, `status` and `old_path` instead of `weight`, and the object
  `from` / `to` (full hashes, `from` null for the empty tree, `to` null for the uncommitted changes). Every field is
  always there, null where it has no value; the numbers rounded to two decimals as the text's, so a float's last bits never show. `total` is the whole
  total, not the rows'. Warnings are `{path, kind: "syntax_errors", side}`, `side` a commit's `before` / `after`, and a scan's
  `{path, kind: "unreadable", side: null}`.
