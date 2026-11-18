//! The languages: each one's grammar, file extensions, node kind → category table, nesting constructs and how its
//! functions and classes are named. The engine reads these tables; it knows no language itself.

pub mod java;

use std::path::Path;

use tree_sitter::Node;

use crate::config::Category;

/// Every language codeweight reads.
pub const ALL: &[&Lang] = &[&java::JAVA];

/// A language's tables.
pub struct Lang {
    /// Its key in the weights file (`[languages.<name>]`).
    pub name: &'static str,
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
    /// The `&&` / `||` of a logical expression, if the node is one.
    pub logical: fn(Node) -> Option<Logical>,
    /// The function or class a node declares, if it does one, and its own name (not qualified).
    pub scope: fn(Node, &[u8]) -> Option<Scope>,
}

/// A nesting construct: which of its children are one level deeper than it.
pub struct Nesting {
    pub kind: &'static str,
    pub body: Body,
    /// A field whose child of the construct's own kind stays at its depth (`else if`, a chained ternary).
    pub chain: Option<&'static str>,
}

/// The children of a nesting construct that are its body, each matched by its field name or its node kind.
pub enum Body {
    Only(&'static [&'static str]),
    AllBut(&'static [&'static str]),
}

impl Body {
    /// Whether the child in `field` of kind `kind` is in the body.
    pub fn holds(&self, field: Option<&str>, kind: &str) -> bool {
        let named = |names: &[&str]| names.iter().any(|n| *n == kind || Some(*n) == field);
        match self {
            Body::Only(names) => named(names),
            Body::AllBut(names) => !named(names),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    And,
    Or,
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

pub fn by_name(name: &str) -> Option<&'static Lang> {
    ALL.iter().copied().find(|l| l.name == name)
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

/// The text of a node, lossy where it isn't UTF-8.
pub fn text(node: Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.byte_range()]).into_owned()
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
                let (Body::Only(names) | Body::AllBut(names)) = &nesting.body;
                for name in names.iter().chain(&nesting.chain) {
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
        assert!(for_path(Path::new("a/b.txt")).is_none());
        assert!(for_path(Path::new("Makefile")).is_none());
    }
}
