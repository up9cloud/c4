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
    /// Excel workbooks (`xlsx`/`xlsm`/`xlsb`/`xls`) — a binary
    /// table-shaped format; file sources only. Feature `excel`.
    Excel,
    /// OpenDocument spreadsheets (`ods`) — a binary table-shaped format;
    /// file sources only. Feature `ods`.
    Ods,
}

impl Format {
    /// Look a format up by its string id (`"json"`, `"jsonc"`, `"yaml"`,
    /// `"toml"`, `"ini"`, `"env"`, `"csv"`, `"excel"`, `"ods"`).
    pub fn from_id(id: &str) -> Option<Format> {
        Some(match id {
            "json" => Format::Json,
            "jsonc" => Format::Jsonc,
            "yaml" => Format::Yaml,
            "toml" => Format::Toml,
            "ini" => Format::Ini,
            "env" => Format::Env,
            "csv" => Format::Csv,
            "excel" => Format::Excel,
            "ods" => Format::Ods,
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
            Format::Excel => "excel",
            Format::Ods => "ods",
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
            Format::Excel => &["xlsx", "xlsm", "xlsb", "xls"],
            Format::Ods => &["ods"],
        }
    }

    /// The default [`TableLayout`] of this format: `db` for the
    /// spreadsheet formats (excel/ods — a sheet is a record grid unless
    /// told otherwise), `kv` for csv (and, vacuously, for the non-table
    /// formats, which never consult a layout).
    pub fn default_layout(self) -> TableLayout {
        match self {
            Format::Excel | Format::Ods => TableLayout::Db,
            _ => TableLayout::Kv,
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
            layout: self.default_layout(),
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
/// - `(Format::Csv, ["csv"], "db")` — custom extensions **plus a
///   [`TableLayout`]** (a layout id string, a `TableLayout`, or a
///   [`CustomLayout`]): every file this entry claims parses under that
///   layout, in merge mode, tree mode and single-file path sources
///   alike. Table sources override it; string sources stay `Kv`; a
///   layout on a non-table format panics at conversion. Without this
///   form an entry uses the format's default layout
///   ([`Format::default_layout`]).
/// - a [`CustomFormat`] — user-defined parser with the extensions it was
///   built with
#[derive(Debug, Clone, PartialEq)]
pub struct FormatSpec {
    pub(crate) format: FormatKind,
    /// Extensions without the leading dot.
    pub(crate) extensions: Vec<String>,
    /// The table layout for files this entry claims (table formats only).
    pub(crate) layout: TableLayout,
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

impl From<&str> for FormatKind {
    /// A built-in format id (`"jsonc"`, `"yaml"`, …). Panics on an unknown
    /// id — this is config-time code. Lets a `(id, text)` string source
    /// name its format by string.
    fn from(id: &str) -> Self {
        FormatKind::Builtin(
            Format::from_id(id).unwrap_or_else(|| panic!("unknown format id: {id:?}")),
        )
    }
}

impl From<CustomFormat> for FormatSpec {
    fn from(custom: CustomFormat) -> Self {
        Self {
            extensions: custom.extensions.clone(),
            format: FormatKind::Custom(custom),
            layout: TableLayout::Kv,
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

/// In a `formats` entry the third element is always a **layout** (a
/// string must be a valid layout id — extensions have no sheet meaning).
/// Panics on unknown ids, and on a layout for a non-table format
/// (`(Format::Yaml, ["yml"], "db")`) — this is config-time code.
impl<const N: usize, L: Into<TableLayout>> From<(Format, [&str; N], L)> for FormatSpec {
    fn from((format, extensions, layout): (Format, [&str; N], L)) -> Self {
        assert!(
            matches!(format, Format::Csv | Format::Excel | Format::Ods),
            "'{}' is not a table format (csv, excel, ods) — table layouts do not apply",
            format.id()
        );
        FormatSpec {
            layout: layout.into(),
            ..format.exts(extensions)
        }
    }
}

/// Panics on an unknown format or layout id — this is config-time code.
impl<const N: usize, L: Into<TableLayout>> From<(&str, [&str; N], L)> for FormatSpec {
    fn from((id, extensions, layout): (&str, [&str; N], L)) -> Self {
        let format = Format::from_id(id).unwrap_or_else(|| panic!("unknown format id: {id:?}"));
        (format, extensions, layout).into()
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

impl Order {
    /// Look an order up by its string id. Accepts `-` or `_` between words
    /// and a couple of short aliases:
    /// `folders_first_alphabetic` (`folders_first`, `default`),
    /// `alphabetic`, `reverse_alphabetic` (`reverse`).
    pub fn from_id(id: &str) -> Option<Order> {
        Some(match id.to_lowercase().replace('-', "_").as_str() {
            "folders_first_alphabetic" | "folders_first" | "default" => {
                Order::FoldersFirstAlphabetic
            }
            "alphabetic" => Order::Alphabetic,
            "reverse_alphabetic" | "reverse" => Order::ReverseAlphabetic,
            _ => return None,
        })
    }

    /// The canonical string id of this order.
    pub fn id(self) -> &'static str {
        match self {
            Order::FoldersFirstAlphabetic => "folders_first_alphabetic",
            Order::Alphabetic => "alphabetic",
            Order::ReverseAlphabetic => "reverse_alphabetic",
        }
    }
}

impl From<&str> for Order {
    /// Panics on an unknown id — this is config-time code. See
    /// [`Order::from_id`] for the accepted spellings.
    fn from(id: &str) -> Self {
        Order::from_id(id).unwrap_or_else(|| panic!("unknown order id: {id:?}"))
    }
}

/// How the table stage interprets a row grid. Chosen per table source
/// (`(format, path, layout)` / `(format, path, sheet, layout)` tuples),
/// per `formats` entry (`(format, [exts], layout)`), or passed
/// explicitly to [`parse_table`](crate::parse_table); everything else
/// uses the format's default —
/// [`Format::default_layout`](crate::Format::default_layout): `kv` for
/// csv, `db` for excel/ods. Converts from a layout id string
/// (`"kv"`/`"kvf"`, `"db"`) or a [`CustomLayout`].
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TableLayout {
    /// `key,value[,format]` positional rows — one config entry per row.
    /// The default for csv.
    #[default]
    Kv,
    /// A database-style grid: the first (non-blank) row holds the keys,
    /// the second **always** the type ids (an empty cell = `auto`), every
    /// following row is one record. Parses to an **array** of one object
    /// per record: `[[a,b],[i8,str],[4,x],[5,y]]` →
    /// `[{a:4,b:"x"},{a:5,b:"y"}]`. Empty cells are omitted from their
    /// record (sparse tables give sparse objects). The default for the
    /// spreadsheet formats (excel/ods). A grid *without* a
    /// type row is a [`CustomLayout`] that inserts a row of `auto` cells
    /// after the header and delegates back to `Db` (see the
    /// `xlsx-sheets` example).
    Db,
    /// A user callback over the raw rows — see [`CustomLayout`].
    Custom(CustomLayout),
}

impl TableLayout {
    /// Look a layout up by its string id: `kv` (alias `kvf`) or `db`.
    pub fn from_id(id: &str) -> Option<TableLayout> {
        Some(match id.to_lowercase().as_str() {
            "kv" | "kvf" => TableLayout::Kv,
            "db" => TableLayout::Db,
            _ => return None,
        })
    }

    /// The string id of this layout (a custom layout reports its own id).
    pub fn id(&self) -> &str {
        match self {
            TableLayout::Kv => "kv",
            TableLayout::Db => "db",
            TableLayout::Custom(custom) => &custom.id,
        }
    }
}

impl From<&str> for TableLayout {
    /// Panics on an unknown id — this is config-time code. See
    /// [`TableLayout::from_id`] for the accepted spellings.
    fn from(id: &str) -> Self {
        TableLayout::from_id(id).unwrap_or_else(|| {
            panic!("unknown table layout id: {id:?} (valid: kv, db, or a CustomLayout)")
        })
    }
}

impl From<CustomLayout> for TableLayout {
    fn from(custom: CustomLayout) -> Self {
        TableLayout::Custom(custom)
    }
}

/// The parser callback of a [`CustomLayout`]: `(rows, path, options)`.
pub(crate) type CustomRowsParser =
    std::sync::Arc<dyn Fn(Vec<Vec<String>>, &Path, &Options) -> Result<Value> + Send + Sync>;

/// A user-defined table layout: an id and a callback that receives the
/// lowered rows (`Vec<Vec<String>>` — csv records, or a sheet's cells as
/// text) and returns any [`Value`]. This is the rows-level escape hatch:
/// binary formats (spreadsheets) can't go through a [`CustomFormat`]
/// (which parses text), but one sheet of a workbook can still get fully
/// custom treatment via
/// `(Format::Excel, path, sheet, CustomLayout::new(…)).into()`. Most
/// custom layouts end by reshaping the rows and calling
/// [`parse_table`](crate::parse_table) (the `xlsx-sheets` example
/// transposes a column-oriented sheet this way).
#[derive(Clone)]
pub struct CustomLayout {
    pub(crate) id: String,
    /// `(rows, path, options)` — `path` labels errors. Crate-private:
    /// build custom layouts through [`CustomLayout::new`].
    pub(crate) parser: CustomRowsParser,
}

impl CustomLayout {
    pub fn new<F>(id: impl Into<String>, parser: F) -> Self
    where
        F: Fn(Vec<Vec<String>>, &Path, &Options) -> Result<Value> + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            parser: std::sync::Arc::new(parser),
        }
    }
}

impl std::fmt::Debug for CustomLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomLayout")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for CustomLayout {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && std::sync::Arc::ptr_eq(&self.parser, &other.parser)
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
/// file into rows and reuse the generic table stage via
/// [`parse_table`](crate::parse_table).
///
/// A markdown pipe-table as a config format (runs as a doc test — a
/// string source can name a custom format directly, no files needed):
///
/// ```
/// use c4::{CustomFormat, Loader};
///
/// # fn main() -> Result<(), c4::Error> {
/// let md = CustomFormat::new("md-table", ["md"], |text, path, options| {
///     let rows: Vec<Vec<String>> = text
///         .lines()
///         .map(str::trim)
///         .filter(|l| l.starts_with('|') && !l.contains("---"))
///         // markdown tables always start with a header row — dropping
///         // it here keeps the data rows positional (key,value[,format]),
///         // exactly what parse_table expects
///         .skip(1)
///         .map(|l| {
///             l.trim_matches('|')
///                 .split('|')
///                 .map(|cell| cell.trim().to_owned())
///                 .collect()
///         })
///         .collect();
///     c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
/// });
///
/// // to read `.md` files instead, push `md` into Options.formats —
/// // custom formats claim extensions exactly like built-ins do
/// let value: c4::Value = Loader::new(c4::Options {
///         sources: vec![(
///             md,
///             "| key  | value | format |
///              | ---- | ----- | ------ |
///              | name | c4    | str    |
///              | port | 8080  | u16    |",
///         )
///             .into()],
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
    pub(crate) id: String,
    /// Extensions without the leading dot.
    pub(crate) extensions: Vec<String>,
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
    /// Every element converts with `.into()` — see [`Source`] for all
    /// the accepted shapes.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         sources: vec![
    ///             "./config".into(),              // a folder …
    ///             "./local.yml".into(),           // … or a single file
    ///             ("yaml", "debug: true").into(), // in-code text
    ///             (std::collections::BTreeMap::from([("port", 8080)]),).into(),
    ///         ],
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// # Ok(())
    /// # }
    /// ```
    pub sources: Vec<Source>,
    /// Which formats to read and which extensions each one claims. The
    /// list carries **no override-order semantics** — it only builds the
    /// extension→format mapping (if several entries claim the same
    /// extension, the last entry wins the claim). Override order between
    /// files is decided by filename alone; see the merge rules in
    /// README.md. Entry forms are on [`FormatSpec`].
    ///
    /// ```
    /// let options = c4::Options {
    ///     formats: vec![
    ///         "jsonc".into(),                                     // default extensions
    ///         (c4::Format::Yaml, ["yml", "yaml", "conf"]).into(), // custom extensions
    ///         (c4::Format::Csv, ["csv"], "db").into(),            // + a table layout
    ///     ],
    ///     ..c4::Options::default()
    /// };
    /// assert_eq!(options.formats.len(), 3);
    /// ```
    pub formats: Vec<FormatSpec>,
    /// **Merge mode only** (`tree: false`); folder sources. Scan
    /// subdirectories too. Default `false`.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // config/base.json and config/prod/db.json both merge in
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         recursive: true,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// # Ok(())
    /// # }
    /// ```
    pub recursive: bool,
    /// **Merge mode only**, and only meaningful with
    /// `recursive: true`. `true` (default): subfolder files merge as if
    /// they sat at the top level — folder names carry no meaning.
    /// `false`: each subfolder becomes a key and its files merge under
    /// it. (The *filename* never becomes a key in merge mode; for
    /// folder-and-filename keys use tree mode.)
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // config/db/a.json = {"host": "x"} — with flat: false it merges
    /// // under the subfolder's key instead of the top level
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         recursive: true,
    ///         flat: false,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert_eq!(value["db"]["host"].as_str(), Some("x"));
    /// # Ok(())
    /// # }
    /// ```
    pub flat: bool,
    /// **All modes; env and table formats.** Treat dotted keys as nested
    /// paths: `a.b.c` becomes `{ "a": { "b": { "c": … } } }`.
    /// Default `true`.
    ///
    /// In the table stage this applies to both layouts: the `kv`
    /// layout's key column, and the `db` layout's key row (a dotted
    /// column nests inside each record).
    ///
    /// A key segment may also end in **array suffixes** — `[]` (append)
    /// and `[<int>]` (index) groups, chained for nested arrays
    /// (`grid[0][1]`, one level per group); any other bracket shape
    /// (`a[x]`, `a[-1]`, `a[]b`, a bare `[]`) keeps the whole segment a
    /// literal key:
    ///
    /// - `name[]` **appends** one new element per occurrence: per kv
    ///   row / env line, and per record for a db column.
    /// - `name[<int>]` addresses element `<int>`: the same index
    ///   deep-merges into one element and indexes may arrive in any
    ///   order. Skipped indexes leave `Null` gaps — `a[1]` + `a[4]`
    ///   give a five-element array with nulls at 0, 2 and 3;
    ///   deserialize such arrays as `Vec<Option<T>>`.
    ///
    /// Suffixes also chain through the dotted path (`a[0].b[].c`) and
    /// work on keys without dots (`ports[]`). The expansion happens
    /// within one parse (one file/sheet/string source) — across sources
    /// arrays still replace like any array. The `dot-key` example tours
    /// all of it.
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// use c4::{Options, TableLayout};
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// let options = Options::default(); // dot_key: true
    ///
    /// // kv layout: the key column nests
    /// let rows = vec![vec!["a.b".to_string(), "1".to_string()]];
    /// let value = c4::parse_table(rows, &TableLayout::Kv, Path::new("doc"), &options)?;
    /// assert_eq!(value["a"]["b"].as_i64(), Some(1));
    ///
    /// // db layout: dotted keys nest inside each record
    /// let rows = vec![
    ///     vec!["a.b".to_string(), "a.c".to_string()], // key row
    ///     vec![String::new(), String::new()],         // type row (all auto)
    ///     vec!["1".to_string(), "2".to_string()],     // one record
    /// ];
    /// let value = c4::parse_table(rows, &TableLayout::Db, Path::new("doc"), &options)?;
    /// assert_eq!(value[0]["a"]["b"].as_i64(), Some(1));
    /// assert_eq!(value[0]["a"]["c"].as_i64(), Some(2));
    ///
    /// // array suffixes: `[]` appends per row, `[<int>]` indexes
    /// let rows = vec![
    ///     vec!["tags[]".to_string(), "red".to_string()],
    ///     vec!["tags[]".to_string(), "blue".to_string()],
    ///     vec!["servers[1].host".to_string(), "beta".to_string()],
    ///     vec!["servers[0].host".to_string(), "alpha".to_string()],
    /// ];
    /// let value = c4::parse_table(rows, &TableLayout::Kv, Path::new("doc"), &options)?;
    /// assert_eq!(value["tags"][1].as_str(), Some("blue"));
    /// assert_eq!(value["servers"][0]["host"].as_str(), Some("alpha"));
    ///
    /// // chained suffixes nest; skipped indexes leave null gaps
    /// let rows = vec![vec!["m[1][0]".to_string(), "9".to_string()]];
    /// let value = c4::parse_table(rows, &TableLayout::Kv, Path::new("doc"), &options)?;
    /// assert!(value["m"][0].is_null());
    /// assert_eq!(value["m"][1][0].as_i64(), Some(9));
    /// # Ok(())
    /// # }
    /// ```
    pub dot_key: bool,
    /// **All modes.** Case-sensitive key merging. Default `true`; when
    /// `false`, keys are normalized to lowercase. (File-extension
    /// matching is always case-insensitive and unrelated to this
    /// option.)
    ///
    /// ```
    /// use std::collections::BTreeMap;
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         sources: vec![
    ///             (BTreeMap::from([("PORT", 1)]),).into(),
    ///             (BTreeMap::from([("port", 8080)]),).into(),
    ///         ],
    ///         case_sensitive: false, // "PORT" and "port" are one key
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert_eq!(value["port"].as_i64(), Some(8080));
    /// # Ok(())
    /// # }
    /// ```
    pub case_sensitive: bool,
    /// **All modes**; folder sources. Load order inside a folder. In
    /// merge mode it decides which file overrides which; in tree mode it
    /// decides key collisions (`a.yml` vs a folder `a/`).
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         order: "alphabetic".into(), // or c4::Order::Alphabetic
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// # Ok(())
    /// # }
    /// ```
    pub order: Order,
    /// **Mode switch** (requires the `tree` feature; `true` without it
    /// is `Error::Unsupported`). `false` (default): merge mode — a
    /// folder's files deep-merge into one value. `true`: tree mode — the
    /// folder's *shape* becomes the value: every subfolder is a key, and
    /// every file is a key named after the file (extension stripped)
    /// holding that file's parsed content — `a/b.json = {c:1}` and
    /// `d.json = {a:123}` load as `{a: {b: {c: 1}}, d: {a: 123}}`.
    ///
    /// The rules, in full:
    /// - Applies to **folder path sources** only; single-file and string
    ///   sources merge into the root as always.
    /// - Always recursive — `recursive` and `flat` do not apply, but
    ///   `order` does: entries load in order, and a key collision
    ///   (`a.yml` next to a folder `a/`) deep-merges, so order decides
    ///   the winner.
    /// - A dotfile whose whole name is its extension (`.env`) keeps the
    ///   full name as its key.
    /// - Extension handling: a claimed extension parses normally; an
    ///   extension-less file follows [`auto_files`](Options::auto_files);
    ///   any other unclaimed extension follows
    ///   [`ignore_unknown_ext`](Options::ignore_unknown_ext).
    /// - A spreadsheet workbook parses to an object keyed by sheet name,
    ///   so its sheets become keys under the file's key — `a/b.xlsx`
    ///   with sheets `c`, `d` loads as `{a: {b: {c: …, d: …}}}`.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // config/a/b.json = {"c": 1}  ->  {a: {b: {c: 1}}}
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         tree: true,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert_eq!(value["a"]["b"]["c"].as_i64(), Some(1));
    /// # Ok(())
    /// # }
    /// ```
    pub tree: bool,
    /// **Tree mode only.** What to do with a file that has *no*
    /// extension. `true` (default): read it and guess the type of its
    /// (trimmed) content with the table `auto` rules — a file containing
    /// `1.1.1.1` becomes an IPv4 value when the `ipv4` feature is on, a
    /// string otherwise. `false`: treat it like a file with an unknown
    /// extension (see [`ignore_unknown_ext`](Options::ignore_unknown_ext)).
    /// (Named `auto_files` after the table `auto` type id it reuses.)
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // tree mode: config/host is an extension-less file holding "1.1.1.1"
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         tree: true,
    ///         auto_files: true,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert!(!value["host"].is_null());
    /// # Ok(())
    /// # }
    /// ```
    pub auto_files: bool,
    /// **Tree mode only.** What to do with a file whose extension no
    /// active format claims: skip it (`true`, default) or fail the load
    /// with `Error::Parse` (`false`). Merge mode always skips such
    /// files.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         tree: true,
    ///         ignore_unknown_ext: false, // a config/*.xyz file now errors
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// # Ok(())
    /// # }
    /// ```
    pub ignore_unknown_ext: bool,
    /// **Spreadsheet formats (excel/ods) only.** Skip sheets whose name
    /// starts with `#`, `.` or `_` — draft/scratch space next to live
    /// config. Default `true`. (A table source that names a sheet
    /// explicitly bypasses this filter, so a prefixed sheet stays
    /// loadable on purpose.)
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // tree mode: game.xlsx has sheets "items" and "_draft"; by
    /// // default "_draft" is skipped — false reads it as a key too
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         sources: vec!["./game.xlsx".into()],
    ///         tree: true,
    ///         ignore_sheet_prefix: false,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// assert!(!value["_draft"].is_null());
    /// # Ok(())
    /// # }
    /// ```
    pub ignore_sheet_prefix: bool,
    /// **Spreadsheet formats (excel/ods) only.** Skip sheets marked
    /// hidden in the workbook (Excel `hidden`/`veryHidden`, OpenDocument
    /// `table:display="false"`). Default `true`. (A table source that
    /// names a sheet explicitly bypasses this filter.)
    ///
    /// ```no_run
    /// # fn main() -> Result<(), c4::Error> {
    /// // the workbook's `config` sheet is marked hidden — read it anyway
    /// let value: c4::Value = c4::Loader::new(c4::Options {
    ///         sources: vec!["./game.xlsx".into()],
    ///         ignore_hidden_sheets: false,
    ///         ..c4::Options::default()
    ///     })
    ///     .load()?;
    /// # Ok(())
    /// # }
    /// ```
    pub ignore_hidden_sheets: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sources: vec![Source::Path("config".into())],
            formats: enabled_formats(),
            recursive: false,
            flat: true,
            dot_key: true,
            case_sensitive: true,
            order: Order::default(),
            tree: false,
            auto_files: true,
            ignore_unknown_ext: true,
            ignore_sheet_prefix: true,
            ignore_hidden_sheets: true,
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
    #[cfg(feature = "excel")]
    formats.push(Format::Excel.into());
    #[cfg(feature = "ods")]
    formats.push(Format::Ods.into());
    formats
}
