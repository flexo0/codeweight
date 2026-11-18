# codeweight

A CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a lambda, a function, a
class, ...) has a weight, set in a weights file. An element nested in branches, loops, lambdas and nested functions
weighs more. A file's weight is the sum of its elements' weights, and so is a function's. Code is parsed by
tree-sitter.

Built so far: Java files (`scan <file.java>`), the weights file, `init`. Not yet: TypeScript and Python, folders,
`commit`, the JSON output. The plan is `docs/design.md`.

## Usage

```
codeweight scan <path>...                 the weight of files, absolute or relative; several allowed
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
functions, `Outer.Inner.method`. Same-named functions of one class are one line, their weights summed: Java overloads,
or a class's constructors (`A.A`). An anonymous class is `new Type`. A weight has at most two decimals.

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

[files]                 # for folder walks (not built yet)
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
tree levels don't count (`a.b().c()` is not deep). An `else if` is at its `if`'s depth, and so are the cases of a
`switch`.

### The categories

| Category | What it is (Java) |
|---|---|
| `statement` | a statement: an expression statement, a local variable, a field, a `return` that ends its function or lambda, `yield`, `assert`, `synchronized`, an enum constant |
| `call` | a method call, `new`, `this(...)` / `super(...)` |
| `branch` | `if`, a ternary `?:`, each `switch` case (`case` / `default`, either style) |
| `loop` | `for`, for-each, `while`, `do` |
| `try` | `try`, try-with-resources |
| `catch` | a `catch` clause |
| `lambda` | a lambda |
| `function` | a method or constructor declaration |
| `class` | a class, interface, enum, record or annotation type declaration |
| `bool_op` | each run of like `&&` / `\|\|` operators: `a && b && c` is 1, `a && b \|\| c` is 2; parentheses don't break a run, a `!` or a call does |
| `jump` | `break`, `continue`, `throw`, an early `return` (one that isn't its function's or lambda's last statement) |

## Building

Rust stable: `cargo build --release`; checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`.

## License

MIT, see [LICENSE](LICENSE).
