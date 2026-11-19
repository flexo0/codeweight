# Milestones

Built: milestones 0-25 (the repo, the CLI and the config, the engine and Java, TypeScript, TSX and Python, folders,
`commit`, the reports, the tuned defaults, Rust, operators and accesses, JavaScript, Go, depth on the structure
only, parameters, cases and arguments, conditions, the defaults tuned on pairs, macros, nested ternaries and default cases, assignments, constructors, a constructor category, every operator counted, the
defaults tuned again by rule, a row of accesses, the defaults tuned without a grid).

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
14. (built) Cases and arguments, two categories for the tuning on example pairs (a bad snippet and its fix, the
    same behaviour, from the linters' tests: 8,940 of them, outside the repo): `case`, a `switch` / `match` case
    apart from `branch`, and `argument`, each argument of a call ([weight.md](weight.md)). Both are neutral by
    default, `case` a branch's 2 and `argument` 0, so every weight is as before until a tuning moves them. The
    pairs a weight broke trade two categories both ways: a switch weighs more than the if-chain or ternary it
    replaces (its default is a case, an `else` nothing), while Clippy's `match` pairs want a heavy arm, so lowering
    `branch` breaks as many as it fixes; a call is lighter than the operators it replaces with 0 or 1 arguments
    (`(x + 7) / 8` → `x.div_ceil(8)`, `f != None` → `f.is_some()`) and heavier with 2 (`Objects.equals(a, b)` →
    `a == b`, `math.Pow(a, 2)` → `a * a`), so call and operator alone are best tied. On part 3's objective (3 per
    pair whose good side weighs more, 1 per tie) a coarse grid gave 2499 with call and operator, 2349 with
    `argument`, 2297 with `case` too; each source left out of a fit got better with `argument`. Not added: a
    `comparison` apart from `operator` (a smaller gain, worse on Ruff's and ESLint's pairs held out), and nothing
    for a `function` against a `lambda`, already two weights (function 1.5 alone fixes ESLint's
    `object-shorthand`). Seen and left: Rust's `if let Ok(_) = x` → `x.is_ok()` (the pattern weighs nothing), a
    slice weighing one access, `??` a `bool_op`, Rust's `as` casts weighing nothing, `!!x` one run of operators.
15. (built) Conditions, for the same pairs: `[conditions]` ([weight.md](weight.md)), rules that weigh what a node's
    kind alone doesn't tell, each as an existing category, all off by default so the defaults weigh as before until
    a tuning turns them on. Each candidate was counted apart in a throwaway build and scored on part 3's objective
    (3 per pair whose good side weighs more, 1 per tie; lower is better) over the 8,940 generated pairs, under the
    defaults and under a rough tuning (call 0.2, operator 0.5, argument 0.2, case 1.5, jump 1.5, function 1.5):
    `casts` −218 / −230 (typescript-eslint's `no-unnecessary-type-assertion`, Clippy's `unnecessary_cast`,
    `zero_ptr`, `ptr_eq`), `let_patterns` −21 / −276 (Clippy's `redundant_pattern_matching`, 90 pairs no weight
    could fix), `magic_numbers` −148 / −95 (Ruff's `FURB152`, Clippy's `from_str_radix_10`, `manual_div_ceil`),
    `each_unary` −47 / −29 (ESLint's `no-extra-boolean-cast`, Ruff's `SIM208`), `nullish_operator` −28 / −24
    (typescript-eslint's `no-unnecessary-boolean-literal-compare`); the last two break no pair. All five on: −469
    under the defaults, the ok pairs 4,392 → 4,765. Left out: a `default` / `_` case weighing nothing, as an `else`
    (−22 under the defaults but +39 tuned: Error Prone's `IfChainToSwitch` wants it, Clippy's `single_match` and
    `match_single_binding` don't); a logical expression as a statement (`a || f();`) weighing a branch (−9, mixed);
    an assignment repeating its target (`x = x + 1`) weighing an operator more (+18: ESLint tests both of its
    rules' options). Seen and not a condition: ESLint's `one-var` and Ruff's `RET503` are options both ways, a
    comprehension's `for` weighing a loop (Ruff's `FURB122`) a weight's matter.
16. (built) The defaults tuned on the example pairs, the 8,940 generated ones and the 31 picked by hand, by the
    objective of milestones 14 and 15 (3 per pair whose good side weighs more, 1 per tie; lower is better), with a
    simple evolutionary algorithm: `pairs/tune.py`, outside the repo beside the pairs. A snippet's weight is linear
    in its counts, so codeweight runs once per count, not per candidate: a run per category and depth cap, per
    parameter cap and per flat category under each of the 32 sets of conditions give each pair's difference, from
    which any candidate's objective is exact (checked against codeweight's own totals). The search, a (40 + 80)
    one on a grid of 0.05 from the defaults, tunes every base weight (`statement` fixed at 1), the nesting, the
    parameters and the conditions; among candidates of one objective the closest to the defaults wins, and what no
    pair asks for is moved back. 6077 → 4890: 764 pairs whose good side weighs more → 561, 3785 ties → 3207 (3172
    pairs tie under any weights, no count differs), the ok ones 4,422 → 5,203; every source gained but ESLint's
    worse ones, 58 → 65 (Clippy 1950 → 1343, Error Prone 1239 → 1062, typescript-eslint 1149 → 950, Ruff 533 →
    419). Held out, 30% of the pairs fit on the rest: 1818 → 1474, as much as the fit. Seeds differ by 4889 to
    4925 and trade weights the pairs barely tell apart: they fix ratios only where a rule's pairs trade two
    categories. What moved, with what putting it back costs: `operator` 0.25 → 0.6 (+402; an operator replaced by
    a call: Clippy's `redundant_pattern_matching`, `partialeq_to_none`, `manual_div_ceil`), `argument` 0 → 0.05
    (+157, mostly ties: a call of fewer arguments), `parameter` 0.5 → 1.65 with `step` 0.3 (+72 and +12, mostly
    Error Prone's `SingleValueTestParameter` and Clippy's `unnecessary_fold`), `branch` 2 → 3 (+64, Error Prone's
    `IfChainToSwitch`), `access` 0.25 → 1.85 (+61: an index, a slice or a field-by-field copy replaced by a call,
    Error Prone's `StringSplitter`, staticcheck's `S1016`, Ruff's `FURB131`), `jump` 1 → 1.55 (+58, Error Prone's
    `RefactorSwitch`), `bool_op` 1 → 0.55 (+33), `call` 0.5 → 0.55 (+25), `function` 2 → 1.55 (+21), `try` 1 →
    0.95 (+3); four conditions on, `let_patterns` (+277 off), `casts` (+238), `magic_numbers` (+145),
    `each_unary` (+38), and `nullish_operator` off (on: 11 ties more). The nesting factor is the corpora's choice,
    not the pairs': any factor from 1.05 to 1.5 scores 4890 (1.0 scores 4892, 2.65 4889), and against the former
    defaults GraphHopper's, OptaPlanner's and jsprit's top 200 functions keep their order best at 1.5 (Spearman
    0.957 to 0.966, at 1.05 about 0.94; codeweight's own 0.989), 18 or 19 of their top 20 kept. A re-tune that
    moves each gene back to the defaults while the pairs allow it takes the factor to 1.05: check the corpora
    first. The totals rise by ×1.3 (codeweight) to ×1.7, ×1.4 to ×1.5 on the Java corpora. The hand-picked
    pairs are as before: 30 hold, `java/nested_ternary_to_switch` still doesn't. This repo's own `codeweight.toml`
    keeps its weights, as its commits' weights were measured by them.
17. (built) Macros: `macro`, a Rust macro invocation apart from `call` ([weight.md](weight.md)), flat as a call.
    Its arguments are a token tree tree-sitter doesn't parse, so what is in it weighs nothing and the macro weighs
    for it: as a call's 0.55, Clippy's `redundant_pattern_matching` (`matches!(x, V4(_))` → `x.is_ipv4()`) and
    `useless_format` (`format!("hello")` → `"hello".to_string()`, a macro for a call: a tie) came out
    with the fix heavier or no lighter. Tuned as milestone 16
    (`pairs/tune.py`, the nesting kept): 1.15 on every seed, 4890 → 4837 (549 pairs whose fix weighs more, 561
    before; `redundant_pattern_matching` −46, `useless_format` −20, `unnecessary_literal_unwrap` +14, whose fix
    is `panic!()` for `None.unwrap()`); held out, 30% of the pairs fit on the rest, 1458 → 1443. Three of four
    seeds also lowered `bool_op` to 0.25, 3 points more: left at 0.55, as noise. On Rust code the totals rose 0.7 to 1.2%, the
    function rankings unchanged (codeweight's own and two apps: Spearman 1.000, the top 20 kept). This repo's own
    `codeweight.toml` sets `macro` to its `call`, 0.5, so its macros weigh as before. Not done: parsing a macro's
    arguments as expressions (`vec![f(x)]`, `assert_eq!(a.len(), 3)`), which would weigh what is in them but makes
    the `matches!` pairs ties rather than wins.
18. (built) Nested ternaries, default cases and `return switch`, as asked after milestone 17's pairs were read: a
    nested ternary is bad, a switch should weigh less than its if-chain, and a `return` before a switch expression
    is no line of its own. Two conditions, on by default: `nested_ternaries`, a ternary chained in a ternary's
    alternative nesting (a `Nesting::ternary`, Java's, TS's and JS's ternary and Python's conditional), and
    `free_default`, a default case weighing nothing as an `else` (each language's `default_case` hook: Java's
    `default` label, TS's and JS's `switch_default`, Go's `default_case`, a bare `_` in Rust and Python). Java's
    last `return switch (...) { ... };` weighs no statement, the switch weighing for it (the `carried_return`
    hook; an early one stays a jump). The pairs whose fix changes the behaviour are deleted from the pairs, and the
    extractors skip their rules: `pairs/semantic.tsv`, 94 rules, 560 pairs (Error Prone's `error`, `likely_error`, `fragile_code` and untagged
    groups; its rules that add a check, a locale, a charset, an annotation or immutability, or that change a type
    or what a test checks; ESLint's `eqeqeq` and `strict`; typescript-eslint's `return-await`,
    `promise-function-async`, `prefer-readonly`). The tuning keeps `case` below `branch`. On the 8,411 pairs
    left the objective is 4124 (4850 with the deleted ones), and 4073 on 8,391 once the pairs the splitting cut
    wrong are dropped too (`pairs/broken.tsv` and a check in `extract/split.py`), then 3185 on 8,095 once the 292
    pairs whose fix rightly weighs no less are dropped (`pairs/heavier.tsv`: a style option for the longer form,
    code or a check added, the same in another form), 110 worse left, every one a simplification, then 133 on
    1,453 once `gen/` is thinned to one pair per rule and difference, the ties under any weights deleted
    (`tune.py dedup`: a linter's many variants of one rewrite no longer count again); `return switch`
    −6, `nested_ternaries` 0, `free_default` +24 (Clippy's `single_match` and `manual_map`, a `match` with an
    empty `_` arm against an `if let`, and Error Prone's `UnnecessaryDefaultInEnumSwitch` now ties), but the
    hand-picked `java/nested_ternary_to_switch` holds at last (23.05 against 21.84). A re-tune moved little:
    4124 → 4105, held out 1252 → 1252, every move ±3 points (`case` 2.4, `lambda` 1.65, `jump` 1.35, `bool_op`
    down to 0.05 on some seeds): the weights are as they were. On the corpora the totals moved less than 1.2% and
    the function rankings held (Spearman 0.997 to 1.000, the top 20 kept).
19. (built) Assignments, as asked after milestone 18's Java pairs were read: `case 1 -> x = f();` should weigh more
    than the switch expression's `case 1 -> f();`, by its `=`. A condition, on by default: `assignments`, a plain
    `=` an `operator` beside its statement, as a compound one is (an `Operator::Assign` the engine adds, never in
    the node's place, so Go's assignment stays a statement); a declaration's initializer is none (`int x = 1`,
    `let`, `const`, `:=`, `var x = 1`), nor is Python's `=`, its declaration too. On the pairs: 133 → 130,
    Clippy's `needless_late_init` (`match n { 1 => a = "one", ... }` against `let a = match n { 1 => "one", ... }`)
    now better; Error Prone's `RefactorSwitch` still worse, its `x = switch (...)` a statement more against an
    `=` less per case (statement 1, operator 0.6). On the corpora the totals rose 1-2% and the function rankings
    held (Spearman 0.996 to 0.999, the top 20 kept but one at jsprit).
20. (built) Every constructor weighs, as asked after an array's `new Integer[] {...}` was seen to weigh nothing: a
    `call`, as `new Foo()` is. Java's `array_creation_expression` (`new int[3]`, `new Integer[] {1, 2}`; a bare
    `{1, 2}` initializer stays none), Rust's `struct_expression` (`P { x: 1 }`, as the tuple struct's `P(1)` is a
    call), Go's `composite_literal` (`P{X: 1}`, `[]int{1}`, `map[K]V{}`; an element's own `{...}` none). TS's, JS's
    and Python's were calls already. On the pairs: 130 → 127, Error Prone's `TruthContainsExactlyElementsInUsage`
    (`containsExactlyElementsIn(new Integer[] {1, 2, 3})` against `containsExactly(1, 2, 3)`) now better. On the
    corpora the totals and the function rankings held (×1.00, Spearman 1.000).
21. (built) `constructor`, a category apart from `call`, as asked after milestone 20: a constructor makes a new
    object where a method call asks one, so it may weigh otherwise. Java's `new Foo()`, array creation and
    `this(...)` / `super(...)`, TS's and JS's `new`, Rust's struct literal, Go's composite literal; Python's `Foo()`
    and Rust's tuple struct `P(1)` stay calls, no syntax telling them from a function's. 0.9 by default, a bit
    above a call's 0.55, by judgment: for Java's, TS's and Go's constructors any weight from 0.25 to 5 scores the
    same on the pairs (134 at 0); with Rust's struct literal 127 → 124, Clippy's `init_numbered_fields`
    (`T { 0: v }` against `T(v)`, a constructor against a call) now better. Its arguments count as a call's, it is flat as `call`;
    `tests/weights.toml` sets it to a call's 2, with a snippet test per language at its own value; this repo's own
    `codeweight.toml` to its call's 0.5.
22. (built) Every operator counts, as asked after a chained assignment (`x = (x.field = new Foo())`) was seen to
    weigh one `=`: like operators in a row were one run (milestone 9's rule, after Cognitive Complexity's for
    `&&` / `||`), now each is one, `bool_op` and `operator` alike: `a && b && c` is 2, `a + b - c` 2, `!!x` 2,
    `a = b = c` 2, Python's chained `a < b < c` 2 and a Rust let chain a `&&` per link (`Lang::chains`, an operator
    per operand after the first). `[conditions] each_unary`, which counted each unary operator out of its run, is
    gone with the runs, and so are `Lang::parens` and the engine's operator of the enclosing expression; the
    `Operator` enum keeps only what tells the categories apart (logical, `??`, other, a plain `=`). On the pairs:
    124 → 123 (worse 37 → 36, ties 13 → 15), Error Prone's `RefactorSwitch` `L209-8` now better (4 `=` against 2),
    Staticcheck's `QF1005` (`math.Pow(x*2, 2)` against `x * 2 * x * 2`) now worse; the weights stay (`operator`
    0.6 is still the best, `bool_op` 0.2 would score 120). The corpora not re-checked.
23. (built) The defaults tuned again on the 1,453 pairs, by a new score (`pairs/tune.py`): still 3 per pair whose good
    side weighs more and 1 per tie, but each pair weighed 1 / sqrt(its rule's pairs), a hand-picked one three times
    that, so a rule with fifty tests doesn't count fifty times one with a single test, plus the distance from the
    defaults (`lam = 1`) in place of the tie-break, so a weight moves only as far as the pairs ask. A soft margin on
    ln(bad / good) was tried as a term too, so that an order held by a hair counts: on five folds of the rules,
    each fitted on four and scored on the fifth, it cut the pairs within 5% of a tie (82 → 22-31) but put more in
    the wrong order on the rules held out (44 → 48-56), and is off; the pull of 1 held out better than 0.3 (39
    against 44 worse). Three seeds agree: `macro` 1.15 → 1.75, `case` 2 → 2.4, `lambda` 2 → 1.65, `jump` 1.55 →
    1.35; the nesting stays linear, 1.5 (a pull of 0.3 would go exponential, 2.05-2.45, for one pair or two). On the
    pairs: worse 36 → 32, ties 15 → 12, Clippy's `manual_map` and `redundant_pattern_matching` now better (a
    `matches!` heavier than the `is_none() && ...` it stands for), `unnecessary_literal_unwrap`'s two `panic!` with a
    format string now worse than `Err(1).unwrap()`. The corpora not re-checked. This repo's own `codeweight.toml`, a
    frozen copy of the defaults before milestone 16 so that its commits weighed on one scale, is gone: the engine's
    changes since had moved that scale anyway, and the repo now weighs itself by the defaults it ships (its commits'
    `[+N]` up to this one are on the old scale).
24. (built) A row of accesses, as asked: each next access in a row weighs more, the first 0.5, the second 1, the
    third and on 1.5. `[accesses]` is `[parameters]`'s form, `access × min(1 + step × place, max)`, `access` 0.5,
    `step` 1, `max` 3; an access's place is how many accesses its object is in a row of (`a.b.c.d`: `.b` 0, `.c` 1,
    `.d` 2), a call, `this` or any other node ending the row ([weight.md](weight.md)). A file on its own that leaves
    the table out weighs every access alike (`step` 0, `max` 1), so the engine's tests stand; a snippet per
    language (`lang::testing::weigh_accesses`) has `access` 9 grow 9, 18, 27. `pairs/tune.py` counts the accesses
    by place and tunes `step` and `max` too. On the pairs: worse 32 → 39, ties 12 → 13, all of it the first
    access's 1.85 → 0.5 (a lone index or slice replaced by a call, Ruff's `FURB131`, `FURB145`, `FURB187`,
    `PERF403`, typescript-eslint's `prefer-string-starts-ends-with`, Clippy's `unnecessary_operation`: `l[:]` at
    0.5 now below `l.copy()`); `step` and `max` move no pair (with `access` at 1: 33). Clippy's `unnecessary_fold`
    `L224` now better. The corpora not re-checked.
25. (built) The defaults tuned by a search without a grid. `pairs/tune.py` had kept every weight on a grid of 0.05
    and mutated one or two genes at a time; it now shakes every real value at once, x to x × U(0.9, 1.1) +
    U(-0.1, 0.1), the shake falling to a hundredth by the last of 3000 generations, to 0.001 (a generation on
    every core, about 30 s a search). A difference up to 0.01, a report's last digit, is now a tie (it was 1e-6:
    the search had split ties by 0.001, `bool_op` 0.548 against `operator` 0.549, which the reports show the
    same). With `access` kept at 0.5 (as asked in milestone 24; free, it goes to 1.1): `call` 0.55 → 0.49, `macro`
    1.75 → 1.6, `constructor` 0.9 → 0.949, `argument` 0.05 → 0.316, `bool_op` 0.55 → 0.544, `operator` 0.6 →
    0.556. On the pairs: worse 39 → 35, ties 13 → 5. On five folds of the rules it holds out as the old search
    does (42 worse; ties 10 → 6): the better fit is mostly the pairs' own (`pairs/README.md`). The corpora not
    re-checked.
