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
  so a tuning on pairs of code (a snippet and its refactoring, milestone 14) can weigh the two apart. By default it
  weighs less than a branch, and the default case nothing (`[conditions] free_default`), as an `else`: a switch
  weighs less than the if-chain it replaces.
- `bool_op`: one per `&&` / `||` operator: `a && b` is 1, `a && b && c` and `a && b || c` are 2,
  `(a && b) || (c && d)` is 3. Every logical operator counts, in a condition or not; like operators in a row are no
  run that counts once (milestone 22: every operator is something to read). Java's `&` / `|` are not logical (they can't be told from bitwise ones without types): they are bitwise
  operators. The rule is the language's `operator` hook, whatever its node kind's table entry is.
- `operator`: one per operator, by the `bool_op` rule: `a + b` is 1, `a + b + c` and `(b + c) * d` are 2,
  `a * b + c * d` is 3. The operators are the arithmetic ones (`+ -`, `* / %`, Python's `//` and `@`, `**`), the
  shifts, the comparisons (`==`, `<`, ..., `instanceof`, `in`, `is`; a chained Python comparison `a < b < c` is one
  per comparison, 2), `&`, `|`, `^`, and the unary ones (`-x`, `!x`, `not x`, `~x`, TS's `typeof`, `++` and `--`;
  `!!x` is 2). A compound assignment (`x += 1`) is its operator, so `x += b + c` is 2; a plain `=` is none, the
  statement already weighing it, but under `[conditions] assignments` (on by default), where it is one too. A negative number (`-1`) is a literal, no operator. A Rust
  dereference (`*x`) and a borrow (`&x`) are none, and so are Go's (`*p`, `&x`) and its receive (`<-ch`). `&&=`,
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
  `new A` without parentheses), nor of what is no call (Java's enum constant `X(1)`, Python's class bases). It is 0.05
  by default, little of its own, what is in an argument weighs (milestone 13); it is a category for the pairs, where a call lighter than the operators it replaces has 0 or 1 arguments (`f.is_some()`
  for `f != None`) and a heavier one 2 (`Objects.equals(a, b)` for `a == b`). The engine counts them
  (`lang::arguments`), the same for every language.
- `macro`: a macro invocation (Rust's `println!(...)`, `vec![...]`, `matches!(...)`), apart from `call`. Its
  arguments are a token tree tree-sitter doesn't parse, so what is in it weighs nothing, nor does it count any
  `argument`: the macro weighs for its content. It is flat by default, as `call`. The pairs (milestone 17) replace
  `matches!(x, P)` by a call (`x.is_ipv4()`), which with a macro weighing a call's 0.55 came out heavier.
- `constructor`: a constructor, apart from `call`: Java's `new Foo()`, an array's `new int[3]` / `new T[] {...}`
  and `this(...)` / `super(...)`, TS's and JS's `new`, Rust's struct literal (`P { x: 1 }`), Go's composite
  literal (`P{X: 1}`, `[]int{1}`). It makes a new object where a method call asks one, so it weighs a bit more,
  0.9 against 0.55 (milestone 21, by judgment: for Java's, TS's and Go's the pairs score the same for any weight
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
  statement that is only a string (or Python's `...`) weighs nothing (the language's `exempt` hook).
- Names: a function or class is named by the classes and functions around it, `Outer.Inner.method`,
  `Outer.method.Local.run`. A Java constructor is `Class.Class`, an anonymous class `new Type` (its type arguments
  dropped: `Outer.method.new Runnable.run`), an enum constant's body the constant's name. Same-named entries of one
  kind in a file are one entry, their weights summed, its line the first one's: overloads, a class's constructors,
  two anonymous classes of a type in one method. The name is then a stable key for `commit` (an added overload is a
  heavier function, not a new one), and (path, kind, name) is unique, so the sort is total. A class and a method of
  the same name stay apart.
- A weight is an `f64`; the text report shows at most two decimals.

## The conditions

The `[conditions]` table's rules weigh what a node's kind alone doesn't tell, each as an existing category, so they
add no weight of their own. The pairs of milestone 15 asked for them; the tuning on them (milestone 16) turned on all
but `nullish_operator`, which there only ties 11 pairs more; `nested_ternaries` and `free_default` were asked for
(milestone 18), `assignments` too (milestone 19). A file on its own that leaves the table out has them all off.

- `casts`: a cast or type assertion is an `operator`, one per cast (no run): Java's `(T) x`, TS's `x as T`,
  `x satisfies T`, `x!` and `<T>x` (TS only: TSX reads it as JSX), Rust's `x as T`, Go's `x.(T)`. A cast is
  something to read and check, as an arithmetic operator is; without the rule an unneeded cast weighs nothing.
  JavaScript and Python have none (Python's `cast(T, x)` is a call); Go's conversion `T(x)` is a call already.
- `let_patterns`: Rust's `let` condition, the pattern `if let` / `while let` (or a let chain's `let`) tests, is an
  `operator`, as the comparison it stands for (`if let Ok(_) = x` as `x.is_ok()` is a call) would be.
- `magic_numbers`: a number literal whose value is neither 0 nor 1 is an `access`, as the named constant in its
  place (`math.pi`) would be. The value is read past `_` separators, a radix prefix and a type suffix (`1_000`,
  `0x1`, `1u8`, `1.0f`, `1L`, `1j`); a negative one (`-2`) is its literal's, the `-` no operator. Every literal
  counts, a constant's definition and a case's label too.
- `nullish_operator`: `??` and `??=` are an `operator`, not a `bool_op`: a default value (as `unwrap_or` is a
  call), not a branch of control flow; `a ?? b ?? c` is 2.
- `nested_ternaries`: a ternary chained in a ternary's alternative (`a ? x : b ? y : z`, Python's `x if a else y if
  b else z`) nests, as one in its consequence does, rather than being a sibling, as an `else if` is: a nested
  ternary is one expression to unravel, where an `else if` chain is a list of statements. On by default.
- `free_default`: a switch's or match's default case weighs nothing, as an `else` weighs nothing: Java's `default`
  (`default:`, `default ->`), TS's and JS's `default:`, Go's `default:` (of a switch or a select), a bare `_` of
  Rust's `match` and Python's `match` (not one with a guard, `_ if x`). So a switch weighs less than the if-chain it
  replaces as long as a `case` weighs less than a `branch`, which the tuning keeps. Its body still nests. On by
  default.
- `assignments`: a plain `=` assignment is an `operator` too, as a compound one (`x += 1`) is, beside its
  statement: writing a variable is something to follow, so `case 1 -> x = f();` weighs more than the switch
  expression's `case 1 -> f();`, and `int x; x = 1;` more than `int x = 1;`. Each counts: `a = b = c` is 2. A
  declaration's initializer is none: Java's `int x = 1`, TS's and JS's `let` / `const` / `var`, Rust's `let`, Go's
  `:=` and `var x = 1`; nor is Python's `=`, which is its declaration too (only `+=` and its like weigh). Go's
  assignment is a statement of its own and stays one. On by default.
