//! TypeScript and TSX: two grammars, one table in the weights file (`[languages.typescript]`).

use tree_sitter::Node;

use super::{Children, Lang, Nesting, Operator, Scope, ScopeKind, is_field_of, only_child, text};
use crate::config::Category::*;

pub static TYPESCRIPT: Lang = Lang {
    name: "typescript",
    key: "typescript",
    extensions: &["ts", "mts", "cts"],
    grammar: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    kinds: kinds!(),
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &[],
    lambdas: LAMBDAS,
    parens: &["parenthesized_expression"],
    operator,
    exempt,
    tail: |_| false,
    scope,
};

pub static TSX: Lang = Lang {
    name: "tsx",
    key: "typescript",
    extensions: &["tsx"],
    grammar: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    // A JSX element is a call: it compiles to one (`jsx(...)`, `React.createElement(...)`).
    kinds: kinds!(("jsx_element", Call), ("jsx_self_closing_element", Call),),
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &[],
    lambdas: LAMBDAS,
    parens: &["parenthesized_expression"],
    operator,
    exempt,
    tail: |_| false,
    scope,
};

/// The kind → category table both grammars share, and a grammar's own entries after it.
macro_rules! kinds {
    ($($extra:tt)*) => {
        &[
            ("class_declaration", Class),
            ("abstract_class_declaration", Class),
            ("class", Class),
            ("interface_declaration", Class),
            ("enum_declaration", Class),
            ("internal_module", Class),
            ("module", Class),
            ("function_declaration", Function),
            ("generator_function_declaration", Function),
            ("function_signature", Function),
            ("method_definition", Function),
            ("method_signature", Function),
            ("abstract_method_signature", Function),
            ("arrow_function", Lambda),
            ("function_expression", Lambda),
            ("generator_function", Lambda),
            ("if_statement", Branch),
            ("ternary_expression", Branch),
            ("switch_case", Branch),
            ("switch_default", Branch),
            ("for_statement", Loop),
            ("for_in_statement", Loop),
            ("while_statement", Loop),
            ("do_statement", Loop),
            ("try_statement", Try),
            ("catch_clause", Catch),
            ("call_expression", Call),
            ("new_expression", Call),
            ("break_statement", Jump),
            ("continue_statement", Jump),
            ("throw_statement", Jump),
            ("member_expression", Access),
            ("subscript_expression", Access),
            ("return_statement", Statement),
            ("expression_statement", Statement),
            ("lexical_declaration", Statement),
            ("variable_declaration", Statement),
            ("public_field_definition", Statement),
            ("property_signature", Statement),
            ("type_alias_declaration", Statement),
            $($extra)*
        ]
    };
}
use kinds;

const LAMBDAS: &[&str] = &[
    "arrow_function",
    "function_expression",
    "generator_function",
];

const fn body(kind: &'static str) -> Nesting {
    Nesting {
        kind,
        body: Children::Only(&["body"]),
        chain: None,
    }
}

const NESTING: &[Nesting] = &[
    Nesting {
        kind: "if_statement",
        body: Children::Only(&["consequence"]),
        chain: None,
    },
    // `else if`: the `if` in an else stays at its depth, an else's block is deeper.
    Nesting {
        kind: "else_clause",
        body: Children::AllBut(&["if_statement"]),
        chain: None,
    },
    Nesting {
        kind: "ternary_expression",
        body: Children::Only(&["consequence", "alternative"]),
        chain: Some(Children::Only(&["alternative"])),
    },
    body("switch_case"),
    body("switch_default"),
    body("for_statement"),
    body("for_in_statement"),
    body("while_statement"),
    body("do_statement"),
    body("try_statement"),
    body("catch_clause"),
    body("finally_clause"),
    body("arrow_function"),
    body("function_expression"),
    body("generator_function"),
];

/// A binary or unary operator (`typeof`, `void`, `delete` too), a compound assignment (`&&=`, `||=` and `??=` are
/// logical), `++` / `--`.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "binary_expression" => Operator::of(token()?),
        "augmented_assignment_expression" => Operator::of_assignment(token()?),
        "unary_expression" => Operator::unary(node.child_by_field_name("argument")?, &["number"]),
        "update_expression" => Some(Operator::Unary),
        _ => None,
    }
}

/// A directive (`"use strict";`), and a namespace's statement (the grammar wraps a namespace in one; the namespace
/// is a declaration, like a class): an expression statement that is only a string or a namespace. A member that is
/// a call's callee (`a.f()`, `new a.B()`, a JSX element's `<a.B>`: the call weighs) or of `this` / `super`.
fn exempt(node: Node, _: &[u8]) -> bool {
    match node.kind() {
        "expression_statement" => {
            only_child(node).is_some_and(|c| matches!(c.kind(), "string" | "internal_module"))
        }
        "member_expression" => {
            is_field_of(node, &["call_expression"], "function")
                || is_field_of(node, &["new_expression"], "constructor")
                || is_field_of(
                    node,
                    &[
                        "jsx_opening_element",
                        "jsx_closing_element",
                        "jsx_self_closing_element",
                    ],
                    "name",
                )
                || node
                    .child_by_field_name("object")
                    .is_some_and(|object| matches!(object.kind(), "this" | "super"))
        }
        _ => false,
    }
}

/// A class, interface, enum or namespace by its name (a class expression without one by the variable it is bound
/// to, else `class`); a function, method or signature by its name; an arrow or function expression bound to a
/// name at the file's or a namespace's level, or a class field's value, by that name. Any other arrow or function
/// expression is a lambda.
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let name = |node: Node| node.child_by_field_name("name").map(|n| text(n, source));
    let (kind, name) = match node.kind() {
        "class_declaration"
        | "abstract_class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "internal_module"
        | "module" => (ScopeKind::Class, name(node)?),
        "class" => {
            let own = name(node).or_else(|| bound_to(node, source).map(|(name, _)| name));
            (ScopeKind::Class, own.unwrap_or_else(|| "class".to_string()))
        }
        "function_declaration"
        | "generator_function_declaration"
        | "function_signature"
        | "method_definition"
        | "method_signature"
        | "abstract_method_signature" => (ScopeKind::Function, name(node)?),
        "arrow_function" | "function_expression" | "generator_function" => {
            match bound_to(node, source) {
                Some((name, true)) => (ScopeKind::Function, name),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(Scope { kind, name })
}

/// The name an expression is bound to as a variable's value (with whether the variable is at the file's or a
/// namespace's level) or as a class field's value (always at a class's level).
fn bound_to(node: Node, source: &[u8]) -> Option<(String, bool)> {
    let parent = node.parent()?;
    if parent.child_by_field_name("value")?.id() != node.id() {
        return None;
    }
    let name = parent.child_by_field_name("name")?;
    match parent.kind() {
        "public_field_definition" => Some((text(name, source), true)),
        "variable_declarator" if name.kind() == "identifier" => {
            Some((text(name, source), at_top(parent.parent()?)))
        }
        _ => None,
    }
}

/// Whether a declaration is at the file's or a namespace's level: in the program or a namespace's body, itself or
/// through an `export`.
fn at_top(declaration: Node) -> bool {
    let mut around = declaration.parent();
    if around.is_some_and(|n| n.kind() == "export_statement") {
        around = around.and_then(|n| n.parent());
    }
    match around {
        Some(n) if n.kind() == "program" => true,
        Some(n) if n.kind() == "statement_block" => n
            .parent()
            .is_some_and(|p| matches!(p.kind(), "internal_module" | "module")),
        _ => false,
    }
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::{TSX, TYPESCRIPT};
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&TYPESCRIPT, text)
    }

    #[test]
    fn branches_an_else_if_chain_and_logical_runs() {
        let weighed = weigh(
            "function f(x: number) {
               if (x > 0 && ok()) {      // branch 10, bool_op 5, call 2, at 0
                 g();                    // statement 1, call 2, at 1: 6
               } else if (x < 0) {       // branch 10 at 0
                 h();                    // 6
               } else {
                 if (x === 0) k();       // branch 10 × 2 at 1; 3 × 3 at 2
               }
               return x ?? y ?? z;       // the last statement: statement 1; one run of ??: 5
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 1.0 + 5.0
        );
        assert_eq!(weighed.file, 0.0);
    }

    #[test]
    fn loops_switch_and_try() {
        let weighed = weigh(
            "function f(xs: number[]) {
               for (let i = 0; i < n(); i++) {   // loop 20, its initializer 1, call 2
                 g();                            // 6 at 1
               }
               for (const x of xs) { h(x); }     // loop 20; 6
               while (a || b) { k(); }           // loop 20, bool_op 5; 6
               do { k(); } while (m());          // loop 20, 6; call 2 at 0
               switch (x) {
                 case 1:                         // branch 10
                   g();                          // 6 at 1
                   break;                        // jump 3 × 2
                 default:                        // branch 10
                   throw new E();                // jump 3 × 2, call 2 × 2
               }
               try {                             // try 30
                 g();                            // 6
               } catch (e) {                     // catch 40
                 h();                            // 6
               } finally {
                 k();                            // 6
               }
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 29.0 + 26.0 + 31.0 + 28.0 + 22.0 + 20.0 + 88.0
        );
    }

    #[test]
    fn an_arrow_bound_to_a_name_at_the_file_s_or_a_class_s_level_is_a_function() {
        let weighed = weigh(
            "export const handler = async (req: R) => {   // file: statement 1; handler: function 100
               const done = () => finish();              // in a function, a lambda: statement 1, lambda 50; call 2 × 2
               if (req.ok) return done();                // branch 10; an early return: jump 3 × 2; call 2 × 2
               return null;                              // the last statement: 1
             };
             const twice = (x: number) => x * 2;         // file: statement 1; twice: function 100
             items.forEach((i) => use(i));               // file: statement 1, call 2, lambda 50; call 2 × 2
             class A {                                   // class 1000
               onClick = () => { this.go(); };           // A: statement 1; A.onClick: 100, its body not deeper: 3
               #p = function () { return 1; };           // A: statement 1; A.#p: 100 + 1
               m() { const cb = () => 1; }               // A.m: 100, statement 1, lambda 50
             }",
        );
        assert_eq!(function(&weighed, "handler"), 100.0 + 55.0 + 20.0 + 1.0);
        assert_eq!(function(&weighed, "twice"), 100.0);
        assert_eq!(weighed.file, 1.0 + 1.0 + 57.0);
        assert_eq!(class(&weighed, "A"), 1002.0);
        assert_eq!(function(&weighed, "A.onClick"), 103.0);
        assert_eq!(function(&weighed, "A.#p"), 101.0);
        assert_eq!(function(&weighed, "A.m"), 151.0);
        assert_eq!(weighed.entries.len(), 6);
    }

    #[test]
    fn names_overloads_namespaces_and_a_chained_ternary() {
        let weighed = weigh(
            "\"use strict\";                              // a directive: 0
             namespace N {                               // N: class 1000 (its statement 0)
               export function f() {}                    // N.f: 100
               export const h = () => 1;                 // N: statement 1; N.h: 100
             }
             interface I { m(): void; p: number; }       // I: 1000, the property 1; I.m: 100
             function o(a: string): void;                // o: 100, three times
             function o(a: number): void;
             function o(a: any) {}
             const r = a ? b() : c ? d() : e();          // file: statement 1; branch 10 + 10; calls 2 × 2 × 3
             abstract class B {                          // B: 1000
               abstract m(): void;                       // B.m: 100
               constructor() { super(); }                // B.constructor: 100, 1 + 2
             }
             const K = class { k() {} };                 // file: statement 1; K: 1000; K.k: 100",
        );
        assert_eq!(class(&weighed, "N"), 1001.0);
        assert_eq!(function(&weighed, "N.f"), 100.0);
        assert_eq!(function(&weighed, "N.h"), 100.0);
        assert_eq!(class(&weighed, "I"), 1001.0);
        assert_eq!(function(&weighed, "I.m"), 100.0);
        assert_eq!(function(&weighed, "o"), 300.0);
        assert_eq!(class(&weighed, "B"), 1000.0);
        assert_eq!(function(&weighed, "B.m"), 100.0);
        assert_eq!(function(&weighed, "B.constructor"), 103.0);
        assert_eq!(class(&weighed, "K"), 1000.0);
        assert_eq!(function(&weighed, "K.k"), 100.0);
        assert_eq!(weighed.file, 33.0 + 1.0);
    }

    #[test]
    fn a_decorator_weighs_its_calls_where_the_grammar_puts_it() {
        let weighed = weigh(
            "@Component({ selector: \"x\" })    // the export's: the file's call 2
             export class C {                  // 1000
               @Input() name = 1;              // C: statement 1, call 2
               @HostListener(\"click\", () => 1) // beside the method: C's call 2, lambda 50
               onClick(@Inject(T) t) {}        // C.onClick: 100, the parameter's call 2
             }",
        );
        assert_eq!(weighed.file, 2.0);
        assert_eq!(class(&weighed, "C"), 1000.0 + 3.0 + 52.0);
        assert_eq!(function(&weighed, "C.onClick"), 102.0);
    }

    #[test]
    fn a_jsx_element_is_a_call() {
        let weighed = testing::weigh(
            &TSX,
            "function App({ items }: P) {
               return (                                  // the last statement: 1
                 <>                                      // a fragment: call 2
                   <h1>{title}</h1>                      // call 2
                   {ok && <B />}                         // bool_op 5, call 2
                   {items.map((i) => <Li key={i} />)}    // call 2, lambda 50; call 2 × 2 at 1
                 </>
               );
             }",
        );
        assert_eq!(
            function(&weighed, "App"),
            100.0 + 1.0 + 2.0 + 2.0 + 7.0 + 56.0
        );
    }

    #[test]
    fn a_run_of_like_operators_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &TYPESCRIPT,
            "function f() {
               let x = (b + c) * d;        // statement 1, two runs: 14
               x = a ** b ** c;            // 1, one run: 7
               x = -1;                     // 1, a negative number: 0
               x = !!y;                    // 1, one run: 7
               x = typeof y === 's';       // 1, typeof, ===: 14
               x <<= 2;                    // 1, 7
               x ??= y ?? z;               // 1, a logical compound assignment, one run of ??: bool_op 5
               x = a in o || b;            // 1, in 7, bool_op 5
               i--;                        // 1, 7
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 8.0 + 1.0 + 8.0 + 15.0 + 8.0 + 6.0 + 13.0 + 8.0
        );
    }

    #[test]
    fn each_member_or_index_is_one_access_but_a_callee_s_or_this_s() {
        let weighed = testing::weigh_expressions(
            &TSX,
            "class A {
               f() {
                 x = o.a.b;                // statement 1, access 18
                 x = o?.a[i];              // 1, 18
                 x = this.v;               // 1, of `this`: 0
                 o.a.g();                  // 1, call 2, `o.a` 9 (the callee `o.a.g` is the call's)
                 new o.B();                // 1, call 2
                 return <o.C />;           // the last statement: 1, call 2
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 19.0 + 19.0 + 1.0 + 12.0 + 3.0 + 3.0
        );
    }

    #[test]
    fn the_weights_file_s_typescript_table_is_tsx_s_too() {
        let weights = format!(
            "{WEIGHTS}
             [languages.typescript.weights]
             call = 7
             [languages.typescript.node_kinds]
             jsx_self_closing_element = \"none\"
             lexical_declaration = \"loop\""
        );
        let text = "function f() { g(); const x = <A />; }";
        // call 7, the statement 1; the declaration a loop 20; the element none.
        let weighed = testing::weigh_under(&TSX, &weights, text);
        assert_eq!(function(&weighed, "f"), 100.0 + 8.0 + 20.0);
        let weighed = testing::weigh_under(&TYPESCRIPT, &weights, "function f() { g(); }");
        assert_eq!(function(&weighed, "f"), 108.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&TYPESCRIPT, WEIGHTS, "class A { m() { g( } ");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }
}
