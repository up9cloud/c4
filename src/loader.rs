//! The loader: scan sources, pick parsers by extension, deep-merge with
//! per-leaf provenance. `load()` is `trace()` minus the labels — one
//! code path for both.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::trace::{SourceRef, TracedValue};
use crate::{Error, Format, FormatKind, Options, Order, Result, Source, Value, de, format};

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
    /// access and any other serde type gives typed config.
    pub fn load<T: DeserializeOwned>(&self) -> Result<T> {
        let value = untrace(self.trace()?);
        T::deserialize(de::ValueDeserializer::new(value))
            .map_err(|e| Error::Deserialize(e.to_string()))
    }

    /// Like [`Loader::load`], but keeps per-leaf provenance — see
    /// [`TracedValue`]. This powers the CLI `--trace` mode; source labels
    /// are a debug/testing aid, not config data.
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
    /// let traced = Loader::new(c4::Options {
    ///     sources: vec![(kv, "port=8080").into()],
    ///     ..c4::Options::default()
    /// })
    /// .trace()?;
    /// let TracedValue::Object(root) = &traced else { unreachable!() };
    /// let TracedValue::Leaf { value, source } = &root["port"] else { unreachable!() };
    /// assert_eq!(value.as_u64(), Some(8080));
    /// assert_eq!(source, &c4::SourceRef::String(0)); // "string:0" in traces
    /// # Ok(())
    /// # }
    /// ```
    pub fn trace(&self) -> Result<TracedValue> {
        let extensions = extension_map(&self.options);
        let mut root = TracedValue::Object(BTreeMap::new());

        for (index, source) in self.options.sources.iter().enumerate() {
            match source {
                // one path source: a folder whose files merge, or a single
                // file — detected here (same rule as `c4::load`)
                Source::Path(path) if path.is_dir() => {
                    self.load_folder(path, &extensions, &mut root)?;
                }
                Source::Path(path) if path.is_file() => {
                    let claim = claimed_format(path, &extensions).ok_or_else(|| Error::Parse {
                        path: path.clone(),
                        message: "no active format claims this file's extension".into(),
                    })?;
                    let value = self.parse_file(claim, path)?;
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &SourceRef::File(path.clone()));
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
                    let value = result.clone().map_err(|message| Error::Parse {
                        path: PathBuf::from(format!("value:{index}")),
                        message,
                    })?;
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &SourceRef::Value(index));
                    }
                }
                Source::String { format, content } => {
                    let label = PathBuf::from(format!("string:{index}"));
                    let value = match format {
                        FormatKind::Builtin(format) => format::parse(
                            *format,
                            content,
                            &crate::TableLayout::Kv,
                            &label,
                            &self.options,
                        )?,
                        FormatKind::Custom(custom) => {
                            (custom.parser)(content, &label, &self.options)?
                        }
                    };
                    if !matches!(value, Value::Null) {
                        self.merge(&mut root, value, &SourceRef::String(index));
                    }
                }
            }
        }
        Ok(root)
    }

    fn parse_file(&self, claim: Claim, path: &Path) -> Result<Value> {
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
                let text = std::fs::read_to_string(path).map_err(Error::Io)?;
                format::parse(format, &text, layout, path, &self.options)
            }
            Claim::Custom(index) => {
                let FormatKind::Custom(custom) = &self.options.formats[index].format else {
                    unreachable!("custom claims always index custom entries");
                };
                let text = std::fs::read_to_string(path).map_err(Error::Io)?;
                (custom.parser)(&text, path, &self.options)
            }
        }
    }

    fn merge(&self, target: &mut TracedValue, value: Value, source: &SourceRef) {
        merge_traced(target, value, source, !self.options.case_sensitive);
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
    fn load_folder(
        &self,
        folder: &Path,
        extensions: &[(String, Claim)],
        root: &mut TracedValue,
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
            let value = match claimed_format(&file, extensions) {
                Some(claim) => self.parse_file(claim, &file)?,
                // extension-less / unknown-extension handling only applies
                // when a file becomes a key; otherwise the file has no key
                // to contribute under and is skipped
                None if self.options.filename_as_key
                    && file_extension(&name).is_none()
                    && self.options.auto_no_ext_files =>
                {
                    // the trimmed content goes through the table auto
                    // detection (same feature-gated guesses)
                    let text = std::fs::read_to_string(&file).map_err(Error::Io)?;
                    format::table_auto(text.trim())
                }
                None if self.options.filename_as_key && !self.options.ignore_unknown_ext => {
                    return Err(Error::Parse {
                        path: file,
                        message: "no active format claims this file's extension".into(),
                    });
                }
                None => continue, // no active format claims this extension
            };
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
            self.merge(root, value, &SourceRef::File(file));
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
    for entry in std::fs::read_dir(dir).map_err(Error::Io)? {
        let entry = entry.map_err(Error::Io)?;
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
/// decrements per level.
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

fn untrace(traced: TracedValue) -> Value {
    match traced {
        TracedValue::Leaf { value, .. } => value,
        TracedValue::Object(entries) => {
            Value::Object(entries.into_iter().map(|(k, v)| (k, untrace(v))).collect())
        }
    }
}
