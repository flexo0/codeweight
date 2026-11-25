# Releasing

## The version

Semver, `MAJOR.MINOR.PATCH`: Cargo.toml's `version`, which `codeweight --version` prints and a tag `vX.Y.Z` marks.
What it versions is what a script, a hook or another tool relies on:

- the CLI: what [cli.md](cli.md)'s compatibility lists, a command's and an option's name and meaning, the defaults,
  the exit codes, the text of the numbers alone (`code +15.5`, `--short`'s `c+15 m+3 d+17 t-5`);
- the JSON's fields ([json.md](json.md));
- the weights file's keys, categories and languages ([weights-file.md](weights-file.md)): an unknown one is an
  error, so a removed or renamed one breaks a user's file;
- the numbers: what the same input weighs by the defaults.

| A change | While 0.x | From 1.0 |
|---|---|---|
| A break: a JSON field renamed, removed or changed in type; a command or option removed, or its meaning changed; a weights-file key, category or language removed or renamed; the numbers' text changed | minor | major |
| The numbers by the defaults: the default weights retuned, a new category or rule that weighs by default | minor | minor |
| An addition: a field, a command, an option, a language, a key | patch | minor |
| A fix (even one that moves a number) | patch | patch |

The compatibility rule is that the CLI and the JSON only grow, so a break is the exception. While 0.x the minor
is the digit that says "may break", as Cargo reads it (0.1.x → 0.2.0 isn't taken by a `"0.1"` requirement), and
the numbers moving is reason enough for it: a tool that compares them across versions must know.

## CHANGELOG.md

At the repo's root, in the crate too: a section per version, newest first, `## 0.2.0 - 2026-12-01`, its lines
grouped as `Changed` (the breaks first, each marked a break), `Weights` (the numbers' changes), `Added` and `Fixed`.
A section is written at its release, from the commit subjects since the last tag (`git log --oneline vX.Y.Z..`),
their weights (`[c+15 m+3 d+17 t-5]`) dropped; the `docs`, `tests` and `ci` commits and refactors that change nothing
visible are left out. 0.1.0's section, written before its tag, is the first release's summary, dated when tagged.

No `Unreleased` section that each commit adds a line to: a commit's subject already says what changed and how it
behaves, and commits pushed close together, by the agents' runs, would each insert at the same line and conflict.

The commits that may be a break or move the numbers are the ones touching the contract's docs or the defaults, since
a change to the JSON, the CLI or the weights file changes its doc in the same commit (AGENTS.md):
`git log --oneline vX.Y.Z.. -- docs/json.md docs/usage.md docs/weights-file.md skills/ src/default.toml`.

## A release, step by step

1. Read the commits since the last tag (and the paths above) and pick the bump by the table.
2. Set Cargo.toml's `version`, then `cargo check`, so Cargo.lock's own `codeweight` entry follows.
3. Write CHANGELOG.md's section for it, dated.
4. The checks (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`) and
   `cargo publish --dry-run`.
5. Commit Cargo.toml, Cargo.lock and CHANGELOG.md: `release: [c0 d+N t0] 0.2.0, ...` (its weight as any
   commit's).
6. `git tag v0.2.0` on that commit and `git push origin master v0.2.0`. If the push is rejected (master moved),
   rebase and tag again before pushing, never move a pushed tag. The tag's push starts the release workflow (below):
   watch it make the GitHub release.
7. `cargo publish`. A published version can't be published again: a mistake is the next patch (`cargo yank` takes a
   broken one off).

## Why by hand

A release is rare and a single crate's, and the steps above are a few commands; a tool would cost more than it
saves here:

- `release-plz` keeps a release PR open and publishes on its merge, but the commits go straight to master, with no
  PRs; it needs a workflow and a crates.io token of its own, its GitHub release would overlap a release workflow on
  the `v*` tag, and its generated changelog (`git-cliff`) expects conventional commits (`feat:`, `fix:`), not
  `<area>: [c d t] <what>`.
- `cargo-release` bumps, tags, publishes and pushes from one command, saving about four of them, for a tool to
  install and a `release.toml`; its rewriting of the changelog pays off only with an `Unreleased` section kept along
  the way (above, why not).
- Both write the release commit from a fixed template, without the weight every subject carries.

## The binaries

`.github/workflows/release.yml`, on a `v*` tag's push: a `--release` binary for each target, archived with README.md
and LICENSE as `codeweight-vX.Y.Z-<target>.tar.gz` (`.zip` for Windows) holding a folder of the same name, and a
GitHub release of the tag with the archives attached, its notes CHANGELOG.md's section for the version. It fails,
releasing nothing, when the tag isn't `v` and Cargo.toml's version or CHANGELOG.md has no section for it: the tagged
commit is wrong then, and a pushed tag isn't moved, so the fix is the next patch's release. A run that failed on its
own (a runner, the network) is rerun from GitHub.

| Target | Runner |
|---|---|
| `x86_64-unknown-linux-musl` | `ubuntu-latest`, with `musl-tools` for the grammars' C |
| `x86_64-apple-darwin` | `macos-latest` (arm64), cross-built |
| `aarch64-apple-darwin` | `macos-latest` |
| `x86_64-pc-windows-msvc` | `windows-latest` |

- Linux is musl, a static binary, not gnu: a gnu one links the runner's glibc and fails (`GLIBC_2.xx not found`) on
  any older one (RHEL 9, Debian 11, an Ubuntu older than the runner's) and on Alpine, where a downloaded binary runs
  too (a CI container, a self-hosted runner). The grammars are C, no C++, so `musl-gcc` is all they need. musl's
  allocator is slower under many threads, which a `scan` of a big tree on many cores pays; a fix would need both a
  Rust `#[global_allocator]` and `tree_sitter::set_allocator` for the grammars' C allocations.
- The toolchain is rust-toolchain.toml's, its target added with `rustup toolchain install <channel> --target`: a
  target added to another toolchain (the runner's stable) isn't the one cargo builds with.
- The target triple is in the archive's name, so a tool downloading the binary for its platform maps to it plainly.

Left: no version is tagged or published yet, 0.1.0 is the first, so the workflow hasn't run on GitHub yet.
