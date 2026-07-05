//! The crate error type: everything scanning, parsing, merging or
//! deserializing can fail with.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while scanning, parsing, merging or deserializing.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O failure.
    Io(std::io::Error),
    /// A source path does not exist.
    NotFound(PathBuf),
    /// A file failed to parse.
    Parse { path: PathBuf, message: String },
    /// A table (csv) cell failed to convert. `row` is 1-based; the header
    /// row, when enabled, counts as row 1.
    Table {
        path: PathBuf,
        row: usize,
        message: String,
    },
    /// The merged value could not deserialize into the requested type.
    Deserialize(String),
    /// An option requires a Cargo feature that is not compiled in.
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::NotFound(p) => write!(f, "source not found: {}", p.display()),
            Error::Parse { path, message } => write!(f, "{}: {message}", path.display()),
            Error::Table { path, row, message } => {
                write!(f, "{} row {row}: {message}", path.display())
            }
            Error::Deserialize(m) => write!(f, "deserialize error: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported: {m}"),
        }
    }
}

impl std::error::Error for Error {}
