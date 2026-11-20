//! codeweight: weighs code, every element by its weight and heavier when nested.

mod cli;
mod config;
mod engine;
mod git;
mod lang;
mod markdown;
mod report;
mod source;

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        // Its reader quit early (`| head`): what it read is all it wanted.
        Err(error) if error.is::<cli::Closed>() => ExitCode::SUCCESS,
        // A runtime error: 1, so a script tells it from a usage error, which clap ends with 2.
        Err(error) => {
            // Not `eprintln!`: it panics when stderr is closed.
            let _ = writeln!(std::io::stderr(), "error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
