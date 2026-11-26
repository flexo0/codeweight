//! The weight of a text in a language: its tree walked once, each node's weight summed by function, class and file.
//! Pure: it knows nothing of files, folders or git.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

use crate::config::{self, Category, Config, LangConfig};
use crate::lang::{self, Lang, Nesting, Operator, Scope, ScopeKind};
use crate::markdown::Counter;

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
    /// A weight of nothing: no code, no entries.
    pub fn none(clean: bool) -> Weighed {
        Weighed {
            total: 0.0,
            file: 0.0,
            file_tests: 0.0,
            entries: Vec::new(),
            clean,
        }
    }

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
    /// By node kind id: a comment, and an attribute or decorator that may stand between one and its declaration.
    comments: Vec<bool>,
    attributes: Vec<bool>,
    /// Weighs a comment's text, by the `[docs]` weights.
    counter: Counter,
    /// The weigher of the language's fallback, which parses a text this one's grammar leaves errors in.
    fallback: Option<Box<Weigher<'c>>>,
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
        let comments = flags(lang.comments);
        let attributes = flags(lang.attributes);

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
            comments,
            attributes,
            counter: Counter::new(&config.docs),
            fallback: lang.fallback.map(|l| Box::new(Weigher::new(l, config))),
        }
    }

    pub fn lang(&self) -> &'static Lang {
        self.lang
    }

    /// A text's code weight.
    pub fn weigh(&self, text: &str) -> Result<Weighed> {
        Ok(self.walk(text, false)?.0)
    }

    /// A text's code weight and its comments' weight, the comments' entries the code's functions and classes but
    /// its test code's.
    pub fn weigh_with_comments(&self, text: &str) -> Result<(Weighed, Weighed)> {
        self.walk(text, true)
    }

    /// The code weight, and with `comments` the comments' (else an empty one): each comment and docstring, its
    /// markers stripped, weighed flat by the `[docs]` weights into its function's, class's or file's entry, or
    /// into the declaration's it is right before; a file's header, a shebang and test code's comments weigh
    /// nothing.
    /// A text the grammar leaves errors in is parsed again by the language's fallback, and the tree with less of
    /// the text in errors is walked, the first on a tie.
    fn walk(&self, text: &str, comments: bool) -> Result<(Weighed, Weighed)> {
        let tree = self.parse(text)?;
        if let Some(fallback) = &self.fallback
            && tree.root_node().has_error()
        {
            let other = fallback.parse(text)?;
            if error_bytes(other.root_node()) < error_bytes(tree.root_node()) {
                return fallback.walk_tree(&other, text, comments);
            }
        }
        self.walk_tree(&tree, text, comments)
    }

    fn parse(&self, text: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.grammar)
            .with_context(|| format!("loading the {} grammar", self.lang.name))?;
        parser
            .parse(text, None)
            .with_context(|| format!("parsing as {}", self.lang.name))
    }

    fn walk_tree(&self, tree: &Tree, text: &str, comments: bool) -> Result<(Weighed, Weighed)> {
        let root = tree.root_node();
        let source = text.as_bytes();

        let mut file = 0.0;
        let mut file_tests = 0.0;
        let mut entries = Entries::default();
        let mut comments = comments.then(|| Comments::new(self.header_end(root, source)));
        let mut stack = vec![Frame {
            node: root,
            depth: 0,
            entry: None,
            in_function: false,
            test: false,
        }];
        // A loop's body its kind doesn't nest (C's loop macro's block), by its node id: its depth.
        let mut bodies: HashMap<usize, u32> = HashMap::new();

        while let Some(frame) = stack.pop() {
            let node = frame.node;
            let depth = if bodies.is_empty() {
                frame.depth
            } else {
                bodies.remove(&node.id()).unwrap_or(frame.depth)
            };
            let loop_body = (self.lang.loop_body)(node);
            if let Some(body) = loop_body {
                bodies.insert(body.id(), depth + 1);
            }

            let mut entry = frame.entry;
            let mut in_function = frame.in_function;
            let mut nested_scope = false;
            let test = frame.test || self.lang.tests.is_some_and(|tests| tests(node, source));
            let scope = (self.lang.scope)(node, source);
            let declares = scope.as_ref().map(|s| s.kind);
            if let Some(scope) = scope {
                let line = node.start_position().row + 1;
                in_function |= scope.kind == ScopeKind::Function;
                entry = Some(entries.enter(frame.entry, scope, test, line));
                nested_scope = frame.in_function;
            }

            let operator = (self.lang.operator)(node);
            let category = self.category(node, source, operator, loop_body.is_some(), declares);
            let weight = self.weight(node, source, category, operator, depth);
            if weight > 0.0 {
                match entry {
                    Some(at) => entries.list[at].weight += weight,
                    None if test => file_tests += weight,
                    None => file += weight,
                }
            }
            if let Some(comments) = &mut comments
                && !test
            {
                comments.add(self, node, source, entry, &entries);
            }

            let frame = Frame {
                node,
                depth,
                entry,
                in_function,
                test,
            };
            self.push_children(frame, declares.is_some(), nested_scope, &mut stack);
        }

        let clean = !root.has_error();
        let comments = match comments {
            Some(comments) => comments.finish(&entries, clean),
            None => Weighed::none(clean),
        };
        let entries = entries.list;
        let total = file + file_tests + entries.iter().map(|e| e.weight).sum::<f64>();
        let code = Weighed {
            total,
            file,
            file_tests,
            entries,
            clean,
        };
        Ok((code, comments))
    }

    /// A node's category, if it weighs as one: an operator's by its kind (a plain `=` none, it weighs beside the
    /// node's own: Go's assignment is a statement), else its kind's in the table, an early return a jump, a named
    /// lambda a function and a free `default:` none.
    fn category(
        &self,
        node: Node,
        source: &[u8],
        operator: Option<Operator>,
        loop_body: bool,
        declares: Option<ScopeKind>,
    ) -> Option<Category> {
        let id = node.kind_id() as usize;
        match operator.filter(|op| *op != Operator::Assign) {
            Some(Operator::Nullish) if self.conditions.nullish_operator => Some(Category::Operator),
            Some(Operator::Logical | Operator::Nullish) => Some(Category::BoolOp),
            Some(_) => Some(Category::Operator),
            None if loop_body => Some(Category::Loop),
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
                    } else if category == Category::Lambda && declares == Some(ScopeKind::Function)
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
        }
    }

    /// A node's own weight at `depth`, its category's (each operator of a chain, an access by its place in its
    /// row), a statement's it also is and a plain `=`'s beside it, its parameters' and a call's arguments'.
    fn weight(
        &self,
        node: Node,
        source: &[u8],
        category: Option<Category>,
        operator: Option<Operator>,
        depth: u32,
    ) -> f64 {
        let assigns = self.conditions.assignments && operator == Some(Operator::Assign);
        // An access weighs more the further it is in a row of them (`[accesses]`): `a.b.c` is `.b` at 0, `.c` at 1.
        let row = match category {
            Some(Category::Access) => self.accesses.factor_at(self.access_place(node, source)),
            _ => 1.0,
        };
        // Each operator counts, one in a chain of them too (`a + b + c` is 2, `a = b = c` 2).
        let times = match operator {
            Some(_) if flag(&self.chains, node.kind_id() as usize) => {
                lang::count_children(node, |_| true).saturating_sub(1)
            }
            _ => 1,
        };
        category
            .map(|c| (c, times as f64 * row))
            .into_iter()
            .chain((self.lang.also_statement)(node).then_some((Category::Statement, 1.0)))
            .chain(assigns.then_some((Category::Operator, 1.0)))
            .map(|(c, n)| n * self.weights.weight(c) * self.nesting.factor_of(c, depth))
            .sum::<f64>()
            + self.parameters((self.lang.parameters)(node, source), depth)
            + match category {
                Some(Category::Call | Category::Constructor) => {
                    self.arguments(lang::arguments(node), depth)
                }
                _ => 0.0,
            }
    }

    /// Pushes `frame`'s node's named children on `stack`, to be visited next in their order, each inheriting
    /// `frame`, one deeper where the node nests it. A node that `declares` a function or class (a named lambda)
    /// nests as one, not as its construct: its body deeper when it is a `nested_scope` (declared in a function).
    fn push_children<'t>(
        &self,
        frame: Frame<'t>,
        declares: bool,
        nested_scope: bool,
        stack: &mut Vec<Frame<'t>>,
    ) {
        let node = frame.node;
        let construct = if declares {
            None
        } else {
            self.constructs
                .get(node.kind_id() as usize)
                .copied()
                .flatten()
        };
        let holder = match construct {
            Some(construct) => Some(Holder::Construct(construct)),
            None => nested_scope.then(|| Holder::Scope(node.child_by_field_name("body"))),
        };
        let first = stack.len();
        let mut cursor = node.walk();
        let mut nth = 0;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    let deeper = holder.is_some_and(|holder| {
                        self.nests(holder, node, child, nth, cursor.field_name())
                    });
                    stack.push(Frame {
                        node: child,
                        depth: frame.depth + u32::from(deeper),
                        ..frame
                    });
                    // A comment is no place: the places are of the code's own children.
                    nth += usize::from(!child.is_extra());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        stack[first..].reverse();
    }

    /// Whether `holder` nests `node`'s child `child` (the `nth` of its code's children, of `field`) one deeper: a
    /// construct's body does but a link of its chain (`else if`), a nested function's or class's body does (all of
    /// its children with no body).
    fn nests(
        &self,
        holder: Holder,
        node: Node,
        child: Node,
        nth: usize,
        field: Option<&str>,
    ) -> bool {
        match holder {
            Holder::Construct(construct) => {
                construct.body.hold(nth, field, child.kind())
                    && !(child.kind_id() == node.kind_id()
                        && !(construct.ternary && self.conditions.nested_ternaries)
                        && construct
                            .chain
                            .as_ref()
                            .is_some_and(|chain| chain.hold(nth, field, child.kind())))
            }
            Holder::Scope(body) => body.is_none_or(|body| body.id() == child.id()),
        }
    }

    /// The text of a comment or a docstring, its markers stripped; `None` for any other node, for a comment of the
    /// file's header (that ends by `header`) and a shebang.
    fn comment_text(&self, node: Node, source: &[u8], header: usize) -> Option<String> {
        if !flag(&self.comments, node.kind_id() as usize) {
            return (self.lang.docstring)(node, source);
        }
        if node.start_byte() < header && !(self.lang.inner_doc)(node) {
            return None;
        }
        Some(lang::comment_text(&lang::text(node, source)))
    }

    /// Where the file's header ends, by byte: the comments at its top (`root`'s first children) up to the last
    /// blank line before its first code, or all of them in a file of no code; a shebang (`#!` on the first line)
    /// too. 0 for no header.
    fn header_end(&self, root: Node, source: &[u8]) -> usize {
        let mut cursor = root.walk();
        let mut end = 0;
        let mut last: Option<Node> = None;
        for child in root.named_children(&mut cursor) {
            let comment = flag(&self.comments, child.kind_id() as usize);
            if let Some(last) = last
                && blank_between(last, child, source)
            {
                end = last.end_byte();
            }
            if !comment {
                return end;
            }
            if child.start_byte() == 0 && source.starts_with(b"#!") {
                end = child.end_byte();
            }
            last = Some(child);
        }
        last.map_or(end, |last| last.end_byte())
    }

    /// The code a comment is right before, if it is: the next node after it, attributes, decorators and comments
    /// between, no blank line. None for a comment after code on its own line (`x = 1  # why`), which is that
    /// line's.
    fn code_after<'t>(&self, comment: Node<'t>, source: &[u8]) -> Option<Node<'t>> {
        let is = |flags: &[bool], node: Node| flag(flags, node.kind_id() as usize);
        if let Some(before) = comment.prev_sibling()
            && !is(&self.comments, before)
            && before.end_position().row == comment.start_position().row
        {
            return None;
        }
        let mut last = comment;
        let mut next = comment.next_named_sibling();
        while let Some(code) = next {
            if blank_between(last, code, source) {
                return None;
            }
            if !is(&self.comments, code) && !is(&self.attributes, code) {
                return Some(code);
            }
            last = code;
            next = code.next_named_sibling();
        }
        None
    }

    /// The first function or class `code` declares (`export function`, a decorated `def`, `const f = () => ...`),
    /// not inside a nesting construct, with whether it is test code.
    fn declared_by(&self, code: Node, source: &[u8]) -> Option<(Scope, bool)> {
        let tests = |node: Node| self.lang.tests.is_some_and(|tests| tests(node, source));
        let mut stack = vec![code];
        while let Some(node) = stack.pop() {
            if let Some(scope) = (self.lang.scope)(node, source) {
                return Some((scope, tests(code) || tests(node)));
            }
            if self
                .constructs
                .get(node.kind_id() as usize)
                .copied()
                .flatten()
                .is_some()
            {
                continue;
            }
            let mut cursor = node.walk();
            let children: Vec<Node> = node
                .named_children(&mut cursor)
                .filter(|c| !c.is_extra())
                .collect();
            stack.extend(children.into_iter().rev());
        }
        None
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

/// Whether a number literal is a magic one: its value neither 0 nor 1 (a sign, which C's grammar puts in the
/// literal, `_` separators, a radix prefix and a type suffix aside: `-1`, `1_000`, `0x1`, `1u8`, `1.0f`, `1L`,
/// `1j`).
fn is_magic(literal: &str) -> bool {
    let text: String = literal
        .trim_start_matches(['-', '+'])
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

/// The bytes of the text in a tree's error nodes, each counted once (an error inside an error is in it); a
/// loop, not recursion, so a deeply nested tree can't overflow the stack.
fn error_bytes(node: Node) -> usize {
    let mut bytes = 0;
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if node.is_error() {
            bytes += node.byte_range().len();
        } else if node.has_error() {
            let mut cursor = node.walk();
            stack.extend(node.children(&mut cursor));
        }
    }
    bytes
}

/// Whether a blank line stands between two nodes, `a` before `b` (a line comment may end with its newline).
fn blank_between(a: Node, b: Node, source: &[u8]) -> bool {
    let newlines = |bytes: &[u8]| bytes.iter().filter(|&&c| c == b'\n').count();
    let gap = source.get(a.end_byte()..b.start_byte()).unwrap_or_default();
    newlines(gap) + usize::from(source[..a.end_byte()].ends_with(b"\n")) >= 2
}

/// A kind's flag; `false` for a kind past the grammar's own (tree-sitter's error node).
fn flag(flags: &[bool], id: usize) -> bool {
    flags.get(id).copied().unwrap_or(false)
}

/// An entry's kind, qualified name and whether it is test code: what tells it from the others.
type EntryKey = (ScopeKind, String, bool);

/// The classes and functions met so far, in the order first met, each with its code's weight: an entry by its key.
#[derive(Default)]
struct Entries {
    list: Vec<Entry>,
    index: HashMap<EntryKey, usize>,
}

impl Entries {
    /// The entry of `scope`, declared at `line` in `parent` (`None` for the file), made the first time its key is
    /// met.
    fn enter(&mut self, parent: Option<usize>, scope: Scope, test: bool, line: usize) -> usize {
        let key = (scope.kind, self.qualified(parent, scope.name), test);
        *self
            .index
            .entry(key)
            .or_insert_with_key(|(kind, name, test)| {
                self.list.push(Entry {
                    kind: *kind,
                    name: name.clone(),
                    line,
                    weight: 0.0,
                    test: *test,
                });
                self.list.len() - 1
            })
    }

    /// A name qualified by its enclosing entry's: `Outer.Inner.method`.
    fn qualified(&self, parent: Option<usize>, name: String) -> String {
        match parent {
            Some(parent) => format!("{}.{}", self.list[parent].name, name),
            None => name,
        }
    }
}

/// The comments' weights, gathered in the walk: outside any entry, by entry, and of the declarations they are right
/// before (by their entry's key once it is there, else the entry they are in).
struct Comments {
    /// Where the file's header ends, by byte: its comments weigh nothing.
    header: usize,
    file: f64,
    /// By entry index; shorter than the entries when the last ones have none.
    weights: Vec<f64>,
    before: Vec<(EntryKey, Option<usize>, f64)>,
    /// The code a run of comments is before, by its node id, and what it declares: searched once for the run.
    declares_last: Option<(usize, Option<(Scope, bool)>)>,
}

impl Comments {
    fn new(header: usize) -> Comments {
        Comments {
            header,
            file: 0.0,
            weights: Vec::new(),
            before: Vec::new(),
            declares_last: None,
        }
    }

    /// Weighs `node` if it is a comment or a docstring, into `entry`'s weight (`None` for the file's), or kept
    /// aside for the declaration it is right before; a test declaration's comment weighs nothing.
    fn add(
        &mut self,
        weigher: &Weigher,
        node: Node,
        source: &[u8],
        entry: Option<usize>,
        entries: &Entries,
    ) {
        let Some(text) = weigher.comment_text(node, source, self.header) else {
            return;
        };
        let weight = weigher.counter.comment(&text);
        // A docstring is its scope's, never the next declaration's.
        let code = (weight > 0.0 && flag(&weigher.comments, node.kind_id() as usize))
            .then(|| weigher.code_after(node, source))
            .flatten();
        let declared = code.and_then(|code| match &self.declares_last {
            Some((id, declared)) if *id == code.id() => declared.clone(),
            _ => {
                let declared = weigher.declared_by(code, source);
                self.declares_last = Some((code.id(), declared.clone()));
                declared
            }
        });
        match declared {
            // Test code's comment, though before it.
            Some((_, true)) => {}
            Some((scope, false)) => {
                let key = (scope.kind, entries.qualified(entry, scope.name), false);
                self.before.push((key, entry, weight));
            }
            None => self.add_to(entry, weight),
        }
    }

    /// A comment's weight added to its entry's, or to the file's outside any.
    fn add_to(&mut self, entry: Option<usize>, weight: f64) {
        match entry {
            Some(at) => {
                if self.weights.len() <= at {
                    self.weights.resize(at + 1, 0.0);
                }
                self.weights[at] += weight;
            }
            None => self.file += weight,
        }
    }

    /// The comments' weight, once the walk has made every entry: the code's entries with the comments' weights,
    /// test code's left out (their comments weigh nothing, and a test and a non-test entry of one name would be
    /// two of one name in the comments' report).
    fn finish(mut self, entries: &Entries, clean: bool) -> Weighed {
        for (key, inside, weight) in std::mem::take(&mut self.before) {
            let at = entries.index.get(&key).copied().or(inside);
            self.add_to(at, weight);
        }
        self.weights.resize(entries.list.len(), 0.0);
        let list: Vec<Entry> = entries
            .list
            .iter()
            .zip(self.weights)
            .filter(|(entry, _)| !entry.test)
            .map(|(entry, weight)| Entry {
                weight,
                ..entry.clone()
            })
            .collect();
        Weighed {
            total: self.file + list.iter().map(|e| e.weight).sum::<f64>(),
            file: self.file,
            file_tests: 0.0,
            entries: list,
            clean,
        }
    }
}

/// What a node's children nest in, one deeper than it.
#[derive(Clone, Copy)]
enum Holder<'t> {
    /// A nesting construct's body.
    Construct(&'static Nesting),
    /// A function or class declared in a function: its body, all of its children with none.
    Scope(Option<Node<'t>>),
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
    use super::{error_bytes, is_magic};

    #[test]
    fn a_magic_number_is_one_but_0_and_1() {
        for literal in [
            "0", "1", "0.0", "1.", "1.0", "1_0e-1", "0x1", "0b0", "0o1", "1u8", "1.0f64", "1L",
            "1d", "1j", "01", "-1", "+0", "-0x1UL",
        ] {
            assert!(!is_magic(literal), "{literal}");
        }
        for literal in [
            "2", "3.14", "42u32", "1e3", "0x1f", "1_000", "0.5", ".5", "100L", "2j", "-2",
        ] {
            assert!(is_magic(literal), "{literal}");
        }
    }

    #[test]
    fn error_bytes_counts_an_error_deep_in_brackets_without_overflowing() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let depth = 200_000;
        let broken = format!("x = {} @@ {};\n", "[".repeat(depth), "]".repeat(depth));
        let tree = parser.parse(&broken, None).unwrap();
        assert_eq!(error_bytes(tree.root_node()), 2);
        let clean = format!("x = {}{};\n", "[".repeat(depth), "]".repeat(depth));
        let tree = parser.parse(&clean, None).unwrap();
        assert_eq!(error_bytes(tree.root_node()), 0);
    }
}
