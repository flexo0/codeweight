# The weights file

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
  `typescript`, `javascript`, `python`, `rust`, `go`); every `node_kinds` key is a named node kind of that language's grammar and its value a
  category or `"none"`; every weight and factor is a finite number, 0 or above; every `[files]` glob compiles. The
  error names the table and the key, and the file.
- `typescript` is the table of both `.ts` and `.tsx` files (two grammars, one key): a TS setting reaches TSX too.
  Its `node_kinds` key may be a kind of either grammar (the `jsx_*` kinds are TSX's only, `type_assertion` TS's
  only); the grammar without it has no node of that kind, so there it does nothing. A `[languages.tsx]` table is an
  unknown language; should TSX ever need settings of its own, it can be added over `typescript`'s without changing
  what an existing file means.
- `javascript` is the table of `.js`, `.jsx`, `.mjs` and `.cjs` files, its own: a TS setting doesn't reach them, nor
  a JS one TS. Its grammar is the one TS's is built on, but it is another language: a JS-only project tunes it under
  its own name, and the two grammars' kinds already differ (JS's class field is `field_definition`, TS's
  `public_field_definition`; JS has `using_declaration`, TS not yet), so one `node_kinds` remap wouldn't reach both.
  Its `node_kinds` key is a kind of JS's grammar (`interface_declaration` is an unknown node kind there).
- A language's weights are `[weights]` with its `[languages.<name>.weights]` over them; its `node_kinds` override
  its own table (see [architecture.md](architecture.md)).
- The defaults hold no `[languages.<name>.weights]` (the example above has one only to show the table): the merge
  never removes a key, so a default per-language weight would beat the same key a user's file sets in `[weights]`,
  and that change would silently do nothing for that language. Tuning (milestone 7, [milestones.md](milestones.md))
  needed none; should one ever be wanted, the merge must first drop a default per-language key whose base key the
  user's file sets.
- `[conditions]` are switches, `true` or `false`; the defaults turn four on, all but `nullish_operator` (the
  tuning on the example pairs, milestone 16); a file on its own may leave the table out (all off), as the engine's
  tests do. Each weighs a node as an existing category, so it adds no weight of
  its own; a cast or pattern kind it makes an `operator` is the language's table's, under the user's `node_kinds`
  (a kind mapped there wins). What each one weighs is [weight.md](weight.md)'s.
- `[files]` is for what is weighed ([architecture.md](architecture.md), `source`).

```toml
[nesting]
model = "linear"        # linear: 1 + factor * depth; exponential: factor ^ depth
factor = 1.5            # each level of nesting adds one and a half base weights
max_depth = 10          # depth is capped here, so one deep method can't explode the total
# weigh the same at any depth: a statement's or a declaration's parts, which it already weighs the depth of
flat = ["call", "argument", "bool_op", "operator", "access", "parameter"]

[parameters]            # the parameter at place k (from 0) weighs parameter * min(1 + step * k, max)
step = 0.3              # 1.65, 2.15, 2.64, 3.14, then 3.3 each: a long list weighs more than its count
max = 2.0

[conditions]            # rules that weigh what a node's kind alone doesn't tell, as an existing category
casts = true            # a cast or type assertion (x as T, (T) x, x!, <T>x, x.(T)) is an operator
let_patterns = true     # Rust's `if let` / `while let` pattern is an operator, as a comparison is
magic_numbers = true    # a number literal but 0 and 1 is an access, as the named constant in its place would be
each_unary = true       # each unary operator counts, no run of them: !!x is 2
nullish_operator = false  # ?? and ??= are an operator, not a bool_op: a default value, no branch

[weights]               # the base weight of each category, for every language; tuned on the example pairs
statement = 1
call      = 0.55        # a call, `new`: a chain of getters is long, not hard
argument  = 0.05        # each argument of a call: little of its own, what is in it weighs
branch    = 3           # if, ternary
case      = 2           # each switch / match case, `default` too: below a branch, as a switch has one more
loop      = 3
try       = 0.95        # the body's nesting is its weight; a catch is the other path
catch     = 2
lambda    = 2
function  = 1.55        # a declaration, below a lambda: a getter is 2.55, not mostly its signature
class     = 5
bool_op   = 0.55        # each run of like && / || operators: a && b && c is 1, a && b || c is 2
jump      = 1.55        # break, continue, an early return, throw
operator  = 0.6         # each run of like operators: a + b - c is 1, (a + b) * c is 2; x += 1, i++, -x, !x
access    = 1.85        # each field access or index: a.b.c is 2, a[i] is 1; not a call's callee, nor this.x
parameter = 1.65        # a function's or lambda's parameter, the first; not self / this / a receiver

[languages.java.weights]            # a language's overrides of [weights]
lambda = 3

[languages.typescript.node_kinds]   # a tree-sitter node kind mapped to a category (or to "none")
satisfies_expression = "statement"

[files]
exclude = [
    "**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/vendor/**", "**/*.d.ts",
    "**/*.min.js", "**/*.pb.go", "**/zz_generated*.go",
]
test_globs = [
    "**/test/**", "**/tests/**", "**/__tests__/**", "**/*Test.java", "**/*Tests.java",
    "**/*.test.ts", "**/*.spec.ts", "**/*.test.tsx", "**/*.spec.tsx", "**/test_*.py", "**/*_test.py",
    "**/*.test.js", "**/*.spec.js", "**/*.test.jsx", "**/*.spec.jsx", "**/*_test.go",
]
```

Folder walks honour `.gitignore` too (the `ignore` crate).
