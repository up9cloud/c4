//! Load config from folders, files and in-code strings/values into one
//! deep-merged [`Value`]. Formats (jsonc, yaml, json, toml, ini, env, csv)
//! are individually Cargo-feature-gated, the table (csv) format's cells
//! carry typed values, and every value can be traced to the source it came
//! from.
//!
//! The [README] is the quick start; this page is the full reference (and
//! `CLAUDE.md` in the repo is the exhaustive spec). Most examples below run
//! without any files — they load in-code string and value sources; only
//! the `load("config")` snippets, which read the caller's filesystem, are
//! marked `no_run`.
//!
//! [README]: https://github.com/up9cloud/c4#readme
//!
//! # Quick start
//!
//! One generic [`load`] takes a path — a folder whose files deep-merge, or
//! a single file — with default options (formats: jsonc + yaml). The
//! annotated target type decides what you get: a dynamic [`Value`] or your
//! own serde type.
//!
//! ```no_run
//! # fn main() -> Result<(), c4::Error> {
//! let value: c4::Value = c4::load("config")?;   // a folder …
//! let one: c4::Value = c4::load("app.yml")?;    // … or a single file
//! let host = value["db"]["host"].as_str().unwrap_or("localhost");
//! # Ok(())
//! # }
//! ```
//!
//! For more than one source, or any non-default option, build a [`Loader`]
//! from [`Options`], as the next section shows.
//!
//! # Sources and options
//!
//! Everything goes through one plain-data [`Options`]; [`Loader`] has
//! exactly [`new`](Loader::new), [`load`](Loader::load) and
//! [`trace`](Loader::trace). You never name [`Source`]: `sources` is a
//! `Vec<Source>` and each element converts with `.into()` — a path-like
//! value (`&str`/`String`/`&Path`/`PathBuf`) is a folder/file source, a
//! `(format, text)` tuple is an in-code string source, and a **1-tuple**
//! `(value,)` wraps any serde type as a typed override (the trailing comma
//! is what keeps it distinct — see [`Source`]).
//!
//! ```no_run
//! use std::path::Path;
//!
//! use c4::{Format, Loader, Options};
//!
//! #[derive(serde::Serialize)]
//! struct Overrides {
//!     debug: bool,
//! }
//!
//! # fn main() -> Result<(), c4::Error> {
//! let value: c4::Value = Loader::new(Options {
//!         sources: vec![
//!             "./config".into(),                              // a folder (or a single file)
//!             Path::new("/etc/myapp").into(),                 // any path-like type
//!             "./local.yml".into(),
//!             ("jsonc", r#"{ "note": "from code" }"#).into(), // string source, by format id
//!             (Format::Toml, "debug = true").into(),          // string source, by Format
//!             (Overrides { debug: true },).into(),            // typed override (1-tuple)
//!         ],
//!         // which formats read which extensions; the last claimer wins.
//!         // all four conversion forms — id/Format × default/custom exts:
//!         formats: vec![
//!             "jsonc".into(),                                 // format id, default extensions
//!             Format::Toml.into(),                            // Format, default extensions
//!             (Format::Yaml, ["yml", "yaml", "conf"]).into(), // Format + custom extensions
//!             ("jsonc", ["json", "jsonc"]).into(),            // format id + custom extensions
//!         ],
//!         recursive: true,
//!         ..Options::default()
//!     })
//!     .load()?;
//! # Ok(())
//! # }
//! ```
//!
//! Sources merge in the order given — later overrides earlier — whatever
//! their kind:
//!
//! ```
//! use c4::{CustomFormat, Loader, Options};
//!
//! # fn main() -> Result<(), c4::Error> {
//! // a tiny `key value type` table format, lowered to `parse_table`
//! let kv = CustomFormat::new("kv", ["kv"], |text, path, options| {
//!     let rows = text
//!         .lines()
//!         .filter(|l| !l.trim().is_empty())
//!         .map(|l| l.split_whitespace().map(str::to_owned).collect())
//!         .collect();
//!     c4::parse_table(rows, path, options)
//! });
//! let value: c4::Value = Loader::new(Options {
//!         sources: vec![
//!             (kv.clone(), "port 1 u16").into(),                            // earlier …
//!             (kv, "port 8080 u16").into(),                                 // … overridden
//!             (std::collections::BTreeMap::from([("debug", true)]),).into(),
//!         ],
//!         ..Options::default()
//!     })
//!     .load()?;
//! assert_eq!(value["port"].as_u64(), Some(8080));
//! assert_eq!(value["debug"].as_bool(), Some(true));
//! # Ok(())
//! # }
//! ```
//!
//! Every [`Options`] field is documented on the struct. In brief:
//! `sources`, `formats`, `recursive`, `flat` (merge-mode subfolder
//! nesting), `dot_key`, `case_sensitive`, `order` (an [`Order`] or an id
//! like `"alphabetic".into()`), and the tree-mode trio `tree`,
//! `auto_files`, `ignore_unknown_ext`.
//!
//! # Cargo features
//!
//! `default = ["jsonc", "yaml"]`; at least one **format** feature is
//! required (a compile error otherwise). Value-parser features are pure
//! std (no extra dependencies). Cargo unions features across the whole
//! build graph, so only applications should turn format features on.
//!
//! | Feature | Enables |
//! | ------- | ------- |
//! | `jsonc` *(default)* | JSONC files (comments + trailing commas) |
//! | `yaml` *(default)* | YAML files |
//! | `json` | strict JSON files; also the table `json` cell |
//! | `toml` / `ini` / `env` / `csv` | those file formats |
//! | `tree` | tree mode ([`Options::tree`]) |
//! | `datetime` (= `date` + `time`) | the `dt` table type |
//! | `date`, `time` | the `date` / `time` table types |
//! | `ipv4`, `ipv6` | the `ipv4` / `ipv6` table types |
//! | `inet` (= `ipv4` + `ipv6` + `cidr`), `cidr` | inet / cidr table types |
//! | `macaddr` (= `macaddr8`), `macaddr8` | MAC table types |
//! | `uuid` | the `uuid` table type |
//! | `numeric` | extended table numeric literals (`0x`, `_`, `123n`) |
//! | `cli` | the `c4` binary (implies all of the above) |
//!
//! # Formats
//!
//! Format and file extension are separate things: each format claims a set
//! of default extensions, and both sides are configurable. Extension
//! matching is case-insensitive; a hidden file `.X` counts as extension
//! `X` (so `.env` matches `env`). Unclaimed extensions are ignored, and the
//! **last** claimer of a contested extension wins — which is also how you
//! reassign one: `vec![Format::Yaml.into(), (Format::Jsonc, ["yml"]).into()]`
//! makes the jsonc parser read `.yml`. `jsonc` is a JSON superset that also
//! claims `.json`, so with the default `formats` enabling both `json` and
//! `jsonc` is redundant (jsonc, added last, wins `.json`).
//!
//! | Format  | Default extensions | Feature |
//! | ------- | ------------------ | ------- |
//! | `jsonc` | `.json`, `.jsonc`  | default |
//! | `yaml`  | `.yml`, `.yaml`    | default |
//! | `json`  | `.json` (strict)   | `json`  |
//! | `toml`  | `.toml`            | `toml`  |
//! | `ini`   | `.ini`             | `ini`   |
//! | `env`   | `.env`, `*.env`    | `env`   |
//! | `csv`   | `.csv`             | `csv`   |
//!
//! # Merge rules
//!
//! 1. Sources merge in the order given; later overrides earlier.
//! 2. Within a folder, entries load in `order` and deep-merge: objects
//!    merge recursively, arrays and scalars replace.
//! 3. Override order between files is decided by **filename alone**, never
//!    by format: `app.json` and `app.yml` sort by name, so `app.yml` loads
//!    later and wins. Prefix filenames (`00_a.json`, `01_a.yml`) for
//!    explicit control.
//!
//! # Tree mode
//!
//! With `Options { tree: true, .. }` (feature `tree`) a folder is not
//! merged — its shape becomes the config: every subfolder is a key, and
//! every file a key named after it (extension stripped):
//!
//! ```text
//! config/a/b.json = {"c": 1}   ->  { "a": { "b": { "c": 1 } },
//! config/d.json   = {"a": 123}       "d": { "a": 123 } }
//! ```
//!
//! Tree mode is always recursive; `order` decides key collisions (`a.yml`
//! next to a folder `a/`). Extension-less files are auto-detected from
//! their content when `auto_files` is set; files with unknown extensions
//! are skipped unless `ignore_unknown_ext` is `false`.
//!
//! # Table formats
//!
//! Table rows map to config entries **positionally** as `key,value[,format]`;
//! the format column is optional (missing or empty = `auto`). Type ids
//! `i8`–`u64`, `i128`/`u128`, `f32`/`f64`, `bool`, `str`, `auto` always
//! exist; `dt`/`date`/`time`/`ipv4`/`ipv6`/`inet`/`cidr`/`macaddr`/
//! `macaddr8`/`uuid` follow their features, and `json` (a whole JSON
//! document as one cell) needs `json` or `jsonc`. `auto` tries
//! bool → date/time/dt → uuid → mac → ip → integer → float → string, and
//! never widens past `i64`/`u64` (use an explicit `i128`/`u128` cell for
//! bigger integers). An explicit `bool` cell also accepts
//! `t/f`, `yes/no`, `y/n`, `on/off`, `1/0` (case-insensitive).
//!
//! The built-in `csv` format is headerless and positional; a header row,
//! renamed/reordered columns or a transposed layout are a [`CustomFormat`]
//! that reshapes the file into `key,value,format` rows and calls
//! [`parse_table`] (see the `csv-header` and `csv-transpose` examples):
//!
//! ```
//! use c4::{CustomFormat, Loader, Options};
//!
//! # fn main() -> Result<(), c4::Error> {
//! let table = CustomFormat::new("kv", ["kv"], |text, path, options| {
//!     let rows = text
//!         .lines()
//!         .filter(|l| !l.trim().is_empty())
//!         .map(|l| l.split_whitespace().map(str::to_owned).collect())
//!         .collect();
//!     c4::parse_table(rows, path, options)
//! });
//! let value: c4::Value = Loader::new(Options {
//!         sources: vec![(table, "name c4 str\nport 8080 u16\ndebug true bool").into()],
//!         ..Options::default()
//!     })
//!     .load()?;
//! assert_eq!(value["name"].as_str(), Some("c4"));
//! assert_eq!(value["port"].as_u64(), Some(8080));
//! assert_eq!(value["debug"].as_bool(), Some(true));
//! # Ok(())
//! # }
//! ```
//!
//! # Custom formats
//!
//! A [`CustomFormat`] is an id, the extensions it claims, and a parser
//! callback. It goes into `Options.formats` like any built-in — one list,
//! one claim order — and a string source can name one directly. See
//! [`CustomFormat`] for a full markdown-pipe-table example.
//!
//! # Provenance
//!
//! [`Loader::trace`] returns a [`TracedValue`] tree that keeps, per leaf,
//! the source it came from and the format it parsed as — the shape the CLI
//! `--trace` prints and the fixtures assert. See [`Loader::trace`] and
//! [`TracedValue`].

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
pub use options::{CustomFormat, Format, FormatKind, FormatSpec, Options, Order};
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
    Loader::new(Options {
        sources: vec![Source::Path(path.into())],
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
