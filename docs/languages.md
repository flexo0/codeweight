# The languages

What each language's elements are, where it adds to or differs from the general rules ([weight.md](weight.md)),
which are Java's as they stand. Every constructor is a `constructor`: Java's `new Foo()`, an array's `new int[3]` /
`new Integer[] {1, 2}` (a bare `{1, 2}` initializer is none) and `this(...)` / `super(...)`, TS's and JS's `new`,
Rust's struct literal, Go's composite literal, C's compound literal; Python's `Foo()` stays a `call`, no syntax telling it from a
function's. Each language's comment kinds weigh in the comments ([weight.md](weight.md)), the same way in all, their
markers stripped by one rule (`//`, `///`, `//!`, `#`, `/*`, `/**`, `/*!`, `*/`, a block's leading `*`). Java's are
`line_comment` and `block_comment`, a Javadoc a block; an annotation is in its declaration's modifiers, so a Javadoc
before `@Override` is the method's.

## TypeScript, TSX and JavaScript

- A function is a function, generator or method declaration, an overload's or an interface's signature
  (`function f(): void;`, `m(): void;`, `abstract m(): void;`): overloads are one entry, summed, as in Java. A
  constructor is `Class.constructor`, a getter and its setter one entry. A class, an interface, an enum and a
  namespace (`namespace N`, `declare module "m"`) are classes; a class expression is named by its own name, else the
  variable it is bound to, else `class`.
- An arrow or a function expression is a lambda, but where it is bound to a name at the file's level (or a
  namespace's) or as a class field's value: `export const f = () => ...` is the function `f`, a field
  `onClick = () => ...` the function `A.onClick`, weighed as a function and nesting as one (a top-level one's body is
  not deeper). Inside a function, `const cb = () => ...` stays a lambda in it, as Java's `Runnable r = () -> ...`
  does: a handler is part of its component. So `function inner() {}` in a function is a nested function of its own
  but `const inner = () => {}` is not. Only a plain identifier names one (not a destructuring); a wrapped value
  (`memo(() => ...)`, `(() => ...) as F`) and `export default () => ...` are lambdas.
- `if` nests its consequence; its `else` is a clause beside it, whose block is deeper but whose `if` (`else if`)
  stays at the depth. `switch` cases as in Java. `for`, `for ... in / of`, `while`, `do` are loops.
- `bool_op` counts each `&&`, `||` and `??`, `&&=`, `||=` and `??=` too;
  `??` and `??=` are an `operator` under `[conditions] nullish_operator` ([weight.md](weight.md)).
- `operator` counts `===` / `!==`, `in` and `instanceof` as comparisons, `typeof`, `void` and `delete` as unary
  operators. `access` counts a member (`a.b`, `a?.b`) and a subscript (`a[i]`).
- A decorator weighs nothing of its own, as a Java annotation; a call in it (`@Input()`, `@Component({...})`) is a
  call, where the grammar puts it: a method's or a field's decorator in the class, a parameter's in the method,
  a class's in the class, but `@d export class` in the file (the export holds it).
- TSX: a JSX element (`jsx_element`, a fragment too, and `jsx_self_closing_element`) is a call, as it compiles to
  one; its attributes and `{...}` weigh what is in them. JSX is no nesting construct.
- A namespace's statement (the grammar wraps `namespace N {}` in an expression statement) weighs nothing.
- JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`; `tree-sitter-javascript`, the grammar TypeScript's is built on) is
  weighed as TypeScript, its table, nesting and naming taken from `typescript.rs`, but for the type kinds its grammar
  hasn't: an interface, an enum, an abstract class, a namespace, a signature, a property signature, a `type` alias.
  Its class field is `field_definition` (TypeScript's `public_field_definition`, its name the `property` field), a
  field all the same: `onClick = () => ...` is the function `A.onClick`. `using x = ...` (`using_declaration`, which
  TypeScript's grammar doesn't parse yet) is a statement, as `let`. The grammar reads JSX in any file, so a JSX
  element is a call in a `.js` file as in a `.jsx` one. Its weights-file table is its own ([config.md](config.md)).
- Flow: tree-sitter-javascript has no Flow (`import type`, annotations, `type T = {...}`, `?T`), so a Flow-typed
  file is mostly errors (React's 1,146 `@flow` files: 61% of their text in ERROR nodes, their code a third lighter
  than read by TSX's grammar, more than half their functions lost). A JavaScript file that doesn't parse cleanly is
  parsed again by TSX's grammar, which reads most of Flow (its `?T`, `*`, `(x: T)` cast and `%checks` too, 9.7% of
  those files' text left in errors), and the parse with less of the text in ERROR nodes is weighed, JavaScript's on
  a tie: a clean file is parsed once and stays JavaScript's. No `@flow` pragma is looked at: React's Babel reads Flow in a
  file without one too, and a file TSX's grammar reads worse (two of React's `@flow` ones) keeps JavaScript's. Read
  by TSX's grammar (the language `flow`, `src/lang/javascript.rs`), a file is weighed by TSX's table, Flow's types
  as TypeScript's (a `type` alias and an object type's members are statements, a `declare function` a function),
  under `[languages.javascript]`'s weights. On React (`--exclude flow-typed/`) the syntax-error warnings went from
  913 to 266, the code from 789,797 to 902,469. A commit whose before parses cleanly and whose after doesn't (its
  first annotation) changes grammar between its sides: `a < b > (c)` is two comparisons to JavaScript's, a call to
  TSX's.
- Known gaps (Flow): what TSX's grammar lacks stays a syntax error, weighed as recovered, with a warning: an
  unparenthesized function type's parameter (`fetch: I => Thenable<V>`), a variance sigil (`+x`, `<+T>`),
  `opaque type`, `declare export`, a generic function type with unnamed parameters (`<T>(?() => T) => void`), Flow's
  `component`, `hook` and `match`. Where the recovery fails as a whole, the file is one error under both grammars
  (5 of React's, `ReactFiberHooks.js` among them). A Flow cast `(x: T)` is a parenthesized expression to the
  grammar, no cast under `[conditions] casts`.
- Comments: `comment`, a line or a block, a JSX `{/* */}` too (`html_comment`, the legacy `<!--`, is none). A
  class member's decorator is beside the member, so a comment before it is the member's.

## Python

- A `def` is a function, a method of the class around it, a nested function of the function around it (its body
  nests); a lambda is a lambda, even bound to a name. A class is `class`.
- `if` nests its body; `elif` (a branch) and `else` are its children beside the body, so a chain is siblings, each
  one's body deeper. `x if c else y` is a branch, its value and alternative deeper, its condition not; a chained one
  (`a if c else b if d else e`) is a sibling. The grammar gives that expression no fields, so its children are
  matched by their place (`Children::Nth`).
- `for` and `while` are loops, their `else` beside the body, its block deeper. `try` and `with` are `try`, each
  `except` a catch.
- A comprehension (list, set, dict, generator): each `for` is a loop, each `if` a branch; its element and its `if`s
  are one deeper, its `for`s (their iterables) at its depth. Its element (a dict comprehension's pair) is a
  statement, as the loop's statement making it would be; a lambda's body is one too ([weight.md](weight.md)).
- `match` is no nesting construct; each `case` is a `case`, its body deeper, its guard (`case x if c`) no branch of
  its own (exempt), its condition at the case's depth.
- `and` / `or` are the logical operators (`not` starts a new run: it is a unary `operator`). `raise` is a jump.
- `access` counts an attribute (`a.b`) and a subscript (`a[i]`), not one in an annotation (`x: os.PathLike`,
  `Dict[str, int]`), which is a type; a parameter's default is no annotation.
- A decorator weighs nothing of its own; a call in it (`@app.route("/")`) is a call of the class or file around the
  decorated definition (the grammar puts the decorators beside the definition, not in it).
- Comments: `comment`, and every string statement (a module's, a class's or a function's docstring, or a string
  standing as a comment elsewhere), its prefix (`r`, `f`, ...) and quotes stripped, a concatenated one's parts each;
  a docstring is its scope's. A shebang is a comment in the grammar, and weighs nothing. A `#` block over a
  decorated `def` is the function's (the decorators are in the definition).

## Rust

- A `fn` is a function: a method of the class around it, a nested function of the function around it (its body
  nests), a trait's signature (`fn a(&self);`) a function as TS's. A struct, an enum, a union, a trait and a module
  with a body are classes; `mod a;` (only where a file is) weighs nothing and is no class.
- An `impl` block (`impl Foo`, `impl<T> Trait for Foo<T>`, `impl Trait for &mut a::Foo`) is a class named by its type
  (its type arguments, references and path dropped), so its methods are `Foo.method`, and a type's struct and every
  `impl` of it in a file are one class (same-named entries of one kind are one). An `impl` weighs nothing of its
  own: the type's declaration does, so a struct with three `impl` blocks weighs one class, and an `impl` of a type
  declared elsewhere only its contents. A type that is no path (`impl T for [u8]`) is named by its text.
- A closure is a lambda, even bound to a name (`let f = |x| ...`), its body deeper; an expression body (`|x| x + 1`)
  is a statement, as a Java lambda's ([weight.md](weight.md)).
- `if` (`if let` too) nests its consequence; its `else` is a clause beside it, as TS's: its block deeper, its `if`
  (`else if`) at the depth. `match` is no nesting construct; each arm is a `case`, its pattern and guard at the arm's
  depth, its value deeper and a statement when it is no block (`=> f(x)` weighs as `=> { f(x) }`), nor an
  expression ending in one (`if`, `match`, a loop) or a jump. `for`, `while` (`while let`), `loop` are loops; a label (`'outer:`) weighs nothing.
- `&&` / `||` are the logical operators; a let chain (`if let Some(x) = a && x > 0`) is a `&&` per link. `-x` and
  `!x` are unary operators; `*x` and `&x` are none. `access` counts a field (`a.b`, `a.0`) and an index (`a[i]`); a
  method call's callee (`a.f()`) is the call's, a path (`a::b`) no access.
- A call is a `call` (a tuple struct's `P(1)` too, which parses as one); a struct literal (`P { x: 1 }`) is a
  `constructor`; a macro invocation (`println!(...)`, `vec![...]`) is a `macro`, a category of its own. `break`, `continue`, `?` (an early return
  of the error) and an early `return` are jumps.
- Statements: an expression statement, `let`, `const`, `static`, a `type` alias, a trait's associated type, a
  struct's field and an enum's variant (as Java's fields and enum constants). `use` and `extern crate` weigh nothing,
  as an import.
- Rust's blocks are expressions, so the grammar wraps a statement around any expression in a block's statement
  place: `g();`, but also an `if`, a `match`, a loop, a block, `return x;`, `break;`. That wrapper weighs nothing
  around a block, an expression ending in one or a jump (the language's `exempt`): an `if` is a branch, `return x;`
  a statement (or a jump), `break;` a jump, as in Java. The early return looks through it (the language's
  `wrappers`): `return x;` as the last statement of its body is the statement wrapping it, so it is no jump.
- A block's tail expression (its value, without `;`) is a statement (the language's `also_statement` hook, which
  weighs the block a statement beside its own category, at the block's depth, the tail's too), so `fn f() -> i32 { a + b }`
  weighs as Java's `return a + b;`, `{ g() }` as `g();`. A tail jump (`return x`, `break`) weighs as itself, not
  twice. The same holds in any block: an `if`'s, an arm's, a closure's, `unsafe { }`'s.
- Known gaps: a macro's arguments are a token tree, not parsed, so only the macro weighs (what is inside
  `vec![f(x), g(y)]` or `assert_eq!(...)` weighs nothing), and a `macro_rules!` definition weighs nothing.
  `let ... else { }` is no branch, its block not deeper; an `async`
  block is no lambda.
- Tests inside a source file (the `tests` hook): an item marked `#[cfg(test)]`, `#[test]` or `#[...::test]`
  (`tokio::test`), the attribute among the attributes and comments right before it, is test code with all in it
  (a `mod tests`, its functions); a file marked `#![cfg(test)]` is all test code. Its entries are test entries
  (a test and a non-test one of one name are two, so `#[cfg(test)] fn f` and `#[cfg(not(test))] fn f` stay
  apart), its code outside any class the file's test code; the code's number (`scan`, `show`, `diff`,
  `--code` alone) leaves it out, the tests' (`--tests` alone) weighs it alone, its delta for a commit. Left as it is: a marked item that is no class or function
  inside a class or function (`#[cfg(test)] const` in an `impl`) weighs in its class's or function's code; a
  `mod tests;` whose file is `tests.rs` marks that file only in its parent, so `tests.rs` is code unless a test
  glob or its own `#![cfg(test)]` says otherwise.
- Comments: `line_comment` and `block_comment`, doc comments (`///`, `/** */`, `//!`, `/*! */`) too; an attribute
  (`#[...]`) between a doc comment and its item keeps it the item's. `//!` and `/*!` document the module from inside,
  so at the file's top they are no header and weigh. A test item's comments weigh nothing, and so does a comment
  right before one (`// ...` over `#[cfg(test)] mod tests`).

## Go

- A `func` declaration is a function; a method is named by its receiver's type and its name (`T.M` for
  `func (r *T) M()` or `func (T) M()`, its pointer and type arguments dropped: `func (l *List[T]) Push` is
  `List.Push`), as Rust's `impl` names them. A method is no member of a class in the tree (it is declared at the
  file's level, maybe in another file than its type), so `T.M` is an entry of its own, its class `T` only where `T`
  is declared.
- A type definition (`type_spec`) is a class: a struct, an interface, and any other defined type (`type Celsius
  float64`, `type HandlerFunc func(...)`), as any of them can have methods and `type Color int` is Go's enum; the
  tree can't tell a struct type from another anyway (`type R bufio.Reader` is one). An alias (`type A = B`) is a
  statement, as Rust's `type`. A struct's field (an embedded one too) is a statement; a named interface's method
  (`M() int`) a function, `I.M`, as Rust's trait signature; an embedded interface or a constraint (`~int | string`)
  weighs nothing. An anonymous struct or interface (`var s struct{...}`, a parameter's `interface{ Close() error }`)
  is no class: its fields are statements where it is, its methods nothing (no entry: they'd be functions nested in
  the function around them).
- A `func` literal is a lambda, even bound to a name (`var handler = func() {...}`, `f := func() {...}`), its body
  deeper.
- `if` nests its consequence and its alternative; an `if` in the alternative (`else if`) stays at its depth, as
  Java's; its initializer (`if err := f(); err != nil`) is at the `if`'s depth, a statement. `for` is the only loop
  (a three-clause, a `range`, a bare `for {}`). A `switch`, a type switch and a `select` are no nesting constructs;
  each case (`default` too) is a `case`, its values (a select case's send or receive, which weigh nothing, as a
  label) at its depth, its statements deeper. No `try`: `defer` and `go` are statements, their call a call. A
  composite literal (`P{X: 1}`, `[]int{1, 2}`, `map[K]V{}`) is a `constructor`, as Java's `new` is; an element's
  own `{...}` inside one is none.
- `break`, `continue`, `goto`, `fallthrough` and an early `return` are jumps; a label (`L:`) weighs nothing, and a
  labeled `return` at a body's end is its last statement. The grammar puts a list of statements between a block and
  its statements (`statement_list`), which the early return looks through (the language's `lists`).
- Statements: an expression statement, an assignment, `:=`, each `var` / `const` spec (a `const (...)` block of
  five is five, as Java's enum constants), a send (`ch <- v`), `defer`, `go`, an alias, a field. `i++`, `i--` and a
  compound assignment (`x += 1`) are statements of their own in the grammar, no expression in one: they weigh a
  statement and an `operator` (the `also_statement` hook), as Java's `x++;` (a plain `x = 1` its statement, and an
  `operator` beside it under `[conditions] assignments`); in a `for` clause's update
  (`for i := 0; i < n; i++`) only the operator, as Java's update is no statement. The update is the loop's head:
  any statement there weighs none (`for ...; i, j = i+1, j-1`). The initializer (`i := 0`) is a statement, as
  Java's `int i = 0`.
- `&&` / `||` are the logical operators. `&^` (and not) is a bitwise and, `^x` a unary operator; `*p`, `&x` and a
  receive (`<-ch`) are none. `access` counts a selector (`a.b`), an index and a slice (`a[i:j]`); a call's callee
  (`a.f()`, `fmt.Println(...)`) is the call's, and a field of the method's receiver (`r.x` in a method of `r`, in
  a `func` literal in it too) is none, as Java's `this.x`. A package's name (`os.Args`) can't be told from a
  variable's, so its selector is an access, as Python's `os.path`. A composite literal (`T{a: 1}`), a type
  conversion (`[]byte(s)`; `int(x)` parses as a call) and a type assertion (`x.(T)`) weigh nothing of their own,
  as Rust's struct expression.
- Tests are files of their own, `*_test.go` (a test glob), so the language has no `tests` hook.
- Comments: `comment`, a doc comment (`// Package a ...` right before `package`, `// F ...` before `func F`) as any;
  `//go:build` and `//go:generate` weigh as text, the first, before a blank line at the top, in the header.

## C

- Read from `.c` and `.h` files: a header is C (its prototypes, structs, inline functions, macros). A C++
  project's `.h` parses with errors and is weighed as tree-sitter recovered it, with a warning;
  `--exclude '**/*.h'` (or `[files] exclude`) leaves headers out.
- A function definition is a function, named by the name its declarator chain ends in (`f` in
  `static int *const f(int)`, `getf` in `int (*getf(void))(int)`, the function returning a function pointer). A
  prototype at the file's level (`int f(int);`, `extern int g(char *);`) is a function too, as Rust's `extern`
  signature and TS's `declare function` are: a header's API weighs its functions, and a prototype and its
  definition in one file are one entry, summed, its line the prototype's (as TS's overloads). A declaration of
  prototypes alone weighs no statement beside them; a prototype inside a function is a statement there, no function
  of its own. A function pointer (`void (*handler)(int)`, a parameter's `int (*cb)(int)`, a `typedef` of one)
  declares no function: a statement.
- A function's parameters are those of the declarator nearest its name (not the function type it returns), `...`
  one; `(void)` is none, and so are a function pointer's.
- A struct, union or enum with a body is a class, named by its own name, else by the `typedef` that names it
  (`typedef struct {...} P;` is `P`, `typedef struct node {...} Node;` is `node`); that `typedef` weighs nothing of
  its own, the class weighing for it. A reference to one (`struct S *p`) weighs nothing; an anonymous one (`struct {...} s;`,
  a struct's `union {...} u;`) is no class, its fields statements where it is, as Go's. One declared in a function is
  a class nested in it (`f.local`). C has no methods and no lambdas.
- `if` nests its consequence; its `else` is a clause beside it, as TS's: its statement deeper, its `if`
  (`else if`) at the depth. A ternary `?:` is a branch, as Java's. A `switch` is no nesting construct; each `case`
  (`default` too) is a `case`, its value at its depth, its statements deeper (the grammar puts a case's statements
  in it, up to the next case). `for`, `while`, `do` are loops. MSVC's structured exceptions: `__try` is a `try`,
  `__except` a `catch`, `__finally`'s block deeper, `__leave` a jump.
- `break`, `continue`, `goto` and an early `return` are jumps; a label (`out:`) weighs nothing, and a labeled
  `return` at a body's end is its last statement (the language's `wrappers`).
- Statements: an expression statement (an empty `;` none), a declaration (`int a, b = 1;` one), a `typedef`, a
  field declaration (`int x, y;` one), an enumerator (as Java's enum constant), an object-like `#define N 10` (a
  constant, as Rust's `const`). `#include`, `#pragma` and a `#define` without a value (a flag, an include guard's)
  weigh nothing.
- `&&` / `||` are the logical operators; `~x` is a unary operator, `*p`, `&x`, `sizeof` and `,` none. A cast
  (`(T) x`) is an `operator` under `[conditions] casts`. The grammar puts a sign in a number literal (`-1` is one
  literal), so `-1` is no operator, as elsewhere, and under `magic_numbers` its sign is read past. `access` counts a
  field (`a.b` and `p->b` alike) and a subscript (`a[i]`); a call's callee (`p->f(x)`, a function pointer's) is the
  call's.
- A compound literal (`(struct P){1, 2}`) is a `constructor`, as Go's composite literal; a bare initializer
  (`int a[] = {1, 2}`, `struct S s = { .a = 1 }`) is none. A function-like macro's call (`MAX(a, b)`) parses as a
  call, and is one: no syntax tells it from a function's.
- A loop macro (`for_each_string_list_item(item, list) { ... }`, the kernel's `list_for_each(p, head) { ... }`) is
  a `loop`: the grammar reads it as a call missing its `;` beside a block, and C has a block after a call only
  where the macro opens a loop or a scope. Its call weighs `loop` (no statement, no call, no arguments), and its
  block, the statement's next sibling (or its construct's, in `if (a) for_each(x, l) { ... }`), is one level
  deeper, as a `for`'s body (`Lang::loop_body`, which the engine nests though it is no child). The missing `;`
  stays a syntax error: the file still warns. A loop macro of one argument (`FOR_EACH(x) { ... }`, `LIST(*p) {
  ... }`) parses cleanly as a definition of `x` of the type `FOR_EACH`; in a function, where C has no definition
  but GCC's nested function (`int g(int a) { ... }`, its declarator a function's, which stays a function `f.g`),
  such a definition, its name in parentheses, is the loop macro: it weighs `loop`, no function and no entry of its
  own, and its body is one level deeper. At the file's level it stays a function (`y` in `FOR_EACH(y) { ... }`),
  as a macro that defines one would be.
- The preprocessor's `#if`, `#ifdef`, `#ifndef`, `#elif` and `#else` blocks weigh nothing of their own and nest
  nothing: they pick the code that is compiled, no control flow read at run time, so their code weighs where it is
  (an include guard adds nothing, a function inside `#ifdef` weighs as outside). An `#if`'s condition weighs as any
  expression (`defined(A) && !defined(B)` is a `bool_op` and an `operator`, `defined` itself nothing).
- Tests are files of their own (`test_*.c`, `*_test.c`, or in a `test` / `tests` folder: test globs), so the
  language has no `tests` hook.
- Known gaps: a macro's body is text the grammar doesn't parse, so a function-like macro (`#define MAX(a, b) ...`)
  weighs nothing and what is in it nothing, as Rust's `macro_rules!`. A macro used where it is no C syntax
  (`EXPORT int f(void)`, a loop macro without a block, `list_for_each(p, l) g(p);`) and directives that split a
  construct (`#ifdef X if (a) { #else if (b) { #endif`) leave syntax errors, weighed as tree-sitter recovered them,
  with a warning. An attribute macro between a function's type and its name (`static int internal_function
  check_matching(...)`) is such a one, and the recovered tree may take it for the name: in glibc's regex (vendored
  in git's `compat/regex/`) 89 functions merge into 3 entries `internal_function`, one a file, summed. No grammar
  tells such a macro from a name; `--exclude 'compat/**'` (or `[files] exclude`) leaves vendored code out. Both arms of an `#if` / `#else` weigh, summed, and so does an `#if 0` block. A `return` in an `#if`
  block is never its body's last statement: it is a jump, even at the end. A recovered construct named by a keyword is
  no function: an `else if (b) { ... }` that an `#if` splits from its `if` (nginx's `ngx_cidr_match`) recovers as a
  definition named `if`, and a struct before an `if` (`struct s {...} if (a) { ... }`) as one whose type is the
  struct. A definition or a prototype named by `if`, `while`, `return`, `sizeof` or another keyword (`KEYWORDS` in
  `src/lang/c.rs`) weighs no `function`, no parameters and no nesting: its code weighs in its enclosing entry (the
  function, or the file), and the struct is a class of its own there.
- Comments: `comment`, line or block; `#pragma` and the other directives are preprocessor nodes, no comments.
