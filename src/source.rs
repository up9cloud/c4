//! Config sources: folders, single files, in-code strings (built-in or
//! custom format) and in-code serde values.

use std::path::PathBuf;

use crate::options::FormatKind;
use crate::{Value, ser};

/// One config source. Sources merge in order — later overrides earlier.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Folder(PathBuf),
    File(PathBuf),
    /// An in-code string; its format may be a built-in or a
    /// [`CustomFormat`] (no `formats` registration needed — the source
    /// names its parser directly).
    String {
        format: FormatKind,
        content: String,
    },
    /// An in-code serde value ([`Source::value`]) — already converted to
    /// a [`Value`], or the serialization error to report at load time.
    Value(std::result::Result<Value, String>),
}

impl Source {
    /// Accepts anything path-like: `&str`, `String`, `&Path`, `PathBuf`.
    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Source::Folder(path.into())
    }

    /// Accepts anything path-like: `&str`, `String`, `&Path`, `PathBuf`.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Source::File(path.into())
    }

    /// An in-code config string — the hook for overriding values
    /// manually. Takes a [`Format`] or a [`CustomFormat`].
    pub fn string(format: impl Into<FormatKind>, content: impl Into<String>) -> Self {
        Source::String {
            format: format.into(),
            content: content.into(),
        }
    }

    /// An in-code serde value — typed overrides without going through a
    /// format parser (works with any `#[derive(Serialize)]` type, no
    /// format feature needed). A serialization failure (e.g. a map with
    /// non-string keys) surfaces at load time; a `Null` root (e.g.
    /// `None`) contributes nothing, like every other source.
    ///
    /// ```
    /// #[derive(serde::Serialize)]
    /// struct Overrides {
    ///     debug: bool,
    /// }
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         sources: vec![c4::Source::value(Overrides { debug: true })],
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert_eq!(value["debug"].as_bool(), Some(true));
    /// # Ok(())
    /// # }
    /// ```
    pub fn value(value: impl serde::Serialize) -> Self {
        Source::Value(
            value
                .serialize(ser::ValueSerializer)
                .map_err(|e| e.to_string()),
        )
    }
}
