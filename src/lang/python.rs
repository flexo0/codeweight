//! Python.

use tree_sitter::Node;

use super::{Children, Lang, Logical, Nesting, Scope, ScopeKind, only_child, text};
use crate::config::Category::{self, *};

pub static PYTHON: Lang = Lang {
    name: "python",
    key: "python",
    extensions: &["py", "pyi"],
    grammar: || tree_sitter_python::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    lambdas: &["lambda"],
    parens: &["parenthesized_expression"],
    logical,
    exempt,
    scope,
};

const KINDS: &[(&str, Category)] = &[
    ("class_definition", Class),
    ("function_definition", Function),
    ("lambda", Lambda),
    ("if_statement", Branch),
    ("elif_clause", Branch),
    ("conditional_expression", Branch),
    ("case_clause", Branch),
    // A comprehension's `if`; a case's guard is exempt.
    ("if_clause", Branch),
    ("for_statement", Loop),
    ("while_statement", Loop),
    // A comprehension's `for`.
    ("for_in_clause", Loop),
    ("try_statement", Try),
    ("with_statement", Try),
    ("except_clause", Catch),
    ("call", Call),
    ("break_statement", Jump),
    ("continue_statement", Jump),
    ("raise_statement", Jump),
    ("return_statement", Statement),
    ("expression_statement", Statement),
    ("assert_statement", Statement),
    ("delete_statement", Statement),
    ("type_alias_statement", Statement),
    ("print_statement", Statement),
    ("exec_statement", Statement),
];

const fn only(kind: &'static str, body: &'static [&'static str]) -> Nesting {
    Nesting {
        kind,
        body: Children::Only(body),
        chain: None,
    }
}

/// A comprehension: its element and its `if`s are deeper, its `for`s (their iterables) at its depth.
const fn comprehension(kind: &'static str) -> Nesting {
    Nesting {
        kind,
        body: Children::AllBut(&["for_in_clause"]),
        chain: None,
    }
}

const NESTING: &[Nesting] = &[
    // `elif` and `else` are the if's children beside its body, not in it.
    only("if_statement", &["consequence"]),
    only("elif_clause", &["consequence"]),
    only("else_clause", &["body"]),
    // `x if c else y`: no fields, its value and alternative are the first and third children.
    Nesting {
        kind: "conditional_expression",
        body: Children::Nth(&[0, 2]),
        chain: Some(Children::Nth(&[2])),
    },
    only("case_clause", &["consequence"]),
    only("for_statement", &["body"]),
    only("while_statement", &["body"]),
    only("try_statement", &["body"]),
    only("except_clause", &["block"]),
    only("finally_clause", &["block"]),
    only("with_statement", &["body"]),
    only("lambda", &["body"]),
    comprehension("list_comprehension"),
    comprehension("set_comprehension"),
    comprehension("dictionary_comprehension"),
    comprehension("generator_expression"),
];

fn logical(node: Node) -> Option<Logical> {
    if node.kind() != "boolean_operator" {
        return None;
    }
    match node.child_by_field_name("operator")?.kind() {
        "and" => Some(Logical::And),
        "or" => Some(Logical::Or),
        _ => None,
    }
}

/// A docstring or a bare `...` (an expression statement that is only a string or the ellipsis), and a `match`
/// case's guard (its `if`, which the case's branch already is).
fn exempt(node: Node) -> bool {
    match node.kind() {
        "expression_statement" => only_child(node)
            .is_some_and(|c| matches!(c.kind(), "string" | "concatenated_string" | "ellipsis")),
        "if_clause" => node.parent().is_some_and(|p| p.kind() == "case_clause"),
        _ => false,
    }
}

/// A class or a function (a method, a nested function) by its name. A lambda is no function, even bound to a name.
fn scope(node: Node, source: &[u8]) -> Option<Scope> {
    let kind = match node.kind() {
        "class_definition" => ScopeKind::Class,
        "function_definition" => ScopeKind::Function,
        _ => return None,
    };
    let name = text(node.child_by_field_name("name")?, source);
    Some(Scope { kind, name })
}

/// Snippets weighed under the test weights (see `lang::testing`), each sum worked out by hand.
#[cfg(test)]
mod tests {
    use super::PYTHON;
    use crate::engine::Weighed;
    use crate::lang::testing::{self, WEIGHTS, class, function};

    fn weigh(text: &str) -> Weighed {
        testing::weigh(&PYTHON, text)
    }

    #[test]
    fn branches_an_elif_chain_and_a_chained_conditional() {
        let weighed = weigh(
            r#"
def f(x):
    """A docstring weighs nothing."""
    if x > 0 and ok():               # branch 10, bool_op 5, call 2, at 0
        g()                          # statement 1, call 2, at 1: 6
    elif x < 0:                      # branch 10 at 0
        h()                          # 6
    else:
        if x == 0:                   # branch 10 × 2 at 1
            k()                      # 3 × 3 at 2
    return a if c else b if d else e()   # the last statement: 1; branch 10 + 10 at 0; call 2 × 2 at 1
"#,
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 17.0 + 6.0 + 10.0 + 6.0 + 20.0 + 9.0 + 1.0 + 20.0 + 4.0
        );
    }

    #[test]
    fn a_run_of_like_boolean_operators_is_one_bool_op() {
        let weighed = weigh(
            "r = a and b and c          # statement 1, one run: 5
r = a and b or c           # 1, two runs: 10
r = (a and b) or (c and d) # 1, three runs: 15
r = not (a and b) and c    # 1, a negation starts a run: 10
",
        );
        assert_eq!(weighed.file, 6.0 + 11.0 + 16.0 + 11.0);
    }

    #[test]
    fn loops_comprehensions_try_with_and_match() {
        let weighed = weigh(
            "
def f(xs):
    for x in xs:                     # loop 20
        g(x)                         # 6 at 1
    else:
        h()                          # 6 at 1
    while a or b:                    # loop 20, bool_op 5
        break                        # jump 3 × 2
    ys = [g(y) for y in xs if y]     # statement 1; its for: loop 20; the element's call 2 × 2, its if: branch 10 × 2
    try:                             # try 30
        g()                          # 6
    except E as e:                   # catch 40
        raise                        # jump 3 × 2
    finally:
        h()                          # 6
    with open(p) as fh:              # try 30, call 2
        g()                          # 6
    match x:
        case [1, _] if q():          # branch 10; the guard 0, its call 2
            g()                      # 6
        case _:                      # branch 10
            pass
",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 32.0 + 31.0 + 45.0 + 88.0 + 38.0 + 18.0 + 10.0
        );
    }

    #[test]
    fn classes_decorators_nested_functions_and_lambdas() {
        let weighed = weigh(
            r#"
import os
X = compute()                        # file: statement 1, call 2

@dataclass
class A:                             # class 1000; a bare decorator 0
    n = 1                            # A: statement 1

    @route("/x")                     # A: call 2
    def m(self):                     # A.m: 100
        def inner(y):                # A.m.inner: 100 at 0
            if y:                    # its body nests: branch 10 × 2 at 1
                return 1             # an early return: jump 3 × 3 at 2
            return 2                 # its last statement: 1 × 2 at 1
        f = lambda v: v + g()        # A.m: statement 1, lambda 50; call 2 × 2 at 1
        return f                     # its last statement: 1

    def stub(self): ...              # A.stub: 100; a bare ... weighs nothing
"#,
        );
        assert_eq!(weighed.file, 3.0);
        assert_eq!(class(&weighed, "A"), 1003.0);
        assert_eq!(function(&weighed, "A.m"), 100.0 + 1.0 + 54.0 + 1.0);
        assert_eq!(function(&weighed, "A.m.inner"), 100.0 + 20.0 + 9.0 + 2.0);
        assert_eq!(function(&weighed, "A.stub"), 100.0);
        assert_eq!(weighed.entries.len(), 4);
    }

    #[test]
    fn the_weights_file_overrides_python_s_weights_and_node_kinds() {
        let weights = format!(
            "{WEIGHTS}
             [languages.python.weights]
             call = 7
             [languages.python.node_kinds]
             decorator = \"call\""
        );
        let weighed = testing::weigh_under(&PYTHON, &weights, "@d\ndef f():\n    g()\n");
        assert_eq!(weighed.file, 7.0);
        assert_eq!(function(&weighed, "f"), 100.0 + 8.0);
    }

    #[test]
    fn a_file_with_syntax_errors_is_still_weighed() {
        let weighed = testing::weigh_under(&PYTHON, WEIGHTS, "def f(:\n    g()\n");
        assert!(!weighed.clean);
        assert!(weighed.total > 0.0);
    }
}
