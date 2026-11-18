# codeweight: the design

Not built yet. Once a part is built, rewrite its section as a reference (what it is, why so, what is left).

## 1. The CLI

```
codeweight scan <path>...                 a file or a folder, absolute or relative; several allowed
codeweight commit <rev>|<A>..<B> [-C <repo>]   the net weight delta of a commit, or between two revisions
codeweight init [<file>]                  writes the default weights file (default ./codeweight.toml)
```

Every command takes `--config <file>`, `--format text|json` (text by default), `--top <N>` (the N heaviest functions)
and `--by file|function` (what the report lists).

- `<rev>` is any git revision: a hash, a short hash, `HEAD~2`, a tag. `-C` is any path inside the repo, relative or
  absolute (default `.`); the repo's root is found from it.
- A single `<rev>` is compared against its first parent (a merge commit too); the root commit against the empty tree.
- A range `A..B` diffs the two trees directly, so a change made and undone inside it nets to zero.
- Exit codes: 0 done, 2 an error (a bad path, revision or weights file).

## 2. The weights file

TOML. The first found wins: `--config`, `./codeweight.toml`, `codeweight.toml` at the repo's root (for `commit`, and
for a `scan` path inside a repo), the built-in defaults (`src/default.toml`). A user's file is merged over the
defaults, so it holds only what it changes.

```toml
[nesting]
model = "linear"        # linear: 1 + factor * depth; exponential: factor ^ depth
factor = 0.5
max_depth = 10          # depth is capped here, so one deep method can't explode the total

[weights]               # the base weight of each category, for every language
statement = 1
call      = 1
branch    = 2           # if, ternary, case
loop      = 3
try       = 2
catch     = 2
lambda    = 2
function  = 3
class     = 5
bool_op   = 1           # each switch between && and || in a condition
jump      = 1           # break, continue, an early return, throw

[languages.java.weights]            # a language's overrides of [weights]
lambda = 3

[languages.typescript.node_kinds]   # a tree-sitter node kind mapped to a category (or to "none")
satisfies_expression = "statement"

[files]
exclude = ["**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/*.d.ts"]
test_globs = ["**/test/**", "**/tests/**", "**/*Test.java", "**/*.test.ts", "**/*.spec.ts", "**/test_*.py"]
test_factor = 1.0       # a test file's weight times this; 0 leaves the tests out
```

Folder walks honour `.gitignore` too (the `ignore` crate).

## 3. The weight

```
weight(node) = base[category(node)] × nest(min(depth, max_depth))
total        = Σ weight(node) over the nodes that have a category
```

- `depth` is the number of nesting constructs around the node: branch, loop, `try` / `catch`, lambda, nested
  function or class. Plain tree levels don't count (`a.b().c()` is not deep).
- `else if` chains and `switch` / `match` cases are siblings of one level, not deeper ones.
- A run of `&&` / `||` counts one `bool_op` per switch between them, not one per operator.
- Each function's weight is kept apart, its nested functions' not in it; code outside any function belongs to its
  class, or else to its file.

## 4. Architecture

| Module | What it does |
|---|---|
| `cli` | the arguments (`clap`), the commands |
| `config` | loads the weights file, merges it over the defaults, validates it |
| `source` | an input as `(path, language, text)` items: from disk (`ignore`, the excludes) or git blobs at a revision |
| `lang` | a module per language (Java, TS, TSX, Python): grammar, extensions, node kind → category, nesting kinds, function names |
| `engine` | one tree walked: each node's weight, summed by function, class and file |
| `git` | the files a commit or a range changed (`git diff-tree -r -M`), their text before and after (`git cat-file --batch`) |
| `report` | the text table and the JSON; for `commit`, the deltas |

Files are weighed in parallel (`rayon`).

## 5. The commit's net delta

1. The changed files: `git diff-tree -r -M <from> <to>` (renames detected, so a moved file is not a deletion and an
   addition).
2. Their text on both sides, all through one `git cat-file --batch`.
3. Per file `delta = weight(after) − weight(before)`: an added file all positive, a deleted one all negative; an
   unsupported one skipped and listed.
4. Per function, matched by its qualified name (`Class.method`, `module.func`): added, removed, heavier, lighter.
5. The report: the total delta first (`+42.5`, `-17`), then the files' and the functions' deltas.

## 6. Milestones

0. The repo: `Cargo.toml`, `README.md` (usage, the weights file), the MIT license, GitHub Actions (`cargo fmt
   --check`, `clippy -D warnings`, `test`).
1. The skeleton: the CLI, the config with its embedded defaults, `init`.
2. The engine and Java, for `scan <file>`: snippet tests with weights worked out by hand.
3. TypeScript, TSX and Python.
4. Folders: the walk, the excludes, the test globs and factor, `rayon`.
5. `commit`: tests on throwaway repos (an add, a delete, a rename, a merge, the root commit, a range).
6. The reports: text, JSON, `--top`, `--by`.
7. Tuning on real code: the default weights adjusted until the heaviest functions found are the hardest ones.
