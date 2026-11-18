//! Rust.

use tree_sitter::Node;

use super::{Children, Lang, Nesting, Operator, Scope, ScopeKind, is_field_of, only_child, text};
use crate::config::Category::{self, *};

pub static RUST: Lang = Lang {
    name: "rust",
    key: "rust",
    extensions: &["rs"],
    grammar: || tree_sitter_rust::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_expression"],
    wrappers: &["expression_statement"],
    lambdas: &["closure_expression"],
    parens: &["parenthesized_expression"],
    operator,
    exempt,
    tail,
    scope,
};

const KINDS: &[(&str, Category)] = &[
    // An `impl` block is a class's too (`scope`), but weighs nothing of its own: its type's declaration does.
    ("struct_item", Class),
    ("enum_item", Class),
    ("union_item", Class),
    ("trait_item", Class),
    ("mod_item", Class),
    ("function_item", Function),
    ("function_signature_item", Function),
    ("closure_expression", Lambda),
    ("if_expression", Branch),
    ("match_arm", Branch),
    ("for_expression", Loop),
    ("while_expression", Loop),
    ("loop_expression", Loop),
    ("call_expression", Call),
    // Its arguments are a token tree, not parsed: only the call weighs.
    ("macro_invocation", Call),
    ("break_expression", Jump),
    ("continue_expression", Jump),
    // `?`: an early return of the error.
    ("try_expression", Jump),
    ("field_expression", Access),
    ("index_expression", Access),
    ("return_expression", Statement),
    ("expression_statement", Statement),
    ("let_declaration", Statement),
    ("const_item", Statement),
    ("static_item", Statement),
    ("type_item", Statement),
    ("associated_type", Statement),
    ("field_declaration", Statement),
    ("enum_variant", Statement),
];

const fn only(kind: &'static str, body: &'static [&'static str]) -> Nesting {
    Nesting {
        kind,
        body: Children::Only(body),
        chain: None,
    }
}

const NESTING: &[Nesting] = &[
    // `else if`: the `if` in an else stays at its depth, an else's block is deeper (as TypeScript's).
    only("if_expression", &["consequence"]),
    Nesting {
        kind: "else_clause",
        body: Children::AllBut(&["if_expression"]),
        chain: None,
    },
    // `match` is no nesting construct; each arm is, its pattern and guard at its depth, its value deeper.
    only("match_arm", &["value"]),
    only("for_expression", &["body"]),
    only("while_expression", &["body"]),
    only("loop_expression", &["body"]),
    only("closure_expression", &["body"]),
];

/// The statements of a block, which a block's last child is when it is no tail expression; and a label.
const NOT_TAILS: &[&str] = &[
    "expression_statement",
    "let_declaration",
    "empty_statement",
    "attribute_item",
    "inner_attribute_item",
    "const_item",
    "static_item",
    "macro_definition",
    "mod_item",
    "foreign_mod_item",
    "struct_item",
    "union_item",
    "enum_item",
    "type_item",
    "function_item",
    "function_signature_item",
    "impl_item",
    "trait_item",
    "associated_type",
    "use_declaration",
    "extern_crate_declaration",
    "label",
];

/// The expressions an expression statement around them adds nothing to, as Java's `if` or `return` is no
/// expression statement: a block and the expressions ending in one (`if`, `match`, a loop, `unsafe { }`), and the
/// jumps (which weigh as `return`, `break` and `continue` statements).
const BARE: &[&str] = &[
    "block",
    "unsafe_block",
    "async_block",
    "gen_block",
    "try_block",
    "const_block",
    "if_expression",
    "match_expression",
    "for_expression",
    "while_expression",
    "loop_expression",
    "return_expression",
    "break_expression",
    "continue_expression",
];

/// A binary operator, a let chain (`if let Some(x) = a && x > 0`, a run of `&&`), a compound assignment, `-x` /
/// `!x`. A dereference (`*x`) and a borrow (`&x`) are none: plumbing, not arithmetic.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "binary_expression" => Operator::of(token()?),
        "let_chain" => Some(Operator::And),
        "compound_assignment_expr" => Operator::of_assignment(token()?),
        "unary_expression" => match node.child(0)?.kind() {
            "-" | "!" => {
                Operator::unary(node.named_child(0)?, &["integer_literal", "float_literal"])
            }
            _ => None,
        },
        _ => None,
    }
}

/// An expression statement around a block, a construct ending in one or a jump (see `BARE`); a module declared
/// without a body (`mod a;`), which is only where its file is. A field that is a call's callee (a method call,
/// `a.f()`: the call weighs) or of `self` (`self.x`, as Java's bare `x` is no access).
fn exempt(node: Node, _: &[u8]) -> bool {
    match node.kind() {
        "expression_statement" => only_child(node).is_some_and(|c| BARE.contains(&c.kind())),
        "mod_item" => node.child_by_field_name("body").is_none(),
        "field_expression" => {
            is_field_of(node, &["call_expression"], "function")
                || node
                    .child_by_field_name("value")
                    .is_some_and(|value| value.kind() == "self")
        }
        _ => false,
    }
}

/// A block whose last child is an expression, no statement: its value. A jump there (`return x`, `break`) weighs
/// as itself.
fn tail(node: Node) -> bool {
    if node.kind() != "block" {
        return false;
    }
    let mut cursor = node.walk();
    let last = node
        .named_children(&mut cursor)
        .filter(|c| !c.is_extra())
        .last();
    last.is_some_and(|last| {
        !last.is_error()
            && !NOT_TAILS.contains(&last.kind())
            && !matches!(
                last.kind(),
                "return_expression" | "break_expression" | "continue_expression"
            )
    })
}

/// A struct, enum, union, trait or module (with a body) by its name; an `impl` block by its type's name, so a type's
/// struct and its `impl` blocks (`impl Foo`, `impl Trait for Foo`) are one class; a function, a method or a trait's
/// signature by its name.
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let name = |node: Node| node.child_by_field_name("name").map(|n| text(n, source));
    let (kind, name) = match node.kind() {
        "struct_item" | "enum_item" | "union_item" | "trait_item" => {
            (ScopeKind::Class, name(node)?)
        }
        "mod_item" => {
            node.child_by_field_name("body")?;
            (ScopeKind::Class, name(node)?)
        }
        "impl_item" => (
            ScopeKind::Class,
            type_name(node.child_by_field_name("type")?, source),
        ),
        "function_item" | "function_signature_item" => (ScopeKind::Function, name(node)?),
        _ => return None,
    };
    Some(Scope { kind, name })
}

/// A type's name without its type arguments, references and path: `Vec<T>`, `&mut a::Foo<'_>` are `Vec`, `Foo`.
/// Any other type (a slice, a tuple, `dyn Trait`) is its text.
fn type_name(node: Node, source: &[u8]) -> String {
    let inner = match node.kind() {
        "generic_type" | "reference_type" | "pointer_type" => node.child_by_field_name("type"),
        "scoped_type_identifier" => node.child_by_field_name("name"),
        _ => None,
    };
    match inner {
        Some(inner) => type_name(inner, source),
        None => text(node, source)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::RUST;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&RUST, text)
    }

    #[test]
    fn branches_an_else_if_chain_a_let_chain_and_the_tail() {
        let weighed = weigh(
            "fn f(x: i32) -> i32 {
               if x > 0 && ok() {               // branch 10, bool_op 5, call 2, at 0 (its statement 0)
                 g();                           // statement 1, call 2, at 1: 6
               } else if x < 0 {                // branch 10 at 0
                 h();                           // 6
               } else {
                 if x == 0 { k(); }             // branch 10 × 2 at 1; 3 × 3 at 2
               }
               if let Some(y) = m(x) && y > 1 { // branch 10, bool_op 5 (a let chain), call 2
                 return y;                      // an early return: jump 3 × 2 (its statement 0)
               }
               x + 1                            // the tail: statement 1, as `return x + 1;`
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 17.0 + 6.0 + 1.0
        );
        assert_eq!(weighed.file, 0.0);
    }

    #[test]
    fn loops_match_arms_question_marks_and_closures() {
        let weighed = weigh(
            "fn f(xs: &[i32]) -> Result<i32, E> {
               for x in xs.iter() {               // loop 20, call 2
                 g(*x)?;                          // at 1: statement 1, jump 3 (`?`), call 2: 12
               }
               'outer: while a || b {             // loop 20, bool_op 5
                 loop { break 'outer; }           // loop 20 × 2; jump 3 × 3
               }
               let n = match xs.len() {           // statement 1, call 2
                 0 => return Err(E),              // branch 10; at 1: an early return, jump 3 × 2, call 2 × 2
                 1 => xs[0],                      // branch 10
                 n if n > 9 => { h(); n }         // branch 10; at 1: statement 1 × 2, call 2 × 2, the tail 1 × 2
                 _ => 0,                          // branch 10
               };
               let c = |y: i32| y + n;            // statement 1, lambda 50
               let d = |y: i32| { k(y); y };      // statement 1, lambda 50; at 1: 6, the tail 1 × 2
               Ok(c(1) + d(2))                    // the tail: statement 1, call 2 × 3
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 34.0 + 74.0 + (3.0 + 20.0 + 10.0 + 18.0 + 10.0) + 51.0 + 59.0 + 7.0
        );
    }

    #[test]
    fn returns_jumps_and_the_statements_around_them() {
        let weighed = weigh(
            "fn f(x: i32) -> i32 {
               if x < 0 {
                 return -1;                     // branch 10; an early return: jump 3 × 2
               }
               while x > 9 { break; }           // loop 20; jump 3 × 2
               return x;                        // the last statement: statement 1
             }
             fn g(x: i32) -> i32 { return x * 2 }         // the last statement, no `;`: 1, no tail of its own
             fn h() { unsafe { k() } }                    // the unsafe block's tail: statement 1, call 2
             fn e() { loop { if done() { break } } }      // loop 20; at 1: branch 10 × 2, call 2 × 2; jump 3 × 3
             fn c() { let k = |y: i32| { return y; }; }   // statement 1, lambda 50; the closure's last: 1 × 2",
        );
        assert_eq!(function(&weighed, "f"), 100.0 + 16.0 + 26.0 + 1.0);
        assert_eq!(function(&weighed, "g"), 101.0);
        assert_eq!(function(&weighed, "h"), 103.0);
        assert_eq!(function(&weighed, "e"), 100.0 + 20.0 + 24.0 + 9.0);
        assert_eq!(function(&weighed, "c"), 100.0 + 51.0 + 2.0);
    }

    #[test]
    fn a_type_s_struct_and_impl_blocks_are_one_class() {
        let weighed = weigh(
            "mod shapes {                                       // shapes: class 1000
               pub struct Circle { r: f64 }                    // shapes.Circle: class 1000, a field 1
               impl Circle {                                   // nothing of its own; shapes.Circle's
                 pub fn new(r: f64) -> Self { Circle { r } }   // shapes.Circle.new: 100, the tail 1
               }
               impl Area for Circle {
                 fn area(&self) -> f64 { self.r * self.r }     // shapes.Circle.area: 100 + 1
               }
               impl<T> Area for &mut Vec<T> {                  // shapes.Vec: 0
                 fn area(&self) -> f64 { 0.0 }                 // shapes.Vec.area: 101
               }
               pub trait Area {                                // shapes.Area: 1000
                 fn area(&self) -> f64;                        // shapes.Area.area: 100
                 fn twice(&self) -> f64 { 2.0 * self.area() }  // shapes.Area.twice: 100, the tail 1, call 2
               }
               pub enum Kind { Round, Square(f64) }            // shapes.Kind: 1000, its variants 1 + 1
             }
             mod other;                                        // only where a file is: 0, no class
             const LIMIT: usize = 10;                          // file: statement 1
             fn outer() {                                      // outer: 100
               fn inner(x: i32) -> i32 { x }                   // outer.inner: 100; its body nests: tail 1 × 2
               inner(1);                                       // outer: statement 1, call 2
             }
             macro_rules! sq { ($x:expr) => { $x * $x }; }     // its rules are not parsed: 0
             static TOTAL: i32 = sq!(2);                       // file: statement 1, call 2",
        );
        assert_eq!(class(&weighed, "shapes"), 1000.0);
        assert_eq!(class(&weighed, "shapes.Circle"), 1001.0);
        assert_eq!(function(&weighed, "shapes.Circle.new"), 101.0);
        assert_eq!(function(&weighed, "shapes.Circle.area"), 101.0);
        assert_eq!(class(&weighed, "shapes.Vec"), 0.0);
        assert_eq!(function(&weighed, "shapes.Vec.area"), 101.0);
        assert_eq!(class(&weighed, "shapes.Area"), 1000.0);
        assert_eq!(function(&weighed, "shapes.Area.area"), 100.0);
        assert_eq!(function(&weighed, "shapes.Area.twice"), 103.0);
        assert_eq!(class(&weighed, "shapes.Kind"), 1002.0);
        assert_eq!(function(&weighed, "outer"), 103.0);
        assert_eq!(function(&weighed, "outer.inner"), 102.0);
        assert_eq!(weighed.file, 1.0 + 3.0);
        assert_eq!(weighed.entries.len(), 12);
        let circle = weighed.entries.iter().find(|e| e.name == "shapes.Circle");
        assert_eq!(circle.map(|e| e.line), Some(2));
    }

    #[test]
    fn a_run_of_like_operators_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &RUST,
            "fn f() {
               let x = (b + c) * d;      // statement 1, two runs: 14
               let y = -1;               // 1, a negative number: 0
               let z = !ok && -a < b;    // 1, `!` 7, bool_op 5, `<` 7, `-` 7
               let r = *p + &q;          // 1, a dereference and a borrow none, `+` 7
               x += 1;                   // 1, 7
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 1.0 + 27.0 + 8.0 + 8.0
        );
    }

    #[test]
    fn each_field_or_index_is_one_access_but_a_method_s_or_self_s() {
        let weighed = testing::weigh_expressions(
            &RUST,
            "impl A {
               fn f(&self) {
                 let x = o.a.b;          // statement 1, access 18
                 let y = self.v;         // 1, of `self`: 0
                 let z = self.v.0;       // 1, 9
                 o.a.g();                // 1, call 2, `o.a` 9 (the callee `o.a.g` is the call's)
                 v[i][j]                 // the tail: statement 1, 18
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 19.0 + 1.0 + 10.0 + 12.0 + 19.0
        );
    }

    #[test]
    fn the_weights_file_s_rust_table() {
        let weights = format!(
            "{WEIGHTS}
             [languages.rust.weights]
             call = 7
             [languages.rust.node_kinds]
             macro_invocation = \"none\""
        );
        let text = "fn f() { g(); println!(\"{}\", h()); }";
        // g(): statement 1, call 7; the macro's statement 1, the macro none (and h() in it unparsed).
        let weighed = testing::weigh_under(&RUST, &weights, text);
        assert_eq!(function(&weighed, "f"), 100.0 + 8.0 + 1.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&RUST, WEIGHTS, "impl A { fn m() { g( } ");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }
}
