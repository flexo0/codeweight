//! The command line: its arguments and the commands.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
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
    /// Only the N heaviest lines of the report
    #[arg(long, global = true, value_name = "N")]
    top: Option<usize>,
    /// What the report lists
    #[arg(long, global = true, value_enum, default_value_t = By::Function)]
    by: By,
}

#[derive(Subcommand)]
enum Command {
    /// Weighs files and folders
    Scan {
        /// A file or a folder, absolute or relative
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// The net weight delta of a commit, or between two revisions
    Commit {
        /// A revision (against its first parent) or a range A..B
        rev: String,
        /// Any path inside the repo
        #[arg(short = 'C', default_value = ".")]
        repo: PathBuf,
    },
    /// Writes the default weights file
    Init {
        /// Where to write it
        #[arg(default_value = config::FILE_NAME)]
        file: PathBuf,
    },
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
        Command::Scan { paths } => scan(&cli, paths),
        Command::Commit { rev, repo } => commit(&cli, rev, repo),
        Command::Init { file } => init(file),
    }
}

fn scan(cli: &Cli, paths: &[PathBuf]) -> Result<()> {
    let config = load_config(cli, paths.first().map(PathBuf::as_path))?;
    let input = source::from_paths(paths, &config.files)?;
    let weighers = Weighers::new(&config);
    // In parallel, each file read when weighed; `collect` keeps the input's order.
    let files = input
        .items
        .into_par_iter()
        .map(|item| {
            let weighed = weighers.weigh(item.lang, &item.text()?, item.test, &item.path)?;
            Ok(Weighted {
                path: item.path,
                weighed,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    print!(
        "{}",
        match cli.format {
            Format::Text => report::scan_text(&files, &input.skipped, cli.by, cli.top),
            Format::Json => report::scan_json(&files, &input.skipped, cli.by, cli.top),
        }
    );
    Ok(())
}

fn commit(cli: &Cli, rev: &str, repo: &Path) -> Result<()> {
    let config = load_config(cli, Some(repo))?;
    let range = git::resolve(repo, rev)?;
    let input = source::from_commit(repo, &range, &config.files)?;
    let weighers = Weighers::new(&config);
    let side = |blob: Option<source::Blob>| -> Result<Option<(PathBuf, Weighed)>> {
        blob.map(|b| {
            let weighed = weighers.weigh(b.lang, &b.text, b.test, &b.path)?;
            Ok((b.path, weighed))
        })
        .transpose()
    };
    let changes = input
        .changes
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
            Format::Text => report::commit_text(&changes, &input.skipped, cli.by, cli.top),
            Format::Json => {
                let revisions = Revisions {
                    from: range.from.as_deref(),
                    to: &range.to,
                };
                report::commit_json(&changes, &input.skipped, &revisions, cli.by, cli.top)
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
    test_factor: f64,
}

impl<'c> Weighers<'c> {
    fn new(config: &'c Config) -> Weighers<'c> {
        Weighers {
            weighers: lang::ALL.iter().map(|l| Weigher::new(l, config)).collect(),
            test_factor: config.files.test_factor,
        }
    }

    /// A text of `lang` weighed, a test's times the test factor.
    fn weigh(&self, lang: &'static Lang, text: &str, test: bool, path: &Path) -> Result<Weighed> {
        let weigher = self
            .weighers
            .iter()
            .find(|w| std::ptr::eq(w.lang(), lang))
            .context("a file of a language with no weigher")?;
        let weighed = weigher
            .weigh(text)
            .with_context(|| format!("weighing {}", path.display()))?;
        Ok(if test {
            weighed.scaled(self.test_factor)
        } else {
            weighed
        })
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
