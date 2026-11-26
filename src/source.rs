//! The inputs as `(path, kind, text)` items, a kind a language's code or docs (Markdown, reST, AsciiDoc): the files and
//! folders named on the command line, a folder walked with its `.gitignore` and the excludes (the weights file's,
//! `--exclude`'s); or a commit's changed files, both their sides' texts.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{Match, WalkBuilder};

use crate::config::{self, Files};
use crate::git::{self, Range};
use crate::lang::{self, Lang};
use crate::markdown::Format;

/// What is weighed: the code (its tests aside), the tests, the docs, or the code's comments; each its own number,
/// never summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Code,
    Tests,
    Docs,
    Comments,
}

/// What a file is weighed as: a language's code, or docs in their format for their docs weight.
#[derive(Clone, Copy)]
pub enum Kind {
    Code(&'static Lang),
    Docs(Format),
}

/// A file to weigh, its text read when it is weighed.
pub struct Item {
    /// As given, or the folder given joined with the file's path in it; without a leading `./`.
    pub path: PathBuf,
    pub kind: Kind,
    /// Whether the test globs match it (never a docs file).
    pub test: bool,
    /// Whether a walk of a folder given found it, not named itself on the command line.
    pub walked: bool,
}

impl Item {
    /// Its text, lossy where it isn't UTF-8. A file a walk found that can't be read (its mode, or gone since the
    /// walk) is unreadable, left out with a warning as the walk's own unreadable paths are; a file given that
    /// can't be read is an error.
    pub fn text(&self) -> Result<Result<String, Unreadable>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Ok(lossy(bytes))),
            Err(error) if self.walked => Ok(Err(Unreadable {
                path: self.path.clone(),
                reason: error.kind().to_string(),
            })),
            Err(error) => Err(error).with_context(|| format!("reading {}", self.path.display())),
        }
    }
}

/// A text, lossy where it isn't UTF-8.
fn lossy(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

/// The files to weigh and the paths under a folder given that the walk couldn't read, all in the order found; a
/// file in no language codeweight reads is left out.
pub struct Input {
    pub items: Vec<Item>,
    pub unreadable: Vec<Unreadable>,
}

/// A path under a folder given that couldn't be read (a folder the walk may not list, most often, or a file read
/// when weighed), left out with a warning so the rest is weighed.
pub struct Unreadable {
    /// As the report shows it, as an item's.
    pub path: PathBuf,
    /// Why, as the OS says it (`permission denied`).
    pub reason: String,
}

impl Unreadable {
    /// The path an error of the walk of `folder` is about (`folder` when it names none), and why.
    fn of(error: &ignore::Error, folder: &Path) -> Unreadable {
        Unreadable {
            path: shown(error_path(error).unwrap_or(folder)),
            reason: match error.io_error() {
                Some(io) => io.kind().to_string(),
                None => error.to_string(),
            },
        }
    }
}

/// The path an error of a walk is about, if it names one.
fn error_path(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::WithDepth { err, .. } => error_path(err),
        _ => None,
    }
}

/// One side of a file a commit changed, its text read from git.
pub struct Blob {
    /// From the repo's root.
    pub path: PathBuf,
    pub kind: Kind,
    /// Whether the test globs match it (never a docs file).
    pub test: bool,
    pub text: String,
}

/// A file a commit changed: its side before and after, `None` where it isn't there or isn't weighed.
pub struct Changed {
    pub before: Option<Blob>,
    pub after: Option<Blob>,
}

/// The files `range` changed in the repo at `repo`, as a walk of the repo's root would weigh them on each side but
/// for the ignore files, for the code, the tests or the docs (`mode`, as a scan keeps them; `None`: all three): a
/// side that is hidden (a part of its path starts with `.`), excluded, of no kind codeweight reads, or not kept is
/// left out, and a change with neither side left; the rest (at least one side each) in git's order. A tracked file
/// that a `.gitignore` (or a `.ignore`, `.git/info/exclude`, the global gitignore) matches is kept, as git tracks
/// it, though a walk leaves it out.
pub fn from_commit(
    repo: &Path,
    range: &Range,
    files: &Files,
    mode: Option<Mode>,
) -> Result<Vec<Changed>> {
    let matchers = Matchers::new(files)?;
    let mut kept = Vec::new();
    for change in git::changes(repo, range)? {
        let keep = |side: Option<git::Side>| {
            let side = side?;
            let hidden = side
                .path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
            if hidden || matchers.excluded(&side.path, false, Path::new("")) {
                return None;
            }
            let (kind, test) = matchers.kind(&side.path, &side.path)?;
            mode.is_none_or(|mode| weighed_for(test, kind, mode))
                .then_some((side, kind, test))
        };
        let before = keep(change.before);
        let after = keep(change.after);
        if before.is_some() || after.is_some() {
            kept.push((before, after));
        }
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
    let mut blob = |side: Option<(git::Side, Kind, bool)>| -> Result<Option<Blob>> {
        let Some((side, kind, test)) = side else {
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
            kind,
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
    Ok(changes)
}

/// A glob of the weights file's `[files]`: `*` and `?` stay within a path's part, `**` crosses parts.
pub fn glob(glob: &str) -> Result<Glob> {
    Ok(GlobBuilder::new(glob).literal_separator(true).build()?)
}

/// An exclude (`[files] exclude` or `--exclude`), a line of a `.gitignore`, checked: it must name a path, so
/// neither a blank, a comment, a bare `/` or `!` nor an empty part (`a//b`).
pub fn exclude(pattern: &str) -> Result<()> {
    excludes(std::slice::from_ref(&pattern.to_string())).map(|_| ())
}

/// The excludes, in order, as one `.gitignore` matched from the globs' base: a later line overrides an earlier
/// one, so `!` takes a path back.
fn excludes(patterns: &[String]) -> Result<Gitignore> {
    let mut builder = GitignoreBuilder::new("");
    builder.allow_unclosed_class(false);
    for pattern in patterns {
        let name = pattern.trim().trim_start_matches('!').trim_matches('/');
        if name.is_empty() || pattern.trim_start().starts_with('#') || pattern.contains("//") {
            bail!(
                "an exclude names a path, as a `.gitignore` line: `build/`, `*.pb.go`, `/src/gen/`"
            );
        }
        builder
            .add_line(None, pattern)
            .with_context(|| format!("the exclude `{pattern}`"))?;
    }
    Ok(builder.build()?)
}

/// The files at `paths`: a file as given, a folder walked (its `.gitignore`s honoured, hidden files and the
/// excluded ones left out, in the order of the names), the test files and the docs among them. A file given twice,
/// in any spelling (relative and absolute, through `..` or a symlink), or found under two given folders, is one
/// item, shown as first found. A folder under a folder given that can't be listed is left out
/// and listed as unreadable (a file there that can't be read is, when its text is read: `Item::text`); a folder
/// given that can't be read is an error.
pub fn from_paths(paths: &[PathBuf], files: &Files) -> Result<Input> {
    let matchers = Matchers::new(files)?;
    let mut input = Input {
        items: Vec::new(),
        unreadable: Vec::new(),
    };
    let mut seen = HashSet::new();
    for path in paths {
        let meta = fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
        let base = Base::of(path);
        if !meta.is_dir() {
            if seen.insert(base.identity(path)) {
                input.add(path, &base.key(path), false, &matchers);
            }
            continue;
        }
        let walk_matchers = matchers.clone();
        let walk_base = base.clone();
        let walk = WalkBuilder::new(path)
            .sort_by_file_name(|a, b| a.cmp(b))
            .filter_entry(move |entry| {
                // An excluded folder isn't walked; the folder given always is, though excluded itself.
                entry.depth() == 0
                    || !entry.file_type().is_some_and(|t| t.is_dir())
                    || !walk_matchers.excluded(&walk_base.key(entry.path()), true, &walk_base.key)
            })
            .build();
        for entry in walk {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.depth().is_some_and(|depth| depth > 0) => {
                    let unreadable = Unreadable::of(&error, path);
                    if seen.insert(base.identity(error_path(&error).unwrap_or(path))) {
                        input.unreadable.push(unreadable);
                    }
                    continue;
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("walking {}", path.display()));
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let key = base.key(entry.path());
            if matchers.excluded(&key, false, &base.key) {
                continue;
            }
            if seen.insert(base.identity(entry.path())) {
                input.add(entry.path(), &key, true, &matchers);
            }
        }
    }
    Ok(input)
}

impl Input {
    /// Only the items a scan of `mode` weighs: of the code, those the test globs don't match; of the tests, those
    /// they match and those of a language with tests in its source files (Rust's); of the docs, the docs files;
    /// of the comments, the code files the test globs don't match.
    pub fn keep(&mut self, mode: Mode) {
        self.items
            .retain(|item| weighed_for(item.test, item.kind, mode));
    }

    /// The file at `path` (its globs' path `key`), if of a kind codeweight reads.
    fn add(&mut self, path: &Path, key: &Path, walked: bool, matchers: &Matchers) {
        let path = shown(path);
        let Some((kind, test)) = matchers.kind(&path, key) else {
            return;
        };
        self.items.push(Item {
            path,
            kind,
            test,
            walked,
        });
    }
}

/// Whether a file (`test`: the test globs match it) is weighed for `mode`: a docs file is the docs' alone; of
/// the code, a test file is the tests' alone, a file of a language with tests in its source files (Rust's) both's,
/// each its part; a code file's comments are the comments', a test file's none (test code's comments weigh
/// nothing).
fn weighed_for(test: bool, kind: Kind, mode: Mode) -> bool {
    match (kind, mode) {
        (Kind::Docs(_), mode) => mode == Mode::Docs,
        (Kind::Code(_), Mode::Docs) => false,
        (Kind::Code(lang), Mode::Tests) => test || lang.tests.is_some(),
        (Kind::Code(_), Mode::Code | Mode::Comments) => !test,
    }
}

/// The `[files]` globs, compiled, the excludes as one `.gitignore`.
#[derive(Clone)]
struct Matchers {
    exclude: Gitignore,
    tests: GlobSet,
    docs: GlobSet,
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
            exclude: excludes(&files.exclude)?,
            tests: set(&mut files.test_globs.iter().map(String::as_str))?,
            docs: set(&mut files.docs_globs.iter().map(String::as_str))?,
        })
    }

    /// Whether the excludes leave out the path `key` (a folder's if `is_dir`), found under the folder `from` (the
    /// key of a folder given, empty for the root), as git ignores it: under an excluded folder below `from`, always
    /// (a `!` can't take it back, git never looks inside); else as the last line that matches it says. The folder
    /// given and those above it aren't matched, so one named on the command line is walked though excluded.
    fn excluded(&self, key: &Path, is_dir: bool, from: &Path) -> bool {
        let ignored =
            |path: &Path, is_dir| matches!(self.exclude.matched(path, is_dir), Match::Ignore(_));
        key.ancestors()
            .skip(1)
            .take_while(|folder| *folder != from && folder.starts_with(from))
            .filter(|folder| !folder.as_os_str().is_empty())
            .any(|folder| ignored(folder, true))
            || ignored(key, is_dir)
    }

    /// The kind of the file at `path` (its globs' path `key`), and whether it is a test file: a file the docs globs
    /// match is docs, in the format its name's extension says, and no test; one the test globs match too is of no kind (a test's snapshot or golden
    /// file, a README in `tests/`: test prose, in no number as test code's comments); else its language's, by
    /// `path`; `None` for a file of no kind codeweight reads.
    fn kind(&self, path: &Path, key: &Path) -> Option<(Kind, bool)> {
        let test = self.tests.is_match(key);
        if self.docs.is_match(key) {
            let format = Format::of(&path.to_string_lossy());
            return (!test).then_some((Kind::Docs(format), false));
        }
        let lang = lang::for_path(path)?;
        Some((Kind::Code(lang), test))
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
    /// The given path with no `..`, `.` or symlink in it, if it can be resolved.
    canonical: Option<PathBuf>,
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
            canonical,
        }
    }

    /// What tells the file at `path`, the given path or one under it, from another, whatever its spelling: its
    /// canonical path (a walk follows no symlink under the folder given, so the given path's canonical one joined
    /// with the rest is), else the path as shown.
    fn identity(&self, path: &Path) -> PathBuf {
        match (&self.canonical, path.strip_prefix(&self.given)) {
            (Some(canonical), Ok(rest)) if rest.as_os_str().is_empty() => canonical.clone(),
            (Some(canonical), Ok(rest)) => canonical.join(rest),
            _ => shown(path),
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

    fn strings(globs: &[&str]) -> Vec<String> {
        globs.iter().map(|g| g.to_string()).collect()
    }

    fn files(exclude: &[&str], test_globs: &[&str]) -> Files {
        Files {
            exclude: strings(exclude),
            test_globs: strings(test_globs),
            docs_globs: strings(&["**/*.md"]),
        }
    }

    fn write(root: &Path, files: &[&str]) {
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
    }

    fn found(input: &Input, root: &Path) -> Vec<String> {
        input
            .items
            .iter()
            .map(|i| {
                format!(
                    "{}{}{}",
                    i.path.strip_prefix(root).unwrap().display(),
                    if i.test { " (test)" } else { "" },
                    match i.kind {
                        Kind::Docs(Format::Markdown) => " (docs)",
                        Kind::Docs(Format::Rst) => " (rst docs)",
                        Kind::Docs(Format::AsciiDoc) => " (adoc docs)",
                        Kind::Code(_) => "",
                    }
                )
            })
            .collect()
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
            ),
        )
        .unwrap();
        assert_eq!(
            found(&input, root),
            vec![
                "src/README.md (docs)".to_string(),
                "src/a/Z.java".to_string(),
                "src/b.py".to_string(),
                "src/c.ts".to_string(),
                "src/view.tsx".to_string(),
                "tests/helper.py (test)".to_string(),
                "tests/test_b.py (test)".to_string(),
            ]
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
            &files(&["src/gen/**"], &["src/test/**"]),
        )
        .unwrap();
        assert_eq!(
            found(&input, root),
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
            &files(&["**/build/**"], &["**/tests/**"]),
        )
        .unwrap();
        assert_eq!(
            found(&input, &root),
            vec!["a.py".to_string(), "tests/b.py (test)".to_string()]
        );
    }

    #[test]
    fn a_scan_keeps_the_code_or_the_tests_and_the_files_with_tests_inside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["a.py", "b.rs", "notes.md", "test_a.py"]);
        let scan = |mode| {
            let mut input =
                from_paths(&[root.to_path_buf()], &files(&[], &["**/test_*.py"])).unwrap();
            input.keep(mode);
            found(&input, root)
        };
        assert_eq!(
            scan(Mode::Code),
            vec!["a.py".to_string(), "b.rs".to_string()]
        );
        assert_eq!(
            scan(Mode::Tests),
            vec!["b.rs".to_string(), "test_a.py (test)".to_string()]
        );
        assert_eq!(scan(Mode::Docs), vec!["notes.md (docs)".to_string()]);
    }

    #[test]
    fn a_file_the_docs_globs_match_is_docs_alone_and_under_a_test_glob_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        write(
            root,
            &[
                "README.md",
                "a.py",
                "build/b.md",
                ".hidden/h.md",
                "docs/guide.markdown",
                "tests/README.md",
                "src/__tests__/a.expect.md",
                "tests/test_a.py",
            ],
        );
        let mut globs = files(&["**/build/**"], &["**/tests/**", "**/__tests__/**"]);
        globs.docs_globs = strings(&["**/*.md", "**/*.markdown"]);
        let scan = |globs: &Files, mode| {
            let mut input = from_paths(&[root.to_path_buf()], globs).unwrap();
            input.keep(mode);
            found(&input, root)
        };
        // The Markdown files under a test glob are in no number, neither the docs' nor the tests'.
        assert_eq!(scan(&globs, Mode::Code), vec!["a.py".to_string()]);
        assert_eq!(
            scan(&globs, Mode::Tests),
            vec!["tests/test_a.py (test)".to_string()]
        );
        assert_eq!(
            scan(&globs, Mode::Docs),
            vec![
                "README.md (docs)".to_string(),
                "docs/guide.markdown (docs)".to_string(),
            ]
        );
        // Named on the command line, one is no more weighed.
        let given = from_paths(&[root.join("tests/README.md")], &globs).unwrap();
        assert!(given.items.is_empty());

        // Overridden: a Markdown file they don't match is read by no kind; one they match is docs, whatever its
        // extension, Markdown but a `.rst`'s, a `.rest`'s or a `.txt`'s, and an `.adoc`'s or an `.asciidoc`'s.
        write(root, &["docs/index.rst", "docs/a.txt", "docs/b.adoc"]);
        globs.docs_globs = strings(&["docs/**", "a.py"]);
        assert!(scan(&globs, Mode::Code).is_empty());
        assert_eq!(
            scan(&globs, Mode::Tests),
            vec!["tests/test_a.py (test)".to_string()]
        );
        assert_eq!(
            scan(&globs, Mode::Docs),
            vec![
                "a.py (docs)".to_string(),
                "docs/a.txt (rst docs)".to_string(),
                "docs/b.adoc (adoc docs)".to_string(),
                "docs/guide.markdown (docs)".to_string(),
                "docs/index.rst (rst docs)".to_string(),
            ]
        );
    }

    #[test]
    fn the_default_docs_globs_take_markdown_rest_asciidoc_and_a_docs_folder_s_text_files_by_their_format()
     {
        let matchers = Matchers::new(&config::Config::defaults().unwrap().files).unwrap();
        let format = |path: &str| match matchers.kind(Path::new(path), Path::new(path)) {
            Some((Kind::Docs(format), false)) => Some(format),
            _ => None,
        };
        for path in ["README.md", "docs/a/guide.markdown"] {
            assert_eq!(format(path), Some(Format::Markdown), "{path}");
        }
        for path in [
            "README.rst",
            "docs/index.rst",
            "docs/topics/urls.txt",
            "a/docs/b.txt",
            "Documentation/devicetree/bindings/a.txt",
        ] {
            assert_eq!(format(path), Some(Format::Rst), "{path}");
        }
        for path in [
            "Documentation/git-add.adoc",
            "docs/src/main/asciidoc/a.asciidoc",
        ] {
            assert_eq!(format(path), Some(Format::AsciiDoc), "{path}");
        }
        // A text file elsewhere is no docs; one under a test glob is none either.
        for path in ["requirements.txt", "src/CMakeLists.txt", "tests/docs/a.txt"] {
            assert_eq!(format(path), None, "{path}");
        }
    }

    #[test]
    fn the_default_test_globs_take_the_common_test_layouts_and_leave_t_and_testing_code() {
        let matchers = Matchers::new(&config::Config::defaults().unwrap().files).unwrap();
        let test = |path: &str| matchers.kind(Path::new(path), Path::new(path)).unwrap().1;
        for path in [
            "src/test/java/a/ATest.java",
            "pkg/a_test.go",
            "integration-tests/rest/src/main/java/a/A.java",
            "crates/x/integration_tests/a.rs",
            "tools/testing/selftests/bpf/a.c",
            "drivers/gpu/drm/i915/selftests/a.c",
            "testsuite/a/A.java",
            "test-suite-kotlin/src/main/kotlin/a/A.java",
            "test-suite/src/main/java/a/A.java",
            "tck/src/main/java/a/A.java",
            "tcks/microprofile-config/src/a/A.java",
            "http-server-tck/src/main/java/a/A.java",
            "lib/list_kunit.c",
            "drivers/base/regmap/regmap-kunit.c",
        ] {
            assert!(test(path), "{path} is a test");
        }
        for path in [
            "src/main/java/a/A.java",
            "t/helper/test-tool.c",
            "packages/core/testing/src/a.ts",
            "src/testing/testing.go",
            "rust/helpers/kunit.c",
            "src/tckit/a.py",
        ] {
            assert!(!test(path), "{path} is code");
        }
    }

    #[test]
    fn an_exclude_is_a_gitignore_line_that_names_a_path() {
        for good in [
            "build/",
            "*.pb.go",
            "/src/gen/",
            "src/gen/",
            "!keep.py",
            "**/build/**",
            "\\#x",
        ] {
            assert!(exclude(good).is_ok(), "{good}");
        }
        for bad in ["", " ", "/", "//", "!", "!/", "a//", "//a/", "#x", "a/[b/"] {
            assert!(exclude(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_excludes_match_as_git_ignores() {
        let matchers = Matchers::new(&files(
            &[
                "*.log",
                "out",
                "build/",
                "/gen/",
                "src/tmp/",
                "lib/*.py",
                "!lib/keep.py",
                "!build/c.py",
            ],
            &[],
        ))
        .unwrap();
        let excluded = |path: &str| matchers.excluded(Path::new(path), false, Path::new(""));
        // No slash: at any depth, a file or a folder; a slash at its end: a folder only.
        for path in [
            "a.log",
            "x/y/a.log",
            "out/a.py",
            "x/out/a.py",
            "build/a.py",
            "x/build/a.py",
        ] {
            assert!(excluded(path), "{path}");
        }
        assert!(!excluded("x/build.py"));
        assert!(!matchers.excluded(Path::new("x/build"), false, Path::new("")));
        assert!(matchers.excluded(Path::new("x/build"), true, Path::new("")));
        // A slash at its start or before its end: from the root.
        assert!(excluded("gen/a.py") && excluded("src/tmp/a.py"));
        assert!(!excluded("x/gen/a.py") && !excluded("x/src/tmp/a.py"));
        // `*` stays within a part; `!` takes a path back, but not one under an excluded folder.
        assert!(excluded("lib/a.py") && !excluded("lib/x/a.py"));
        assert!(!excluded("lib/keep.py"));
        assert!(excluded("build/c.py"));
    }

    #[test]
    fn a_folder_exclude_leaves_out_the_folders_it_names_and_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        write(
            root,
            &[
                "a.py",
                "build/b.py",
                "src/build/c.py",
                "src/gen/d.py",
                "gen/e.py",
                "src/build.py",
                "src/x/gen/f.py",
            ],
        );
        let input =
            from_paths(&[root.to_path_buf()], &files(&["build/", "src/gen/"], &[])).unwrap();
        assert_eq!(
            found(&input, root),
            vec![
                "a.py".to_string(),
                "gen/e.py".to_string(),
                "src/build.py".to_string(),
                "src/x/gen/f.py".to_string(),
            ]
        );
    }

    #[test]
    fn a_file_given_is_weighed_though_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["gen/a.ts"]);
        let file = root.join("gen/a.ts");
        let input = from_paths(&[file.clone(), file], &files(&["**/gen/**"], &[])).unwrap();
        assert_eq!(found(&input, root), vec!["gen/a.ts".to_string()]);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_reached_through_a_symlink_or_dot_dot_is_one_item_shown_as_first_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["s/t/x.py"]);
        std::os::unix::fs::symlink(root.join("s"), root.join("link")).unwrap();
        std::os::unix::fs::symlink(root.join("s/t/x.py"), root.join("x.py")).unwrap();
        let globs = files(&[], &[]);
        let input = from_paths(&[root.join("s"), root.join("link")], &globs).unwrap();
        assert_eq!(found(&input, root), vec!["s/t/x.py".to_string()]);
        let input = from_paths(&[root.join("link"), root.join("s")], &globs).unwrap();
        assert_eq!(found(&input, root), vec!["link/t/x.py".to_string()]);
        let input = from_paths(&[root.join("x.py"), root.join("s/t/../t/x.py")], &globs).unwrap();
        assert_eq!(found(&input, root), vec!["x.py".to_string()]);
    }

    #[test]
    fn a_folder_given_is_walked_though_excluded_the_excludes_under_it_still_apply() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        write(
            root,
            &[
                "build/proj/a.py",
                "build/proj/src/b.py",
                "build/proj/src/c.d.ts",
                "build/proj/node_modules/d.js",
                "build/proj/build/e.py",
                "gen/f.ts",
            ],
        );
        let excludes = files(&["build/", "node_modules/", "*.d.ts", "gen/**"], &[]);
        let input = from_paths(&[root.join("build/proj")], &excludes).unwrap();
        assert_eq!(
            found(&input, root),
            vec![
                "build/proj/a.py".to_string(),
                "build/proj/src/b.py".to_string()
            ]
        );
        // Each file under it a line matches itself is left out still.
        let input = from_paths(&[root.join("gen")], &excludes).unwrap();
        assert!(found(&input, root).is_empty());
        // From the root, the excluded folder isn't walked.
        let input = from_paths(&[root.to_path_buf()], &excludes).unwrap();
        assert!(found(&input, root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_can_t_be_read_is_left_out_and_listed_the_rest_weighed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, &["a.py", "locked/b.py", "z.py"]);
        let locked = root.join("locked");
        let mode = |mode| fs::set_permissions(&locked, fs::Permissions::from_mode(mode)).unwrap();
        mode(0o000);
        if fs::read_dir(&locked).is_ok() {
            // Run as root, which reads it anyway.
            mode(0o755);
            return;
        }
        // Given twice, it is listed once; the folder given itself can't be read: an error.
        let walked = from_paths(&[root.to_path_buf(), root.to_path_buf()], &files(&[], &[]));
        let given = from_paths(std::slice::from_ref(&locked), &files(&[], &[]));
        mode(0o755);
        let input = walked.unwrap();
        assert_eq!(
            found(&input, root),
            vec!["a.py".to_string(), "z.py".to_string()]
        );
        let unreadable: Vec<_> = input
            .unreadable
            .iter()
            .map(|u| (u.path.strip_prefix(root).unwrap(), u.reason.as_str()))
            .collect();
        assert_eq!(unreadable, vec![(Path::new("locked"), "permission denied")]);
        assert!(given.is_err());
    }

    #[test]
    fn a_leading_dot_is_dropped() {
        assert_eq!(shown(Path::new("./src/a.ts")), Path::new("src/a.ts"));
        assert_eq!(shown(Path::new(".")), Path::new("."));
        assert_eq!(shown(Path::new("../a.ts")), Path::new("../a.ts"));
    }
}
