# Architecture

| Module | What it does |
|---|---|
| `cli` | the arguments (`clap`), the commands |
| `config` | loads the weights file, merges it over the defaults, validates it |
| `source` | an input as `(path, kind, text)` items, a kind a language's code or Markdown: from disk (`ignore`, the excludes) or git blobs at a revision |
| `lang` | a module per language (Java, TS and TSX, JavaScript, Python, Rust, Go): grammar, extensions, node kind → category, nesting kinds, function names |
| `engine` | one tree walked: each node's weight, summed by function, class and file |
| `markdown` | a Markdown text's docs weight: its words and symbol runs, line by line, no tree-sitter |
| `git` | the files a commit, a range or the uncommitted changes changed (`git diff-tree -r -M`; the uncommitted ones as a tree, `git add -A` into a copy of the index, `git write-tree`), their text before and after (`git cat-file --batch`) |
| `report` | the three numbers (a line each, `--short`'s one line, their JSON), and one number's report, the text table and the JSON; for `show` and `diff`, the deltas |

Files are weighed in parallel (`rayon`).

Built: every module. The dependencies: `tree-sitter`, `tree-sitter-java`, `tree-sitter-typescript`,
`tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-rust`, `tree-sitter-go`, `clap`, `serde`, `serde_json` (the JSON report), `toml`, `anyhow`, `ignore`, `globset` (the
`[files]` globs; `ignore` is built on it, so it costs no build), `rayon`, and `tempfile` for the tests. `git` is run
as a command, no `git2` / `gix`.

- `source` (built): a file given is weighed as is, though an exclude matches it; a folder is walked by `ignore`, its
  `.gitignore`s (and `.ignore`s, the global gitignore) honoured, hidden files and folders left out, each folder's
  entries in the order of their names, so the walk is the same on every run. A file the excludes match is left
  out, unlisted; a folder whose every path is excluded (an exclude ending in `/**`) isn't walked at all. A file in no
  language codeweight reads is left out, unlisted. A path under a folder given that the walk can't read (a folder
  it may not list) is left out and listed as unreadable, a warning, so one root-owned folder doesn't make the tree
  unscannable; a folder given that can't be read is an error, as it leaves nothing to weigh. The globs (`*` and `?` within a path's part, `**` across parts)
  are matched against a file's path from the root of the git repo it is in (the root `config::find` uses), so one
  glob means the same from any folder scanned, and later for `show` and `diff`; outside a repo, from the current folder when
  the path is in it, else from the folder given (a file given: its name), never with the folders above, so a project
  under a `build` or `tests` folder is neither excluded nor taken for tests whatever the path's spelling. A
  file the test globs match is a test file: the code's number leaves it out, the tests' keeps it and the files of a
  language with tests inside its source files (Rust's), each weighed for its tests alone. A file the docs globs match is
  Markdown (`source::Kind`), docs alone and never a test file, though a test glob matches it too: the docs' number has
  only such files, the code's and the tests' none (`source::Mode`: the code, the tests or the docs). The general
  command weighs every file once and splits it into the three (`Weighed::split`); `--code`, `--tests` or `--docs`
  keeps only that one's files (`Input::keep`). A path is shown as given,
  or the folder given joined with the path in it, without a leading `./`; a path given twice, or under two folders
  given, is weighed once.
- `source` for `show` and `diff` (`from_commit`): a changed file's each side is kept as a walk of the repo's root would keep
  it: its path (from the root, as `git diff-tree` gives it) not hidden (no part starting with `.`), not excluded, in
  a language codeweight reads or Markdown, and, with `--code`, `--tests` or `--docs`, kept for that one as a scan keeps a file (never two in
  one number; without them every side is kept, its parts split as a scan's); a side that isn't a regular file (a symlink, a
  submodule) is none. A side's language is its own path's, so a rename `a.js -> a.ts` is an added file. A change with
  no side kept is left out, unlisted. Then
  the kept sides' blobs, each once, all through one `git cat-file --batch`; all of a commit's texts are in memory
  at once (a commit's, or a range's, changed files only), then weighed in parallel.
- The files are listed first, then read and weighed in parallel (`rayon`), each read when weighed so the texts are
  never all in memory; the results are collected in the listed order, and the report sorts them (weight, then path
  and name), so the output doesn't depend on which thread finished first.

- `lang::Lang` is a table of plain data and five hooks: its name and its weights file key, the extensions, the
  grammar, the node kind → category list, the nesting constructs (each with its body: the children it nests, by
  field name or node kind, either listed or all but those listed, or by their place among the named children; and
  its chain, the children of its own kind that stay at its depth, for `else if`), the return, lambda and parenthesis
  kinds, the cast, pattern and number kinds (for the `[conditions]`, the number ones for a negative literal too),
  the wrappers (the statement kinds a return is looked through for its place: Rust's expression statement,
  Go's labeled one), the lists (a list of statements a grammar puts between a body and its statements, a return the
  last of it being its body's last: Go's `statement_list`),
  `operator` (a node's operator, by its run: `&&`, `+` / `-`, a comparison, a unary one, ...; a logical one is a
  `bool_op`, any other an `operator`), `exempt` (a node that weighs nothing though its kind has a category, where
  the kind can't tell: a docstring, a case's guard, a call's callee, `this.x`), `also_statement` (a node that
  weighs a statement beside its kind's category: Rust's block ending in a value, Go's `i++` and `x += 1`, statements
  that are their own operator), `scope` (the function or class a node declares, and its own name) and `tests` (a
  node that is test code inside a source file, it and all in it: Rust's `#[cfg(test)]` items; none for a language whose tests are files of their own). A lambda `scope` names a function weighs and nests as a function (TS's named
  arrows). A test checks that
  every kind and field the tables name is in the grammar, so a typo can't silently weigh nothing.
- A call's arguments are counted alike in every language (`lang::arguments`: its `arguments` field's list items, or
  one lone argument), no hook.
- `engine::Weigher` turns a language's tables and its weights into arrays by node kind id once, then walks each
  tree with an explicit stack (a long `a + b + c + ...` chain is a deep tree, too deep for recursion); a node's
  weight is added to its entry's sum, so a file's sums are in source order and deterministic.
- The engine's tests are snippets weighed under `tests/weights.toml` (round numbers, complete on its own, the
  defaults never under it), each sum worked out by hand in the test. The CLI's tests run the binary in temp folders.
