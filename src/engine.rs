//! The weight of a text in a language: its tree walked once, each node's weight summed by function, class and file.
//! Pure: it knows nothing of files, folders or git.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

use crate::config::{self, Category, Config, LangConfig};
use crate::lang::{self, Lang, Nesting, Operator, ScopeKind};

/// A text's weight.
#[derive(Clone, Debug, PartialEq)]
pub struct Weighed {
    /// The whole text's.
    pub total: f64,
    /// Of the code outside any class or function, its test code aside.
    pub file: f64,
    /// Of the test code outside any class or function (Rust's `#[cfg(test)] use ...;`).
    pub file_tests: f64,
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
    /// Whether it is test code (a test and a non-test one of one name are two entries).
    pub test: bool,
}

impl Weighed {
    /// Its code outside any class or function, test code or not.
    pub fn outside(&self) -> f64 {
        self.file + self.file_tests
    }

    /// All of it test code: a test file's.
    pub fn all_tests(mut self) -> Weighed {
        self.file_tests += self.file;
        self.file = 0.0;
        for entry in &mut self.entries {
            entry.test = true;
        }
        self
    }

    /// Its test code alone (`tests`), or its code without its tests.
    pub fn only(self, tests: bool) -> Weighed {
        let (code, test) = if tests { (0.0, 1.0) } else { (1.0, 0.0) };
        let mut weighed = self.times(code, test);
        weighed.entries.retain(|e| e.test == tests);
        weighed
    }

    /// The weight of its code without its tests, and of its tests alone: `only(false)`'s and `only(true)`'s
    /// totals, with no copy of it.
    pub fn split(&self) -> (f64, f64) {
        let entries = |tests: bool| {
            self.entries
                .iter()
                .filter(|e| e.test == tests)
                .map(|e| e.weight)
                .sum::<f64>()
        };
        (self.file + entries(false), self.file_tests + entries(true))
    }

    fn times(mut self, code: f64, tests: f64) -> Weighed {
        self.file *= code;
        self.file_tests *= tests;
        for entry in &mut self.entries {
            entry.weight *= if entry.test { tests } else { code };
        }
        self.total = self.outside() + self.entries.iter().map(|e| e.weight).sum::<f64>();
        self
    }
}

/// Weighs texts of one language under one weights file.
pub struct Weigher<'c> {
    lang: &'static Lang,
    weights: &'c LangConfig,
    nesting: &'c config::Nesting,
    parameters: &'c config::Parameters,
    accesses: &'c config::Accesses,
    conditions: &'c config::Conditions,
    grammar: tree_sitter::Language,
    /// By node kind id: its category, the language's table with the weights file's `node_kinds` over it.
    categories: Vec<Option<Category>>,
    constructs: Vec<Option<&'static Nesting>>,
    returns: Vec<bool>,
    wrappers: Vec<bool>,
    lists: Vec<bool>,
    lambdas: Vec<bool>,
    /// By node kind id: an operator chaining its operands, an operator per operand after the first.
    chains: Vec<bool>,
    /// By node kind id: a number literal, which may be a magic one (`[conditions] magic_numbers`).
    numbers: Vec<bool>,
}

impl<'c> Weigher<'c> {
    pub fn new(lang: &'static Lang, config: &'c Config) -> Weigher<'c> {
        let grammar = (lang.grammar)();
        let count = grammar.node_kind_count();
        let weights = config.lang(lang);

        let mut categories = vec![None; count];
        let conditions = &config.conditions;
        // The kinds a condition weighs, as an operator: the table's entries first, the user's over all.
        let conditional = [
            (conditions.casts, lang.casts),
            (conditions.let_patterns, lang.patterns),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .flat_map(|(_, kinds)| kinds.iter().map(|k| (*k, Some(Category::Operator))));
        let overrides = weights.node_kinds.iter().map(|(k, c)| (k.as_str(), *c));
        for (kind, category) in lang
            .kinds
            .iter()
            .map(|(k, c)| (*k, Some(*c)))
            .chain(conditional)
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
        let wrappers = flags(lang.wrappers);
        let lists = flags(lang.lists);
        let lambdas = flags(lang.lambdas);
        let chains = flags(lang.chains);
        let numbers = flags(lang.numbers);

        Weigher {
            lang,
            weights,
            nesting: &config.nesting,
            parameters: &config.parameters,
            accesses: &config.accesses,
            conditions,
            grammar,
            categories,
            constructs,
            returns,
            wrappers,
            lists,
            lambdas,
            chains,
            numbers,
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
        let mut file_tests = 0.0;
        let mut entries: Vec<Entry> = Vec::new();
        let mut index: HashMap<(ScopeKind, String, bool), usize> = HashMap::new();
        let mut stack = vec![Frame {
            node: root,
            depth: 0,
            entry: None,
            in_function: false,
            test: false,
        }];
        let mut children = Vec::new();

        while let Some(frame) = stack.pop() {
            let node = frame.node;
            let id = node.kind_id() as usize;

            let mut entry = frame.entry;
            let mut in_function = frame.in_function;
            let mut nested_scope = false;
            let test = frame.test || self.lang.tests.is_some_and(|tests| tests(node, source));
            let scope = (self.lang.scope)(node, source);
            let declares = scope.as_ref().map(|s| s.kind);
            if let Some(scope) = scope {
                let name = match frame.entry {
                    Some(parent) => format!("{}.{}", entries[parent].name, scope.name),
                    None => scope.name,
                };
                let at = *index.entry((scope.kind, name, test)).or_insert_with_key(
                    |(kind, name, test)| {
                        entries.push(Entry {
                            kind: *kind,
                            name: name.clone(),
                            line: node.start_position().row + 1,
                            weight: 0.0,
                            test: *test,
                        });
                        entries.len() - 1
                    },
                );
                entry = Some(at);
                nested_scope = frame.in_function;
                in_function |= scope.kind == ScopeKind::Function;
            }

            // Each operator counts, one in a chain of them too (`a + b + c` is 2, `a = b = c` 2). A plain `=`
            // weighs beside the node's own category (Go's assignment is a statement), never in its place.
            let operator = (self.lang.operator)(node);
            let assigns = self.conditions.assignments && operator == Some(Operator::Assign);
            let category = match operator.filter(|op| *op != Operator::Assign) {
                Some(Operator::Nullish) if self.conditions.nullish_operator => {
                    Some(Category::Operator)
                }
                Some(Operator::Logical | Operator::Nullish) => Some(Category::BoolOp),
                Some(_) => Some(Category::Operator),
                None if self.conditions.magic_numbers
                    && flag(&self.numbers, id)
                    && is_magic(&lang::text(node, source)) =>
                {
                    Some(Category::Access)
                }
                None if (self.lang.exempt)(node, source) => None,
                None => self
                    .categories
                    .get(id)
                    .copied()
                    .flatten()
                    .and_then(|category| {
                        if flag(&self.returns, id) {
                            if self.is_early_return(node, source) {
                                Some(Category::Jump)
                            } else if (self.lang.carried_return)(node) {
                                None
                            } else {
                                Some(category)
                            }
                        } else if category == Category::Lambda
                            && declares == Some(ScopeKind::Function)
                        {
                            Some(Category::Function)
                        } else if category == Category::Case
                            && self.conditions.free_default
                            && (self.lang.default_case)(node, source)
                        {
                            None
                        } else {
                            Some(category)
                        }
                    }),
            };
            // An access weighs more the further it is in a row of them (`[accesses]`): `a.b.c` is `.b` at 0, `.c` at 1.
            let row = match category {
                Some(Category::Access) => self.accesses.factor_at(self.access_place(node, source)),
                _ => 1.0,
            };
            let times = match operator {
                Some(_) if flag(&self.chains, id) => {
                    lang::count_children(node, |_| true).saturating_sub(1)
                }
                _ => 1,
            };
            let weight = category
                .map(|c| (c, times as f64 * row))
                .into_iter()
                .chain((self.lang.also_statement)(node).then_some((Category::Statement, 1.0)))
                .chain(assigns.then_some((Category::Operator, 1.0)))
                .map(|(c, n)| n * self.weights.weight(c) * self.nesting.factor_of(c, frame.depth))
                .sum::<f64>()
                + self.parameters((self.lang.parameters)(node, source), frame.depth)
                + match category {
                    Some(Category::Call | Category::Constructor) => {
                        self.arguments(lang::arguments(node), frame.depth)
                    }
                    _ => 0.0,
                };
            if weight > 0.0 {
                match entry {
                    Some(at) => entries[at].weight += weight,
                    None if test => file_tests += weight,
                    None => file += weight,
                }
            }

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
                                && !(construct.ternary && self.conditions.nested_ternaries)
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
                    test,
                });
            }
        }

        let total = file + file_tests + entries.iter().map(|e| e.weight).sum::<f64>();
        Ok(Weighed {
            total,
            file,
            file_tests,
            entries,
            clean: !root.has_error(),
        })
    }

    /// An access's place in its row: how many accesses its object is in a row of (`a.b.c`'s `.c` is at 1, its `.b`
    /// at 0). A call, `this` or anything not weighed as an access ends the row: `a.f().b` is at 0.
    fn access_place(&self, node: Node, source: &[u8]) -> usize {
        let mut place = 0;
        let mut node = node;
        while let Some(object) = lang::first_child(node).filter(|o| self.is_access(*o, source)) {
            place += 1;
            node = object;
        }
        place
    }

    /// Whether a node weighs as an access by its kind: not an operator, not exempt (a call's callee, `this.x`).
    fn is_access(&self, node: Node, source: &[u8]) -> bool {
        self.categories
            .get(node.kind_id() as usize)
            .copied()
            .flatten()
            == Some(Category::Access)
            && (self.lang.operator)(node).is_none()
            && !(self.lang.exempt)(node, source)
    }

    /// The weight of `count` parameters at `depth`, each by its place.
    fn parameters(&self, count: usize, depth: u32) -> f64 {
        let base = self.weights.weight(Category::Parameter)
            * self.nesting.factor_of(Category::Parameter, depth);
        (0..count)
            .map(|place| base * self.parameters.factor_at(place))
            .sum()
    }

    /// The weight of a call's `count` arguments at `depth`.
    fn arguments(&self, count: usize, depth: u32) -> f64 {
        self.weights.weight(Category::Argument)
            * self.nesting.factor_of(Category::Argument, depth)
            * count as f64
    }

    /// Whether a return is early: not the last statement of its function's or lambda's body (the statement it is
    /// wrapped in, if it is; the last of the body's list of statements, if it has one).
    fn is_early_return(&self, node: Node, source: &[u8]) -> bool {
        let Some(mut body) = node.parent() else {
            return true;
        };
        let mut node = node;
        if flag(&self.wrappers, body.kind_id() as usize) {
            node = body;
            let Some(parent) = body.parent() else {
                return true;
            };
            body = parent;
        }
        // The statements' list the return is the last of, and the body it is in.
        let list = body;
        if flag(&self.lists, body.kind_id() as usize) {
            let Some(parent) = body.parent() else {
                return true;
            };
            body = parent;
        }
        let Some(owner) = body.parent() else {
            return true;
        };
        let owns = flag(&self.lambdas, owner.kind_id() as usize)
            || (self.lang.scope)(owner, source).is_some_and(|s| s.kind == ScopeKind::Function);
        if !owns || owner.child_by_field_name("body").map(|b| b.id()) != Some(body.id()) {
            return true;
        }
        lang::last_child(list).map(|last| last.id()) != Some(node.id())
    }
}

/// Whether a number literal is a magic one: its value neither 0 nor 1 (`_` separators, a radix prefix and a type
/// suffix aside: `1_000`, `0x1`, `1u8`, `1.0f`, `1L`, `1j`).
fn is_magic(literal: &str) -> bool {
    let text: String = literal
        .chars()
        .filter(|c| *c != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    let radix = match text.get(..2) {
        Some("0x") => 16,
        Some("0o") => 8,
        Some("0b") => 2,
        _ => 10,
    };
    let value = if radix == 10 {
        // The mantissa and its exponent; what follows is a type suffix.
        let end = text
            .char_indices()
            .find(|&(i, c)| {
                !(c.is_ascii_digit()
                    || c == '.'
                    || (c == 'e'
                        && text[i + 1..]
                            .starts_with(|n: char| n.is_ascii_digit() || n == '-' || n == '+'))
                    || ((c == '-' || c == '+') && text[..i].ends_with('e')))
            })
            .map_or(text.len(), |(i, _)| i);
        text[..end].parse::<f64>().ok()
    } else {
        let digits = &text[2..];
        let end = digits
            .find(|c: char| !c.is_digit(radix))
            .unwrap_or(digits.len());
        u64::from_str_radix(&digits[..end], radix)
            .ok()
            .map(|v| v as f64)
    };
    value.is_none_or(|v| v != 0.0 && v != 1.0)
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
    /// Whether it is test code, or inside some.
    test: bool,
}

#[cfg(test)]
mod tests {
    use super::is_magic;

    #[test]
    fn a_magic_number_is_one_but_0_and_1() {
        for literal in [
            "0", "1", "0.0", "1.", "1.0", "1_0e-1", "0x1", "0b0", "0o1", "1u8", "1.0f64", "1L",
            "1d", "1j", "01",
        ] {
            assert!(!is_magic(literal), "{literal}");
        }
        for literal in [
            "2", "3.14", "42u32", "1e3", "0x1f", "1_000", "0.5", ".5", "100L", "2j",
        ] {
            assert!(is_magic(literal), "{literal}");
        }
    }
}
