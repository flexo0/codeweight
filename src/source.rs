//! The inputs as `(path, language, text)` items: the files named on the command line. (Folder walks come with
//! milestone 4, git blobs at a revision with milestone 5.)

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::lang::{self, Lang};

/// A file to weigh.
pub struct Item {
    pub path: PathBuf,
    pub lang: &'static Lang,
    pub text: String,
}

/// The files to weigh, and those skipped as in no language codeweight reads.
pub struct Input {
    pub items: Vec<Item>,
    pub skipped: Vec<PathBuf>,
}

/// The files at `paths`, each as given.
pub fn from_paths(paths: &[PathBuf]) -> Result<Input> {
    let mut input = Input {
        items: Vec::new(),
        skipped: Vec::new(),
    };
    for path in paths {
        let meta = fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
        if meta.is_dir() {
            bail!(
                "{} is a folder: scanning folders is not built yet",
                path.display()
            );
        }
        match lang::for_path(path) {
            Some(lang) => input.items.push(Item {
                path: path.clone(),
                lang,
                text: read(path)?,
            }),
            None => input.skipped.push(path.clone()),
        }
    }
    Ok(input)
}

/// A file's text, lossy where it isn't UTF-8.
fn read(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    })
}
