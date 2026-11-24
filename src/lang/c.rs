//! C.

use tree_sitter::Node;

use super::{
    Lang, Nesting, Operator, Scope, ScopeKind, count_children, first_child, is_field_of, text,
};
use crate::config::Category::{self, *};

pub static C: Lang = Lang {
    name: "c",
    key: "c",
    extensions: &["c", "h"],
    grammar: || tree_sitter_c::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &["labeled_statement"],
    lists: &[],
    lambdas: &[],
    chains: &[],
    casts: &["cast_expression"],
    patterns: &[],
    numbers: &["number_literal"],
    operator,
    exempt,
    also_statement: |_| false,
    parameters,
    scope,
    tests: None,
    default_case: |node, _| node.child_by_field_name("value").is_none(),
    carried_return: |_| false,
};

const KINDS: &[(&str, Category)] = &[
    // A struct, union or enum with a body and a name (its own or its `typedef`'s); a reference to one
    // (`struct S *p`) and an anonymous one weigh nothing (`exempt`).
    ("struct_specifier", Class),
    ("union_specifier", Class),
    ("enum_specifier", Class),
    ("function_definition", Function),
    // A prototype's (`int f(int);`, at the file's level): a function, as Rust's `extern` signature and TS's
    // `declare function` are; any other one (a definition's, a function pointer's) weighs nothing (`exempt`).
    ("function_declarator", Function),
    ("if_statement", Branch),
    ("conditional_expression", Branch),
    ("case_statement", Case),
    ("for_statement", Loop),
    ("while_statement", Loop),
    ("do_statement", Loop),
    // MSVC's structured exceptions: `__try`, `__except`.
    ("seh_try_statement", Try),
    ("seh_except_clause", Catch),
    // A macro's call (`MAX(a, b)`) parses as a call, which it stays: no syntax tells it from a function's.
    ("call_expression", Call),
    // `(struct P){1, 2}`: a constructor, as Go's composite literal is (a bare `{1, 2}` initializer is none).
    ("compound_literal_expression", Constructor),
    ("break_statement", Jump),
    ("continue_statement", Jump),
    ("goto_statement", Jump),
    ("seh_leave_statement", Jump),
    // `a.b` and `p->b` alike.
    ("field_expression", Access),
    ("subscript_expression", Access),
    ("return_statement", Statement),
    ("expression_statement", Statement),
    ("declaration", Statement),
    ("type_definition", Statement),
    ("field_declaration", Statement),
    ("enumerator", Statement),
    // `#define N 10`: a constant, as Rust's `const`; a flag without a value (`#define A_H`, an include guard's)
    // weighs nothing (`exempt`); a function-like macro's body isn't parsed, so it weighs nothing, as Rust's
    // `macro_rules!`.
    ("preproc_def", Statement),
];

const NESTING: &[Nesting] = &[
    // `else if`: the `if` in an else stays at its depth, an else's statement is deeper (as TypeScript's).
    Nesting::only("if_statement", &["consequence"]),
    Nesting::all_but("else_clause", &["if_statement"]),
    Nesting::ternary("conditional_expression"),
    // A `switch` is no nesting construct; each case is, its value at its depth, its statements deeper.
    Nesting::all_but("case_statement", &["value"]),
    Nesting::body("for_statement"),
    Nesting::body("while_statement"),
    Nesting::body("do_statement"),
    Nesting::body("seh_try_statement"),
    Nesting::body("seh_except_clause"),
    Nesting::body("seh_finally_clause"),
];

/// The kinds of a struct, union or enum.
const SPECIFIERS: &[&str] = &["struct_specifier", "union_specifier", "enum_specifier"];

/// A binary operator, an assignment, `-x` / `+x` / `!x` / `~x`, `++` / `--`. A dereference (`*p`), an address
/// (`&x`) and `sizeof` are none: plumbing, not arithmetic.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "binary_expression" => Operator::of(token()?),
        "assignment_expression" => Operator::of_assignment(token()?),
        "unary_expression" => {
            Operator::unary(node.child_by_field_name("argument")?, &["number_literal"])
        }
        "update_expression" => Some(Operator::Other),
        _ => None,
    }
}

/// An empty statement (`;`); a `#define` without a value (`#define A_H`), a flag; a function declarator but a
/// prototype's; a declaration of prototypes alone, which are functions, no statement; a struct, union or enum that
/// declares no class (a reference to one, `struct S *p`, or an anonymous one); a `typedef` of a struct, union or
/// enum it defines (`typedef struct {...} P;`), which only names the class; a field that is a call's callee
/// (`p->f(x)`, a function pointer's call: the call weighs).
fn exempt(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "expression_statement" => first_child(node).is_none(),
        "preproc_def" => node.child_by_field_name("value").is_none(),
        "function_declarator" => function_of(node) != Some(Declares::Prototype),
        "declaration" => {
            let mut cursor = node.walk();
            let mut declarators = node
                .children_by_field_name("declarator", &mut cursor)
                .peekable();
            declarators.peek().is_some() && declarators.all(is_prototype)
        }
        kind if SPECIFIERS.contains(&kind) => scope(node, source).is_none(),
        "type_definition" => node.child_by_field_name("type").is_some_and(|t| {
            SPECIFIERS.contains(&t.kind()) && t.child_by_field_name("body").is_some()
        }),
        "field_expression" => is_field_of(node, &["call_expression"], "function"),
        _ => false,
    }
}

/// A definition's or a prototype's parameters, each one (`...` too); none for `(void)`, nor a function pointer's,
/// nor the function type a function returns (`int (*f(void))(int)` has none).
fn parameters(node: Node, source: &[u8]) -> usize {
    if node.kind() != "parameter_list" || node.parent().and_then(function_of).is_none() {
        return 0;
    }
    count_children(node, |c| match c.kind() {
        "parameter_declaration" => !is_void(c, source),
        "variadic_parameter" => true,
        _ => false,
    })
}

/// Whether a parameter is a lone `void`, which says there are none (`int f(void)`).
fn is_void(parameter: Node, source: &[u8]) -> bool {
    parameter.child_by_field_name("declarator").is_none()
        && parameter
            .child_by_field_name("type")
            .is_some_and(|t| &source[t.byte_range()] == b"void")
}

/// The kinds of a declarator around another: a name's pointer, array, function, parentheses.
const DECLARATORS: &[&str] = &[
    "function_declarator",
    "pointer_declarator",
    "array_declarator",
    "parenthesized_declarator",
    "attributed_declarator",
    "init_declarator",
];

/// The declarator a declarator holds, one level in.
fn inner_declarator(node: Node) -> Option<Node> {
    match node.kind() {
        "parenthesized_declarator" => first_child(node),
        kind if DECLARATORS.contains(&kind) => node.child_by_field_name("declarator"),
        _ => None,
    }
}

/// The name a declarator chain ends in: `f` in `*const f(int)`, `getf` in `(*getf(void))(int)`.
fn declared_name(declarator: Node) -> Option<Node> {
    let mut node = declarator;
    loop {
        if matches!(
            node.kind(),
            "identifier" | "type_identifier" | "field_identifier"
        ) {
            return Some(node);
        }
        node = inner_declarator(node)?;
    }
}

/// What a function declarator declares, if it is its function's own (the one nearest the name: `f(void)` in
/// `int (*f(void))(int)`, the one further out being the function type `f` returns): a definition's, or a
/// prototype's at the file's level (one in a function is a statement there, no function of its own). A function
/// pointer's (`int (*fp)(int)`, a parameter's) declares none.
fn function_of(declarator: Node) -> Option<Declares> {
    if declarator.kind() != "function_declarator"
        || declarator.child_by_field_name("declarator")?.kind() != "identifier"
    {
        return None;
    }
    let mut node = declarator;
    loop {
        let outer = node.parent()?;
        match outer.kind() {
            "function_definition" if is_field_of(node, &["function_definition"], "declarator") => {
                return Some(Declares::Definition);
            }
            "declaration" if is_field_of(node, &["declaration"], "declarator") => {
                let mut around = outer.parent();
                while let Some(scope) = around {
                    if scope.kind() == "function_definition" {
                        return None;
                    }
                    around = scope.parent();
                }
                return Some(Declares::Prototype);
            }
            kind if DECLARATORS.contains(&kind) && kind != "init_declarator" => node = outer,
            _ => return None,
        }
    }
}

/// What a function's own declarator declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Declares {
    Definition,
    Prototype,
}

/// Whether a declaration's declarator declares a prototype (`f(int)`, `*f(int)`).
fn is_prototype(declarator: Node) -> bool {
    let mut node = Some(declarator);
    while let Some(at) = node {
        if function_of(at) == Some(Declares::Prototype) {
            return true;
        }
        node = inner_declarator(at);
    }
    false
}

/// A function definition or a prototype by its name (a prototype and its definition are one entry, summed, as
/// TS's overloads); a struct, union or enum with a body by its name, else by the `typedef` that
/// names it (`typedef struct {...} P;` is `P`); an anonymous one (`struct {...} s;`, a struct's `union {...} u;`)
/// is no class.
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let (kind, name) = match node.kind() {
        "function_definition" => (
            ScopeKind::Function,
            declared_name(node.child_by_field_name("declarator")?)?,
        ),
        "function_declarator" if function_of(node) == Some(Declares::Prototype) => {
            (ScopeKind::Function, node.child_by_field_name("declarator")?)
        }
        kind if SPECIFIERS.contains(&kind) => {
            node.child_by_field_name("body")?;
            let name = match node.child_by_field_name("name") {
                Some(name) => name,
                None => {
                    let typedef = node.parent().filter(|p| p.kind() == "type_definition")?;
                    declared_name(typedef.child_by_field_name("declarator")?)?
                }
            };
            (ScopeKind::Class, name)
        }
        _ => return None,
    };
    Some(Scope {
        kind,
        name: text(name, source),
    })
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::C;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&C, text)
    }

    #[test]
    fn branches_an_else_if_chain_a_ternary_and_the_last_return() {
        let weighed = weigh(
            "int f(int x) {
               if (x > 0 && ok()) {     // branch 10, bool_op 5, call 2, at 0
                 g();                   // statement 1, call 2, at 1: 6
               } else if (x < 0) {      // branch 10 at 0
                 h();                   // 6
               } else {
                 if (x == 0) k();       // branch 10 × 2 at 1; 3 × 3 at 2
               }
               int y = x ? 1 : 2;       // statement 1, branch 10
               if (y) return -1;        // branch 10; an early return: jump 3 × 2
             L:
               return x;                // the last statement, though labeled: statement 1
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 11.0 + 16.0 + 1.0
        );
        assert_eq!(weighed.file, 0.0);
    }

    #[test]
    fn loops_switch_cases_and_jumps() {
        let weighed = weigh(
            "void f(int n) {
               for (int i = 0; i < n; i++) {  // loop 20; its initializer: statement 1
                 if (i > 9) break;            // at 1: branch 10 × 2; jump 3 × 3
               }
               while (n) n--;                 // loop 20; at 1: statement 1 × 2
               do { g(); } while (n);         // loop 20; at 1: 3 × 2
               for (;;) continue;             // loop 20; jump 3 × 2
               switch (n) {
               case 0:                        // case 10
               case 1:                        // case 10
                 g();                         // at 1: 3 × 2
                 break;                       // jump 3 × 2
               default:                       // case 10
                 goto out;                    // jump 3 × 2
               }
             out:
               g();                           // statement 1, call 2
               ;                              // an empty statement: nothing
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 21.0 + 29.0 + 22.0 + 26.0 + 26.0 + (10.0 + 10.0 + 6.0 + 6.0 + 10.0 + 6.0) + 3.0
        );
    }

    #[test]
    fn structs_unions_enums_typedefs_prototypes_and_defines() {
        let weighed = weigh(
            "#include <stdio.h>
             #define N 10
             #define MAX(a, b) ((a) > (b) ? (a) : (b))
             struct point { int x, y; };
             typedef struct { int w; char *name; } Box;
             typedef struct node { struct node *next; union { int i; float f; } u; } Node;
             enum color { RED, GREEN = 2 };
             typedef int score;
             typedef int (*cb)(int);
             int count;
             int area(struct point *p);
             int area(struct point *p) { return p->x * p->y; }
             static inline int twice(int v) { return 2 * v; }
             void (*handler)(int);
             int run(void) {
               struct local { int v; } l;
               int g(int);
               return 0;
             }",
        );
        // `#include`, the function-like macro: nothing; `#define N 10`, the typedef of an int and of a function
        // pointer, `count` and `handler`: a statement each.
        assert_eq!(weighed.file, 5.0);
        // A field declaration is a statement, its names one or more.
        assert_eq!(class(&weighed, "point"), 1001.0);
        // Named by its typedef, which weighs nothing beside the class.
        assert_eq!(class(&weighed, "Box"), 1002.0);
        // Named by its own name; the anonymous union is no class, its fields the struct's.
        assert_eq!(class(&weighed, "node"), 1004.0);
        assert_eq!(class(&weighed, "color"), 1002.0);
        // The prototype and the definition: one entry, a function each, on the prototype's line.
        assert_eq!(function(&weighed, "area"), 201.0);
        let area = weighed.entries.iter().find(|e| e.name == "area");
        assert_eq!(area.map(|e| e.line), Some(11));
        assert_eq!(function(&weighed, "twice"), 101.0);
        // Its struct a nested class, its field at 1; the declaration of `l`, a prototype inside a function and the
        // return a statement each.
        assert_eq!(class(&weighed, "run.local"), 1002.0);
        assert_eq!(function(&weighed, "run"), 103.0);
        assert_eq!(weighed.entries.len(), 8);
    }

    #[test]
    fn each_operator_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &C,
            "void f(void) {
               x = (b + c) * d;            // statement 1, two: 14
               y = -1;                     // 1, a negative number: 0
               r = !ok && -a < b;          // 1, `!` 7, bool_op 5, `<` 7, `-` 7
               m = a ^ b | ~c << 2;        // 1, `^` 7, `|` 7, `~` 7, `<<` 7
               q = *p + &v;                // 1, a dereference and an address none, `+` 7
               x += 1;                     // 1, 7
               x <<= 1;                    // 1, 7
               x++;                        // 1, 7
               n = sizeof(int);            // 1
               for (i = 0; i < 9; i++) {}  // loop 20; `<` 7, `++` 7
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 1.0 + 27.0 + 29.0 + 8.0 + 8.0 + 8.0 + 8.0 + 1.0 + 34.0
        );
    }

    #[test]
    fn each_field_or_index_is_one_access_but_a_callee_s() {
        let weighed = testing::weigh_expressions(
            &C,
            "void f(struct s *p) {
               x = o.a.b;                  // statement 1, access 18
               y = p->v[i][j];             // 1, 27
               o.a.g(1);                   // 1, call 2, `o.a` 9 (the callee `o.a.g` is the call's)
               p->ops->read(p);            // 1, call 2, `p->ops` 9
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 19.0 + 28.0 + 12.0 + 12.0);
    }

    #[test]
    fn an_access_weighs_more_the_further_it_is_in_a_row() {
        let weighed = testing::weigh_accesses(
            &C,
            "void f(void) {
               x = o.a.b.c.d;   // statement 1, accesses 9 + 18 + 27 + 27
               x = p->a[i];     // 1, 9 + 18
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 82.0 + 28.0);
    }

    #[test]
    fn parameters_weigh_by_their_place() {
        let weighed = testing::weigh_parameters(
            &C,
            "int f(int a, char **b, ...);                 // f: 100; 10 + 15 + 20
             int f(int a, char **b, ...) { return 0; }    // f: 100, 45; statement 1
             int g(void) { return 0; }                    // g: 100, no parameter; 1
             int (*getf(int x))(char, char) { return 0; } // getf: 100, 10, not the returned type's; 1
             void h(int (*cb)(int, int)) {                // h: 100, 10, not the pointer's
               int (*fp)(int) = 0;                        // statement 1, not the pointer's
             }
             extern int k(char, long);                    // k: 100, unnamed ones too: 10 + 15
             typedef int (*cbt)(int, int);                // file: statement 1",
        );
        assert_eq!(function(&weighed, "f"), 291.0);
        assert_eq!(function(&weighed, "g"), 101.0);
        assert_eq!(function(&weighed, "getf"), 111.0);
        assert_eq!(function(&weighed, "h"), 111.0);
        assert_eq!(function(&weighed, "k"), 125.0);
        assert_eq!(weighed.file, 1.0);
    }

    #[test]
    fn a_call_s_arguments_weigh_each() {
        let weighed = testing::weigh_arguments(
            &C,
            "void f(void) {              // function 100
               g();                     // statement 1, call 2, no argument
               g(a, b);                 // statement 1, call 2, 2 arguments 4 each
               h((struct P){1, 2});     // statement 1, call 2, an argument 4, a constructor 2 (no arguments)
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 3.0 + 11.0 + 9.0);
    }

    #[test]
    fn a_compound_literal_weighs_constructor() {
        let weighed = testing::weigh_constructors(
            &C,
            "void f(void) {
               struct P p = (struct P){1, 2};   // statement 1, constructor 8
               int a[] = {1, 2};                // statement 1: a bare initializer none
               g((P){0});                       // statement 1, call 2, constructor 8
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 9.0 + 1.0 + 11.0);
    }

    #[test]
    fn a_switch_s_cases_weigh_case_not_branch() {
        let weighed = testing::weigh_cases(
            &C,
            "void f(int x) {   // function 100
               switch (x) {
               case 1:         // case 7
                 g();          // (statement 1, call 2) × 2
               default:        // case 7
                 ;
               }
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 13.0 + 7.0);
    }

    #[test]
    fn a_default_case_weighs_nothing_under_free_default_and_a_ternary_s_chain_nests_under_nested_ternaries()
     {
        let (off, on) = testing::ternaries_and_defaults(
            &C,
            "void f(int x) {
               switch (x) {
               case 1: g(); break;
               default: g();             // on: the default case 10 weighs nothing
               }
             }",
        );
        assert_eq!(on - off, -10.0);
        let (off, on) = testing::ternaries_and_defaults(
            &C,
            "int f(int a, int b) { return a ? 1 : b ? 2 : 3; } // on: the second branch at 1",
        );
        assert_eq!((off, on), (121.0, 131.0));
    }

    #[test]
    fn the_conditions_weigh_casts_and_magic_numbers() {
        let added = testing::conditions_add(
            &C,
            "void f(void) {
               x = (int)y;         // a cast: operator 7
               n = 0x10 + 1;       // 0x10 magic: access 9
               z = 1.0f - 1;       // none
               m = 10UL;           // access 9
               k = -1;             // a negative number, its sign in the literal: none
               j = -2;             // access 9
             }",
        );
        assert_eq!(added, 7.0 + 9.0 + 9.0 + 9.0);
    }

    #[test]
    fn the_preprocessor_s_blocks_are_no_nesting_and_weigh_their_conditions() {
        let weighed = testing::weigh_expressions(
            &C,
            "#ifndef A_H
             #define A_H
             #if defined(X) && !defined(Y)
             int f(void) {
             #ifdef DEBUG
               log();              // statement 1, call 2, at 0: a directive is no nesting
             #else
               if (x) return 1;    // branch 10; an early return: jump 3 × 2
             #endif
               return 0;           // statement 1
             }
             int g(void) {
             #ifdef W
               return 1;           // in a directive, it isn't its body's last: jump 3
             #else
               return 2;           // jump 3
             #endif
             }
             #endif
             #endif",
        );
        // The include guard's `#define A_H` nothing; the condition: bool_op 5, `!` 7.
        assert_eq!(weighed.file, 12.0);
        assert_eq!(function(&weighed, "f"), 100.0 + 3.0 + 16.0 + 1.0);
        assert_eq!(function(&weighed, "g"), 106.0);
    }

    #[test]
    fn msvc_s_structured_exceptions_are_a_try_and_a_catch() {
        let weighed = weigh(
            "void f(void) {
               __try { g(); }              // try 30; at 1: 3 × 2
               __except (1) { h(); }       // catch 40; at 1: 6
               __try { __leave; }          // try 30; at 1: jump 3 × 2
               __finally { g(); }          // at 1: 6
             }",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 36.0 + 46.0 + 36.0 + 6.0);
    }

    #[test]
    fn the_weights_file_s_c_table() {
        let weights = format!(
            "{WEIGHTS}
             [languages.c.weights]
             call = 7
             [languages.c.node_kinds]
             goto_statement = \"none\""
        );
        let text = "void f(void) { g(); goto out; out: ; }";
        // g(): statement 1, call 7; the goto none.
        let weighed = testing::weigh_under(&C, &weights, text);
        assert_eq!(function(&weighed, "f"), 100.0 + 8.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&C, WEIGHTS, "int f(void) { g( }");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }
}
