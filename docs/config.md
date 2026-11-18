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
  `typescript`, `javascript`, `python`, `rust`); every `node_kinds` key is a named node kind of that language's grammar and its value a
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
- `[files]` is for what is weighed ([architecture.md](architecture.md), `source`).

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
try       = 1           # the body's nesting is its weight; a catch is the other path
catch     = 2
lambda    = 2
function  = 2           # a declaration, as a lambda: a getter is 3, not mostly its signature
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
exclude = ["**/node_modules/**", "**/build/**", "**/target/**", "**/generated/**", "**/*.d.ts", "**/*.min.js"]
test_globs = [
    "**/test/**", "**/tests/**", "**/__tests__/**", "**/*Test.java", "**/*Tests.java",
    "**/*.test.ts", "**/*.spec.ts", "**/*.test.tsx", "**/*.spec.tsx", "**/test_*.py", "**/*_test.py",
    "**/*.test.js", "**/*.spec.js", "**/*.test.jsx", "**/*.spec.jsx",
]
test_factor = 1.0       # a test's weight times this in a commit's delta; 0 leaves the test files out
```

Folder walks honour `.gitignore` too (the `ignore` crate).
