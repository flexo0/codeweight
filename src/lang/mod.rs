//! The languages: each one's grammar, file extensions, node kind → category table, nesting constructs and how its
//! functions and classes are named. The engine reads these tables; it knows no language itself.

pub mod java;
pub mod python;
pub mod typescript;

use std::path::Path;

use tree_sitter::Node;

use crate::config::Category;

/// Every language codeweight reads.
pub const ALL: &[&Lang] = &[
    &java::JAVA,
    &typescript::TYPESCRIPT,
    &typescript::TSX,
    &python::PYTHON,
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
    /// The kinds of a lambda, whose last statement isn't an early return either.
    pub lambdas: &'static [&'static str],
    /// The kinds of a parenthesized expression, which a run of `&&` / `||` goes through.
    pub parens: &'static [&'static str],
    /// The `&&` / `||` / `??` of a logical expression, if the node is one.
    pub logical: fn(Node) -> Option<Logical>,
    /// Whether a node weighs nothing although its kind has a category: the cases its kind can't tell (a docstring,
    /// a `match` case's guard).
    pub exempt: fn(Node) -> bool,
    /// The function or class a node declares, if it does one, and its own name (not qualified). A lambda it names
    /// a function is one: it weighs as a function, and nests as one, not as a lambda.
    pub scope: fn(Node, &[u8]) -> Option<Scope>,
}

/// A nesting construct: which of its children are one level deeper than it.
pub struct Nesting {
    pub kind: &'static str,
    /// Its children one level deeper.
    pub body: Children,
    /// Its children that stay at its depth when they are of its own kind (`else if`, a chained ternary).
    pub chain: Option<Children>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    And,
    Or,
    /// TypeScript's `??`.
    Nullish,
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

/// A node's one named child, comments aside, if it has just one.
pub fn only_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor).filter(|c| !c.is_extra());
    let first = children.next()?;
    children.next().is_none().then_some(first)
}

/// The text of a node, lossy where it isn't UTF-8.
pub fn text(node: Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).into_owned()
}

/// The languages' snippet tests: weighed under `tests/weights.toml` (statement 1, call 2, branch 10, loop 20, try 30,
/// catch 40, lambda 50, function 100, class 1000, bool_op 5, jump 3; nest(depth) = 1 + depth, depth capped at 3),
/// each sum worked out by hand: `weight × nest(depth)` per element.
#[cfg(test)]
pub mod testing {
    use super::{Lang, ScopeKind};
    use crate::config::Config;
    use crate::engine::{Weighed, Weigher};

    pub const WEIGHTS: &str = include_str!("../../tests/weights.toml");

    pub fn weigh_under(lang: &'static Lang, weights: &str, text: &str) -> Weighed {
        let config = Config::from_str_alone(weights).unwrap();
        let weighed = Weigher::new(lang, &config).weigh(text).unwrap();
        let sum = weighed.file + weighed.entries.iter().map(|e| e.weight).sum::<f64>();
        assert_eq!(weighed.total, sum);
        weighed
    }

    /// A snippet that parses cleanly, weighed under the test weights.
    pub fn weigh(lang: &'static Lang, text: &str) -> Weighed {
        let weighed = weigh_under(lang, WEIGHTS, text);
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
                .chain(lang.lambdas.iter().copied())
                .chain(lang.parens.iter().copied());
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
        assert_eq!(
            for_path(Path::new("a/b.py")).map(|l| l.name),
            Some("python")
        );
        assert!(for_path(Path::new("a/b.txt")).is_none());
        assert!(for_path(Path::new("Makefile")).is_none());
    }
}
