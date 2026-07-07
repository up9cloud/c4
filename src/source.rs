//! Config sources: a filesystem path (folder or single file), an in-code
//! string (built-in or custom format), or an in-code serde value.
//!
//! You rarely name [`Source`] directly. Build the `sources` list as a
//! plain `Vec<Source>` and convert each element with `.into()` (the `From`
//! impls below): a path-like value is a folder/file source, a
//! `(format, text)` tuple is a string source, and a 1-tuple `(value,)` is
//! a typed serde override.

use std::path::{Path, PathBuf};

use crate::Value;
use crate::options::FormatKind;

/// One config source. Sources merge in order — later overrides earlier.
///
/// Construct sources by conversion, not by hand:
/// - a path-like value (`&str`, `String`, `&Path`, `PathBuf`) becomes
///   [`Source::Path`] — loaded as a folder or a single file, detected at
///   load time;
/// - a `(format, text)` tuple becomes [`Source::String`] — the format is a
///   [`Format`](crate::Format), a format-id string (`"jsonc"`), or a
///   [`CustomFormat`](crate::CustomFormat);
/// - a **1-tuple** `(value,)` wraps any `Serialize` type as a typed
///   override — the single-element tuple keeps it distinct from the
///   conversions above (a blanket `From<impl Serialize>` would overlap
///   them, since `&str`/`String`/tuples are all `Serialize`).
///
/// So one `Options.sources` `vec![…]` can mix all three kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A filesystem path — a folder whose files deep-merge, or a single
    /// file. Which one is decided at load time (`is_dir` / `is_file`); a
    /// path that is neither is [`Error::NotFound`](crate::Error::NotFound).
    Path(PathBuf),
    /// An in-code string; its format may be a built-in or a
    /// [`CustomFormat`](crate::CustomFormat) (no `formats` registration
    /// needed — the source names its parser directly).
    String { format: FormatKind, content: String },
    /// An in-code serde value (from a 1-tuple `(value,)` source) — already
    /// converted to a [`Value`], or the serialization error to report at
    /// load time.
    Value(std::result::Result<Value, String>),
}

impl From<&str> for Source {
    fn from(path: &str) -> Self {
        Source::Path(PathBuf::from(path))
    }
}

impl From<String> for Source {
    fn from(path: String) -> Self {
        Source::Path(PathBuf::from(path))
    }
}

impl From<&Path> for Source {
    fn from(path: &Path) -> Self {
        Source::Path(path.to_path_buf())
    }
}

impl From<PathBuf> for Source {
    fn from(path: PathBuf) -> Self {
        Source::Path(path)
    }
}

/// A `(format, text)` tuple is an in-code string source. `format` is a
/// [`Format`](crate::Format), a format-id `&str`, or a
/// [`CustomFormat`](crate::CustomFormat); `text` is anything `Into<String>`.
impl<F, S> From<(F, S)> for Source
where
    F: Into<FormatKind>,
    S: Into<String>,
{
    fn from((format, content): (F, S)) -> Self {
        Source::String {
            format: format.into(),
            content: content.into(),
        }
    }
}

/// A **1-tuple** `(value,)` is a typed serde override — the value is
/// serialized through the crate serializer (no format parser; works with
/// any `#[derive(Serialize)]` type). A serialization failure (e.g. a map
/// with non-string keys) surfaces at load time as
/// [`Error::Parse`](crate::Error::Parse); a `Null` root (e.g. `None`)
/// contributes nothing.
///
/// The single-element tuple is what keeps this distinct: `&str`, `String`,
/// tuples and paths are all themselves `Serialize`, so a blanket
/// `From<impl Serialize>` would overlap the conversions above — but a
/// 1-tuple `(T,)` is a type of its own. Mind the trailing comma:
///
/// ```
/// #[derive(serde::Serialize)]
/// struct Overrides {
///     debug: bool,
/// }
///
/// # fn main() -> Result<(), c4::Error> {
/// let value: c4::Value = c4::Loader::new(c4::Options {
///         sources: vec![(Overrides { debug: true },).into()],
///         ..c4::Options::default()
///     })
///     .load()?;
/// assert_eq!(value["debug"].as_bool(), Some(true));
/// # Ok(())
/// # }
/// ```
impl<T: serde::Serialize> From<(T,)> for Source {
    fn from((value,): (T,)) -> Self {
        Source::Value(
            value
                .serialize(crate::ser::ValueSerializer)
                .map_err(|e| e.to_string()),
        )
    }
}
