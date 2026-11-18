//! The text report of a scan. (The JSON output and the commit's deltas come with milestones 5 and 6.)

use std::cmp::Ordering;
use std::fmt::Write;
use std::path::PathBuf;

use crate::engine::Weighed;
use crate::lang::ScopeKind;

/// What the report lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum By {
    File,
    Function,
}

/// A weighed file.
pub struct Weighted {
    pub path: PathBuf,
    pub weighed: Weighed,
}

/// The report: the total, then the files or the functions and classes (with each file's code outside them),
/// heaviest first, then by path and name; `top` keeps the first lines; then the warnings and the skipped files.
pub fn text(files: &[Weighted], skipped: &[PathBuf], by: By, top: Option<usize>) -> String {
    let total: f64 = files.iter().map(|f| f.weighed.total).sum();
    let mut rows: Vec<(f64, String, String)> = Vec::new();
    for file in files {
        let path = file.path.display().to_string();
        match by {
            By::File => rows.push((file.weighed.total, path, String::new())),
            By::Function => {
                if file.weighed.file != 0.0 {
                    rows.push((file.weighed.file, path.clone(), "(file)".to_string()));
                }
                for entry in &file.weighed.entries {
                    if entry.weight == 0.0 {
                        continue;
                    }
                    let name = match entry.kind {
                        ScopeKind::Function => entry.name.clone(),
                        ScopeKind::Class => format!("{} (class)", entry.name),
                    };
                    rows.push((entry.weight, path.clone(), name));
                }
            }
        }
    }
    rows.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    rows.truncate(top.unwrap_or(rows.len()));

    let weights: Vec<String> = rows.iter().map(|r| number(r.0)).collect();
    let width = weights.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::new();
    let _ = writeln!(out, "total {}", number(total));
    for (row, weight) in rows.iter().zip(&weights) {
        let line = format!("{weight:>width$}  {}  {}", row.1, row.2);
        let _ = writeln!(out, "{}", line.trim_end());
    }
    for file in files.iter().filter(|f| !f.weighed.clean) {
        let _ = writeln!(
            out,
            "warning: {} has syntax errors, weighed as tree-sitter recovered it",
            file.path.display()
        );
    }
    for path in skipped {
        let _ = writeln!(
            out,
            "skipped: {} (not a language codeweight reads)",
            path.display()
        );
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_drop_trailing_zeros() {
        assert_eq!(number(42.0), "42");
        assert_eq!(number(42.5), "42.5");
        assert_eq!(number(1.0 / 3.0), "0.33");
        assert_eq!(number(-17.0), "-17");
        assert_eq!(number(0.0), "0");
    }
}
