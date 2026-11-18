//! Java.

use tree_sitter::Node;

use super::{Children, Lang, Logical, Nesting, Scope, ScopeKind, text};
use crate::config::Category::{self, *};

pub static JAVA: Lang = Lang {
    name: "java",
    key: "java",
    extensions: &["java"],
    grammar: || tree_sitter_java::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    lambdas: &["lambda_expression"],
    parens: &["parenthesized_expression"],
    logical,
    exempt: |_| false,
    scope,
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
    ("switch_block_statement_group", Branch),
    ("switch_rule", Branch),
    ("for_statement", Loop),
    ("enhanced_for_statement", Loop),
    ("while_statement", Loop),
    ("do_statement", Loop),
    ("try_statement", Try),
    ("try_with_resources_statement", Try),
    ("catch_clause", Catch),
    ("method_invocation", Call),
    ("object_creation_expression", Call),
    ("explicit_constructor_invocation", Call),
    ("break_statement", Jump),
    ("continue_statement", Jump),
    ("throw_statement", Jump),
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
    Nesting {
        kind: "if_statement",
        body: Children::Only(&["consequence", "alternative"]),
        chain: Some(Children::Only(&["alternative"])),
    },
    Nesting {
        kind: "ternary_expression",
        body: Children::Only(&["consequence", "alternative"]),
        chain: Some(Children::Only(&["alternative"])),
    },
    Nesting {
        kind: "switch_block_statement_group",
        body: Children::AllBut(&["switch_label"]),
        chain: None,
    },
    Nesting {
        kind: "switch_rule",
        body: Children::AllBut(&["switch_label"]),
        chain: None,
    },
    Nesting {
        kind: "for_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "enhanced_for_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "while_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "do_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "try_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "try_with_resources_statement",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "catch_clause",
        body: Children::Only(&["body"]),
        chain: None,
    },
    Nesting {
        kind: "finally_clause",
        body: Children::Only(&["block"]),
        chain: None,
    },
    Nesting {
        kind: "lambda_expression",
        body: Children::Only(&["body"]),
        chain: None,
    },
];

fn logical(node: Node) -> Option<Logical> {
    if node.kind() != "binary_expression" {
        return None;
    }
    match node.child_by_field_name("operator")?.kind() {
        "&&" => Some(Logical::And),
        "||" => Some(Logical::Or),
        _ => None,
    }
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
    fn a_run_of_like_logical_operators_is_one_bool_op() {
        let weighed = weigh(
            "class A {
               void f() {
                 boolean r = a && b && c;    // statement 1, one run: 5
                 r = a && b || c;            // 1, two runs: 10
                 r = (a && b) || (c && d);   // 1, three runs: 15
                 r = !(a && b) && c;         // 1, a negation starts a run: 10
                 r = a || g(b || c);         // 1, a call's argument starts a run: 10, call 2
                 r = a & b | c;              // 1, not logical: 0
               }
             }",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 6.0 + 11.0 + 16.0 + 11.0 + 13.0 + 1.0
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
                 return switch (x) {    // the last statement: statement 1
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
        assert_eq!(
            function(&weighed, "A.r"),
            100.0 + 1.0 + 10.0 + 6.0 + 10.0 + 2.0
        );
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
                 run(() -> g());                 // statement 1, call 2, lambda 50 at 0; g() 2 × 2 at 1
                 new Thread(new Runnable() {     // statement 1, calls 2 + 2
                   public void run() {           // in the anonymous class, in f: function 100 × 2 at 1
                     h();                        // 3 × 3 at 2
                   }
                 });
               }
             }",
        );
        assert_eq!(function(&weighed, "A.f"), 100.0 + 57.0 + 5.0);
        assert_eq!(function(&weighed, "A.f.new Runnable.run"), 200.0 + 9.0);
        assert_eq!(class(&weighed, "A.f.new Runnable"), 0.0);
        assert_eq!(weighed.total, 1000.0 + 162.0 + 209.0);
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
}
