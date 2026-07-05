//! The plain-data configuration surface: [`Options`] and every type its
//! fields are made of — formats and their specs (built-in and custom),
//! folder order, and the table options.

use std::path::Path;

use crate::{Result, Source, Value};

/// A config file format.
///
/// The variants always exist; whether a format can actually be parsed
/// depends on the matching Cargo feature. Which extensions a format reads
/// is part of [`FormatSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Json,
    Jsonc,
    Yaml,
    Toml,
    Ini,
    Env,
    Csv,
}

impl Format {
    /// Look a format up by its string id (`"json"`, `"jsonc"`, `"yaml"`,
    /// `"toml"`, `"ini"`, `"env"`, `"csv"`).
    pub fn from_id(id: &str) -> Option<Format> {
        Some(match id {
            "json" => Format::Json,
            "jsonc" => Format::Jsonc,
            "yaml" => Format::Yaml,
            "toml" => Format::Toml,
            "ini" => Format::Ini,
            "env" => Format::Env,
            "csv" => Format::Csv,
            _ => return None,
        })
    }

    /// The string id of this format.
    pub fn id(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Jsonc => "jsonc",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Ini => "ini",
            Format::Env => "env",
            Format::Csv => "csv",
        }
    }

    /// The extensions (without the dot) this format claims by default.
    ///
    /// Extension matching is always case-insensitive, and a hidden file
    /// named `.X` is treated as having extension `X` (so `.env` matches the
    /// `env` extension).
    pub fn default_extensions(self) -> &'static [&'static str] {
        match self {
            Format::Json => &["json"],
            Format::Jsonc => &["json", "jsonc"],
            Format::Yaml => &["yml", "yaml"],
            Format::Toml => &["toml"],
            Format::Ini => &["ini"],
            Format::Env => &["env"],
            Format::Csv => &["csv"],
        }
    }

    /// Pair this format with custom extensions.
    pub fn exts<I, S>(self, extensions: I) -> FormatSpec
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        FormatSpec {
            format: FormatKind::Builtin(self),
            extensions: extensions.into_iter().map(Into::into).collect(),
        }
    }
}

/// What a [`FormatSpec`] entry parses with: a built-in format or a
/// user-defined [`CustomFormat`].
#[derive(Debug, Clone, PartialEq)]
pub enum FormatKind {
    Builtin(Format),
    Custom(CustomFormat),
}

/// A format plus the extensions it reads — one entry of [`Options::formats`].
///
/// Accepted forms (all `.into()`):
/// - `Format::Yaml` or `"yaml"` — default extensions
/// - `(Format::Yaml, ["yml", "conf"])` or `("yaml", ["yml", "conf"])` —
///   custom extensions
/// - a [`CustomFormat`] — user-defined parser with the extensions it was
///   built with
#[derive(Debug, Clone, PartialEq)]
pub struct FormatSpec {
    pub format: FormatKind,
    /// Extensions without the leading dot.
    pub extensions: Vec<String>,
}

impl From<Format> for FormatSpec {
    fn from(format: Format) -> Self {
        format.exts(format.default_extensions().iter().copied())
    }
}

impl From<Format> for FormatKind {
    fn from(format: Format) -> Self {
        FormatKind::Builtin(format)
    }
}

impl From<CustomFormat> for FormatKind {
    fn from(custom: CustomFormat) -> Self {
        FormatKind::Custom(custom)
    }
}

impl From<CustomFormat> for FormatSpec {
    fn from(custom: CustomFormat) -> Self {
        Self {
            extensions: custom.extensions.clone(),
            format: FormatKind::Custom(custom),
        }
    }
}

impl From<&str> for FormatSpec {
    /// Panics on an unknown format id — this is config-time code.
    fn from(id: &str) -> Self {
        Format::from_id(id)
            .unwrap_or_else(|| panic!("unknown format id: {id:?}"))
            .into()
    }
}

impl<const N: usize> From<(Format, [&str; N])> for FormatSpec {
    fn from((format, extensions): (Format, [&str; N])) -> Self {
        format.exts(extensions)
    }
}

impl<const N: usize> From<(&str, [&str; N])> for FormatSpec {
    /// Panics on an unknown format id — this is config-time code.
    fn from((id, extensions): (&str, [&str; N])) -> Self {
        Format::from_id(id)
            .unwrap_or_else(|| panic!("unknown format id: {id:?}"))
            .exts(extensions)
    }
}

/// File load order inside a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    /// Subfolders first, then files, each group alphabetic — how directory
    /// listings are conventionally sorted.
    #[default]
    FoldersFirstAlphabetic,
    /// Pure lexicographic order, folders and files interleaved.
    Alphabetic,
    ReverseAlphabetic,
}

/// Which `type` column values a table (csv) source accepts.
///
/// Scalars (`i8`–`u64`, `f32`/`f64`, `bool`, `str`, `auto` and their
/// aliases) are always allowed; the extended types are opt-in flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableTypes {
    pub null: bool,
    pub array: bool,
    pub json: bool,
}

impl TableTypes {
    /// Scalars only — the default.
    pub fn scalars() -> Self {
        Self::default()
    }

    /// Every supported type.
    pub fn all() -> Self {
        Self {
            null: true,
            array: true,
            json: true,
        }
    }
}

/// Names of the key / value / format columns of a table source.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumns {
    pub key: String,
    pub value: String,
    /// Name of the format column — the third column holding the value's
    /// type id.
    pub format: String,
}

impl Default for TableColumns {
    fn default() -> Self {
        Self {
            key: "key".into(),
            value: "value".into(),
            format: "format".into(),
        }
    }
}

/// Options for table-shaped formats (csv today; spreadsheet formats later).
#[derive(Debug, Clone, PartialEq)]
pub struct TableOptions {
    pub types: TableTypes,
    /// Delimiter splitting `arr:<type>` value cells.
    pub delimiter: char,
    /// Whether the first row is a header. Default `false`: rows are read
    /// positionally as `key,value[,format]` — the format column may be
    /// omitted entirely or left empty per row, in which case the value is
    /// auto-detected. With `header: true` the columns are located by name
    /// (file column order is free).
    pub header: bool,
    /// Column names, matched against the header row (only used when
    /// `header` is `true`).
    pub columns: TableColumns,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self {
            types: TableTypes::scalars(),
            delimiter: ';',
            header: false,
            columns: TableColumns::default(),
        }
    }
}

/// The parser callback of a [`CustomFormat`]: `(text, path, options)`.
pub(crate) type CustomParser =
    std::sync::Arc<dyn Fn(&str, &Path, &Options) -> Result<Value> + Send + Sync>;

/// A user-defined format: an id, the extensions it claims, and a parser
/// callback. It goes into [`Options::formats`] like any built-in format
/// (`formats: [..the defaults, md.into()]`), so the usual rule applies —
/// the last claimer of an extension owns it, and a custom format can take
/// over a built-in extension. Table-shaped custom formats can lower their
/// file into rows and reuse the generic table stage via [`parse_table`].
///
/// A markdown pipe-table as a config format (runs as a doc test — a
/// string source can name a custom format directly, no files needed):
///
/// ```
/// use c4::{CustomFormat, Loader, Source};
///
/// # fn main() -> Result<(), c4::Error> {
/// let md = CustomFormat::new("md-table", ["md"], |text, path, options| {
///     let rows: Vec<Vec<String>> = text
///         .lines()
///         .map(str::trim)
///         .filter(|l| l.starts_with('|') && !l.contains("---"))
///         // markdown tables always start with a header row — dropping
///         // it here keeps the data rows positional (key,value[,format])
///         // with no Options.table changes needed
///         .skip(1)
///         .map(|l| {
///             l.trim_matches('|')
///                 .split('|')
///                 .map(|cell| cell.trim().to_owned())
///                 .collect()
///         })
///         .collect();
///     c4::parse_table(rows, path, options)
/// });
///
/// // to read `.md` files instead, push `md` into Options.formats —
/// // custom formats claim extensions exactly like built-ins do
/// let value: c4::Value = Loader::new(c4::Options {
///         sources: vec![Source::string(
///             md,
///             "| key  | value | format |
///              | ---- | ----- | ------ |
///              | name | c4    | str    |
///              | port | 8080  | u16    |",
///         )],
///         ..c4::Options::default()
///     })
///     .load()?;
/// assert_eq!(value["name"].as_str(), Some("c4"));
/// assert_eq!(value["port"].as_u64(), Some(8080));
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CustomFormat {
    pub id: String,
    /// Extensions without the leading dot.
    pub extensions: Vec<String>,
    /// `(text, path, options)` — `path` labels errors. Crate-private:
    /// build custom formats through [`CustomFormat::new`].
    pub(crate) parser: CustomParser,
}

impl CustomFormat {
    pub fn new<I, S, F>(id: impl Into<String>, extensions: I, parser: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        F: Fn(&str, &Path, &Options) -> Result<Value> + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            extensions: extensions.into_iter().map(Into::into).collect(),
            parser: std::sync::Arc::new(parser),
        }
    }
}

impl std::fmt::Debug for CustomFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomFormat")
            .field("id", &self.id)
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}

impl PartialEq for CustomFormat {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.extensions == other.extensions
            && std::sync::Arc::ptr_eq(&self.parser, &other.parser)
    }
}

/// All loader options.
///
/// Plain data on purpose: set the fields you care about and take the rest
/// from `..Options::default()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The ordered config sources; later sources override earlier ones.
    /// Default: the `config` folder under the current working directory.
    pub sources: Vec<Source>,
    /// Which formats to read and which extensions each one claims. The
    /// list carries **no override-order semantics** — it only builds the
    /// extension→format mapping (if several entries claim the same
    /// extension, the last entry wins the claim). Override order between
    /// files is decided by filename alone; see the merge rules in
    /// README.md.
    pub formats: Vec<FormatSpec>,
    /// Scan subdirectories. Default `false`.
    pub recursive: bool,
    /// When recursive, ignore subfolder paths instead of turning them
    /// into nested keys. Default `true` — merge mode flattens; nesting
    /// by folder structure is tree mode's job (or set `flat: false`).
    pub flat: bool,
    /// Treat dotted env/table keys as nested paths: `a.b.c` becomes
    /// `{ "a": { "b": { "c": … } } }`. Default `true`.
    pub dot_key: bool,
    /// Case-sensitive key merging. Default `true`; when `false`, keys are
    /// normalized to lowercase. (File-extension matching is always
    /// case-insensitive and unrelated to this option.)
    pub case_sensitive: bool,
    /// Load order inside a folder — in merge mode and in tree mode
    /// (where it decides key collisions like `a.yml` vs a folder `a/`).
    pub order: Order,
    /// Table-format (csv) options.
    pub table: TableOptions,
    /// Tree mode (requires the `tree` feature): instead of merging a
    /// folder's files into one value, every subfolder becomes a key and
    /// every file becomes a key named after the file (extension
    /// stripped), holding that file's parsed content. `a/b.json = {c:1}`
    /// and `d.json = {a:123}` load as `{a: {b: {c: 1}}, d: {a: 123}}`.
    /// `recursive`, `flat` and `order` only apply when this is `false`.
    /// Default `false`.
    pub tree: bool,
    /// Tree mode: parse files *without* an extension by running the
    /// table `auto` detection over the (trimmed) file content — the same
    /// feature-gated guesses, so `a/blabla` containing `1.1.1.1` becomes
    /// an IPv4 value when the `ipv4` feature is on, a string otherwise.
    /// When `false`, extension-less files fall under
    /// `ignore_unknown_ext`. (Named `auto_files` to keep it apart from
    /// the table `auto` format id it reuses.) Default `true`.
    pub auto_files: bool,
    /// Tree mode: skip files whose extension no active format claims
    /// (`true`), or error on them (`false`). Default `true`. (In merge
    /// mode unclaimed files are always skipped.)
    pub ignore_unknown_ext: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sources: vec![Source::folder("config")],
            formats: enabled_formats(),
            recursive: false,
            flat: true,
            dot_key: true,
            case_sensitive: true,
            order: Order::default(),
            table: TableOptions::default(),
            tree: false,
            auto_files: true,
            ignore_unknown_ext: true,
        }
    }
}

/// Every format compiled into this build with its default extensions.
/// Later entries win contested extension claims, so `jsonc` (a JSON
/// superset) takes `.json` over strict `json` when both are enabled.
#[allow(clippy::vec_init_then_push)] // every push is cfg-gated on its feature
fn enabled_formats() -> Vec<FormatSpec> {
    let mut formats = Vec::new();
    #[cfg(feature = "json")]
    formats.push(Format::Json.into());
    #[cfg(feature = "jsonc")]
    formats.push(Format::Jsonc.into());
    #[cfg(feature = "yaml")]
    formats.push(Format::Yaml.into());
    #[cfg(feature = "toml")]
    formats.push(Format::Toml.into());
    #[cfg(feature = "ini")]
    formats.push(Format::Ini.into());
    #[cfg(feature = "env")]
    formats.push(Format::Env.into());
    #[cfg(feature = "csv")]
    formats.push(Format::Csv.into());
    formats
}
