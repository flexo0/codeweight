//! The inputs as `(path, language, text)` items: the files and folders named on the command line, a folder walked
//! with its `.gitignore` and the weights file's excludes; or a commit's changed files, both their sides' texts.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::config::{self, Files};
use crate::git::{self, Range};
use crate::lang::{self, Lang};

/// A file to weigh, its text read when it is weighed.
pub struct Item {
    /// As given, or the folder given joined with the file's path in it; without a leading `./`.
    pub path: PathBuf,
    pub lang: &'static Lang,
    /// Whether the test globs match it.
    pub test: bool,
}

impl Item {
    /// Its text, lossy where it isn't UTF-8.
    pub fn text(&self) -> Result<String> {
        let bytes =
            fs::read(&self.path).with_context(|| format!("reading {}", self.path.display()))?;
        Ok(lossy(bytes))
    }
}

/// A text, lossy where it isn't UTF-8.
fn lossy(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

/// The files to weigh, and those skipped as in no language codeweight reads, both in the order found.
pub struct Input {
    pub items: Vec<Item>,
    pub skipped: Vec<PathBuf>,
}

/// One side of a file a commit changed, its text read from git.
pub struct Blob {
    /// From the repo's root.
    pub path: PathBuf,
    pub lang: &'static Lang,
    /// Whether the test globs match it.
    pub test: bool,
    pub text: String,
}

/// A file a commit changed: its side before and after, `None` where it isn't there or isn't weighed.
pub struct Changed {
    pub before: Option<Blob>,
    pub after: Option<Blob>,
}

/// A commit's changed files to weigh (at least one side each), and those skipped as in no language codeweight
/// reads, in git's order.
pub struct CommitInput {
    pub changes: Vec<Changed>,
    pub skipped: Vec<PathBuf>,
}

/// The files `range` changed in the repo at `repo`, as a walk of the repo's root would weigh them on each side:
/// a side that is hidden (a part of its path starts with `.`), excluded, in no language codeweight reads, or a
/// test at `test_factor` 0 is left out. A change with neither side left is skipped and listed when a side was in
/// no language codeweight reads, else left out unlisted.
pub fn from_commit(repo: &Path, range: &Range, files: &Files) -> Result<CommitInput> {
    let matchers = Matchers::new(files)?;
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for change in git::changes(repo, range)? {
        let mut unsupported = None;
        let mut keep = |side: Option<git::Side>| {
            let side = side?;
            let hidden = side
                .path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
            if hidden || matchers.exclude.is_match(&side.path) {
                return None;
            }
            let Some(lang) = lang::for_path(&side.path) else {
                unsupported = Some(side.path);
                return None;
            };
            let test = matchers.tests.is_match(&side.path);
            (!test || files.test_factor != 0.0).then_some((side, lang, test))
        };
        let before = keep(change.before);
        let after = keep(change.after);
        if before.is_none() && after.is_none() {
            skipped.extend(unsupported);
            continue;
        }
        kept.push((before, after));
    }

    // How many sides use each blob (two for a pure rename's), so each is read once and moved out at its last use.
    let mut uses: HashMap<String, usize> = HashMap::new();
    for (side, _, _) in kept
        .iter()
        .flat_map(|(before, after)| [before, after])
        .flatten()
    {
        *uses.entry(side.blob.clone()).or_default() += 1;
    }
    let wanted: Vec<&str> = uses.keys().map(String::as_str).collect();
    let mut texts = git::blobs(repo, &wanted)?;
    let mut blob = |side: Option<(git::Side, &'static Lang, bool)>| -> Result<Option<Blob>> {
        let Some((side, lang, test)) = side else {
            return Ok(None);
        };
        let missing = || format!("no blob {} for {}", side.blob, side.path.display());
        let left = uses.get_mut(&side.blob).with_context(missing)?;
        *left -= 1;
        let bytes = if *left == 0 {
            texts.remove(&side.blob)
        } else {
            texts.get(&side.blob).cloned()
        }
        .with_context(missing)?;
        Ok(Some(Blob {
            path: side.path,
            lang,
            test,
            text: lossy(bytes),
        }))
    };
    let mut changes = Vec::with_capacity(kept.len());
    for (before, after) in kept {
        changes.push(Changed {
            before: blob(before)?,
            after: blob(after)?,
        });
    }
    Ok(CommitInput { changes, skipped })
}

/// A glob of the weights file's `[files]`: `*` and `?` stay within a path's part, `**` crosses parts.
pub fn glob(glob: &str) -> Result<Glob> {
    Ok(GlobBuilder::new(glob).literal_separator(true).build()?)
}

/// The files at `paths`: a file as given, a folder walked (its `.gitignore`s honoured, hidden files and the
/// excluded ones left out, in the order of the names), the test files among them. A path given twice, or found
/// under two given folders, is one item.
pub fn from_paths(paths: &[PathBuf], files: &Files) -> Result<Input> {
    let matchers = Matchers::new(files)?;
    let mut input = Input {
        items: Vec::new(),
        skipped: Vec::new(),
    };
    let mut seen = HashSet::new();
    for path in paths {
        let meta = fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
        let base = Base::of(path);
        if !meta.is_dir() {
            input.add(path, &base.key(path), &matchers, &mut seen);
            continue;
        }
        let excluded = matchers.folders.clone();
        let walk_base = base.clone();
        let walk = WalkBuilder::new(path)
            .sort_by_file_name(|a, b| a.cmp(b))
            .filter_entry(move |entry| {
                // A folder every path under which is excluded isn't walked; the folder given always is.
                entry.depth() == 0
                    || !entry.file_type().is_some_and(|t| t.is_dir())
                    || !excluded.is_match(walk_base.key(entry.path()))
            })
            .build();
        for entry in walk {
            let entry = entry.with_context(|| format!("walking {}", path.display()))?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let key = base.key(entry.path());
            if matchers.exclude.is_match(&key) {
                continue;
            }
            input.add(entry.path(), &key, &matchers, &mut seen);
        }
    }
    Ok(input)
}

impl Input {
    /// Only the items a scan of the code weighs (those the test globs don't match), or a scan of the tests (those
    /// they match, and those of a language with tests in its source files, Rust's).
    pub fn keep(&mut self, tests: bool) {
        self.items.retain(|item| match tests {
            true => item.test || item.lang.tests.is_some(),
            false => !item.test,
        });
    }

    fn add(&mut self, path: &Path, key: &Path, matchers: &Matchers, seen: &mut HashSet<PathBuf>) {
        let path = shown(path);
        if !seen.insert(path.clone()) {
            return;
        }
        let Some(lang) = lang::for_path(&path) else {
            self.skipped.push(path);
            return;
        };
        let test = matchers.tests.is_match(key);
        self.items.push(Item { path, lang, test });
    }
}

/// The `[files]` globs, compiled.
struct Matchers {
    exclude: GlobSet,
    /// The excludes that end in `/**` without it: a folder they match has every path under it excluded.
    folders: GlobSet,
    tests: GlobSet,
}

impl Matchers {
    fn new(files: &Files) -> Result<Matchers> {
        let set = |globs: &mut dyn Iterator<Item = &str>| -> Result<GlobSet> {
            let mut set = GlobSetBuilder::new();
            for g in globs {
                set.add(glob(g).with_context(|| format!("the glob `{g}`"))?);
            }
            Ok(set.build()?)
        };
        Ok(Matchers {
            exclude: set(&mut files.exclude.iter().map(String::as_str))?,
            folders: set(&mut files.exclude.iter().filter_map(|g| g.strip_suffix("/**")))?,
            tests: set(&mut files.test_globs.iter().map(String::as_str))?,
        })
    }
}

/// What the `[files]` globs are matched against for paths under a given one: the path from the root of the git
/// repo it is in; outside a repo, from the current folder when it is in it, else from the folder given (a file
/// given: its name). Never the folders above, so a project under a `build` or `tests` folder isn't excluded or
/// taken for tests.
#[derive(Clone)]
struct Base {
    /// The given path as given.
    given: PathBuf,
    /// The given path's own key.
    key: PathBuf,
}

impl Base {
    fn of(given: &Path) -> Base {
        let canonical = given.canonicalize().ok();
        let from = |root: PathBuf| {
            let path = canonical.as_deref()?;
            path.strip_prefix(root).ok().map(Path::to_path_buf)
        };
        let key = canonical
            .as_deref()
            .and_then(config::repo_root)
            .and_then(from)
            .or_else(|| {
                std::env::current_dir()
                    .ok()?
                    .canonicalize()
                    .ok()
                    .and_then(from)
            })
            .unwrap_or_else(|| {
                if given.is_dir() {
                    PathBuf::new()
                } else {
                    given.file_name().map(PathBuf::from).unwrap_or_default()
                }
            });
        Base {
            given: given.to_path_buf(),
            key,
        }
    }

    /// The path matched against the globs for `path`, the given path or one under it.
    fn key(&self, path: &Path) -> PathBuf {
        match path.strip_prefix(&self.given) {
            Ok(rest) if rest.as_os_str().is_empty() => self.key.clone(),
            Ok(rest) => self.key.join(rest),
            Err(_) => shown(path),
        }
    }
}

/// A path as the report shows it: without a leading `./`.
fn shown(path: &Path) -> PathBuf {
    let mut components = path.components();
    match components.next() {
        Some(Component::CurDir) if components.clone().next().is_some() => {
            components.as_path().to_path_buf()
        }
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(exclude: &[&str], test_globs: &[&str], test_factor: f64) -> Files {
        Files {
            exclude: exclude.iter().map(|g| g.to_string()).collect(),
            test_globs: test_globs.iter().map(|g| g.to_string()).collect(),
            test_factor,
        }
    }

    fn write(root: &Path, files: &[&str]) {
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
    }

    fn found(input: &Input, root: &Path) -> (Vec<String>, Vec<String>) {
        let relative = |p: &Path| p.strip_prefix(root).unwrap().display().to_string();
        (
            input
                .items
                .iter()
                .map(|i| {
                    format!(
                        "{}{}",
                        relative(&i.path),
                        if i.test { " (test)" } else { "" }
                    )
                })
                .collect(),
            input.skipped.iter().map(|p| relative(p)).collect(),
        )
    }

    #[test]
    fn a_folder_is_walked_in_name_order_its_excludes_and_gitignore_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        write(
            root,
            &[
                "src/b.py",
                "src/a/Z.java",
                "src/c.ts",
                "src/c.d.ts",
                "src/view.tsx",
                "src/README.md",
                "src/node_modules/x/y.ts",
                "src/gen/g.ts",
                "ignored/i.py",
                ".hidden/h.py",
                "tests/test_b.py",
                "tests/helper.py",
            ],
        );
        let input = from_paths(
            &[root.to_path_buf()],
            &files(
                &["**/node_modules/**", "**/*.d.ts", "src/gen/*"],
                &["**/tests/**"],
                1.0,
            ),
        )
        .unwrap();
        assert_eq!(
            found(&input, root),
            (
                vec![
                    "src/a/Z.java".to_string(),
                    "src/b.py".to_string(),
                    "src/c.ts".to_string(),
                    "src/view.tsx".to_string(),
                    "tests/helper.py (test)".to_string(),
                    "tests/test_b.py (test)".to_string(),
                ],
                vec!["src/README.md".to_string()]
            )
        );
    }

    #[test]
    fn the_globs_match_the_path_from_the_repo_s_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        write(root, &["src/gen/g.ts", "src/keep.ts", "src/test/t.ts"]);
        // `src/gen/**` names a folder from the root, though the folder walked is `src`.
        let input = from_paths(
            &[root.join("src"), root.join("src/test/t.ts")],
            &files(&["src/gen/**"], &["src/test/**"], 1.0),
        )
        .unwrap();
        assert_eq!(
            found(&input, root).0,
            vec![
                "src/keep.ts".to_string(),
                "src/test/t.ts (test)".to_string()
            ]
        );
    }

    #[test]
    fn outside_a_repo_the_globs_never_see_the_folders_above() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("build/tests/proj");
        write(&root, &["a.py", "tests/b.py"]);
        let input = from_paths(
            &[root.clone(), root.join("a.py")],
            &files(&["**/build/**"], &["**/tests/**"], 1.0),
        )
        .unwrap();
        assert_eq!(
            found(&input, &root).0,
            vec!["a.py".to_string(), "tests/b.py (test)".to_string()]
        );
    }

    #[test]
    fn a_scan_keeps_the_code_or_the_tests_and_the_files_with_tests_inside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["a.py", "b.rs", "test_a.py"]);
        let scan = |tests| {
            let mut input =
                from_paths(&[root.to_path_buf()], &files(&[], &["**/test_*.py"], 1.0)).unwrap();
            input.keep(tests);
            found(&input, root).0
        };
        assert_eq!(scan(false), vec!["a.py".to_string(), "b.rs".to_string()]);
        assert_eq!(
            scan(true),
            vec!["b.rs".to_string(), "test_a.py (test)".to_string()]
        );
    }

    #[test]
    fn a_file_given_is_weighed_though_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["gen/a.ts"]);
        let file = root.join("gen/a.ts");
        let input = from_paths(&[file.clone(), file], &files(&["**/gen/**"], &[], 1.0)).unwrap();
        assert_eq!(found(&input, root).0, vec!["gen/a.ts".to_string()]);
    }

    #[test]
    fn a_leading_dot_is_dropped() {
        assert_eq!(shown(Path::new("./src/a.ts")), Path::new("src/a.ts"));
        assert_eq!(shown(Path::new(".")), Path::new("."));
        assert_eq!(shown(Path::new("../a.ts")), Path::new("../a.ts"));
    }
}
