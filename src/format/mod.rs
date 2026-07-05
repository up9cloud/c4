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

use crate::{Error, Format, Options, Result, Value};

pub(crate) fn parse(format: Format, text: &str, path: &Path, options: &Options) -> Result<Value> {
    let _ = (text, options); // unused when few formats are compiled in
    match format {
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
        Format::Csv => csv::parse(text, path, options),
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
