//! The command line: its arguments and the commands.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rayon::prelude::*;

use crate::config::{self, Config};
use crate::engine::{Weighed, Weigher};
use crate::git;
use crate::lang::{self, Lang};
use crate::report::{self, By, Changed, Revisions, Weighted};
use crate::source;

/// Weighs code: every element by its weight, heavier when nested.
#[derive(Parser)]
#[command(name = "codeweight", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// The weights file (else ./codeweight.toml, else codeweight.toml at the repo's root, else the defaults)
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
    /// The report's format
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Only the N heaviest lines of the report (the report, not the total alone)
    #[arg(long, global = true, value_name = "N")]
    top: Option<usize>,
    /// What the report lists [default: function] (the report, not the total alone)
    #[arg(long, global = true, value_enum)]
    by: Option<By>,
}

impl Cli {
    fn by(&self) -> By {
        self.by.unwrap_or(By::Function)
    }

    /// Whether the text is the whole report, not the total alone: `--verbose`, or a list asked for.
    fn report(&self, verbose: bool) -> bool {
        verbose || self.top.is_some() || self.by.is_some()
    }
}

#[derive(Subcommand)]
enum Command {
    /// The weight of files and folders, their tests left out: the total alone
    Scan(ScanArgs),
    /// The weight of the tests in files and folders (the test files, a Rust file's tests): the total alone
    ScanTests(ScanArgs),
    /// The net weight delta of a commit or between two revisions, its tests left out: the total alone
    Commit(CommitArgs),
    /// The net weight delta of a commit's tests or of the tests between two revisions: the total alone
    CommitTests(CommitArgs),
    /// The net weight delta of the uncommitted changes (the untracked files too) against HEAD, their tests left
    /// out: the total alone
    PreCommit(PreCommitArgs),
    /// The net weight delta of the uncommitted changes' tests (the untracked files too) against HEAD: the total
    /// alone
    PreCommitTests(PreCommitArgs),
    /// Writes the default weights file
    Init {
        /// Where to write it
        #[arg(default_value = config::FILE_NAME)]
        file: PathBuf,
    },
}

/// The arguments of `scan` and `scan-tests`.
#[derive(Args)]
struct ScanArgs {
    /// A file or a folder, absolute or relative
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// The whole report: the total, the functions (or files), and the warnings
    #[arg(short, long)]
    verbose: bool,
}

/// The arguments of `commit` and `commit-tests`.
#[derive(Args)]
struct CommitArgs {
    /// A revision (against its first parent) or a range A..B
    rev: String,
    /// Any path inside the repo
    #[arg(short = 'C', default_value = ".")]
    repo: PathBuf,
    /// The whole report: the total, the functions (or files), and the warnings
    #[arg(short, long)]
    verbose: bool,
}

/// The arguments of `pre-commit` and `pre-commit-tests`.
#[derive(Args)]
struct PreCommitArgs {
    /// Any path inside the repo
    #[arg(short = 'C', default_value = ".")]
    repo: PathBuf,
    /// Only the staged changes (the index against HEAD, as `git commit` would commit them)
    #[arg(long)]
    only_staged: bool,
    /// The whole report: the total, the functions (or files), and the warnings
    #[arg(short, long)]
    verbose: bool,
}

impl PreCommitArgs {
    /// The uncommitted changes these arguments ask for.
    fn changes(&self) -> Changes<'static> {
        Changes::Uncommitted {
            staged: self.only_staged,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// Runs the command line's command, its output to stdout.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Scan(a) => scan(&cli, &a.paths, a.verbose, false),
        Command::ScanTests(a) => scan(&cli, &a.paths, a.verbose, true),
        Command::Commit(a) => commit(&cli, Changes::Rev(&a.rev), &a.repo, a.verbose, false),
        Command::CommitTests(a) => commit(&cli, Changes::Rev(&a.rev), &a.repo, a.verbose, true),
        Command::PreCommit(a) => commit(&cli, a.changes(), &a.repo, a.verbose, false),
        Command::PreCommitTests(a) => commit(&cli, a.changes(), &a.repo, a.verbose, true),
        Command::Init { file } => init(file),
    }
}

/// Weighs the code at `paths`, or its tests (`tests`), each at its full weight.
fn scan(cli: &Cli, paths: &[PathBuf], verbose: bool, tests: bool) -> Result<()> {
    let config = load_config(cli, paths.first().map(PathBuf::as_path))?;
    let mut input = source::from_paths(paths, &config.files)?;
    input.keep(tests);
    let weighers = Weighers::new(&config);
    // In parallel, each file read when weighed; `collect` keeps the input's order.
    let files = input
        .items
        .into_par_iter()
        .map(|item| {
            let weighed = weighers.weigh(item.lang, &item.text()?, item.test, &item.path)?;
            Ok(Weighted {
                path: item.path,
                weighed: weighed.only(tests),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let report = cli.report(verbose);
    print!(
        "{}",
        match cli.format {
            Format::Text if report => {
                report::scan_text(&files, &input.unreadable, cli.by(), cli.top)
            }
            Format::Text => {
                eprint!("{}", report::scan_warnings(&files, &input.unreadable));
                report::scan_number(&files)
            }
            Format::Json => {
                report::scan_json(&files, &input.unreadable, cli.by(), cli.top)
            }
        }
    );
    Ok(())
}

/// The changes a delta is of.
#[derive(Clone, Copy)]
enum Changes<'a> {
    /// A commit or a range.
    Rev(&'a str),
    /// The uncommitted changes, all of them or the staged ones only.
    Uncommitted { staged: bool },
}

/// Weighs the net delta of a commit, a range or the uncommitted changes: of the code, or of its tests (`tests`),
/// each at its full weight.
fn commit(cli: &Cli, changes: Changes, repo: &Path, verbose: bool, tests: bool) -> Result<()> {
    let config = load_config(cli, Some(repo))?;
    let range = match changes {
        Changes::Rev(rev) => git::resolve(repo, rev)?,
        Changes::Uncommitted { staged } => git::uncommitted(repo, staged)?,
    };
    // The uncommitted changes' tree is no commit of the repo's.
    let to = matches!(changes, Changes::Rev(_)).then_some(range.to.as_str());
    let changes = source::from_commit(repo, &range, &config.files, tests)?;
    let weighers = Weighers::new(&config);
    let side = |blob: Option<source::Blob>| -> Result<Option<(PathBuf, Weighed)>> {
        blob.map(|b| {
            let weighed = weighers.weigh(b.lang, &b.text, b.test, &b.path)?;
            Ok((b.path, weighed.only(tests)))
        })
        .transpose()
    };
    let changes = changes
        .into_par_iter()
        .map(|change| {
            let before = side(change.before)?;
            let after = side(change.after)?;
            let (path, old_path) = match (&before, &after) {
                (Some((old, _)), Some((new, _))) if old != new => (new.clone(), Some(old.clone())),
                (_, Some((path, _))) | (Some((path, _)), None) => (path.clone(), None),
                (None, None) => bail!("a change with neither side"),
            };
            Ok(Changed {
                path,
                old_path,
                before: before.map(|(_, w)| w),
                after: after.map(|(_, w)| w),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    print!(
        "{}",
        match cli.format {
            Format::Text if cli.report(verbose) => {
                report::commit_text(&changes, cli.by(), cli.top)
            }
            Format::Text => {
                eprint!("{}", report::commit_warnings_text(&changes));
                report::commit_number(&changes)
            }
            Format::Json => {
                let revisions = Revisions {
                    from: range.from.as_deref(),
                    to,
                };
                report::commit_json(&changes, &revisions, cli.by(), cli.top)
            }
        }
    );
    Ok(())
}

/// The weights file found for a path (`inside`: the first scanned, or the commit's repo), loaded.
fn load_config(cli: &Cli, inside: Option<&Path>) -> Result<Config> {
    let cwd = std::env::current_dir().context("reading the current folder")?;
    let found = config::find(cli.config.as_deref(), &cwd, inside)?;
    Config::load(found.as_deref())
}

/// A weigher per language.
struct Weighers<'c> {
    weighers: Vec<Weigher<'c>>,
}

impl<'c> Weighers<'c> {
    fn new(config: &'c Config) -> Weighers<'c> {
        Weighers {
            weighers: lang::ALL.iter().map(|l| Weigher::new(l, config)).collect(),
        }
    }

    /// A text of `lang` weighed, a test file's (`test`) all test code.
    fn weigh(&self, lang: &'static Lang, text: &str, test: bool, path: &Path) -> Result<Weighed> {
        let weigher = self
            .weighers
            .iter()
            .find(|w| std::ptr::eq(w.lang(), lang))
            .context("a file of a language with no weigher")?;
        let weighed = weigher
            .weigh(text)
            .with_context(|| format!("weighing {}", path.display()))?;
        Ok(if test { weighed.all_tests() } else { weighed })
    }
}

fn init(file: &Path) -> Result<()> {
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .with_context(|| {
            format!(
                "creating {} (an existing file is not overwritten)",
                file.display()
            )
        })?;
    out.write_all(config::DEFAULTS.as_bytes())
        .with_context(|| format!("writing {}", file.display()))?;
    println!("wrote {}", file.display());
    Ok(())
}
