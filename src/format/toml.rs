//! TOML.

use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let parsed: toml::Value = text.parse().map_err(|e: toml::de::Error| Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(convert(parsed))
}

fn convert(value: toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Int(i),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        // follows the dt value-parser feature, like the table `dt` type
        #[cfg(feature = "datetime")]
        toml::Value::Datetime(dt) => Value::DateTime(dt.to_string()),
        #[cfg(not(feature = "datetime"))]
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(convert).collect()),
        toml::Value::Table(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, convert(v))).collect())
        }
    }
}
