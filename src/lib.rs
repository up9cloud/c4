//! Load a folder of config files into one deep-merged [`Value`].
//!
//! `README.md` is the user-facing overview and `CLAUDE.md` the complete
//! spec; the README examples are mirrored here as doc tests. Examples
//! that read the caller's working directory (`./config`, `/etc/myapp`)
//! are `no_run` — they compile but depend on the user's filesystem; the
//! [`CustomFormat`] example is fully runnable via a string source.
//!
//! Simplest form — one generic [`load`] taking the path of a folder (or
//! a single file), default options (formats: jsonc + yaml). The
//! annotated target type decides whether you get a dynamic [`Value`] or
//! your own struct:
//!
//! ```no_run
//! # fn main() -> Result<(), c4::Error> {
//! let value: c4::Value = c4::load("config")?;
//! # Ok(())
//! # }
//! ```
//!
//! Advanced — explicit sources (later overrides earlier) and options:
//!
//! ```no_run
//! use std::path::Path;
//!
//! use c4::{Format, Loader, Options, Source};
//!
//! # fn main() -> Result<(), c4::Error> {
//! let value: c4::Value = Loader::new(Options {
//!         sources: vec![
//!             Source::folder("./config"),
//!             Source::folder(Path::new("/etc/myapp")), // any path-like type
//!             Source::file("./local.yml"),
//!             Source::string(Format::Jsonc, r#"{ "debug": true }"#),
//!         ],
//!         recursive: true,
//!         formats: vec![
//!             "jsonc".into(),                              // format id, default extensions
//!             Format::Toml.into(),                         // enum form, default extensions
//!             (Format::Yaml, ["yml", "yaml", "conf"]).into(), // custom extensions
//!             ("jsonc", ["json", "jsonc"]).into(),         // string id + custom extensions
//!         ],
//!         ..Options::default()
//!     })
//!     .load()?;
//! # Ok(())
//! # }
//! ```

#[cfg(not(any(
    feature = "json",
    feature = "jsonc",
    feature = "yaml",
    feature = "toml",
    feature = "ini",
    feature = "env",
    feature = "csv"
)))]
compile_error!("c4: enable at least one format feature (json, jsonc, yaml, toml, ini, env, csv)");

mod de;
mod error;
mod format;
mod loader;
mod options;
mod ser;
mod source;
mod trace;
pub(crate) mod valid;
mod value;

pub use error::{Error, Result};
pub use loader::Loader;
pub use options::{
    CustomFormat, Format, FormatKind, FormatSpec, Options, Order, TableColumns, TableOptions,
    TableTypes,
};
pub use source::Source;
pub use trace::{SourceRef, TracedValue};
pub use value::Value;

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// Load one path with default options — the shortest way in. An
/// existing file loads as a single file source; anything else is
/// treated as a folder (a missing path is [`Error::NotFound`]). The
/// annotated target type decides what you get back:
///
/// ```no_run
/// #[derive(serde::Deserialize)]
/// struct MyConfig {
///     host: String,
///     port: u16,
/// }
///
/// # fn main() -> Result<(), c4::Error> {
/// let value: c4::Value = c4::load("config")?;   // a folder …
/// let value: c4::Value = c4::load("app.yml")?;  // … or one file
/// let cfg: MyConfig = c4::load("config")?;
/// # Ok(())
/// # }
/// ```
pub fn load<T: DeserializeOwned>(path: impl Into<PathBuf>) -> Result<T> {
    let path = path.into();
    let source = if path.is_file() {
        Source::File(path)
    } else {
        Source::Folder(path)
    };
    Loader::new(Options {
        sources: vec![source],
        ..Options::default()
    })
    .load()
}

/// The generic table stage: interpret a plain row table
/// (`[[key, value, format], …]`) with the `options.table` semantics — the
/// same stage the csv format feeds. Public so custom table-shaped formats
/// (spreadsheets, markdown tables, …) can lower their file into rows and
/// reuse it; `path` labels errors.
pub fn parse_table(rows: Vec<Vec<String>>, path: &Path, options: &Options) -> Result<Value> {
    format::table::parse(rows, path, options)
}
