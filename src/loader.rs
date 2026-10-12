//! The loader: scan sources, pick parsers by extension, deep-merge. One
//! code path for `trace()` and `load()`, generic over the merge target:
//! the traced tree, or a plain value that skips the per-leaf provenance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::trace::{SourceRef, Trace, TracedValue, Warning};
use crate::{Error, Format, FormatKind, Options, Order, Result, Source, Value, error, format};

/// Loads and merges the config sources named by its [`Options`].
#[derive(Debug, Clone, Default)]
pub struct Loader {
    pub options: Options,
}

impl Loader {
    /// Everything — sources included — comes in through [`Options`];
    /// there is no separate builder step.
    pub fn new(options: Options) -> Self {
        Self { options }
    }

    /// Load and merge all sources, deserializing into `T`.
    ///
    /// [`Value`] implements `Deserialize`, so `T = Value` gives dynamic
    /// access and any other serde type gives typed config. A `Value` —
    /// the whole result, or a field of your struct — is handed over as
    /// merged, typed variants (`DateTime`, `Ipv4`, …) included, exactly as
    /// [`Loader::trace`] shows them.
    ///
    /// A **string** reaching a `bool`, integer or float field parses with
    /// the rules of that table type id (`u16`, `bool`, … — see the crate
    /// docs' table section), so ini, env and cnf values, which are all
    /// strings, load into typed fields. The explicit `bool` tokens apply
    /// (`yes`/`no`, `on`/`off`, `1`/`0`, …), and out-of-range or malformed
    /// text is [`Error::Deserialize`] (`'70000' is not a valid u16`). Untyped
    /// targets (`Value`, `String`) keep the string.
    ///
    /// The reverse holds too: a number or bool reaching a `String` field
    /// becomes its text (`password: 123456` → `"123456"`; a float in its
    /// shortest round-trip form, so `1.0` stays `"1.0"`).
    ///
    /// Every [`Error::Deserialize`] names the key path it failed at —
    /// `db.password: invalid type: …`, `items[2].id: …`. An empty value
    /// (`PORT=`) cannot fill a number or bool field, not even an `Option`
    /// one (serde picks `None` or `Some` before it sees the inner type);
    /// the error says to remove the line instead.
    ///
    /// To type one part of a loaded [`Value`], use [`Value::into_typed`].
    ///
    /// ```
    /// #[derive(serde::Deserialize)]
    /// struct Server {
    ///     port: u16,
    ///     debug: bool,
    /// }
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// // the 1-tuple value source stands in for an ini/env/cnf file here
    /// let server: Server = c4::Loader::new(c4::Options {
    ///     sources: vec![(serde_json::json!({ "port": "8080", "debug": "yes" }),).into()],
    ///     ..c4::Options::default()
    /// })
    /// .load()?;
    /// assert_eq!(server.port, 8080);
    /// assert!(server.debug);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Warning`]s are dropped — call [`Loader::trace`] to see why some
    /// input was not used (a skipped cnf `!include`, for instance).
    pub fn load<T: DeserializeOwned>(&self) -> Result<T> {
        let (value, _warnings) = self.run::<Value>()?;
        value.into_typed()
    }

    /// Like [`Loader::load`], but keeps per-leaf provenance and the
    /// [`Warning`]s — see [`Trace`]. This powers the CLI `--trace` mode;
    /// source labels are a debug/testing aid, not config data.
    ///
    /// ```
    /// use c4::{CustomFormat, Loader, TracedValue};
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// // a custom-format string source keeps this example runnable
    /// // under every feature combination (`Loader::default()` would
    /// // read ./config instead)
    /// let kv = CustomFormat::new("kv", ["kv"], |text, path, options| {
    ///     let rows = text
    ///         .lines()
    ///         .map(|line| line.split('=').map(str::to_owned).collect())
    ///         .collect();
    ///     c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
    /// });
    ///
    /// let trace = Loader::new(c4::Options {
    ///     sources: vec![(kv, "port=8080").into()],
    ///     ..c4::Options::default()
    /// })
    /// .trace()?;
    /// assert!(trace.warnings.is_empty());
    /// let TracedValue::Object(root) = &trace.value else { unreachable!() };
    /// let TracedValue::Leaf { value, source } = &root["port"] else { unreachable!() };
    /// assert_eq!(value.as_u64(), Some(8080));
    /// assert_eq!(source, &c4::SourceRef::String(0)); // "string:0" in traces
    /// # Ok(())
    /// # }
    /// ```
    pub fn trace(&self) -> Result<Trace> {
        let (value, warnings) = self.run::<TracedValue>()?;
        Ok(Trace { value, warnings })
    }

    /// Scan, parse and merge every source into `R` — the one code path
    /// behind both [`Loader::trace`] (a traced tree) and [`Loader::load`]
    /// (a plain value, skipping the per-leaf provenance it would drop).
    fn run<R: Tree>(&self) -> Result<(R, Vec<Warning>)> {
        let extensions = extension_map(&self.options);
        let mut root = R::root();
        let mut warnings = Vec::new();

        for (index, source) in self.options.sources.iter().enumerate() {
            match source {
                // one path source: a folder whose files merge, or a single
                // file — detected here (same rule as `c4::load`)
                Source::Path(path) if path.is_dir() => {
                    self.load_folder(path, &extensions, &mut root, &mut warnings)?;
                }
                Source::Path(path) if path.is_file() => {
                    let claim = claimed_format(path, &extensions).ok_or_else(|| Error::Parse {
                        path: path.clone(),
                        message: unclaimed_message(path),
                    })?;
                    let source = SourceRef::File(path.clone());
                    let mut messages = Vec::new();
                    let value = self.parse_file(claim, path, &mut messages)?;
                    note(&mut warnings, &source, messages);
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &source);
                    }
                }
                Source::Path(path) => return Err(Error::NotFound(path.clone())),
                Source::Table {
                    path,
                    format,
                    sheet,
                    layout,
                } => {
                    // one file of a table format, read under an explicit
                    // layout (and, for spreadsheets, an explicit sheet)
                    if !path.is_file() {
                        return Err(Error::Parse {
                            path: path.clone(),
                            message: "table source is not a file (table sources read \
                                      exactly one file; for in-code text use a \
                                      (format, text) string source)"
                                .into(),
                        });
                    }
                    let value = format::parse_table_file(
                        *format,
                        path,
                        sheet.as_deref(),
                        layout,
                        &self.options,
                    )?;
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &SourceRef::File(path.clone()));
                    }
                }
                Source::Value(result) => {
                    let mut value = result.clone().map_err(|message| Error::Parse {
                        path: PathBuf::from(format!("value:{index}")),
                        message,
                    })?;
                    format::strip_commented_data_keys(&mut value, &self.options);
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &SourceRef::Value(index));
                    }
                }
                Source::String { format, content } => {
                    let label = PathBuf::from(format!("string:{index}"));
                    let mut messages = Vec::new();
                    let value = match format {
                        // `format::parse` filters commented data keys
                        // itself; a custom parser's result is filtered here
                        FormatKind::Builtin(format) => format::parse(
                            *format,
                            format::strip_bom(content),
                            &crate::TableLayout::Kv,
                            &label,
                            &self.options,
                            &mut messages,
                        )?,
                        FormatKind::Custom(custom) => {
                            let mut value =
                                (custom.parser)(format::strip_bom(content), &label, &self.options)?;
                            format::strip_commented_data_keys(&mut value, &self.options);
                            value
                        }
                    };
                    let source = SourceRef::String(index);
                    note(&mut warnings, &source, messages);
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &source);
                    }
                }
            }
        }
        Ok((root, warnings))
    }

    fn parse_file(&self, claim: Claim, path: &Path, warnings: &mut Vec<String>) -> Result<Value> {
        match claim {
            Claim::Builtin { format, spec } => {
                // the claiming formats entry decides the table layout
                let layout = &self.options.formats[spec].layout;
                // binary formats (spreadsheets) parse from the path
                // directly — their bytes must never go through the text
                // pipeline
                if format::is_binary(format) {
                    return format::parse_binary(format, path, layout, &self.options);
                }
                let text = format::read_text(path)?;
                format::parse(format, &text, layout, path, &self.options, warnings)
            }
            Claim::Custom(index) => {
                let FormatKind::Custom(custom) = &self.options.formats[index].format else {
                    unreachable!("custom claims always index custom entries");
                };
                let text = format::read_text(path)?;
                // built-in formats filter commented data keys inside
                // `format::parse`/`parse_binary`; do it for custom ones here
                let mut value = (custom.parser)(&text, path, &self.options)?;
                format::strip_commented_data_keys(&mut value, &self.options);
                Ok(value)
            }
        }
    }

    fn merge<R: Tree>(&self, target: &mut R, value: Value, source: &SourceRef) {
        target.merge(value, source, !self.options.case_sensitive);
    }

    /// Load one folder source: scan it (honoring [`Options::dir_depth`]),
    /// parse each file, and merge it in. The keying options decide the
    /// shape — [`filename_as_key`](Options::filename_as_key) wraps each
    /// file's value under a key named after the file, and
    /// [`dirname_as_key`](Options::dirname_as_key) wraps under the
    /// subfolder names leading to it. With both off this is a flat merge;
    /// with both on (the `tree` feature's default) it is a folder tree.
    /// Entries load in [`Options::order`], so key collisions deep-merge
    /// with the later entry winning.
    fn load_folder<R: Tree>(
        &self,
        folder: &Path,
        extensions: &[(String, Claim)],
        root: &mut R,
        warnings: &mut Vec<Warning>,
    ) -> Result<()> {
        let mut files = Vec::new();
        walk(
            folder,
            Vec::new(),
            &self.options,
            self.options.dir_depth,
            &mut files,
        )?;
        for (file, dirs) in files {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            let mut messages = Vec::new();
            let value = match claimed_format(&file, extensions) {
                Some(claim) => self.parse_file(claim, &file, &mut messages)?,
                // extension-less / unknown-extension handling only applies
                // when a file becomes a key; otherwise the file has no key
                // to contribute under and is skipped
                None if self.options.filename_as_key
                    && file_extension(&name).is_none()
                    && self.options.auto_no_ext_files =>
                {
                    // the trimmed content goes through the table auto
                    // detection (same feature-gated guesses)
                    let text = format::read_text(&file)?;
                    format::table_auto(text.trim())
                }
                None if self.options.filename_as_key && !self.options.ignore_unknown_ext => {
                    return Err(Error::Parse {
                        message: unclaimed_message(&file),
                        path: file,
                    });
                }
                None => continue, // no active format claims this extension
            };
            let source = SourceRef::File(file);
            note(warnings, &source, messages);
            if matches!(value, Value::Null) {
                continue; // empty file contributes nothing
            }
            let mut value = value;
            if self.options.filename_as_key {
                value = Value::Object(BTreeMap::from([(file_stem_key(&name), value)]));
            }
            if self.options.dirname_as_key {
                for key in dirs.into_iter().rev() {
                    value = Value::Object(BTreeMap::from([(key, value)]));
                }
            }
            self.merge(root, value, &source);
        }
        Ok(())
    }
}

/// The [`filename_as_key`](Options::filename_as_key) key of a file: its
/// name without the extension. A hidden file whose whole name is its
/// extension (`.env`) keeps the full name.
fn file_stem_key(name: &str) -> String {
    let hidden = name.starts_with('.');
    let body = if hidden { &name[1..] } else { name };
    match body.rfind('.') {
        Some(i) => format!("{}{}", if hidden { "." } else { "" }, &body[..i]),
        None => name.to_owned(),
    }
}

/// What an extension resolves to: a built-in format (with the index of
/// its [`Options::formats`] entry, whose layout applies), or a custom
/// entry (by index).
#[derive(Clone, Copy)]
enum Claim {
    Builtin { format: Format, spec: usize },
    Custom(usize),
}

/// The extension→parser mapping of [`Options::formats`]: lowercased
/// extension, last claimer wins — built-in and custom entries follow the
/// same rule.
fn extension_map(options: &Options) -> Vec<(String, Claim)> {
    let mut map: Vec<(String, Claim)> = Vec::new();
    for (index, spec) in options.formats.iter().enumerate() {
        let claim = match &spec.format {
            FormatKind::Builtin(format) => Claim::Builtin {
                format: *format,
                spec: index,
            },
            FormatKind::Custom(_) => Claim::Custom(index),
        };
        for ext in &spec.extensions {
            let ext = ext.to_lowercase();
            map.retain(|(e, _)| *e != ext);
            map.push((ext, claim));
        }
    }
    map
}

/// The extension of a file name, lowercased. A hidden file `.X` counts as
/// having extension `X` (that is how `.env` matches `env`).
fn file_extension(name: &str) -> Option<String> {
    let hidden = name.starts_with('.');
    let stem = if hidden { &name[1..] } else { name };
    match stem.rfind('.') {
        Some(i) => Some(stem[i + 1..].to_lowercase()),
        None if hidden && !stem.is_empty() => Some(stem.to_lowercase()),
        None => None,
    }
}

/// Why no format claims `path`, and how to fix it: the built-in format
/// whose default extensions include it is either compiled out (name the
/// feature) or missing from `Options.formats`.
fn unclaimed_message(path: &Path) -> String {
    let base = "no active format claims this file's extension";
    let Some(ext) = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(file_extension)
    else {
        return base.into();
    };
    // jsonc before json: it is the default reader of `.json`
    let reader = Format::ALL
        .into_iter()
        .find(|f| f.default_extensions().contains(&ext.as_str()));
    match reader {
        Some(f) if !format::is_compiled(f) => format!(
            "{base}: '.{ext}' is read by the `{id}` format — enable the `{id}` feature",
            id = f.id()
        ),
        Some(f) => format!(
            "{base}: the `{}` format reads it but is not in `formats`",
            f.id()
        ),
        None => base.into(),
    }
}

fn claimed_format(path: &Path, extensions: &[(String, Claim)]) -> Option<Claim> {
    let name = path.file_name()?.to_str()?;
    let ext = file_extension(name)?;
    extensions
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, claim)| *claim)
}

/// A directory's entries as `(name, path, is_dir)`.
fn read_entries(dir: &Path) -> Result<Vec<(String, PathBuf, bool)>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(error::io(dir))? {
        let entry = entry.map_err(error::io(dir))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        entries.push((name.to_owned(), path.clone(), path.is_dir()));
    }
    Ok(entries)
}

fn sort_entries(entries: &mut [(String, PathBuf, bool)], order: Order) {
    match order {
        Order::FoldersFirstAlphabetic => entries.sort_by(|a, b| (!a.2, &a.0).cmp(&(!b.2, &b.0))),
        Order::Alphabetic => entries.sort_by(|a, b| a.0.cmp(&b.0)),
        Order::ReverseAlphabetic => entries.sort_by(|a, b| b.0.cmp(&a.0)),
    }
}

/// All files of a folder in load order, each with the subfolder names
/// leading to it. `depth` is the remaining [`Options::dir_depth`] budget:
/// `0` stops descent, `-1` recurses without limit, a positive value
/// decrements per level. Commented-out names (`#…`, `_…`) are skipped
/// here — by name, so keyed or not, a `_draft.yml` file and everything
/// under a `_wip/` folder stay out of the scan.
fn walk(
    dir: &Path,
    prefix: Vec<String>,
    options: &Options,
    depth: isize,
    out: &mut Vec<(PathBuf, Vec<String>)>,
) -> Result<()> {
    let mut entries = read_entries(dir)?;
    sort_entries(&mut entries, options.order);
    for (name, path, is_dir) in entries {
        let ignore_commented = if is_dir {
            options.ignore_commented_dirnames
        } else {
            options.ignore_commented_filenames
        };
        if ignore_commented && crate::options::is_commented(&name) {
            continue;
        }
        if is_dir {
            if depth != 0 {
                let mut prefix = prefix.clone();
                prefix.push(name);
                walk(
                    &path,
                    prefix,
                    options,
                    if depth < 0 { -1 } else { depth - 1 },
                    out,
                )?;
            }
        } else {
            out.push((path, prefix.clone()));
        }
    }
    Ok(())
}

fn note(warnings: &mut Vec<Warning>, source: &SourceRef, messages: Vec<String>) {
    warnings.extend(messages.into_iter().map(|message| Warning {
        source: source.clone(),
        message,
    }));
}

/// What the sources merge into. Both trees deep-merge the same way —
/// objects recurse, everything else replaces, keys lowercased when merging
/// is case-insensitive — and differ only in whether leaves keep `source`.
trait Tree {
    fn root() -> Self;
    fn merge(&mut self, value: Value, source: &SourceRef, lowercase: bool);
}

impl Tree for TracedValue {
    fn root() -> Self {
        TracedValue::Object(BTreeMap::new())
    }

    fn merge(&mut self, value: Value, source: &SourceRef, lowercase: bool) {
        merge_traced(self, value, source, lowercase);
    }
}

impl Tree for Value {
    fn root() -> Self {
        Value::Object(BTreeMap::new())
    }

    fn merge(&mut self, value: Value, _source: &SourceRef, lowercase: bool) {
        merge_plain(self, value, lowercase);
    }
}

/// [`merge_traced`] without the provenance.
fn merge_plain(target: &mut Value, value: Value, lowercase: bool) {
    match (target, value) {
        (Value::Object(entries), Value::Object(incoming)) => {
            for (key, value) in incoming {
                let key = if lowercase { key.to_lowercase() } else { key };
                match entries.get_mut(&key) {
                    Some(slot) => merge_plain(slot, value, lowercase),
                    None => {
                        entries.insert(key, lowercased(value, lowercase));
                    }
                }
            }
        }
        (slot, value) => *slot = lowercased(value, lowercase),
    }
}

/// [`to_traced`] without the provenance: object keys lowercased at every
/// level (arrays are leaves, so their objects keep their keys).
fn lowercased(value: Value, lowercase: bool) -> Value {
    match value {
        Value::Object(map) if lowercase => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key.to_lowercase(), lowercased(value, lowercase)))
                .collect(),
        ),
        other => other,
    }
}

/// Deep-merge `value` (labeled with `source`) into the traced tree:
/// objects merge recursively, everything else replaces.
fn merge_traced(target: &mut TracedValue, value: Value, source: &SourceRef, lowercase: bool) {
    match (target, value) {
        (TracedValue::Object(entries), Value::Object(incoming)) => {
            for (key, value) in incoming {
                let key = if lowercase { key.to_lowercase() } else { key };
                match entries.get_mut(&key) {
                    Some(slot) => merge_traced(slot, value, source, lowercase),
                    None => {
                        entries.insert(key, to_traced(value, source, lowercase));
                    }
                }
            }
        }
        (slot, value) => *slot = to_traced(value, source, lowercase),
    }
}

fn to_traced(value: Value, source: &SourceRef, lowercase: bool) -> TracedValue {
    match value {
        Value::Object(map) => TracedValue::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let key = if lowercase { key.to_lowercase() } else { key };
                    (key, to_traced(value, source, lowercase))
                })
                .collect(),
        ),
        other => TracedValue::Leaf {
            value: other,
            source: source.clone(),
        },
    }
}
