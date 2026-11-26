//! Java.

use tree_sitter::Node;

use super::{Lang, Nesting, Operator, Scope, ScopeKind, count_children, is_field_of, text};
use crate::config::Category::{self, *};

pub static JAVA: Lang = Lang {
    name: "java",
    key: "java",
    extensions: &["java"],
    grammar: || tree_sitter_java::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &[],
    lists: &[],
    lambdas: &["lambda_expression"],
    chains: &[],
    casts: &["cast_expression"],
    patterns: &[],
    numbers: NUMBERS,
    operator,
    exempt,
    also_statement: |node| {
        node.kind() != "block" && is_field_of(node, &["lambda_expression"], "body")
    },
    parameters,
    scope,
    tests: None,
    default_case,
    carried_return: |node| {
        node.kind() == "return_statement"
            && node
                .named_child(0)
                .is_some_and(|value| value.kind() == "switch_expression")
    },
    loop_body: |_| None,
    comments: &["line_comment", "block_comment"],
    attributes: &[],
    docstring: |_, _| None,
    inner_doc: |_| false,
    fallback: None,
};

const KINDS: &[(&str, Category)] = &[
    ("class_declaration", Class),
    ("interface_declaration", Class),
    ("enum_declaration", Class),
    ("record_declaration", Class),
    ("annotation_type_declaration", Class),
    ("method_declaration", Function),
    ("constructor_declaration", Function),
    ("compact_constructor_declaration", Function),
    ("lambda_expression", Lambda),
    ("if_statement", Branch),
    ("ternary_expression", Branch),
    ("switch_block_statement_group", Case),
    ("switch_rule", Case),
    ("for_statement", Loop),
    ("enhanced_for_statement", Loop),
    ("while_statement", Loop),
    ("do_statement", Loop),
    ("try_statement", Try),
    ("try_with_resources_statement", Try),
    ("catch_clause", Catch),
    ("method_invocation", Call),
    ("object_creation_expression", Constructor),
    // `new int[3]`, `new Integer[] {1, 2}`: a constructor, as `new Foo()` is (a bare `{1, 2}` initializer is none).
    ("array_creation_expression", Constructor),
    // `this(...)`, `super(...)`.
    ("explicit_constructor_invocation", Constructor),
    ("break_statement", Jump),
    ("continue_statement", Jump),
    ("throw_statement", Jump),
    ("field_access", Access),
    ("array_access", Access),
    ("return_statement", Statement),
    ("expression_statement", Statement),
    ("local_variable_declaration", Statement),
    ("yield_statement", Statement),
    ("assert_statement", Statement),
    ("synchronized_statement", Statement),
    ("field_declaration", Statement),
    ("constant_declaration", Statement),
    ("enum_constant", Statement),
    ("annotation_type_element_declaration", Statement),
];

const NESTING: &[Nesting] = &[
    Nesting::chained("if_statement"),
    Nesting::ternary("ternary_expression"),
    Nesting::all_but("switch_block_statement_group", &["switch_label"]),
    Nesting::all_but("switch_rule", &["switch_label"]),
    Nesting::body("for_statement"),
    Nesting::body("enhanced_for_statement"),
    Nesting::body("while_statement"),
    Nesting::body("do_statement"),
    Nesting::body("try_statement"),
    Nesting::body("try_with_resources_statement"),
    Nesting::body("catch_clause"),
    Nesting::only("finally_clause", &["block"]),
    Nesting::body("lambda_expression"),
];

/// A switch's group or rule whose labels hold `default` (`default:`, `default ->`).
fn default_case(node: Node, _: &[u8]) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == "switch_label")
        .any(|label| {
            let mut cursor = label.walk();
            label
                .children(&mut cursor)
                .any(|token| token.kind() == "default")
        })
}

/// The number literals.
const NUMBERS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
];

/// A binary or unary operator, an assignment, `++` / `--`, `instanceof`. `&` / `|` are bitwise: they can't
/// be told from logical ones without types.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "binary_expression" => Operator::of(token()?),
        "assignment_expression" => Operator::of_assignment(token()?),
        "unary_expression" => Operator::unary(node.child_by_field_name("operand")?, NUMBERS),
        "update_expression" => Some(Operator::Other),
        "instanceof_expression" => Some(Operator::Other),
        _ => None,
    }
}

/// A field of `this` or `super` (`this.x`, as a bare `x` is no access).
fn exempt(node: Node, _: &[u8]) -> bool {
    node.kind() == "field_access"
        && node
            .child_by_field_name("object")
            .is_some_and(|object| matches!(object.kind(), "this" | "super"))
}

/// A class, interface, enum, record or annotation type by its name; an anonymous class as `new Type`; an enum
/// constant's body by the constant's name; a method or constructor by its name (a constructor's is its class's).
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let kind = match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration" => ScopeKind::Class,
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" => {
            ScopeKind::Function
        }
        "class_body" => {
            let parent = node.parent()?;
            let name = match parent.kind() {
                "object_creation_expression" => {
                    let type_name = text(parent.child_by_field_name("type")?, source);
                    let type_name = type_name.split('<').next().unwrap_or_default().trim();
                    format!("new {type_name}")
                }
                "enum_constant" => text(parent.child_by_field_name("name")?, source),
                _ => return None,
            };
            return Some(Scope {
                kind: ScopeKind::Class,
                name,
            });
        }
        _ => return None,
    };
    let name = text(node.child_by_field_name("name")?, source);
    Some(Scope { kind, name })
}

/// A method's, a constructor's or a lambda's parameters (a record's components too, its canonical constructor's),
/// a receiver (`Foo this`) aside; a lambda's lone one (`x -> ...`) and its inferred ones (`(a, b) -> ...`).
fn parameters(node: Node, _: &[u8]) -> usize {
    match node.kind() {
        "formal_parameters" => count_children(node, |c| c.kind() != "receiver_parameter"),
        "inferred_parameters" => count_children(node, |_| true),
        "lambda_expression" => usize::from(
            node.child_by_field_name("parameters")
                .is_some_and(|p| p.kind() == "identifier"),
        ),
        _ => 0,
    }
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::JAVA;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh_under(weights: &str, text: &str) -> Weighed {
        testing::weigh_under(&JAVA, weights, text)
    }

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&JAVA, text)
    }

    #[test]
    fn statements_and_calls() {
        let weighed = weigh(
            "class A {
               void f() {
                 int x = 1;   // statement 1
                 g(x);        // statement 1, call 2
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 1.0 + 3.0);
        assert_eq!(class(&weighed, "A"), 1000.0);
        assert_eq!(weighed.file, 0.0);
        assert_eq!(weighed.total, 1104.0);
    }

    #[test]
    fn an_else_if_chain_is_siblings_and_an_if_in_an_else_block_is_nested() {
        let weighed = weigh(
            "class A {
               int f(int x) {
                 if (x > 0) {         // branch 10 at 0
                   g();               // (1 + 2) × 2 at 1
                 } else if (x < 0) {  // branch 10 at 0
                   h();               // 6 at 1
                 } else {
                   if (x == 0) {      // branch 10 × 2 at 1
                     k();             // 3 × 3 at 2
                   }
                 }
                 return x;            // the last statement: statement 1
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 1.0
        );
    }

    #[test]
    fn a_head_stays_at_its_construct_s_depth() {
        let weighed = weigh(
            "class A {
               void f() {
                 if (a() && b()) {      // branch 10, bool_op 5, calls 2 + 2, all at 0
                   while (c() || d) {   // loop 20 × 2, bool_op 5 × 2, call 2 × 2, at 1
                     e();               // 3 × 3 at 2
                   }
                 }
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 19.0 + 54.0 + 9.0);
    }

    #[test]
    fn each_logical_operator_is_one_bool_op() {
        let weighed = weigh(
            "class A {
               void f() {
                 boolean r = a && b && c;    // statement 1, two: 10
                 r = a && b || c;            // 1, two: 10
                 r = (a && b) || (c && d);   // 1, three: 15
                 r = !(a && b) && c;         // 1, two: 10
                 r = a || g(b || c);         // 1, two: 10, call 2
                 r = a & b | c;              // 1, not logical: 0
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 11.0 + 11.0 + 16.0 + 11.0 + 13.0 + 1.0
        );
    }

    #[test]
    fn each_operator_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &JAVA,
            "class A {
               void f() {
                 int x = b;                // statement 1
                 x = b + c;                // 1, one: 7
                 x = (b + c) * d;          // 1, two: 14
                 x = a + b - c;            // 1, two: 14
                 x = a * b + c * d;        // 1, three: 21
                 x += b + c;               // 1, a compound assignment and its operand's: 14
                 x = -1;                   // 1, a negative number: 0
                 x = -y;                   // 1, 7
                 x++;                      // 1, 7
                 boolean r = a < b && !(c instanceof D);  // 1, bool_op 5, `<`, `!`, instanceof: 21
                 r = a & b | c;            // 1, bitwise: 14
                 r = !!r;                  // 1, each !: 14
                 if (x > 0) {              // branch 10, 7 at 0
                   x = x * 2;              // (1 + 7) × 2 at 1
                 }
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0
                + 1.0
                + 8.0
                + 15.0
                + 15.0
                + 22.0
                + 15.0
                + 1.0
                + 8.0
                + 8.0
                + 27.0
                + 15.0
                + 15.0
                + 17.0
                + 16.0
        );
    }

    #[test]
    fn each_field_access_or_index_is_one_access() {
        let weighed = testing::weigh_expressions(
            &JAVA,
            "class A {
               int v;                      // statement 1
               void f() {
                 x = o.a;                  // statement 1, access 9
                 x = o.a.b;                // 1, 18
                 x = this.v;               // 1, of `this`: 0
                 x = this.v.w;             // 1, 9
                 x = arr[i];               // 1, 9
                 x = o.a[i + 1];           // 1, 18, operator 7
                 o.a.g();                  // 1, call 2, `o.a` 9 (the call names `g` itself)
                 System.out.println(x);    // 1, call 2, `System.out` 9
               }
             }",
        );
        assert_eq!(class(&weighed, "A"), 1001.0);
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 + 19.0 + 1.0 + 10.0 + 10.0 + 26.0 + 12.0 + 12.0
        );
    }

    #[test]
    fn an_access_weighs_more_the_further_it_is_in_a_row() {
        let weighed = testing::weigh_accesses(
            &JAVA,
            "class A {
               int v;                      // statement 1
               void f() {
                 x = o.a.b.c.d;            // statement 1, accesses 9 + 18 + 27 + 27
                 x = this.v.w.u;           // 1, `this.v` 0, then 9 + 18
                 x = arr[i][j];            // 1, 9 + 18
                 x = o.f().a.b;            // 1, call 2, the call ends the row: 9 + 18
                 x = o.a[i.j];             // 1, 9 + 18, the index a row of its own: 9
               }
             }",
        );
        assert_eq!(class(&weighed, "A"), 1001.0);
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 82.0 + 28.0 + 28.0 + 30.0 + 37.0
        );
    }

    #[test]
    fn loops() {
        let weighed = weigh(
            "class A {
               void f() {
                 for (int i = 0; i < n(); i++) {   // loop 20, its init statement 1, call 2, at 0
                   g();                            // 6 at 1
                 }
                 for (int x : xs()) {              // loop 20, call 2
                   h(x);                           // 6 at 1
                 }
                 do {                              // loop 20
                   k();                            // 6 at 1
                 } while (m());                    // call 2 at 0
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 29.0 + 28.0 + 28.0);
    }

    #[test]
    fn switch_cases_are_siblings() {
        let weighed = weigh(
            "class A {
               void f(int x) {
                 switch (x) {
                   case 1:              // branch 10 at 0
                     g();               // 6 at 1
                     break;             // jump 3 × 2 at 1
                   default:             // branch 10 at 0
                     if (x > 1) {       // branch 10 × 2 at 1
                       h();             // 3 × 3 at 2
                     }
                 }
               }
               int r(int x) {
                 return switch (x) {    // the last statement, its value weighing for it: none
                   case 1 -> g();       // branch 10; statement and call at 1: 6
                   default -> {         // branch 10
                     yield 2;           // statement 1 × 2 at 1
                   }
                 };
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 + 6.0 + 6.0 + 10.0 + 20.0 + 9.0
        );
        assert_eq!(function(&weighed, "A.r"), 100.0 + 10.0 + 6.0 + 10.0 + 2.0);
    }

    #[test]
    fn try_catch_and_finally() {
        let weighed = weigh(
            "class A {
               void f() {
                 try {                    // try 30 at 0
                   g();                   // 6 at 1
                 } catch (E e) {          // catch 40 at 0
                   h();                   // 6 at 1
                 } finally {
                   k();                   // 6 at 1
                 }
               }
               void r() {
                 try (R r = open()) {     // try 30, its resource's call 2, at 0
                   g();                   // 6 at 1
                 }
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 30.0 + 6.0 + 40.0 + 6.0 + 6.0
        );
        assert_eq!(function(&weighed, "A.r"), 100.0 + 30.0 + 2.0 + 6.0);
    }

    #[test]
    fn lambdas_and_anonymous_classes_nest() {
        let weighed = weigh(
            "class A {
               void f() {
                 run(() -> g());                 // statement 1, call 2, lambda 50 at 0; g() and its
                                                 // statement (an expression body) 3 × 2 at 1
                 new Thread(new Runnable() {     // statement 1, calls 2 + 2
                   public void run() {           // in the anonymous class, in f: function 100 × 2 at 1
                     h();                        // 3 × 3 at 2
                   }
                 });
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 59.0 + 5.0);
        assert_eq!(function(&weighed, "A.f.new Runnable.run"), 200.0 + 9.0);
        assert_eq!(class(&weighed, "A.f.new Runnable"), 0.0);
        assert_eq!(weighed.total, 1000.0 + 164.0 + 209.0);
    }

    #[test]
    fn a_local_class_nests() {
        let weighed = weigh(
            "class A {
               void f() {
                 class L {            // class 1000 at 0
                   void g() {         // function 100 × 2 at 1
                     h();             // 3 × 3 at 2
                   }
                 }
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0);
        assert_eq!(class(&weighed, "A.f.L"), 1000.0);
        assert_eq!(function(&weighed, "A.f.L.g"), 209.0);
    }

    #[test]
    fn jumps_and_early_returns() {
        let weighed = weigh(
            "class A {
               int f(int x) {
                 if (x < 0) return -1;   // branch 10; an early return: jump 3 × 2 at 1
                 while (true) {          // loop 20
                   if (x > 9) break;     // branch 10 × 2 at 1; jump 3 × 3 at 2
                   x--;                  // statement 1 × 2 at 1
                   continue;             // jump 3 × 2 at 1
                 }
                 throw new E();          // jump 3, call 2
               }
               void g() {
                 h(x -> {                // statement 1, call 2, lambda 50
                   if (x) { return 1; }  // branch 10 × 2 at 1; an early return: jump 3 × 3 at 2
                   return 2;             // the lambda's last statement: statement 1 × 2 at 1
                 });
               }
               int k() {
                 return 1; // a comment after it is not a statement
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 + 6.0 + 20.0 + 20.0 + 9.0 + 2.0 + 6.0 + 5.0
        );
        assert_eq!(
            function(&weighed, "A.g"),
            100.0 + 1.0 + 2.0 + 50.0 + 20.0 + 9.0 + 2.0
        );
        assert_eq!(function(&weighed, "A.k"), 101.0);
    }

    #[test]
    fn a_chained_ternary_is_siblings() {
        let weighed = weigh(
            "class A {
               int r = a ? b() : c ? d() : e();   // statement 1; branch 10 + 10 at 0; calls 2 × 2 × 3 at 1
             }",
        );
        assert_eq!(class(&weighed, "A"), 1000.0 + 1.0 + 20.0 + 12.0);
    }

    #[test]
    fn qualified_names_and_same_named_functions() {
        let weighed = weigh(
            "package p;
             import q.R;
             class Outer {
               int x = init();              // to Outer: statement 1, call 2
               static { setup(); }          // to Outer: 3
               Outer() { this(1); }         // Outer.Outer: 100, call 2
               Outer(int a) { }             // Outer.Outer: 100
               void m() {}                  // Outer.m: 100
               void m(int a) { a(); }       // Outer.m: 100 + 3
               class Inner { void n() {} }
               enum E { A { void f() {} }, B }   // Outer.E: 1000 and a statement 1 per constant
               interface I { void i(); }
               void Inner() {}              // a function apart from the class of the same name
             }
             class Second {}",
        );
        assert_eq!(class(&weighed, "Outer"), 1006.0);
        assert_eq!(function(&weighed, "Outer.Outer"), 202.0);
        assert_eq!(function(&weighed, "Outer.m"), 203.0);
        assert_eq!(class(&weighed, "Outer.Inner"), 1000.0);
        assert_eq!(function(&weighed, "Outer.Inner.n"), 100.0);
        assert_eq!(function(&weighed, "Outer.Inner"), 100.0);
        assert_eq!(class(&weighed, "Outer.E"), 1002.0);
        assert_eq!(class(&weighed, "Outer.E.A"), 0.0);
        assert_eq!(function(&weighed, "Outer.E.A.f"), 100.0);
        assert_eq!(class(&weighed, "Outer.I"), 1000.0);
        assert_eq!(function(&weighed, "Outer.I.i"), 100.0);
        assert_eq!(class(&weighed, "Second"), 1000.0);
        assert_eq!(weighed.file, 0.0);
        let m = weighed
            .entries
            .iter()
            .find(|e| e.name == "Outer.m")
            .unwrap();
        assert_eq!(m.line, 8);
    }

    const DEEP: &str = "class A {
        void f() {
          if (a) {                     // branch 10 at 0
            if (b) {                   // 10 at 1
              if (c) {                 // 10 at 2
                if (d) {               // 10 at 3
                  if (e) {             // 10 at 4, capped at 3
                    g();               // statement 1, call 2, at 5, capped at 3
                  }
                }
              }
            }
          }
        }
      }";

    #[test]
    fn depth_is_capped_at_max_depth() {
        let weighed = weigh(DEEP);
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 * (1.0 + 2.0 + 3.0 + 4.0 + 4.0) + 3.0 * 4.0
        );
    }

    #[test]
    fn the_exponential_model() {
        let weights = WEIGHTS
            .replace("model = \"linear\"", "model = \"exponential\"")
            .replace("factor = 1", "factor = 2");
        let weighed = weigh_under(&weights, DEEP);
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 * (1.0 + 2.0 + 4.0 + 8.0 + 8.0) + 3.0 * 8.0
        );
    }

    #[test]
    fn a_flat_category_weighs_the_same_at_any_depth() {
        let weights = WEIGHTS.replace("flat = []", "flat = [\"call\"]");
        let weighed = weigh_under(&weights, DEEP);
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 10.0 * (1.0 + 2.0 + 3.0 + 4.0 + 4.0) + 1.0 * 4.0 + 2.0
        );
    }

    #[test]
    fn an_expression_body_weighs_as_a_block_returning_it() {
        let expression = weigh("class A { void f() { run(x -> g(x)); } }");
        let block = weigh("class A { void f() { run(x -> { return g(x); }); } }");
        // statement 1, call 2, lambda 50 at 0; the body's statement 1 and call 2 at 1, × 2
        assert_eq!(function(&expression, "A.f"), 100.0 + 53.0 + 6.0);
        assert_eq!(function(&block, "A.f"), function(&expression, "A.f"));
    }

    #[test]
    fn the_weights_file_overrides_java_s_weights_and_node_kinds() {
        let weights = format!(
            "{WEIGHTS}
             [languages.java.weights]
             call = 7
             [languages.java.node_kinds]
             expression_statement = \"none\"
             local_variable_declaration = \"loop\""
        );
        let weighed = weigh_under(
            &weights,
            "class A {
               void f() {
                 g();         // call 7, the statement none
                 int x = 1;   // loop 20
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 7.0 + 20.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = weigh_under(WEIGHTS, "class A { void f() { g( } ");
        assert!(!weighed.clean);
        assert!(weighed.entries.iter().any(|e| e.name == "A"));
        assert!(weighed.total > 0.0);
    }

    #[test]
    fn parameters_weigh_by_their_place() {
        let weighed = testing::weigh_parameters(
            &JAVA,
            "class A {
               void none() {}                                  // function 100
               void f(int a, String b, long c, double d) {}    // 100; 10 + 15 + 20 + 20
               void r(A this, int a) {}                        // 100; the receiver none, a 10
               void v(String... xs) {}                         // 100 + 10
               void l() {                                      // 100
                 run((a, b) -> a);                             // statement 1, call 2, lambda 50, 10 + 15,
                                                               // the body's statement 1 × 2
                 run(x -> x);                                  // 55, 10
                 run((int y) -> y);                            // 55, 10
               }
             }
             record R(int x, int y) {}                         // class 1000; its components 10 + 15",
        );
        assert_eq!(function(&weighed, "A.none"), 100.0);
        assert_eq!(function(&weighed, "A.f"), 165.0);
        assert_eq!(function(&weighed, "A.r"), 110.0);
        assert_eq!(function(&weighed, "A.v"), 110.0);
        assert_eq!(
            function(&weighed, "A.l"),
            100.0 + 55.0 * 3.0 + 25.0 + 10.0 + 10.0
        );
        assert_eq!(class(&weighed, "R"), 1025.0);
    }

    #[test]
    fn a_call_s_arguments_weigh_each() {
        let weighed = testing::weigh_arguments(
            &JAVA,
            "class A {
               void f(boolean c) {    // function 100
                 g();                 // statement 1, call 2, no argument
                 g(a, b.c(), 1);      // statement 1, call 2, 3 arguments 4 each; b.c() a call 2, no argument
                 new B(x, y);         // statement 1, call 2, 2 arguments
                 if (c) {             // branch 10
                   g(a);              // (statement 1, call 2, argument 4) × 2
                 }
               }
             }
             enum E { X(1, 2) }       // class 1000, X a statement 1: an enum constant's arguments are no call's",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 3.0 + 17.0 + 11.0 + 10.0 + 14.0
        );
        assert_eq!(class(&weighed, "E"), 1001.0);
    }

    #[test]
    fn an_early_return_of_a_switch_is_still_a_jump() {
        let weighed = weigh(
            "class A {
               int r(int x) {
                 if (x > 0) {                       // branch 10
                   return switch (x) {              // an early return: jump 3 × 2 at 1
                     default -> 1;                  // branch 10 × 2 at 1; its statement 1 × 3 at 2
                   };
                 }
                 return 0;                          // the last statement: statement 1
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.r"),
            100.0 + 10.0 + 6.0 + 20.0 + 3.0 + 1.0
        );
    }

    #[test]
    fn a_nested_ternary_nests_and_a_default_weighs_nothing_under_their_conditions() {
        let (off, on) = testing::ternaries_and_defaults(
            &JAVA,
            "class A {
               // off: branch 10 + 10 at 0, the calls 2 × 2 × 3 at 1; on: the chained ternary at 1, 10 × 2,
               // its calls at 2, 2 × 3 × 2
               int r = a ? b() : c ? d() : e();
               void f(int x) {
                 switch (x) {
                   case 1: g(); break;
                   default: g();                    // on: the default case 10 weighs nothing
                 }
                 int y = switch (x) {
                   case 1 -> 2;
                   default -> 3;                    // on: nothing either
                 };
               }
             }",
        );
        assert_eq!(
            on - off,
            (10.0 + 4.0 + 20.0 + 12.0) - (20.0 + 12.0) - 10.0 - 10.0
        );
    }

    #[test]
    fn a_plain_assignment_is_an_operator_under_assignments() {
        let added = testing::assignments_add(
            &JAVA,
            "class A {
               void f(int x) {
                 int y = 1;               // a declaration's initializer: none
                 y = 2;                   // operator 7 at 0
                 y += 3;                  // a compound one is an operator either way
                 a = b = 4;               // each: 14
                 switch (x) {
                   case 1 -> y = 5;       // at 1: 14
                   default -> g();
                 }
               }
             }",
        );
        assert_eq!(added, 7.0 + 14.0 + 14.0);
    }

    #[test]
    fn every_constructor_weighs_constructor_not_call() {
        let weighed = testing::weigh_constructors(
            &JAVA,
            "class A {
               A(int x) { this(); }                     // A.A: function 100 × 2, constructor 8 (no statement around it)
               A() { super(); }                         // 8
               void f() {                               // function 100
                 Object a = new Foo();                  // statement 1, constructor 8
                 int[] b = new int[3];                  // an array's: 1 + 8
                 Integer[] c = new Integer[] {1, 2};    // 1 + 8
                 int[] d = {1, 2};                      // a bare initializer: statement 1 only
                 g();                                   // statement 1, call 2
               }
             }",
        );
        assert_eq!(function(&weighed, "A.A"), 200.0 + 8.0 + 8.0);
        assert_eq!(function(&weighed, "A.f"), 100.0 + 9.0 * 3.0 + 1.0 + 3.0);
    }

    #[test]
    fn a_switch_s_cases_weigh_case_not_branch() {
        let weighed = testing::weigh_cases(
            &JAVA,
            "class A {
               int f(int x) {                // function 100
                 switch (x) {
                   case 1: g(); break;       // case 7; (statement 1, call 2, jump 3) × 2
                   default: g();             // case 7; (1 + 2) × 2
                 }
                 int y = switch (x) {        // statement 1
                   case 1 -> 2;              // case 7; its expression statement 1 × 2
                   default -> 3;             // case 7 + 2
                 };
                 return x > 0 ? y : 0;       // statement 1, the ternary a branch 10
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 19.0 + 13.0 + 1.0 + 18.0 + 11.0
        );
    }

    #[test]
    fn the_conditions_weigh_casts_and_magic_numbers() {
        let added = testing::conditions_add(
            &JAVA,
            "class A {
               void f() {
                 int a = (int) x;      // a cast: operator 7
                 int d = 42 + 1;       // 42 magic: access 9; 1 not
                 int e = -1;           // a negative 1: no operator, not magic
               }
             }",
        );
        assert_eq!(added, 7.0 + 9.0);
    }

    #[test]
    fn comments_weigh_in_their_entry_or_the_declaration_s_they_are_before() {
        let text = "/* Header words */

package a;
/** Class doc */
class A {
    /** Method doc */
    @Override
    void f() {
        // inside f
        g(); // trailing
    }

    // between methods

    /**
     * Two words
     */
    void h() {}
}
";
        let comments = testing::comments(&JAVA, text);
        // The header dropped; A's doc and the comment between its methods; f's doc, inside and trailing; h's doc.
        assert_eq!(comments.file, 0.0);
        assert_eq!(class(&comments, "A"), 2.0 + 2.0);
        assert_eq!(function(&comments, "A.f"), 2.0 + 2.0 + 1.0);
        assert_eq!(function(&comments, "A.h"), 2.0);
        // A doc comment at the top, touching its declaration, is no header.
        let comments = testing::comments(&JAVA, "/** Class doc */\nclass A {}\n");
        assert_eq!(class(&comments, "A"), 2.0);
        let header = testing::comments(&JAVA, "// Header words\n\nclass A {}\n");
        assert_eq!(header.total, 0.0);
    }
}
