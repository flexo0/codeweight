//! codeweight: weighs code, every element by its weight and heavier when nested.

mod cli;
mod config;
mod engine;
mod git;
mod lang;
mod report;
mod source;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}
