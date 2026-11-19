//! The docs weight of a Markdown text: its words and its symbol runs, each weighed by `[docs]`. Pure: text in,
//! weight out; it knows nothing of files, folders or git.

use crate::config::Docs;
use crate::engine::Weighed;

/// A Markdown text's docs weight, `words × word + symbol runs × symbol`, as a file's weight: no entries, always
/// clean (any text is Markdown).
pub fn weigh(text: &str, docs: &Docs) -> Weighed {
    let counts = count(text);
    let weight = counts.words as f64 * docs.word + counts.symbols as f64 * docs.symbol;
    Weighed {
        total: weight,
        file: weight,
        file_tests: 0.0,
        entries: Vec::new(),
        clean: true,
    }
}

/// How many words and symbol runs a text has.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Counts {
    words: usize,
    symbols: usize,
}

/// The text's words and symbol runs, line by line. A fenced block's lines (its fences too) are counted as they
/// are; outside them, YAML front matter at the top, a table's separator line and `<!-- comments -->` count
/// nothing, and a URL and a link's destination (`[a](b/c.md)`'s `b/c.md`) are one word each.
fn count(text: &str) -> Counts {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut runs = Runs::default();
    // A comment opened here closes when a `-->` comes after its `<!--`; with none, its `<!--` is text.
    let last_close = text.rfind("-->");
    let mut fence: Option<Fence> = None;
    let mut in_comment = false;
    let mut offset = front_matter(text);
    for line in text[offset..].split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        if let Some(open) = fence {
            if open.closed_by(line) {
                fence = None;
            }
            runs.plain(line);
            continue;
        }
        let mut from = 0;
        if in_comment {
            match line.find("-->") {
                Some(end) => from = end + 3,
                None => continue,
            }
        } else if let Some(open) = Fence::opened_by(line) {
            fence = Some(open);
            runs.plain(line);
            continue;
        } else if is_table_separator(line) {
            continue;
        }
        let closes_after = |at: usize| last_close.is_some_and(|close| close >= start + at + 4);
        in_comment = runs.prose(line, from, closes_after);
    }
    runs.counts
}

/// The length of the YAML front matter at the top of `text`, its `---` lines included: a first line `---` and the
/// next line `---` or `...`; 0 when there is none.
fn front_matter(text: &str) -> usize {
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return 0;
    };
    if first.trim_end() != "---" {
        return 0;
    }
    let mut length = first.len();
    for line in lines {
        length += line.len();
        if matches!(line.trim_end(), "---" | "...") {
            return length;
        }
    }
    0
}

/// A fenced block's opening fence: its character (`` ` `` or `~`) and its length.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fence {
    char: char,
    length: usize,
}

impl Fence {
    /// The fence `line` opens, if it is one: three or more backticks or tildes after any indentation (a list's
    /// block is indented), a backtick fence's info string without backticks.
    fn opened_by(line: &str) -> Option<Fence> {
        let rest = line.trim_start();
        let char = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        let length = rest.len() - rest.trim_start_matches(char).len();
        if length < 3 || (char == '`' && rest[length..].contains('`')) {
            return None;
        }
        Some(Fence { char, length })
    }

    /// Whether `line` closes the block: as many of its character or more, nothing after them but spaces.
    fn closed_by(self, line: &str) -> bool {
        let rest = line.trim();
        let length = rest.len() - rest.trim_start_matches(self.char).len();
        length >= self.length && length == rest.len()
    }
}

/// Whether `line` is a table's separator line (`|---|:--:|`, `--- | ---`): cells of hyphens, a colon at either end,
/// between pipes.
fn is_table_separator(line: &str) -> bool {
    let line = line.trim();
    if !line.contains('|') {
        return false;
    }
    let inner = line.strip_prefix('|').unwrap_or(line);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').all(|cell| {
        let cell = cell.trim();
        let cell = cell.strip_prefix(':').unwrap_or(cell);
        let cell = cell.strip_suffix(':').unwrap_or(cell);
        !cell.is_empty() && cell.bytes().all(|b| b == b'-')
    })
}

/// The kind of run a character is in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Run {
    Word,
    Symbol,
}

/// The runs counted so far, and the one the last character is in.
#[derive(Default)]
struct Runs {
    counts: Counts,
    current: Option<Run>,
}

impl Runs {
    /// One more character.
    fn push(&mut self, c: char) {
        let run = if c.is_whitespace() {
            None
        } else if c.is_alphanumeric() {
            Some(Run::Word)
        } else {
            Some(Run::Symbol)
        };
        if run != self.current {
            match run {
                Some(Run::Word) => self.counts.words += 1,
                Some(Run::Symbol) => self.counts.symbols += 1,
                None => {}
            }
        }
        self.current = run;
    }

    /// A URL: one word on its own.
    fn url(&mut self) {
        self.counts.words += 1;
        self.current = None;
    }

    /// A line counted as it is, every character by the rule.
    fn plain(&mut self, line: &str) {
        line.chars().for_each(|c| self.push(c));
    }

    /// A line of prose from byte `from`: a URL one word, a link's destination one word, a comment nothing (both
    /// outside inline code, whose `](` and `<!--` are code), `closes_after(at)` telling whether a comment at byte `at` closes. Whether the line ends in a comment
    /// that goes on.
    fn prose(&mut self, line: &str, from: usize, closes_after: impl Fn(usize) -> bool) -> bool {
        let mut at = from;
        // Where the inline code the scan is in ends, its closing backticks' end.
        let mut code_until = 0;
        while let Some(c) = line[at..].chars().next() {
            let rest = &line[at..];
            if at >= code_until {
                if c == '`' {
                    let ticks = rest.len() - rest.trim_start_matches('`').len();
                    if let Some(close) = closing_ticks(&rest[ticks..], ticks) {
                        code_until = at + ticks + close + ticks;
                    }
                    for _ in 0..ticks {
                        self.push('`');
                    }
                    at += ticks;
                    continue;
                }
                if rest.starts_with("](") {
                    self.push(']');
                    self.push('(');
                    at += 2;
                    let length = destination_length(&line[at..]);
                    if length > 0 {
                        self.url();
                        at += length;
                    }
                    continue;
                }
                if rest.starts_with("<!--") && closes_after(at) {
                    match rest[4..].find("-->") {
                        Some(end) => {
                            at += 4 + end + 3;
                            continue;
                        }
                        None => return true,
                    }
                }
            }
            if c.is_alphanumeric()
                && self.current != Some(Run::Word)
                && let Some(length) = url_length(rest)
            {
                self.url();
                at += length;
                continue;
            }
            self.push(c);
            at += c.len_utf8();
        }
        false
    }
}

/// Where in `rest` the backticks closing inline code opened by `ticks` of them start: the next run of exactly as
/// many.
fn closing_ticks(rest: &str, ticks: usize) -> Option<usize> {
    let mut at = 0;
    while let Some(found) = rest[at..].find('`') {
        let start = at + found;
        let length = rest[start..].len() - rest[start..].trim_start_matches('`').len();
        if length == ticks {
            return Some(start);
        }
        at = start + length;
    }
    None
}

/// The length of the link destination `text` starts with, after a link's `](`: one in `<...>`, or else up to a
/// space or the `)` closing the link (a `(` in it matched by a `)`), a `\` escaping the next character; 0 for none.
fn destination_length(text: &str) -> usize {
    if text.starts_with('<')
        && let Some(end) = text.find('>')
    {
        return end + 1;
    }
    let mut depth = 0;
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '(' => depth += 1,
            ')' if depth == 0 => return i,
            ')' => depth -= 1,
            c if c.is_whitespace() => return i,
            _ => {}
        }
    }
    text.len()
}

/// The length of the URL `text` starts with, if it does: `scheme://` and what follows up to a space or a
/// character no URL holds unescaped, less a sentence's trailing punctuation and an unmatched `)` (a link's).
fn url_length(text: &str) -> Option<usize> {
    let scheme = text
        .char_indices()
        .find(|&(i, c)| {
            !(c.is_ascii_alphabetic()
                || (i > 0 && (c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))))
        })
        .map_or(text.len(), |(i, _)| i);
    if scheme == 0 || !text[scheme..].starts_with("://") {
        return None;
    }
    let body = scheme + 3;
    let mut end = text[body..]
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | '|' | '[' | ']'))
        .map_or(text.len(), |i| body + i);
    loop {
        let url = &text[..end];
        let punctuation = url.ends_with(['?', '!', '.', ',', ':', ';', '*', '_', '~', '\'']);
        let unmatched = url.ends_with(')') && url.matches(')').count() > url.matches('(').count();
        if !(punctuation || unmatched) {
            break;
        }
        end -= 1;
    }
    (end > body).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// The docs weight under the test `[docs]`: a word 1, a symbol run 10.
    fn weight(text: &str) -> f64 {
        let config = Config::from_str_alone(crate::lang::testing::WEIGHTS).unwrap();
        assert_eq!((config.docs.word, config.docs.symbol), (1.0, 10.0));
        weigh(text, &config.docs).total
    }

    #[test]
    fn words_and_symbol_runs() {
        assert_eq!(weight("a = b + c"), 3.0 + 20.0);
        assert_eq!(weight("Hello, world."), 2.0 + 20.0);
        assert_eq!(weight("## Setup"), 1.0 + 10.0);
        assert_eq!(weight("**important**"), 1.0 + 20.0);
        assert_eq!(weight("a == b"), 2.0 + 10.0);
        assert_eq!(weight("snake_case"), 2.0 + 10.0);
        assert_eq!(weight("well-known"), 2.0 + 10.0);
        assert_eq!(weight("don't"), 2.0 + 10.0);
        assert_eq!(weight("foo(a, b);"), 3.0 + 30.0);
        assert_eq!(weight("x1 42"), 2.0);
    }

    #[test]
    fn letters_of_any_script_are_words() {
        assert_eq!(weight("Привет, мир!"), 2.0 + 20.0);
        assert_eq!(weight("ёлка-палка"), 2.0 + 10.0);
    }

    #[test]
    fn one_rule_in_inline_code_and_fenced_blocks() {
        assert_eq!(weight("`foo(a, b);`"), 3.0 + 40.0);
        assert_eq!(weight("```rust\nfoo(a, b);\n```\n"), 4.0 + 50.0);
        assert_eq!(weight("~~~~\na = b\n~~~~\n"), 2.0 + 30.0);
    }

    #[test]
    fn a_table_s_separator_line_weighs_nothing() {
        assert_eq!(weight("| a | b |\n"), 2.0 + 30.0);
        assert_eq!(weight("|---|:--:|\n"), 0.0);
        assert_eq!(weight("| a | b |\n|---|:--:|\n| c | d |\n"), 4.0 + 60.0);
        assert_eq!(weight("a | b\n--- | ---\n"), 2.0 + 10.0);
        // A rule without a pipe is no table's.
        assert_eq!(weight("a\n\n---\n"), 1.0 + 10.0);
    }

    #[test]
    fn front_matter_at_the_top_weighs_nothing() {
        assert_eq!(weight("---\ntitle: A b\ntags: [x]\n---\ntext\n"), 1.0);
        assert_eq!(weight("---\ntitle: A\n...\n"), 0.0);
        // Not at the top, or never closed, it is text.
        assert_eq!(weight("text\n---\nt: x\n---\n"), 3.0 + 30.0);
        assert_eq!(weight("---\nt: x\n"), 2.0 + 20.0);
    }

    #[test]
    fn a_comment_weighs_nothing() {
        assert_eq!(weight("a <!-- b c --> d"), 2.0);
        assert_eq!(weight("<!--\nb c\n-->\nd\n"), 1.0);
        assert_eq!(weight("a <!-- b\nc --> d, e\n"), 3.0 + 10.0);
        assert_eq!(weight("a<!-- b -->c"), 1.0);
        // Never closed, it is text.
        assert_eq!(weight("a <!-- b"), 2.0 + 10.0);
        // In inline code it is code, though a `-->` comes after.
        assert_eq!(weight("`<!--` x\n-->\n"), 1.0 + 20.0);
    }

    #[test]
    fn inside_a_fenced_block_everything_counts() {
        assert_eq!(
            weight("```\n<!-- x -->\n|---|\n```\n"),
            // The fences, the comment (`<!--`, x, `-->`), the separator.
            20.0 + 1.0 + 20.0 + 10.0
        );
        // A fence first: no front matter.
        assert_eq!(weight("```\n---\nt: x\n---\n```\n"), 2.0 + 50.0);
        // A comment's `-->` in a block doesn't close one opened before it, nor is one opened in it.
        assert_eq!(weight("a\n```\n<!-- b\n```\nc -->\n"), 3.0 + 40.0);
    }

    #[test]
    fn a_url_is_one_word() {
        assert_eq!(
            weight("[the docs](https://example.com/a/b?c=d&e=f)"),
            3.0 + 30.0
        );
        assert_eq!(weight("see https://example.com/a-b.html."), 2.0 + 10.0);
        assert_eq!(weight("<https://example.com/x>"), 1.0 + 20.0);
        assert_eq!(
            weight("(see https://en.wikipedia.org/wiki/A_(b))"),
            2.0 + 20.0
        );
        // A link's destination is one word, whatever its form; its title is text.
        assert_eq!(weight("[weight.md](weight.md)"), 3.0 + 40.0);
        assert_eq!(weight("[a](../docs/a.md#b-c)"), 2.0 + 30.0);
        assert_eq!(weight("![logo](img/logo.png \"The logo\")"), 4.0 + 40.0);
        assert_eq!(weight("[a](<b c.md>) [d](e_(f).md)"), 4.0 + 60.0);
        assert_eq!(weight("[]()"), 10.0);
        // In inline code, it is code.
        assert_eq!(weight("`[a](b.md)`"), 3.0 + 40.0);
        // No scheme, no URL.
        assert_eq!(weight("example.com"), 2.0 + 10.0);
        // In a fenced block, it is text.
        assert_eq!(weight("```\nhttps://a.b\n```\n"), 3.0 + 40.0);
    }

    #[test]
    fn an_empty_file_weighs_nothing() {
        assert_eq!(weight(""), 0.0);
        assert_eq!(weight("\n  \n\t\n"), 0.0);
    }

    #[test]
    fn a_file_s_weight_with_no_entries() {
        let docs = Docs {
            word: 1.0,
            symbol: 1.0,
        };
        assert_eq!(
            weigh("# Title\n\nSome text.\n", &docs),
            Weighed {
                total: 5.0,
                file: 5.0,
                file_tests: 0.0,
                entries: Vec::new(),
                clean: true,
            }
        );
    }
}
