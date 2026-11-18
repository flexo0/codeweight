# codeweight: the design

Built: milestones 0-2 (the repo, the CLI and the config, the engine and Java). Once a part is built, its section is
rewritten as a reference (what it is, why so, what is left).

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
- Exit codes: 0 done, 2 an error (a bad path, revision or weights file). `main` prints the error with its context
  chain (`error: ...`) to stderr; clap's own usage errors exit 2 too.

Built: `scan` of files, `init`, the four options. `init` never overwrites a file (it is created new, else an error).
`--by` defaults to `function`. Not built yet, an error for now: `commit` (milestone 5), a folder given to `scan`
(milestone 4), `--format json` (milestone 6). The text report (`report`) is a stopgap for milestone 6: the total,
then one line per function or class (`<weight>  <path>  <name>`, a class as `Name (class)`, a file's code outside
any class as `(file)`, the zero-weight ones left out) or per file with `--by file`, heaviest first, then by path and
name; `--top` keeps the first N lines. Then the warnings (syntax errors), then the skipped files.

## 2. The weights file

TOML. The first found wins: `--config`, `./codeweight.toml`, `codeweight.toml` at the repo's root (for `commit`, and
for a `scan` path inside a repo), the built-in defaults (`src/default.toml`). A user's file is merged over the
defaults, so it holds only what it changes.

Built (`config`, `config::find`):

- The repo's root is the nearest folder up from the path that holds `.git` (no `git` run for it); for `scan`, from
  the first path given. A `--config` file that doesn't exist is an error.
- The merge is on the TOML trees: a table key by key, any other value replaced whole (so an array of the user's
  replaces the default one, not appends to it). The merged tree is then read with `deny_unknown_fields`, so an
  unknown key anywhere is an error.
- Checked after: every `[weights]` key is a category (and every category has a weight, which only a file on its own,
  without the defaults under it, can miss); every `[languages.<name>]` is a language codeweight reads (just `java`
  for now); every `node_kinds` key is a named node kind of that language's grammar and its value a category or
  `"none"`; every weight and factor is a finite number, 0 or above. The error names the table and the key, and the
  file.
- A language's weights are `[weights]` with its `[languages.<name>.weights]` over them; its `node_kinds` override
  its own table (see section 4).
- The defaults hold no `[languages.<name>.weights]` (the example above has one only to show the table): the merge
  never removes a key, so a default per-language weight would beat the same key a user's file sets in `[weights]`,
  and that change would silently do nothing for that language. Should tuning (milestone 7) want one, the merge must
  first drop a default per-language key whose base key the user's file sets.
- `[files]` is read and checked for its types only; folder walks use it from milestone 4.

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
bool_op   = 1           # each run of like && / || operators: a && b && c is 1, a && b || c is 2
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
- Only a construct's body is one level deeper; its head (a condition, a loop's header, a catch's parameter, a case's
  label, try's resources) stays at the construct's depth, so the same condition weighs the same wherever it is. A
  `for`'s condition and update stay at the loop's depth although they run on every pass. A catch or finally clause
  is at its try's depth (it is beside the try's body, not in it), its block one deeper.
- `else if` chains and `switch` / `match` cases are siblings of one level, not deeper ones. The switch itself is not
  a nesting construct; each case is (a branch, its statements one deeper). An `if` directly in an if's `alternative`
  stays at that if's depth; an `if` inside an `else { }` block is nested. A ternary chained in a ternary's
  alternative (`a ? x : b ? y : z`) is a sibling the same way.
- A function or class declared inside a function (a local or anonymous class, its methods, a nested function)
  nests its body; a member class or method doesn't, whatever the depth of classes around it. Depth isn't reset at a
  function: a method of a class in an `if` in a method counts both.
- `bool_op`: one per run of like `&&` / `||` operators, the first run counted too, not one per operator: `a && b`
  and `a && b && c` are 1, `a && b || c` is 2, `(a && b) || (c && d)` is 3. A logical expression counts one when its
  operator differs from that of the logical expression it is an operand of, or it is no operand of one; parentheses
  are see-through, anything else (`!`, a call) starts a new run. Every logical expression counts, in a condition or
  not. Java's `&` / `|` are not logical (they can't be told from bitwise ones without types). The rule is the
  language's `logical` hook, whatever its node kind's table entry is.
- `jump`: `break`, `continue`, `throw`, and an early return: a return that isn't the last statement (comments aside)
  of its function's or lambda's body; the last one is a `statement`. So `if (x) return a; return b;` is a jump and a
  statement, and both returns of an `if / else` at the end are jumps.
- Each function's weight is kept apart, its nested functions' not in it; code outside any function belongs to its
  class, or else to its file. A declaration's own weight (`function`, `class`) is its own entry's; a lambda is not a
  function, its code belongs to the function around it.
- Names: a function or class is named by the classes and functions around it, `Outer.Inner.method`,
  `Outer.method.Local.run`. A Java constructor is `Class.Class`, an anonymous class `new Type` (its type arguments
  dropped: `Outer.method.new Runnable.run`), an enum constant's body the constant's name. Same-named entries of one
  kind in a file are one entry, their weights summed, its line the first one's: overloads, a class's constructors,
  two anonymous classes of a type in one method. The name is then a stable key for `commit` (an added overload is a
  heavier function, not a new one), and (path, kind, name) is unique, so the sort is total. A class and a method of
  the same name stay apart.
- A weight is an `f64`; the text report shows at most two decimals.

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

Built so far: `cli`, `config`, `lang` (Java), `engine`, and stopgaps of `source` (the files given, read as UTF-8,
lossy) and `report` (the text above). Dependencies come with the milestone that uses them; so far `tree-sitter`,
`tree-sitter-java`, `clap`, `serde`, `toml`, `anyhow`, and `tempfile` for the tests.

- `lang::Lang` is a table of plain data and two hooks: the extensions, the grammar, the node kind → category list,
  the nesting constructs (each with its body: the children it nests, by field name or node kind, either listed or
  all but those listed; and its chain field, for `else if`), the return, lambda and parenthesis kinds, `logical`
  (a node's `&&` / `||`) and `scope` (the function or class a node declares, and its own name). A test checks that
  every kind and field the tables name is in the grammar, so a typo can't silently weigh nothing.
- `engine::Weigher` turns a language's tables and its weights into arrays by node kind id once, then walks each
  tree with an explicit stack (a long `a + b + c + ...` chain is a deep tree, too deep for recursion); a node's
  weight is added to its entry's sum, so a file's sums are in source order and deterministic.
- The engine's tests are snippets weighed under `tests/weights.toml` (round numbers, complete on its own, the
  defaults never under it), each sum worked out by hand in the test. The CLI's tests run the binary in temp folders.

## 5. The commit's net delta

1. The changed files: `git diff-tree -r -M <from> <to>` (renames detected, so a moved file is not a deletion and an
   addition).
2. Their text on both sides, all through one `git cat-file --batch`.
3. Per file `delta = weight(after) − weight(before)`: an added file all positive, a deleted one all negative; an
   unsupported one skipped and listed.
4. Per function, matched by its qualified name (`Class.method`, `module.func`): added, removed, heavier, lighter.
5. The report: the total delta first (`+42.5`, `-17`), then the files' and the functions' deltas.

## 6. Milestones

0. (built) The repo: `Cargo.toml`, `README.md` (usage, the weights file), the MIT license, GitHub Actions (`cargo fmt
   --check`, `clippy -D warnings`, `test`).
1. (built) The skeleton: the CLI, the config with its embedded defaults, `init`.
2. (built) The engine and Java, for `scan <file>`: snippet tests with weights worked out by hand.
3. TypeScript, TSX and Python.
4. Folders: the walk, the excludes, the test globs and factor, `rayon`.
5. `commit`: tests on throwaway repos (an add, a delete, a rename, a merge, the root commit, a range).
6. The reports: text, JSON, `--top`, `--by`.
7. Tuning on real code: the default weights adjusted until the heaviest functions found are the hardest ones.
