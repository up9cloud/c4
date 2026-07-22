//! One module per format, each gated by its Cargo feature. `parse` is the
//! single dispatch point used by the loader.

#[cfg(feature = "csv")]
mod csv;
#[cfg(feature = "env")]
mod env;
#[cfg(feature = "ini")]
mod ini;
#[cfg(feature = "json")]
mod json;
#[cfg(feature = "jsonc")]
mod jsonc;
// both spreadsheet formats (binary, calamine-read) live in one module
#[cfg(any(feature = "excel", feature = "ods"))]
mod sheet;
// the generic table stage is always compiled: custom formats reuse it via
// `crate::parse_table` regardless of which format features are on
pub(crate) mod table;

// filename_as_key runs extensionless files through the same auto detection
pub(crate) use table::auto as table_auto;
#[cfg(feature = "toml")]
mod toml;
#[cfg(feature = "yaml")]
mod yaml;

use std::path::Path;

use crate::{Error, Format, Options, Result, TableLayout, Value};

/// Binary formats parse from the path, never from text — the loader
/// calls [`parse_binary`] for them instead of reading the file to a
/// string, and [`parse`] (the text entry, which string sources use)
/// rejects them.
pub(crate) fn is_binary(format: Format) -> bool {
    matches!(format, Format::Excel | Format::Ods)
}

pub(crate) fn parse_binary(
    format: Format,
    path: &Path,
    layout: &TableLayout,
    options: &Options,
) -> Result<Value> {
    let _ = (layout, options); // unused when neither spreadsheet feature is on
    match format {
        #[cfg(feature = "excel")]
        Format::Excel => sheet::parse_excel(path, None, layout, options),
        #[cfg(feature = "ods")]
        Format::Ods => sheet::parse_ods(path, None, layout, options),
        other => Err(Error::Parse {
            path: path.to_path_buf(),
            message: format!("format '{}' is not compiled into this build", other.id()),
        }),
    }
}

/// A `Source::Table`: one file of a table format read under an explicit
/// [`TableLayout`], optionally naming a spreadsheet sheet.
pub(crate) fn parse_table_file(
    format: Format,
    path: &Path,
    sheet_name: Option<&str>,
    layout: &TableLayout,
    options: &Options,
) -> Result<Value> {
    let _ = (sheet_name, layout, options); // unused without table-format features
    let parse_error = |message: String| Error::Parse {
        path: path.to_path_buf(),
        message,
    };
    match format {
        #[cfg(feature = "csv")]
        Format::Csv => {
            if let Some(sheet) = sheet_name {
                // also catches a 3-tuple layout typo, which DWIM turns
                // into a sheet name — echo it so the mistake is visible
                return Err(parse_error(format!(
                    "csv sources cannot name a sheet (got '{sheet}'; \
                     did you mean a layout — kv, db?)"
                )));
            }
            let text = std::fs::read_to_string(path).map_err(Error::Io)?;
            csv::parse(&text, layout, path, options)
        }
        #[cfg(feature = "excel")]
        Format::Excel => sheet::parse_excel(path, sheet_name, layout, options),
        #[cfg(feature = "ods")]
        Format::Ods => sheet::parse_ods(path, sheet_name, layout, options),
        // table formats whose feature is off keep the usual message
        #[allow(unreachable_patterns)] // reachable only with features off
        Format::Csv | Format::Excel | Format::Ods => Err(parse_error(format!(
            "format '{}' is not compiled into this build",
            format.id()
        ))),
        other => Err(parse_error(format!(
            "'{}' is not a table format (csv, excel, ods)",
            other.id()
        ))),
    }
}

pub(crate) fn parse(
    format: Format,
    text: &str,
    layout: &TableLayout,
    path: &Path,
    options: &Options,
) -> Result<Value> {
    let _ = (text, layout, options); // unused when few formats are compiled in
    match format {
        // binary formats never come through the text pipeline; a string
        // source naming one lands here
        Format::Excel | Format::Ods => Err(Error::Parse {
            path: path.to_path_buf(),
            message: format!("'{}' is a binary format — file sources only", format.id()),
        }),
        #[cfg(feature = "json")]
        Format::Json => json::parse(text, path),
        #[cfg(feature = "jsonc")]
        Format::Jsonc => jsonc::parse(text, path),
        #[cfg(feature = "yaml")]
        Format::Yaml => yaml::parse(text, path),
        #[cfg(feature = "toml")]
        Format::Toml => toml::parse(text, path),
        #[cfg(feature = "ini")]
        Format::Ini => ini::parse(text, path),
        #[cfg(feature = "env")]
        Format::Env => env::parse(text, options),
        #[cfg(feature = "csv")]
        Format::Csv => csv::parse(text, layout, path, options),
        #[allow(unreachable_patterns)]
        other => Err(Error::Parse {
            path: path.to_path_buf(),
            message: format!("format '{}' is not compiled into this build", other.id()),
        }),
    }
}

/// Convert a parsed serde_json tree into a [`Value`] (strict json and the
/// table `json` cell type come through here; jsonc converts its parser's
/// own value type and needs no serde_json).
#[cfg(feature = "json")]
pub(crate) fn from_serde_json(value: serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                Value::Uint(u)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(items) => {
            Value::Array(items.into_iter().map(from_serde_json).collect())
        }
        serde_json::Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, from_serde_json(v)))
                .collect(),
        ),
    }
}

/// One array suffix of a dot_key segment: `[]` (append) or `[<int>]`
/// (index).
enum ArrayPart {
    Append,
    Index(usize),
}

/// Split one dot_key segment into its base name and chained array
/// suffixes. Only the exact shape addresses arrays: a non-empty base
/// name, then one or more back-to-back `[]` / `[<digits>]` groups
/// (indexes fitting `usize`) running to the segment's end — each group
/// is one nesting level (`a[1][2]`). Any violation anywhere makes the
/// whole segment a literal key (`None`).
fn split_segment(segment: &str) -> Option<(&str, Vec<ArrayPart>)> {
    let open = segment.find('[')?;
    let base = &segment[..open];
    if base.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    let mut rest = &segment[open..];
    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let (digits, after) = inner.split_once(']')?;
        if digits.is_empty() {
            parts.push(ArrayPart::Append);
        } else if digits.bytes().all(|b| b.is_ascii_digit()) {
            parts.push(ArrayPart::Index(digits.parse().ok()?));
        } else {
            return None;
        }
        rest = after;
    }
    Some((base, parts))
}

/// Merge `value` into `root` under `key` (env and table formats). With
/// `dot_key` on, `a.b.c` expands into nested objects and a segment's
/// array suffixes address arrays — `name[]` appends one new element
/// per occurrence, `name[<int>]` addresses that element, growing the
/// array with `Null`s, and chained suffixes nest (see
/// [`split_segment`]). This walks the tree built so far instead of
/// expand-then-merge because append has to see the existing array.
/// With `dot_key` off the whole key is one literal object key.
pub(crate) fn insert_key(root: &mut Value, key: &str, value: Value, dot_key: bool) {
    if dot_key {
        insert_segments(root, key.split('.'), value);
    } else {
        deep_merge(
            root,
            Value::Object(std::collections::BTreeMap::from([(key.to_owned(), value)])),
        );
    }
}

fn insert_segments<'a>(
    slot: &mut Value,
    mut segments: impl Iterator<Item = &'a str>,
    value: Value,
) {
    let Some(segment) = segments.next() else {
        deep_merge(slot, value);
        return;
    };
    // a segment's base name lives in an object; on a kind collision the
    // later row wins (merge rule 2), so anything else is replaced
    if !matches!(slot, Value::Object(_)) {
        *slot = Value::Object(Default::default());
    }
    let Value::Object(entries) = slot else {
        unreachable!()
    };
    let (name, parts) = match split_segment(segment) {
        Some((name, parts)) => (name, parts),
        None => (segment, Vec::new()), // literal key, no array suffixes
    };
    let mut target = entries.entry(name.to_owned()).or_insert(Value::Null);
    for part in parts {
        // each suffix descends one array level
        if !matches!(target, Value::Array(_)) {
            *target = Value::Array(Vec::new());
        }
        let Value::Array(items) = target else {
            unreachable!()
        };
        let index = match part {
            ArrayPart::Append => items.len(),
            ArrayPart::Index(index) => index,
        };
        while items.len() <= index {
            items.push(Value::Null); // unfilled gaps stay Null
        }
        target = &mut items[index];
    }
    insert_segments(target, segments, value);
}

/// Plain-value deep merge (objects recurse, everything else replaces) —
/// used when a format merges its own rows/lines into one file value.
pub(crate) fn deep_merge(target: &mut Value, incoming: Value) {
    match (target, incoming) {
        (Value::Object(entries), Value::Object(incoming)) => {
            for (key, value) in incoming {
                match entries.get_mut(&key) {
                    Some(slot) => deep_merge(slot, value),
                    None => {
                        entries.insert(key, value);
                    }
                }
            }
        }
        (slot, value) => *slot = value,
    }
}
