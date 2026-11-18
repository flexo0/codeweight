//! Go.

use tree_sitter::Node;

use super::{Children, Lang, Nesting, Operator, Scope, ScopeKind, is_field_of, only_child, text};
use crate::config::Category::{self, *};

pub static GO: Lang = Lang {
    name: "go",
    key: "go",
    extensions: &["go"],
    grammar: || tree_sitter_go::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &["labeled_statement"],
    lists: &["statement_list"],
    lambdas: &["func_literal"],
    parens: &["parenthesized_expression"],
    operator,
    exempt,
    also_statement,
    scope,
    tests: None,
};

const KINDS: &[(&str, Category)] = &[
    // A type definition, of a struct, an interface or any other type (`type Celsius float64`), which can have
    // methods; an alias (`type A = B`) is a statement.
    ("type_spec", Class),
    ("function_declaration", Function),
    ("method_declaration", Function),
    // An interface's method; in an anonymous interface none (`exempt`).
    ("method_elem", Function),
    ("func_literal", Lambda),
    ("if_statement", Branch),
    ("expression_case", Branch),
    ("type_case", Branch),
    ("communication_case", Branch),
    ("default_case", Branch),
    ("for_statement", Loop),
    ("call_expression", Call),
    ("break_statement", Jump),
    ("continue_statement", Jump),
    ("goto_statement", Jump),
    ("fallthrough_statement", Jump),
    ("selector_expression", Access),
    ("index_expression", Access),
    ("slice_expression", Access),
    // `i++` / `i--` (`inc_statement`, `dec_statement`) are operators, and statements beside (`also_statement`).
    ("return_statement", Statement),
    ("expression_statement", Statement),
    ("assignment_statement", Statement),
    ("short_var_declaration", Statement),
    ("send_statement", Statement),
    ("defer_statement", Statement),
    ("go_statement", Statement),
    ("var_spec", Statement),
    ("const_spec", Statement),
    ("type_alias", Statement),
    ("field_declaration", Statement),
];

const fn only(kind: &'static str, body: &'static [&'static str]) -> Nesting {
    Nesting {
        kind,
        body: Children::Only(body),
        chain: None,
    }
}

const NESTING: &[Nesting] = &[
    // `else if`: the `if` in an alternative stays at its depth (as Java's).
    Nesting {
        kind: "if_statement",
        body: Children::Only(&["consequence", "alternative"]),
        chain: Some(Children::Only(&["alternative"])),
    },
    only("for_statement", &["body"]),
    // A `switch` or `select` is no nesting construct; each case is, its values (or its send or receive) at its
    // depth, its statements deeper.
    only("expression_case", &["statement_list"]),
    only("type_case", &["statement_list"]),
    only("communication_case", &["statement_list"]),
    only("default_case", &["statement_list"]),
    only("func_literal", &["body"]),
];

/// The number literals, which a `-` makes a negative number, no operator.
const NUMBERS: &[&str] = &["int_literal", "float_literal", "imaginary_literal"];

/// A binary operator (`&^`, and not, as a bitwise and), a compound assignment, `-x` / `+x` / `!x` / `^x`, `i++` /
/// `i--`. A dereference (`*p`), an address (`&x`) and a receive (`<-ch`) are none: plumbing, not arithmetic.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "binary_expression" => match token()? {
            "&^" => Some(Operator::BitAnd),
            token => Operator::of(token),
        },
        "assignment_statement" => match token()? {
            "&^=" => Some(Operator::BitAnd),
            token => Operator::of_assignment(token),
        },
        "unary_expression" => match token()? {
            "-" | "+" | "!" | "^" => Operator::unary(node.child_by_field_name("operand")?, NUMBERS),
            _ => None,
        },
        "inc_statement" | "dec_statement" => Some(Operator::Unary),
        _ => None,
    }
}

/// A `for` clause's update, part of the loop's head (as Java's `i++` there is no statement); a select case's send
/// (`case ch <- v:`, its label); an anonymous interface's method; a selector that is a call's callee (a method
/// call or a package's function, `a.f()`: the call weighs) or a field of the method's receiver (`r.x`, as Java's
/// `this.x`).
fn exempt(node: Node, source: &[u8]) -> bool {
    if is_update(node) {
        return true;
    }
    match node.kind() {
        "send_statement" => is_field_of(node, &["communication_case"], "communication"),
        "method_elem" => !in_named_interface(node),
        "selector_expression" => {
            is_field_of(node, &["call_expression"], "function") || is_receiver_s(node, source)
        }
        _ => false,
    }
}

/// `i++`, `i--` and `x += 1` (a compound assignment): a statement that is its own operator, as Java's `x++;` is a
/// statement and an operator; but in a `for` clause's update, which is no statement.
fn also_statement(node: Node) -> bool {
    let own_operator = match node.kind() {
        "inc_statement" | "dec_statement" => true,
        "assignment_statement" => operator(node).is_some(),
        _ => false,
    };
    own_operator && !is_update(node)
}

/// Whether a node is a `for` clause's update (`i++` in `for i := 0; i < n; i++`).
fn is_update(node: Node) -> bool {
    is_field_of(node, &["for_clause"], "update")
}

/// Whether an interface's method is of a named interface (`type I interface { M() }`), not of an anonymous one
/// (a parameter's `interface{ Close() error }`).
fn in_named_interface(method: Node) -> bool {
    method
        .parent()
        .and_then(|interface| interface.parent())
        .is_some_and(|spec| spec.kind() == "type_spec")
}

/// Whether a selector is a field of the receiver of the method it is in (`r.x` in `func (r *T) M()`).
fn is_receiver_s(selector: Node, source: &[u8]) -> bool {
    let Some(operand) = selector.child_by_field_name("operand") else {
        return false;
    };
    if operand.kind() != "identifier" {
        return false;
    }
    let mut around = selector.parent();
    while let Some(node) = around {
        match node.kind() {
            "method_declaration" => {
                return receiver(node)
                    .and_then(|r| r.child_by_field_name("name"))
                    .is_some_and(|name| source[name.byte_range()] == source[operand.byte_range()]);
            }
            "function_declaration" => return false,
            _ => around = node.parent(),
        }
    }
    false
}

/// A method's receiver, its parameter (`r *T` in `func (r *T) M()`).
fn receiver(method: Node) -> Option<Node> {
    let list = method.child_by_field_name("receiver")?;
    let mut cursor = list.walk();
    list.named_children(&mut cursor)
        .find(|c| c.kind() == "parameter_declaration")
}

/// A type definition by its name; a function by its name; a method by its receiver's type and its name (`T.M` for
/// `func (r *T) M()`, as Rust's `impl` names them), so a type's methods sit by its class wherever they are; a named
/// interface's method by its name.
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let name = |node: Node| node.child_by_field_name("name").map(|n| text(n, source));
    let (kind, name) = match node.kind() {
        "type_spec" => (ScopeKind::Class, name(node)?),
        "function_declaration" => (ScopeKind::Function, name(node)?),
        "method_declaration" => {
            let method = name(node)?;
            let name = match receiver(node).and_then(|r| r.child_by_field_name("type")) {
                Some(of) => format!("{}.{method}", type_name(of, source)),
                None => method,
            };
            (ScopeKind::Function, name)
        }
        "method_elem" if in_named_interface(node) => (ScopeKind::Function, name(node)?),
        _ => return None,
    };
    Some(Scope { kind, name })
}

/// A receiver type's name without its pointer and type arguments: `*T`, `List[T]` are `T`, `List`.
fn type_name(node: Node, source: &[u8]) -> String {
    let inner = match node.kind() {
        "pointer_type" | "parenthesized_type" => only_child(node),
        "generic_type" => node.child_by_field_name("type"),
        _ => None,
    };
    match inner {
        Some(inner) => type_name(inner, source),
        None => text(node, source),
    }
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::GO;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&GO, text)
    }

    #[test]
    fn branches_an_else_if_chain_and_the_last_return() {
        let weighed = weigh(
            "package p
             func f(x int) int {
               if x > 0 && ok() {          // branch 10, bool_op 5, call 2, at 0
                 g()                       // statement 1, call 2, at 1: 6
               } else if x < 0 {           // branch 10 at 0
                 h()                       // 6
               } else {
                 if x == 0 { k() }         // branch 10 × 2 at 1; 3 × 3 at 2
               }
               if err := m(); err != nil { // branch 10; its initializer: statement 1, call 2
                 return -1                 // an early return: jump 3 × 2
               }
             L:
               return x                    // the last statement, though labeled: statement 1
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 13.0 + 6.0 + 1.0
        );
        assert_eq!(weighed.file, 0.0);
    }

    #[test]
    fn for_switch_and_select_cases_and_jumps() {
        let weighed = weigh(
            "package p
             func f(xs []int, ch chan int) {
               for i := 0; i < len(xs); i++ {   // loop 20; at 0: statement 1, call 2 (the update none)
                 if i > 9 { break }             // at 1: branch 10 × 2; jump 3 × 3
               }
               for _, x := range xs {           // loop 20
                 continue                       // jump 3 × 2
               }
               for { g() }                      // loop 20; at 1: 3 × 2
               switch x := len(xs); x {         // its initializer: statement 1, call 2
               case 0, 1:                       // branch 10
                 g()                            // at 1: 3 × 2
                 fallthrough                    // jump 3 × 2
               default:                         // branch 10
               }
               switch v := any(xs).(type) {     // call 2
               case int:                        // branch 10
               case string, error:              // branch 10
               }
               select {
               case ch <- 1:                    // branch 10, its send none
                 h()                            // 6
               case v, ok := <-ch:              // branch 10
                 goto L                         // jump 3 × 2
               }
             L:
               g()                              // statement 1, call 2
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0
                + 23.0
                + 29.0
                + 26.0
                + 26.0
                + (3.0 + 10.0 + 6.0 + 6.0 + 10.0)
                + (2.0 + 10.0 + 10.0)
                + (10.0 + 6.0 + 10.0 + 6.0)
                + 3.0
        );
    }

    #[test]
    fn func_literals_are_lambdas_and_defer_and_go_statements() {
        let weighed = weigh(
            "package p
             var handler = func() { g() }     // file: statement 1, lambda 50; at 1: 3 × 2
             func f() {
               defer func() {                 // statement 1, call 2, lambda 50
                 if r := recover(); r != nil { // at 1: branch 10 × 2; statement 1 × 2, call 2 × 2
                   return                     // at 2: an early return, jump 3 × 3
                 }
               }()
               go g()                         // statement 1, call 2
               add := func(a, b int) int {    // statement 1, lambda 50
                 return a + b                 // the lambda's last: statement 1 × 2
               }
               add(1, 2)                      // statement 1, call 2
             }",
        );
        assert_eq!(weighed.file, 57.0);
        assert_eq!(
            function(&weighed, "f"),
            100.0 + (53.0 + 20.0 + 6.0 + 9.0) + 3.0 + (51.0 + 2.0) + 3.0
        );
        assert_eq!(weighed.entries.len(), 1);
    }

    #[test]
    fn types_methods_and_interfaces() {
        let weighed = weigh(
            "package p
             import \"fmt\"                         // nothing
             type Point struct {                   // Point: class 1000
               X, Y int                            // a field: statement 1
               Base                                // an embedded one: 1
             }
             func (p *Point) Move(dx int) {        // Point.Move: function 100
               p.X += dx                           // statement 1
             }
             func (Point) String() string {        // Point.String: 100
               return fmt.Sprint(1)                // statement 1, call 2
             }
             func (l *List[T]) Push(v T) {}        // List.Push: 100, no List class here
             type Shape interface {                // Shape: 1000
               Area() float64                      // Shape.Area: 100
               fmt.Stringer                        // an embedded interface: nothing
             }
             type Celsius float64                  // Celsius: 1000
             type ID = string                      // file: statement 1
             type (
               A struct{}                          // A: 1000
               B int                               // B: 1000
             )
             const (
               Red Celsius = iota                  // file: statement 1
               Green                               // 1
             )
             func use(c interface{ Close() error }) { // use: 100, the anonymous method nothing
               var s struct{ n int }               // statement 1, its field 1
               type local struct{ v int }          // use.local: class 1000; its field at 1: 1 × 2
             }",
        );
        assert_eq!(class(&weighed, "Point"), 1002.0);
        assert_eq!(function(&weighed, "Point.Move"), 101.0);
        assert_eq!(function(&weighed, "Point.String"), 103.0);
        assert_eq!(function(&weighed, "List.Push"), 100.0);
        assert_eq!(class(&weighed, "Shape"), 1000.0);
        assert_eq!(function(&weighed, "Shape.Area"), 100.0);
        assert_eq!(class(&weighed, "Celsius"), 1000.0);
        assert_eq!(class(&weighed, "A"), 1000.0);
        assert_eq!(class(&weighed, "B"), 1000.0);
        assert_eq!(function(&weighed, "use"), 102.0);
        assert_eq!(class(&weighed, "use.local"), 1002.0);
        assert_eq!(weighed.file, 3.0);
        assert_eq!(weighed.entries.len(), 11);
        let push = weighed.entries.iter().find(|e| e.name == "List.Push");
        assert_eq!(push.map(|e| e.line), Some(13));
    }

    #[test]
    fn a_run_of_like_operators_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &GO,
            "package p
             func f() {
               x := (b + c) * d          // statement 1, two runs: 14
               y := -1                   // 1, a negative number: 0
               r := !ok && -a < b        // 1, `!` 7, bool_op 5, `<` 7, `-` 7
               m := a &^ b | ^c          // 1, `&^` 7, `|` 7, `^` 7
               q := *p + <-ch            // 1, a dereference and a receive none, `+` 7
               x += 1                    // statement 1, 7
               x &^= 1                   // 1, 7
               x++                       // 1, 7
               x = y                     // 1
               for i := 0; i < 9; i++ {} // loop 20; the initializer 1, `<` 7, the update 7, no statement
             }",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 1.0 + 27.0 + 22.0 + 8.0 + 8.0 + 8.0 + 8.0 + 1.0 + 35.0
        );
    }

    #[test]
    fn each_field_or_index_is_one_access_but_a_callee_s_or_the_receiver_s() {
        let weighed = testing::weigh_expressions(
            &GO,
            "package p
             func (r *T) f() {
               x := o.a.b                // statement 1, access 18
               y := r.v                  // 1, of the receiver: 0
               z := r.v.w                // 1, 9
               o.a.g()                   // 1, call 2, `o.a` 9 (the callee `o.a.g` is the call's)
               w := v[i][j]              // 1, 18
               s := v[1:2]               // 1, 9
               func() { _ = r.v }()      // 1, call 2, lambda 50; at 1: 1 × 2, of the receiver 0
             }
             func g(r *T) { _ = r.v }    // g: 100; statement 1, no receiver here: 9",
        );
        assert_eq!(
            function(&weighed, "T.f"),
            100.0 + 19.0 + 1.0 + 10.0 + 12.0 + 19.0 + 10.0 + 55.0
        );
        assert_eq!(function(&weighed, "g"), 110.0);
    }

    #[test]
    fn the_weights_file_s_go_table() {
        let weights = format!(
            "{WEIGHTS}
             [languages.go.weights]
             call = 7
             [languages.go.node_kinds]
             defer_statement = \"none\""
        );
        let text = "package p\nfunc f() { g(); defer h() }";
        // g(): statement 1, call 7; defer h(): the defer none, call 7.
        let weighed = testing::weigh_under(&GO, &weights, text);
        assert_eq!(function(&weighed, "f"), 100.0 + 8.0 + 7.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&GO, WEIGHTS, "package p\nfunc (r *T) m() { g( }");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }
}
