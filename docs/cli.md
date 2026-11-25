# The CLI

```
codeweight scan [<path>...] [<excl>] [<one>|--short] [-v] [--json]                     weigh files and folders (default `.`)
codeweight show [<rev>|<A>..<B>] [-C <path>] [<excl>] [<one>|--short] [-v] [--json]    weigh a commit (default `HEAD`) or a range
codeweight diff [-C <path>] [--staged] [<excl>] [<one>|--short] [-v] [--json]          weigh the net delta of the uncommitted changes
codeweight init [<file>|-]         write the default weights file (default ./codeweight.toml), - to stdout

<one>: --code|--tests|--docs|--comments [--top <N>] [--by file|function]
<excl>: --exclude <pattern>, repeatable
```

`scan`, `show` and `diff` take `--config <file>`, before the command or after; `init` doesn't (it reads no
weights file, so a `--config` there is a usage error, not ignored). The three are each the general command, the four
numbers, or with `--code`, `--tests`, `--docs` or `--comments` (at most one) the specific one, that number alone and
its report;
`--top <N>` (the N heaviest functions) and `--by file|function` (what the report lists) are the specific one's
alone; `--verbose` (`-v`) is the specific one's whole report, the general one's warnings on stderr. `--json` prints
JSON instead of text; `--short`, the general one's alone, one line.

- `scan`'s `<path>...` are files and folders, absolute or relative, `.` when left out; `show`'s `<rev>`, `HEAD`
  (as `rg` / `tokei` walk `.` and `git show` shows `HEAD`), so the common case needs no argument.
- `<rev>` is any git revision: a hash, a short hash, `HEAD~2`, a tag. `-C` is any path inside the repo, relative or
  absolute (default `.`), `--repo` its long name (every option a long one, the short ones the common ones');
  `git -C` finds the repo from it, and the weights file is looked up at its root.
- A single `<rev>` is compared against its first parent (a merge commit too); the root commit against the empty tree.
- A range `A..B` diffs the two trees directly, so a change made and undone inside it nets to zero: one net delta,
  not `git show A..B`'s one per commit.
- `diff`: the uncommitted changes, `HEAD` against the working tree, untracked files too
  (the commit `git add -A && git commit` would make), so the weight of a commit can be seen before it is made; not
  `git diff`'s index against the working tree, which would leave out what is staged and what is untracked.
  `--staged` (git's name for it, as in `git diff --staged`): `HEAD` against the index, the commit `git commit` would
  make, for a commit staged part by part (`git add -p`) or with other changes left in the working tree.
- `show` and `diff` weigh the code, the docs, the tests and the comments of a delta as `scan` does of the files:
  each side of a change kept as `scan` keeps a file, for `--code`, `--tests`, `--docs` or `--comments` as each keeps
  one.
- `--exclude <pattern>` (repeatable, `scan`, `show` and `diff`) leaves out more for one run, added to the weights
  file's `[files] exclude`, not in their place: a folder or two is left out without a weights file, and without
  copying the default excludes into one (a user's list replaces the defaults', [config.md](config.md)). A pattern
  is an exclude's: a glob, or a folder ending in `/`, gitignore's form (`build/` at any depth, `src/gen/` or
  `/build/` from the globs' root), as the shorter way to write `**/build/**`. It is what is weighed, so it comes
  after the command's own options and before the output's. A bad pattern is a usage error (exit 2).
- Exit codes: 0 done, 1 an error (a bad path, revision or weights file), 2 a usage error, as is the convention, so a
  script tells a typo in an option from a missing file. `main` prints the error with its context chain
  (`error: ...`) to stderr and ends with 1; the usage errors are clap's own, exit 2. A closed stdout or stderr
  (`| head` quit early) is no error: the writes return `cli::Closed` and `main` ends with 0, silent.

Built (`cli`, `report`): every command and option. The delta commands are named by git's verbs, `show` for what a
commit (or a range) changed and `diff` (`diff --staged`) for what is not committed yet, so a git user knows them on
sight; they were first `commit` (read as one that makes a commit) and `pre-commit` (read as the hook framework's,
and as what is about to be committed, though without `--staged` it is everything uncommitted). The first names are
gone, no alias: renamed before the first release, nothing to stay compatible with yet. A command of several words would be kebab-case, as git's and cargo's are and as the options
are. `init` never overwrites a file (it is created new, else an error);
`init -` prints the defaults to stdout instead, as `-` means for a file argument. `--by` defaults to `function`;
`--top` keeps the report's first N lines, whichever `--by` lists (so "the N heaviest functions", or files).
The help says what a command does in the imperative (`Weigh files and folders`), as clap's own `Print help` and
cargo's do, and what an option or argument is as a noun phrase (`The weights file ...`); a command's `about` (`-h`
and the commands' list) is a short line, the rest its `long_about` (`--help`). Every command lists its options in
the same order, `--config` last before `--help`.

- The general command (no `--code`, `--tests`, `--docs` or `--comments`) prints four lines, the code's, the comments',
  the docs' and the tests' number in that order, each signed as a delta is, `scan`'s too, a zero without a sign
  (`code +15.5`, `comments +3`, `docs 0`, `tests -5`), so a script or a person reads all four at once; the
  comments' come right after the code's, as they are its prose. `--short` prints the four on one line in the same
  order, `c+15 m+3 d0 t-5`, the comments' `m` (messages, `c` being the code's): all four always there, each rounded
  to a whole number (from the two decimals the text shows, so `0.5` is `+1`), a zero `d0`. A commit's subject
  carries the line, and in code that explains itself in docstrings the comments are as much of a change as its
  code: without them a subject hides prose moved from the docs into the code (`c0 d-1134` that was `m+438` too). `--json` prints the
  four as one object (below). The input is read once: one walk of the paths, or one `git diff-tree` /
  `git cat-file --batch` pass, each file weighed once and split into its parts (a Rust file with tests counts its
  code toward the code and its tests toward the tests; a code file's comments toward the comments).
- A specific command (`--code`, `--tests`, `--docs` or `--comments`) prints that number alone (`scan` a number, a
  delta signed, `+42.5`); `--verbose`, `--top` or `--by` print its whole report (a list was asked for). `--top` and
  `--by` need one of the four flags, and `--short` none: a list of several kinds in one would sum what never sums,
  and one number is short already. The four flags exclude each other, `--short` and `--json` too, all usage errors
  (exit 2). `--code` weighs the code without its tests, `--tests` the tests alone, `--docs` the docs alone,
  `--comments` the code's comments alone, the same for a delta: the code, the tests, the docs and the comments are
  always four numbers, never summed into one, so none hides another (a word of prose and a statement are no one
  scale). Each weighs at its full weight; there is no factor that mixes them. The comments are a code file's
  comments and docstrings ([weight.md](weight.md)), its test code's left out, so a test file is never in
  `--comments`' report; their report has a row per function, class and file, as the code's.
  The tests are the test files (by path) and the test code inside a source file (Rust's, see [languages.md](languages.md)). The docs
  are the Markdown files, those the docs globs match (by path, `[files] docs_globs`, see [config.md](config.md)),
  each weighed by its words, punctuation and symbol runs ([weight.md](weight.md)); a file they match is docs alone, though it is
  under `tests/` or the test globs match it, and `--code` and `--tests` never see it. A Markdown file
  is one line of the report, its `(file)` (`--by function` too: it has no functions), and never has a warning.
  With the numbers alone (the four, `--short`, or a specific one's total), the warnings (syntax errors, unreadable
  folders) are left out, and the general one's go to stderr with `--verbose`: a file tree-sitter can't parse cleanly
  (a grammar behind the language, Java 25's statements before `super(...)`, say) is weighed near enough, and a
  warning on every run is noise, read when asked for. A specific command's JSON is its whole report, `--verbose` or not: a machine reads the
  fields it wants.
- `scan`'s text with `--verbose`: the total, then one line per function or class (`<weight>  <path>  <name>`, a class as
  `Name (class)`, a file's code outside any class as `(file)`) or per file with `--by file`, heaviest first, then by
  path and the shown name; then the warnings (syntax errors, then the folders that couldn't be read: `<path> can't be read (<why>), left
  out`). A line that weighs 0 (at two
  decimals) is left out. A file in no language codeweight reads isn't listed, in the text or the JSON: the
  report is of the code weighed.
- `show`'s and `diff`'s text with `--verbose`: the total delta (`total +42.5`), then `<delta>  <status>  <path>  <name>` per line, a renamed
  file's path `old -> new`. A commit has no "heaviest": its lines sort by the delta's size, a gain before a loss of
  the same size, then by path and name, so `--top` keeps the largest changes either way. A line whose delta is 0 (at
  two decimals) is left out, a pure rename's too: the report is of weight, not of files moved.
- The JSON is the machine contract, its fields in [json.md](json.md). The general command's is `{code, comments, docs,
  tests, warnings}`, the numbers rounded to two decimals, a commit's `from` / `to` first, as below; the warnings as the
  report's.
- A specific command's JSON: one object with the text's lines as `rows`, in its
  order, `--top` and `--by` acting alike, so the two formats can't disagree (one row list and one sort feed both).
  A row by file is `{path, weight}`, by function `{path, kind, name, line, weight}`, `kind` `function`, `class` or
  `file`; a commit's row has `before`, `after`, `delta`, `status` and `old_path` instead of `weight`, and the object
  `from` / `to` (full hashes, `from` null for the empty tree, `to` null for `diff`'s uncommitted changes). Every field is
  always there, null where it has no value; the numbers rounded to two decimals as the text's, so a float's last bits never show. `total` is the whole
  total, not the rows'. Warnings are `{path, kind: "syntax_errors", side}`, `side` a commit's `before` / `after`, and a scan's
  `{path, kind: "unreadable", side: null}`.

## Compatibility

What a script or a hook relies on stays as it is, so codeweight grows by adding, never by changing:

- a command's name, an option's name and meaning, a default (`.`, `HEAD`, `--by function`), the exit codes (0, 1, 2);
- the text of the numbers alone: their lines (`code +15.5`), which may gain a line only at their end (the comments'
  line came before the first release, so it could go after the code's), `--short`'s line (`c+15 m+3 d+17 t-5`,
  scripts match it with a pattern), one number's total; a report's
  lines may gain a column only at their end;
- the JSON's fields ([json.md](json.md)): a field is added, never renamed, removed or changed in type.

So: a new option, command or `--by` value is added; a rename keeps the old name as a hidden alias; a combination that is a usage error today (`--short` with `--code`, `--tests` or
`--docs`) may become a meaning later, never the other way, which is why they are errors now and
not guessed at (`-v` without one of them was one, then became the warnings on stderr).

Should a break happen all the same, the version says so: [releasing.md](releasing.md)'s table, a minor bump while
0.x, a major one from 1.0.
