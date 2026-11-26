//! The runs of a reStructuredText text, for its docs weight: Markdown's runs (`markdown::Runs`), its lines told
//! apart by reST's rules instead of Markdown's. Pure: text in, counts out.

use std::collections::HashSet;

use crate::markdown::{Counts, Runs, closing_ticks, url_length};

/// The directives whose content is code, as a fenced block's is in Markdown.
const CODE_DIRECTIVES: &[&str] = &[
    "code",
    "code-block",
    "sourcecode",
    "parsed-literal",
    "doctest",
    "testcode",
    "testoutput",
    "testsetup",
    "testcleanup",
    "productionlist",
];

/// What the line scanned is in, the block the lines before it opened.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Block {
    /// Body text: paragraphs, lists, tables, a directive's content but a code directive's.
    Text,
    /// A comment: the lines indented more than its `..` (`indent`), blank ones too but in an empty comment's (`..`
    /// alone on its line, which a blank line ends).
    Comment { indent: usize, empty: bool },
    /// A code directive's options, its lines indented more than its `..` up to a blank line, then its code.
    Options { indent: usize },
    /// A literal block's or a code directive's code, the lines indented more than `indent`, blank ones too.
    Code { indent: usize },
    /// A doctest block, from its `>>>` up to a blank line: code.
    Doctest,
}

/// The text's words, function words (of `function_words`, lowercase), punctuation runs and symbol runs, line by
/// line, as a Markdown text's are but by reST's rules: a comment (`..` and text that is no directive, target,
/// footnote nor substitution, with its indented lines) counts nothing; a literal block (the indented lines after a
/// `::`), a code directive's content and a doctest block are code; a directive's `..` and `::`, a role's and a
/// field's colons (`:ref:`, `:param x:`) and a section's adornment line are markup, a symbol run each; a table's
/// border (`+---+---+`, `===  ===`) counts nothing; a URL and a link's embedded URI (`` `a <b.html>`_ ``) are a
/// word each; only double backticks are inline code.
pub(crate) fn count(text: &str, function_words: &HashSet<String>) -> Counts {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut runs = Runs::new(function_words);
    let mut block = Block::Text;
    for line in text.split_inclusive('\n') {
        let blank = line.trim().is_empty();
        let indent = indentation(line);
        let taken = match block {
            Block::Text => false,
            Block::Comment { indent: i, empty } => {
                if blank {
                    !empty
                } else {
                    indent > i
                }
            }
            Block::Options { indent: i } | Block::Code { indent: i } => blank || indent > i,
            Block::Doctest => !blank,
        };
        if taken {
            match block {
                Block::Options { indent: i } if blank => block = Block::Code { indent: i },
                Block::Options { .. } => prose(&mut runs, line, true),
                Block::Code { .. } | Block::Doctest => runs.plain(line),
                Block::Comment { .. } | Block::Text => {}
            }
            continue;
        }
        block = if blank {
            Block::Text
        } else {
            text_line(&mut runs, line, indent)
        };
    }
    runs.end();
    runs.counts
}

/// A body text's line weighed, at `indent`, and the block it opens.
fn text_line(runs: &mut Runs, line: &str, indent: usize) -> Block {
    let rest = line.trim();
    if let Some(after) = rest.strip_prefix("..")
        && (after.is_empty() || after.starts_with(char::is_whitespace))
    {
        let block = explicit(runs, after.trim_start(), indent);
        // Its text is weighed without the line's end, which ends a run.
        runs.end();
        return block;
    }
    if is_table_border(rest) {
        return Block::Text;
    }
    if indent == 0 && is_adornment(rest) {
        rest.chars().for_each(|c| runs.markup(c));
        runs.end();
        return Block::Text;
    }
    if rest == ">>>" || rest.starts_with(">>> ") {
        runs.plain(line);
        return Block::Doctest;
    }
    if rest == "::" {
        runs.markup(':');
        runs.markup(':');
        runs.end();
        return Block::Code { indent };
    }
    prose(runs, line, true);
    if rest.ends_with("::") {
        return Block::Code {
            indent: text_indent(line),
        };
    }
    Block::Text
}

/// An explicit markup line, its text after the `..` (`after`, trimmed), at `indent`: a directive, a hyperlink
/// target, a footnote, a citation or a substitution definition weighed, its `..` markup; else a comment, nothing.
/// The block it opens.
fn explicit(runs: &mut Runs, after: &str, indent: usize) -> Block {
    if after.is_empty() {
        return Block::Comment {
            indent,
            empty: true,
        };
    }
    let target = after.starts_with(['_', '[']);
    let substitution = after
        .strip_prefix('|')
        .and_then(|s| s.find('|'))
        .map(|close| close + 2);
    let named = directive_name(after);
    if !target && substitution.is_none() && named.is_none() {
        return Block::Comment {
            indent,
            empty: false,
        };
    }
    runs.markup('.');
    runs.markup('.');
    runs.end();
    if target {
        prose(runs, after, false);
        return Block::Text;
    }
    let after = match substitution {
        Some(end) => {
            prose(runs, &after[..end], false);
            runs.end();
            after[end..].trim_start()
        }
        None => after,
    };
    let Some(name) = directive_name(after) else {
        prose(runs, after, false);
        return Block::Text;
    };
    for c in after[..name].chars() {
        if c == ':' {
            runs.markup(c);
        } else {
            runs.push(c);
        }
    }
    runs.end();
    runs.markup(':');
    runs.markup(':');
    prose(runs, &after[name + 2..], false);
    if CODE_DIRECTIVES.contains(&&after[..name]) {
        Block::Options { indent }
    } else {
        Block::Text
    }
}

/// The length of the directive's name `text` starts with (`code-block` of `code-block:: python`, `py:function`):
/// letters, digits and single `-`, `_`, `+`, `.` or `:` between them, then `::` and a space or the end.
fn directive_name(text: &str) -> Option<usize> {
    let end = text.find("::")?;
    let name = &text[..end];
    let after = &text[end + 2..];
    let valid = name.starts_with(char::is_alphanumeric)
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '+' | '.' | ':'))
        && (after.is_empty() || after.starts_with(char::is_whitespace));
    valid.then_some(end)
}

/// Whether `line` (trimmed) is a section's adornment, an overline or underline: three or more of one punctuation
/// character.
fn is_adornment(line: &str) -> bool {
    let mut chars = line.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_punctuation() && line.len() >= 3 && chars.all(|c| c == first)
}

/// Whether `line` (trimmed) is a table's border: a grid table's (`+---+:==+`, between `+`s) or a simple table's
/// (`===  ===`, two columns or more).
fn is_table_border(line: &str) -> bool {
    if line.len() >= 3 && line.starts_with('+') && line.ends_with('+') {
        return line.contains(['-', '='])
            && line.chars().all(|c| matches!(c, '+' | '-' | '=' | ':'));
    }
    line.chars().all(|c| c == '=' || c == ' ') && line.split_whitespace().count() >= 2
}

/// The columns `line` is indented by, a tab to the next multiple of 8.
fn indentation(line: &str) -> usize {
    let mut columns = 0;
    for c in line.chars() {
        match c {
            ' ' => columns += 1,
            '\t' => columns = (columns / 8 + 1) * 8,
            _ => break,
        }
    }
    columns
}

/// The column the text of `line` starts at: past its indentation and a list item's bullet (`-`, `*`, `+`, `•`) or
/// enumerator (`1.`, `#.`, `a)`, `(i)`) and the spaces after it.
fn text_indent(line: &str) -> usize {
    let indent = indentation(line);
    let rest = line.trim_start();
    let marker = if rest.starts_with(['-', '*', '+', '•']) {
        rest.chars().next().map_or(0, char::len_utf8)
    } else {
        let open = usize::from(rest.starts_with('('));
        let label = rest[open..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .unwrap_or(0);
        let close = rest[open + label..].chars().next();
        let closes = if open == 1 {
            close == Some(')')
        } else {
            matches!(close, Some('.' | ')'))
        };
        if label > 0 && closes {
            open + label + 1
        } else {
            0
        }
    };
    let after = &rest[marker..];
    if marker > 0 && after.starts_with(char::is_whitespace) {
        indent + marker + indentation(after)
    } else {
        indent
    }
}

/// A line of body text, or the part of one from `text`: a URL one word, a link's embedded URI one word, a role's
/// colons markup and a field's (`field`: one may start `text`), double backticks inline code.
fn prose(runs: &mut Runs, text: &str, field: bool) {
    let start = text.len() - text.trim_start().len();
    let field_end = if field {
        field_marker(&text[start..]).map(|end| start + end)
    } else {
        None
    };
    let mut at = 0;
    // Where the inline literal the scan is in ends, its closing backticks' end.
    let mut code_until = 0;
    while let Some(c) = text[at..].chars().next() {
        let rest = &text[at..];
        runs.code = at < code_until;
        if at < code_until {
            runs.push(c);
            at += c.len_utf8();
            continue;
        }
        if c == ':' && (at == start || Some(at) == field_end) && field_end.is_some() {
            runs.markup(c);
            at += 1;
            continue;
        }
        if c == ':'
            && !runs.in_word()
            && let Some(end) = role_end(rest)
        {
            for c in rest[..end].chars() {
                if c == ':' {
                    runs.markup(c);
                } else {
                    runs.push(c);
                }
            }
            at += end;
            continue;
        }
        if c == '`' {
            let ticks = rest.len() - rest.trim_start_matches('`').len();
            if ticks == 2
                && let Some(close) = closing_ticks(&rest[ticks..], ticks)
            {
                code_until = at + ticks + close + ticks;
            }
            for _ in 0..ticks {
                runs.push('`');
            }
            at += ticks;
            continue;
        }
        if c == '<'
            && let Some(end) = embedded_uri(rest)
        {
            runs.push('<');
            runs.url();
            at += end;
            continue;
        }
        if c.is_alphanumeric()
            && !runs.in_word()
            && let Some(length) = url_length(rest)
        {
            runs.url();
            at += length;
            continue;
        }
        runs.push(c);
        at += c.len_utf8();
    }
}

/// Where the closing colon of the field marker `text` starts with is (`:param x:` of `:param x: a`): a name
/// between colons, then a space or the end; `None` for none.
fn field_marker(text: &str) -> Option<usize> {
    let name = text.strip_prefix(':')?;
    let close = name
        .char_indices()
        .find(|&(i, c)| c == ':' && name[i + 1..].chars().next().is_none_or(char::is_whitespace))
        .map(|(i, _)| i)?;
    let valid = close > 0 && !name.starts_with(char::is_whitespace) && !name[..close].contains('`');
    valid.then_some(close + 1)
}

/// The length of the role `text` starts with, up to its backtick (`:ref:` of `` :ref:`x` ``, `:py:func:`): a
/// name of letters, digits, `-`, `_`, `+`, `.` and `:` between colons; `None` for none.
fn role_end(text: &str) -> Option<usize> {
    let end = text[1..]
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '+' | '.' | ':')))
        .map_or(text.len(), |i| i + 1);
    let role = &text[..end];
    let valid = end >= 3
        && role.ends_with(':')
        && role.chars().any(char::is_alphanumeric)
        && text[end..].starts_with('`');
    valid.then_some(end)
}

/// The length of the embedded URI `text` starts with up to its `>` (`<b.html` of `` <b.html>`_ ``): no space in
/// it, a backtick right after it; `None` for none.
fn embedded_uri(text: &str) -> Option<usize> {
    let close = text.find('>')?;
    let uri = &text[1..close];
    let valid =
        !uri.is_empty() && !uri.contains(char::is_whitespace) && text[close + 1..].starts_with('`');
    valid.then_some(close)
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::markdown::{Format, weigh};

    /// The docs weight of a reST text under the test `[docs]`: a word 1, a symbol run 10, a punctuation run 100, a
    /// function word (`the`, `of`) 1000.
    fn weight(text: &str) -> f64 {
        let config = Config::from_str_alone(crate::lang::testing::WEIGHTS).unwrap();
        weigh(text, Format::Rst, &config.docs).total
    }

    #[test]
    fn prose_weighs_as_markdown_s() {
        assert_eq!(weight("Hello, world."), 2.0 + 200.0);
        assert_eq!(weight("the end of it"), 2000.0 + 2.0);
        assert_eq!(weight("**important** and *this*"), 3.0 + 40.0);
        assert_eq!(weight("see https://example.com/a-b.html."), 2.0 + 100.0);
        assert_eq!(weight("Привет, мир!"), 2.0 + 200.0);
        assert_eq!(weight(""), 0.0);
    }

    #[test]
    fn a_section_s_adornment_is_one_symbol_run() {
        assert_eq!(weight("=====\nTitle\n=====\n"), 1.0 + 20.0);
        // Of any punctuation, prose's too, and `~`, which opens no block as a Markdown fence does.
        assert_eq!(weight("Title\n\"\"\"\"\"\n"), 1.0 + 10.0);
        assert_eq!(weight("A\n~~~\n\nthe\n\nB\n~~~\n"), 1000.0 + 2.0 + 20.0);
        // Indented, or of two characters, it is text.
        assert_eq!(weight("  ...\n"), 100.0);
    }

    #[test]
    fn a_comment_weighs_nothing() {
        assert_eq!(
            weight(".. SPDX-License-Identifier: GPL-2.0\n\nText.\n"),
            1.0 + 100.0
        );
        // Its indented lines too, a blank line among them; the first line back at its indentation ends it.
        assert_eq!(weight(".. a comment\n   on lines\n\n   still\nText\n"), 1.0);
        assert_eq!(weight("..\n   b c\nd\n"), 1.0);
        // An empty comment, a blank line after it, takes no indented block.
        assert_eq!(weight("..\n\n   b c\n"), 2.0);
        // Not at a line's start, nor `..` before a word, it is text.
        assert_eq!(weight("a .. b ..c"), 3.0 + 200.0);
    }

    #[test]
    fn a_directive_s_markup_is_symbol_runs() {
        // `..` and `::` a symbol each, the name and the arguments words.
        assert_eq!(weight(".. note:: the end\n"), 1000.0 + 2.0 + 20.0);
        assert_eq!(weight(".. code-block:: python\n"), 3.0 + 20.0 + 100.0);
        // Its last word ends with its line.
        assert_eq!(weight(".. note:: a\nb\n"), 3.0 + 20.0);
        // A name's colon is markup: `py`, `:`, `function`, the argument `f`, `()`.
        assert_eq!(weight(".. py:function:: f()\n"), 3.0 + 30.0 + 100.0);
        // Its content is body text.
        assert_eq!(
            weight(".. note::\n\n   The end.\n"),
            1000.0 + 2.0 + 20.0 + 100.0
        );
        // A hyperlink target, a footnote and a substitution are markup too, not comments.
        assert_eq!(
            weight(".. _the docs: https://a.org/b\n"),
            1000.0 + 2.0 + 10.0 + 200.0
        );
        assert_eq!(weight(".. [1] A note.\n"), 3.0 + 30.0 + 100.0);
        assert_eq!(weight(".. |logo| image:: logo.png\n"), 4.0 + 40.0 + 100.0);
    }

    #[test]
    fn a_code_directive_s_content_and_a_literal_block_are_code() {
        // In code a function word is a word.
        let code = ".. code-block:: python\n   :linenos:\n\n   the = of\n\nThe end.\n";
        assert_eq!(
            weight(code),
            // The directive (`..`, code, `-`, block, `::`, python), its option (`:`, linenos, `:`), the code,
            // the end.
            3.0 + 20.0 + 100.0 + 1.0 + 20.0 + 2.0 + 10.0 + 1000.0 + 1.0 + 100.0
        );
        // A paragraph's `::` opens a literal block of the lines indented more.
        assert_eq!(
            weight("Example::\n\n    the\n\nthe\n"),
            2.0 + 100.0 + 1000.0
        );
        assert_eq!(weight("::\n\n    the\n"), 1.0 + 10.0);
        // A list item's text is past its bullet: its next paragraph, indented as its text, is no code.
        assert_eq!(
            weight("- Example::\n\n      the\n\n  the\n"),
            2.0 + 100.0 + 100.0 + 1000.0
        );
        // A doctest block, up to a blank line.
        assert_eq!(weight(">>> the\nof\n\nthe\n"), 2.0 + 10.0 + 1000.0);
        // Not a code directive, its content is text.
        assert_eq!(weight(".. note::\n\n   the\n"), 1.0 + 20.0 + 1000.0);
    }

    #[test]
    fn a_role_s_and_a_field_s_colons_are_markup() {
        // `:`, `ref`, ``:` ``, `the`, `` ` ``: the role's colons symbols, its text prose.
        assert_eq!(weight(":ref:`the`"), 1.0 + 1000.0 + 20.0 + 100.0);
        assert_eq!(weight(":py:func:`f`"), 3.0 + 30.0 + 100.0);
        // A field's: `:`, `param`, `x`, `:`, then its text.
        assert_eq!(weight(":param x: the end\n"), 2.0 + 20.0 + 1000.0 + 1.0);
        assert_eq!(weight("   :maxdepth: 2\n"), 2.0 + 20.0);
        // Not before a backtick, nor at a line's start, colons are punctuation.
        assert_eq!(weight("a: b:c"), 3.0 + 200.0);
    }

    #[test]
    fn double_backticks_are_inline_code_and_an_embedded_uri_one_word() {
        assert_eq!(weight("``the``"), 1.0 + 200.0);
        // Single backticks are interpreted text, prose.
        assert_eq!(weight("`the`"), 1000.0 + 200.0);
        // `` ` ``, `a`, `<`, the URI, `` >`_ ``.
        assert_eq!(weight("`a <b/c.html>`_"), 2.0 + 20.0 + 100.0);
        assert_eq!(weight("`a <https://x.org/y>`_"), 2.0 + 20.0 + 100.0);
        // `<` and `>` around a word with no backtick after are symbols.
        assert_eq!(weight("a <b> c"), 3.0 + 20.0);
    }

    #[test]
    fn a_table_s_borders_weigh_nothing() {
        let grid = "+---+---+\n| a | b |\n+===+===+\n";
        assert_eq!(weight(grid), 2.0 + 30.0);
        assert_eq!(weight("===  ===\na    b\n===  ===\n"), 2.0);
    }
}
