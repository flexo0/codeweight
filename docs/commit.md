# The commit's net delta

Built (`git`, `source::from_commit`, `report`):

1. The two sides (`git::resolve`): a revision is `git rev-parse --verify <rev>^{commit}`, compared against its first
   parent (a merge too, so it shows what the merge brought in), the root commit against the empty tree. The parent is the commit object's first `parent` line (`git cat-file commit`), not
   `git rev-list --parents`: that follows a shallow clone's grafts, so the commit at its boundary (CI's depth-1
   checkout) would look like a root and its whole tree added. The object is read as its bytes: its author and
   message may be in any encoding (an `encoding` header's, or Latin-1 an older git kept as given:
   spring-framework's commits of 2012-2014 by `Jürgen`). A parent missing from the repo is an error that says to
   fetch more history. `A..B` is the two revisions, an empty side `HEAD` as in git;
   `A...B` (against the merge base) is an error, not silently read as `A..B`. A revision starting with `-` is an
   error, so it can't pass for an option.
   `diff` (`git::uncommitted`): `HEAD` (the empty tree in a repo with no commit) against a tree of the working
   tree, made by `git add -A` into a copy of the index (`GIT_INDEX_FILE`, in a temp folder of its own) and
   `git write-tree`, the index's path (`git rev-parse --git-path index`) read as its bytes, so a repo in a folder
   whose name isn't UTF-8 works too. So the untracked files are in it and the ignored ones aren't, renames are
   found between a tracked file and an untracked one, the index's stat cache spares hashing the unchanged files, and the rest of
   the pipeline is the one a commit goes through. The user's index is never touched; the blobs and trees go to the
   repo's objects, unreferenced, for `git gc`. `--staged` skips the `git add -A`: the tree is the index's own,
   written from its copy (`git write-tree` may update an index's cached trees, so not from the repo's own), the
   empty tree where the repo has no index yet. The other way, `git diff-index HEAD` with the after side read from
   the disk, would have been a second path for the texts and the untracked files listed apart, without renames.
2. The changed files: `git diff-tree -r -M -z --no-commit-id <from> <to>`, the empty tree by its hash
   (`git hash-object -t tree --stdin`) where there's no `from` (`--root` takes a commit, not a tree) (renames detected, so a moved file is not a
   deletion and an addition; `-z`, so no path is quoted). Git's own rename rules decide: two files of the same text,
   one deleted and one added, are a rename. A path is read as its bytes, so a file whose name isn't UTF-8 (git
   allows it on Linux) is weighed like any other, its name shown with `�` for the bytes that aren't.
3. Their text on both sides, all through one `git cat-file --batch`, written from a thread of its own while the
   output is read, so neither pipe can fill and block both.
4. Per file `delta = weight(after) − weight(before)`: an added file all positive, a deleted one all negative; an
   unsupported one left out.
5. Per function and class, and the file's code outside them, matched by kind and qualified name (`Class.method`,
   `module.func`) within the file, a renamed file's old and new: added, removed, heavier, lighter. A function moved
   to another file is removed from one and added to the other.
6. The report: the four deltas, the code's, the docs', the tests' and the comments'; or one of them (`--code`,
   `--tests`, `--docs`, `--comments`), its total delta (`+42.5`, `-17`), then its files' or functions' deltas ([cli.md](cli.md)).

The code, the docs, the tests and the comments are four deltas, never one: a change's code, its test files and a
Rust file's tests left out; its docs; its tests alone; its code's comments; each side kept and weighed once as
`scan` would, its parts split (`Weighed::split`, the comments' `Weighed` beside), or with `--code`, `--tests`,
`--docs` or `--comments` kept and weighed for that one alone
(`source::from_commit`'s `mode`, `Weighed::only`). A sum of the two, or a factor that weighs
the tests less in it, would let a change to one hide in the other's number.

A side keeps what a walk of the repo's root would, but for the ignore files: `show` and `diff` weigh what git
tracks, while `scan` (the `ignore` crate's walk) leaves out what a `.gitignore`, a `.ignore`, `.git/info/exclude` or
the global gitignore matches, even a committed file (one added before its line, or with `git add -f`: a committed
`src/ign.py` that `src/.gitignore` names). The trees are read with the walk's hidden-file rule and the excludes, not
the ignore files, so such a file's delta is weighed, and `show` of a commit is then not the difference of two scans.
It is rare: 37 of about 300,000 tracked files in 23 large open-source repos (`git ls-files -ci --exclude-standard`).

The `show` and `diff` tests make throwaway repos (`tempfile`, git's global and system config off): an add, a delete, a
rename, a merge, the root commit, a range, the excludes, the code and the tests apart, a bad revision, a shallow clone's
boundary; the uncommitted changes (staged, unstaged, untracked, ignored, the index unchanged after), the staged ones alone, a rename
into an untracked file, a repo with no commit (and no index).
