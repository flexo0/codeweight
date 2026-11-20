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
    /// The weights file (else ./codeweight.toml, else codeweight.toml at the repo's root, else the defaults); not
    /// `init`'s
    // Last in every command's help, before `--help`: unordered, a global option lands wherever its order ties with
    // the command's own.
    #[arg(long, global = true, value_name = "FILE", display_order = 100)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Weigh files and folders
    ///
    /// The code's weight, the docs' and the tests', or one of them alone (`--code`, `--docs`, `--tests`).
    Scan(ScanArgs),
    /// Weigh the net delta of a commit or a range
    ///
    /// A revision against its first parent, or a range A..B: the net delta of the code's weight, the docs' and the
    /// tests', or one of them alone (`--code`, `--docs`, `--tests`).
    Commit(CommitArgs),
    /// Weigh the net delta of the uncommitted changes
    ///
    /// The uncommitted changes against HEAD, the untracked files too, or the staged ones alone (`--staged`): the
    /// net delta of the code's weight, the docs' and the tests', or one of them alone (`--code`, `--docs`,
    /// `--tests`).
    PreCommit(PreCommitArgs),
    /// Write the default weights file
    ///
    /// Never over an existing file; `-` prints it to stdout.
    Init {
        /// Where to write it, `-` for stdout
        #[arg(default_value = config::FILE_NAME)]
        file: PathBuf,
    },
}

/// The arguments of `scan`.
#[derive(Args)]
struct ScanArgs {
    /// A file or a folder, absolute or relative
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[command(flatten)]
    output: Output,
}

/// The arguments of `commit`.
#[derive(Args)]
struct CommitArgs {
    /// A revision (against its first parent) or a range A..B
    #[arg(default_value = "HEAD")]
    rev: String,
    /// Any path inside the repo
    #[arg(short = 'C', default_value = ".")]
    repo: PathBuf,
    #[command(flatten)]
    output: Output,
}

/// The arguments of `pre-commit`.
#[derive(Args)]
struct PreCommitArgs {
    /// Any path inside the repo
    #[arg(short = 'C', default_value = ".")]
    repo: PathBuf,
    /// Only the staged changes (the index against HEAD, as `git commit` would commit them)
    #[arg(long)]
    staged: bool,
    #[command(flatten)]
    output: Output,
}

/// The one number a specific command prints; none for the three of the general one.
#[derive(Args)]
#[group(id = "only", multiple = false)]
struct Only {
    /// Only the code's weight, its tests left out: the total, or with `-v` the whole report
    #[arg(long)]
    code: bool,
    /// Only the tests' weight (the test files, a Rust file's tests): the total, or with `-v` the whole report
    #[arg(long)]
    tests: bool,
    /// Only the docs weight of the Markdown files (those the docs globs match): the total, or with `-v` the whole
    /// report
    #[arg(long)]
    docs: bool,
}

/// What a command prints, the same for the three.
#[derive(Args)]
struct Output {
    #[command(flatten)]
    only: Only,
    /// As JSON: the three numbers and the warnings; with `--code`, `--tests` or `--docs`, the whole report
    #[arg(long)]
    json: bool,
    /// The three numbers on one line, each rounded to a whole number (`c+15 d+17 t-5`)
    #[arg(long, conflicts_with_all = ["json", "only"])]
    short: bool,
    /// The whole report: the total, the functions (or files), and the warnings (with `--code`, `--tests` or
    /// `--docs`)
    #[arg(short, long, requires = "only")]
    verbose: bool,
    /// Only the N heaviest lines of the report (with `--code`, `--tests` or `--docs`)
    #[arg(long, value_name = "N", requires = "only")]
    top: Option<usize>,
    /// What the report lists [default: function] (with `--code`, `--tests` or `--docs`)
    #[arg(long, value_enum, requires = "only")]
    by: Option<By>,
}

impl Output {
    /// The one number asked for, `None` for all three.
    fn mode(&self) -> Option<Mode> {
        let only = &self.only;
        [
            (only.code, Mode::Code),
            (only.tests, Mode::Tests),
            (only.docs, Mode::Docs),
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

    /// The general command's three numbers as asked: `--json` (`json`, with the warnings), `--short` or three
    /// lines, those two with the warnings (`warnings`) to stderr.
    fn parts(
        &self,
        parts: Parts,
        json: impl FnOnce() -> String,
        warnings: impl FnOnce() -> String,
    ) -> Result<()> {
        if self.json {
            return out(&json());
        }
        warn(&warnings())?;
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
        Cli::command()
            .error(
                ErrorKind::ArgumentConflict,
                "`init` takes no `--config`: it writes the defaults, reading no weights file",
            )
            .exit();
    }
    match &cli.command {
        Command::Scan(a) => scan(&cli, a),
        Command::Commit(a) => commit(&cli, Changes::Rev(&a.rev), &a.repo, &a.output),
        Command::PreCommit(a) => {
            let changes = Changes::Uncommitted { staged: a.staged };
            commit(&cli, changes, &a.repo, &a.output)
        }
        Command::Init { file } => init(file),
    }
}

/// Weighs the code at the paths, its docs and its tests, each at its full weight: the three numbers, or the one
/// asked for (`--code`, `--docs`, `--tests`) with its report.
fn scan(cli: &Cli, args: &ScanArgs) -> Result<()> {
    let output = &args.output;
    let mode = output.mode();
    let config = load_config(cli, args.paths.first().map(PathBuf::as_path))?;
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
        warn(&report::scan_warnings(&files, &input.unreadable))?;
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

/// Weighs the net delta of a commit, a range or the uncommitted changes: of the code, its docs and its tests,
/// each at its full weight: the three numbers, or the one asked for (`--code`, `--docs`, `--tests`) with its
/// report.
fn commit(cli: &Cli, changes: Changes, repo: &Path, output: &Output) -> Result<()> {
    let mode = output.mode();
    let config = load_config(cli, Some(repo))?;
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
        warn(&report::commit_warnings_text(&changes))?;
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

/// A weigher per language, and the docs weights for Markdown.
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

    /// A text of `kind` weighed, and its three parts: Markdown its docs weight; code its tests and its code
    /// without them, a test file's (`test`) all test code. For one number (`mode`) the weight is of that part
    /// alone, else of the whole text.
    fn weigh(
        &self,
        kind: Kind,
        text: &str,
        test: bool,
        path: &Path,
        mode: Option<Mode>,
    ) -> Result<(Weighed, Parts)> {
        let lang = match kind {
            Kind::Markdown => {
                let weighed = markdown::weigh(text, self.docs);
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
        let weighed = weigher
            .weigh(text)
            .with_context(|| format!("weighing {}", path.display()))?;
        let weighed = if test { weighed.all_tests() } else { weighed };
        let (code, tests) = weighed.split();
        let parts = Parts {
            code,
            docs: 0.0,
            tests,
        };
        let weighed = match mode {
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
