# codeweight

A CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a lambda, a function, a
class, ...) has a weight, set in a weights file. An element nested in branches, loops, lambdas and nested functions
weighs more. A file's weight is the sum of its elements' weights, and so is a function's. Code is parsed by
tree-sitter.

It reads Java (`.java`), TypeScript (`.ts`, `.mts`, `.cts`), TSX (`.tsx`) and Python (`.py`, `.pyi`). Built so far:
`scan` of files and folders, the weights file, `init`. Not yet: `commit`, the JSON output. The plan is
`docs/design.md`.

## Usage

```
codeweight scan <path>...                 the weight of files and folders, absolute or relative; several allowed
codeweight commit <rev>|<A>..<B> [-C <repo>]   the net weight delta of a commit, or between two revisions (not built yet)
codeweight init [<file>]                  writes the default weights file (default ./codeweight.toml), never over an existing one
```

Every command takes:

- `--config <file>`: the weights file (see below for where it is looked up without one);
- `--format text|json`: the report's format, text by default (JSON is not built yet);
- `--top <N>`: only the report's N heaviest lines;
- `--by file|function`: what the report lists, the functions by default.

The text report starts with the total. After it come the functions and classes, heaviest first, then by path and
name, each as `<weight>  <path>  <name>`; with `--by file`, the files. A class's line (`Name (class)`) has its code
outside its functions; `(file)` is a file's code outside any class. A function is named by its enclosing classes and
functions, `Outer.Inner.method`. Same-named functions of one class are one line, their weights summed: overloads, or
a Java class's constructors (`A.A`). A Java anonymous class is `new Type`; a TypeScript constructor is
`A.constructor`. A TypeScript arrow or function expression bound to a name at the file's level or as a class field
(`export const f = () => ...`, `onClick = () => ...`) is a function of that name; anywhere else (inside a function
too) it is a lambda, its code the function's around it. A weight has at most two decimals.

A folder is walked for the files in it: its `.gitignore` is honoured, hidden files and the `[files]` excludes are
left out (a file named on the command line is weighed even if excluded). The globs are matched against a file's path
from the root of its git repo (outside a repo, from the current folder, or from the folder given when the path is
outside the current one). A test file (`test_globs`) weighs its
weight times `test_factor`; at 0 it is left out. Files are weighed in parallel; the output is the same on every run.

A file that doesn't parse cleanly is still weighed, as tree-sitter recovered it, with a warning. A file in a language
codeweight doesn't read is skipped and listed, never an error.

Exit codes: 0 done, 2 an error (a bad path or weights file).

## The weights file

TOML. The first one found wins: `--config`, then `./codeweight.toml`, then `codeweight.toml` at the root of the git
repo the first scanned path is in, then the built-in defaults. Your file is merged over the defaults, so it holds only
what it changes. A key, category, language or node kind codeweight doesn't know is an error, not ignored.
`codeweight init` writes out the defaults ([src/default.toml](src/default.toml)) to start from. The example below is
the defaults plus the two per-language tables a file of yours can add (the defaults have neither, so a `[weights]` key
you set reaches every language):

```toml
[nesting]
model = "linear"        # linear: 1 + factor * depth; exponential: factor ^ depth
factor = 0.5
max_depth = 10          # depth is capped here, so one deep method can't explode the total

[weights]               # the base weight of each category, for every language
statement = 1
call      = 1
branch    = 2
loop      = 3
try       = 2
catch     = 2
lambda    = 2
function  = 3
class     = 5
bool_op   = 1
jump      = 1

[languages.java.weights]            # yours only: a language's overrides of [weights]
lambda = 3

[languages.java.node_kinds]         # yours only: a tree-sitter node kind mapped to a category, or to "none"
synchronized_statement = "try"

[files]                 # what a folder walk weighs (globs: * and ? within a path's part, ** across parts)
exclude = ["**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/*.d.ts"]
test_globs = ["**/test/**", "**/tests/**", "**/*Test.java", "**/*.test.ts", "**/*.spec.ts", "**/test_*.py"]
test_factor = 1.0       # a test file's weight times this; 0 leaves the tests out
```

### The weight

```
weight(node) = weights[category(node)] × nest(min(depth, max_depth))
```

`depth` is the number of nesting constructs around a node: a branch, a loop, a `try` / `catch` / `finally`, a
`switch` case, a lambda, and a function or class declared inside a function. Only a construct's body is deeper. Its
head (a condition, a loop's header, a catch's parameter, a case's label) stays at the construct's own depth. Plain
tree levels don't count (`a.b().c()` is not deep). An `else if` (Python's `elif`) is at its `if`'s depth, and so
are the cases of a `switch` or a `match`.

The languages' keys are `java`, `typescript` (for `.ts` and `.tsx` alike: its `node_kinds` may name a kind of either
grammar, the `jsx_*` ones being TSX's) and `python`. A node kind is a tree-sitter one, as the language's grammar
names it.

### The categories

| Category | Java | TypeScript, TSX | Python |
|---|---|---|---|
| `statement` | an expression statement, a local variable, a field, a `return` that ends its function or lambda, `yield`, `assert`, `synchronized`, an enum constant | an expression statement, `const` / `let` / `var`, a class field, an interface's property, a `type` alias, a `return` that ends its function | an expression statement (an assignment too; not a docstring or a bare `...`), `assert`, `del`, a `type` alias, a `return` that ends its function |
| `call` | a method call, `new`, `this(...)` / `super(...)` | a call, `new`; in TSX each JSX element | a call |
| `branch` | `if`, a ternary `?:`, each `switch` case | `if`, a ternary `?:`, each `switch` case | `if`, `elif`, `x if c else y`, each `match` case, a comprehension's `if` |
| `loop` | `for`, for-each, `while`, `do` | `for`, `for ... in` / `of`, `while`, `do` | `for`, `while`, a comprehension's `for` |
| `try` | `try`, try-with-resources | `try` | `try`, `with` |
| `catch` | a `catch` clause | a `catch` clause | an `except` clause |
| `lambda` | a lambda | an arrow or function expression not bound to a name at the file's level or as a field | a `lambda` |
| `function` | a method or constructor | a function, method, signature, an arrow bound to a name at the file's level or as a field | a `def` |
| `class` | a class, interface, enum, record or annotation type | a class, interface, enum, namespace | a `class` |
| `bool_op` | each run of like `&&` / `\|\|` | each run of like `&&` / `\|\|` / `??` | each run of like `and` / `or` |
| `jump` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `raise`, an early `return` |

A run of `bool_op`: `a && b && c` is 1, `a && b || c` is 2; parentheses don't break a run, a negation or a call does.
An early `return` is one that isn't its function's or lambda's last statement. Annotations and decorators weigh
nothing of their own; a call in one (`@Input()`, `@app.route("/")`) is a call.

## Building

Rust 1.100 (pinned in `rust-toolchain.toml`, so rustup picks it): `cargo build --release`; checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`.

## License

MIT, see [LICENSE](LICENSE).
