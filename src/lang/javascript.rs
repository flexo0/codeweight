//! JavaScript, JSX too: the grammar TypeScript's is built on, so its node kinds are TypeScript's. It takes
//! `typescript`'s table but for the type kinds, its nesting and its hooks; its table in the weights file is its own
//! (`[languages.javascript]`). A file its grammar leaves errors in, Flow's types mostly (`@flow`, which it has no
//! syntax for), is parsed again by TSX's grammar (`FLOW`), which reads most of them.

use super::Lang;
use super::typescript::{BASE, TSX_TABLES, js_kinds};
use crate::config::Category::*;

pub static JAVASCRIPT: Lang = Lang {
    name: "javascript",
    key: "javascript",
    extensions: &["js", "jsx", "mjs", "cjs"],
    grammar: || tree_sitter_javascript::LANGUAGE.into(),
    kinds: js_kinds!(
        // A class field (TypeScript's `public_field_definition`), a `using` declaration (`let`'s, disposed of).
        ("field_definition", Statement),
        ("using_declaration", Statement),
        // A JSX element is a call, as in TSX: the grammar reads JSX in any file.
        ("jsx_element", Call),
        ("jsx_self_closing_element", Call),
    ),
    fallback: Some(&FLOW),
    ..BASE
};

/// JavaScript read by TSX's grammar, its fallback: TSX's tables (Flow's types weigh as TypeScript's), the weights
/// file's `[languages.javascript]`. It has no extensions: a JavaScript file is weighed by it only when its parse
/// leaves less of the text in errors than JavaScript's.
pub static FLOW: Lang = Lang {
    name: "flow",
    key: "javascript",
    extensions: &[],
    ..TSX_TABLES
};

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::JAVASCRIPT;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&JAVASCRIPT, text)
    }

    #[test]
    fn branches_loops_switch_and_try() {
        let weighed = weigh(
            "function f(xs) {
               if (x > 0 && ok()) {              // branch 10, bool_op 5, call 2, at 0
                 g();                            // statement 1, call 2, at 1: 6
               } else if (x < 0) {               // branch 10 at 0
                 h();                            // 6
               }
               for (const x of xs) { h(x); }     // loop 20; 6
               while (a || b) { k(); }           // loop 20, bool_op 5; 6
               switch (x) {
                 case 1:                         // branch 10
                   break;                        // jump 3 × 2
                 default:                        // branch 10
                   throw new E();                // jump 3 × 2, call 2 × 2
               }
               try {                             // try 30
                 g();                            // 6
               } catch (e) {                     // catch 40
                 h();                            // 6
               }
               return x ?? y;                    // the last statement: statement 1; bool_op 5
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 26.0 + 31.0 + 36.0 + 82.0 + 6.0
        );
        assert_eq!(weighed.file, 0.0);
    }

    #[test]
    fn functions_classes_and_bound_arrows_are_named_as_typescript_s() {
        let weighed = weigh(
            "\"use strict\";                              // a directive: 0
             export const handler = async (req) => {     // file: statement 1; handler: function 100
               const done = () => finish();              // a lambda: statement 1, lambda 50; call, its statement 3 × 2
               return done();                            // the last statement: 1, call 2
             };
             function* gen() { yield 1; }                // gen: 100, statement 1
             class A extends B {                         // class 1000
               onClick = () => { this.go(); };           // A: statement 1; A.onClick: 100, 3
               #p = 1;                                   // A: statement 1
               static s = function () { return 1; };     // A: statement 1; A.s: 100 + 1
               constructor() { super(); }                // A.constructor: 100, 1 + 2
               get v() { return this.#p; }               // A.v: 100 + 1, a getter and its setter one entry
               set v(x) { this.#p = x; }                 // 1
             }
             const K = class { k() {} };                 // file: statement 1; K: 1000; K.k: 100
             module.exports = { handler };               // file: statement 1",
        );
        assert_eq!(function(&weighed, "handler"), 100.0 + 57.0 + 3.0);
        assert_eq!(function(&weighed, "gen"), 101.0);
        assert_eq!(class(&weighed, "A"), 1003.0);
        assert_eq!(function(&weighed, "A.onClick"), 103.0);
        assert_eq!(function(&weighed, "A.s"), 101.0);
        assert_eq!(function(&weighed, "A.constructor"), 103.0);
        assert_eq!(function(&weighed, "A.v"), 202.0);
        assert_eq!(class(&weighed, "K"), 1000.0);
        assert_eq!(function(&weighed, "K.k"), 100.0);
        assert_eq!(weighed.file, 3.0);
    }

    #[test]
    fn a_jsx_element_is_a_call() {
        let weighed = weigh(
            "function App({ items }) {
               return (                                  // the last statement: 1
                 <>                                      // a fragment: call 2
                   <h1>{title}</h1>                      // call 2
                   {ok && <B />}                         // bool_op 5, call 2
                   {items.map((i) => <Li key={i} />)}    // call 2, lambda 50; call and statement 3 × 2 at 1
                 </>
               );
             }",
        );
        assert_eq!(
            function(&weighed, "App"),
            100.0 + 1.0 + 2.0 + 2.0 + 7.0 + 58.0
        );
    }

    #[test]
    fn operators_and_accesses_as_typescript_s() {
        let weighed = testing::weigh_expressions(
            &JAVASCRIPT,
            "function f() {
               let x = (b + c) * d;        // statement 1, two: 14
               x = typeof y === 's';       // 1, typeof, ===: 14
               x = -1;                     // 1, a negative number: 0
               x ??= y;                    // 1, a logical compound assignment: bool_op 5
               x = o?.a[i];                // 1, access 18
               o.a.g();                    // 1, call 2, `o.a` 9
               i++;                        // 1, 7
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 15.0 + 1.0 + 6.0 + 19.0 + 12.0 + 8.0
        );
    }

    #[test]
    fn the_weights_file_s_javascript_table_is_its_own() {
        let weights = format!(
            "{WEIGHTS}
             [languages.javascript.weights]
             call = 7
             [languages.typescript.weights]
             call = 9"
        );
        let text = "function f() { g(); }";
        let weighed = testing::weigh_under(&JAVASCRIPT, &weights, text);
        assert_eq!(function(&weighed, "f"), 108.0);
        let weighed = testing::weigh_under(&crate::lang::typescript::TYPESCRIPT, &weights, text);
        assert_eq!(function(&weighed, "f"), 110.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&JAVASCRIPT, WEIGHTS, "class A { m() { g( } ");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }

    #[test]
    fn flow_s_types_are_read_by_tsx_s_grammar_and_weigh_as_typescript_s() {
        // JavaScript's grammar leaves the types in errors; TSX's reads them, cleanly.
        let weighed = weigh(
            "// @flow
             import type { A } from './a';             // 0
             type P = { a: ?string, b: number };       // file: statement 1, its two members 1 each
             function f(x: ?number, y: A): string {    // f: function 100
               return g((x: any));                     // the last statement 1, call 2; a cast weighs nothing
             }",
        );
        assert_eq!(weighed.file, 3.0);
        assert_eq!(function(&weighed, "f"), 103.0);
    }

    #[test]
    fn javascript_s_grammar_is_kept_unless_tsx_s_leaves_less_in_errors() {
        let weights = WEIGHTS.replacen("operator  = 0", "operator  = 7", 1);
        let weigh = |text| testing::weigh_under(&JAVASCRIPT, &weights, text);
        // `a < b > (c)`: two comparisons to JavaScript's grammar, a call `a<b>(c)` to TSX's.
        let clean = weigh("a < b > (c);                  // statement 1, two operators 7 each");
        assert_eq!((clean.file, clean.clean), (15.0, true));
        // `g(;` is a missing `)` to both: no text in errors either way, JavaScript's kept.
        let tie = weigh("a < b > (c);  g(;              // 15; statement 1, call 2");
        assert_eq!((tie.file, tie.clean), (18.0, false));
        let typed = weigh(
            "a < b > (c);                  // the call: statement 1, call 2
             function f(a: number) {}      // f: 100",
        );
        assert_eq!((typed.file, typed.clean), (3.0, true));
        assert_eq!(function(&typed, "f"), 100.0);
    }

    #[test]
    fn a_file_read_by_tsx_s_grammar_takes_javascript_s_table() {
        let weights = format!(
            "{WEIGHTS}
             [languages.javascript.weights]
             call = 7
             [languages.typescript.weights]
             call = 9"
        );
        let weighed = testing::weigh_under(&JAVASCRIPT, &weights, "function f(x: number) { g(); }");
        assert!(weighed.clean);
        assert_eq!(function(&weighed, "f"), 108.0);
    }

    #[test]
    fn parameters_weigh_by_their_place_as_typescript_s() {
        let weighed = testing::weigh_parameters(
            &JAVASCRIPT,
            "function f(a, b = 1, { c }, ...d) {}   // f: 100; 10 + 15 + 20 + 20
             items.map(x => x);                    // file: statement 1, call 2, lambda 50, 10, statement 1 × 2",
        );
        assert_eq!(function(&weighed, "f"), 165.0);
        assert_eq!(weighed.file, 65.0);
    }

    #[test]
    fn a_call_s_arguments_and_a_switch_s_cases_weigh_as_typescript_s() {
        let arguments = testing::weigh_arguments(
            &JAVASCRIPT,
            "f(a, b);     // statement 1, call 2, 2 arguments 4 each
             new A(c);    // statement 1, call 2, an argument",
        );
        assert_eq!(arguments.file, 11.0 + 7.0);
        let cases = testing::weigh_cases(
            &JAVASCRIPT,
            "switch (x) {
               case 1:    // case 7
                 break;   // jump 3 × 2
               default:   // case 7
             }",
        );
        assert_eq!(cases.file, 13.0 + 7.0);
    }

    #[test]
    fn the_conditions_weigh_as_typescript_s_but_casts() {
        let added = testing::conditions_add(
            &JAVASCRIPT,
            "const g = h ?? 3;   // ??, an operator 7, no bool_op 5; 3 magic: access 9",
        );
        assert_eq!(added, (7.0 - 5.0) + 9.0);
    }

    #[test]
    fn comments_weigh_in_their_entry_or_the_declaration_s_they_are_before() {
        let text = "/* one two */\nfunction f() {\n  /** three\n   * four */\n}\n";
        let comments = testing::comments(&JAVASCRIPT, text);
        assert_eq!((comments.file, function(&comments, "f")), (0.0, 2.0 + 2.0));
    }
}
