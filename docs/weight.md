# The weight

```
weight(node) = base[category(node)] × nest(min(depth, max_depth)), nest 1 for a category of [nesting] flat
total        = Σ weight(node) over the nodes that have a category
```

- `depth` is the number of nesting constructs around the node: branch, loop, `try` / `catch`, lambda, nested
  function or class. Plain tree levels don't count (`a.b().c()` is not deep).
- Only a construct's body is one level deeper; its head (a condition, a loop's header, a catch's parameter, a case's
  label, try's resources) stays at the construct's depth, so the same condition weighs the same wherever it is. A
  `for`'s condition and update stay at the loop's depth although they run on every pass. A catch or finally clause
  is at its try's depth (it is beside the try's body, not in it), its block one deeper.
- Depth multiplies the structure, not a statement's parts: the categories of `[nesting] flat` (by default `call`,
  `bool_op`, `operator`, `access`) weigh their base weight at any depth. The statement around them already weighs
  its depth, so multiplying its parts too would charge the depth again for each of them, and a long statement in a
  loop would weigh more for the loop than a short one; it is long, not harder for being nested. (Cognitive
  Complexity does the same: its nesting increment is for structure, a run of `&&` / `||` +1 flat.)
- An expression body is a statement, as the `return` of its value would be: a lambda's (`x -> f(x)` weighs as
  `x -> { return f(x); }`), a Rust arm's value or closure's body that is no block (`=> f(x)` as `=> { f(x) }`), a
  Python comprehension's element (as the loop's statement making it). It holds a statement's place in a construct's
  deeper body, so without one its depth would weigh nothing once its parts are flat. A ternary's values are no
  statements: a ternary is an expression in a statement, which weighs it. The language's `also_statement` hook.
- `else if` chains and `switch` / `match` cases are siblings of one level, not deeper ones. The switch itself is not
  a nesting construct; each case is (a `case`, its statements one deeper). An `if` directly in an if's `alternative`
  stays at that if's depth; an `if` inside an `else { }` block is nested. A ternary chained in a ternary's
  alternative (`a ? x : b ? y : z`) is a sibling the same way, but under `[conditions] nested_ternaries` (on by
  default), where it nests.
- A function or class declared inside a function (a local or anonymous class, its methods, a nested function)
  nests its body; a member class or method doesn't, whatever the depth of classes around it. Depth isn't reset at a
  function: a method of a class in an `if` in a method counts both.
- `case`: each case of a `switch` or a `match` (`default` and `_` too), apart from `branch` (an `if`, a ternary),
  so a tuning on pairs of code (a snippet and its refactoring) can weigh the two apart. By default it
  weighs less than a branch, and the default case nothing (`[conditions] free_default`), as an `else`: a switch
  weighs less than the if-chain it replaces.
- `bool_op`: one per `&&` / `||` operator: `a && b` is 1, `a && b && c` and `a && b || c` are 2,
  `(a && b) || (c && d)` is 3. Every logical operator counts, in a condition or not; like operators in a row are no
  run that counts once (every operator is something to read). Java's `&` / `|` are not logical (they can't be told from bitwise ones without types): they are bitwise
  operators. The rule is the language's `operator` hook, whatever its node kind's table entry is.
- `operator`: one per operator, by the `bool_op` rule: `a + b` is 1, `a + b + c` and `(b + c) * d` are 2,
  `a * b + c * d` is 3. The operators are the arithmetic ones (`+ -`, `* / %`, Python's `//` and `@`, `**`), the
  shifts, the comparisons (`==`, `<`, ..., `instanceof`, `in`, `is`; a chained Python comparison `a < b < c` is one
  per comparison, 2), `&`, `|`, `^`, and the unary ones (`-x`, `!x`, `not x`, `~x`, TS's `typeof`, `++` and `--`;
  `!!x` is 2). A compound assignment (`x += 1`) is its operator, so `x += b + c` is 2; a plain `=` is none, the
  statement already weighing it, but under `[conditions] assignments` (on by default), where it is one too. A negative number (`-1`) is a literal, no operator. A Rust
  dereference (`*x`) and a borrow (`&x`) are none, and so are Go's (`*p`, `&x`) and its receive (`<-ch`), and C's (`*p`, `&x`). `&&=`,
  `||=`, `??=` are logical: `bool_op`.
- `access`: one per field access or index, so `obj.a` is 1, `obj.a.b` and `a[i][j]` are 2, each weighed by its
  place in a row of them, from 0: `access × min(1 + step × place, max)` (`[accesses]`). The place is how many
  accesses its object is in a row of: `obj.a.b.c`'s `.a` is at 0, `.b` at 1, `.c` at 2; a call, `this` or
  anything else that is no access ends the row (`o.f().a.b`'s `.a` is at 0, `this.v.w`'s `.w` too), and an index's
  own expression is a row of its own (`o.a[i.j]`: `[...]` at 1, `.j` at 0). With the defaults (0.5, `step` 1, `max`
  3) that is 0.5, 1, then 1.5 each: reaching deep through objects, past what is at hand, weighs more than its count
  (a magic number, an `access` by `[conditions]`, is at 0). A file on its own that leaves the table out weighs
  every access alike (`step` 0). A call's callee is the
  call's, no access: `obj.f()` is a call, `obj.a.f()` a call and an access (`obj.a`), as Java's grammar has it (its
  method call holds its object, no field access around the name); TS's `new a.B()` and a JSX element's `<a.B />`
  the same. An access to `this` / `super` (Java, TS), `self` / `cls` (Python), `self` (Rust) or a method's receiver
  (Go's `r.x` in `func (r *T) M()`) is none, as a bare `x` (Java's implicit `this`) is none; `this.a.b` is 1. A
  qualified name in a type or an import is no access: the grammars parse those as other kinds (`scoped_identifier`,
  `nested_type_identifier`, `dotted_name`), and a Python annotation's, which is an expression, is exempt. Both are the language's `exempt` hook.
- `parameter`: a function's or lambda's parameter weighs by its place, from 0: `parameter × min(1 + step × place,
  max)` (`[parameters]`), so with the defaults 1.65, 2.15, 2.64, 3.14, then 3.3 from the fifth on. A long list weighs more than
  its count, as each next parameter is one more to keep in mind and to pass in order. A receiver is none (Java's
  `A this`, TS's `this: T`, Python's first `self` / `cls`, Rust's `self`, Go's receiver), as an access to it is none,
  and neither is a function type's (`(a: A) => B`, `fn(i32)`, Go's `func(int)`), which declares no function. Each
  language's `parameters` hook counts them; the weight is the declaration's, at its depth (flat by default).
- `argument`: each argument of a node that is a `call` or a `constructor`, at its depth (flat by default): the items of its
  `arguments` field's list, or one for a lone argument in its place (Python's `f(x for x in xs)`, a tagged
  template's string); none of a call without the field (a Rust macro, whose arguments are tokens; a JSX element;
  `new A` without parentheses), nor of what is no call (Java's enum constant `X(1)`, Python's class bases). It is 0.316
  by default, besides what is in an argument; it is a category for the pairs, where a call lighter than the operators it replaces has 0 or 1 arguments (`f.is_some()`
  for `f != None`) and a heavier one 2 (`Objects.equals(a, b)` for `a == b`). The engine counts them
  (`lang::arguments`), the same for every language.
- `macro`: a macro invocation (Rust's `println!(...)`, `vec![...]`, `matches!(...)`), apart from `call`. Its
  arguments are a token tree tree-sitter doesn't parse, so what is in it weighs nothing, nor does it count any
  `argument`: the macro weighs for its content. It is flat by default, as `call`. The example pairs replace
  `matches!(x, P)` by a call (`x.is_ipv4()`), which with a macro weighing a call's 0.55 came out heavier.
- `constructor`: a constructor, apart from `call`: Java's `new Foo()`, an array's `new int[3]` / `new T[] {...}`
  and `this(...)` / `super(...)`, TS's and JS's `new`, Rust's struct literal (`P { x: 1 }`), Go's composite
  literal (`P{X: 1}`, `[]int{1}`), C's compound literal (`(struct P){1, 2}`). It makes a new object where a method call asks one, so it weighs a bit more,
  0.9 against 0.55 (by judgment: for Java's, TS's and Go's the pairs score the same for any weight
  from 0.25 to 5). Its
  arguments count as a call's; it is flat by default, as `call`. A bare initializer (`int[] a = {1, 2}`, a Go
  element's own `{...}`) is none, and Python's `Foo()` and Rust's tuple struct `P(1)` stay calls: no syntax tells
  them from a function's call.
- `jump`: `break`, `continue`, `throw`, and an early return: a return that isn't the last statement (comments aside)
  of its function's or lambda's body; the last one is a `statement`, but Java's `return switch (x) { ... };`, whose
  switch expression weighs for it as a switch statement in its place would (the language's `carried_return`
  hook); an early one is still a jump. So `if (x) return a; return b;` is a jump and a
  statement, and both returns of an `if / else` at the end are jumps.
- Each function's weight is kept apart, its nested functions' not in it; code outside any function belongs to its
  class, or else to its file. A declaration's own weight (`function`, `class`) is its own entry's; a lambda is not a
  function, its code belongs to the function around it.
- A docstring or a bare `...` (Python), a directive like `"use strict";` (TS) is no statement: an expression
  statement that is only a string (or Python's `...`) weighs nothing (the language's `exempt` hook). A Python
  docstring weighs in the comments instead ([The comments weight](#the-comments-weight)); a directive weighs nowhere.
- Names: a function or class is named by the classes and functions around it, `Outer.Inner.method`,
  `Outer.method.Local.run`. A Java constructor is `Class.Class`, an anonymous class `new Type` (its type arguments
  dropped: `Outer.method.new Runnable.run`), an enum constant's body the constant's name. Same-named entries of one
  kind in a file are one entry, their weights summed, its line the first one's: overloads, a class's constructors,
  two anonymous classes of a type in one method. The name is then a stable key for `show` and `diff` (an added overload is a
  heavier function, not a new one), and (path, kind, name) is unique, so the sort is total. A class and a method of
  the same name stay apart.
- A weight is an `f64`; the text report shows at most two decimals.

## The conditions

The `[conditions]` table's rules weigh what a node's kind alone doesn't tell, each as an existing category, so they
add no weight of their own. The example pairs asked for them; the tuning on them turned on
all but `nullish_operator`, which there only ties 11 pairs more; `nested_ternaries`, `free_default` and
`assignments` were asked for later, once the tuned pairs were read. A file on its own that leaves the table out has them all off.

- `casts`: a cast or type assertion is an `operator`, one per cast (no run): Java's and C's `(T) x`, TS's `x as T`,
  `x satisfies T`, `x!` and `<T>x` (TS only: TSX reads it as JSX), Rust's `x as T`, Go's `x.(T)`. A cast is
  something to read and check, as an arithmetic operator is; without the rule an unneeded cast weighs nothing.
  JavaScript and Python have none (Python's `cast(T, x)` is a call); Go's conversion `T(x)` is a call already.
- `let_patterns`: Rust's `let` condition, the pattern `if let` / `while let` (or a let chain's `let`) tests, is an
  `operator`, as the comparison it stands for (`if let Ok(_) = x` as `x.is_ok()` is a call) would be.
- `magic_numbers`: a number literal whose value is neither 0 nor 1 is an `access`, as the named constant in its
  place (`math.pi`) would be. The value is read past `_` separators, a radix prefix and a type suffix (`1_000`,
  `0x1`, `1u8`, `1.0f`, `1L`, `1j`), and past a sign C's grammar puts in the literal (`-1`); a negative one (`-2`) is its literal's, the `-` no operator. Every literal
  counts, a constant's definition and a case's label too.
- `nullish_operator`: `??` and `??=` are an `operator`, not a `bool_op`: a default value (as `unwrap_or` is a
  call), not a branch of control flow; `a ?? b ?? c` is 2.
- `nested_ternaries`: a ternary chained in a ternary's alternative (`a ? x : b ? y : z`, Python's `x if a else y if
  b else z`) nests, as one in its consequence does, rather than being a sibling, as an `else if` is: a nested
  ternary is one expression to unravel, where an `else if` chain is a list of statements. On by default.
- `free_default`: a switch's or match's default case weighs nothing, as an `else` weighs nothing: Java's `default`
  (`default:`, `default ->`, `case null, default`), TS's and JS's `default:`, Go's `default:` (of a switch or a select),
  C's `default:`, a bare `_` of Rust's `match` and Python's `match` (not one with a guard, `_ if x`). So a switch weighs
  less than the if-chain it replaces as long as a `case` weighs less than a `branch`, which the tuning keeps. Its body
  still nests. On by default.
- `assignments`: a plain `=` assignment is an `operator` too, as a compound one (`x += 1`) is, beside its
  statement: writing a variable is something to follow, so `case 1 -> x = f();` weighs more than the switch
  expression's `case 1 -> f();`, and `int x; x = 1;` more than `int x = 1;`. Each counts: `a = b = c` is 2. A
  declaration's initializer is none: Java's `int x = 1`, TS's and JS's `let` / `const` / `var`, Rust's `let`, Go's
  `:=` and `var x = 1`, C's `int x = 1`; nor is Python's `=`, which is its declaration too (only `+=` and its like weigh). Go's
  assignment is a statement of its own and stays one. On by default.

## The docs weight

A docs file's weight, a Markdown, a reStructuredText or an AsciiDoc file's, is its docs weight, a number of its own, never summed
with code's but on its scale (`markdown`, pure as the engine is: text in, weight out); the docs' number of `scan`,
`show` and `diff` weighs it, a file the `[files] docs_globs` match ([cli.md](cli.md)) and no test glob does: a docs
file under the test
globs (a test's snapshot or golden file, a README in `tests/`) is test prose, in no number, as test code's comments
are (below). Such files can outweigh a project's own docs: React's compiler snapshots (`__tests__/**/*.expect.md`)
were 318k of its 557k docs, Angular's test Markdown (`test/golden/*.md`, ...) about 317k of 851k.

```
docs weight = words × word + function words × function_word
            + punctuation runs × punctuation + symbol runs × symbol
                                  ([docs], 0.64, 0.32, 0.32 and 0.62 by default)
```

- A word is a run of Unicode letters and digits (any script: `мир` is one); a run of anything else that isn't a
  space is counted once however long, as punctuation or a symbol. One rule everywhere, prose, markup, inline code
  and fenced blocks alike, with no nesting: `a = b + c` is 5 runs, `Hello, world.` 4, `## Setup` 2,
  `**important**` 3, `a == b` 3, `snake_case`, `well-known` and `don't` 3 each, `foo(a, b);` 6. Markup is what a
  reader of the source reads too, so `**`, `##` and a fence's backticks are a run each.
- A punctuation run is prose's: sentence punctuation, quotes, parentheses, dashes and backticks, any of them
  together (`,`, `),`, `."`, `...`, `—`, inline code's `` ` ``), or a lone `-` or `_` (`well-known`, `snake_case`).
  Any other run is a symbol: an operator (`=`, `->`, `!=`, `--`), markup (`**`, `##`, `](`, a table's `|`), a
  path's `/`, and a punctuation run with one of them in it (`)[`). So `Hello, world.` is 2 words and 2
  punctuation runs, `a -> b` 2 words and a symbol, `foo(a, b);` 3 words and 3 punctuation runs.
- A function word is a word of `[docs] function_words`, whatever its case, in prose: the glue between the words
  that say something (`a`, `the`, `of`, `is`, `it`, a possessive's `s`), not a word of logic (`and`, `or`, `not`,
  `if`, `each`), which a description of code leans on. In inline code and in fenced blocks it is a word, a name or
  a keyword there (`` `a` ``, `for`, `in`). A word only, not a part of one: `the_end` has `the`, `theory` none. The
  defaults' list is English; a list of one's own replaces it whole, so another language's words go in beside the
  English ones.
- Outside fenced blocks only (fenced by three or more backticks or tildes, at any indentation, as a list's are; one
  never closed runs to the end), which a scanner of lines tells:
  - YAML front matter at the top (a first line `---`, up to the next `---` or `...`) weighs nothing, nor does a
    table's separator line (`|---|:--:|`, `--- | ---`: cells of hyphens, a colon at either end, between pipes; a
    `---` without a pipe is a rule, a symbol run), nor a `<!-- comment -->`, inline or over lines. A `<!--` with no
    `-->` after it is text, and so is one in inline code (it is code there), though a `-->` comes after.
  - A URL is one word: an absolute one (`scheme://...`, bare or in `<...>`), up to a space or a character no URL
    holds unescaped (`<`, `>`, `"`, `` ` ``, `|`, `[`, `]`), less a sentence's trailing punctuation and an
    unmatched `)`; and a link's or an image's destination whatever its form, the text after `](` up to a space or
    its `)` (a `(` in it matched by a `)`), or in `<...>`. A relative link is one reference as an absolute one is,
    and a doc's renaming rewrites the links to it without weighing their paths' lengths:
    `[weight.md](weight.md)` is 3 words (`weight`, `md`, the destination), 2 symbol runs (`[`, `](`) and 2
    punctuation runs (`.`, `)`), `see https://a.org/b.` 2 words and 1 punctuation run. A link's title is text;
    a reference definition (`[id]: url`) is text but its absolute URL; inline code's `](` is code, no link.
- A reStructuredText file (`.rst`, `.rest`, and `.txt`: Sphinx's sources, Django's docs) is counted by the same
  runs, its lines told apart by reST's rules where Markdown's are (`rst`): no front matter, `<!-- -->`, fence nor
  `](`, but
  - a comment (`..` and text that is no directive, hyperlink target, footnote, citation nor substitution, with the
    lines indented under it: an SPDX line, a note to the editors) weighs nothing, as a `<!-- -->` does;
  - a literal block (the lines indented under a paragraph that ends in `::`, or under a `::` alone), a code
    directive's content (`code-block`, `code`, `sourcecode`, `parsed-literal`, `doctest`, `testcode`, ...; its
    options are text) and a doctest block (from `>>>` up to a blank line) are code, as a fenced block is;
  - markup is a symbol run each, as Markdown's is, though made of prose's punctuation: a directive's `..` and `::`
    (`.. note::` is 2 symbol runs and a word), a role's colons (`` :ref:`x` `` is `:`, `ref`, ``:` ``, `x` and a
    backtick's punctuation), a field's (`:param x:`, a directive's option `:maxdepth:`), and a section's adornment
    line, three or more of one punctuation character at a line's start (`=====`, `~~~~`, `"""""`);
  - a table's borders weigh nothing, as a Markdown table's separator line does: a grid table's (`+---+===+`) and a
    simple table's (`===  ===`, two columns or more; one column's is an adornment line);
  - a URL is one word, and so is a link's embedded URI whatever its form (`` `a <b.html>`_ ``); only double
    backticks are inline code (``` ``x`` ```), a single pair being interpreted text, prose.

  `.txt` is reST because Sphinx's sources often are: Django's docs are 682 `.txt` files under `docs/`, and Linux
  has 743 plain-text ones under `Documentation/` beside its 4,121 `.rst`. The default globs take a `.txt` there
  only, as one elsewhere is seldom docs (`requirements.txt`, `CMakeLists.txt`, test data). A plain text read as reST
  weighs as text; read as Markdown, a `~~~~` line (an underline, a diagram's) opens a fenced block that may run to
  the file's end. With them (2026-11-26) Django's docs number went from 369 to 682k, Flask's from 286 to 56k,
  Linux's from 2.7k to 3.6M; reST's rules move a file's weight by a percent or two from what Markdown's give.
- An AsciiDoc file (`.adoc`, `.asciidoc`: git's manual pages, Spring's, Quarkus' and Hibernate's guides) is
  counted by the same runs, its lines told apart by AsciiDoc's rules (`asciidoc`), as a reST file's are by reST's:
  - a comment (a line starting `//`, a `////` block up to the same line) weighs nothing;
  - a listing, literal, passthrough or fenced block (`----`, `....`, `++++`, as many as it opened with, or
    ```` ``` ````), a literal paragraph (indented at a block's start, up to a blank line; not a list item nor a
    description list's text under its term, git's tab-indented option texts) and a paragraph of a code style
    (`[source,java]`, `[listing]`, `[literal]`) are code, as a fenced block is;
  - markup is a symbol run each, though made of prose's punctuation: a block's delimiter (`----`, `....`,
    `====`), a section title's underline (`-`, `=`, `~`, `^` or `+` repeated under a title as long, give or take
    one: a `----` under one is no listing), a description list's `::`, `:::`, `::::` or `;;`, an attribute
    entry's colons (`:toc: left`), a block title's `.` (`.Example`), a list continuation's `+` and a macro's
    colons (`linkgit:`, `include::`, `xref:`);
  - a table's border (`|===`) weighs nothing;
  - a URL is one word, and so are a macro's target (`linkgit:git-add[1]` is `linkgit`, `:`, the target, `[`, `1`,
    `]`) and a cross reference's id (`<<id,text>>`); backticks are inline code, as Markdown's.

  git moved its docs from `.txt` to `.adoc` (2025-02-14), so the reST rules no longer read them: its docs number
  had fallen from about 570k to 8k. With AsciiDoc in the globs (2026-11-26) git's went from 9.4k to 632k, Spring
  Framework's from 5k to 320k, Quarkus' from 28k to 752k, Hibernate ORM's from 7k to 252k, Micronaut's from 11k
  to 133k; AsciiDoc's rules give a file a percent or two less than Markdown's would (git's 958 files 623k against
  630k), a cross reference's or a macro's target being a word, not a path's words.
- A docs file is one row, its weight its file's (no functions nor classes), and always clean: any text is
  Markdown, reST or AsciiDoc.
- The defaults put the docs on the code's scale, a weight being what a text costs to read and keep: a
  description that says all a snippet does, no more, weighs what the snippet does. They are fitted on such pairs,
  code-snippet-pairs' `calib/`: seven snippets of Rust, Python and Java (50 to 170), each described in prose, six
  of them also as a terse spec of expressions, operators and tables. At 1 each a description weighed twice its
  code; one factor for all four would be 0.49. The thirteen descriptions pin down the scale, not each kind's share:
  fitted apart, the shares swing (punctuation from 0.6 to 2 with inline code's backticks standing in for code,
  function words near 0), for little gain. So the shares are a judgement: a function word and a punctuation run
  weigh half a word, glue a reader passes over, and the fit sets the scale and the symbol, 0.64 a word and 0.62 a
  symbol. A third of prose's words are function words, so a word that says something weighs more than one factor
  would make it. The descriptions miss by 10% on the mean, 33% at worst (the terse `Retry.apply`); `## Setup` is
  1.26, `the end of it` 1.6, `a -> b` 1.58.

## The comments weight

A code file's comments and Python's docstrings weigh in a number of their own, the comments' (`--comments`), never
summed with the code's, the docs' or the tests', so those stay what they were and a commit that only adds comments
is `code 0`, `docs 0`, `tests 0` with a comments delta. A comment costs reading and keeping in step with the code it
sits in, but it is text, not code: it weighs as a Markdown file's text does.

- What weighs: every comment, line or block, doc comment or not, in every language (each one's comment kinds in
  [languages.md](languages.md)), and every Python string statement (a module's, a class's or a function's docstring,
  or a string standing as a comment). Whether a comment is a line or a block is syntax, not meaning: Rust's `///`,
  Go's doc comments and Python's are line comments, and a block is as often a license or commented-out code.
- How: the comment's text without its markers (`//`, `///`, `//!`, `#`, `/*`, `/**`, `/*!`, `*/`, a block's `*`
  that begins each line after its first, a docstring's quotes and prefix) weighed by the docs weight's counter and
  its `[docs]` weights, no factor of its own: the `[docs]` defaults are fitted so that a description weighs what
  the code it describes does ([The docs weight](#the-docs-weight)), which is what a comment is, and there is nothing
  to fit other weights on. The same line scanner, so a doc
  comment's fenced example is a fenced block and a URL one word; but a comment is no Markdown file: no front matter,
  and a `<!--` is text. Commented-out code and `@param` tags weigh as any text (`foo(a, b);` is 3 words and 3
  punctuation runs), with no special case.
- Pragmas weigh as text too (`# noqa`, `// @ts-ignore`, `// eslint-disable-next-line`, `//go:generate`): a reader
  reads them, they often carry their reason in the same comment, and a list of them per language is upkeep that is
  never complete, for a few words each. A file-level one before a blank line at the top (`//go:build`, `/*
  eslint-disable */`) is in the file's header, which weighs nothing.
- Flat, as Markdown: no nesting factor. A comment in a deep loop costs what one at the top costs.
- Where: a comment is its function's, class's or file's entry, as the code is, so the comments' report (`--comments`)
  has a row per function, class and file. A comment right before a declaration is the declaration's: nothing but
  comments, attributes or decorators between them, no blank line (Javadoc, JSDoc, `///`, a Go doc comment, a `#`
  block over a `def`), the declaration the first function or class the next code declares, not inside a nesting
  construct (`export function f`, a decorated `def`, `const f = () => ...`). A comment after code on its own line
  (`x = 1  # why`) is that line's. A docstring is its scope's, never the next declaration's.
- What weighs nothing:
  - A file's header: the comments at its top, before its first code, up to the last blank line between them and it
    (a license, a copyright, an SPDX line, a generated-file banner); all of them in a file of no code. One per
    file, it swells a scan and says nothing of the code; in a diff it shows only when a file is added. A doc comment
    touching the first declaration still weighs, and so do Rust's `//!` and `/*!` (the module's docs, as Python's
    module docstring is code and weighs too).
  - A shebang (`#!/usr/bin/env python3`), a comment in Python's grammar.
  - Test code's comments: a test file's, a Rust `#[cfg(test)]` or `#[test]` item's, and one right before such an
    item. The tests' number is the test code's; its comments are in no number.
  - A docs file's: it has no comments number (0), its comments (`<!-- -->`, reST's `..`, AsciiDoc's `//`) staying nothing.
