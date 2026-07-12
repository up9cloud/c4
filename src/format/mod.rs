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

// tree mode runs extensionless files through the same auto detection
#[cfg(feature = "tree")]
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

/// Wrap `value` under `key`, expanding `a.b.c` into nested objects when
/// `dot_key` is on (env and table formats).
pub(crate) fn expand_key(key: &str, value: Value, dot_key: bool) -> Value {
    use std::collections::BTreeMap;
    if dot_key && key.contains('.') {
        let mut value = value;
        for part in key.split('.').rev() {
            value = Value::Object(BTreeMap::from([(part.to_owned(), value)]));
        }
        value
    } else {
        Value::Object(std::collections::BTreeMap::from([(key.to_owned(), value)]))
    }
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
