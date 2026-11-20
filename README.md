# codeweight

A CLI that weighs code. Every element of a file (a statement, a call, a branch, a loop, a lambda, a function, a
class, ...) has a weight, set in a weights file. An element nested in branches, loops, lambdas and nested functions
weighs more. A file's weight is the sum of its elements' weights, and so is a function's. Code is parsed by
tree-sitter.

It reads Java (`.java`), TypeScript (`.ts`, `.mts`, `.cts`), TSX (`.tsx`), JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`),
Python (`.py`, `.pyi`), Rust (`.rs`) and Go (`.go`). It weighs docs too, a number of their own: a Markdown file's
(`.md`, `.markdown`) docs weight is its words and symbol runs. The design is
in `docs/`, a file per subject (see `docs/README.md`).

## Example

codeweight run on its own sources, and on what a commit added or took away: its code, its docs and its tests,
three numbers, never one sum:

```
$ codeweight scan src
code +9519.02
docs 0
tests +3036.49
$ codeweight commit b88842c
code +1095.35
docs +2240
tests +534.04
$ codeweight commit b88842c --short
c+1095 d+2240 t+534
```

`--code` (or `--tests`, `--docs`) prints that number alone, and with `-v` the functions behind it, heaviest first
(see "Usage").

## Install

With Rust 1.100 or later:

```
cargo install --git https://github.com/flexo0/codeweight
```

It isn't on crates.io yet; once it is, `cargo install codeweight`. To build from a clone, see "Building" below.

For a coding agent that reads `SKILL.md` skills (Claude Code and others), `skills/codeweight/` tells it how to run
codeweight: link or copy the folder into the agent's skills, e.g. from a clone
`ln -s "$PWD/skills/codeweight" ~/.claude/skills/codeweight`.

## Usage

```
codeweight scan <path>... [<one>|--short] [--json]           the weight of files and folders; several allowed
codeweight commit <rev>|<A>..<B> [-C <repo>] [<one>|--short] [--json]
                                                             the net weight delta of a commit or between two
                                                             revisions
codeweight pre-commit [-C <repo>] [--staged] [<one>|--short] [--json]
                                                             the net weight delta of the uncommitted changes (or
                                                             the staged ones alone)
codeweight init [<file>|-]     writes the default weights file (default ./codeweight.toml), never over an existing
                               one; `-` prints it to stdout

<one>: --code|--tests|--docs [-v] [--top <N>] [--by file|function]
```

`scan`, `commit` and `pre-commit` take `--config <file>`: the weights file (see below for where it is looked up
without one); `init` doesn't, it reads none. They take too:

- `--code`, `--tests` or `--docs` (at most one): that number alone, and its report;
- `--verbose` (`-v`): the whole report instead of the total alone;
- `--top <N>`: only the report's first N lines (the heaviest; for a delta, the largest changes);
- `--by file|function`: what the report lists, the functions by default;
- `--short`: the three numbers on one line;
- `--json`: JSON instead of text (described below).

`-v`, `--top` and `--by` need one of `--code`, `--tests` and `--docs`, `--short` none of them, and `--short` and
`--json` don't go together: each is a usage error (exit 2) otherwise.

Without `--code`, `--tests` or `--docs`, a command prints three numbers, each on its line, in the order code, docs,
tests: `code +15.5`, `docs +17`, `tests -5`. Each is signed (`scan`'s too), with at most two decimals, a zero
without a sign (`docs 0`). `--short` prints them on one line, each rounded to a whole number, all three always
there: `c+15 d+17 t-5`, a zero part `d0`. The code's is the weight of the code, its tests left out. The tests' is
the weight of the tests alone: the test files (`test_globs`), and in a Rust file the items marked `#[cfg(test)]`,
`#[test]` or a runner's `#[tokio::test]` (any `#[...::test]`), all in them (a `mod tests` and its functions), and a
whole file marked `#![cfg(test)]`. The docs' is the weight of the Markdown files (`docs_globs`), each by its words
and symbol runs (`[docs]`, below; a word 1 and a symbol run 1 by default, so `## Setup` is 2 and `foo(a, b);` 6).
The code, the tests and the docs are always three numbers, never one sum, each at its full weight: a change to all
three has its code in one, its tests in another and its docs in the third. The input is read once and each file
weighed once, a Rust file with tests counting toward the code and the tests both.

With `--code`, `--tests` or `--docs`, a command prints that number alone: `scan` its total (`4698.25`), `commit` and
`pre-commit` their net delta, signed (`+42.5`, `-17`, `0`). `--verbose` (`-v`) prints its whole report instead, as
`--top` and `--by` do. With the numbers alone (the three, `--short`, or one total), a file's syntax errors and a
folder that can't be read are a warning on stderr.

The text report (`--code -v`, or `--tests`, `--docs`) starts with the total. After it come the functions and classes, heaviest first, then by path and
name, each as `<weight>  <path>  <name>`; with `--by file`, the files. A class's line (`Name (class)`) has its code
outside its functions; `(file)` is a file's code outside any class. A function is named by its enclosing classes and
functions, `Outer.Inner.method`. Same-named functions of one class are one line, their weights summed: overloads, or
a Java class's constructors (`A.A`). A Java anonymous class is `new Type`; a TypeScript constructor is
`A.constructor`. A TypeScript (or JavaScript) arrow or function expression bound to a name at the file's level or as
a class field (`export const f = () => ...`, `onClick = () => ...`) is a function of that name; anywhere else (inside
a function too) it is a lambda, its code the function's around it. A Rust method is named by its `impl` block's type
(`Foo.method`), so a type's struct and all its `impl` blocks in a file are one class; a Go method by its receiver's
type (`T.M` for `func (r *T) M()`). A weight has at most two decimals.

A folder is walked for the files in it: its `.gitignore` is honoured, hidden files and the `[files]` excludes are
left out (a file named on the command line is weighed even if excluded). The globs are matched against a file's path
from the root of its git repo (outside a repo, from the current folder, or from the folder given when the path is
outside the current one). The code leaves the test files (`test_globs`) and a Rust file's tests out, the tests
are only them. A file the docs globs match (`docs_globs`) is docs alone, though it is under `tests/`: neither the
code nor the tests weigh it, the docs only such files, each one line of `--docs`' report (its `(file)`, as it has no
functions). Files are weighed in parallel; the output is the same on every run.

A file that doesn't parse cleanly is still weighed, as tree-sitter recovered it, with a warning. A file in a language
codeweight doesn't read is left out, never an error. A folder under the one scanned that can't be read (a
root-owned docker volume, say) is left out with a warning and the rest weighed; a folder given that can't be read is
an error.

Exit codes: 0 done, 1 an error (a bad path, revision or weights file, or not in a git repo), 2 a usage error (an
unknown command or option, options that don't go together). A reader that quits early (`codeweight scan -v . | head`)
ends it quietly, exit 0.

### `commit`, `pre-commit`

`codeweight commit <rev>` weighs what a commit changed: its files before and after, against its first parent (a
merge's too), the root commit against the empty tree; in a shallow clone, a commit whose parent wasn't fetched is
an error (fetch more history), not a root. `codeweight commit A..B` compares the two revisions' trees
directly, so a change made and undone between them nets to zero (an empty side is `HEAD`; `A...B` is an error).
`<rev>` is any git revision (a hash, `HEAD~2`, a tag); `-C <path>` is any path inside the repo (default `.`). It runs
`git` (`git diff-tree -r -M`, so a renamed file is one file, and one `git cat-file --batch` for the texts).

`codeweight pre-commit` weighs the uncommitted changes, before you commit them: `HEAD` against the
working tree, staged or not, the untracked files too and the ignored ones left out, as `git add -A && git commit`
would commit them (in a repo with no commit yet, against the empty tree). With `--staged`, only the staged
changes: `HEAD` against the index, as `git commit` would commit them, the unstaged and untracked ones left out. Your
index is left as it is: the working tree is added to a copy of it. git writes the changed files' blobs to the repo's objects, which nothing refers to,
and `git gc` drops them later.

A file's delta is its weight after minus before: an added file's is all positive, a deleted one's all negative. Its
functions and classes are matched by their qualified name within the file (a renamed file's old and new), each one
`added`, `removed`, `heavier` or `lighter`. The changed files are weighed as a walk of the repo's root weighs them:
the `[files]` excludes and hidden paths are left out, and so is a file codeweight doesn't read. Their code, docs
and tests are weighed as `scan` weighs them, with `--code`, `--tests` or `--docs` as `scan` does with it.

With `--code -v` (or `--tests`, `--docs`), the text report starts with the total delta (`total +42.5`, `total -17`). After it comes one line per function and
class (or per file with `--by file`), `<delta>  <status>  <path>  <name>`, a renamed file's path as `old -> new`; the
largest changes first, either way (a gain before a loss of the same size), then by path and name. A line whose delta
is 0 (an untouched function, a pure rename) is left out; `--top` keeps the first N lines.

### The JSON

Without `--code`, `--tests` or `--docs`, `--json` prints one object, the three numbers and the warnings:

- `code`, `docs`, `tests`: the three numbers (for `commit` and `pre-commit`, the deltas), each rounded to two
  decimals;
- `warnings`: as a report's, below;
- for `commit` and `pre-commit`, also `from` and `to`, as a report's, below.

```json
{
  "from": "60baba96b16d35eb98e1c87993574780995f624c",
  "to": "b48be68e77d42c8c9a8a28f2060d0bbe261779d4",
  "code": 6.0,
  "docs": 17.0,
  "tests": -5.0,
  "warnings": []
}
```

With `--code`, `--tests` or `--docs`, `--json` prints that number's report, one JSON object with the same lines as
the text, in the same order, `--top` and `--by` acting alike; the whole report, `--verbose` or not. Every field
below is always there, `null` where it has no value. A weight or a delta is a number rounded to two decimals, as the
text shows it.

- `total`: the whole weight (or delta), not only the rows shown.
- `by`: `"function"` or `"file"`.
- `rows`: the report's lines. By file, `{path, weight}`; by function, `{path, kind, name, line, weight}`, `kind`
  one of `"function"`, `"class"` (its code outside its functions) and `"file"` (a file's code outside any class, its
  `name` and `line` `null`), `name` qualified (`Outer.Inner.method`), `line` the first line, from 1 (for `commit`, the after
side's, a removed one's the before side's).
- `warnings`: the files with syntax errors, `{path, kind: "syntax_errors", side}`, `side` `null` for a scan (a
  Markdown file never has one); then, for a scan, the paths under a folder given that couldn't be read and were left out,
  `{path, kind: "unreadable", side: null}`.

For `commit`, also `from` (the full hash compared from, `null` for the root commit's empty tree) and `to` (the full
hash compared to, `null` for `pre-commit`'s uncommitted changes); each row has, instead of `weight`, `before`, `after` (0 on the
side it isn't), `delta` and `status` (`"added"`, `"removed"`, `"heavier"` or `"lighter"`), and `old_path` (the path
before a rename, else `null`); a warning's `side` is `"before"` or `"after"`.

```json
{
  "from": "60baba96b16d35eb98e1c87993574780995f624c",
  "to": "b48be68e77d42c8c9a8a28f2060d0bbe261779d4",
  "total": 6.0,
  "by": "function",
  "rows": [
    {"path": "a.py", "old_path": null, "kind": "function", "name": "f", "line": 1,
     "before": 4.5, "after": 10.5, "delta": 6.0, "status": "heavier"}
  ],
  "warnings": []
}
```

## The weights file

TOML. The first one found wins: `--config`, then `./codeweight.toml`, then `codeweight.toml` at the root of the git
repo the first scanned path is in (for `commit`, the repo's), then the built-in defaults. Your file is merged over the defaults, so it holds only
what it changes. A key, category, language or node kind codeweight doesn't know is an error, not ignored.
`codeweight init` writes out the defaults ([src/default.toml](src/default.toml)) to start from. The example below is
the defaults plus the two per-language tables a file of yours can add (the defaults have neither, so a `[weights]` key
you set reaches every language):

```toml
[nesting]
model = "linear"        # linear: 1 + factor * depth; exponential: factor ^ depth
factor = 1.5            # each level of nesting adds one and a half base weights
max_depth = 10          # depth is capped here, so one deep method can't explode the total
# weigh the same at any depth: a statement's or a declaration's parts, which it already weighs the depth of
flat = ["call", "macro", "constructor", "argument", "bool_op", "operator", "access", "parameter"]

[parameters]            # the parameter at place k (from 0) weighs parameter * min(1 + step * k, max)
step = 0.3              # 1.65, 2.15, 2.64, 3.14, then 3.3 each: a long list weighs more than its count
max = 2.0

[accesses]              # an access at place k (from 0) in a row weighs access * min(1 + step * k, max)
step = 1
max = 3.0

[conditions]            # rules that weigh what a node's kind alone doesn't tell, as an existing category
casts = true            # a cast or type assertion (x as T, (T) x, x!, <T>x, x.(T)) is an operator
let_patterns = true     # Rust's `if let` / `while let` pattern is an operator, as a comparison is
magic_numbers = true    # a number literal but 0 and 1 is an access, as the named constant in its place would be
nullish_operator = false  # ?? and ??= are an operator, not a bool_op: a default value, no branch
nested_ternaries = true   # a ternary in a ternary's alternative (a ? x : b ? y : z) nests, no else-if sibling
free_default = true       # a switch's or match's default case (default, a bare _) weighs nothing, as an else
assignments = true        # a plain = is an operator, as += is; a declaration's initializer (int x = 1) isn't

[weights]               # the base weight of each category, for every language
statement = 1
call      = 0.49
macro     = 1.6
constructor = 0.949
argument  = 0.316
branch    = 3
case      = 2.4
loop      = 3
try       = 0.95
catch     = 2
lambda    = 1.65
function  = 1.55
class     = 5
bool_op   = 0.544
jump      = 1.35
operator  = 0.556
access    = 0.5
parameter = 1.65

[languages.java.weights]            # yours only: a language's overrides of [weights]
lambda = 3

[languages.java.node_kinds]         # yours only: a tree-sitter node kind mapped to a category, or to "none"
synchronized_statement = "try"

[docs]                  # a Markdown file's docs weight: words * word + symbol runs * symbol
word = 1                # a run of letters and digits; a URL is one word
symbol = 1              # a run of anything else that isn't a space, once however long: `**`, `);`, `##` are one each

[files]                 # what a folder walk weighs (globs: * and ? within a path's part, ** across parts)
exclude = [
    "**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/vendor/**", "**/*.d.ts",
    "**/*.min.js", "**/*.pb.go", "**/zz_generated*.go",
]
test_globs = [
    "**/test/**", "**/tests/**", "**/__tests__/**", "**/*Test.java", "**/*Tests.java",
    "**/*.test.ts", "**/*.spec.ts", "**/*.test.tsx", "**/*.spec.tsx", "**/test_*.py", "**/*_test.py",
    "**/*.test.js", "**/*.spec.js", "**/*.test.jsx", "**/*.spec.jsx", "**/*_test.go",
]
docs_globs = ["**/*.md", "**/*.markdown"]   # docs alone, weighed by [docs], though a test glob matches them too
```

The defaults were first tuned on real Java, TypeScript and Python code (and re-checked on Rust) so that the heaviest
functions are the hardest ones, then on 8,971 example pairs, a bad snippet and its fix from the linters' tests, so
that the fix weighs less, and again on those pairs thinned to 1,453, each rule weighing alike, by a search without a
grid. A call weighs about half a statement (a long chain of
getters or a flat run of setters is long, not hard), and each level of nesting adds one and a half base weights (`factor = 1.5`), so nested control flow outweighs flat code. Nesting multiplies the
structure only (statements, branches, loops, jumps, lambdas, ...): a statement's parts, its calls, operators,
accesses and `&&` / `||` (`[nesting] flat`), weigh the same at any depth, since their statement already weighs its
depth; a long statement in a loop is long, not harder for the loop. A branch weighs 3, a loop 3, a jump 1.35 (an
early return or a `break` is a path to follow). An operator weighs 0.556, a `&&` / `||` 0.544. A field access or an index weighs 0.5, 1, then 1.5 by
its place in a row (`[accesses]`): `a.b` 0.5, `a.b.c.d` 0.5 + 1 + 1.5, so reaching deep through objects weighs more
than its count. A parameter weighs 1.65 to
3.3 by its place (each next 0.3 times more, from the fifth on 3.3): three weigh 6.4, seven 19.5, so a long list
weighs more than its count. A function's own weight is 1.55, below a lambda's 1.65: a getter weighs 2.55, its
signature not most of it, and a class of getters and setters stays below one of logic. A `try` weighs 0.95 (its
body's nesting is its weight), a `catch` 2 (the other path, as a branch). A `switch` / `match` case weighs 2.4, below
an `if`'s 3, and its default nothing, as an `else`: a switch weighs less than the if-chain it replaces. A ternary
chained in a ternary's alternative (`a ? x : b ? y : z`) nests, where an `else if` doesn't. A call weighs 0.49 and each of its
arguments 0.316 besides what is in it: `f(a, b)` is 1.1. A Rust macro (`println!(...)`) weighs 1.6, about three calls: what is in it isn't parsed,
so it weighs for its content. A constructor (`new Foo()`, `new int[3]`, a struct literal) weighs 0.949, above
a call: it makes a new object, where a call asks one. The `[conditions]` are rules that weigh what a node's kind alone doesn't tell (a cast, Rust's
`if let` pattern, a magic number, `!!x`, `??`, a nested ternary, a default case), each as an existing category; the
defaults turn on all but `??` as an operator, and a weights file on its own that leaves the table out weighs as the categories alone.

### The weight

```
weight(node) = weights[category(node)] × nest(min(depth, max_depth))    nest = 1 for a category of [nesting] flat
```

`depth` is the number of nesting constructs around a node: a branch, a loop, a `try` / `catch` / `finally`, a
`switch` case, a lambda, and a function or class declared inside a function. Only a construct's body is deeper. Its
head (a condition, a loop's header, a catch's parameter, a case's label) stays at the construct's own depth. Plain
tree levels don't count (`a.b().c()` is not deep). An `else if` (Python's `elif`) is at its `if`'s depth, and so
are the cases of a `switch` or a `match`.

The languages' keys are `java`, `typescript` (for `.ts` and `.tsx` alike: its `node_kinds` may name a kind of either
grammar, the `jsx_*` ones being TSX's), `javascript` (its own, not `typescript`'s), `python`, `rust` and `go`. A
node kind is a tree-sitter one, as the language's grammar names it.

### The categories

| Category | Java | TypeScript, TSX, JavaScript | Python | Rust | Go |
|---|---|---|---|---|---|
| `statement` | an expression statement, a local variable, a field, a `return` that ends its function or lambda, a lambda's expression body (`x -> f(x)`, as `{ return f(x); }`), `yield`, `assert`, `synchronized`, an enum constant | an expression statement, `const` / `let` / `var`, a class field, an interface's property, a `type` alias, a `return` that ends its function, an arrow's expression body (`x => f(x)`) | an expression statement (an assignment too; not a docstring or a bare `...`), `assert`, `del`, a `type` alias, a `return` that ends its function, a lambda's body, a comprehension's element | an expression statement, `let`, `const`, `static`, a `type` alias, a field, an enum variant, a block's tail expression (its value, no `;`), a closure's expression body, a `match` arm's value that is no block, a `return` that ends its function or closure | an expression statement, an assignment, `:=`, each `var` / `const` spec, a send, `defer`, `go`, a type alias, a field, `i++` / `x += 1` (an `operator` too; not as a `for` clause's update), a `return` that ends its function or literal |
| `call` | a method call | a call; in TSX and JavaScript each JSX element | a call (a class's `Foo()` too: no syntax tells it from a function's) | a call (a tuple struct's `P(1)` too) | a call (`new(T)`, `make(...)` too) |
| `constructor` | `new Foo()`, an array's `new int[3]` / `new T[] {...}` (not a bare `{...}` initializer), `this(...)` / `super(...)` | `new Foo()` | — | a struct literal (`P { x: 1 }`) | a composite literal (`P{X: 1}`, `[]int{1}`, `map[K]V{}`; not an element's own `{...}`) |
| `macro` | — | — | — | a macro invocation (`println!(...)`, `vec![...]`: its arguments aren't parsed) | — |
| `argument` | each argument of a call or constructor (not an enum constant's) | each argument of a call or `new`, a tagged template's string one; none of a JSX element | each argument of a call (`*a`, `k=v`, `**kw` too), a lone generator (`f(x for x in xs)`) one; not a class's bases | each argument of a call; none of a macro | each argument of a call (a type, as `make`'s, too) |
| `branch` | `if`, a ternary `?:` | `if`, a ternary `?:` | `if`, `elif`, `x if c else y`, a comprehension's `if` | `if` (`if let` too) | `if` |
| `case` | each `switch` case (`default` too), of a statement or an expression | each `switch` case (`default` too) | each `match` case | each `match` arm | each `switch`, type switch or `select` case (`default` too) |
| `loop` | `for`, for-each, `while`, `do` | `for`, `for ... in` / `of`, `while`, `do` | `for`, `while`, a comprehension's `for` | `for`, `while`, `loop` | `for` |
| `try` | `try`, try-with-resources | `try` | `try`, `with` | — | — |
| `catch` | a `catch` clause | a `catch` clause | an `except` clause | — | — |
| `lambda` | a lambda | an arrow or function expression not bound to a name at the file's level or as a field | a `lambda` | a closure | a `func` literal |
| `function` | a method or constructor | a function, method, signature, an arrow bound to a name at the file's level or as a field | a `def` | a `fn`, a trait's signature | a function, a method, a named interface's method |
| `class` | a class, interface, enum, record or annotation type | a class, interface, enum, namespace | a `class` | a struct, enum, union, trait, `mod`; an `impl` is its type's class, weighing nothing of its own | a type definition (`type T struct {...}`, `interface`, `type C float64`), not an alias |
| `bool_op` | each `&&` / `\|\|` | each `&&` / `\|\|` / `??` | each `and` / `or` | each `&&` / `\|\|`, a let chain's | each `&&` / `\|\|` |
| `jump` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `raise`, an early `return` | `break`, `continue`, `?`, an early `return` | `break`, `continue`, `goto`, `fallthrough`, an early `return` |
| `operator` | each arithmetic, comparison (`instanceof` too), bitwise or unary operator, a compound assignment (a plain `=` under `assignments`), `++` / `--` | the same (`in`, `typeof`, `void`, `delete` too) | the same (`not`, `in`, `is` too) | the same (not `*x`, `&x`) | the same (`&^` too; not `*p`, `&x`, `<-ch`) |
| `access` | a field access, an array index | a member (`a.b`, `a?.b`), a subscript | an attribute, a subscript | a field (`a.b`, `a.0`), an index | a selector (`a.b`), an index, a slice (`a[i:j]`) |
| `parameter` | a method's, constructor's or lambda's parameter (a record's component too), not a receiver (`A this`) | a function's, method's, signature's or arrow's parameter, not a `this` one nor a function type's | a function's or lambda's parameter (`*args`, `**kwargs` too), not the first `self` / `cls` nor a bare `*` / `/` | a function's or closure's parameter, not `self` nor a `fn(...)` type's | each name of a function's, method's, literal's or interface method's parameters (an unnamed one once), not the receiver, the results nor a function type's |

JavaScript's elements are TypeScript's but for the type ones (an interface, an enum, a namespace, a signature, an
interface's property, a `type` alias); a `using` declaration is a statement, as `let`.

Every operator counts: `a && b && c` is 2 `bool_op`, `a + b - c` 2 `operator`, `a * b + c * d` 3, `x += b + c` 2,
`!!x` 2, Python's `a < b < c` 2; a plain `=` is 1 under `[conditions] assignments` (a declaration's initializer
none, `a = b = c` 2), a negative number (`-1`) none. An `access` is one
per `.` or `[]`, each next in a row heavier (`[accesses]`): `a.b.c` is 0.5 + 1. A call's callee is the call's (`a.f()` is a call, `a.b.f()` a call and an access), an
access to `this` / `super` / `self` / `cls` or a Go method's receiver is none (`this.x`, a receiver's `r.x` weigh as
a bare `x`), a qualified name in a type or an import (`java.util.List`, `a::b`, a Python annotation's `os.PathLike`)
is none.
An early `return` is one that isn't its function's or lambda's last statement. A Rust `if`, `match` or loop
used as a statement weighs as itself, not as an expression statement too. Annotations and decorators weigh
nothing of their own; a call in one (`@Input()`, `@app.route("/")`) is a call.

## Building

Rust 1.100 (pinned in `rust-toolchain.toml`, so rustup picks it): `cargo build --release`; checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`.

## License

MIT, see [LICENSE](LICENSE).
