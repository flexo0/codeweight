//! The runs of an AsciiDoc text, for its docs weight: Markdown's runs (`markdown::Runs`), its lines told apart by
//! AsciiDoc's rules instead of Markdown's. Pure: text in, counts out.

use std::collections::HashSet;

use crate::markdown::{Counts, Runs, closing_ticks, url_length};

/// The block styles whose paragraph is code, as a listing block's content is (`[source,java]`).
const CODE_STYLES: &[&str] = &["source", "listing", "literal"];

/// What the line scanned is in, the block the lines before it opened.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Block<'t> {
    /// Body text: paragraphs, lists, tables, an example's, a sidebar's, a quote's or an open block's content.
    Text,
    /// A listing (`----`), literal (`....`), passthrough (`++++`) or fenced (```` ``` ````) block, up to its
    /// `delimiter`, the line that opened it: code.
    Code { delimiter: &'t str },
    /// A comment block, up to its `delimiter` (`////`, as many slashes as it opened with): nothing.
    Comment { delimiter: &'t str },
    /// A literal paragraph (indented) or one of a code style (`[source]`), up to a blank line: code.
    Paragraph,
}

/// The style the block attribute lines before a block set on it (`[source,java]`, `[verse]`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Style {
    None,
    /// `source`, `listing` or `literal`: its paragraph is code.
    Code,
    /// Any other: an indented paragraph of it is no literal one.
    Other,
}

/// The scan's state between lines.
struct Scan<'t> {
    block: Block<'t>,
    /// Whether the line scanned starts a block: the text's first, one after a blank line, a section's title, a
    /// block's attributes or title, a list continuation (`+`).
    start: bool,
    style: Style,
    /// Whether the last line but blank ones and comments is a description list's term with no text after it
    /// (`` `-n`:: ``): an indented paragraph after it is its text, no literal one.
    term: bool,
    /// The length in characters of the line before, when it can be a section's title an underline makes.
    title: Option<usize>,
}

/// The text's words, function words (of `function_words`, lowercase), punctuation runs and symbol runs, line by
/// line, as a Markdown text's are but by AsciiDoc's rules: a comment (`//` at a line's start, a `////` block)
/// counts nothing; a listing, literal, passthrough or fenced block (`----`, `....`, `++++`, ```` ``` ````), a
/// literal paragraph (indented, but a description list's text) and a paragraph of a code style (`[source]`) are
/// code; a block's delimiter, a section's underline, a description list's `::` (`;;`), an attribute entry's colons
/// (`:toc:`), a block title's `.` and a macro's colon (`linkgit:`, `include::`) are markup, a symbol run each; a
/// table's border (`|===`) counts nothing; a URL, a macro's target and a cross reference's id (`<<id,text>>`) are a
/// word each; backticks are inline code.
pub(crate) fn count(text: &str, function_words: &HashSet<String>) -> Counts {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut runs = Runs::new(function_words);
    let mut scan = Scan {
        block: Block::Text,
        start: true,
        style: Style::None,
        term: false,
        title: None,
    };
    for line in text.split_inclusive('\n') {
        scan.line(&mut runs, line);
    }
    runs.end();
    runs.counts
}

impl<'t> Scan<'t> {
    /// One more line weighed.
    fn line(&mut self, runs: &mut Runs, line: &'t str) {
        let rest = line.trim_end();
        let blank = rest.trim_start().is_empty();
        match self.block {
            Block::Code { delimiter } => {
                if rest == delimiter {
                    delimiter_markup(runs, rest);
                    self.block_ended();
                } else {
                    runs.plain(line);
                }
                return;
            }
            Block::Comment { delimiter } => {
                if rest == delimiter {
                    self.block_ended();
                }
                return;
            }
            Block::Paragraph if !blank => {
                runs.plain(line);
                return;
            }
            Block::Paragraph | Block::Text => self.block = Block::Text,
        }
        if blank {
            self.start = true;
            self.title = None;
            return;
        }
        let start = self.start;
        let title = self.title.take();
        self.start = false;
        if rest.starts_with("//") {
            if is_uniform(rest, '/') && rest.len() >= 4 {
                self.block = Block::Comment { delimiter: rest };
            } else {
                // A comment line leaves what is around it as it was.
                self.start = start;
            }
            return;
        }
        if let Some(length) = title
            && is_underline(rest)
            && length.abs_diff(rest.chars().count()) < 2
        {
            delimiter_markup(runs, rest);
            self.block_ended();
            return;
        }
        if let Some(block) = delimited(rest) {
            if let Some(language) = rest.strip_prefix("```") {
                delimiter_markup(runs, "```");
                prose(runs, language);
                runs.end();
            } else {
                delimiter_markup(runs, rest);
            }
            self.block = block;
            self.style = Style::None;
            self.term = false;
            return;
        }
        if is_table_border(rest) {
            self.block_ended();
            return;
        }
        if start {
            if let Some(style) = attribute_list(rest) {
                if style != Style::None {
                    self.style = style;
                }
                prose(runs, rest);
                runs.end();
                self.start = true;
                return;
            }
            if let Some(title) = rest.strip_prefix('.')
                && title.starts_with(|c: char| !c.is_whitespace() && c != '.')
            {
                runs.markup(".");
                prose(runs, title);
                runs.end();
                self.start = true;
                return;
            }
            if let Some(name) = attribute_entry(rest) {
                runs.markup(":");
                prose(runs, &rest[1..name]);
                runs.markup(":");
                prose(runs, &rest[name + 1..]);
                runs.end();
                self.start = true;
                return;
            }
            let literal = line.starts_with([' ', '\t'])
                && self.style == Style::None
                && !self.term
                && !is_list_item(rest.trim_start());
            if self.style == Style::Code || literal {
                runs.plain(line);
                self.block = Block::Paragraph;
                self.style = Style::None;
                self.term = false;
                return;
            }
        }
        self.style = Style::None;
        self.term = false;
        if rest == "+" {
            runs.markup("+");
            runs.end();
            self.start = true;
            return;
        }
        if let Some((at, end)) = term_separator(rest) {
            prose(runs, &rest[..at]);
            runs.markup(&rest[at..end]);
            prose(runs, &line[end..]);
            self.term = rest[end..].trim().is_empty();
            return;
        }
        prose(runs, line);
        if rest.starts_with('=') && rest.trim_start_matches('=').starts_with(' ') {
            // A section's title, `== Usage`: the next line starts a block.
            self.start = true;
        } else if start
            && !line.starts_with([' ', '\t', '.'])
            && rest.contains(char::is_alphanumeric)
        {
            self.title = Some(rest.chars().count());
        }
    }

    /// A block or a section's title ended: the next line starts one.
    fn block_ended(&mut self) {
        self.block = Block::Text;
        self.start = true;
        self.style = Style::None;
        self.term = false;
    }
}

/// A delimiter's or an underline's line, `text`, weighed: markup, a symbol run.
fn delimiter_markup(runs: &mut Runs, text: &str) {
    if text.is_empty() {
        return;
    }
    runs.code = false;
    runs.end();
    runs.markup(text);
    runs.end();
}

/// The code block that `line` (its end trimmed) opens as its delimiter, if it is one: four or more `-`, `.` or `+`
/// alone (a listing, a literal, a passthrough block), or a fence of three backticks and a language after them.
fn delimited(line: &str) -> Option<Block<'_>> {
    if line.starts_with("```") {
        return Some(Block::Code { delimiter: "```" });
    }
    let first = line.chars().next()?;
    let verbatim = matches!(first, '-' | '.' | '+') && line.len() >= 4 && is_uniform(line, first);
    verbatim.then_some(Block::Code { delimiter: line })
}

/// Whether `line` is all of `c`.
fn is_uniform(line: &str, c: char) -> bool {
    line.chars().all(|d| d == c)
}

/// Whether `line` (its end trimmed) can be a section title's underline: one of `=`, `-`, `~`, `^` or `+` repeated.
fn is_underline(line: &str) -> bool {
    line.chars().next().is_some_and(|first| {
        matches!(first, '=' | '-' | '~' | '^' | '+') && is_uniform(line, first)
    })
}

/// Whether `line` (its end trimmed) is a table's border: `|===`, or a data table's `,===`, `:===`, `!===`.
fn is_table_border(line: &str) -> bool {
    line.starts_with(['|', ',', ':', '!']) && line.len() >= 4 && is_uniform(&line[1..], '=')
}

/// The style of the block attribute line `line` (its end trimmed) is, if it is one: a list in brackets, empty or
/// from a word's character, `#`, `.`, `%`, `,`, `{` or a quote (`[source,java]`, `[[anchor]]`, `[#id]`; not
/// `[--all]`); its first positional attribute the style.
fn attribute_list(line: &str) -> Option<Style> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let opens = inner.chars().next().is_none_or(|c| {
        c.is_alphanumeric() || matches!(c, '_' | '#' | '.' | '%' | ',' | '{' | '"' | '\'' | '[')
    });
    if !opens {
        return None;
    }
    if inner.starts_with('[') {
        return Some(Style::None);
    }
    let style = inner
        .split([',', '#', '.', '%'])
        .next()
        .unwrap_or("")
        .trim();
    Some(if style.is_empty() {
        Style::None
    } else if CODE_STYLES.contains(&style) {
        Style::Code
    } else {
        Style::Other
    })
}

/// Where the closing colon of the attribute entry `line` is (`:toc: left`, `:sectnums!:`): a name of letters,
/// digits, `-` and `_` (and a `!`) between colons at the line's start, then a space or the end.
fn attribute_entry(line: &str) -> Option<usize> {
    let name = line.strip_prefix(':')?;
    let close = name.find(':')?;
    let valid = close > 0
        && name[..close]
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '!'))
        && name[close + 1..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
    valid.then_some(close + 1)
}

/// Whether `line` (trimmed) starts a list item: a bullet (`*`, `-`, `•`), an ordered item's `.` or `1.`, a
/// callout (`<1>`) or a description list's term.
fn is_list_item(line: &str) -> bool {
    let marker = line
        .find(char::is_whitespace)
        .map_or(line, |end| &line[..end]);
    let bullet = !marker.is_empty() && (is_uniform(marker, '*') || is_uniform(marker, '.'));
    let ordered = marker.len() >= 2
        && marker.ends_with(['.', ')'])
        && marker[..marker.len() - 1]
            .chars()
            .all(|c| c.is_ascii_alphanumeric());
    let callout = marker.starts_with('<') && marker.ends_with('>');
    let spaced = line.len() > marker.len();
    (spaced && (bullet || ordered || callout || matches!(marker, "-" | "•")))
        || term_separator(line).is_some()
}

/// Where the separator of the description list's term `line` (its end trimmed) starts with is, and its end:
/// the first `::`, `:::`, `::::` or `;;` after the term's text, then a space or the line's end.
fn term_separator(line: &str) -> Option<(usize, usize)> {
    let text = line.trim_start();
    let mut from = line.len() - text.len() + text.chars().next()?.len_utf8();
    while let Some(found) = line[from..].find([':', ';']) {
        let at = from + found;
        let mark = if line[at..].starts_with(':') {
            ':'
        } else {
            ';'
        };
        let length = line[at..].len() - line[at..].trim_start_matches(mark).len();
        let fits = if mark == ':' {
            (2..=4).contains(&length)
        } else {
            length == 2
        };
        let end = at + length;
        if fits && line[end..].chars().next().is_none_or(char::is_whitespace) {
            return Some((at, end));
        }
        from = end;
    }
    None
}

/// A line of body text, or a part of one: a URL one word, a macro's colons markup (`linkgit:`, `include::`) and
/// its target one word, a cross reference's id one word, backticks inline code.
fn prose(runs: &mut Runs, text: &str) {
    let mut at = 0;
    // Where the inline code the scan is in ends, its closing backticks' end.
    let mut code_until = 0;
    while let Some(c) = text[at..].chars().next() {
        let rest = &text[at..];
        runs.code = at < code_until;
        if at < code_until {
            runs.push(c);
            at += c.len_utf8();
            continue;
        }
        if c == '`' {
            let ticks = rest.len() - rest.trim_start_matches('`').len();
            if let Some(close) = closing_ticks(&rest[ticks..], ticks) {
                code_until = at + ticks + close + ticks;
            }
            for _ in 0..ticks {
                runs.push('`');
            }
            at += ticks;
            continue;
        }
        if c.is_alphanumeric() && !runs.in_word() {
            if let Some(length) = url_length(rest) {
                runs.url();
                at += length;
                continue;
            }
            if let Some((name, target)) = macro_parts(rest) {
                rest[..name].chars().for_each(|c| runs.push(c));
                runs.end();
                runs.markup(&rest[name..target.start]);
                if !target.is_empty() {
                    runs.url();
                }
                at += target.end;
                continue;
            }
        }
        if let Some(id) = rest.strip_prefix("<<").and_then(xref_id) {
            runs.markup("<<");
            runs.url();
            at += 2 + id;
            continue;
        }
        runs.push(c);
        at += c.len_utf8();
    }
}

/// The parts of the macro `text` starts with (`linkgit:git-add[1]`, `include::a.adoc[]`, `footnote:[a]`): its
/// name's end, and its target's range, between its colons and its `[`; `None` for none. A name of letters, digits,
/// `-` and `_`, from a letter; one or two colons; a target of no space nor bracket; a `]` after the `[`.
fn macro_parts(text: &str) -> Option<(usize, std::ops::Range<usize>)> {
    if !text.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let name = text.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))?;
    let after = &text[name..];
    let colons = after.len() - after.trim_start_matches(':').len();
    if !(1..=2).contains(&colons) {
        return None;
    }
    let start = name + colons;
    let length = text[start..].find(|c: char| c.is_whitespace() || matches!(c, '[' | ']'))?;
    let end = start + length;
    let valid = text[end..].starts_with('[') && text[end..].contains(']');
    valid.then_some((name, start..end))
}

/// The length of the cross reference's id `text` starts with, past its `<<` (`a-b` of `<<a-b,the text>>`): up to
/// a `,` or the `>>`, no space in it; `None` for none.
fn xref_id(text: &str) -> Option<usize> {
    let close = text.find(">>")?;
    let id = text[..close].find(',').unwrap_or(close);
    let valid = id > 0 && !text[..id].contains(char::is_whitespace);
    valid.then_some(id)
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::markdown::{Format, weigh};

    /// The docs weight of an AsciiDoc text under the test `[docs]`: a word 1, a symbol run 10, a punctuation run
    /// 100, a function word (`the`, `of`) 1000.
    fn weight(text: &str) -> f64 {
        let config = Config::from_str_alone(crate::lang::testing::WEIGHTS).unwrap();
        weigh(text, Format::AsciiDoc, &config.docs).total
    }

    #[test]
    fn prose_weighs_as_markdown_s() {
        assert_eq!(weight("Hello, world."), 2.0 + 200.0);
        assert_eq!(weight("the end of it"), 2000.0 + 2.0);
        assert_eq!(weight("*important* and *this*"), 3.0 + 40.0);
        assert_eq!(weight("see https://example.com/a-b.html."), 2.0 + 100.0);
        assert_eq!(weight(""), 0.0);
    }

    #[test]
    fn a_comment_weighs_nothing() {
        assert_eq!(weight("// a note to the editors\nText.\n"), 1.0 + 100.0);
        assert_eq!(weight("////\nthe\n\nof\n////\nText.\n"), 1.0 + 100.0);
        // Closed by as many slashes as it opened with.
        assert_eq!(weight("//////\nthe\n////\nof\n//////\nb\n"), 1.0);
        // Not at a line's start it is text.
        assert_eq!(weight("a // b"), 2.0 + 10.0);
    }

    #[test]
    fn a_section_s_title_and_underline() {
        // The underline one symbol run, `-` and `=` alike.
        assert_eq!(weight("NAME\n----\nthe\n"), 1.0 + 10.0 + 1000.0);
        assert_eq!(weight("git-add(1)\n==========\n"), 3.0 + 300.0 + 10.0);
        // `==` before the title a symbol run, and the next line is no title's either.
        assert_eq!(weight("== Usage\n----\nthe\n----\n"), 2.0 + 30.0);
    }

    #[test]
    fn a_listing_a_literal_block_and_a_fence_are_code() {
        // In code a function word is a word; the delimiters a symbol run each.
        assert_eq!(weight("----\nthe = of\n----\nthe\n"), 2.0 + 30.0 + 1000.0);
        assert_eq!(weight("....\nthe\n....\n"), 1.0 + 20.0);
        // Closed by the same line only; with none, to the end.
        assert_eq!(weight("-----\nthe\n----\nof\n"), 2.0 + 20.0);
        // A fence's language is a word.
        assert_eq!(weight("```java\nthe\n```\n"), 2.0 + 20.0);
        // A delimiter right under a text line as long, give or take one, is its underline.
        assert_eq!(weight("Text\n----\nthe\n"), 1.0 + 10.0 + 1000.0);
        assert_eq!(weight("Text\n\n----\nthe\n----\n"), 2.0 + 20.0);
        assert_eq!(weight("A\n----\nthe\n----\n"), 2.0 + 20.0);
    }

    #[test]
    fn a_literal_paragraph_and_a_code_style_s_are_code() {
        // An indented paragraph, up to a blank line.
        assert_eq!(
            weight("Run:\n\n   the add\n   of\n\nthe\n"),
            4.0 + 100.0 + 1000.0
        );
        // Indented past a paragraph's first line, it is the paragraph's.
        assert_eq!(weight("a\n  the\n"), 1.0 + 1000.0);
        // A description list's text, an indented list item, are no literal paragraph.
        assert_eq!(weight("`-n`::\n\tthe\n"), 1.0 + 20.0 + 1000.0);
        assert_eq!(weight("`-n`::\n\n\tthe\n"), 1.0 + 20.0 + 1000.0);
        assert_eq!(weight("  * the\n"), 10.0 + 1000.0);
        // `[source]` makes a paragraph code, its attributes a list's; `[verse]` leaves an indented one text.
        assert_eq!(weight("[source,sh]\nthe\n\nthe\n"), 3.0 + 120.0 + 1000.0);
        assert_eq!(weight("[verse]\n  the\n"), 1.0 + 20.0 + 1000.0);
    }

    #[test]
    fn a_description_list_s_and_an_attribute_entry_s_colons_are_markup() {
        assert_eq!(weight("Term:: the end\n"), 2.0 + 10.0 + 1000.0);
        assert_eq!(weight("Term;;\n"), 1.0 + 10.0);
        assert_eq!(weight(":toc: left\n"), 2.0 + 20.0);
        // Not a term, colons are punctuation.
        assert_eq!(weight("a: b::c"), 3.0 + 200.0);
    }

    #[test]
    fn a_block_title_and_a_list_continuation_are_markup() {
        assert_eq!(weight(".Example\n"), 1.0 + 10.0);
        // An indented paragraph after it is literal: `the` a word.
        assert_eq!(weight("* a\n+\n  the\n"), 2.0 + 20.0);
    }

    #[test]
    fn a_macro_s_colons_are_markup_and_its_target_a_word() {
        // `linkgit`, `:`, the target, `[`, `1`, `]`.
        assert_eq!(weight("linkgit:git-add[1]"), 3.0 + 30.0);
        assert_eq!(weight("include::a/b.adoc[]\n"), 2.0 + 20.0);
        assert_eq!(weight("footnote:[the]"), 1000.0 + 1.0 + 20.0);
        // A URL's text in brackets is prose.
        assert_eq!(weight("https://a.org/b[the docs]"), 2.0 + 1000.0 + 20.0);
        // `<<`, the id, `,`, the text, `>>`.
        assert_eq!(
            weight("<<EDITING_PATCHES,the patches>>"),
            2.0 + 1000.0 + 100.0 + 20.0
        );
    }

    #[test]
    fn backticks_are_inline_code_and_a_table_s_border_nothing() {
        assert_eq!(weight("`the`"), 1.0 + 200.0);
        assert_eq!(weight("|===\n|a |b\n|===\n"), 2.0 + 20.0);
    }
}
