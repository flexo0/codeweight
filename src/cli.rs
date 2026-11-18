//! The command line: its arguments and the commands.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::{self, Config};
use crate::engine::Weigher;
use crate::lang;
use crate::report::{self, By, Weighted};
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
    /// Weighs files
    Scan {
        /// A file, absolute or relative
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
        Command::Commit { .. } => bail!("`commit` is not built yet"),
        Command::Init { file } => init(file),
    }
}

fn scan(cli: &Cli, paths: &[PathBuf]) -> Result<()> {
    if cli.format == Format::Json {
        bail!("the JSON output is not built yet");
    }
    let cwd = std::env::current_dir().context("reading the current folder")?;
    let found = config::find(
        cli.config.as_deref(),
        &cwd,
        paths.first().map(PathBuf::as_path),
    )?;
    let config = Config::load(found.as_deref())?;
    let input = source::from_paths(paths)?;
    let weighers: Vec<Weigher> = lang::ALL
        .iter()
        .map(|lang| Weigher::new(lang, &config))
        .collect();
    let mut files = Vec::new();
    for item in input.items {
        let weigher = weighers
            .iter()
            .find(|w| w.lang().name == item.lang.name)
            .context("a file of a language with no weigher")?;
        let weighed = weigher
            .weigh(&item.text)
            .with_context(|| format!("weighing {}", item.path.display()))?;
        files.push(Weighted {
            path: item.path,
            weighed,
        });
    }
    print!("{}", report::text(&files, &input.skipped, cli.by, cli.top));
    Ok(())
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
