# Using codeweight

It reads Java (`.java`), TypeScript (`.ts`, `.mts`, `.cts`), TSX (`.tsx`), JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`),
Python (`.py`, `.pyi`), Rust (`.rs`) and Go (`.go`), and weighs Markdown (`.md`, `.markdown`) as docs.

```
codeweight scan [<path>...] [<one>|--short] [--json]         weigh files and folders; several allowed,
                                                             default `.`
codeweight show [<rev>|<A>..<B>] [-C <path>] [<one>|--short] [--json]
                                                             weigh the net delta of a commit or a range; default
                                                             `HEAD`
codeweight diff [-C <path>] [--staged] [<one>|--short] [--json]
                                                             weigh the net delta of the uncommitted changes (or
                                                             the staged ones alone)
codeweight init [<file>|-]     write the default weights file (default ./codeweight.toml), never over an existing
                               one; `-` prints it to stdout

<one>: --code|--tests|--docs [-v] [--top <N>] [--by file|function]
```

`scan`, `show` and `diff` take `--config <file>`: the weights file ([weights-file.md](weights-file.md) says where it is looked up
without one); `init` doesn't, it reads none. They take too:

- `--code`, `--tests` or `--docs` (at most one): that number alone, and its report;
- `--verbose` (`-v`): the whole report instead of the total alone;
- `--top <N>`: only the report's first N lines (the heaviest; for a delta, the largest changes);
- `--by file|function`: what the report lists, the functions by default;
- `--short`: the three numbers on one line;
- `--json`: JSON instead of text (described in [json.md](json.md)).

`-v`, `--top` and `--by` need one of `--code`, `--tests` and `--docs`, `--short` none of them, and `--short` and
`--json` don't go together: each is a usage error (exit 2) otherwise.

Without `--code`, `--tests` or `--docs`, a command prints three numbers, each on its line, in the order code, docs,
tests: `code +15.5`, `docs +17`, `tests -5`. Each is signed (`scan`'s too), with at most two decimals, a zero
without a sign (`docs 0`). `--short` prints them on one line, each rounded to a whole number, all three always
there: `c+15 d+17 t-5`, a zero part `d0`. The code's is the weight of the code, its tests left out. The tests' is
the weight of the tests alone: the test files (`test_globs`), and in a Rust file the items marked `#[cfg(test)]`,
`#[test]` or a runner's `#[tokio::test]` (any `#[...::test]`), all in them (a `mod tests` and its functions), and a
whole file marked `#![cfg(test)]`. The docs' is the weight of the Markdown files (`docs_globs`), each by its words
and symbol runs (`[docs]`, see [weights-file.md](weights-file.md); a word 1 and a symbol run 1 by default, so `## Setup` is 2 and `foo(a, b);` 6).
The code, the tests and the docs are always three numbers, never one sum, each at its full weight: a change to all
three has its code in one, its tests in another and its docs in the third. The input is read once and each file
weighed once, a Rust file with tests counting toward the code and the tests both.

With `--code`, `--tests` or `--docs`, a command prints that number alone: `scan` its total (`4698.25`), `show` and
`diff` their net delta, signed (`+42.5`, `-17`, `0`). `--verbose` (`-v`) prints its whole report instead, as
`--top` and `--by` do. With the numbers alone (the three, `--short`, or one total), a file's syntax errors and a
folder that can't be read are a warning on stderr.

The text report (`--code -v`, or `--tests`, `--docs`) starts with the total. After it come the functions and classes, heaviest first, then by path and
name, each as `<weight>  <path>  <name>`; with `--by file`, the files. A class's line (`Name (class)`) has its code
outside its functions; `(file)` is a file's code outside any class. A function is named by its enclosing classes and
functions, `Outer.Inner.method`. Same-named functions of one class are one line, their weights summed: overloads, or
a Java class's constructors (`A.A`). A Java anonymous class is `new Type`; a TypeScript constructor is
`A.constructor`. A TypeScript (or JavaScript) arrow or function expression bound to a name at the file's level or as
a class field (`export const f = () => ...`, `onClick = () => ...`) is a function of that name; anywhere else (inside
a function too) it is a lambda, its code the function's around it. A Rust method is named by its `impl` block's type
(`Foo.method`), so a type's struct and all its `impl` blocks in a file are one class; a Go method by its receiver's
type (`T.M` for `func (r *T) M()`). A weight has at most two decimals.

A folder is walked for the files in it: its `.gitignore` is honoured, hidden files and the `[files]` excludes are
left out (a file named on the command line is weighed even if excluded). The globs are matched against a file's path
from the root of its git repo (outside a repo, from the current folder, or from the folder given when the path is
outside the current one). The code leaves the test files (`test_globs`) and a Rust file's tests out, the tests
are only them. A file the docs globs match (`docs_globs`) is docs alone, though it is under `tests/`: neither the
code nor the tests weigh it, the docs only such files, each one line of `--docs`' report (its `(file)`, as it has no
functions). Files are weighed in parallel; the output is the same on every run.

A file that doesn't parse cleanly is still weighed, as tree-sitter recovered it, with a warning. A file in a language
codeweight doesn't read is left out, never an error. A folder under the one scanned that can't be read (a
root-owned docker volume, say) is left out with a warning and the rest weighed; a folder given that can't be read is
an error.

Exit codes: 0 done, 1 an error (a bad path, revision or weights file, or not in a git repo), 2 a usage error (an
unknown command or option, options that don't go together). A reader that quits early (`codeweight scan -v . | head`)
ends it quietly, exit 0.

## `show`, `diff`

`codeweight show <rev>` weighs what a commit changed: its files before and after, against its first parent (a
merge's too), the root commit against the empty tree; in a shallow clone, a commit whose parent wasn't fetched is
an error (fetch more history), not a root. `codeweight show A..B` compares the two revisions' trees
directly, one net delta for the range (not one per commit, as `git show A..B` would), so a change made and undone
between them nets to zero (an empty side is `HEAD`; `A...B` is an error).
`<rev>` is any git revision (a hash, `HEAD~2`, a tag), `HEAD` when left out (`codeweight show`, as `git show`);
`-C <path>` (`--repo <path>`) is any path inside the repo (default `.`), as `git -C`. It runs
`git` (`git diff-tree -r -M`, so a renamed file is one file, and one `git cat-file --batch` for the texts).

`codeweight diff` weighs the uncommitted changes, before you commit them: `HEAD` against the
working tree, staged or not, the untracked files too and the ignored ones left out, as `git add -A && git commit`
would commit them (in a repo with no commit yet, against the empty tree); so not as `git diff`, which compares the
index with the working tree, the tracked files alone. With `--staged`, only the staged changes, as
`git diff --staged`: `HEAD` against the index, as `git commit` would commit them, the unstaged and untracked ones
left out. `-C` / `--repo` as for `show`. Your
index is left as it is: the working tree is added to a copy of it. git writes the changed files' blobs to the repo's objects, which nothing refers to,
and `git gc` drops them later.

A file's delta is its weight after minus before: an added file's is all positive, a deleted one's all negative. Its
functions and classes are matched by their qualified name within the file (a renamed file's old and new), each one
`added`, `removed`, `heavier` or `lighter`. The changed files are weighed as a walk of the repo's root weighs them:
the `[files]` excludes and hidden paths are left out, and so is a file codeweight doesn't read. Their code, docs
and tests are weighed as `scan` weighs them, with `--code`, `--tests` or `--docs` as `scan` does with it.

With `--code -v` (or `--tests`, `--docs`), the text report starts with the total delta (`total +42.5`, `total -17`). After it comes one line per function and
class (or per file with `--by file`), `<delta>  <status>  <path>  <name>`, a renamed file's path as `old -> new`; the
largest changes first, either way (a gain before a loss of the same size), then by path and name. A line whose delta
is 0 (an untouched function, a pure rename) is left out; `--top` keeps the first N lines.
