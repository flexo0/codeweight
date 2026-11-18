# codeweight: the design

Built: milestones 0-8 (the repo, the CLI and the config, the engine and Java, TypeScript, TSX and Python, folders,
`commit`, the reports, the tuned defaults, Rust).
Once a part is built, its section is rewritten as a reference (what it is, why so, what is left).

## 1. The CLI

```
codeweight scan <path>... [-v]            a file or a folder, absolute or relative; several allowed; tests left out
codeweight scanTests <path>... [-v]       the same, of the tests alone
codeweight commit <rev>|<A>..<B> [-C <repo>] [-v]   the net weight delta of a commit, or between two revisions
codeweight init [<file>]                  writes the default weights file (default ./codeweight.toml)
```

Every command takes `--config <file>`, `--format text|json` (text by default), `--top <N>` (the N heaviest functions)
and `--by file|function` (what the report lists).

- `<rev>` is any git revision: a hash, a short hash, `HEAD~2`, a tag. `-C` is any path inside the repo, relative or
  absolute (default `.`); `git -C` finds the repo from it, and the weights file is looked up at its root.
- A single `<rev>` is compared against its first parent (a merge commit too); the root commit against the empty tree.
- A range `A..B` diffs the two trees directly, so a change made and undone inside it nets to zero.
- Exit codes: 0 done, 2 an error (a bad path, revision or weights file). `main` prints the error with its context
  chain (`error: ...`) to stderr; clap's own usage errors exit 2 too.

Built (`cli`, `report`): every command and option. `init` never overwrites a file (it is created new, else an
error). `--by` defaults to `function`; `--top` keeps the report's first N lines, whichever `--by` lists (so "the N
heaviest functions", or files).

- `scan` prints its total alone, a number, so a script or a person reads one value, and `commit` its net delta alone,
  signed (`+42.5`); `--verbose` (`-v`), `--top` or `--by` print the whole report (a list was asked for). `scan` weighs the code without its tests, `scanTests`
  (alias `scan-tests`) the tests alone: two numbers, not one mixed by `test_factor`, so neither hides the other.
  Each weighs at its full weight; `test_factor` is `commit`'s alone (a delta is one number for a change to both).
  The tests are the test files (by path) and the test code inside a source file (Rust's, see section 3). With the
  total alone, the syntax warnings go to stderr and the skipped files aren't listed. The JSON is the whole report,
  `--verbose` or not: a machine reads the fields it wants.
- `scan`'s text with `--verbose`: the total, then one line per function or class (`<weight>  <path>  <name>`, a class as
  `Name (class)`, a file's code outside any class as `(file)`) or per file with `--by file`, heaviest first, then by
  path and the shown name; then the warnings (syntax errors), then the skipped files. A line that weighs 0 (at two
  decimals) is left out.
- `commit`'s text with `--verbose`: the total delta (`total +42.5`), then `<delta>  <status>  <path>  <name>` per line, a renamed
  file's path `old -> new`. A commit has no "heaviest": its lines sort by the delta's size, a gain before a loss of
  the same size, then by path and name, so `--top` keeps the largest changes either way. A line whose delta is 0 (at
  two decimals) is left out, a pure rename's too: the report is of weight, not of files moved.
- The JSON (the machine contract, its fields in the README): one object with the text's lines as `rows`, in its
  order, `--top` and `--by` acting alike, so the two formats can't disagree (one row list and one sort feed both).
  A row by file is `{path, weight}`, by function `{path, kind, name, line, weight}`, `kind` `function`, `class` or
  `file`; a commit's row has `before`, `after`, `delta`, `status` and `old_path` instead of `weight`, and the object
  `from` / `to` (full hashes, `from` null for the empty tree). Every field is always there, null where it has no
  value; the numbers rounded to two decimals as the text's, so a float's last bits never show. `total` is the whole
  total, not the rows'. Warnings are `{path, kind: "syntax_errors", side}`, `side` a commit's `before` / `after`.

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
  without the defaults under it, can miss); every `[languages.<name>]` is a language codeweight reads (`java`,
  `typescript`, `python`, `rust`); every `node_kinds` key is a named node kind of that language's grammar and its value a
  category or `"none"`; every weight and factor is a finite number, 0 or above; every `[files]` glob compiles. The
  error names the table and the key, and the file.
- `typescript` is the table of both `.ts` and `.tsx` files (two grammars, one key): a TS setting reaches TSX too.
  Its `node_kinds` key may be a kind of either grammar (the `jsx_*` kinds are TSX's only, `type_assertion` TS's
  only); the grammar without it has no node of that kind, so there it does nothing. A `[languages.tsx]` table is an
  unknown language; should TSX ever need settings of its own, it can be added over `typescript`'s without changing
  what an existing file means.
- A language's weights are `[weights]` with its `[languages.<name>.weights]` over them; its `node_kinds` override
  its own table (see section 4).
- The defaults hold no `[languages.<name>.weights]` (the example above has one only to show the table): the merge
  never removes a key, so a default per-language weight would beat the same key a user's file sets in `[weights]`,
  and that change would silently do nothing for that language. Tuning (milestone 7) needed none; should one ever be
  wanted, the merge must first drop a default per-language key whose base key the user's file sets.
- `[files]` is for what is weighed (section 4, `source`).

```toml
[nesting]
model = "linear"        # linear: 1 + factor * depth; exponential: factor ^ depth
factor = 1.0
max_depth = 10          # depth is capped here, so one deep method can't explode the total

[weights]               # the base weight of each category, for every language
statement = 1
call      = 0.5         # a chain of getters is long, not hard
branch    = 2           # if, ternary, case
loop      = 3
try       = 2
catch     = 2
lambda    = 2
function  = 3
class     = 5
bool_op   = 1           # each run of like && / || operators: a && b && c is 1, a && b || c is 2
jump      = 1           # break, continue, an early return, throw
operator  = 0.25        # each run of like operators: a + b - c is 1, (a + b) * c is 2; x += 1, i++, -x, !x
access    = 0.25        # each field access or index: a.b.c is 2, a[i] is 1; not a call's callee, nor this.x

[languages.java.weights]            # a language's overrides of [weights]
lambda = 3

[languages.typescript.node_kinds]   # a tree-sitter node kind mapped to a category (or to "none")
satisfies_expression = "statement"

[files]
exclude = ["**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/*.d.ts"]
test_globs = [
    "**/test/**", "**/tests/**", "**/__tests__/**", "**/*Test.java", "**/*Tests.java",
    "**/*.test.ts", "**/*.spec.ts", "**/*.test.tsx", "**/*.spec.tsx", "**/test_*.py", "**/*_test.py",
]
test_factor = 1.0       # a test's weight times this in a commit's delta; 0 leaves the test files out
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
  not. Java's `&` / `|` are not logical (they can't be told from bitwise ones without types): they are bitwise
  operators. The rule is the language's `operator` hook, whatever its node kind's table entry is.
- `operator`: one per run of like operators, by the `bool_op` rule (parentheses see-through), so a long flat sum is
  one and a mix of precedences is more: `a = b` is none, `a + b + c` and `a + b - c` are 1, `(b + c) * d` and
  `a * b + c` are 2, `a * b + c * d` is 3. Like operators are those of one precedence level: `+ -`, `* / %` (Python's
  `//` and `@`), `**`, the shifts, the comparisons (`==`, `<`, ..., `instanceof`, `in`, `is`; a chained Python
  comparison `a < b < c` is one), `&`, `|`, `^`, and the unary ones (`-x`, `!x`, `not x`, `~x`, TS's `typeof`, `++`
  and `--`). A compound assignment (`x += 1`) is its operator, so `x += b + c` is one run, as `x = x + b + c`; a
  plain `=` is none, the statement already weighs it. A negative number (`-1`) is a literal, no operator. A Rust
  dereference (`*x`) and a borrow (`&x`) are none. `&&=`, `||=`, `??=` are logical: `bool_op`.
- `access`: one per field access or index, so `obj.a` is 1, `obj.a.b` and `a[i][j]` are 2. A call's callee is the
  call's, no access: `obj.f()` is a call, `obj.a.f()` a call and an access (`obj.a`), as Java's grammar has it (its
  method call holds its object, no field access around the name); TS's `new a.B()` and a JSX element's `<a.B />`
  the same. An access to `this` / `super` (Java, TS), `self` / `cls` (Python) or `self` (Rust) is none, as a bare
  `x` (Java's implicit `this`) is none; `this.a.b` is 1. A qualified name in a type or an import is no access: the
  grammars parse those as other kinds (`scoped_identifier`, `nested_type_identifier`, `dotted_name`), and a Python
  annotation's, which is an expression, is exempt. Both are the language's `exempt` hook.
- `jump`: `break`, `continue`, `throw`, and an early return: a return that isn't the last statement (comments aside)
  of its function's or lambda's body; the last one is a `statement`. So `if (x) return a; return b;` is a jump and a
  statement, and both returns of an `if / else` at the end are jumps.
- Each function's weight is kept apart, its nested functions' not in it; code outside any function belongs to its
  class, or else to its file. A declaration's own weight (`function`, `class`) is its own entry's; a lambda is not a
  function, its code belongs to the function around it.
- A docstring or a bare `...` (Python), a directive like `"use strict";` (TS) is no statement: an expression
  statement that is only a string (or Python's `...`) weighs nothing (the language's `exempt` hook).
- Names: a function or class is named by the classes and functions around it, `Outer.Inner.method`,
  `Outer.method.Local.run`. A Java constructor is `Class.Class`, an anonymous class `new Type` (its type arguments
  dropped: `Outer.method.new Runnable.run`), an enum constant's body the constant's name. Same-named entries of one
  kind in a file are one entry, their weights summed, its line the first one's: overloads, a class's constructors,
  two anonymous classes of a type in one method. The name is then a stable key for `commit` (an added overload is a
  heavier function, not a new one), and (path, kind, name) is unique, so the sort is total. A class and a method of
  the same name stay apart.
- A weight is an `f64`; the text report shows at most two decimals.

### TypeScript and TSX

- A function is a function, generator or method declaration, an overload's or an interface's signature
  (`function f(): void;`, `m(): void;`, `abstract m(): void;`): overloads are one entry, summed, as in Java. A
  constructor is `Class.constructor`, a getter and its setter one entry. A class, an interface, an enum and a
  namespace (`namespace N`, `declare module "m"`) are classes; a class expression is named by its own name, else the
  variable it is bound to, else `class`.
- An arrow or a function expression is a lambda, but where it is bound to a name at the file's level (or a
  namespace's) or as a class field's value: `export const f = () => ...` is the function `f`, a field
  `onClick = () => ...` the function `A.onClick`, weighed as a function and nesting as one (a top-level one's body is
  not deeper). Inside a function, `const cb = () => ...` stays a lambda in it, as Java's `Runnable r = () -> ...`
  does: a handler is part of its component. So `function inner() {}` in a function is a nested function of its own
  but `const inner = () => {}` is not. Only a plain identifier names one (not a destructuring); a wrapped value
  (`memo(() => ...)`, `(() => ...) as F`) and `export default () => ...` are lambdas.
- `if` nests its consequence; its `else` is a clause beside it, whose block is deeper but whose `if` (`else if`)
  stays at the depth. `switch` cases as in Java. `for`, `for ... in / of`, `while`, `do` are loops.
- `bool_op` counts runs of `&&`, `||` and `??` (a run of `??` is one, as of `&&`), `&&=`, `||=` and `??=` too.
- `operator` counts `===` / `!==`, `in` and `instanceof` as comparisons, `typeof`, `void` and `delete` as unary
  operators. `access` counts a member (`a.b`, `a?.b`) and a subscript (`a[i]`).
- A decorator weighs nothing of its own, as a Java annotation; a call in it (`@Input()`, `@Component({...})`) is a
  call, where the grammar puts it: a method's or a field's decorator in the class, a parameter's in the method,
  a class's in the class, but `@d export class` in the file (the export holds it).
- TSX: a JSX element (`jsx_element`, a fragment too, and `jsx_self_closing_element`) is a call, as it compiles to
  one; its attributes and `{...}` weigh what is in them. JSX is no nesting construct.
- A namespace's statement (the grammar wraps `namespace N {}` in an expression statement) weighs nothing.

### Python

- A `def` is a function, a method of the class around it, a nested function of the function around it (its body
  nests); a lambda is a lambda, even bound to a name. A class is `class`.
- `if` nests its body; `elif` (a branch) and `else` are its children beside the body, so a chain is siblings, each
  one's body deeper. `x if c else y` is a branch, its value and alternative deeper, its condition not; a chained one
  (`a if c else b if d else e`) is a sibling. The grammar gives that expression no fields, so its children are
  matched by their place (`Children::Nth`).
- `for` and `while` are loops, their `else` beside the body, its block deeper. `try` and `with` are `try`, each
  `except` a catch.
- A comprehension (list, set, dict, generator): each `for` is a loop, each `if` a branch; its element and its `if`s
  are one deeper, its `for`s (their iterables) at its depth.
- `match` is no nesting construct; each `case` is a branch, its body deeper, its guard (`case x if c`) no branch of
  its own (exempt), its condition at the case's depth.
- `and` / `or` are the logical operators (`not` starts a new run: it is a unary `operator`). `raise` is a jump.
- `access` counts an attribute (`a.b`) and a subscript (`a[i]`), not one in an annotation (`x: os.PathLike`,
  `Dict[str, int]`), which is a type; a parameter's default is no annotation.
- A decorator weighs nothing of its own; a call in it (`@app.route("/")`) is a call of the class or file around the
  decorated definition (the grammar puts the decorators beside the definition, not in it).

### Rust

- A `fn` is a function: a method of the class around it, a nested function of the function around it (its body
  nests), a trait's signature (`fn a(&self);`) a function as TS's. A struct, an enum, a union, a trait and a module
  with a body are classes; `mod a;` (only where a file is) weighs nothing and is no class.
- An `impl` block (`impl Foo`, `impl<T> Trait for Foo<T>`, `impl Trait for &mut a::Foo`) is a class named by its type
  (its type arguments, references and path dropped), so its methods are `Foo.method`, and a type's struct and every
  `impl` of it in a file are one class (same-named entries of one kind are one). An `impl` weighs nothing of its
  own: the type's declaration does, so a struct with three `impl` blocks weighs one class, and an `impl` of a type
  declared elsewhere only its contents. A type that is no path (`impl T for [u8]`) is named by its text.
- A closure is a lambda, even bound to a name (`let f = |x| ...`), its body deeper; an expression body (`|x| x + 1`)
  is no statement, as a Java lambda's.
- `if` (`if let` too) nests its consequence; its `else` is a clause beside it, as TS's: its block deeper, its `if`
  (`else if`) at the depth. `match` is no nesting construct; each arm is a branch, its pattern and guard at the arm's
  depth, its value deeper. `for`, `while` (`while let`), `loop` are loops; a label (`'outer:`) weighs nothing.
- `&&` / `||` are the logical operators; a let chain (`if let Some(x) = a && x > 0`) is a run of `&&`. `-x` and
  `!x` are unary operators; `*x` and `&x` are none. `access` counts a field (`a.b`, `a.0`) and an index (`a[i]`); a
  method call's callee (`a.f()`) is the call's, a path (`a::b`) no access.
- A call and a macro invocation (`println!(...)`, `vec![...]`) are calls. `break`, `continue`, `?` (an early return
  of the error) and an early `return` are jumps.
- Statements: an expression statement, `let`, `const`, `static`, a `type` alias, a trait's associated type, a
  struct's field and an enum's variant (as Java's fields and enum constants). `use` and `extern crate` weigh nothing,
  as an import.
- Rust's blocks are expressions, so the grammar wraps a statement around any expression in a block's statement
  place: `g();`, but also an `if`, a `match`, a loop, a block, `return x;`, `break;`. That wrapper weighs nothing
  around a block, an expression ending in one or a jump (the language's `exempt`): an `if` is a branch, `return x;`
  a statement (or a jump), `break;` a jump, as in Java. The early return looks through it (the language's
  `wrappers`): `return x;` as the last statement of its body is the statement wrapping it, so it is no jump.
- A block's tail expression (its value, without `;`) is a statement (the language's `tail` hook, which weighs the
  block a statement beside its own category, at the block's depth, the tail's too), so `fn f() -> i32 { a + b }`
  weighs as Java's `return a + b;`, `{ g() }` as `g();`. A tail jump (`return x`, `break`) weighs as itself, not
  twice. The same holds in any block: an `if`'s, an arm's, a closure's, `unsafe { }`'s.
- Known gaps: a macro's arguments are a token tree, not parsed, so only the macro call weighs (what is inside
  `vec![f(x), g(y)]` or `assert_eq!(...)` weighs nothing), and a `macro_rules!` definition weighs nothing.
  `let ... else { }` is no branch, its block not deeper; an `async`
  block is no lambda.
- Tests inside a source file (the `tests` hook): an item marked `#[cfg(test)]`, `#[test]` or `#[...::test]`
  (`tokio::test`), the attribute among the attributes and comments right before it, is test code with all in it
  (a `mod tests`, its functions); a file marked `#![cfg(test)]` is all test code. Its entries are test entries
  (a test and a non-test one of one name are two, so `#[cfg(test)] fn f` and `#[cfg(not(test))] fn f` stay
  apart), its code outside any class the file's test code; `scan` leaves it out, `scanTests` weighs it alone,
  `commit` weighs it times `test_factor` as a test file. Left as it is: a marked item that is no class or function
  inside a class or function (`#[cfg(test)] const` in an `impl`) weighs in its class's or function's code; a
  `mod tests;` whose file is `tests.rs` marks that file only in its parent, so `tests.rs` is code unless a test
  glob or its own `#![cfg(test)]` says otherwise.

## 4. Architecture

| Module | What it does |
|---|---|
| `cli` | the arguments (`clap`), the commands |
| `config` | loads the weights file, merges it over the defaults, validates it |
| `source` | an input as `(path, language, text)` items: from disk (`ignore`, the excludes) or git blobs at a revision |
| `lang` | a module per language (Java, TS, TSX, Python, Rust): grammar, extensions, node kind → category, nesting kinds, function names |
| `engine` | one tree walked: each node's weight, summed by function, class and file |
| `git` | the files a commit or a range changed (`git diff-tree -r -M`), their text before and after (`git cat-file --batch`) |
| `report` | the text table and the JSON; for `commit`, the deltas |

Files are weighed in parallel (`rayon`).

Built: every module. The dependencies: `tree-sitter`, `tree-sitter-java`, `tree-sitter-typescript`,
`tree-sitter-python`, `tree-sitter-rust`, `clap`, `serde`, `serde_json` (the JSON report), `toml`, `anyhow`, `ignore`, `globset` (the
`[files]` globs; `ignore` is built on it, so it costs no build), `rayon`, and `tempfile` for the tests. `git` is run
as a command, no `git2` / `gix`.

- `source` (built): a file given is weighed as is, though an exclude matches it; a folder is walked by `ignore`, its
  `.gitignore`s (and `.ignore`s, the global gitignore) honoured, hidden files and folders left out, each folder's
  entries in the order of their names, so the walk is the same on every run. A file the excludes match is left
  out, unlisted; a folder whose every path is excluded (an exclude ending in `/**`) isn't walked at all. A file in no
  language codeweight reads is skipped and listed. The globs (`*` and `?` within a path's part, `**` across parts)
  are matched against a file's path from the root of the git repo it is in (the root `config::find` uses), so one
  glob means the same from any folder scanned, and later for `commit`; outside a repo, from the current folder when
  the path is in it, else from the folder given (a file given: its name), never with the folders above, so a project
  under a `build` or `tests` folder is neither excluded nor taken for tests whatever the path's spelling. A
  file the test globs match is a test file: `scan` leaves it out, `scanTests` keeps it and the files of a language
  with tests inside its source files (Rust's), each weighed for its tests alone. A path is shown as given,
  or the folder given joined with the path in it, without a leading `./`; a path given twice, or under two folders
  given, is weighed once.
- `source` for `commit` (`from_commit`): a changed file's each side is kept as a walk of the repo's root would keep
  it: its path (from the root, as `git diff-tree` gives it) not hidden (no part starting with `.`), not excluded, in
  a language codeweight reads, not a test at `test_factor` 0; a side that isn't a regular file (a symlink, a
  submodule) is none. A side's language is its own path's, so a rename `a.js -> a.ts` is an added file. A change with
  no side kept is skipped and listed when a side was in no language codeweight reads, else left out unlisted. Then
  the kept sides' blobs, each once, all through one `git cat-file --batch`; all of a commit's texts are in memory
  at once (a commit's, or a range's, changed files only), then weighed in parallel.
- The files are listed first, then read and weighed in parallel (`rayon`), each read when weighed so the texts are
  never all in memory; the results are collected in the listed order, and the report sorts them (weight, then path
  and name), so the output doesn't depend on which thread finished first.

- `lang::Lang` is a table of plain data and five hooks: its name and its weights file key, the extensions, the
  grammar, the node kind → category list, the nesting constructs (each with its body: the children it nests, by
  field name or node kind, either listed or all but those listed, or by their place among the named children; and
  its chain, the children of its own kind that stay at its depth, for `else if`), the return, lambda and parenthesis
  kinds, the wrappers (the statement kinds a return is looked through for its place: Rust's expression statement),
  `operator` (a node's operator, by its run: `&&`, `+` / `-`, a comparison, a unary one, ...; a logical one is a
  `bool_op`, any other an `operator`), `exempt` (a node that weighs nothing though its kind has a category, where
  the kind can't tell: a docstring, a case's guard, a call's callee, `this.x`), `tail` (a node that weighs a statement beside
  its kind's category: Rust's block ending in a value), `scope` (the function or class a node
  declares, and its own name) and `tests` (a node that is test code inside a source file, it and all in it:
  Rust's `#[cfg(test)]` items; none for a language whose tests are files of their own). A lambda `scope` names a function weighs and nests as a function (TS's named
  arrows). A test checks that
  every kind and field the tables name is in the grammar, so a typo can't silently weigh nothing.
- `engine::Weigher` turns a language's tables and its weights into arrays by node kind id once, then walks each
  tree with an explicit stack (a long `a + b + c + ...` chain is a deep tree, too deep for recursion); a node's
  weight is added to its entry's sum, so a file's sums are in source order and deterministic.
- The engine's tests are snippets weighed under `tests/weights.toml` (round numbers, complete on its own, the
  defaults never under it), each sum worked out by hand in the test. The CLI's tests run the binary in temp folders.

## 5. The commit's net delta

Built (`git`, `source::from_commit`, `report`):

1. The two sides (`git::resolve`): a revision is `git rev-parse --verify <rev>^{commit}`, compared against its first
   parent (a merge too, so it shows what the merge brought in), the root commit against the empty tree
   (`git diff-tree --root`). The parent is the commit object's first `parent` line (`git cat-file commit`), not
   `git rev-list --parents`: that follows a shallow clone's grafts, so the commit at its boundary (CI's depth-1
   checkout) would look like a root and its whole tree added. A parent missing from the repo is an error that says
   to fetch more history. `A..B` is the two revisions, an empty side `HEAD` as in git;
   `A...B` (against the merge base) is an error, not silently read as `A..B`. A revision starting with `-` is an
   error, so it can't pass for an option.
2. The changed files: `git diff-tree -r -M -z --no-commit-id <from> <to>` (renames detected, so a moved file is not a
   deletion and an addition; `-z`, so no path is quoted). Git's own rename rules decide: two files of the same text,
   one deleted and one added, are a rename.
3. Their text on both sides, all through one `git cat-file --batch`, written from a thread of its own while the
   output is read, so neither pipe can fill and block both.
4. Per file `delta = weight(after) − weight(before)`: an added file all positive, a deleted one all negative; an
   unsupported one skipped and listed.
5. Per function and class, and the file's code outside them, matched by kind and qualified name (`Class.method`,
   `module.func`) within the file, a renamed file's old and new: added, removed, heavier, lighter. A function moved
   to another file is removed from one and added to the other.
6. The report: the total delta first (`+42.5`, `-17`), then the files' or the functions' deltas (section 1).

The `commit` tests make throwaway repos (`tempfile`, git's global and system config off): an add, a delete, a
rename, a merge, the root commit, a range, the excludes and test factor, a bad revision, a shallow clone's
boundary.

## 6. Milestones

0. (built) The repo: `Cargo.toml`, `README.md` (usage, the weights file), the MIT license, GitHub Actions (`cargo fmt
   --check`, `clippy -D warnings`, `test`).
1. (built) The skeleton: the CLI, the config with its embedded defaults, `init`.
2. (built) The engine and Java, for `scan <file>`: snippet tests with weights worked out by hand.
3. (built) TypeScript, TSX and Python.
4. (built) Folders: the walk, the excludes, the test globs and factor, `rayon`.
5. (built) `commit`: tests on throwaway repos (an add, a delete, a rename, a merge, the root commit, a range).
6. (built) The reports: text, JSON, `--top`, `--by`.
7. (built) Tuning on real code: the default weights adjusted until the heaviest functions found are the hardest
   ones. Weighed on GraphHopper's and OptaPlanner's cores (Java), a React app (TS, TSX) and two Python tools. Under
   the first defaults (call 1, nesting factor 0.5) long flat methods ranked with the hardest algorithms: a config
   reader of a hundred `setX(config.getY("key", getX()))` lines (three calls each) weighed as much as a nested graph
   search. Now a call is 0.5 (a getter chain is long, not hard) and each level of nesting adds a whole base weight
   (factor 1.0): the flat readers fell out of the top, the nested searches and parsers stayed first. The other
   weights held. The test globs gained `__tests__`, `*.test.tsx` / `*.spec.tsx`, `*Tests.java` and `*_test.py`.
   Left as it is, not a weight's matter: a TS object literal's methods and arrows (a store of handlers) are lambdas
   of the file, so such a file's `(file)` line can be its heaviest.
8. (built) Rust (`.rs`, `tree-sitter-rust`): its table, the engine's look through a statement wrapping a return
   (`wrappers`) and the tail expression as a statement (`tail`); snippet tests. The defaults re-checked on a Rust
   repo, codeweight itself: the heaviest are the engine's tree walk (`Weigher.weigh`, a loop of matches and nested
   ifs), the weights file's checks (`Config.from_value`) and the two inputs' listings (`from_commit`,
   `from_paths`); a language's long `const` tables weigh a statement each, a struct its class and a statement a
   field, the one-line getters (`Config.lang`, `Weigher.lang`) least.
   The defaults held, no weight changed. Rust's integration tests are under `tests/`, which a test glob already
   matches; its unit tests are in the source files (the `tests` hook, section 3, finds them since).
9. (built) Operators and accesses: `a = b`, `a = b + c` and `a = (b + c) * d` weighed alike, as `obj.a` and
   `obj.a.b`. Now `operator` (runs of like operators, the `bool_op` rule; the `logical` hook became `operator`) and
   `access` (a field access or index), both 0.25 by default: below a call's 0.5, an expression's detail is not its
   control flow. Checked on GraphHopper's core and jsprit (Java) and codeweight itself: the totals grew 3 to 6%,
   the heaviest functions stayed first (their nesting still dominates), and the ones that grew most for their
   weight are the arithmetic and bit packing ones (`IntEncodedValueImpl.getInt` +42%, `ArrayUtil.merge` +37%). The
   engine's test weights set both 0, so the snippets about other things leave them out; the snippets about them
   are weighed with both set (`lang::testing::weigh_expressions`).
