//! Python.

use tree_sitter::Node;

use super::{
    Children, Lang, Nesting, Operator, Scope, ScopeKind, count_children, first_child, is_field_of,
    only_child, text,
};
use crate::config::Category::{self, *};

pub static PYTHON: Lang = Lang {
    name: "python",
    key: "python",
    extensions: &["py", "pyi"],
    grammar: || tree_sitter_python::LANGUAGE.into(),
    kinds: KINDS,
    nesting: NESTING,
    returns: &["return_statement"],
    wrappers: &[],
    lists: &[],
    lambdas: &["lambda"],
    chains: &["comparison_operator"],
    casts: &[],
    patterns: &[],
    numbers: NUMBERS,
    operator,
    exempt,
    also_statement,
    parameters,
    scope,
    tests: None,
    default_case,
    carried_return: |_| false,
};

/// The number literals.
const NUMBERS: &[&str] = &["integer", "float"];

/// A `case _:`, the wildcard alone and no guard.
fn default_case(node: Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    let patterns: Vec<Node> = node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "case_pattern")
        .collect();
    node.child_by_field_name("guard").is_none()
        && matches!(patterns.as_slice(), [pattern] if text(*pattern, source) == "_")
}

const KINDS: &[(&str, Category)] = &[
    ("class_definition", Class),
    ("function_definition", Function),
    ("lambda", Lambda),
    ("if_statement", Branch),
    ("elif_clause", Branch),
    ("conditional_expression", Branch),
    ("case_clause", Case),
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
    ("attribute", Access),
    ("subscript", Access),
    ("return_statement", Statement),
    ("expression_statement", Statement),
    ("assert_statement", Statement),
    ("delete_statement", Statement),
    ("type_alias_statement", Statement),
    ("print_statement", Statement),
    ("exec_statement", Statement),
];

/// A comprehension: its element and its `if`s are deeper, its `for`s (their iterables) at its depth.
const fn comprehension(kind: &'static str) -> Nesting {
    Nesting::all_but(kind, &["for_in_clause"])
}

const NESTING: &[Nesting] = &[
    // `elif` and `else` are the if's children beside its body, not in it.
    Nesting::only("if_statement", &["consequence"]),
    Nesting::only("elif_clause", &["consequence"]),
    Nesting::body("else_clause"),
    // `x if c else y`: no fields, its value and alternative are the first and third children.
    Nesting {
        kind: "conditional_expression",
        body: Children::Nth(&[0, 2]),
        chain: Some(Children::Nth(&[2])),
        ternary: true,
    },
    Nesting::only("case_clause", &["consequence"]),
    Nesting::body("for_statement"),
    Nesting::body("while_statement"),
    Nesting::body("try_statement"),
    Nesting::only("except_clause", &["block"]),
    Nesting::only("finally_clause", &["block"]),
    Nesting::body("with_statement"),
    Nesting::body("lambda"),
    comprehension("list_comprehension"),
    comprehension("set_comprehension"),
    comprehension("dictionary_comprehension"),
    comprehension("generator_expression"),
];

/// A lambda's body and a comprehension's element (a dict comprehension's pair): an expression body, its value,
/// which weighs a statement as the `return` of it, or the loop's statement making it, would.
fn also_statement(node: Node) -> bool {
    is_field_of(
        node,
        &[
            "lambda",
            "list_comprehension",
            "set_comprehension",
            "dictionary_comprehension",
            "generator_expression",
        ],
        "body",
    )
}

/// A function's or a lambda's parameters (`*args` and `**kwargs` each one), its first one aside when it is `self` or
/// `cls` (as `self.x` is no access); a bare `*` or `/` is no parameter, only where the others are.
fn parameters(node: Node, source: &[u8]) -> usize {
    if !matches!(node.kind(), "parameters" | "lambda_parameters") {
        return 0;
    }
    let all = count_children(node, |c| {
        !matches!(c.kind(), "keyword_separator" | "positional_separator")
    });
    let receiver = first_child(node).is_some_and(|first| {
        let name = match first.kind() {
            "typed_parameter" => first.named_child(0),
            _ => Some(first),
        };
        name.is_some_and(|n| {
            n.kind() == "identifier" && matches!(&source[n.byte_range()], b"self" | b"cls")
        })
    });
    all - usize::from(receiver)
}

/// `and` / `or`, a binary or unary operator (`not` too), a comparison (a chained one, `a < b < c`, an operator per
/// comparison), a compound assignment.
fn operator(node: Node) -> Option<Operator> {
    let token = || node.child_by_field_name("operator").map(|op| op.kind());
    match node.kind() {
        "boolean_operator" | "binary_operator" => Operator::of(token()?),
        "augmented_assignment" => Operator::of_assignment(token()?),
        "comparison_operator" => Some(Operator::Other),
        "unary_operator" => Operator::unary(node.child_by_field_name("argument")?, NUMBERS),
        "not_operator" => Some(Operator::Other),
        _ => None,
    }
}

/// A docstring or a bare `...` (an expression statement that is only a string or the ellipsis), and a `match`
/// case's guard (its `if`, which the case's branch already is). An attribute that is a call's callee (`a.f()`: the
/// call weighs) or of `self` / `cls` (`self.x`, as Java's bare `x` is no access); an attribute or subscript in an
/// annotation (`os.PathLike`, `Dict[str, int]`: a type, as other languages' grammars parse theirs).
fn exempt(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "expression_statement" => only_child(node)
            .is_some_and(|c| matches!(c.kind(), "string" | "concatenated_string" | "ellipsis")),
        "if_clause" => node.parent().is_some_and(|p| p.kind() == "case_clause"),
        "attribute" => {
            is_field_of(node, &["call"], "function")
                || node.child_by_field_name("object").is_some_and(|object| {
                    object.kind() == "identifier"
                        && matches!(&source[object.byte_range()], b"self" | b"cls")
                })
                || in_annotation(node)
        }
        "subscript" => in_annotation(node),
        _ => false,
    }
}

/// Whether a node is in a type annotation.
fn in_annotation(node: Node) -> bool {
    let mut around = node.parent();
    while let Some(node) = around {
        match node.kind() {
            "type" => return true,
            "block" | "module" => return false,
            _ => around = node.parent(),
        }
    }
    false
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
    fn a_nested_conditional_nests_and_a_wildcard_case_weighs_nothing_under_their_conditions() {
        let (off, on) = testing::ternaries_and_defaults(
            &PYTHON,
            "# off: branch 10 + 10 at 0, the calls 2 × 2 × 3 at 1; on: the chained one 10 × 2, its calls 2 × 3 × 2
r = b() if a else d() if c else e()
def f(x):
    match x:
        case 1:
            g()
        case _ if x:        # a guard: no default, case 10 either way
            g()
        case _:             # on: the default case 10 weighs nothing
            g()
",
        );
        assert_eq!(on - off, (10.0 + 4.0 + 20.0 + 12.0) - (20.0 + 12.0) - 10.0);
    }

    #[test]
    fn a_plain_assignment_is_none_under_assignments() {
        // Python's `=` is its declaration too.
        let added = testing::assignments_add(&PYTHON, "y = 1\ny += 2\nif c:\n    y = 3\n");
        assert_eq!(added, 0.0);
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
    fn each_boolean_operator_is_one_bool_op() {
        let weighed = weigh(
            "r = a and b and c          # statement 1, two: 10
r = a and b or c           # 1, two: 10
r = (a and b) or (c and d) # 1, three: 15
r = not (a and b) and c    # 1, two: 10
",
        );
        assert_eq!(weighed.file, 11.0 + 11.0 + 16.0 + 11.0);
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
    ys = [g(y) for y in xs if y]     # statement 1; its for: loop 20; the element's call and
                                     # statement 3 × 2, its if: branch 10 × 2
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
            100.0 + 32.0 + 31.0 + 45.0 + 90.0 + 38.0 + 18.0 + 10.0
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
        f = lambda v: v + g()        # A.m: statement 1, lambda 50; call, statement 3 × 2 at 1
        return f                     # its last statement: 1

    def stub(self): ...              # A.stub: 100; a bare ... weighs nothing
"#,
        );
        assert_eq!(weighed.file, 3.0);
        assert_eq!(class(&weighed, "A"), 1003.0);
        assert_eq!(function(&weighed, "A.m"), 100.0 + 1.0 + 56.0 + 1.0);
        assert_eq!(function(&weighed, "A.m.inner"), 100.0 + 20.0 + 9.0 + 2.0);
        assert_eq!(function(&weighed, "A.stub"), 100.0);
        assert_eq!(weighed.entries.len(), 4);
    }

    #[test]
    fn each_operator_is_one_operator() {
        let weighed = testing::weigh_expressions(
            &PYTHON,
            "def f():
    x = (b + c) * d     # statement 1, two: 14
    x = a < b <= c      # 1, a chained comparison, each of its two: 14
    x = k not in y      # 1, 7
    x = not a and b     # 1, `not` 7, bool_op 5
    x //= 2             # 1, 7
    x = -1              # 1, a negative number: 0
    x = -y @ z          # 1, two: 14
",
        );
        assert_eq!(
            function(&weighed, "f"),
            100.0 + 15.0 + 15.0 + 8.0 + 13.0 + 8.0 + 1.0 + 15.0
        );
    }

    #[test]
    fn each_attribute_or_subscript_is_one_access_but_a_callee_s_or_self_s() {
        let weighed = testing::weigh_expressions(
            &PYTHON,
            "class A:
    def f(self, x: Dict[str, int] = o.d) -> os.PathLike:   # an annotation's: 0; the default's 9
        x: os.PathLike = o.a.b  # statement 1, access 18
        x = self.v      # 1, of `self`: 0
        x = self.v.w    # 1, 9
        x = cls.k       # 1, of `cls`: 0
        o.a.g()         # 1, call 2, `o.a` 9 (the callee `o.a.g` is the call's)
        x = d[k][0]     # 1, 18
",
        );
        assert_eq!(
            function(&weighed, "A.f"),
            100.0 + 9.0 + 19.0 + 1.0 + 10.0 + 1.0 + 12.0 + 19.0
        );
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

    #[test]
    fn parameters_weigh_by_their_place() {
        let weighed = testing::weigh_parameters(
            &PYTHON,
            r#"
def f(a, b: int, c=1, *args, d, **kw):  # f: 100; 10 + 15 + 20 + 20 + 20 + 20
    pass
def g(a, /, b, *, c):                   # g: 100; the bare / and * none: 10 + 15 + 20
    pass
class A:                                # A: 1000
    def m(self, x):                     # A.m: 100; self none, x 10
        pass
    def k(cls):                         # A.k: 100
        pass
    def s(self: "A"):                   # A.s: 100
        pass
h = lambda x, y: x                      # file: statement 1, lambda 50, 10 + 15, the body's statement 1 × 2
"#,
        );
        assert_eq!(function(&weighed, "f"), 205.0);
        assert_eq!(function(&weighed, "g"), 145.0);
        assert_eq!(class(&weighed, "A"), 1000.0);
        assert_eq!(function(&weighed, "A.m"), 110.0);
        assert_eq!(function(&weighed, "A.k"), 100.0);
        assert_eq!(function(&weighed, "A.s"), 100.0);
        assert_eq!(weighed.file, 78.0);
    }

    #[test]
    fn a_call_s_arguments_weigh_each() {
        let weighed = testing::weigh_arguments(
            &PYTHON,
            "
f()                    # statement 1, call 2, no argument
f(a, b=1, *c, **d)     # statement 1, call 2, 4 arguments 4 each
sum(x for x in xs)     # statement 1, call 2, the generator one argument 4; its for a loop 20, its element a
                       # statement 1 × 2
class A(B, C):         # A: class 1000, its bases no call's arguments
    x = 1              # statement 1
",
        );
        assert_eq!(weighed.file, 3.0 + 19.0 + 29.0);
        assert_eq!(class(&weighed, "A"), 1001.0);
    }

    #[test]
    fn a_match_s_cases_weigh_case_not_branch() {
        let weighed = testing::weigh_cases(
            &PYTHON,
            "
match x:
    case 1:            # case 7
        f()            # (statement 1, call 2) × 2
    case _:            # case 7
        y = 1          # statement 1 × 2
if x:                  # branch 10
    y = 2              # statement 1 × 2
",
        );
        assert_eq!(weighed.file, 13.0 + 9.0 + 12.0);
    }

    #[test]
    fn the_conditions_weigh_magic_numbers() {
        let added = testing::conditions_add(
            &PYTHON,
            "
c = d * 3.14       # 3.14 magic: access 9
e = x[0] + 1       # 0 and 1 not
",
        );
        assert_eq!(added, 9.0);
    }
}
