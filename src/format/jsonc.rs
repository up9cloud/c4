//! JSONC — JSON with comments and trailing commas (a JSON superset).
//! Converts jsonc-parser's own value type directly, so this format needs
//! no serde_json dependency.

use std::path::Path;

use jsonc_parser::JsonValue;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let parsed =
        jsonc_parser::parse_to_value(text, &Default::default()).map_err(|e| Error::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    match parsed {
        Some(value) => convert(value, path),
        None => Ok(Value::Null), // empty document
    }
}

fn convert(value: JsonValue<'_>, path: &Path) -> Result<Value> {
    Ok(match value {
        JsonValue::Null => Value::Null,
        JsonValue::Boolean(b) => Value::Bool(b),
        JsonValue::String(s) => Value::String(s.into_owned()),
        // the parser keeps numbers as raw text
        JsonValue::Number(raw) => {
            if let Ok(i) = raw.parse::<i64>() {
                Value::Int(i)
            } else if let Ok(u) = raw.parse::<u64>() {
                Value::Uint(u)
            } else if let Ok(f) = raw.parse::<f64>() {
                Value::Float(f)
            } else {
                return Err(Error::Parse {
                    path: path.to_path_buf(),
                    message: format!("invalid number literal: {raw}"),
                });
            }
        }
        JsonValue::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|v| convert(v, path))
                .collect::<Result<_>>()?,
        ),
        JsonValue::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| Ok((k, convert(v, path)?)))
                .collect::<Result<_>>()?,
        ),
    })
}
