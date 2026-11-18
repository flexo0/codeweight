# Milestones

Built: milestones 0-11 (the repo, the CLI and the config, the engine and Java, TypeScript, TSX and Python, folders,
`commit`, the reports, the tuned defaults, Rust, operators and accesses, JavaScript, Go).

0. (built) The repo: `Cargo.toml`, `README.md` (usage, the weights file), the MIT license, GitHub Actions (`cargo fmt
   --check`, `clippy -D warnings`, `test`).
1. (built) The skeleton: the CLI, the config with its embedded defaults, `init`.
2. (built) The engine and Java, for `scan <file>`: snippet tests with weights worked out by hand.
3. (built) TypeScript, TSX and Python.
4. (built) Folders: the walk, the excludes, the test globs and factor, `rayon`.
5. (built) `commit`: tests on throwaway repos (an add, a delete, a rename, a merge, the root commit, a range, the
   uncommitted changes).
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
   Re-tuned after milestone 9 on GraphHopper's core, jsprit's core and OptaPlanner's core (Java), two React apps
   (TS, TSX), two Python tools and two Rust repos: each function's weight split by category (a run per category,
   its weight 1, the others 0; the linear model makes a weight the sum of the parts), so any set of weights could
   be ranked without a rerun. The function rankings barely move for any one base weight (call 0.35, lambda 1, branch
   3 with loop 2: at most two functions in or out of a corpus's top 20, the top 200's rank correlation 0.97 to
   0.999), nor for a linear factor of 0.5 or 1.5; size times nesting ranks them, and only the exponential model
   reorders the top. Fix commits per file (GraphHopper, OptaPlanner) can't tell the sets apart either: every one
   correlates with them as lines of code do. So the base weights are mostly the prices in a total and in a commit's
   delta, and there `function` 3 was the one off: a method's own weight was 17 to 26% of a Java corpus's total,
   OptaPlanner's above all its statements, as 39 to 47% of the Java methods are getters, setters and delegations
   whose body weighs 1.5 at most (the median body 2, TS, Python and Rust 13 to 17). Now `function` is 2, a lambda's
   (both a function): a getter is 3, a getter and its setter no longer a loop of two calls. A `try` is now 1: its
   body's nesting already weighs it (a Python `with` is one too), and its `catch` is the other path, 2 as a branch.
   The Java totals fell 6 to 9%, the others 2 to 3%; the files leaving the top 15 are an interface of signatures
   (`UniConstraintStream`), a config bean (`TerminationConfig`) and a builder of setters, those entering hold logic
   (`KOptDescriptor`, `EntitySelectorFactory`, `MaxTimeInVehicleConstraint`). Branch 2 and loop 3 held: the swap
   moves no ranking, and a branch's weight is each switch case's too.
8. (built) Rust (`.rs`, `tree-sitter-rust`): its table, the engine's look through a statement wrapping a return
   (`wrappers`) and the tail expression as a statement (`tail`); snippet tests. The defaults re-checked on a Rust
   repo, codeweight itself: the heaviest are the engine's tree walk (`Weigher.weigh`, a loop of matches and nested
   ifs), the weights file's checks (`Config.from_value`) and the two inputs' listings (`from_commit`,
   `from_paths`); a language's long `const` tables weigh a statement each, a struct its class and a statement a
   field, the one-line getters (`Config.lang`, `Weigher.lang`) least.
   The defaults held, no weight changed. Rust's integration tests are under `tests/`, which a test glob already
   matches; its unit tests are in the source files (the `tests` hook, [languages.md](languages.md), finds them since).
9. (built) Operators and accesses: `a = b`, `a = b + c` and `a = (b + c) * d` weighed alike, as `obj.a` and
   `obj.a.b`. Now `operator` (runs of like operators, the `bool_op` rule; the `logical` hook became `operator`) and
   `access` (a field access or index), both 0.25 by default: below a call's 0.5, an expression's detail is not its
   control flow. Checked on GraphHopper's core and jsprit (Java) and codeweight itself: the totals grew 3 to 6%,
   the heaviest functions stayed first (their nesting still dominates), and the ones that grew most for their
   weight are the arithmetic and bit packing ones (`IntEncodedValueImpl.getInt` +42%, `ArrayUtil.merge` +37%). The
   engine's test weights set both 0, so the snippets about other things leave them out; the snippets about them
   are weighed with both set (`lang::testing::weigh_expressions`).
10. (built) JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`, `tree-sitter-javascript` 0.25 on `tree-sitter` 0.27):
    TypeScript's table but for the type kinds, its nesting and hooks; its own weights-file key, `javascript`
    ([config.md](config.md)); snippet tests. The test globs gained `*.test.js` / `*.spec.js` and their `.jsx`, the
    excludes `*.min.js` (a minified bundle is built, not written). The defaults are TS's, untuned on JS.
11. (built) Go (`.go`, `tree-sitter-go` 0.25 on `tree-sitter` 0.27): its table, nesting and hooks
    ([languages.md](languages.md)), its weights-file key `go`; snippet tests. The engine gained a language
    table, the lists (Go's `statement_list`, between a block and its statements, which the early return looks
    through), and the `tail` hook became `also_statement`, Rust's tail and Go's `i++` / `x += 1` behind it.
    The test globs gained `*_test.go`, the excludes `vendor` (`go mod vendor`'s copies of the dependencies) and
    the generated Go files that sit by the source, `*.pb.go` (protobuf) and `zz_generated*.go` (Kubernetes'
    generators). The defaults are untuned on Go.
12. (built) Depth multiplies the structure, not a statement's parts: `[nesting] flat` ([weight.md](weight.md)), by
    default `call`, `bool_op`, `operator` and `access`, which weigh the same at any depth, as their statement
    already weighs its depth. An expression body (a lambda's, a Rust arm's value, a Python comprehension's element)
    became a statement, so its depth still weighs and `x -> f(x)` weighs as `x -> { return f(x); }`. The engine's
    test weights set `flat = []`, so the hand-worked sums are still `weight × nest(depth)`. Re-checked on
    codeweight, GraphHopper, jsprit, OptaPlanner, a TS / JS app, a Python and a Rust app: the totals fell 6 to 17%
    (codeweight's rose 3%, its Rust `match` tables of one-value arms now weighing a statement an arm, as Java's
    `case X: return "x";`), the functions' order held (Spearman 0.998 to 0.9997, 9 or 10 of the top 10 kept), the
    deep ones fell most (a nested function of long statements by a third). The factor held at 1.0.
13. (built) Parameters: `parameter`, a category weighed by its place (`[parameters]`, [weight.md](weight.md)), 0.5
    for the first, 0.1 more for each next, 1.0 from the sixth on; each language's `parameters` hook counts a
    declaration's parameters, a receiver and a function type's aside. A call's arguments stay free: what is in
    them weighs, and a long argument list mirrors a long parameter list, which weighs once, where it is declared.
    The engine's test weights set it 0 but in the snippets about it (`lang::testing::weigh_parameters`).
    Checked on codeweight, GraphHopper, jsprit, OptaPlanner, a TS / JS app, a Python and a Rust app: parameters
    are 2 to 5% of a total (the Java ones most, as a class's own weight is 4 to 6%), the top 10 held in every one
    (Spearman 0.966 to 0.999, the small functions reordering most), those rising most are constructors and helpers
    of long lists (`IntEncodedValueImpl`'s, `DroolsVariableFactory.createVariable`).
