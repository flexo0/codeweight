//! The reports of a scan (the weights) and of a commit (the deltas), as text or JSON: the code's, the docs' and the
//! tests' numbers; or one of them, its total, then the files or the functions and classes, then the warnings, or
//! the total alone.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write;
use std::iter::Sum;
use std::ops::{Add, Sub};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::engine::Weighed;
use crate::lang::ScopeKind;
use crate::source::Unreadable;

/// What the report lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    File,
    Function,
}

/// A weighed file.
pub struct Weighted {
    pub path: PathBuf,
    pub weighed: Weighed,
}

/// A file a commit changed, weighed on each side it is there.
pub struct Changed {
    /// The path after (before, for a deleted file).
    pub path: PathBuf,
    /// The path before, when it was renamed.
    pub old_path: Option<PathBuf>,
    pub before: Option<Weighed>,
    pub after: Option<Weighed>,
}

/// What a commit's revisions were, as full hashes; `from` is `None` for the empty tree, `to` for the uncommitted
/// changes.
pub struct Revisions<'a> {
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
}

/// The three numbers of a file, a scan or a delta: the code's (its tests aside), the docs' and the tests'.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Parts {
    pub code: f64,
    pub docs: f64,
    pub tests: f64,
}

impl Add for Parts {
    type Output = Parts;

    fn add(self, other: Parts) -> Parts {
        Parts {
            code: self.code + other.code,
            docs: self.docs + other.docs,
            tests: self.tests + other.tests,
        }
    }
}

impl Sub for Parts {
    type Output = Parts;

    fn sub(self, other: Parts) -> Parts {
        Parts {
            code: self.code - other.code,
            docs: self.docs - other.docs,
            tests: self.tests - other.tests,
        }
    }
}

impl Sum for Parts {
    fn sum<I: Iterator<Item = Parts>>(parts: I) -> Parts {
        parts.fold(Parts::default(), Add::add)
    }
}

/// The three numbers as text, a line each in the order code, docs, tests, each signed (`code +15.5`, `docs 0`).
pub fn parts_text(parts: Parts) -> String {
    format!(
        "code {}\ndocs {}\ntests {}\n",
        signed(parts.code),
        signed(parts.docs),
        signed(parts.tests)
    )
}

/// The three numbers on one line, each rounded to a whole number and signed: `c+15;d0;t-5`.
pub fn parts_short(parts: Parts) -> String {
    // Rounded from the two decimals the text shows, so the two agree (`0.5` is `+1`).
    let whole = |part: f64| match rounded(part).round() {
        0.0 => "0".to_string(),
        n if n > 0.0 => format!("+{n}"),
        n => format!("{n}"),
    };
    format!(
        "c{};d{};t{}\n",
        whole(parts.code),
        whole(parts.docs),
        whole(parts.tests)
    )
}

/// A scan's three numbers as JSON, with its warnings (the fields are described in the README).
pub fn scan_parts_json(parts: Parts, files: &[Weighted], unreadable: &[Unreadable]) -> String {
    json(&PartsReport {
        from: None,
        to: None,
        code: rounded(parts.code),
        docs: rounded(parts.docs),
        tests: rounded(parts.tests),
        warnings: scan_json_warnings(files, unreadable),
    })
}

/// A commit's three deltas as JSON, with its revisions and warnings (the fields are described in the README).
pub fn commit_parts_json(parts: Parts, changes: &[Changed], revisions: &Revisions) -> String {
    json(&PartsReport {
        from: Some(revisions.from),
        to: Some(revisions.to),
        code: rounded(parts.code),
        docs: rounded(parts.docs),
        tests: rounded(parts.tests),
        warnings: commit_json_warnings(changes),
    })
}

/// What a row is: a function, a class (its code outside its functions), or a file's code outside any class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Function,
    Class,
    File,
}

/// A line of a report: a file (`--by file`, no kind) or an entry of one.
struct Row {
    path: String,
    old_path: Option<String>,
    kind: Option<Kind>,
    name: Option<String>,
    line: Option<usize>,
    /// Whether it is there on each side (a scan: after only).
    was: bool,
    is: bool,
    before: f64,
    after: f64,
}

impl Row {
    fn delta(&self) -> f64 {
        self.after - self.before
    }

    /// Its name as the text shows it, and sorts by.
    fn label(&self) -> String {
        match (self.kind, &self.name) {
            (Some(Kind::Class), Some(name)) => format!("{name} (class)"),
            (Some(Kind::File), _) => "(file)".to_string(),
            (_, Some(name)) => name.clone(),
            (_, None) => String::new(),
        }
    }

    fn status(&self) -> &'static str {
        match (self.was, self.is) {
            (false, _) => "added",
            (_, false) => "removed",
            _ if self.delta() > 0.0 => "heavier",
            _ => "lighter",
        }
    }
}

/// The rows of one file, its sides' weights: the file itself, or its file code and its entries.
fn rows_of(
    path: &Path,
    old_path: Option<&Path>,
    before: Option<&Weighed>,
    after: Option<&Weighed>,
    by: By,
) -> Vec<Row> {
    let path = path.display().to_string();
    let old_path = old_path.map(|p| p.display().to_string());
    let row = |kind, name, line, was, is, before, after| Row {
        path: path.clone(),
        old_path: old_path.clone(),
        kind,
        name,
        line,
        was,
        is,
        before,
        after,
    };
    let (was, is) = (before.is_some(), after.is_some());
    let total = |side: Option<&Weighed>, part: fn(&Weighed) -> f64| side.map_or(0.0, part);
    if by == By::File {
        let whole = |w: &Weighed| w.total;
        return vec![row(
            None,
            None,
            None,
            was,
            is,
            total(before, whole),
            total(after, whole),
        )];
    }
    let file = |w: &Weighed| w.outside();
    let mut rows = vec![row(
        Some(Kind::File),
        None,
        None,
        was,
        is,
        total(before, file),
        total(after, file),
    )];
    // Each entry of either side, matched by its kind and qualified name; in the after side's order, then the
    // removed ones in the before side's.
    let old_entries = before.map_or(&[][..], |w| &w.entries[..]);
    let new_entries = after.map_or(&[][..], |w| &w.entries[..]);
    let old: HashMap<(ScopeKind, &str), usize> = old_entries
        .iter()
        .enumerate()
        .map(|(at, e)| ((e.kind, e.name.as_str()), at))
        .collect();
    let mut matched = vec![false; old.len()];
    let kind = |kind| match kind {
        ScopeKind::Function => Kind::Function,
        ScopeKind::Class => Kind::Class,
    };
    for entry in new_entries {
        let was = old.get(&(entry.kind, entry.name.as_str())).map(|&at| {
            matched[at] = true;
            old_entries[at].weight
        });
        rows.push(row(
            Some(kind(entry.kind)),
            Some(entry.name.clone()),
            Some(entry.line),
            was.is_some(),
            true,
            was.unwrap_or(0.0),
            entry.weight,
        ));
    }
    for (entry, _) in old_entries
        .iter()
        .zip(&matched)
        .filter(|(_, matched)| !**matched)
    {
        rows.push(row(
            Some(kind(entry.kind)),
            Some(entry.name.clone()),
            Some(entry.line),
            true,
            false,
            entry.weight,
            0.0,
        ));
    }
    rows
}

/// The rows of a report, those that weigh (or change) nothing left out, sorted by `key` as shown (the heaviest
/// first; two weights a float's hair apart that print the same are equal), then by path and name, the first `top`
/// kept.
fn sorted(mut rows: Vec<Row>, key: fn(&Row) -> f64, top: Option<usize>) -> Vec<Row> {
    rows.retain(|row| rounded(key(row)) != 0.0);
    let mut keyed: Vec<(Row, String)> = rows
        .into_iter()
        .map(|row| {
            let label = row.label();
            (row, label)
        })
        .collect();
    keyed.sort_by(|(a, a_label), (b, b_label)| {
        let (ka, kb) = (rounded(key(a)), rounded(key(b)));
        kb.abs()
            .partial_cmp(&ka.abs())
            .unwrap_or(Ordering::Equal)
            // A gain before a loss of the same size.
            .then_with(|| kb.partial_cmp(&ka).unwrap_or(Ordering::Equal))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a_label.cmp(b_label))
    });
    keyed.truncate(top.unwrap_or(keyed.len()));
    keyed.into_iter().map(|(row, _)| row).collect()
}

/// A scan's report as text: the total, then a line per row (`<weight>  <path>  <name>`), then the warnings (syntax
/// errors, unreadable paths).
pub fn scan_text(
    files: &[Weighted],
    unreadable: &[Unreadable],
    by: By,
    top: Option<usize>,
) -> String {
    let rows = scan_rows(files, by, top);
    let weights: Vec<String> = rows.iter().map(|r| number(r.after)).collect();
    let width = weights.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::new();
    let _ = writeln!(out, "total {}", number(scan_total(files)));
    for (row, weight) in rows.iter().zip(&weights) {
        let line = format!("{weight:>width$}  {}  {}", row.path, row.label());
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out.push_str(&scan_warnings(files, unreadable));
    out
}

/// A scan's total alone, a number on its line.
pub fn scan_number(files: &[Weighted]) -> String {
    format!("{}\n", number(scan_total(files)))
}

/// A scan's warnings (syntax errors, then the paths that couldn't be read), a line each.
pub fn scan_warnings(files: &[Weighted], unreadable: &[Unreadable]) -> String {
    let mut out = String::new();
    for file in files.iter().filter(|f| !f.weighed.clean) {
        let _ = writeln!(
            out,
            "warning: {} has syntax errors, weighed as tree-sitter recovered it",
            file.path.display()
        );
    }
    for path in unreadable {
        let _ = writeln!(
            out,
            "warning: {} can't be read ({}), left out",
            path.path.display(),
            path.reason
        );
    }
    out
}

/// A scan's report as JSON (its fields are described in the README).
pub fn scan_json(
    files: &[Weighted],
    unreadable: &[Unreadable],
    by: By,
    top: Option<usize>,
) -> String {
    let rows = scan_rows(files, by, top)
        .into_iter()
        .map(|row| match row.kind {
            None => JsonRow::ScanFile {
                path: row.path,
                weight: rounded(row.after),
            },
            Some(kind) => JsonRow::ScanEntry {
                path: row.path,
                kind,
                name: row.name,
                line: row.line,
                weight: rounded(row.after),
            },
        })
        .collect();
    json(&Report {
        from: None,
        to: None,
        total: rounded(scan_total(files)),
        by,
        rows,
        warnings: scan_json_warnings(files, unreadable),
    })
}

/// A scan's warnings as JSON: syntax errors, then the paths that couldn't be read.
fn scan_json_warnings(files: &[Weighted], unreadable: &[Unreadable]) -> Vec<Warning> {
    files
        .iter()
        .filter(|f| !f.weighed.clean)
        .map(|f| Warning::syntax(&f.path, None))
        .chain(unreadable.iter().map(|u| Warning::unreadable(&u.path)))
        .collect()
}

fn scan_total(files: &[Weighted]) -> f64 {
    files.iter().map(|f| f.weighed.total).sum()
}

fn scan_rows(files: &[Weighted], by: By, top: Option<usize>) -> Vec<Row> {
    let rows = files
        .iter()
        .flat_map(|f| rows_of(&f.path, None, None, Some(&f.weighed), by))
        .collect();
    sorted(rows, |row| row.after, top)
}

/// A commit's report as text: the total delta, then a line per row (`<delta>  <status>  <path>  <name>`), the
/// largest changes first, then the warnings.
pub fn commit_text(changes: &[Changed], by: By, top: Option<usize>) -> String {
    let rows = commit_rows(changes, by, top);
    let deltas: Vec<String> = rows.iter().map(|r| signed(r.delta())).collect();
    let width = deltas.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::new();
    let _ = writeln!(out, "total {}", signed(commit_total(changes)));
    for (row, delta) in rows.iter().zip(&deltas) {
        let path = match &row.old_path {
            Some(old) => format!("{old} -> {}", row.path),
            None => row.path.clone(),
        };
        let line = format!(
            "{delta:>width$}  {:<7}  {path}  {}",
            row.status(),
            row.label()
        );
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out.push_str(&commit_warnings_text(changes));
    out
}

/// A commit's total delta alone, a signed number on its line.
pub fn commit_number(changes: &[Changed]) -> String {
    format!("{}\n", signed(commit_total(changes)))
}

/// A commit's warnings (syntax errors on a side), a line each.
pub fn commit_warnings_text(changes: &[Changed]) -> String {
    let mut out = String::new();
    for (change, side, _) in commit_warnings(changes) {
        let _ = writeln!(
            out,
            "warning: {} has syntax errors {side}, weighed as tree-sitter recovered it",
            change.path.display()
        );
    }
    out
}

/// A commit's report as JSON (its fields are described in the README).
pub fn commit_json(
    changes: &[Changed],
    revisions: &Revisions,
    by: By,
    top: Option<usize>,
) -> String {
    let rows = commit_rows(changes, by, top)
        .into_iter()
        .map(|row| {
            let status = row.status();
            let (before, after, delta) = (
                rounded(row.before),
                rounded(row.after),
                rounded(row.delta()),
            );
            match row.kind {
                None => JsonRow::CommitFile {
                    path: row.path,
                    old_path: row.old_path,
                    before,
                    after,
                    delta,
                    status,
                },
                Some(kind) => JsonRow::CommitEntry {
                    path: row.path,
                    old_path: row.old_path,
                    kind,
                    name: row.name,
                    line: row.line,
                    before,
                    after,
                    delta,
                    status,
                },
            }
        })
        .collect();
    json(&Report {
        from: Some(revisions.from),
        to: Some(revisions.to),
        total: rounded(commit_total(changes)),
        by,
        rows,
        warnings: commit_json_warnings(changes),
    })
}

/// A commit's warnings as JSON: the sides with syntax errors.
fn commit_json_warnings(changes: &[Changed]) -> Vec<Warning> {
    commit_warnings(changes)
        .map(|(change, _, side)| Warning::syntax(&change.path, Some(side)))
        .collect()
}

fn commit_total(changes: &[Changed]) -> f64 {
    let total = |side: &Option<Weighed>| side.as_ref().map_or(0.0, |w| w.total);
    changes
        .iter()
        .map(|c| total(&c.after) - total(&c.before))
        .sum()
}

fn commit_rows(changes: &[Changed], by: By, top: Option<usize>) -> Vec<Row> {
    let rows = changes
        .iter()
        .flat_map(|c| {
            rows_of(
                &c.path,
                c.old_path.as_deref(),
                c.before.as_ref(),
                c.after.as_ref(),
                by,
            )
        })
        .collect();
    sorted(rows, Row::delta, top)
}

/// The sides with syntax errors: the change, the side as the text says it, the side as the JSON does.
fn commit_warnings(
    changes: &[Changed],
) -> impl Iterator<Item = (&Changed, &'static str, &'static str)> {
    changes.iter().flat_map(|change| {
        let dirty = |side: &Option<Weighed>| side.as_ref().is_some_and(|w| !w.clean);
        [
            (dirty(&change.before), "before the change", "before"),
            (dirty(&change.after), "after the change", "after"),
        ]
        .into_iter()
        .filter(|(dirty, _, _)| *dirty)
        .map(move |(_, text, json)| (change, text, json))
    })
}

/// The JSON report: a scan's has no `from` and `to`.
#[derive(Serialize)]
struct Report<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<Option<&'a str>>,
    total: f64,
    by: By,
    rows: Vec<JsonRow>,
    warnings: Vec<Warning>,
}

/// The JSON of the three numbers: a scan's has no `from` and `to`.
#[derive(Serialize)]
struct PartsReport<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<Option<&'a str>>,
    code: f64,
    docs: f64,
    tests: f64,
    warnings: Vec<Warning>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum JsonRow {
    ScanFile {
        path: String,
        weight: f64,
    },
    ScanEntry {
        path: String,
        kind: Kind,
        name: Option<String>,
        line: Option<usize>,
        weight: f64,
    },
    CommitFile {
        path: String,
        old_path: Option<String>,
        before: f64,
        after: f64,
        delta: f64,
        status: &'static str,
    },
    CommitEntry {
        path: String,
        old_path: Option<String>,
        kind: Kind,
        name: Option<String>,
        line: Option<usize>,
        before: f64,
        after: f64,
        delta: f64,
        status: &'static str,
    },
}

#[derive(Serialize)]
struct Warning {
    path: String,
    kind: &'static str,
    /// A commit's side, `before` or `after`; none for a scan.
    side: Option<&'static str>,
}

impl Warning {
    fn syntax(path: &Path, side: Option<&'static str>) -> Warning {
        Warning {
            path: path.display().to_string(),
            kind: "syntax_errors",
            side,
        }
    }

    fn unreadable(path: &Path) -> Warning {
        Warning {
            path: path.display().to_string(),
            kind: "unreadable",
            side: None,
        }
    }
}

fn json(report: &impl Serialize) -> String {
    // A report of plain strings and numbers always serializes.
    let mut text = serde_json::to_string_pretty(report).unwrap_or_default();
    text.push('\n');
    text
}

/// A weight rounded to the two decimals the reports show.
fn rounded(weight: f64) -> f64 {
    let rounded = (weight * 100.0).round() / 100.0;
    // No `-0`.
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// A weight as shown: at most two decimals, no trailing zeros.
pub fn number(weight: f64) -> String {
    let text = format!("{weight:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// A delta as shown: `+` before a gain.
fn signed(delta: f64) -> String {
    let text = number(delta);
    if rounded(delta) > 0.0 {
        format!("+{text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Entry;

    #[test]
    fn numbers_drop_trailing_zeros() {
        assert_eq!(number(42.0), "42");
        assert_eq!(number(42.5), "42.5");
        assert_eq!(number(1.0 / 3.0), "0.33");
        assert_eq!(number(-17.0), "-17");
        assert_eq!(number(0.0), "0");
        assert_eq!(signed(42.5), "+42.5");
        assert_eq!(signed(-17.0), "-17");
        assert_eq!(signed(0.001), "0");
        assert_eq!(rounded(-0.001), 0.0);
        assert!(rounded(-0.001).is_sign_positive());
    }

    #[test]
    fn the_three_numbers_are_signed_the_short_ones_whole() {
        let parts = Parts {
            code: 15.5,
            docs: 0.001,
            tests: -5.0,
        };
        assert_eq!(parts_text(parts), "code +15.5\ndocs 0\ntests -5\n");
        assert_eq!(parts_short(parts), "c+16;d0;t-5\n");
        // Whole from the two decimals the text shows: 0.499 shows as 0.5, so +1; -0.4 is 0, no sign.
        let small = Parts {
            code: 0.499,
            docs: -0.4,
            tests: -2.5,
        };
        assert_eq!(parts_short(small), "c+1;d0;t-3\n");
    }

    #[test]
    fn weights_that_print_the_same_sort_by_path() {
        let row = |path: &str, after: f64| Row {
            path: path.into(),
            old_path: None,
            kind: None,
            name: None,
            line: None,
            was: false,
            is: true,
            before: 0.0,
            after,
        };
        // 0.1 + 0.2 is a float's hair above 0.3; both show as 0.3.
        let rows = sorted(
            vec![row("b.py", 0.1 + 0.2), row("a.py", 0.3)],
            |r| r.after,
            None,
        );
        let paths: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["a.py", "b.py"]);
    }

    fn weighed(file: f64, entries: &[(ScopeKind, &str, f64)]) -> Weighed {
        let entries: Vec<Entry> = entries
            .iter()
            .enumerate()
            .map(|(line, (kind, name, weight))| Entry {
                kind: *kind,
                name: name.to_string(),
                line: line + 1,
                weight: *weight,
                test: false,
            })
            .collect();
        Weighed {
            total: file + entries.iter().map(|e| e.weight).sum::<f64>(),
            file,
            file_tests: 0.0,
            entries,
            clean: true,
        }
    }

    fn changes() -> Vec<Changed> {
        use ScopeKind::{Class, Function};
        vec![
            Changed {
                path: "a.py".into(),
                old_path: Some("old.py".into()),
                before: Some(weighed(
                    1.0,
                    &[
                        (Function, "f", 5.0),
                        (Function, "g", 4.0),
                        (Class, "C", 2.0),
                    ],
                )),
                after: Some(weighed(
                    1.0,
                    &[
                        (Function, "f", 7.0),
                        (Class, "C", 2.0),
                        (Function, "h", 4.0),
                    ],
                )),
            },
            Changed {
                path: "gone.py".into(),
                old_path: None,
                before: Some(weighed(2.0, &[(Function, "x", 3.0)])),
                after: None,
            },
        ]
    }

    #[test]
    fn a_commit_s_entries_are_matched_by_name_the_largest_change_first() {
        // f +2; g removed -4, h added +4 (a gain before a loss of the same size); gone.py's x -3 and file code -2.
        assert_eq!(
            commit_text(&changes(), By::Function, None),
            "total -3\n\
             +4  added    old.py -> a.py  h\n\
             -4  removed  old.py -> a.py  g\n\
             -3  removed  gone.py  x\n\
             +2  heavier  old.py -> a.py  f\n\
             -2  removed  gone.py  (file)\n"
        );
        assert_eq!(
            commit_text(&changes(), By::File, Some(1)),
            "total -3\n-5  removed  gone.py\n"
        );
    }

    #[test]
    fn a_commit_s_json_has_every_field() {
        let json = commit_json(
            &changes(),
            &Revisions {
                from: None,
                to: Some("abc"),
            },
            By::Function,
            Some(1),
        );
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "from": null,
                "to": "abc",
                "total": -3.0,
                "by": "function",
                "rows": [{
                    "path": "a.py", "old_path": "old.py", "kind": "function", "name": "h", "line": 3,
                    "before": 0.0, "after": 4.0, "delta": 4.0, "status": "added"
                }],
                "warnings": []
            })
        );
    }

    #[test]
    fn a_scan_s_json_mirrors_its_text() {
        let files = vec![Weighted {
            path: "a.py".into(),
            weighed: Weighed {
                clean: false,
                ..weighed(1.0 / 3.0, &[(ScopeKind::Class, "C", 2.0)])
            },
        }];
        let unreadable = [Unreadable {
            path: "data".into(),
            reason: "permission denied".into(),
        }];
        assert_eq!(
            scan_warnings(&files, &unreadable),
            "warning: a.py has syntax errors, weighed as tree-sitter recovered it\n\
             warning: data can't be read (permission denied), left out\n"
        );
        let value: serde_json::Value =
            serde_json::from_str(&scan_json(&files, &unreadable, By::Function, None)).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "total": 2.33,
                "by": "function",
                "rows": [
                    {"path": "a.py", "kind": "class", "name": "C", "line": 1, "weight": 2.0},
                    {"path": "a.py", "kind": "file", "name": null, "line": null, "weight": 0.33}
                ],
                "warnings": [
                    {"path": "a.py", "kind": "syntax_errors", "side": null},
                    {"path": "data", "kind": "unreadable", "side": null}
                ]
            })
        );
        let by_file: serde_json::Value =
            serde_json::from_str(&scan_json(&files, &[], By::File, None)).unwrap();
        assert_eq!(
            by_file["rows"],
            serde_json::json!([{"path": "a.py", "weight": 2.33}])
        );
    }
}
