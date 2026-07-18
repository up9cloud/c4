//! Load config from folders, files and in-code strings/values into one
//! deep-merged [`Value`]. File formats (jsonc, yaml, json, toml, ini, env,
//! csv, excel, ods) are individually Cargo-feature-gated, the table
//! formats' cells (csv and spreadsheet sheets) carry typed values, and
//! every value can be traced to the source it came from.
//!
//! What c4 offers is less a parser than a **convention** (in the spirit of
//! node-config): deterministic override order, deep merge, and a
//! `key,value[,format]` table rule that lets non-programmers own config in
//! CSV or Excel/OpenDocument spreadsheets — plus documented custom-format
//! escape hatches for everything else. If all you need is one simple file
//! read once, bespoke zero-dependency loading code (your AI writes it) is
//! lighter than any library; c4 earns its place when a team needs shared
//! rules.
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
//! a single file — with default options (default format: jsonc). The
//! annotated target type decides what you get: a dynamic [`Value`] or your
//! own serde type.
//!
//! ```no_run
//! # fn main() -> Result<(), c4::Error> {
//! let value: c4::Value = c4::load("config")?;   // a folder …
//! let one: c4::Value = c4::load("app.json")?;   // … or a single file
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
//! `(format, text)` tuple is an in-code string source, a **1-tuple**
//! `(value,)` wraps any serde type as a typed override (the trailing comma
//! is what keeps it distinct — see [`Source`]), and a
//! `(format, path, layout)` / `(format, path, sheet, layout)` tuple is a
//! **table source** — one csv/excel/ods file read under an explicit
//! [`TableLayout`]; naming a spreadsheet sheet is always the 4-tuple
//! (see *Table formats* below).
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
//!             (Format::Csv, "./items.csv", "db").into(),      // table source: file + layout
//!             (Format::Excel, "./game.xlsx", "drops", "db").into(), // sheet + layout (4-tuple)
//!         ],
//!         // which formats read which extensions; the last claimer wins.
//!         // all conversion forms — id/Format × default/custom exts,
//!         // optionally + a table layout for every file the entry claims:
//!         formats: vec![
//!             "jsonc".into(),                                 // format id, default extensions
//!             Format::Toml.into(),                            // Format, default extensions
//!             (Format::Yaml, ["yml", "yaml", "conf"]).into(), // Format + custom extensions
//!             ("jsonc", ["json", "jsonc"]).into(),            // format id + custom extensions
//!             (Format::Csv, ["csv"], "db").into(),            // + layout: all csv = record grids
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
//!     c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
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
//! like `"alphabetic".into()`), the tree-mode trio `tree`,
//! `auto_files`, `ignore_unknown_ext`, and the spreadsheet pair
//! `ignore_sheet_prefix`, `ignore_hidden_sheets`.
//!
//! # Cargo features
//!
//! `default = ["jsonc"]` — deliberately light, one parser dependency; at
//! least one **format** feature is required (a compile error otherwise).
//! Value-parser features are pure std (no extra dependencies). Cargo
//! unions features across the whole build graph, so only applications
//! should turn format features on.
//!
//! | Feature | Enables |
//! | ------- | ------- |
//! | `jsonc` *(default)* | JSONC files (comments + trailing commas); also the `jsonc` table cell |
//! | `yaml` | YAML files |
//! | `json` | strict JSON files; also the table `json` cell |
//! | `toml` / `ini` / `env` / `csv` | those file formats |
//! | `excel` | Excel workbooks (`.xlsx`/`.xlsm`/`.xlsb`/`.xls`) |
//! | `ods` | OpenDocument spreadsheets (`.ods`) |
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
//! # File formats
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
//! | `yaml`  | `.yml`, `.yaml`    | `yaml`  |
//! | `json`  | `.json` (strict)   | `json`  |
//! | `toml`  | `.toml`            | `toml`  |
//! | `ini`   | `.ini`             | `ini`   |
//! | `env`   | `.env`, `*.env`    | `env`   |
//! | `csv`   | `.csv`             | `csv`   |
//! | `excel` | `.xlsx`, `.xlsm`, `.xlsb`, `.xls` | `excel` |
//! | `ods`   | `.ods`             | `ods`   |
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
//! Table files (csv, and each spreadsheet sheet) parse under a
//! [`TableLayout`]. Each format has a default —
//! [`Format::default_layout`]: **`kv` for csv, `db` for excel/ods**
//! (spreadsheets are grids) — overridable per source with a table-source
//! tuple or per extension in `formats`:
//!
//! - **`kv`** (csv default): `key,value[,format]` rows, one config entry
//!   per row.
//! - **`db`** (spreadsheet default): a record grid — row 1 holds the
//!   keys, row 2 always the
//!   type ids (empty cell = `auto`), every following row is one record;
//!   the file parses to an **array** of objects, so a config sheet full
//!   of game items deserializes straight into a `Vec<Item>`. Empty cells
//!   are omitted from their record. The type row is positional — the
//!   row right after the keys is the type row even when it is entirely
//!   blank (= every column `auto`); it is never skipped as a blank row.
//! - **a [`CustomLayout`]**: your callback over the raw rows — reshape
//!   them and call [`parse_table`]. A db grid
//!   *without* a type row is the canonical case: insert a row of `auto`
//!   cells after the header and delegate to `db` (see the `xlsx-sheets`
//!   example).
//!
//! ```text
//! sources: vec![
//!     (Format::Csv,   "items.csv", "db").into(),          // layout per file
//!     (Format::Excel, "game.xlsx", "config", "kv").into(),// sheet + layout (4-tuple)
//! ]
//! formats: vec![
//!     (Format::Csv, ["csv"], "db").into(),   // or per extension: every
//! ]                                          // claimed csv file is a grid
//! ```
//!
//! The 3-tuple's third element is **always the layout** (it suits csv,
//! where the file is the table); naming a spreadsheet sheet is always
//! the 4-tuple, which names the layout explicitly too. Two db sources
//! do not concatenate: arrays replace like any array.
//!
//! Rows map keys to values **positionally** (in `kv`: col 0 = key,
//! col 1 = value, col 2 = format; in `db`: the type row types its
//! column). The **format** of a cell is one of the ids below — the
//! column/row is optional, and a missing or empty format means `auto`:
//!
//! | Cell format | Aliases | Cargo feature | Notes |
//! | ----------- | ------- | ------------- | ----- |
//! | `auto` *(or empty)* | — | — | guesses bool → date/time/dt → uuid → mac → ip → integer → float → string, using only compiled-in types; never widens past `i64`/`u64`; leading-zero numbers stay strings |
//! | `i8` `i16` `i32` `i64` | `int`, `integer` = `i64` | — | signed integers |
//! | `u8` `u16` `u32` `u64` | `uint` = `u64` | — | unsigned integers |
//! | `i128` `u128` | — | — | explicit-only — `auto` never guesses them |
//! | `f32` `f64` | `float`, `double`, `number` = `f64` | — | floats (`f32` rounds through f32 precision) |
//! | `bool` | `boolean` | — | also accepts `t/f`, `yes/no`, `y/n`, `on/off`, `1/0` (case-insensitive); `auto` only accepts `true`/`false` |
//! | `str` | `string`, `text` | — | the cell text as-is |
//! | `dt` | `datetime` | `datetime` | `YYYY-MM-DD`, optional time part |
//! | `date`, `time` | — | same-named | `YYYY-MM-DD` / `hh:mm:ss[.frac]` |
//! | `ipv4`, `ipv6` | — | same-named | IP addresses |
//! | `inet`, `cidr` | — | `inet`, `cidr` | PostgreSQL inet/cidr shapes (optional/required netmask) |
//! | `macaddr`, `macaddr8` | — | same-named | the PostgreSQL MAC spellings |
//! | `uuid` | — | `uuid` | hyphenated or bare 32-hex |
//! | `json`, `jsonc` | — | same-named | a whole document in one cell, parsed by exactly that format's parser |
//! | `array<sep><format>` | — | — | splits the cell into a flat list by `sep` (default `,`), each element parsed by `format` (default `auto`): `array\|` on `1\|2\|3` → `[1,2,3]`, `array\|u8` types them `u8`, `array\|str` keeps `["1","2","3"]` |
//! | `csv<sep><layout>` | — | `csv` | parses the whole cell as a CSV document (delimiter `sep`, default `,`) under `layout` (default `kv`): `csv,kv` → object, `csv,db` → array of objects |
//!
//! `array` and `csv` are explicit-only (like `json`/`jsonc`) — `auto`
//! never produces a list. `array` is always compiled (a native split);
//! `csv` needs the `csv` feature. Both take an optional, **positional**
//! suffix after the separator: `array<sep><format>` applies one type id
//! to every element, and `csv<sep><layout>` picks the inner layout — in
//! each case naming the suffix means writing the separator too
//! (`array,i8`, `csv,db`), and the separator is a single character
//! (ASCII for `csv`). `array<sep><format>` types **every** element the
//! same; when a list's elements need **different** formats, use a `csv`
//! cell instead (one `key,value,format` row each: `a,1,i8` / `b,2,i16`).
//! Only the built-in layout ids `kv`/`db` are nameable in `csv`.
//!
//! An id whose feature is off is an unknown format and fails the row;
//! only `auto` degrades (it just stops guessing). The `numeric` feature
//! extends every numeric cell with `0x`/`0o`/`0b` radixes, `_`
//! separators and BigInt-style `123n`.
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
//!     c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
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
//! # Spreadsheet formats
//!
//! `excel` (`.xlsx`/`.xlsm`/`.xlsb`/`.xls`) and `ods` (`.ods`)
//! read workbooks as table formats: each sheet is the same positional
//! `key,value[,format]` table as csv, anchored at cell A1 (column A =
//! key, B = value, C = optional type id; error messages carry real
//! spreadsheet row numbers). Numbers, booleans and date-formatted cells
//! convert to the text the table stage expects, so a `dt`/`date`/`time`
//! format column works on real spreadsheet dates.
//!
//! Sheets parse as **db record grids by default**
//! ([`Format::default_layout`]) — name a layout in a table source or a
//! `formats` entry for anything else. Which sheets are read:
//!
//! - `tree: false` (default): exactly the sheet named `config`; every
//!   other sheet is ignored, and a workbook without a `config` sheet
//!   contributes nothing — so extra sheets are free working space.
//! - `tree: true`: every sheet becomes a key under the file's key —
//!   `a/b.xlsx` with sheets `c`, `d` loads as `{a: {b: {c: …, d: …}}}`.
//! - Sheets whose name starts with `#`, `.` or `_` are skipped
//!   ([`Options::ignore_sheet_prefix`]), and so are sheets hidden in the
//!   workbook ([`Options::ignore_hidden_sheets`]) — both default `true`,
//!   giving planners draft/scratch space next to live config.
//!
//! - A **table source that names a sheet** —
//!   `(Format::Excel, "game.xlsx", "items", "db").into()` — reads exactly
//!   that sheet (even a prefixed/hidden one; a missing sheet is an
//!   error) and merges it **under the sheet name as key**, so several
//!   sources can each give one sheet of the same workbook its own
//!   layout. The `xlsx-sheets` example composes five sheets this way.
//!
//! Both are binary formats, so they load from files only — a
//! `(Format::Excel, text)` string source is an error.
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
    feature = "csv",
    feature = "excel",
    feature = "ods"
)))]
compile_error!(
    "c4: enable at least one format feature (json, jsonc, yaml, toml, ini, env, csv, excel, ods)"
);

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
    CustomFormat, CustomLayout, Format, FormatKind, FormatSpec, Options, Order, TableLayout,
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
    Loader::new(Options {
        sources: vec![Source::Path(path.into())],
        ..Options::default()
    })
    .load()
}

/// The generic table stage: interpret a plain row table under an
/// explicit [`TableLayout`] — `&TableLayout::Kv` for
/// `key,value[,format]` rows (what the csv format feeds by default),
/// `&TableLayout::Db` for a record grid, or a [`CustomLayout`] callback.
/// Public so custom table-shaped formats (markdown tables, …) and custom
/// layouts can lower their input into rows and reuse it; `path` labels
/// errors. The layout is always passed — there is deliberately no
/// defaulting variant.
pub fn parse_table(
    rows: Vec<Vec<String>>,
    layout: &TableLayout,
    path: &Path,
    options: &Options,
) -> Result<Value> {
    format::table::parse(rows, layout, path, options)
}
