//! The languages: each one's grammar, file extensions, node kind → category table, nesting constructs and how its
//! functions and classes are named. The engine reads these tables; it knows no language itself.

pub mod c;
pub mod go;
pub mod java;
pub mod javascript;
pub mod python;
pub mod rust;
pub mod typescript;

use std::path::Path;

use tree_sitter::Node;

use crate::config::Category;

/// Every language codeweight reads.
pub const ALL: &[&Lang] = &[
    &java::JAVA,
    &typescript::TYPESCRIPT,
    &typescript::TSX,
    &javascript::JAVASCRIPT,
    &python::PYTHON,
    &rust::RUST,
    &go::GO,
    &c::C,
];

/// A language's tables.
pub struct Lang {
    /// Its name, for messages.
    pub name: &'static str,
    /// Its key in the weights file (`[languages.<key>]`); languages of one key (TypeScript and TSX) share its table.
    pub key: &'static str,
    /// The file extensions it is read from, without the dot.
    pub extensions: &'static [&'static str],
    pub grammar: fn() -> tree_sitter::Language,
    /// A node kind → its category; a kind not listed has none.
    pub kinds: &'static [(&'static str, Category)],
    /// The constructs that make their body one level deeper.
    pub nesting: &'static [Nesting],
    /// The kinds of a return: an early one (not its function's or lambda's last statement) is a jump.
    pub returns: &'static [&'static str],
    /// The kinds of a statement around another (Rust's `expression_statement`, an expression and its `;`; Go's
    /// `labeled_statement`): a return in one is at the statement's place, so the last statement of a body is a
    /// return so wrapped, not an early one.
    pub wrappers: &'static [&'static str],
    /// The kinds of a list of statements a grammar puts between a body and its statements (Go's `statement_list`):
    /// a return that is the last of the list is its body's last statement.
    pub lists: &'static [&'static str],
    /// The kinds of a lambda, whose last statement isn't an early return either.
    pub lambdas: &'static [&'static str],
    /// The kinds of an operator node that chains its operands (Python's `a < b < c`, Rust's let chain): an operator
    /// for each operand after the first.
    pub chains: &'static [&'static str],
    /// The kinds of a cast or type assertion: an `operator` under `[conditions] casts`.
    pub casts: &'static [&'static str],
    /// The kinds of a pattern tested in a condition (Rust's `let` in `if let`): an `operator` under
    /// `[conditions] let_patterns`.
    pub patterns: &'static [&'static str],
    /// The kinds of a number literal: a `-` before one makes a negative number, no operator, and one but 0 and 1 is
    /// an `access` under `[conditions] magic_numbers`.
    pub numbers: &'static [&'static str],
    /// The operator of an expression, if the node is one: a binary or unary operator, a compound assignment, `++`.
    pub operator: fn(Node) -> Option<Operator>,
    /// Whether a node weighs nothing although its kind has a category: the cases its kind can't tell (a docstring,
    /// a `match` case's guard, a call's callee, `this.x`).
    pub exempt: fn(Node, &[u8]) -> bool,
    /// Whether a node weighs a statement beside its own kind's category: Rust's block that ends in a value, an
    /// expression without its `;` (as the `return` of that value would); an expression body (a lambda's, a Rust
    /// arm's value, a Python comprehension's element), which is no statement but holds the place of one, so that
    /// its depth weighs as a block's statement's would; Go's `i++` and `x += 1`, statements that are their own
    /// operator (as Java's `x++;` is a statement and an operator).
    pub also_statement: fn(Node) -> bool,
    /// The number of parameters a node declares: a function's or lambda's list of them (a receiver, `self` or
    /// `this` aside; not a function type's), or a lambda's lone one (`x -> ...`); 0 for any other node.
    pub parameters: fn(Node, &[u8]) -> usize,
    /// The function or class a node declares, if it does one, and its own name (not qualified). A lambda it names
    /// a function is one: it weighs as a function, and nests as one, not as a lambda.
    pub scope: fn(Node, &[u8]) -> Option<Scope>,
    /// Whether a node is test code in a source file, it and all in it (Rust's `#[cfg(test)]` and `#[test]`
    /// items); `None` for a language whose tests are files of their own, which the test globs find.
    pub tests: Option<fn(Node, &[u8]) -> bool>,
    /// Whether a case is its switch's or match's default (`default`, Rust's and Python's bare `_`), which weighs
    /// nothing under `[conditions] free_default`, as an `else` weighs nothing.
    pub default_case: fn(Node, &[u8]) -> bool,
    /// Whether a return that is its body's last weighs no statement, its value weighing for it: Java's `return
    /// switch (x) { ... };`, as a switch statement there would weigh none.
    pub carried_return: fn(Node) -> bool,
    /// Whether a node is a loop its kind doesn't tell, and if so its body: C's loop macro, which the grammar reads
    /// as a call missing its `;` beside a block, the node after it (`for_each(x, list) { ... }`), or with one
    /// argument as a definition, its body (`FOR_EACH(x) { ... }`). The node weighs a `loop`, not its kind's
    /// category; its body, a child or not, is one level deeper than it.
    pub loop_body: for<'t> fn(Node<'t>) -> Option<Node<'t>>,
}

/// A nesting construct: which of its children are one level deeper than it.
pub struct Nesting {
    pub kind: &'static str,
    /// Its children one level deeper.
    pub body: Children,
    /// Its children that stay at its depth when they are of its own kind (`else if`, a chained ternary).
    pub chain: Option<Children>,
    /// Whether it is a ternary, whose chain nests under `[conditions] nested_ternaries`.
    pub ternary: bool,
}

impl Nesting {
    /// A construct whose children of these field names or node kinds are deeper.
    pub const fn only(kind: &'static str, body: &'static [&'static str]) -> Nesting {
        Nesting {
            kind,
            body: Children::Only(body),
            chain: None,
            ternary: false,
        }
    }

    /// A construct whose `body` is deeper: a loop, a `try`, a lambda.
    pub const fn body(kind: &'static str) -> Nesting {
        Nesting::only(kind, &["body"])
    }

    /// A construct whose children are all deeper but those of these field names or node kinds.
    pub const fn all_but(kind: &'static str, but: &'static [&'static str]) -> Nesting {
        Nesting {
            kind,
            body: Children::AllBut(but),
            chain: None,
            ternary: false,
        }
    }

    /// An `if` or a ternary whose `consequence` and `alternative` are deeper, its `alternative` at its depth when it
    /// is of its own kind (`else if`, a chained ternary).
    pub const fn chained(kind: &'static str) -> Nesting {
        Nesting {
            kind,
            body: Children::Only(&["consequence", "alternative"]),
            chain: Some(Children::Only(&["alternative"])),
            ternary: false,
        }
    }

    /// A ternary (`c ? x : y`), chained as an `if` (`a ? x : b ? y : z` siblings) but under
    /// `[conditions] nested_ternaries`, where its chained alternative nests as its consequence does.
    pub const fn ternary(kind: &'static str) -> Nesting {
        Nesting {
            ternary: true,
            ..Nesting::chained(kind)
        }
    }
}

/// Some of a node's named children.
pub enum Children {
    /// Those matched by their field name or their node kind.
    Only(&'static [&'static str]),
    /// All but those matched by their field name or their node kind.
    AllBut(&'static [&'static str]),
    /// Those at these places among the named children, comments aside, from 0 (for a grammar that gives them no
    /// field).
    Nth(&'static [usize]),
}

impl Children {
    /// Whether the `nth` named child, in `field` and of kind `kind`, is one of them.
    pub fn hold(&self, nth: usize, field: Option<&str>, kind: &str) -> bool {
        let named = |names: &[&str]| names.iter().any(|n| *n == kind || Some(*n) == field);
        match self {
            Children::Only(names) => named(names),
            Children::AllBut(names) => !named(names),
            Children::Nth(places) => places.contains(&nth),
        }
    }

    /// The field names and node kinds it matches by.
    #[cfg(test)]
    pub fn names(&self) -> &'static [&'static str] {
        match self {
            Children::Only(names) | Children::AllBut(names) => names,
            Children::Nth(_) => &[],
        }
    }
}

/// An operator, by the category it weighs. Each one counts, a chain of them too: `a + b + c` is 2, `a && b && c` 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    /// `&&`, `||`, `and`, `or`, a let chain's `&&`: a `bool_op`.
    Logical,
    /// TypeScript's and JavaScript's `??`: a `bool_op`, an `operator` under `[conditions] nullish_operator`.
    Nullish,
    /// Any other: arithmetic, comparison (`instanceof`, `in`, `is` too), bitwise, unary (`-x`, `!x`, `not x`,
    /// `typeof x`), `++` / `--`, a compound assignment's (`+=` is `+`): an `operator`.
    Other,
    /// A plain `=` assignment, a declaration's initializer aside: an `operator` beside the node's own weight under
    /// `[conditions] assignments`, else nothing.
    Assign,
}

impl Operator {
    /// A binary operator by its token.
    pub fn of(token: &str) -> Option<Operator> {
        Some(match token {
            "&&" | "||" | "and" | "or" => Operator::Logical,
            "??" => Operator::Nullish,
            "+" | "-" | "*" | "/" | "%" | "//" | "@" | "**" | "<<" | ">>" | ">>>" | "==" | "!="
            | "===" | "!==" | "<" | "<=" | ">" | ">=" | "<>" | "instanceof" | "in" | "not in"
            | "is" | "is not" | "&" | "|" | "^" => Operator::Other,
            _ => return None,
        })
    }

    /// An assignment's operator by its token: a compound one's (`+=` is `+`), a plain `=`'s `Assign`.
    pub fn of_assignment(token: &str) -> Option<Operator> {
        match token {
            "=" => Some(Operator::Assign),
            token => Operator::of(token.strip_suffix('=')?),
        }
    }

    /// A unary operator on `operand`; none on a number (`-1` is a literal).
    pub fn unary(operand: Node, numbers: &[&str]) -> Option<Operator> {
        (!numbers.contains(&operand.kind())).then_some(Operator::Other)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeKind {
    Class,
    Function,
}

/// A function or class a node declares.
pub struct Scope {
    pub kind: ScopeKind,
    pub name: String,
}

/// The languages whose table is `[languages.<key>]`.
pub fn by_key(key: &str) -> impl Iterator<Item = &'static Lang> + '_ {
    ALL.iter().copied().filter(move |l| l.key == key)
}

/// The weights file's language keys, each once, in `ALL`'s order.
pub fn keys() -> Vec<&'static str> {
    let mut keys: Vec<&str> = Vec::new();
    for lang in ALL {
        if !keys.contains(&lang.key) {
            keys.push(lang.key);
        }
    }
    keys
}

/// The language of a file, by its extension.
pub fn for_path(path: &Path) -> Option<&'static Lang> {
    let extension = path.extension()?.to_str()?;
    ALL.iter()
        .copied()
        .find(|l| l.extensions.contains(&extension))
}

/// Whether `kind` is a named node kind of `grammar`.
pub fn is_named_kind(grammar: &tree_sitter::Language, kind: &str) -> bool {
    named_kind_ids(grammar, kind).next().is_some()
}

/// The ids of the named node kind `kind` (a kind may have several, when the grammar aliases it).
pub fn named_kind_ids<'a>(
    grammar: &'a tree_sitter::Language,
    kind: &'a str,
) -> impl Iterator<Item = u16> + 'a {
    (0..grammar.node_kind_count() as u16).filter(move |&id| {
        grammar.node_kind_is_named(id) && grammar.node_kind_for_id(id) == Some(kind)
    })
}

/// Whether `node` is its parent's `field`, its parent of one of `kinds` (a call's callee).
pub fn is_field_of(node: Node, kinds: &[&str], field: &str) -> bool {
    node.parent().is_some_and(|parent| {
        kinds.contains(&parent.kind())
            && parent
                .child_by_field_name(field)
                .is_some_and(|child| child.id() == node.id())
    })
}

/// A node's one named child, comments aside, if it has just one.
pub fn only_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor).filter(|c| !c.is_extra());
    let first = children.next()?;
    children.next().is_none().then_some(first)
}

/// A node's last named child, comments aside, if it has any.
pub fn last_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !c.is_extra())
        .last()
}

/// The number of a node's named children, comments aside, that `counts`.
pub fn count_children(node: Node, counts: impl Fn(Node) -> bool) -> usize {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !c.is_extra() && counts(*c))
        .count()
}

/// A node's first named child, comments aside, if it has any.
pub fn first_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| !c.is_extra())
}

/// The number of arguments a call passes: the items of its `arguments` field's list (comments aside), or 1 for a
/// lone argument in their place (Python's `f(x for x in xs)`, a tagged template's string); 0 without the field (a
/// Rust macro, whose arguments are tokens; a JSX element; `new A` without parentheses).
pub fn arguments(call: Node) -> usize {
    match call.child_by_field_name("arguments") {
        Some(list) if matches!(list.kind(), "argument_list" | "arguments") => {
            count_children(list, |_| true)
        }
        Some(_) => 1,
        None => 0,
    }
}

/// The text of a node, lossy where it isn't UTF-8.
pub fn text(node: Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).into_owned()
}

/// The languages' snippet tests: weighed under `tests/weights.toml` (statement 1, call 2, argument 0, branch 10, case
/// 10, loop 20, try 30, catch 40, lambda 50, function 100, class 1000, bool_op 5, jump 3, operator 0, access 0;
/// nest(depth) = 1 + depth, depth capped at 3), each sum worked out by hand: `weight × nest(depth)` per element. The
/// snippets about operators and accesses are weighed with those at 7 and 9 (`weigh_expressions`), those about
/// parameters with `parameter` 10 (`weigh_parameters`): 10, 15, 20, then 20 each, by their place; those about
/// arguments with `argument` 4 (`weigh_arguments`), those about cases with `case` 7 (`weigh_cases`), those about
/// the `[conditions]` with every one on and `operator` 7, `access` 9 (`conditions_add`), those about a row of
/// accesses with `access` 9 growing by its place, 9, 18, 27, then 27 each (`weigh_accesses`).
#[cfg(test)]
pub mod testing {
    use super::{Lang, ScopeKind};
    use crate::config::{Category, Config};
    use crate::engine::{Weighed, Weigher};

    pub const WEIGHTS: &str = include_str!("../../tests/weights.toml");

    pub fn weigh_under(lang: &'static Lang, weights: &str, text: &str) -> Weighed {
        let config = Config::from_str_alone(weights).unwrap();
        let weighed = Weigher::new(lang, &config).weigh(text).unwrap();
        let sum = weighed.outside() + weighed.entries.iter().map(|e| e.weight).sum::<f64>();
        assert_eq!(weighed.total, sum);
        weighed
    }

    /// A snippet that parses cleanly, weighed under the test weights.
    pub fn weigh(lang: &'static Lang, text: &str) -> Weighed {
        let weighed = weigh_under(lang, WEIGHTS, text);
        assert!(weighed.clean, "syntax errors in {text}");
        weighed
    }

    /// A snippet that parses cleanly, weighed under the test weights with `operator` 7 and `access` 9.
    pub fn weigh_expressions(lang: &'static Lang, text: &str) -> Weighed {
        let weights = WEIGHTS
            .replacen("operator  = 0", "operator  = 7", 1)
            .replacen("access    = 0", "access    = 9", 1);
        let weighed = weigh_under(lang, &weights, text);
        let config = Config::from_str_alone(&weights).unwrap();
        let weights = config.lang(lang);
        assert_eq!(
            (
                weights.weight(Category::Operator),
                weights.weight(Category::Access)
            ),
            (7.0, 9.0)
        );
        assert!(weighed.clean, "syntax errors in {text}");
        weighed
    }

    /// A snippet that parses cleanly, weighed under the test weights with `operator` 7, `access` 9 and
    /// `[accesses] step = 1, max = 3`: an access in a row weighs 9, 18, 27, then 27 each, by its place.
    pub fn weigh_accesses(lang: &'static Lang, text: &str) -> Weighed {
        let weights = WEIGHTS
            .replacen("operator  = 0", "operator  = 7", 1)
            .replacen("access    = 0", "access    = 9", 1)
            + "\n[accesses]\nstep = 1\nmax = 3\n";
        let config = Config::from_str_alone(&weights).unwrap();
        assert_eq!(config.accesses.factor_at(2), 3.0);
        let weighed = weigh_under(lang, &weights, text);
        assert!(weighed.clean, "syntax errors in {text}");
        weighed
    }

    /// A snippet that parses cleanly, weighed under the test weights with `parameter` 10.
    pub fn weigh_parameters(lang: &'static Lang, text: &str) -> Weighed {
        let weights = WEIGHTS.replacen("parameter = 0 ", "parameter = 10 ", 1);
        let config = Config::from_str_alone(&weights).unwrap();
        assert_eq!(config.lang(lang).weight(Category::Parameter), 10.0);
        let weighed = weigh_under(lang, &weights, text);
        assert!(weighed.clean, "syntax errors in {text}");
        weighed
    }

    /// What every `[conditions]` rule on adds to a snippet's total, under the test weights with `operator` 7 and
    /// `access` 9 (the categories the conditions weigh as).
    pub fn conditions_add(lang: &'static Lang, text: &str) -> f64 {
        let off = weigh_expressions(lang, text);
        let on = WEIGHTS
            .replacen("operator  = 0", "operator  = 7", 1)
            .replacen("access    = 0", "access    = 9", 1)
            + "\n[conditions]\ncasts = true\nlet_patterns = true\nmagic_numbers = true\n\
               nullish_operator = true\n";
        let config = Config::from_str_alone(&on).unwrap();
        assert!(config.conditions.casts && config.conditions.nullish_operator);
        let on = weigh_under(lang, &on, text);
        assert!(on.clean, "syntax errors in {text}");
        on.total - off.total
    }

    /// A snippet that parses cleanly, weighed under the test weights with `argument` 4.
    pub fn weigh_arguments(lang: &'static Lang, text: &str) -> Weighed {
        weigh_with(
            lang,
            "argument  = 0 ",
            "argument  = 4 ",
            Category::Argument,
            4.0,
            text,
        )
    }

    /// What `[conditions] assignments` on adds to a snippet's total, under the test weights with `operator` 7.
    pub fn assignments_add(lang: &'static Lang, text: &str) -> f64 {
        let off = weigh_expressions(lang, text);
        let on = WEIGHTS
            .replacen("operator  = 0", "operator  = 7", 1)
            .replacen("access    = 0", "access    = 9", 1)
            + "\n[conditions]\nassignments = true\n";
        let config = Config::from_str_alone(&on).unwrap();
        assert!(config.conditions.assignments);
        let on = weigh_under(lang, &on, text);
        assert!(on.clean, "syntax errors in {text}");
        on.total - off.total
    }

    /// A snippet's total under the test weights, without and with `[conditions] nested_ternaries` and
    /// `free_default` on.
    pub fn ternaries_and_defaults(lang: &'static Lang, text: &str) -> (f64, f64) {
        let on = format!("{WEIGHTS}\n[conditions]\nnested_ternaries = true\nfree_default = true\n");
        let config = Config::from_str_alone(&on).unwrap();
        assert!(config.conditions.nested_ternaries && config.conditions.free_default);
        let on = weigh_under(lang, &on, text);
        assert!(on.clean, "syntax errors in {text}");
        (weigh(lang, text).total, on.total)
    }

    /// A snippet that parses cleanly, weighed under the test weights with `macro` 6 (a call stays 2).
    pub fn weigh_macros(lang: &'static Lang, text: &str) -> Weighed {
        weigh_with(
            lang,
            "macro     = 2 ",
            "macro     = 6 ",
            Category::Macro,
            6.0,
            text,
        )
    }

    /// A snippet that parses cleanly, weighed under the test weights with `constructor` 8 (a call stays 2).
    pub fn weigh_constructors(lang: &'static Lang, text: &str) -> Weighed {
        weigh_with(
            lang,
            "constructor = 2 ",
            "constructor = 8 ",
            Category::Constructor,
            8.0,
            text,
        )
    }

    /// A snippet that parses cleanly, weighed under the test weights with `case` 7 (a branch stays 10).
    pub fn weigh_cases(lang: &'static Lang, text: &str) -> Weighed {
        weigh_with(
            lang,
            "case      = 10 ",
            "case      = 7 ",
            Category::Case,
            7.0,
            text,
        )
    }

    fn weigh_with(
        lang: &'static Lang,
        line: &str,
        with: &str,
        category: Category,
        weight: f64,
        text: &str,
    ) -> Weighed {
        let weights = WEIGHTS.replacen(line, with, 1);
        let config = Config::from_str_alone(&weights).unwrap();
        assert_eq!(config.lang(lang).weight(category), weight);
        let weighed = weigh_under(lang, &weights, text);
        assert!(weighed.clean, "syntax errors in {text}");
        weighed
    }

    pub fn weight(weighed: &Weighed, kind: ScopeKind, name: &str) -> f64 {
        let found: Vec<_> = weighed
            .entries
            .iter()
            .filter(|e| e.kind == kind && e.name == name)
            .collect();
        assert_eq!(found.len(), 1, "{kind:?} {name} in {:?}", weighed.entries);
        found[0].weight
    }

    pub fn function(weighed: &Weighed, name: &str) -> f64 {
        weight(weighed, ScopeKind::Function, name)
    }

    pub fn class(weighed: &Weighed, name: &str) -> f64 {
        weight(weighed, ScopeKind::Class, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind a language's tables name is one its grammar has, so a typo can't silently weigh nothing.
    #[test]
    fn the_tables_name_real_node_kinds() {
        for lang in ALL {
            let grammar = (lang.grammar)();
            let field = |name: &str| grammar.field_id_for_name(name).is_some();
            let kinds = lang
                .kinds
                .iter()
                .map(|(kind, _)| *kind)
                .chain(lang.nesting.iter().map(|n| n.kind))
                .chain(lang.returns.iter().copied())
                .chain(lang.wrappers.iter().copied())
                .chain(lang.lists.iter().copied())
                .chain(lang.lambdas.iter().copied())
                .chain(lang.chains.iter().copied())
                .chain(lang.casts.iter().copied())
                .chain(lang.patterns.iter().copied())
                .chain(lang.numbers.iter().copied());
            for kind in kinds {
                assert!(is_named_kind(&grammar, kind), "{}: {kind}", lang.name);
            }
            for nesting in lang.nesting {
                let chain = nesting.chain.as_ref().map_or(&[][..], Children::names);
                for name in nesting.body.names().iter().chain(chain) {
                    assert!(
                        is_named_kind(&grammar, name) || field(name),
                        "{}: {name}",
                        lang.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_file_s_language_is_its_extension_s() {
        assert_eq!(
            for_path(Path::new("a/B.java")).map(|l| l.name),
            Some("java")
        );
        assert_eq!(
            for_path(Path::new("a/b.ts")).map(|l| l.name),
            Some("typescript")
        );
        assert_eq!(for_path(Path::new("a/b.tsx")).map(|l| l.name), Some("tsx"));
        for js in ["a/b.js", "a/b.jsx", "a/b.mjs", "a/b.cjs"] {
            assert_eq!(for_path(Path::new(js)).map(|l| l.name), Some("javascript"));
        }
        assert_eq!(
            for_path(Path::new("a/b.py")).map(|l| l.name),
            Some("python")
        );
        assert_eq!(for_path(Path::new("a/b.rs")).map(|l| l.name), Some("rust"));
        assert_eq!(for_path(Path::new("a/b.go")).map(|l| l.name), Some("go"));
        assert_eq!(for_path(Path::new("a/b.c")).map(|l| l.name), Some("c"));
        assert_eq!(for_path(Path::new("a/b.h")).map(|l| l.name), Some("c"));
        assert!(for_path(Path::new("a/b.txt")).is_none());
        assert!(for_path(Path::new("Makefile")).is_none());
    }
}
