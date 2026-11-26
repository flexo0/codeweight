//! The command line: its arguments and the commands.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use rayon::prelude::*;

use crate::config::{self, Config, Docs};
use crate::engine::{Weighed, Weigher};
use crate::git;
use crate::lang;
use crate::markdown;
use crate::report::{self, By, Changed, Parts, Revisions, Weighted};
use crate::source::{self, Kind, Mode};

/// Weigh code: every element by its weight, heavier when nested.
#[derive(Parser)]
#[command(name = "codeweight", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// The weights file [default: ./codeweight.toml, else the repo root's codeweight.toml, else the built-in
    /// weights]; `init` takes none
    // Last in every command's help, before `--help`: unordered, a global option lands wherever its order ties with
    // the command's own.
    #[arg(long, global = true, value_name = "FILE", display_order = 100)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Weigh files and folders
    ///
    /// Prints four weights: the code's (its tests left out), its comments', the docs' and the tests'; with
    /// `--code`, `--docs`, `--tests` or `--comments`, that one alone.
    Scan(ScanArgs),
    /// Weigh the net delta of a commit or a range
    ///
    /// A commit against its first parent, or a range A..B its two trees against each other (one net delta, not a
    /// delta per commit): prints the net delta, after minus before, of four weights: the code's (its tests left
    /// out), its comments', the docs' and the tests'; with `--code`, `--docs`, `--tests` or `--comments`, that one
    /// alone.
    Show(ShowArgs),
    /// Weigh the net delta of the uncommitted changes
    ///
    /// HEAD against the working tree, the untracked files too (unlike `git diff`, which compares the index with
    /// the working tree), or against the index alone (`--staged`, as `git diff --staged`): prints the net delta,
    /// after minus before, of four weights: the code's (its tests left out), its comments', the docs' and the
    /// tests'; with `--code`, `--docs`, `--tests` or `--comments`, that one alone.
    Diff(DiffArgs),
    /// Write the default weights file
    ///
    /// Never over an existing file; `-` prints it to stdout.
    Init {
        /// The file to write, `-` for stdout
        #[arg(default_value = config::FILE_NAME)]
        file: PathBuf,
    },
}

/// The arguments of `scan`.
#[derive(Args)]
struct ScanArgs {
    /// The files and folders to weigh
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[command(flatten)]
    excludes: Excludes,
    #[command(flatten)]
    output: Output,
}

/// The arguments of `show`.
#[derive(Args)]
struct ShowArgs {
    /// The commit to weigh (against its first parent: a hash, a tag, `HEAD~2`), or a range A..B
    #[arg(default_value = "HEAD")]
    rev: String,
    #[command(flatten)]
    repo: Repo,
    #[command(flatten)]
    excludes: Excludes,
    #[command(flatten)]
    output: Output,
}

/// The arguments of `diff`.
#[derive(Args)]
struct DiffArgs {
    #[command(flatten)]
    repo: Repo,
    /// Only the staged changes: the index against HEAD, what `git commit` would commit
    #[arg(long)]
    staged: bool,
    #[command(flatten)]
    excludes: Excludes,
    #[command(flatten)]
    output: Output,
}

/// The repo a delta is of, the same for `show` and `diff`.
#[derive(Args)]
struct Repo {
    /// Any path inside the git repo (as `git -C`)
    #[arg(short = 'C', long = "repo", value_name = "PATH", default_value = ".")]
    repo: PathBuf,
}

/// What a run leaves out besides the weights file's excludes, the same for the three.
#[derive(Args)]
struct Excludes {
    /// The paths to leave out besides the weights file's excludes, each a `.gitignore` line (`build/`, `*.gen.ts`,
    /// `/src/gen/`, `!keep.ts`); repeatable
    #[arg(long = "exclude", value_name = "PATTERN", value_parser = exclude)]
    patterns: Vec<String>,
}

impl Excludes {
    /// `config`'s excludes with these after them.
    fn add_to(&self, config: &mut Config) {
        config.files.exclude.extend(self.patterns.iter().cloned());
    }
}

/// An `--exclude` checked, a bad one a usage error.
fn exclude(pattern: &str) -> Result<String, String> {
    source::exclude(pattern)
        .map(|()| pattern.to_string())
        .map_err(|error| format!("{error:#}"))
}

/// The one number a specific command prints; none for the four of the general one.
#[derive(Args)]
#[group(id = "only", multiple = false)]
struct Only {
    /// Only the code's weight, its tests left out; with `-v`, its report
    #[arg(long)]
    code: bool,
    /// Only the tests' weight (the test files, a Rust file's `#[cfg(test)]` code); with `-v`, its report
    #[arg(long)]
    tests: bool,
    /// Only the docs' weight (the Markdown, reST and AsciiDoc files); with `-v`, its report
    #[arg(long)]
    docs: bool,
    /// Only the comments' weight (the code's comments and docstrings, test code's left out); with `-v`, its report
    #[arg(long)]
    comments: bool,
}

/// What a command prints, the same for the three.
#[derive(Args)]
struct Output {
    #[command(flatten)]
    only: Only,
    /// JSON instead of text: the four numbers and the warnings; with `--code`, `--tests`, `--docs` or
    /// `--comments`, the whole report
    #[arg(long)]
    json: bool,
    /// The four numbers on one line, each rounded to a whole number, the comments' `m`: `c+15 m+3 d+17 t-5`
    #[arg(long, conflicts_with_all = ["json", "only"])]
    short: bool,
    /// The warnings too (syntax errors, unreadable folders), to stderr; with `--code`, `--tests`, `--docs` or
    /// `--comments`, the whole report: the total, a line per function (or file), the warnings
    #[arg(short, long)]
    verbose: bool,
    /// Only the report's N heaviest lines (a delta's: the N largest changes); implies `-v`
    #[arg(long, value_name = "N", requires = "only")]
    top: Option<usize>,
    /// The report's lines, a function or a file each [default: function]; implies `-v`
    #[arg(long, value_enum, requires = "only")]
    by: Option<By>,
}

impl Output {
    /// The one number asked for, `None` for all four.
    fn mode(&self) -> Option<Mode> {
        let only = &self.only;
        [
            (only.code, Mode::Code),
            (only.tests, Mode::Tests),
            (only.docs, Mode::Docs),
            (only.comments, Mode::Comments),
        ]
        .into_iter()
        .find_map(|(asked, mode)| asked.then_some(mode))
    }

    fn by(&self) -> By {
        self.by.unwrap_or(By::Function)
    }

    /// Whether a specific command's text is the whole report, not the total alone: `--verbose`, or a list asked
    /// for.
    fn report(&self) -> bool {
        self.verbose || self.top.is_some() || self.by.is_some()
    }

    /// The general command's numbers as asked: `--json` (`json`, the four with the warnings), `--short` (the four on
    /// one line) or four lines, those two with the warnings (`warnings`) to stderr when `--verbose`.
    fn parts(
        &self,
        parts: Parts,
        json: impl FnOnce() -> String,
        warnings: impl FnOnce() -> String,
    ) -> Result<()> {
        if self.json {
            return out(&json());
        }
        if self.verbose {
            warn(&warnings())?;
        }
        out(&if self.short {
            report::parts_short(parts)
        } else {
            report::parts_text(parts)
        })
    }
}

/// Runs the command line's command, its output to stdout.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::Init { .. }) && cli.config.is_some() {
        let mut command = Cli::command();
        // Built, so the error's usage is `init`'s, `codeweight init`, not the whole CLI's.
        command.build();
        let mut init = command.find_subcommand("init").cloned().unwrap_or(command);
        init.error(
            ErrorKind::ArgumentConflict,
            "`init` takes no `--config`: it writes the defaults, reading no weights file",
        )
        .exit();
    }
    match &cli.command {
        Command::Scan(a) => scan(&cli, a),
        Command::Show(a) => delta(
            &cli,
            Changes::Rev(&a.rev),
            &a.repo.repo,
            &a.excludes,
            &a.output,
        ),
        Command::Diff(a) => {
            let changes = Changes::Uncommitted { staged: a.staged };
            delta(&cli, changes, &a.repo.repo, &a.excludes, &a.output)
        }
        Command::Init { file } => init(file),
    }
}

/// Weighs the code at the paths, its docs, its tests and its comments, each at its full weight: the four numbers,
/// or the one asked for (`--code`, `--docs`, `--tests`, `--comments`) with its report.
fn scan(cli: &Cli, args: &ScanArgs) -> Result<()> {
    let output = &args.output;
    let mode = output.mode();
    let mut config = load_config(cli, args.paths.first().map(PathBuf::as_path))?;
    args.excludes.add_to(&mut config);
    let mut input = source::from_paths(&args.paths, &config.files)?;
    if let Some(mode) = mode {
        input.keep(mode);
    }
    let weighers = Weighers::new(&config);
    // In parallel, each file read when weighed; `collect` keeps the input's order.
    let (files, parts): (Vec<Weighted>, Vec<Parts>) = input
        .items
        .into_par_iter()
        .map(|item| {
            let (weighed, parts) =
                weighers.weigh(item.kind, &item.text()?, item.test, &item.path, mode)?;
            Ok((
                Weighted {
                    path: item.path,
                    weighed,
                },
                parts,
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .unzip();
    if mode.is_none() {
        let parts = parts.into_iter().sum();
        return output.parts(
            parts,
            || report::scan_parts_json(parts, &files, &input.unreadable),
            || report::scan_warnings(&files, &input.unreadable),
        );
    };
    out(&if output.json {
        report::scan_json(&files, &input.unreadable, output.by(), output.top)
    } else if output.report() {
        report::scan_text(&files, &input.unreadable, output.by(), output.top)
    } else {
        report::scan_number(&files)
    })
}

/// The changes a delta is of.
#[derive(Clone, Copy)]
enum Changes<'a> {
    /// A commit or a range.
    Rev(&'a str),
    /// The uncommitted changes, all of them or the staged ones only.
    Uncommitted { staged: bool },
}

/// Weighs the net delta of a commit, a range or the uncommitted changes: of the code, its docs, its tests and its
/// comments, each at its full weight: the four numbers, or the one asked for (`--code`, `--docs`, `--tests`,
/// `--comments`) with its report.
fn delta(
    cli: &Cli,
    changes: Changes,
    repo: &Path,
    excludes: &Excludes,
    output: &Output,
) -> Result<()> {
    let mode = output.mode();
    let mut config = load_config(cli, Some(repo))?;
    excludes.add_to(&mut config);
    let range = match changes {
        Changes::Rev(rev) => git::resolve(repo, rev)?,
        Changes::Uncommitted { staged } => git::uncommitted(repo, staged)?,
    };
    // The uncommitted changes' tree is no commit of the repo's.
    let to = matches!(changes, Changes::Rev(_)).then_some(range.to.as_str());
    let revisions = Revisions {
        from: range.from.as_deref(),
        to,
    };
    let changes = source::from_commit(repo, &range, &config.files, mode)?;
    let weighers = Weighers::new(&config);
    let side = |blob: Option<source::Blob>| -> Result<Option<(PathBuf, Weighed, Parts)>> {
        blob.map(|b| {
            let (weighed, parts) = weighers.weigh(b.kind, &b.text, b.test, &b.path, mode)?;
            Ok((b.path, weighed, parts))
        })
        .transpose()
    };
    let (changes, parts): (Vec<Changed>, Vec<Parts>) = changes
        .into_par_iter()
        .map(|change| {
            let before = side(change.before)?;
            let after = side(change.after)?;
            let (path, old_path) = match (&before, &after) {
                (Some((old, _, _)), Some((new, _, _))) if old != new => {
                    (new.clone(), Some(old.clone()))
                }
                (_, Some((path, _, _))) | (Some((path, _, _)), None) => (path.clone(), None),
                (None, None) => bail!("a change with neither side"),
            };
            let parts = |side: &Option<(PathBuf, Weighed, Parts)>| {
                side.as_ref().map_or_else(Parts::default, |(_, _, p)| *p)
            };
            let delta = parts(&after) - parts(&before);
            Ok((
                Changed {
                    path,
                    old_path,
                    before: before.map(|(_, w, _)| w),
                    after: after.map(|(_, w, _)| w),
                },
                delta,
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .unzip();
    if mode.is_none() {
        let parts = parts.into_iter().sum();
        return output.parts(
            parts,
            || report::commit_parts_json(parts, &changes, &revisions),
            || report::commit_warnings_text(&changes),
        );
    };
    out(&if output.json {
        report::commit_json(&changes, &revisions, output.by(), output.top)
    } else if output.report() {
        report::commit_text(&changes, output.by(), output.top)
    } else {
        report::commit_number(&changes)
    })
}

/// stdout or stderr closed before all was written to it (`codeweight scan | head`): no error to report, the
/// reader has all it wanted.
#[derive(Debug)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the output was closed")
    }
}

impl std::error::Error for Closed {}

/// Writes `text` to stdout.
fn out(text: &str) -> Result<()> {
    write_all(io::stdout().lock(), text).context("writing to stdout")
}

/// Writes `text` (the warnings) to stderr.
fn warn(text: &str) -> Result<()> {
    write_all(io::stderr().lock(), text).context("writing to stderr")
}

/// Writes `text` and flushes it, a closed pipe `Closed`.
fn write_all(mut to: impl Write, text: &str) -> Result<()> {
    match to.write_all(text.as_bytes()).and_then(|()| to.flush()) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Err(Closed.into()),
        written => Ok(written?),
    }
}

/// The weights file found for a path (`inside`: the first scanned, or the commit's repo), loaded.
fn load_config(cli: &Cli, inside: Option<&Path>) -> Result<Config> {
    let cwd = std::env::current_dir().context("reading the current folder")?;
    let found = config::find(cli.config.as_deref(), &cwd, inside)?;
    Config::load(found.as_deref())
}

/// A weigher per language, and the docs weights for the docs files.
struct Weighers<'c> {
    weighers: Vec<Weigher<'c>>,
    docs: &'c Docs,
}

impl<'c> Weighers<'c> {
    fn new(config: &'c Config) -> Weighers<'c> {
        Weighers {
            weighers: lang::ALL.iter().map(|l| Weigher::new(l, config)).collect(),
            docs: &config.docs,
        }
    }

    /// A text of `kind` weighed, and its four parts: docs their docs weight; code its tests, its code without
    /// them, a test file's (`test`) all test code, and its comments, a test file's none. For one number (`mode`)
    /// the weight is of that part alone, else of the whole text's code.
    fn weigh(
        &self,
        kind: Kind,
        text: &str,
        test: bool,
        path: &Path,
        mode: Option<Mode>,
    ) -> Result<(Weighed, Parts)> {
        let lang = match kind {
            Kind::Docs(format) => {
                let weighed = markdown::weigh(text, format, self.docs);
                let parts = Parts {
                    docs: weighed.total,
                    ..Parts::default()
                };
                return Ok((weighed, parts));
            }
            Kind::Code(lang) => lang,
        };
        let weigher = self
            .weighers
            .iter()
            .find(|w| std::ptr::eq(w.lang(), lang))
            .context("a file of a language with no weigher")?;
        // Test code's comments weigh nothing, so a test file's aren't looked for.
        let with_comments = !test && matches!(mode, None | Some(Mode::Comments));
        let (weighed, comments) = if with_comments {
            weigher.weigh_with_comments(text)
        } else {
            weigher.weigh(text).map(|w| {
                let none = Weighed::none(w.clean);
                (w, none)
            })
        }
        .with_context(|| format!("weighing {}", path.display()))?;
        let weighed = if test { weighed.all_tests() } else { weighed };
        let (code, tests) = weighed.split();
        let parts = Parts {
            code,
            docs: 0.0,
            tests,
            comments: comments.total,
        };
        let weighed = match mode {
            Some(Mode::Comments) => comments,
            Some(mode) => weighed.only(mode == Mode::Tests),
            None => weighed,
        };
        Ok((weighed, parts))
    }
}

/// Writes the defaults to `file`, never over an existing one, or to stdout for `-`.
fn init(file: &Path) -> Result<()> {
    if file == Path::new("-") {
        return out(config::DEFAULTS);
    }
    let mut created = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .with_context(|| {
            format!(
                "creating {} (an existing file is not overwritten)",
                file.display()
            )
        })?;
    created
        .write_all(config::DEFAULTS.as_bytes())
        .with_context(|| format!("writing {}", file.display()))?;
    out(&format!("wrote {}\n", file.display()))
}
