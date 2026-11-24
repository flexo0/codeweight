# The weights file

TOML. The first one found wins: `--config`, then `./codeweight.toml`, then `codeweight.toml` at the root of the git
repo the first scanned path is in (for `show` and `diff`, the repo's), then the built-in defaults. Your file is merged over the defaults, so it holds only
what it changes. A key, category, language or node kind codeweight doesn't know is an error, not ignored.
`codeweight init` writes out the defaults ([src/default.toml](../src/default.toml)) to start from. The example below is
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

[docs]                  # a Markdown file's docs weight: words * word + function words * function_word + punctuation
                        # runs * punctuation + symbol runs * symbol, on the code's scale: a description weighs what
                        # the code it describes does
word = 0.64             # a run of letters and digits; a URL is one word
function_word = 0.32    # a word of function_words in prose (not in inline code or a fenced block): glue
punctuation = 0.32      # a run of prose's punctuation, once however long: `,`, `),`, `."`, a backtick, a lone - or _
symbol = 0.62           # a run of anything else that isn't a space, once however long: `->`, `**`, `##`, `](`, `|`
function_words = [      # whatever their case; a list of one's own replaces this one (add another language's to it)
    "a", "an", "the", "of", "to", "in", "on", "at", "by", "for", "with", "from", "into", "as",
    "is", "are", "was", "were", "be", "been", "being", "it", "its", "this", "that", "these", "those",
    "there", "which", "s",
]

[files]                 # what a folder walk weighs (globs: * and ? within a path's part, ** across parts)
# an exclude ending in / is a folder: build/ at any depth, src/gen/ or /build/ from the root; --exclude adds more
exclude = [
    "**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/vendor/**", "**/*.d.ts",
    "**/*.min.js", "**/*.pb.go", "**/zz_generated*.go",
]
test_globs = [
    "**/test/**", "**/tests/**", "**/__tests__/**", "**/*Test.java", "**/*Tests.java",
    "**/*.test.ts", "**/*.spec.ts", "**/*.test.tsx", "**/*.spec.tsx", "**/test_*.py", "**/*_test.py",
    "**/*.test.js", "**/*.spec.js", "**/*.test.jsx", "**/*.spec.jsx", "**/*_test.go",
    "**/test_*.c", "**/*_test.c",
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

## The weight

```
weight(node) = weights[category(node)] × nest(min(depth, max_depth))    nest = 1 for a category of [nesting] flat
```

`depth` is the number of nesting constructs around a node: a branch, a loop, a `try` / `catch` / `finally`, a
`switch` case, a lambda, and a function or class declared inside a function. Only a construct's body is deeper. Its
head (a condition, a loop's header, a catch's parameter, a case's label) stays at the construct's own depth. Plain
tree levels don't count (`a.b().c()` is not deep). An `else if` (Python's `elif`) is at its `if`'s depth, and so
are the cases of a `switch` or a `match`.

The languages' keys are `java`, `typescript` (for `.ts` and `.tsx` alike: its `node_kinds` may name a kind of either
grammar, the `jsx_*` ones being TSX's), `javascript` (its own, not `typescript`'s), `python`, `rust`, `go` and `c`. A
node kind is a tree-sitter one, as the language's grammar names it.

## The categories

| Category | Java | TypeScript, TSX, JavaScript | Python | Rust | Go | C |
|---|---|---|---|---|---|---|
| `statement` | an expression statement, a local variable, a field, a `return` that ends its function or lambda, a lambda's expression body (`x -> f(x)`, as `{ return f(x); }`), `yield`, `assert`, `synchronized`, an enum constant | an expression statement, `const` / `let` / `var`, a class field, an interface's property, a `type` alias, a `return` that ends its function, an arrow's expression body (`x => f(x)`) | an expression statement (an assignment too; not a docstring or a bare `...`), `assert`, `del`, a `type` alias, a `return` that ends its function, a lambda's body, a comprehension's element | an expression statement, `let`, `const`, `static`, a `type` alias, a field, an enum variant, a block's tail expression (its value, no `;`), a closure's expression body, a `match` arm's value that is no block, a `return` that ends its function or closure | an expression statement, an assignment, `:=`, each `var` / `const` spec, a send, `defer`, `go`, a type alias, a field, `i++` / `x += 1` (an `operator` too; not as a `for` clause's update), a `return` that ends its function or literal | an expression statement (not an empty `;`), a declaration, a `typedef` (not one defining a struct), a field, an enumerator, `#define N 10`, a `return` that ends its function |
| `call` | a method call | a call; in TSX and JavaScript each JSX element | a call (a class's `Foo()` too: no syntax tells it from a function's) | a call (a tuple struct's `P(1)` too) | a call (`new(T)`, `make(...)` too) | a call (a macro's `MAX(a, b)` too) |
| `constructor` | `new Foo()`, an array's `new int[3]` / `new T[] {...}` (not a bare `{...}` initializer), `this(...)` / `super(...)` | `new Foo()` | — | a struct literal (`P { x: 1 }`) | a composite literal (`P{X: 1}`, `[]int{1}`, `map[K]V{}`; not an element's own `{...}`) | a compound literal (`(struct P){1, 2}`; not a bare `{...}`) |
| `macro` | — | — | — | a macro invocation (`println!(...)`, `vec![...]`: its arguments aren't parsed) | — | — |
| `argument` | each argument of a call or constructor (not an enum constant's) | each argument of a call or `new`, a tagged template's string one; none of a JSX element | each argument of a call (`*a`, `k=v`, `**kw` too), a lone generator (`f(x for x in xs)`) one; not a class's bases | each argument of a call; none of a macro | each argument of a call (a type, as `make`'s, too) | each argument of a call |
| `branch` | `if`, a ternary `?:` | `if`, a ternary `?:` | `if`, `elif`, `x if c else y`, a comprehension's `if` | `if` (`if let` too) | `if` | `if`, a ternary `?:` |
| `case` | each `switch` case (`default` too), of a statement or an expression | each `switch` case (`default` too) | each `match` case | each `match` arm | each `switch`, type switch or `select` case (`default` too) | each `switch` case (`default` too) |
| `loop` | `for`, for-each, `while`, `do` | `for`, `for ... in` / `of`, `while`, `do` | `for`, `while`, a comprehension's `for` | `for`, `while`, `loop` | `for` | `for`, `while`, `do` |
| `try` | `try`, try-with-resources | `try` | `try`, `with` | — | — | MSVC's `__try` |
| `catch` | a `catch` clause | a `catch` clause | an `except` clause | — | — | MSVC's `__except` |
| `lambda` | a lambda | an arrow or function expression not bound to a name at the file's level or as a field | a `lambda` | a closure | a `func` literal | — |
| `function` | a method or constructor | a function, method, signature, an arrow bound to a name at the file's level or as a field | a `def` | a `fn`, a trait's signature | a function, a method, a named interface's method | a definition, a prototype at the file's level |
| `class` | a class, interface, enum, record or annotation type | a class, interface, enum, namespace | a `class` | a struct, enum, union, trait, `mod`; an `impl` is its type's class, weighing nothing of its own | a type definition (`type T struct {...}`, `interface`, `type C float64`), not an alias | a struct, union or enum with a body |
| `bool_op` | each `&&` / `\|\|` | each `&&` / `\|\|` / `??` | each `and` / `or` | each `&&` / `\|\|`, a let chain's | each `&&` / `\|\|` | each `&&` / `\|\|` |
| `jump` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `throw`, an early `return` | `break`, `continue`, `raise`, an early `return` | `break`, `continue`, `?`, an early `return` | `break`, `continue`, `goto`, `fallthrough`, an early `return` | `break`, `continue`, `goto`, an early `return` |
| `operator` | each arithmetic, comparison (`instanceof` too), bitwise or unary operator, a compound assignment (a plain `=` under `assignments`), `++` / `--` | the same (`in`, `typeof`, `void`, `delete` too) | the same (`not`, `in`, `is` too) | the same (not `*x`, `&x`) | the same (`&^` too; not `*p`, `&x`, `<-ch`) | the same (not `*p`, `&x`, `sizeof`) |
| `access` | a field access, an array index | a member (`a.b`, `a?.b`), a subscript | an attribute, a subscript | a field (`a.b`, `a.0`), an index | a selector (`a.b`), an index, a slice (`a[i:j]`) | a field (`a.b`, `p->b`), a subscript |
| `parameter` | a method's, constructor's or lambda's parameter (a record's component too), not a receiver (`A this`) | a function's, method's, signature's or arrow's parameter, not a `this` one nor a function type's | a function's or lambda's parameter (`*args`, `**kwargs` too), not the first `self` / `cls` nor a bare `*` / `/` | a function's or closure's parameter, not `self` nor a `fn(...)` type's | each name of a function's, method's, literal's or interface method's parameters (an unnamed one once), not the receiver, the results nor a function type's | a definition's or prototype's parameter (`...` too), not `(void)` nor a function pointer's |

JavaScript's elements are TypeScript's but for the type ones (an interface, an enum, a namespace, a signature, an
interface's property, a `type` alias); a `using` declaration is a statement, as `let`. C's `#if` / `#ifdef` blocks
weigh nothing of their own: their code weighs where it is, their condition its operators.

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
