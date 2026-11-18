# The weight

```
weight(node) = base[category(node)] × nest(min(depth, max_depth))
total        = Σ weight(node) over the nodes that have a category
```

- `depth` is the number of nesting constructs around the node: branch, loop, `try` / `catch`, lambda, nested
  function or class. Plain tree levels don't count (`a.b().c()` is not deep).
- Only a construct's body is one level deeper; its head (a condition, a loop's header, a catch's parameter, a case's
  label, try's resources) stays at the construct's depth, so the same condition weighs the same wherever it is. A
  `for`'s condition and update stay at the loop's depth although they run on every pass. A catch or finally clause
  is at its try's depth (it is beside the try's body, not in it), its block one deeper.
- `else if` chains and `switch` / `match` cases are siblings of one level, not deeper ones. The switch itself is not
  a nesting construct; each case is (a branch, its statements one deeper). An `if` directly in an if's `alternative`
  stays at that if's depth; an `if` inside an `else { }` block is nested. A ternary chained in a ternary's
  alternative (`a ? x : b ? y : z`) is a sibling the same way.
- A function or class declared inside a function (a local or anonymous class, its methods, a nested function)
  nests its body; a member class or method doesn't, whatever the depth of classes around it. Depth isn't reset at a
  function: a method of a class in an `if` in a method counts both.
- `bool_op`: one per run of like `&&` / `||` operators, the first run counted too, not one per operator: `a && b`
  and `a && b && c` are 1, `a && b || c` is 2, `(a && b) || (c && d)` is 3. A logical expression counts one when its
  operator differs from that of the logical expression it is an operand of, or it is no operand of one; parentheses
  are see-through, anything else (`!`, a call) starts a new run. Every logical expression counts, in a condition or
  not. Java's `&` / `|` are not logical (they can't be told from bitwise ones without types): they are bitwise
  operators. The rule is the language's `operator` hook, whatever its node kind's table entry is.
- `operator`: one per run of like operators, by the `bool_op` rule (parentheses see-through), so a long flat sum is
  one and a mix of precedences is more: `a = b` is none, `a + b + c` and `a + b - c` are 1, `(b + c) * d` and
  `a * b + c` are 2, `a * b + c * d` is 3. Like operators are those of one precedence level: `+ -`, `* / %` (Python's
  `//` and `@`), `**`, the shifts, the comparisons (`==`, `<`, ..., `instanceof`, `in`, `is`; a chained Python
  comparison `a < b < c` is one), `&`, `|`, `^`, and the unary ones (`-x`, `!x`, `not x`, `~x`, TS's `typeof`, `++`
  and `--`). A compound assignment (`x += 1`) is its operator, so `x += b + c` is one run, as `x = x + b + c`; a
  plain `=` is none, the statement already weighs it. A negative number (`-1`) is a literal, no operator. A Rust
  dereference (`*x`) and a borrow (`&x`) are none, and so are Go's (`*p`, `&x`) and its receive (`<-ch`). `&&=`,
  `||=`, `??=` are logical: `bool_op`.
- `access`: one per field access or index, so `obj.a` is 1, `obj.a.b` and `a[i][j]` are 2. A call's callee is the
  call's, no access: `obj.f()` is a call, `obj.a.f()` a call and an access (`obj.a`), as Java's grammar has it (its
  method call holds its object, no field access around the name); TS's `new a.B()` and a JSX element's `<a.B />`
  the same. An access to `this` / `super` (Java, TS), `self` / `cls` (Python), `self` (Rust) or a method's receiver
  (Go's `r.x` in `func (r *T) M()`) is none, as a bare `x` (Java's implicit `this`) is none; `this.a.b` is 1. A
  qualified name in a type or an import is no access: the grammars parse those as other kinds (`scoped_identifier`,
  `nested_type_identifier`, `dotted_name`), and a Python annotation's, which is an expression, is exempt. Both are the language's `exempt` hook.
- `jump`: `break`, `continue`, `throw`, and an early return: a return that isn't the last statement (comments aside)
  of its function's or lambda's body; the last one is a `statement`. So `if (x) return a; return b;` is a jump and a
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
