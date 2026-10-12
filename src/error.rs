//! The crate error type: everything scanning, parsing, merging or
//! deserializing can fail with.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while scanning, parsing, merging or deserializing.
#[derive(Debug)]
pub enum Error {
    /// Reading `path` (a file, or a folder while scanning) failed.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A source path does not exist.
    NotFound(PathBuf),
    /// A file failed to parse.
    Parse { path: PathBuf, message: String },
    /// A table row failed to convert (csv, a spreadsheet sheet, or rows
    /// handed to [`parse_table`](crate::parse_table)). `row` is 1-based —
    /// for a sheet, the real spreadsheet row number.
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
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
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

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The `map_err` adapter for an I/O call on `path`.
pub(crate) fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
