# The commit's net delta

Built (`git`, `source::from_commit`, `report`):

1. The two sides (`git::resolve`): a revision is `git rev-parse --verify <rev>^{commit}`, compared against its first
   parent (a merge too, so it shows what the merge brought in), the root commit against the empty tree
   (`git diff-tree --root`). The parent is the commit object's first `parent` line (`git cat-file commit`), not
   `git rev-list --parents`: that follows a shallow clone's grafts, so the commit at its boundary (CI's depth-1
   checkout) would look like a root and its whole tree added. A parent missing from the repo is an error that says
   to fetch more history. `A..B` is the two revisions, an empty side `HEAD` as in git;
   `A...B` (against the merge base) is an error, not silently read as `A..B`. A revision starting with `-` is an
   error, so it can't pass for an option.
2. The changed files: `git diff-tree -r -M -z --no-commit-id <from> <to>` (renames detected, so a moved file is not a
   deletion and an addition; `-z`, so no path is quoted). Git's own rename rules decide: two files of the same text,
   one deleted and one added, are a rename. A path is read as its bytes, so a file whose name isn't UTF-8 (git
   allows it on Linux) is weighed like any other, its name shown with `�` for the bytes that aren't.
3. Their text on both sides, all through one `git cat-file --batch`, written from a thread of its own while the
   output is read, so neither pipe can fill and block both.
4. Per file `delta = weight(after) − weight(before)`: an added file all positive, a deleted one all negative; an
   unsupported one skipped and listed.
5. Per function and class, and the file's code outside them, matched by kind and qualified name (`Class.method`,
   `module.func`) within the file, a renamed file's old and new: added, removed, heavier, lighter. A function moved
   to another file is removed from one and added to the other.
6. The report: the total delta first (`+42.5`, `-17`), then the files' or the functions' deltas ([cli.md](cli.md)).

The `commit` tests make throwaway repos (`tempfile`, git's global and system config off): an add, a delete, a
rename, a merge, the root commit, a range, the excludes and test factor, a bad revision, a shallow clone's
boundary.
