//! The files a commit, a range or the uncommitted changes changed and their blobs, through the `git` command:
//! `git diff-tree -r -M` for the changes, one `git cat-file --batch` for the texts.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

/// The two sides compared: `from` is `None` for the empty tree (a root commit's parent); `to` a commit, or the
/// tree of the uncommitted changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    pub from: Option<String>,
    pub to: String,
}

/// One side of a changed file: its path from the repo's root and its blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub path: PathBuf,
    pub blob: String,
}

/// A changed file: its side before and after, `None` where the file isn't there (added, deleted) or isn't a
/// regular file (a symlink, a submodule).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub before: Option<Side>,
    pub after: Option<Side>,
}

/// The two sides of `spec`: a revision against its first parent (the root commit against the empty tree), or
/// `A..B` the two revisions (an empty side is `HEAD`, as in git).
pub fn resolve(repo: &Path, spec: &str) -> Result<Range> {
    if spec.contains("...") {
        bail!("`{spec}`: a symmetric range `A...B` isn't supported, give `A..B` or a revision");
    }
    if let Some((from, to)) = spec.split_once("..") {
        let side = |rev: &str| if rev.is_empty() { "HEAD" } else { rev }.to_string();
        return Ok(Range {
            from: Some(commit(repo, &side(from))?),
            to: commit(repo, &side(to))?,
        });
    }
    let to = commit(repo, spec)?;
    // From the commit object itself, not `git rev-list --parents`: that follows a shallow clone's grafts, which
    // make the commit at its boundary look like a root, its whole tree added.
    let object = run(repo, &["cat-file", "commit", &to])?;
    let from = first_parent(&object).map(str::to_string);
    if let Some(parent) = &from {
        let there = git(repo)
            .args(["cat-file", "-e", &format!("{parent}^{{commit}}")])
            .stderr(Stdio::null())
            .status()
            .context("running git (is it installed?)")?;
        if !there.success() {
            bail!(
                "the parent {parent} of `{spec}` isn't in the repo (a shallow clone? fetch more history, \
                 e.g. `git fetch --deepen=1`)"
            );
        }
    }
    Ok(Range { from, to })
}

/// A commit object's first parent: its first `parent` header, before the blank line that ends the headers.
fn first_parent(object: &str) -> Option<&str> {
    object
        .lines()
        .take_while(|line| !line.is_empty())
        .find_map(|line| line.strip_prefix("parent "))
        .map(str::trim)
}

/// The commit a revision names, as its full hash.
fn commit(repo: &Path, rev: &str) -> Result<String> {
    if rev.starts_with('-') {
        bail!("`{rev}` is not a revision");
    }
    let found = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .with_context(|| format!("`{rev}` is not a commit of the repo"))?;
    Ok(found.trim().to_string())
}

/// The uncommitted changes, as the commit `git add -A && git commit` would make: `HEAD` (the empty tree before
/// the first commit) against the working tree, its untracked files too, the ignored ones left out; or, `staged`,
/// as `git commit` would make it, `HEAD` against the index. The working tree is added to a copy of the index (and
/// the tree written from the copy), so the repo's own index isn't touched; its blobs and trees are written to the
/// repo's objects, unreferenced (`git gc` drops them).
pub fn uncommitted(repo: &Path, staged: bool) -> Result<Range> {
    let index = run(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )?;
    let index = PathBuf::from(index.trim_end_matches('\n'));
    let head = git(repo)
        .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .output()
        .context("running git (is it installed?)")?;
    // A repo with no commit yet has no `HEAD` to compare from: the empty tree.
    let from = head
        .status
        .success()
        .then(|| String::from_utf8_lossy(&head.stdout).trim().to_string());
    let scratch = Scratch::new()?;
    let copy = scratch.0.join("index");
    if index.exists() {
        fs::copy(&index, &copy)
            .with_context(|| format!("copying {} to {}", index.display(), copy.display()))?;
    }
    let with_copy = || {
        let mut command = git(repo);
        command.env("GIT_INDEX_FILE", &copy);
        command
    };
    if !staged {
        run_command(with_copy(), &["add", "-A", "--", ":/"])?;
    }
    let to = run_command(with_copy(), &["write-tree"])?;
    let to = String::from_utf8(to).context("`git write-tree` output")?;
    Ok(Range {
        from,
        to: to.trim().to_string(),
    })
}

/// A folder of its own in the temp folder, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let dir = std::env::temp_dir().join(format!(
            "codeweight-{}-{nanos}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: a folder left in the temp folder harms nothing.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The files changed between the range's two sides, renames detected (`-M`), in git's order (by path).
pub fn changes(repo: &Path, range: &Range) -> Result<Vec<Change>> {
    // The empty tree by its hash, not `--root`: that takes a commit, and `to` may be a tree.
    let empty;
    let from = match &range.from {
        Some(from) => from.as_str(),
        None => {
            empty = run(repo, &["hash-object", "-t", "tree", "--stdin"])?;
            empty.trim()
        }
    };
    let args = [
        "diff-tree",
        "-r",
        "-M",
        "-z",
        "--no-commit-id",
        from,
        &range.to,
    ];
    parse_diff_tree(&run_bytes(repo, &args)?)
}

/// `git diff-tree -r -z` output: per file `:<mode> <mode> <blob> <blob> <status>\0<path>\0`, a rename or copy with
/// a second path (`<from>\0<to>\0`). A path is its bytes as they are (`-z` leaves them unquoted), UTF-8 or not.
fn parse_diff_tree(output: &[u8]) -> Result<Vec<Change>> {
    let mut fields = output.split(|&b| b == 0).filter(|f| !f.is_empty());
    let mut changes = Vec::new();
    while let Some(header) = fields.next() {
        let header = std::str::from_utf8(header).with_context(|| {
            format!(
                "unexpected `git diff-tree` output: `{}`",
                String::from_utf8_lossy(header)
            )
        })?;
        let parts: Vec<&str> = header.trim_start_matches(':').split(' ').collect();
        let [mode_before, mode_after, blob_before, blob_after, status] = parts[..] else {
            bail!("unexpected `git diff-tree` output: `{header}`");
        };
        let mut path = || {
            fields
                .next()
                .map(path_of)
                .with_context(|| format!("no path after `{header}` in `git diff-tree` output"))
        };
        let path_before = path()?;
        let path_after = if status.starts_with(['R', 'C']) {
            path()?
        } else {
            path_before.clone()
        };
        let side = |mode: &str, blob: &str, path: PathBuf| {
            // A regular file only (100644, 100755): not a symlink (120000), a submodule (160000) or none (000000).
            mode.starts_with("100").then(|| Side {
                path,
                blob: blob.to_string(),
            })
        };
        let mut before = side(mode_before, blob_before, path_before);
        let after = side(mode_after, blob_after, path_after);
        // A copy leaves its source as it was.
        if status.starts_with('C') {
            before = None;
        }
        changes.push(Change { before, after });
    }
    Ok(changes)
}

/// A path as git wrote it: its bytes as they are on Unix, where a file's name may be any bytes; elsewhere read as
/// UTF-8 (git's encoding of paths there), an invalid byte replaced.
fn path_of(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// The contents of `blobs`, all through one `git cat-file --batch`.
pub fn blobs(repo: &Path, blobs: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
    let mut child = git(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running `git cat-file --batch` (is git installed?)")?;
    let stdin = child.stdin.take().context("git cat-file's stdin")?;
    let stdout = child.stdout.take().context("git cat-file's stdout")?;
    let read = std::thread::scope(|scope| {
        // Written from a thread of its own, so a full pipe either way can't block both.
        let writer = scope.spawn(move || -> std::io::Result<()> {
            let mut stdin = std::io::BufWriter::new(stdin);
            for blob in blobs {
                writeln!(stdin, "{blob}")?;
            }
            stdin.flush()
        });
        let read = read_batch(BufReader::new(stdout), blobs.len());
        let written = writer.join();
        match written {
            Ok(Ok(())) => read,
            // A write fails when git quit early; its own error says why.
            Ok(Err(error)) => read.and(Err(error).context("writing to `git cat-file --batch`")),
            Err(_) => bail!("the thread writing to `git cat-file --batch` panicked"),
        }
    });
    let output = child
        .wait_with_output()
        .context("waiting for `git cat-file --batch`")?;
    if !output.status.success() {
        bail!(
            "`git cat-file --batch` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    read
}

/// `count` objects of `git cat-file --batch` output: `<blob> <type> <size>\n<contents>\n` each.
fn read_batch(mut out: impl BufRead, count: usize) -> Result<HashMap<String, Vec<u8>>> {
    let mut contents = HashMap::with_capacity(count);
    let mut header = String::new();
    for _ in 0..count {
        header.clear();
        out.read_line(&mut header)
            .context("reading `git cat-file --batch`")?;
        let parts: Vec<&str> = header.trim_end().split(' ').collect();
        let [blob, _kind, size] = parts[..] else {
            bail!("`git cat-file --batch`: `{}`", header.trim_end());
        };
        let size: usize = size
            .parse()
            .with_context(|| format!("`git cat-file --batch`: `{}`", header.trim_end()))?;
        let mut bytes = vec![0; size + 1];
        out.read_exact(&mut bytes)
            .with_context(|| format!("reading the blob {blob}"))?;
        bytes.pop();
        contents.insert(blob.to_string(), bytes);
    }
    Ok(contents)
}

/// Runs `git -C <repo> <args>`, its stdout as text; its stderr in the error when it fails.
fn run(repo: &Path, args: &[&str]) -> Result<String> {
    String::from_utf8(run_bytes(repo, args)?)
        .with_context(|| format!("`git {}` output", args.join(" ")))
}

/// Runs `git -C <repo> <args>`, its stdout as bytes; its stderr in the error when it fails.
fn run_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    run_command(git(repo), args)
}

/// `git -C <repo>`, to be given its arguments.
fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo);
    command
}

/// Runs a `git` command with `args`, its stdout as bytes (its stdin empty); its stderr in the error when it fails.
fn run_command(mut command: Command, args: &[&str]) -> Result<Vec<u8>> {
    let output = command
        .args(args)
        .output()
        .context("running git (is it installed?)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let why = match stderr.trim() {
            "" => output.status.to_string(),
            stderr => stderr.to_string(),
        };
        bail!("`git {}` failed: {why}", args.join(" "));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_tree_output_is_parsed_renames_and_non_files_too() {
        let z = "0000000000000000000000000000000000000000";
        let output = format!(
            ":000000 100644 {z} aaa A\0new.py\0\
             :100644 000000 bbb {z} D\0gone.py\0\
             :100644 100755 ccc ddd M\0same.py\0\
             :100644 100644 eee fff R087\0old.ts\0moved.ts\0\
             :100644 120000 ggg hhh T\0link.py\0\
             :000000 160000 {z} iii A\0sub\0"
        );
        let side = |path: &str, blob: &str| {
            Some(Side {
                path: PathBuf::from(path),
                blob: blob.to_string(),
            })
        };
        assert_eq!(
            parse_diff_tree(output.as_bytes()).unwrap(),
            vec![
                Change {
                    before: None,
                    after: side("new.py", "aaa")
                },
                Change {
                    before: side("gone.py", "bbb"),
                    after: None
                },
                Change {
                    before: side("same.py", "ccc"),
                    after: side("same.py", "ddd")
                },
                Change {
                    before: side("old.ts", "eee"),
                    after: side("moved.ts", "fff")
                },
                Change {
                    before: side("link.py", "ggg"),
                    after: None
                },
                Change {
                    before: None,
                    after: None
                },
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_path_that_isn_t_utf8_is_kept_as_its_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let output = b":000000 100644 0000 aaa A\0caf\xe9.py\0";
        let changes = parse_diff_tree(output).unwrap();
        let after = changes[0].after.as_ref().unwrap();
        assert_eq!(after.path.as_os_str().as_bytes(), b"caf\xe9.py");
        assert_eq!(after.blob, "aaa");
    }

    #[test]
    fn the_first_parent_is_the_first_parent_header() {
        let merge = "tree t\nparent p1\nparent p2\nauthor a\n\nparent in the message\n";
        assert_eq!(first_parent(merge), Some("p1"));
        assert_eq!(
            first_parent("tree t\nauthor a\n\nparent in the message\n"),
            None
        );
    }

    #[test]
    fn batch_output_is_read_by_sizes() {
        let output = b"aaa blob 3\nx\ny\nbbb blob 0\n\n";
        let read = read_batch(&output[..], 2).unwrap();
        assert_eq!(read["aaa"], b"x\ny");
        assert_eq!(read["bbb"], b"");
    }
}
