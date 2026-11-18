//! The weight of a text in a language: its tree walked once, each node's weight summed by function, class and file.
//! Pure: it knows nothing of files, folders or git.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

use crate::config::{self, Category, Config, LangConfig};
use crate::lang::{self, Lang, Logical, Nesting, ScopeKind};

/// A text's weight.
#[derive(Clone, Debug, PartialEq)]
pub struct Weighed {
    /// The whole text's.
    pub total: f64,
    /// Of the code outside any class or function.
    pub file: f64,
    /// Each class's (its code outside its functions) and each function's (its nested functions' not in it), in the
    /// order they first appear; same-named ones of one kind are one entry.
    pub entries: Vec<Entry>,
    /// Whether the text parsed without errors (if not, tree-sitter's recovered tree is what was weighed).
    pub clean: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: ScopeKind,
    /// Qualified by its enclosing classes and functions: `Outer.Inner.method`.
    pub name: String,
    /// The line it starts on (its first declaration's, for same-named ones), from 1.
    pub line: usize,
    pub weight: f64,
}

impl Weighed {
    /// Every weight times `factor` (a test file's, times the test factor).
    pub fn scaled(mut self, factor: f64) -> Weighed {
        self.total *= factor;
        self.file *= factor;
        for entry in &mut self.entries {
            entry.weight *= factor;
        }
        self
    }
}

/// Weighs texts of one language under one weights file.
pub struct Weigher<'c> {
    lang: &'static Lang,
    weights: &'c LangConfig,
    nesting: &'c config::Nesting,
    grammar: tree_sitter::Language,
    /// By node kind id: its category, the language's table with the weights file's `node_kinds` over it.
    categories: Vec<Option<Category>>,
    constructs: Vec<Option<&'static Nesting>>,
    returns: Vec<bool>,
    lambdas: Vec<bool>,
    parens: Vec<bool>,
}

impl<'c> Weigher<'c> {
    pub fn new(lang: &'static Lang, config: &'c Config) -> Weigher<'c> {
        let grammar = (lang.grammar)();
        let count = grammar.node_kind_count();
        let weights = config.lang(lang);

        let mut categories = vec![None; count];
        let overrides = weights.node_kinds.iter().map(|(k, c)| (k.as_str(), *c));
        for (kind, category) in lang
            .kinds
            .iter()
            .map(|(k, c)| (*k, Some(*c)))
            .chain(overrides)
        {
            for id in lang::named_kind_ids(&grammar, kind) {
                categories[id as usize] = category;
            }
        }
        let mut constructs = vec![None; count];
        for nesting in lang.nesting {
            for id in lang::named_kind_ids(&grammar, nesting.kind) {
                constructs[id as usize] = Some(nesting);
            }
        }
        let flags = |kinds: &[&str]| {
            let mut flags = vec![false; count];
            for kind in kinds {
                for id in lang::named_kind_ids(&grammar, kind) {
                    flags[id as usize] = true;
                }
            }
            flags
        };
        let returns = flags(lang.returns);
        let lambdas = flags(lang.lambdas);
        let parens = flags(lang.parens);

        Weigher {
            lang,
            weights,
            nesting: &config.nesting,
            grammar,
            categories,
            constructs,
            returns,
            lambdas,
            parens,
        }
    }

    pub fn lang(&self) -> &'static Lang {
        self.lang
    }

    pub fn weigh(&self, text: &str) -> Result<Weighed> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.grammar)
            .with_context(|| format!("loading the {} grammar", self.lang.name))?;
        let tree = parser
            .parse(text, None)
            .with_context(|| format!("parsing as {}", self.lang.name))?;
        let root = tree.root_node();
        let source = text.as_bytes();

        let mut file = 0.0;
        let mut entries: Vec<Entry> = Vec::new();
        let mut index: HashMap<(ScopeKind, String), usize> = HashMap::new();
        let mut stack = vec![Frame {
            node: root,
            depth: 0,
            entry: None,
            in_function: false,
            logical: None,
        }];
        let mut children = Vec::new();

        while let Some(frame) = stack.pop() {
            let node = frame.node;
            let id = node.kind_id() as usize;

            let mut entry = frame.entry;
            let mut in_function = frame.in_function;
            let mut nested_scope = false;
            let scope = (self.lang.scope)(node, source);
            let declares = scope.as_ref().map(|s| s.kind);
            if let Some(scope) = scope {
                let name = match frame.entry {
                    Some(parent) => format!("{}.{}", entries[parent].name, scope.name),
                    None => scope.name,
                };
                let at = *index
                    .entry((scope.kind, name))
                    .or_insert_with_key(|(kind, name)| {
                        entries.push(Entry {
                            kind: *kind,
                            name: name.clone(),
                            line: node.start_position().row + 1,
                            weight: 0.0,
                        });
                        entries.len() - 1
                    });
                entry = Some(at);
                nested_scope = frame.in_function;
                in_function |= scope.kind == ScopeKind::Function;
            }

            let logical = (self.lang.logical)(node);
            let category = match logical {
                Some(op) => (frame.logical != Some(op)).then_some(Category::BoolOp),
                None if (self.lang.exempt)(node) => None,
                None => self.categories.get(id).copied().flatten().map(|category| {
                    if flag(&self.returns, id) && self.is_early_return(node, source) {
                        Category::Jump
                    } else if category == Category::Lambda && declares == Some(ScopeKind::Function)
                    {
                        Category::Function
                    } else {
                        category
                    }
                }),
            };
            if let Some(category) = category {
                let weight = self.weights.weight(category) * self.nesting.factor_at(frame.depth);
                match entry {
                    Some(at) => entries[at].weight += weight,
                    None => file += weight,
                }
            }

            let child_logical = match logical {
                Some(_) => logical,
                None if flag(&self.parens, id) => frame.logical,
                None => None,
            };
            // A node that declares a function or class (a named lambda) nests as one, not as its construct.
            let construct = match declares {
                Some(_) => None,
                None => self.constructs.get(id).copied().flatten(),
            };
            let scope_body = nested_scope
                .then(|| node.child_by_field_name("body"))
                .flatten();

            let mut cursor = node.walk();
            let mut nth = 0;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        children.push((child, nth, cursor.field_name()));
                        // A comment is no place: the places are of the code's own children.
                        nth += usize::from(!child.is_extra());
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            for (child, nth, field) in children.drain(..).rev() {
                let deeper = match construct {
                    Some(construct) => {
                        construct.body.hold(nth, field, child.kind())
                            && !(child.kind_id() == node.kind_id()
                                && construct
                                    .chain
                                    .as_ref()
                                    .is_some_and(|chain| chain.hold(nth, field, child.kind())))
                    }
                    None if nested_scope => scope_body.is_none_or(|body| body.id() == child.id()),
                    None => false,
                };
                stack.push(Frame {
                    node: child,
                    depth: frame.depth + u32::from(deeper),
                    entry,
                    in_function,
                    logical: child_logical,
                });
            }
        }

        let total = file + entries.iter().map(|e| e.weight).sum::<f64>();
        Ok(Weighed {
            total,
            file,
            entries,
            clean: !root.has_error(),
        })
    }

    /// Whether a return is early: not the last statement of its function's or lambda's body.
    fn is_early_return(&self, node: Node, source: &[u8]) -> bool {
        let Some(body) = node.parent() else {
            return true;
        };
        let Some(owner) = body.parent() else {
            return true;
        };
        let owns = flag(&self.lambdas, owner.kind_id() as usize)
            || (self.lang.scope)(owner, source).is_some_and(|s| s.kind == ScopeKind::Function);
        if !owns || owner.child_by_field_name("body").map(|b| b.id()) != Some(body.id()) {
            return true;
        }
        let mut cursor = body.walk();
        let last = body
            .named_children(&mut cursor)
            .filter(|child| !child.is_extra())
            .last();
        last.map(|last| last.id()) != Some(node.id())
    }
}

/// A kind's flag; `false` for a kind past the grammar's own (tree-sitter's error node).
fn flag(flags: &[bool], id: usize) -> bool {
    flags.get(id).copied().unwrap_or(false)
}

/// A node to visit, with what it inherits from around it.
struct Frame<'t> {
    node: Node<'t>,
    /// The number of nesting constructs around it.
    depth: u32,
    /// The class or function it belongs to; `None` for the file.
    entry: Option<usize>,
    /// Whether it is inside a function, so a function or class declared here nests.
    in_function: bool,
    /// The operator of the logical expression it is an operand of (through parentheses), if any.
    logical: Option<Logical>,
}
